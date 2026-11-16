//! The fixtures' signal, which the tests work out as generate.ts does.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use slunk_player::TrackDecoder;

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

/// A whole file, decoded as the engine decodes it.
pub fn decode(path: &Path) -> Vec<f32> {
    let mut decoder = TrackDecoder::open(path).unwrap();
    let mut out = Vec::new();
    while decoder.next(&mut out).unwrap() {}
    out
}

/// A WAV file of 16-bit interleaved samples.
pub fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    fs::write(path, bytes).unwrap();
}

/// An AIFF file of 16-bit interleaved stereo at 44.1 kHz, whose rate is an
/// 80-bit extended float: 2^15 × 0xAC44 / 2^15.
pub fn write_aiff(path: &Path, samples: &[i16]) {
    let data: Vec<u8> = samples.iter().flat_map(|s| s.to_be_bytes()).collect();
    let frames = (samples.len() / 2) as u32;
    let mut comm = Vec::new();
    comm.extend_from_slice(&2u16.to_be_bytes());
    comm.extend_from_slice(&frames.to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&[0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]);
    let mut body = b"AIFF".to_vec();
    body.extend_from_slice(b"COMM");
    body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
    body.extend_from_slice(&comm);
    body.extend_from_slice(b"SSND");
    body.extend_from_slice(&(8 + data.len() as u32).to_be_bytes());
    body.extend_from_slice(&[0; 8]);
    body.extend_from_slice(&data);
    let mut bytes = b"FORM".to_vec();
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&body);
    fs::write(path, bytes).unwrap();
}
