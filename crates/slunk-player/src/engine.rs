//! The engine: a decoder thread that writes each track's samples right after
//! the last one's, and the renderer that plays them (backend.md, "Joining
//! tracks").

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::Error;
use crate::convert::Converter;
use crate::decode::TrackDecoder;
use crate::render::{MARKERS, Renderer};
use crate::stream::{Marker, NONE, Played, Shared};

const CHANNELS: usize = 2;
/// How far the decoder stays ahead of what plays, in seconds.
const AHEAD: f64 = 2.0;
/// The ring's size, in seconds: room for what's ahead, and for what a change
/// to what's up next writes after it.
const RING: f64 = 5.0;
/// How long the decoder waits when there's nothing to do, and while
/// paused.
const IDLE: Duration = Duration::from_millis(5);
const IDLE_PAUSED: Duration = Duration::from_millis(50);

/// A track to play: an entry, which the events name, and its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// What the player calls it; a track queued twice is two entries
    pub entry: u64,
    pub path: PathBuf,
    /// Frames of the file to leave out from its start
    pub start: u64,
}

impl Track {
    pub fn new(entry: u64, path: impl Into<PathBuf>) -> Self {
        Self {
            entry,
            path: path.into(),
            start: 0,
        }
    }
}

/// What the engine says happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// An entry's first sample was heard: rendered, and past the output's
    /// latency.
    Started { entry: u64 },
    /// An entry couldn't play. What was to follow it is dropped: the owner
    /// says what follows instead, with `set_upcoming()`.
    Failed { entry: u64, error: Error },
    /// Everything given was heard to its end.
    Ended,
}

/// An event, with the play it belongs to: each `play()` and `stop()` starts
/// a new epoch, and an event of an older one is about what's no longer
/// playing.
#[derive(Debug, Clone, PartialEq)]
pub struct Stamped {
    pub epoch: u64,
    pub event: Event,
}

/// The fade's length, where the player cuts the audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fade {
    Off,
    #[default]
    Ms10,
    Ms50,
    Ms200,
}

impl Fade {
    fn frames(self, rate: u32) -> u32 {
        let ms = match self {
            Fade::Off => 0,
            Fade::Ms10 => 10,
            Fade::Ms50 => 50,
            Fade::Ms200 => 200,
        };
        rate * ms / 1000
    }
}

/// Where the engine is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The entry playing
    pub entry: Option<u64>,
    /// Frames of it played, at the output's rate
    pub frames: u64,
    pub rate: u32,
}

enum Command {
    Play(Vec<Track>),
    Upcoming { after: u64, tracks: Vec<Track> },
    Stop,
}

/// The engine, which the player commands. Its renderer goes to the output.
pub struct Engine {
    commands: Sender<Command>,
    shared: Arc<Shared>,
    rate: u32,
    /// Plays and stops sent, which the decoder counts as it takes them
    epoch: AtomicU64,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// An engine for an output at `rate`, and its renderer. `events` hears
    /// what happens, on the decoder thread.
    pub fn new(rate: u32, events: impl FnMut(Stamped) + Send + 'static) -> (Self, Renderer) {
        let (samples_in, samples_out) =
            RingBuffer::new((f64::from(rate) * RING) as usize * CHANNELS);
        let (markers_in, markers_out) = RingBuffer::new(MARKERS);
        let (played_in, played_out) = RingBuffer::new(MARKERS);
        let shared = Arc::new(Shared::new(Fade::default().frames(rate)));
        let renderer = Renderer::new(samples_out, markers_out, played_in, shared.clone(), rate);
        let (commands, commands_out) = mpsc::channel();
        let worker = Worker {
            rate,
            samples: samples_in,
            markers: markers_in,
            played: played_out,
            shared: shared.clone(),
            events: Box::new(events),
            epoch: 0,
            commands: commands_out,
            written: 0,
            pending: Vec::new(),
            pending_from: 0,
            outbox: VecDeque::new(),
            placed: Vec::new(),
            playing: 0,
            head: None,
            starts: VecDeque::new(),
            upcoming: VecDeque::new(),
            current: None,
            converter: None,
            converter_start: 0,
            scratch: Vec::new(),
            done: true,
            ended: true,
        };
        let thread = std::thread::Builder::new()
            .name("slunk-player decoder".into())
            .spawn(move || worker.run())
            .ok();
        let engine = Self {
            commands,
            shared,
            rate,
            epoch: AtomicU64::new(0),
            thread,
        };
        (engine, renderer)
    }

    /// Plays `tracks` in order from now, after what plays fades out. Its
    /// events are of a new epoch.
    pub fn play(&self, tracks: Vec<Track>) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        let _ = self.commands.send(Command::Play(tracks));
    }

    /// What follows entry `after`, in order, once `after` is the entry
    /// playing: the one heard last, or since a play, the first one given.
    /// What's decoded of entries no longer in it is dropped unheard. Once
    /// another entry plays, it's out of date, and ignored.
    pub fn set_upcoming(&self, after: u64, tracks: Vec<Track>) {
        let _ = self.commands.send(Command::Upcoming { after, tracks });
    }

    /// Stops, after what plays fades out. What it says next is of a new
    /// epoch.
    pub fn stop(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        let _ = self.commands.send(Command::Stop);
    }

    /// The epoch of the last play or stop: events of others are out of
    /// date.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }

    pub fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::Release);
    }

    pub fn playing(&self) -> bool {
        self.shared.playing.load(Ordering::Acquire)
    }

    /// The volume, from 0 to 1, which changes over 10 ms.
    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.shared
            .volume
            .store(volume.to_bits(), Ordering::Relaxed);
    }

    pub fn set_fade(&self, fade: Fade) {
        self.shared
            .fade
            .store(fade.frames(self.rate), Ordering::Relaxed);
    }

    /// The output's rate.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// What's heard: the entry, and how far into it.
    pub fn position(&self) -> Position {
        let entry = self.shared.entry.load(Ordering::Acquire);
        let start = self.shared.entry_start.load(Ordering::Acquire);
        let heard = self.shared.heard.load(Ordering::Acquire);
        Position {
            entry: (entry != NONE).then_some(entry),
            frames: heard.saturating_sub(start),
            rate: self.rate,
        }
    }

    /// Whether it plays, has more to play, and has nothing written to play
    /// yet: a file opening, or a disk that can't keep up.
    pub fn starved(&self) -> bool {
        self.playing()
            && !self.shared.draining.load(Ordering::Acquire)
            && self.shared.written.load(Ordering::Acquire)
                <= self.shared.played.load(Ordering::Acquire)
    }

    /// Frames played as silence for want of samples, since the engine
    /// started.
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// From the callback to when what it wrote is heard, as the output
    /// reported it.
    pub fn latency(&self) -> Duration {
        Duration::from_nanos(self.shared.latency.load(Ordering::Relaxed))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // The worker stops when the commands' sender goes
        let (sender, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.commands, sender));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// An entry the decoder has started, and where its first frame is.
struct Placed {
    entry: u64,
    at: u64,
}

/// The entry playing, which `set_upcoming()` names.
#[derive(Clone, Copy)]
struct Head {
    entry: u64,
    /// It has been heard: since a play, the first entry isn't until then
    heard: bool,
}

/// The track being decoded.
struct Decoding {
    track: Track,
    decoder: TrackDecoder,
    /// Frames still to leave out from its start
    skip: u64,
}

struct Worker {
    rate: u32,
    samples: Producer<f32>,
    markers: Producer<Marker>,
    played: Consumer<Played>,
    shared: Arc<Shared>,
    events: Box<dyn FnMut(Stamped) + Send>,
    /// Plays and stops taken
    epoch: u64,
    commands: Receiver<Command>,
    /// Frames written to the ring
    written: u64,
    /// Samples not in the ring yet, for want of room
    pending: Vec<f32>,
    /// How many of `pending` are in the ring already
    pending_from: usize,
    /// Markers to queue once what they name is written
    outbox: VecDeque<(Marker, u64)>,
    /// The entries started since the last play, in order
    placed: Vec<Placed>,
    /// Which of them is heard
    playing: usize,
    /// The entry playing, if any
    head: Option<Head>,
    /// Entries rendered and not heard yet: an entry and where it starts
    starts: VecDeque<(u64, u64)>,
    /// What's to decode after them
    upcoming: VecDeque<Track>,
    current: Option<Decoding>,
    converter: Option<Converter>,
    /// Where the converter's output starts in the stream
    converter_start: u64,
    scratch: Vec<f32>,
    /// Nothing is left to decode
    done: bool,
    /// And it has all played, and `Ended` was said
    ended: bool,
}

impl Worker {
    fn run(mut self) {
        loop {
            loop {
                match self.commands.try_recv() {
                    Ok(command) => self.command(command),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            }
            self.hear();
            if !self.work() {
                // With everything heard, only a command can change anything
                let waited = if self.ended {
                    self.commands
                        .recv()
                        .map_err(|_| RecvTimeoutError::Disconnected)
                } else if self.shared.playing.load(Ordering::Relaxed) {
                    self.commands.recv_timeout(IDLE)
                } else {
                    self.commands.recv_timeout(IDLE_PAUSED)
                };
                match waited {
                    Ok(command) => self.command(command),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }

    fn command(&mut self, command: Command) {
        match command {
            Command::Play(tracks) => {
                self.cut();
                self.head = tracks.first().map(|track| Head {
                    entry: track.entry,
                    heard: false,
                });
                self.upcoming = tracks.into();
                self.set_done(false);
                self.ended = false;
            }
            Command::Upcoming { after, tracks } => {
                if self.head.is_some_and(|head| head.entry == after) {
                    self.set_upcoming(tracks);
                }
            }
            Command::Stop => {
                self.cut();
                self.set_done(true);
                self.ended = true;
            }
        }
    }

    fn emit(&mut self, event: Event) {
        (self.events)(Stamped {
            epoch: self.epoch,
            event,
        });
    }

    /// Drops what's buffered after what plays fades out, and starts a new
    /// epoch.
    fn cut(&mut self) {
        self.epoch += 1;
        self.pending.clear();
        self.pending_from = 0;
        self.outbox.clear();
        self.current = None;
        self.converter = None;
        self.upcoming.clear();
        self.placed.clear();
        self.playing = 0;
        self.head = None;
        self.starts.clear();
        let _ = self.markers.push(Marker::Cut {
            to: self.written,
            fade_in: false,
        });
        self.shared.entry.store(NONE, Ordering::Release);
    }

    /// An entry can't play: what was to follow it goes, until the owner
    /// says what follows instead.
    fn fail(&mut self, entry: u64, error: Error) {
        self.upcoming.clear();
        if self
            .head
            .is_some_and(|head| head.entry == entry && !head.heard)
        {
            self.head = None;
        }
        self.emit(Event::Failed { entry, error });
    }

    /// What follows the entry playing changed: what's decoded ahead stays as
    /// far as it agrees, and the rest is skipped.
    fn set_upcoming(&mut self, tracks: Vec<Track>) {
        if self.placed.is_empty() {
            // The entry playing isn't open yet: it stays first
            let head = self.upcoming.pop_front();
            self.upcoming = head.into_iter().chain(tracks).collect();
            if !self.upcoming.is_empty() {
                self.set_done(false);
                self.ended = false;
            }
            return;
        }
        let ahead = &self.placed[(self.playing + 1).min(self.placed.len())..];
        let agree = ahead
            .iter()
            .zip(&tracks)
            .take_while(|(placed, track)| placed.entry == track.entry)
            .count();
        if agree == ahead.len() {
            self.upcoming = tracks.into_iter().skip(agree).collect();
            if !self.upcoming.is_empty() {
                self.set_done(false);
                self.ended = false;
            }
            return;
        }
        // From the first entry that differs, what's decoded is skipped
        let from = ahead[agree].at;
        let keep = self.playing + 1 + agree;
        if self.current.is_some() && self.placed.len() > keep {
            self.current = None;
        }
        self.placed.truncate(keep);
        // What the converter holds of the current track's end comes out,
        // then everything after `from` goes
        if let Some(mut converter) = self.converter.take() {
            converter.finish(&mut self.pending);
        }
        let to = self.written + self.pending_frames();
        self.outbox.push_back((Marker::Skip { at: from, to }, to));
        self.upcoming = tracks.into_iter().skip(agree).collect();
        self.set_done(self.upcoming.is_empty() && self.current.is_none());
        self.ended = false;
    }

    /// Takes what the callback reports, and says what's heard.
    fn hear(&mut self) {
        while let Ok(Played::Started { entry, at }) = self.played.pop() {
            self.starts.push_back((entry, at));
        }
        let heard = self.shared.heard.load(Ordering::Acquire);
        while let Some(&(entry, at)) = self.starts.front() {
            if at > heard {
                break;
            }
            self.starts.pop_front();
            // One rendered before a cut, or skipped late, isn't placed any
            // more: it's not what plays
            let Some(index) = self
                .placed
                .iter()
                .position(|placed| placed.entry == entry && placed.at == at)
            else {
                continue;
            };
            self.playing = index;
            self.head = Some(Head { entry, heard: true });
            self.shared.entry_start.store(at, Ordering::Release);
            self.shared.entry.store(entry, Ordering::Release);
            self.emit(Event::Started { entry });
        }
        if self.done
            && !self.ended
            && self.pending_frames() == 0
            && self.outbox.is_empty()
            && self.starts.is_empty()
            && heard >= self.written
        {
            self.ended = true;
            self.emit(Event::Ended);
        }
    }

    /// One step of work: writing what's pending, or decoding a packet. False
    /// when there's nothing to do for now.
    fn work(&mut self) -> bool {
        if !self.flush() {
            return false;
        }
        let played = self.shared.played.load(Ordering::Acquire);
        if self.written.saturating_sub(played) as f64 >= AHEAD * f64::from(self.rate) {
            return false;
        }
        if self.current.is_none() && !self.open_next() {
            return false;
        }
        let Some(decoding) = self.current.as_mut() else {
            return true;
        };
        self.scratch.clear();
        match decoding.decoder.next(&mut self.scratch) {
            Ok(true) => {
                let mut frames = &self.scratch[..];
                if decoding.skip > 0 {
                    let dropped = (decoding.skip as usize).min(frames.len() / CHANNELS);
                    decoding.skip -= dropped as u64;
                    frames = &frames[dropped * CHANNELS..];
                }
                match self.converter.as_mut() {
                    Some(converter) => converter.push(frames, &mut self.pending),
                    None => self.pending.extend_from_slice(frames),
                }
            }
            Ok(false) => self.current = None,
            Err(error) => {
                let entry = decoding.track.entry;
                self.current = None;
                self.fail(entry, error);
            }
        }
        true
    }

    /// Opens the next track, and places it right after what's decoded. False
    /// when there's none.
    fn open_next(&mut self) -> bool {
        while let Some(track) = self.upcoming.pop_front() {
            let mut decoder = match TrackDecoder::open(&track.path) {
                Ok(decoder) => decoder,
                Err(error) => {
                    self.fail(track.entry, error);
                    continue;
                }
            };
            // AAC in MP4 says its rate only once it decodes
            self.scratch.clear();
            let first = match decoder.next(&mut self.scratch) {
                Ok(first) => first,
                Err(error) => {
                    self.fail(track.entry, error);
                    continue;
                }
            };
            let rate = decoder.rate();
            // A converter at the same rate goes on across the join
            let continues = self
                .converter
                .as_ref()
                .is_some_and(|converter| converter.from() == rate);
            if !continues {
                if let Some(mut converter) = self.converter.take() {
                    converter.finish(&mut self.pending);
                }
                if rate != self.rate && rate > 0 {
                    match Converter::new(rate, self.rate) {
                        Ok(converter) => {
                            self.converter_start = self.written + self.pending_frames();
                            self.converter = Some(converter);
                        }
                        Err(error) => {
                            self.fail(track.entry, error);
                            continue;
                        }
                    }
                }
            }
            let at = match self.converter.as_ref() {
                Some(converter) => self.converter_start + converter.output_at(converter.taken()),
                None => self.written + self.pending_frames(),
            };
            self.outbox.push_back((
                Marker::Start {
                    at,
                    entry: track.entry,
                },
                0,
            ));
            self.placed.push(Placed {
                entry: track.entry,
                at,
            });
            let mut decoding = Decoding {
                skip: track.start,
                track,
                decoder,
            };
            if first {
                let mut frames = &self.scratch[..];
                let dropped = (decoding.skip as usize).min(frames.len() / CHANNELS);
                decoding.skip -= dropped as u64;
                frames = &frames[dropped * CHANNELS..];
                match self.converter.as_mut() {
                    Some(converter) => converter.push(frames, &mut self.pending),
                    None => self.pending.extend_from_slice(frames),
                }
                self.current = Some(decoding);
            }
            self.set_done(false);
            return true;
        }
        if !self.done {
            if let Some(mut converter) = self.converter.take() {
                converter.finish(&mut self.pending);
            }
            self.set_done(true);
        }
        false
    }

    /// Writes what's pending, and queues the markers whose frames are
    /// written. False while the ring is full.
    fn flush(&mut self) -> bool {
        if self.pending_from < self.pending.len() {
            let rest = &self.pending[self.pending_from..];
            let room = self.samples.slots() / CHANNELS * CHANNELS;
            let count = rest.len().min(room);
            if count > 0 {
                if let Ok(mut chunk) = self.samples.write_chunk(count) {
                    let (first, second) = chunk.as_mut_slices();
                    first.copy_from_slice(&rest[..first.len()]);
                    second.copy_from_slice(&rest[first.len()..count]);
                    chunk.commit_all();
                }
                self.pending_from += count;
                self.written += (count / CHANNELS) as u64;
                self.shared.written.store(self.written, Ordering::Release);
            }
            if self.pending_from == self.pending.len() {
                self.pending.clear();
                self.pending_from = 0;
            }
        }
        while let Some(&(marker, ready)) = self.outbox.front() {
            if ready > self.written || self.markers.is_full() {
                break;
            }
            let _ = self.markers.push(marker);
            self.outbox.pop_front();
        }
        self.pending.is_empty()
    }

    /// Frames written to `pending` and not to the ring yet.
    fn pending_frames(&self) -> u64 {
        ((self.pending.len() - self.pending_from) / CHANNELS) as u64
    }

    /// Whether anything is left to decode, which the callback hears too:
    /// silence after the end isn't an underrun.
    fn set_done(&mut self, done: bool) {
        self.done = done;
        self.shared.draining.store(done, Ordering::Release);
    }
}
