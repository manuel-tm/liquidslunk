//! What the decoder thread and the audio callback share: the samples, in a
//! ring buffer, and markers that say where each entry starts in them, and
//! what's no longer wanted (backend.md, "Joining tracks").
//!
//! Positions are frames of the stream: every frame the decoder has written,
//! counted from the engine's start.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// No entry.
pub const NONE: u64 = u64::MAX;

/// What the callback does at a point of the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// An entry's first frame is at `at`.
    Start { at: u64, entry: u64 },
    /// What's buffered from `at` to `to` isn't wanted: it's dropped unheard.
    /// Markers queued before it, from `at` on, go with it.
    Skip { at: u64, to: u64 },
    /// What plays fades out at once, then what's buffered up to `to` is
    /// dropped, and markers queued before it with it. What comes at `to`
    /// fades in when `fade_in`; a track's own start doesn't.
    Cut { to: u64, fade_in: bool },
}

/// What the callback tells the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Played {
    /// An entry's first frame played.
    Started { entry: u64, at: u64 },
}

/// The engine's settings, and what the callback has played.
#[derive(Debug)]
pub struct Shared {
    /// Whether the player plays, or pauses
    pub playing: AtomicBool,
    /// The volume, an `f32`'s bits, from 0 to 1
    pub volume: AtomicU32,
    /// A fade's length, in frames; 0 cuts
    pub fade: AtomicU32,
    /// Frames of the stream played
    pub played: AtomicU64,
    /// The entry playing, or `NONE`
    pub entry: AtomicU64,
    /// Where it started in the stream
    pub entry_start: AtomicU64,
    /// Frames played as silence for want of samples, while more was coming
    pub underruns: AtomicU64,
    /// Nothing more is coming: what's written is the end
    pub draining: AtomicBool,
    /// From a callback to when what it wrote is heard, in nanoseconds
    pub latency: AtomicU64,
}

impl Shared {
    pub fn new(fade: u32) -> Self {
        Self {
            playing: AtomicBool::new(true),
            volume: AtomicU32::new(1.0f32.to_bits()),
            fade: AtomicU32::new(fade),
            played: AtomicU64::new(0),
            entry: AtomicU64::new(NONE),
            entry_start: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            draining: AtomicBool::new(true),
            latency: AtomicU64::new(0),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
}
