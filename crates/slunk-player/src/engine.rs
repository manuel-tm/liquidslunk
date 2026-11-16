//! The engine: a decoder thread that writes each track's samples right after
//! the last one's, and the renderer that plays them (backend.md, "Joining
//! tracks").

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
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
/// How long the decoder waits when there's nothing to do.
const IDLE: Duration = Duration::from_millis(5);

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
    /// An entry's first sample played.
    Started { entry: u64 },
    /// An entry couldn't play, and was skipped.
    Failed { entry: u64, error: Error },
    /// Everything given played to its end.
    Ended,
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
    Upcoming(Vec<Track>),
    Stop,
}

/// The engine, which the player commands. Its renderer goes to the output.
pub struct Engine {
    commands: Sender<Command>,
    events: Receiver<Event>,
    shared: Arc<Shared>,
    rate: u32,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// An engine for an output at `rate`, and its renderer.
    pub fn new(rate: u32) -> (Self, Renderer) {
        let (samples_in, samples_out) =
            RingBuffer::new((f64::from(rate) * RING) as usize * CHANNELS);
        let (markers_in, markers_out) = RingBuffer::new(MARKERS);
        let (played_in, played_out) = RingBuffer::new(MARKERS);
        let shared = Arc::new(Shared::new(Fade::default().frames(rate)));
        let renderer = Renderer::new(samples_out, markers_out, played_in, shared.clone(), rate);
        let (commands, commands_out) = mpsc::channel();
        let (events_in, events) = mpsc::channel();
        let worker = Worker {
            rate,
            samples: samples_in,
            markers: markers_in,
            played: played_out,
            shared: shared.clone(),
            events: events_in,
            commands: commands_out,
            written: 0,
            pending: Vec::new(),
            pending_from: 0,
            outbox: VecDeque::new(),
            placed: Vec::new(),
            playing: 0,
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
            events,
            shared,
            rate,
            thread,
        };
        (engine, renderer)
    }

    /// Plays `tracks` in order from now, after what plays fades out.
    pub fn play(&self, tracks: Vec<Track>) {
        let _ = self.commands.send(Command::Play(tracks));
    }

    /// What follows the entry playing, in order. What's decoded of entries
    /// no longer in it is dropped unheard.
    pub fn set_upcoming(&self, tracks: Vec<Track>) {
        let _ = self.commands.send(Command::Upcoming(tracks));
    }

    /// Stops, after what plays fades out.
    pub fn stop(&self) {
        let _ = self.commands.send(Command::Stop);
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

    pub fn position(&self) -> Position {
        let entry = self.shared.entry.load(Ordering::Acquire);
        let start = self.shared.entry_start.load(Ordering::Relaxed);
        let played = self.shared.played.load(Ordering::Acquire);
        Position {
            entry: (entry != NONE).then_some(entry),
            frames: played.saturating_sub(start),
            rate: self.rate,
        }
    }

    /// Frames played as silence for want of samples, since the engine
    /// started.
    pub fn underruns(&self) -> u64 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// The events, in order.
    pub fn events(&self) -> &Receiver<Event> {
        &self.events
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
    events: Sender<Event>,
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
    /// Which of them plays
    playing: usize,
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
                match self.commands.recv_timeout(IDLE) {
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
                self.cut(true);
                self.upcoming = tracks.into();
                self.set_done(false);
                self.ended = false;
            }
            Command::Upcoming(tracks) => self.set_upcoming(tracks),
            Command::Stop => {
                self.cut(false);
                self.set_done(true);
                self.ended = true;
            }
        }
    }

    /// Drops what's buffered after what plays fades out.
    fn cut(&mut self, playing_next: bool) {
        self.pending.clear();
        self.pending_from = 0;
        self.outbox.clear();
        self.current = None;
        self.converter = None;
        self.upcoming.clear();
        self.placed.clear();
        self.playing = 0;
        let _ = self.markers.push(Marker::Cut {
            to: self.written,
            fade_in: false,
        });
        if !playing_next {
            self.shared.entry.store(NONE, Ordering::Release);
        }
    }

    /// What follows the entry playing changed: what's decoded ahead stays as
    /// far as it agrees, and the rest is skipped.
    fn set_upcoming(&mut self, tracks: Vec<Track>) {
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

    /// Takes what the callback reports.
    fn hear(&mut self) {
        while let Ok(Played::Started { entry, at }) = self.played.pop() {
            if let Some(index) = self
                .placed
                .iter()
                .position(|placed| placed.entry == entry && placed.at == at)
            {
                self.playing = index;
            }
            let _ = self.events.send(Event::Started { entry });
        }
        if self.done && !self.ended && self.pending_frames() == 0 && self.outbox.is_empty() {
            let played = self.shared.played.load(Ordering::Acquire);
            if played >= self.written {
                self.ended = true;
                let _ = self.events.send(Event::Ended);
            }
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
                let _ = self.events.send(Event::Failed { entry, error });
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
                    let _ = self.events.send(Event::Failed {
                        entry: track.entry,
                        error,
                    });
                    continue;
                }
            };
            // AAC in MP4 says its rate only once it decodes
            self.scratch.clear();
            let first = match decoder.next(&mut self.scratch) {
                Ok(first) => first,
                Err(error) => {
                    let _ = self.events.send(Event::Failed {
                        entry: track.entry,
                        error,
                    });
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
                            let _ = self.events.send(Event::Failed {
                                entry: track.entry,
                                error,
                            });
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
