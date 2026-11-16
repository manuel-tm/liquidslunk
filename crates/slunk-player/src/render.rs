//! The audio callback's side of the stream: it plays the samples the decoder
//! wrote, acts on the markers as it reaches them, applies the volume and the
//! fades, and works out what's heard (backend.md, "Joining tracks"). It runs
//! on the output's real-time thread, so it never allocates, locks or waits.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use rtrb::{Consumer, Producer};

use crate::stream::{Marker, Played, Shared};

const CHANNELS: usize = 2;
/// Markers the ring holds, which the callback can look through at once.
pub const MARKERS: usize = 64;
/// How long a change of volume takes, in seconds, so it doesn't click.
const VOLUME_RAMP: f32 = 0.010;
/// Renders remembered, to find what's heard an output's latency later: at
/// 512 frames a render, six seconds at 44.1 kHz.
const HISTORY: usize = 512;

/// Plays the stream into an output's buffers.
pub struct Renderer {
    samples: Consumer<f32>,
    markers: Consumer<Marker>,
    played: Producer<Played>,
    shared: Arc<Shared>,
    rate: u32,
    position: u64,
    /// The volume as it's applied, moving toward the one set
    gain: f32,
    /// How far a fade is, from 0, silent, to 1
    fade: f32,
    /// A cut under way: what plays fades out, then the stream goes on from
    /// here
    cut: Option<u64>,
    /// Frames still to drop, when a skip found them not written yet
    dropping: u64,
    /// Where the next marker is due
    next_at: u64,
    /// A frame has played since the last cut: silence for want of samples
    /// is an underrun, not a track still to start
    primed: bool,
    /// Frames given to the output, silence included
    output: u64,
    /// Where the output and the stream were as each of the last renders
    /// ended, oldest first from `history_next`
    history: Box<[(u64, u64)]>,
    history_next: usize,
    history_len: usize,
}

impl Renderer {
    pub(crate) fn new(
        samples: Consumer<f32>,
        markers: Consumer<Marker>,
        played: Producer<Played>,
        shared: Arc<Shared>,
        rate: u32,
    ) -> Self {
        let gain = shared.volume();
        Self {
            samples,
            markers,
            played,
            shared,
            rate,
            position: 0,
            gain,
            fade: 1.0,
            cut: None,
            dropping: 0,
            next_at: 0,
            primed: false,
            output: 0,
            history: vec![(0, 0); HISTORY].into_boxed_slice(),
            history_next: 0,
            history_len: 0,
        }
    }

    /// The output's sample rate.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub(crate) fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Frames written and not played yet.
    pub fn available(&self) -> usize {
        self.samples.slots() / CHANNELS
    }

    /// Fills `out`, interleaved, with `channels` samples a frame: the left
    /// and right on the first two, silence on the others, and their mix on
    /// a mono output.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        self.look_for_cut();
        self.next_at = self.front_at();
        let playing = self.shared.playing.load(Ordering::Relaxed);
        let volume = self.shared.volume();
        let fade_frames = self.shared.fade.load(Ordering::Relaxed);
        let fade_step = if fade_frames == 0 {
            1.0
        } else {
            1.0 / fade_frames as f32
        };
        let gain_step = 1.0 / (self.rate as f32 * VOLUME_RAMP).max(1.0);
        let draining = self.shared.draining.load(Ordering::Acquire);
        let mut underruns = 0;

        for frame in out.chunks_mut(channels) {
            if self.dropping > 0 {
                self.drop_frames(self.dropping);
            }
            if let Some(to) = self.cut
                && (self.fade <= 0.0 || self.position >= to)
            {
                self.cut = None;
                self.primed = false;
                self.drop_frames(to.saturating_sub(self.position));
                self.fade = if playing { 1.0 } else { 0.0 };
                self.next_at = self.front_at();
            }
            if self.position >= self.next_at {
                self.process_due();
            }
            let target = if playing && self.cut.is_none() {
                1.0
            } else {
                0.0
            };
            if self.fade <= 0.0 && target == 0.0 && self.cut.is_none() {
                frame.fill(0.0);
                continue;
            }
            let Ok(chunk) = self.samples.read_chunk(CHANNELS) else {
                frame.fill(0.0);
                if self.primed && !draining {
                    underruns += 1;
                }
                continue;
            };
            self.primed = true;
            let (first, second) = chunk.as_slices();
            let left = first.first().copied().unwrap_or_default();
            let right = first.get(1).or(second.first()).copied().unwrap_or_default();
            chunk.commit_all();
            self.position += 1;

            self.fade = if target > self.fade {
                (self.fade + fade_step).min(target)
            } else {
                (self.fade - fade_step).max(target)
            };
            self.gain = if volume > self.gain {
                (self.gain + gain_step).min(volume)
            } else {
                (self.gain - gain_step).max(volume)
            };
            let level = self.gain * self.fade;
            let (left, right) = (left * level, right * level);
            match frame {
                [mono] => *mono = (left + right) / 2.0,
                [l, r, rest @ ..] => {
                    *l = left;
                    *r = right;
                    rest.fill(0.0);
                }
                [] => {}
            }
        }

        self.shared.played.store(self.position, Ordering::Release);
        if underruns > 0 {
            self.shared
                .underruns
                .fetch_add(underruns, Ordering::Relaxed);
        }
        self.output += (out.len() / channels) as u64;
        self.hear();
    }

    /// Works out what's heard now: what was rendered when the output was an
    /// output's latency behind where it is. With no latency, what's rendered
    /// is heard.
    fn hear(&mut self) {
        self.history[self.history_next] = (self.output, self.position);
        self.history_next = (self.history_next + 1) % HISTORY;
        self.history_len = (self.history_len + 1).min(HISTORY);
        let latency = self.shared.latency.load(Ordering::Relaxed);
        let behind = (u128::from(latency) * u128::from(self.rate) / 1_000_000_000) as u64;
        let target = self.output.saturating_sub(behind);
        let heard = (1..=self.history_len)
            .map(|back| self.history[(self.history_next + HISTORY - back) % HISTORY])
            .find(|&(output, _)| output <= target)
            .map(|(_, position)| position);
        if let Some(heard) = heard {
            self.shared.heard.store(heard, Ordering::Release);
        }
    }

    /// A cut anywhere in the queue makes what's before it moot: the last one
    /// is the one that counts.
    fn look_for_cut(&mut self) {
        let mut queued = [Marker::Skip { at: 0, to: 0 }; MARKERS];
        let count = self.peek(&mut queued);
        let last = queued[..count]
            .iter()
            .rposition(|marker| matches!(marker, Marker::Cut { .. }));
        if let Some(index) = last {
            if let Marker::Cut { to } = queued[index] {
                self.cut = Some(to);
            }
            self.consume(index + 1);
        }
    }

    /// Acts on the markers due at the current position.
    fn process_due(&mut self) {
        loop {
            let mut queued = [Marker::Skip { at: 0, to: 0 }; MARKERS];
            let count = self.peek(&mut queued);
            let Some(&front) = queued[..count].first() else {
                self.next_at = u64::MAX;
                return;
            };
            let due = at(front);
            if due > self.position {
                self.next_at = due;
                return;
            }
            // A skip from here or before covers what's queued before it
            let skip =
                queued[..count]
                    .iter()
                    .enumerate()
                    .find_map(|(index, marker)| match *marker {
                        Marker::Skip { at, to } if at <= self.position => Some((index, at, to)),
                        _ => None,
                    });
            match skip {
                Some((index, from, to)) => {
                    for &marker in &queued[..index] {
                        if at(marker) < from {
                            self.apply(marker);
                        }
                    }
                    self.consume(index + 1);
                    if from < self.position {
                        // It came late, after what it drops began to play:
                        // that fades out, as at a cut
                        self.cut = Some(to);
                    } else {
                        self.drop_frames(to.saturating_sub(self.position));
                    }
                }
                None => {
                    self.consume(1);
                    self.apply(front);
                }
            }
        }
    }

    fn apply(&mut self, marker: Marker) {
        if let Marker::Start { at, entry, fade_in } = marker {
            if fade_in {
                self.fade = 0.0;
            }
            let _ = self.played.push(Played::Started { entry, at });
        }
    }

    /// Drops frames unheard, or as many as are written, and the rest later.
    fn drop_frames(&mut self, frames: u64) {
        let available = (self.samples.slots() / CHANNELS) as u64;
        let dropped = frames.min(available);
        if let Ok(chunk) = self.samples.read_chunk(dropped as usize * CHANNELS) {
            chunk.commit_all();
        }
        self.position += dropped;
        self.dropping = frames - dropped;
    }

    /// Copies the queued markers, without taking them.
    fn peek(&mut self, into: &mut [Marker; MARKERS]) -> usize {
        let count = self.markers.slots().min(MARKERS);
        let Ok(chunk) = self.markers.read_chunk(count) else {
            return 0;
        };
        let (first, second) = chunk.as_slices();
        into[..first.len()].copy_from_slice(first);
        into[first.len()..first.len() + second.len()].copy_from_slice(second);
        count
    }

    fn consume(&mut self, count: usize) {
        if let Ok(chunk) = self.markers.read_chunk(count) {
            chunk.commit_all();
        }
    }

    fn front_at(&mut self) -> u64 {
        let mut queued = [Marker::Skip { at: 0, to: 0 }; MARKERS];
        let count = self.peek(&mut queued);
        queued[..count]
            .first()
            .map_or(u64::MAX, |&marker| at(marker))
    }
}

/// Where a marker is due: a cut, at once.
fn at(marker: Marker) -> u64 {
    match marker {
        Marker::Start { at, .. } | Marker::Skip { at, .. } => at,
        Marker::Cut { .. } => 0,
    }
}
