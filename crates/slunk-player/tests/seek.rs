//! Seeking (backend.md, "Seeking"): a seek to any frame decodes what
//! decoding from the start decodes from there, sample for sample, in every
//! format the fixtures have.

use std::path::Path;

use slunk_player::TrackDecoder;

mod common;
use common::{RATE, decode, fixture, pcm, signal, write_aiff, write_wav};

/// The seeking fixtures' frames (generate.ts).
const FRAMES: usize = 22050;

/// What's decoded from a seek to `frame`, to the end, and whether the seek
/// found music there.
fn decode_from(path: &Path, frame: u64) -> (bool, Vec<f32>) {
    let mut decoder = TrackDecoder::open(path).unwrap();
    let found = decoder.seek(frame).unwrap();
    let mut out = Vec::new();
    while decoder.next(&mut out).unwrap() {}
    (found, out)
}

/// Seeks to frames across the file: its start, around where packets and
/// FLAC's frames meet, the middle, and its last frames, and checks each
/// against the file decoded from its start.
fn seeks_exactly(path: &Path) {
    let whole = decode(path);
    let frames = whole.len() / 2;
    let mut points = vec![0, 1, 1023, 1024, 1025, 1152, 2112, 4095, 4096, 4607, 4608];
    points.extend([4609, 9000, frames / 2, frames - 1025, frames - 1, frames]);
    for frame in points.into_iter().filter(|&frame| frame <= frames) {
        let (found, rest) = decode_from(path, frame as u64);
        assert!(
            rest == whole[frame * 2..],
            "{}: a seek to {frame} decodes {} frames, and should decode {}",
            path.display(),
            rest.len() / 2,
            frames - frame,
        );
        assert!(found || rest.is_empty(), "{}: {frame}", path.display());
    }
    // Past the end, there's nothing
    let (_, rest) = decode_from(path, frames as u64 + 5000);
    assert!(rest.is_empty(), "{}", path.display());
}

#[test]
fn flac_and_apple_lossless_seek_to_the_signal() {
    for name in ["seek.flac", "seek-alac.m4a"] {
        assert!(decode(&fixture(name)) == signal(0, FRAMES), "{name}");
        seeks_exactly(&fixture(name));
    }
    // A short last FLAC frame, as WebKit padded
    seeks_exactly(&fixture("join-1.flac"));
}

#[test]
fn wav_and_aiff_seek_to_the_signal() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("seek.wav");
    let aiff = dir.path().join("seek.aiff");
    write_wav(&wav, RATE, 2, &pcm(0, FRAMES));
    write_aiff(&aiff, &pcm(0, FRAMES));
    for path in [&wav, &aiff] {
        seeks_exactly(path);
        let (_, rest) = decode_from(path, 12345);
        assert!(rest == signal(12345, FRAMES - 12345), "{}", path.display());
    }
}

#[test]
fn aac_seeks_as_it_decodes_from_its_start() {
    // In M4A, trimmed by its iTunSMPB, and in ADTS, with its delay
    assert_eq!(decode(&fixture("seek-aac.m4a")).len() / 2, FRAMES);
    seeks_exactly(&fixture("seek-aac.m4a"));
    seeks_exactly(&fixture("seek.aac"));
}

#[test]
fn mp3_seeks_within_what_its_lame_tag_leaves() {
    // Its frames are silence, so this checks where a seek lands: the frames
    // after it, less the delay and padding the tag says
    seeks_exactly(&fixture("lame.mp3"));
    let (_, rest) = decode_from(&fixture("lame.mp3"), 10000);
    assert_eq!(rest.len() / 2, 20 * 1152 - 576 - 1000 - 10000);
}
