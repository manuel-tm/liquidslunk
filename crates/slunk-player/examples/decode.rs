//! Decodes files as the engine does, and prints each one's rate and frames,
//! for checking a library's files (backend.md, "The player engine"), and
//! with `--seeks`, checks seeking in them (backend.md, "Seeking").
//!
//!   cargo run -p slunk-player --release --example decode -- [--seeks] a.flac b.m4a …
//!
//! Each line is the file's rate, its frames, how long decoding took, and its
//! path, tab-separated; a file that can't be decoded says why. With
//! `--seeks`, each file is also sought at a tenth, half and nine tenths of
//! it, and a second before its end, and what each seek decodes is checked
//! against decoding from the start: the line then also has the longest a
//! seek took, to its first packet decoded, and whether every seek was exact.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use slunk_player::{Error, TrackDecoder};

/// Frames compared after each seek.
const WINDOW: u64 = 8192;

fn main() {
    let mut seeks = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--seeks" => seeks = true,
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    for path in paths {
        let began = Instant::now();
        let (rate, frames) = match count(&path) {
            Ok(counted) => counted,
            Err((error, frames)) => {
                println!("failed\t{error}\tafter {frames}\t{}", path.display());
                continue;
            }
        };
        let took = began.elapsed().as_secs_f64() * 1000.0;
        let checked = if seeks {
            match check_seeks(&path, frames, rate) {
                Ok((slowest, exact)) => format!(
                    "\tseeks {:.1} ms {}",
                    slowest.as_secs_f64() * 1000.0,
                    if exact { "exact" } else { "NOT EXACT" }
                ),
                Err(error) => format!("\tseeks failed: {error}"),
            }
        } else {
            String::new()
        };
        println!(
            "{rate}\t{frames}\t{took:.0} ms{checked}\t{}",
            path.display()
        );
    }
}

/// The file's rate and frames, decoded from its start; or why it can't be,
/// and the frames decoded before.
fn count(path: &Path) -> Result<(u32, u64), (Error, u64)> {
    let mut decoder = TrackDecoder::open(path).map_err(|error| (error, 0))?;
    let mut frames = 0u64;
    let mut samples = Vec::new();
    loop {
        samples.clear();
        match decoder.next(&mut samples) {
            Ok(true) => frames += (samples.len() / 2) as u64,
            Ok(false) => return Ok((decoder.rate(), frames)),
            Err(error) => return Err((error, frames)),
        }
    }
}

/// Seeks across the file, and compares what each seek decodes with what
/// decoding from the start has there: the longest a seek took, and whether
/// they were all the same.
fn check_seeks(path: &Path, frames: u64, rate: u32) -> Result<(Duration, bool), Error> {
    let points = [
        frames / 10,
        frames / 2,
        frames * 9 / 10,
        frames.saturating_sub(u64::from(rate)),
    ];
    // What's decoded from the start, after each point
    let mut windows = vec![Vec::new(); points.len()];
    let mut decoder = TrackDecoder::open(path)?;
    let mut at = 0u64;
    let mut samples = Vec::new();
    loop {
        samples.clear();
        if !decoder.next(&mut samples)? {
            break;
        }
        let decoded = (samples.len() / 2) as u64;
        for (window, &point) in windows.iter_mut().zip(&points) {
            let start = point.max(at);
            let end = (point + WINDOW).min(at + decoded);
            if start < end {
                window.extend_from_slice(
                    &samples[((start - at) * 2) as usize..((end - at) * 2) as usize],
                );
            }
        }
        at += decoded;
    }
    let mut slowest = Duration::ZERO;
    let mut exact = true;
    for (window, &point) in windows.iter().zip(&points) {
        let mut decoder = TrackDecoder::open(path)?;
        let mut out = Vec::new();
        // As the engine does: the first packet tells the rate
        decoder.next(&mut out)?;
        out.clear();
        let began = Instant::now();
        let found = decoder.seek(point)? && decoder.next(&mut out)?;
        slowest = slowest.max(began.elapsed());
        while found && out.len() < window.len() && decoder.next(&mut out)? {}
        out.truncate(window.len());
        exact &= out == *window;
    }
    Ok((slowest, exact))
}
