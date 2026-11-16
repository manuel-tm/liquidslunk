//! The player (backend.md, "The player"): the queue, on a thread of its own,
//! and the engine that plays it. It takes commands, makes the engine follow
//! the queue, and takes the queue's own steps as the engine says what's
//! heard. What changes goes to its owner through `Options::notify`.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::Error;
use crate::engine::{Engine, Event, Fade, Stamped, Track};
use crate::output::{Output, default_config};
use crate::queue::{Entry, Queue, Repeat, Step};
use crate::render::Renderer;

/// Entries given to the engine to follow the current one, so even short
/// tracks join.
const AHEAD: usize = 3;
/// How often the player looks at the engine while it has audio.
const TICK: Duration = Duration::from_millis(50);
/// How often the time is said while a track plays.
const TIME_EVERY: Duration = Duration::from_millis(250);
/// How long the engine goes without samples before the player says it
/// waits.
const WAITING_AFTER: Duration = Duration::from_millis(250);
/// How long the output runs on once nothing plays: long enough for a fade
/// and the output's latency to be heard.
const RESTING_AFTER: Duration = Duration::from_secs(2);
/// Volume Up and Volume Down's step.
pub const VOLUME_STEP: f64 = 0.1;

/// What the player asks of the library.
pub trait Library: Send + 'static {
    /// A track's file, or none when the track isn't there.
    fn path(&self, track_id: i64) -> Option<PathBuf>;
    /// Which of these tracks are still there.
    fn existing(&self, track_ids: &[i64]) -> Vec<i64>;
}

/// Where the player plays.
pub enum Device {
    /// The system's default output
    System,
    /// A renderer at this rate, handed over each time the player makes an
    /// engine, for its owner to play: for tests
    Given {
        rate: u32,
        renderers: Sender<Renderer>,
    },
}

pub struct Options {
    pub library: Box<dyn Library>,
    /// Hears what changes, on the player's thread
    pub notify: Box<dyn FnMut(Update) + Send>,
    pub device: Device,
}

/// The player, as it is.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// In the order they play
    pub entries: Vec<Entry>,
    /// The current entry's index, with something queued
    pub current: Option<usize>,
    pub playing: bool,
    /// Whether it plays, but waits for its file
    pub waiting: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// From 0 to 1, in hundredths
    pub volume: f64,
    pub muted: bool,
    pub fade: Fade,
    /// The queue to remember, as `Queue::remembered()` writes it
    pub remembered: Option<String>,
}

impl State {
    pub fn current_entry(&self) -> Option<Entry> {
        self.current
            .and_then(|index| self.entries.get(index).copied())
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            playing: false,
            waiting: false,
            shuffle: false,
            repeat: Repeat::Off,
            volume: 1.0,
            muted: false,
            fade: Fade::default(),
            remembered: None,
        }
    }
}

/// A track that can't play.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub track_id: i64,
    /// Its file, if the library has one
    pub path: Option<PathBuf>,
    pub error: Error,
}

/// What changed.
#[derive(Debug, Clone, PartialEq)]
pub enum Update {
    State(State),
    /// Seconds of the current track heard
    Time(f64),
    Failed(Failure),
}

/// What the player is told to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// What its owner remembers, once, as the library opens: a queue as
    /// `Queue::remembered()` writes it, which comes back paused, and the
    /// settings. Later ones are ignored: the player has its own by then.
    Restore {
        queue: Option<String>,
        volume: f64,
        muted: bool,
        shuffle: bool,
        repeat: Repeat,
        fade: Fade,
    },
    Play {
        track_ids: Vec<i64>,
        start: Option<i64>,
    },
    PlayNext(Vec<i64>),
    Add(Vec<i64>),
    Toggle,
    SetPlaying(bool),
    Next,
    Previous,
    Jump(u64),
    Move {
        key: u64,
        by: i64,
    },
    Remove(u64),
    Clear,
    SetShuffle(bool),
    SetRepeat(Repeat),
    /// From 0 to 1
    SetVolume {
        volume: f64,
        muted: bool,
    },
    /// Changes the volume by this much, and unmutes
    ChangeVolume(f64),
    SetFade(Fade),
    /// Tracks may have been deleted: their entries leave the queue
    Prune,
}

enum Message {
    Command(Command),
    Engine(Stamped),
    Quit,
}

/// The player: a handle to its thread, which stops when it's dropped.
pub struct Player {
    messages: Sender<Message>,
    shown: Arc<Mutex<Shown>>,
    thread: Option<JoinHandle<()>>,
}

/// What the player last said, for its owner to read at any time.
#[derive(Debug, Clone, Default)]
struct Shown {
    state: State,
    time: f64,
}

impl Player {
    /// Starts the player, with nothing queued. It opens the output only
    /// once something plays.
    pub fn start(options: Options) -> Player {
        let (messages, received) = mpsc::channel();
        let shown = Arc::new(Mutex::new(Shown::default()));
        let worker = Worker {
            queue: Queue::default(),
            library: options.library,
            notify: options.notify,
            device: options.device,
            audio: None,
            messages: received,
            sender: messages.clone(),
            shown: shown.clone(),
            loaded: None,
            upcoming: Vec::new(),
            failed_ahead: HashMap::new(),
            volume: 1.0,
            muted: false,
            fade: Fade::default(),
            restored: false,
            published: None,
            told: None,
            told_at: Instant::now(),
            starved_since: None,
            waiting: false,
            idle_since: None,
            resting: false,
            random: RandomState::new().hash_one(Instant::now()) | 1,
        };
        let thread = std::thread::Builder::new()
            .name("slunk-player".into())
            .spawn(move || worker.run())
            .ok();
        Player {
            messages,
            shown,
            thread,
        }
    }

    /// Hands the player a command, which it takes in order.
    pub fn send(&self, command: Command) {
        let _ = self.messages.send(Message::Command(command));
    }

    /// The player as it last said it was.
    pub fn state(&self) -> State {
        lock(&self.shown).state.clone()
    }

    /// Seconds of the current track heard, as the player last said.
    pub fn time(&self) -> f64 {
        lock(&self.shown).time
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.messages.send(Message::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The engine, and the output that plays its renderer.
struct Audio {
    engine: Engine,
    /// None when the renderer was given to the owner
    output: Option<Output>,
}

/// The entry the engine plays from.
#[derive(Debug, Clone, Copy)]
struct Loaded {
    key: u64,
    /// The queue's restarts when it was loaded: a restart loads it again
    restarts: u64,
    /// Whether it has been heard
    heard: bool,
}

struct Worker {
    queue: Queue,
    library: Box<dyn Library>,
    notify: Box<dyn FnMut(Update) + Send>,
    device: Device,
    /// Made when something first plays, and again after the output is lost
    audio: Option<Audio>,
    messages: Receiver<Message>,
    /// For the engine's events
    sender: Sender<Message>,
    shown: Arc<Mutex<Shown>>,
    loaded: Option<Loaded>,
    /// What the engine was last given to follow the loaded entry, by key
    upcoming: Vec<u64>,
    /// Entries given to follow it that couldn't play, with why: they're
    /// skipped, and their failure said, once the queue reaches them
    failed_ahead: HashMap<u64, Failure>,
    volume: f64,
    muted: bool,
    fade: Fade,
    restored: bool,
    published: Option<State>,
    /// The time last said, and when
    told: Option<f64>,
    told_at: Instant,
    /// Since when the engine has had nothing to play
    starved_since: Option<Instant>,
    waiting: bool,
    /// Since when nothing has played
    idle_since: Option<Instant>,
    /// The output is paused, for want of anything to play
    resting: bool,
    /// For shuffling's seeds
    random: u64,
}

impl Worker {
    fn run(mut self) {
        loop {
            let message = if self.audio.is_some() && !self.resting {
                self.messages.recv_timeout(TICK)
            } else {
                self.messages
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected)
            };
            match message {
                Ok(Message::Command(command)) => self.command(command),
                Ok(Message::Engine(stamped)) => self.engine_event(stamped),
                Ok(Message::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.tick();
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Restore {
                queue,
                volume,
                muted,
                shuffle,
                repeat,
                fade,
            } => {
                if self.restored {
                    return;
                }
                self.restored = true;
                self.queue = Queue::restored(queue.as_deref(), shuffle, repeat);
                self.set_volume(volume, muted);
                self.set_fade(fade);
            }
            Command::Play { track_ids, start } => {
                let seed = self.seed();
                self.queue.step(Step::Play {
                    track_ids,
                    start,
                    seed,
                });
            }
            Command::PlayNext(track_ids) => self.queue.step(Step::PlayNext { track_ids }),
            Command::Add(track_ids) => self.queue.step(Step::Add { track_ids }),
            Command::Toggle => self.queue.step(Step::Toggle),
            Command::SetPlaying(playing) => self.queue.step(Step::SetPlaying(playing)),
            Command::Next => self.queue.step(Step::Next),
            Command::Previous => {
                let time = self.heard_time();
                self.queue.step(Step::Previous { time });
            }
            Command::Jump(key) => self.queue.step(Step::Jump { key }),
            Command::Move { key, by } => self.queue.step(Step::Move { key, by }),
            Command::Remove(key) => self.queue.step(Step::Remove { key }),
            Command::Clear => self.queue.step(Step::Clear),
            Command::SetShuffle(shuffle) => {
                let seed = self.seed();
                self.queue.step(Step::SetShuffle { shuffle, seed });
            }
            Command::SetRepeat(repeat) => self.queue.step(Step::SetRepeat(repeat)),
            Command::SetVolume { volume, muted } => self.set_volume(volume, muted),
            Command::ChangeVolume(by) => self.set_volume(self.volume + by, false),
            Command::SetFade(fade) => self.set_fade(fade),
            Command::Prune => self.prune(),
        }
        self.follow();
        self.publish();
        self.tell_time();
    }

    fn engine_event(&mut self, stamped: Stamped) {
        let Some(audio) = &self.audio else { return };
        // About what played before the last play or stop
        if stamped.epoch != audio.engine.epoch() {
            return;
        }
        match stamped.event {
            Event::Started { entry } => self.started(entry),
            Event::Failed { entry, error } => self.failed(entry, error),
            Event::Ended => self.ended(),
        }
        self.follow();
        self.publish();
        self.tell_time();
    }

    /// An entry was heard: the current one, from its start, or one that
    /// follows it, joined to its end.
    fn started(&mut self, entry: u64) {
        // The engine took what followed the last entry: it's given again
        self.upcoming.clear();
        for _ in 0..=self.queue.entries.len() {
            let Some(current) = self.queue.current_entry() else {
                break;
            };
            if current.key == entry {
                return self.heard(entry);
            }
            // What failed ahead is skipped as it would have played
            let Some(next) = self.queue.up_next() else {
                break;
            };
            let failure = self.failed_ahead.remove(&next.key);
            if next.key != entry && failure.is_none() {
                break;
            }
            self.queue.step(Step::Handoff);
            if let Some(failure) = failure {
                (self.notify)(Update::Failed(failure));
                self.queue.step(Step::Failed);
            }
        }
        // The queue changed as it was heard: it follows what's heard, if
        // it's still queued, and otherwise loads its own current entry
        if let Some(index) = self.queue.entries.iter().position(|e| e.key == entry) {
            self.queue.current = Some(index);
            self.queue.playing = true;
            self.heard(entry);
        }
    }

    fn heard(&mut self, entry: u64) {
        self.queue.step(Step::Started);
        self.loaded = Some(Loaded {
            key: entry,
            restarts: self.queue.restarts,
            heard: true,
        });
    }

    /// An entry can't play, and the engine waits to be told what follows.
    fn failed(&mut self, entry: u64, error: Error) {
        let Some(track_id) = self
            .queue
            .entries
            .iter()
            .find(|queued| queued.key == entry)
            .map(|queued| queued.track_id)
        else {
            return;
        };
        let failure = Failure {
            track_id,
            path: self.library.path(track_id),
            error,
        };
        match self.loaded {
            Some(loaded) if loaded.key == entry => {
                (self.notify)(Update::Failed(failure));
                self.upcoming.clear();
                if !loaded.heard {
                    // It couldn't start: the queue goes on without it
                    self.loaded = None;
                    self.queue.step(Step::Failed);
                }
                // Otherwise it broke off as it played, and what follows
                // joins where it ends
            }
            _ if self.upcoming.contains(&entry) => {
                self.failed_ahead.insert(entry, failure);
                self.upcoming.clear();
            }
            _ => {}
        }
    }

    /// Everything the engine was given was heard: the current track ended
    /// with nothing joined to it.
    fn ended(&mut self) {
        let Some(loaded) = self.loaded.take() else {
            return;
        };
        self.upcoming.clear();
        if self.queue.current_entry().map(|entry| entry.key) != Some(loaded.key) {
            return;
        }
        self.queue.step(Step::Ended);
        for _ in 0..self.queue.entries.len() {
            let Some(current) = self.queue.current_entry() else {
                break;
            };
            let Some(failure) = self.failed_ahead.remove(&current.key) else {
                break;
            };
            (self.notify)(Update::Failed(failure));
            self.queue.step(Step::Failed);
        }
    }

    /// The engine follows the queue: it plays the current entry from its
    /// start once it changes, or restarts, and what follows it as that
    /// changes. A paused entry is loaded only once it plays.
    fn follow(&mut self) {
        let Some(entry) = self.queue.current_entry() else {
            self.failed_ahead.clear();
            self.unload();
            return;
        };
        let moved = self
            .loaded
            .is_none_or(|loaded| loaded.key != entry.key || loaded.restarts != self.queue.restarts);
        if moved {
            if self.queue.playing {
                self.load(entry);
            } else {
                self.unload();
            }
        } else {
            self.give_upcoming();
        }
        if self.queue.playing
            && self.loaded.is_some()
            && let Err(error) = self.wake()
        {
            self.fail_output(entry, error);
        }
        if let Some(audio) = &self.audio {
            audio.engine.set_playing(self.queue.playing);
        }
    }

    /// Plays an entry from its start, with what follows it.
    fn load(&mut self, entry: Entry) {
        if let Err(error) = self.wake() {
            return self.fail_output(entry, error);
        }
        self.failed_ahead.clear();
        let following: Vec<Entry> = self.queue.following().take(AHEAD).collect();
        let tracks = std::iter::once(entry)
            .chain(following.iter().copied())
            .map(|entry| self.track(entry))
            .collect();
        self.upcoming = following.iter().map(|entry| entry.key).collect();
        if let Some(audio) = &self.audio {
            audio.engine.play(tracks);
        }
        self.loaded = Some(Loaded {
            key: entry.key,
            restarts: self.queue.restarts,
            heard: false,
        });
    }

    fn unload(&mut self) {
        if self.loaded.take().is_some()
            && let Some(audio) = &self.audio
        {
            audio.engine.stop();
        }
        self.upcoming.clear();
    }

    /// Gives the engine what follows the loaded entry, when that changed.
    fn give_upcoming(&mut self) {
        let Some(loaded) = self.loaded else { return };
        let following: Vec<Entry> = self
            .queue
            .following()
            .filter(|entry| !self.failed_ahead.contains_key(&entry.key))
            .take(AHEAD)
            .collect();
        let keys: Vec<u64> = following.iter().map(|entry| entry.key).collect();
        if keys == self.upcoming {
            return;
        }
        let tracks = following
            .into_iter()
            .map(|entry| self.track(entry))
            .collect();
        if let Some(audio) = &self.audio {
            audio.engine.set_upcoming(loaded.key, tracks);
        }
        self.upcoming = keys;
    }

    /// An entry's track, for the engine. One that isn't in the library has
    /// no path, so it fails as a missing file does.
    fn track(&self, entry: Entry) -> Track {
        Track::new(
            entry.key,
            self.library.path(entry.track_id).unwrap_or_default(),
        )
    }

    /// There's no output to play to: the player pauses, and says why.
    fn fail_output(&mut self, entry: Entry, error: Error) {
        (self.notify)(Update::Failed(Failure {
            track_id: entry.track_id,
            path: self.library.path(entry.track_id),
            error,
        }));
        self.queue.step(Step::SetPlaying(false));
        self.loaded = None;
        self.upcoming.clear();
    }

    /// The engine and its output, ready to play: made, or woken from rest.
    fn wake(&mut self) -> Result<(), Error> {
        self.idle_since = None;
        if let Some(audio) = &self.audio {
            if self.resting {
                if let Some(output) = &audio.output {
                    output.resume()?;
                }
                self.resting = false;
            }
            return Ok(());
        }
        let sender = self.sender.clone();
        let events = move |stamped| {
            let _ = sender.send(Message::Engine(stamped));
        };
        let audio = match &self.device {
            Device::System => {
                let (rate, _) = default_config()?;
                let (engine, renderer) = Engine::new(rate, events);
                let output = Output::start(renderer)?;
                Audio {
                    engine,
                    output: Some(output),
                }
            }
            Device::Given { rate, renderers } => {
                let (engine, renderer) = Engine::new(*rate, events);
                let _ = renderers.send(renderer);
                Audio {
                    engine,
                    output: None,
                }
            }
        };
        audio.engine.set_volume(self.gain());
        audio.engine.set_fade(self.fade);
        audio.engine.set_playing(false);
        self.audio = Some(audio);
        self.resting = false;
        Ok(())
    }

    /// Looks at the engine and the output: whether it waits, the time, the
    /// output lost, and resting once nothing plays.
    fn tick(&mut self) {
        let Some(audio) = &self.audio else { return };
        let lost = audio
            .output
            .as_ref()
            .is_some_and(|output| output.errors().iter().any(|error| error.lost));
        let starved = self.loaded.is_some() && audio.engine.starved();
        if lost {
            return self.lose_output();
        }
        self.starved_since = match (starved, self.starved_since) {
            (false, _) => None,
            (true, None) => Some(Instant::now()),
            (true, since) => since,
        };
        let waiting = self
            .starved_since
            .is_some_and(|since| since.elapsed() >= WAITING_AFTER);
        if waiting != self.waiting {
            self.waiting = waiting;
            self.publish();
        }
        if self.queue.playing {
            self.idle_since = None;
            if self.told_at.elapsed() >= TIME_EVERY {
                self.tell_time();
            }
        } else if !self.resting {
            let since = *self.idle_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= RESTING_AFTER {
                self.rest();
            }
        }
    }

    /// The output's device is gone, with none to follow: the player pauses,
    /// and makes the engine and its output again at the next play.
    fn lose_output(&mut self) {
        self.audio = None;
        self.loaded = None;
        self.upcoming.clear();
        self.resting = false;
        self.starved_since = None;
        self.waiting = false;
        self.queue.step(Step::SetPlaying(false));
        self.publish();
        self.tell_time();
    }

    /// Pauses the output, so the device rests while nothing plays.
    fn rest(&mut self) {
        if let Some(output) = self.audio.as_ref().and_then(|audio| audio.output.as_ref()) {
            let _ = output.pause();
        }
        self.resting = true;
        // What's heard has caught up with where it paused
        self.tell_time();
    }

    fn set_volume(&mut self, volume: f64, muted: bool) {
        // In hundredths, so steps of 10% don't drift
        self.volume = (volume.clamp(0.0, 1.0) * 100.0).round() / 100.0;
        self.muted = muted;
        if let Some(audio) = &self.audio {
            audio.engine.set_volume(self.gain());
        }
    }

    fn gain(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume as f32 }
    }

    fn set_fade(&mut self, fade: Fade) {
        self.fade = fade;
        if let Some(audio) = &self.audio {
            audio.engine.set_fade(fade);
        }
    }

    /// Entries whose tracks are gone leave the queue.
    fn prune(&mut self) {
        let ids: Vec<i64> = self
            .queue
            .entries
            .iter()
            .map(|entry| entry.track_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if ids.is_empty() {
            return;
        }
        let existing = self.library.existing(&ids);
        if existing.len() < ids.len() {
            self.queue.step(Step::Keep {
                track_ids: existing,
            });
        }
    }

    /// Seconds of the current entry heard.
    fn heard_time(&self) -> f64 {
        let (Some(audio), Some(loaded)) = (&self.audio, self.loaded) else {
            return 0.0;
        };
        let position = audio.engine.position();
        if position.entry != Some(loaded.key) {
            return 0.0;
        }
        position.frames as f64 / f64::from(position.rate)
    }

    /// Says the player's state, when it changed.
    fn publish(&mut self) {
        let state = State {
            entries: self.queue.entries.clone(),
            current: self.queue.current,
            playing: self.queue.playing,
            waiting: self.waiting,
            shuffle: self.queue.shuffle,
            repeat: self.queue.repeat,
            volume: self.volume,
            muted: self.muted,
            fade: self.fade,
            remembered: self.queue.remembered(),
        };
        if self.published.as_ref() == Some(&state) {
            return;
        }
        lock(&self.shown).state = state.clone();
        self.published = Some(state.clone());
        (self.notify)(Update::State(state));
    }

    /// Says the time, when it changed.
    fn tell_time(&mut self) {
        self.told_at = Instant::now();
        let time = self.heard_time();
        lock(&self.shown).time = time;
        if self.told == Some(time) {
            return;
        }
        self.told = Some(time);
        (self.notify)(Update::Time(time));
    }

    /// A seed for shuffling (xorshift64*).
    fn seed(&mut self) -> u32 {
        self.random ^= self.random >> 12;
        self.random ^= self.random << 25;
        self.random ^= self.random >> 27;
        (self.random.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }
}
