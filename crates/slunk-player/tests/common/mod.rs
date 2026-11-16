//! The fixtures' signal, which the tests work out as generate.ts does.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The fixtures' tracks, in frames, cut from one signal (generate.ts)
pub const TRACKS: [usize; 3] = [5003, 6421, 4900];
pub const RATE: u32 = 44100;

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The fixtures' signal from frame `first`, as the decoder gives it:
/// interleaved stereo, each 16-bit sample over 32,768. generate.ts writes
/// the same.
pub fn signal(first: usize, frames: usize) -> Vec<f32> {
    pcm(first, frames)
        .into_iter()
        .map(|sample| f32::from(sample) / 32768.0)
        .collect()
}

/// The same, as the 16-bit samples a file holds.
pub fn pcm(first: usize, frames: usize) -> Vec<i16> {
    (first..first + frames)
        .flat_map(|frame| [triangle(frame, 211), triangle(frame, 173)])
        .map(|sample| sample as i16)
        .collect()
}

pub fn triangle(frame: usize, period: usize) -> i32 {
    let amplitude = 12000i64;
    let phase = (frame % period) as i64;
    let period = period as i64;
    let value = if phase < period / 2 {
        4 * amplitude * phase / period - amplitude
    } else {
        3 * amplitude - 4 * amplitude * phase / period
    };
    value as i32
}

/// Where track `index` starts in the signal.
pub fn offset(index: usize) -> usize {
    TRACKS[..index].iter().sum()
}
