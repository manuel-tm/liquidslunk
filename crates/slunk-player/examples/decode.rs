//! Decodes files as the engine does, and prints each one's rate and frames,
//! for checking a library's files (backend.md, "The player engine").
//!
//!   cargo run -p slunk-player --release --example decode -- a.flac b.m4a …
//!
//! Each line is the file's rate, its frames, how long decoding took, and its
//! path, tab-separated; a file that can't be decoded says why.

use std::path::PathBuf;
use std::time::Instant;

use slunk_player::TrackDecoder;

fn main() {
    for path in std::env::args().skip(1).map(PathBuf::from) {
        let began = Instant::now();
        let mut decoder = match TrackDecoder::open(&path) {
            Ok(decoder) => decoder,
            Err(error) => {
                println!("failed\t{error}\t\t{}", path.display());
                continue;
            }
        };
        let mut frames = 0u64;
        let mut samples = Vec::new();
        let result = loop {
            samples.clear();
            match decoder.next(&mut samples) {
                Ok(true) => frames += (samples.len() / 2) as u64,
                Ok(false) => break Ok(()),
                Err(error) => break Err(error),
            }
        };
        match result {
            Ok(()) => println!(
                "{}\t{frames}\t{:.0} ms\t{}",
                decoder.rate(),
                began.elapsed().as_secs_f64() * 1000.0,
                path.display()
            ),
            Err(error) => println!("failed\t{error}\tafter {frames}\t{}", path.display()),
        }
    }
}
