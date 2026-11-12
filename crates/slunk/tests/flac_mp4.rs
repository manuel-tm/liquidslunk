//! A FLAC file served as an MP4 that holds its frames, as
//! docs/development/backend.md describes it ("Serving FLAC as MP4"): finding
//! the frames, the MP4's header and its ranges, and the MP4s the media
//! context keeps.
//!
//! The FLAC streams here are built by the tests, with verbatim frames, whose
//! samples are their bytes as they are. So a test can put anything in a
//! frame, like what looks like the next frame's header.

use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use slunk::media::audio::flac::{
    FlacError, FlacFrames, crc8, crc16, read_frames, read_frames_from,
};
use slunk::media::audio::mp4::header;
use slunk::media::{AudioSource, FlacMp4, Library, Media};
use slunk::repo::store::Store;
use slunk::repo::{connection, migrator};

const RATE: u32 = 44_100;
const CHANNELS: u8 = 2;

/// A frame to build: its samples per channel, and its bytes after the
/// subframes' headers, two per sample per channel.
struct Frame {
    samples: u32,
    data: Vec<u8>,
}

/// Bytes that look random enough, the same each time.
fn noise(length: usize, seed: u32) -> Vec<u8> {
    let mut state = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..length)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state >> 24) as u8
        })
        .collect()
}

fn frame(samples: u32, seed: u32) -> Frame {
    Frame {
        samples,
        data: noise(samples as usize * 2 * usize::from(CHANNELS), seed),
    }
}

/// A number, coded as UTF-8 is, as frame headers code them.
fn utf8_number(number: u64) -> Vec<u8> {
    if number < 0x80 {
        return vec![number as u8];
    }
    let mut more = 1;
    while number >= 1 << (6 * more + 6 - more) {
        more += 1;
    }
    let lead_bits = 6 - more;
    let lead =
        (0xff00u16 >> (more + 1)) as u8 | (number >> (6 * more)) as u8 & ((1 << lead_bits) - 1);
    let mut bytes = vec![lead];
    for index in (0..more).rev() {
        bytes.push(0x80 | ((number >> (6 * index)) & 0x3f) as u8);
    }
    bytes
}

/// A frame's header: its block size in 16 bits, 44.1 kHz, stereo, 16 bits a
/// sample. `number` is the frame's, or with variable block sizes, its first
/// sample's.
fn frame_header(variable: bool, number: u64, samples: u32) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xf8 | u8::from(variable), 0x79, 0x18];
    bytes.extend(utf8_number(number));
    bytes.extend(((samples - 1) as u16).to_be_bytes());
    bytes.push(crc8(&bytes));
    bytes
}

/// A whole frame: its header, a verbatim subframe per channel, and its
/// CRC-16.
fn frame_bytes(variable: bool, number: u64, frame: &Frame) -> Vec<u8> {
    let mut bytes = frame_header(variable, number, frame.samples);
    let per_channel = frame.data.len() / usize::from(CHANNELS);
    for channel in frame.data.chunks(per_channel) {
        // Verbatim, with no wasted bits
        bytes.push(0x02);
        bytes.extend(channel);
    }
    bytes.extend(crc16(0, &bytes).to_be_bytes());
    bytes
}

fn streaminfo(frames: &[Frame], total: u64) -> [u8; 34] {
    let min = frames.iter().map(|f| f.samples).min().unwrap_or(0) as u16;
    let max = frames.iter().map(|f| f.samples).max().unwrap_or(0) as u16;
    let mut block = [0; 34];
    block[0..2].copy_from_slice(&min.to_be_bytes());
    block[2..4].copy_from_slice(&max.to_be_bytes());
    let packed = (u64::from(RATE) << 44) | (u64::from(CHANNELS - 1) << 41) | (15 << 36) | total;
    block[10..18].copy_from_slice(&packed.to_be_bytes());
    block
}

/// A FLAC file: `fLaC`, STREAMINFO, a padding block, then the frames. Also
/// where each frame starts and ends.
struct Built {
    bytes: Vec<u8>,
    frames: Vec<(usize, usize)>,
    streaminfo: [u8; 34],
}

fn build(variable: bool, frames: &[Frame]) -> Built {
    let total = frames.iter().map(|f| u64::from(f.samples)).sum();
    build_with_total(variable, frames, total)
}

fn build_with_total(variable: bool, frames: &[Frame], total: u64) -> Built {
    let info = streaminfo(frames, total);
    let mut bytes = b"fLaC".to_vec();
    bytes.extend([0x00, 0, 0, 34]);
    bytes.extend(info);
    bytes.extend([0x81, 0, 0, 5, 0, 0, 0, 0, 0]);
    let mut spans = Vec::new();
    let mut first_sample = 0;
    for (index, frame) in frames.iter().enumerate() {
        let number = if variable { first_sample } else { index as u64 };
        let start = bytes.len();
        bytes.extend(frame_bytes(variable, number, frame));
        spans.push((start, bytes.len()));
        first_sample += u64::from(frame.samples);
    }
    Built {
        bytes,
        frames: spans,
        streaminfo: info,
    }
}

fn read(bytes: &[u8]) -> Result<FlacFrames, FlacError> {
    read_frames_from(Cursor::new(bytes))
}

/// The frames' spans in the file, as found.
fn spans(frames: &FlacFrames) -> Vec<(usize, usize)> {
    let mut at = frames.start as usize;
    frames
        .sizes
        .iter()
        .map(|&size| {
            let span = (at, at + size as usize);
            at += size as usize;
            span
        })
        .collect()
}

fn invalid(result: Result<FlacFrames, FlacError>) -> String {
    match result {
        Err(FlacError::Invalid(message)) => message,
        other => panic!("expected an invalid FLAC file, got {other:?}"),
    }
}

#[test]
fn frames_are_found_from_their_headers_and_crcs() {
    let frames: Vec<Frame> = (0..6).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    let found = read(&built.bytes).unwrap();
    assert_eq!(spans(&found), built.frames);
    assert_eq!(found.blocks, vec![(6, 4096)]);
    assert_eq!(found.samples(), 6 * 4096);
    assert_eq!(found.streaminfo, built.streaminfo);
    assert_eq!(
        (found.sample_rate, found.channels, found.bits_per_sample),
        (RATE, 2, 16)
    );
    assert_eq!(found.end(), built.bytes.len() as u64);
}

#[test]
fn a_header_inside_a_frame_isnt_a_frame() {
    let mut frames: Vec<Frame> = (0..4).map(|n| frame(4096, n)).collect();
    // A sync code, then the next frame's header, valid with its CRC-8
    frames[1].data[100..102].copy_from_slice(&[0xff, 0xf8]);
    let fake = frame_header(false, 2, 4096);
    frames[1].data[2000..2000 + fake.len()].copy_from_slice(&fake);
    let built = build(false, &frames);
    let found = read(&built.bytes).unwrap();
    assert_eq!(spans(&found), built.frames);
}

#[test]
fn variable_block_sizes_follow_by_sample() {
    let frames = vec![
        frame(4096, 1),
        frame(4096, 2),
        frame(1000, 3),
        frame(2304, 4),
        frame(17, 5),
    ];
    let built = build(true, &frames);
    let found = read(&built.bytes).unwrap();
    assert_eq!(spans(&found), built.frames);
    assert_eq!(found.blocks, vec![(2, 4096), (1, 1000), (1, 2304), (1, 17)]);
}

#[test]
fn tags_before_and_after_are_left_out() {
    let frames: Vec<Frame> = (0..3).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    // An ID3v2 tag of 20 bytes, before
    let mut id3v2 = b"ID3\x04\x00\x00\x00\x00\x00\x14".to_vec();
    id3v2.extend([0; 20]);
    // An ID3v1 tag, after
    let mut id3v1 = b"TAG".to_vec();
    id3v1.extend([b' '; 125]);
    let bytes = [&id3v2[..], &built.bytes, &id3v1].concat();
    let found = read(&bytes).unwrap();
    let shifted: Vec<(usize, usize)> = built
        .frames
        .iter()
        .map(|(start, end)| (start + id3v2.len(), end + id3v2.len()))
        .collect();
    assert_eq!(spans(&found), shifted);
    assert_eq!(found.end(), (bytes.len() - 128) as u64);
}

#[test]
fn a_file_cut_off_or_that_doesnt_add_up_isnt_indexed() {
    let frames: Vec<Frame> = (0..3).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    // Cut off inside the last frame
    let cut = &built.bytes[..built.bytes.len() - 1000];
    assert_eq!(invalid(read(cut)), "its last frame is cut off");
    // STREAMINFO says more samples than the frames hold
    let wrong = build_with_total(false, &frames, 5 * 4096);
    assert_eq!(
        invalid(read(&wrong.bytes)),
        "its frames hold 12288 samples, and its STREAMINFO says 20480"
    );
    // Something else where the first frame should be
    let mut moved = built.bytes.clone();
    moved.insert(built.frames[0].0, 0);
    assert_eq!(
        invalid(read(&moved)),
        "there's no frame where its metadata ends"
    );
    // No frames at all
    let empty = &built.bytes[..built.frames[0].0];
    assert_eq!(
        invalid(read(empty)),
        "there's no frame where its metadata ends"
    );
    // An unknown total, 0, isn't checked
    let unknown = build_with_total(false, &frames, 0);
    assert_eq!(read(&unknown.bytes).unwrap().samples(), 3 * 4096);
}

#[test]
fn other_files_arent_flac() {
    for bytes in [
        &b"RIFF....WAVEfmt "[..],
        b"ID3\x04\x00\x00\x00\x00\x00\x00",
        b"",
    ] {
        assert!(matches!(read(bytes), Err(FlacError::NotFlac)));
    }
}

#[test]
fn the_audio_fixtures_are_indexed() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio");
    for name in ["flac.flac", "flac-variants.flac", "untagged.flac"] {
        let frames =
            read_frames(&fixtures.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!frames.sizes.is_empty(), "{name}");
        let length = fs::metadata(fixtures.join(name)).unwrap().len();
        assert!(frames.end() <= length, "{name}");
    }
}

/// An MP4's boxes, by their path: `moov/trak/mdia/mdhd`. Each is its
/// content, after its size and type.
fn find<'a>(bytes: &'a [u8], path: &str) -> &'a [u8] {
    let (name, rest) = path.split_once('/').unwrap_or((path, ""));
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let mut size = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let mut content = at + 8;
        if size == 1 {
            size = u64::from_be_bytes(bytes[at + 8..at + 16].try_into().unwrap()) as usize;
            content = at + 16;
        }
        if &bytes[at + 4..at + 8] == name.as_bytes() {
            let inside = &bytes[content..(at + size).min(bytes.len())];
            return if rest.is_empty() {
                inside
            } else {
                find(inside, rest)
            };
        }
        at += size;
    }
    panic!("no {name} box");
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn the_mp4_holds_the_frames_as_they_are() {
    let frames: Vec<Frame> = (0..25).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.flac");
    fs::write(&path, &built.bytes).unwrap();
    let mp4 = FlacMp4::new(&path).unwrap();
    let whole = mp4.read(0, mp4.len() - 1).unwrap();
    assert_eq!(whole.len() as u64, mp4.len());

    // Every timescale is the sample rate, and every length the samples
    let samples = 25 * 4096;
    let mvhd = find(&whole, "moov/mvhd");
    assert_eq!((u32_at(mvhd, 12), u32_at(mvhd, 16)), (RATE, samples));
    let tkhd = find(&whole, "moov/trak/tkhd");
    assert_eq!(u32_at(tkhd, 20), samples);
    let mdhd = find(&whole, "moov/trak/mdia/mdhd");
    assert_eq!((u32_at(mdhd, 12), u32_at(mdhd, 16)), (RATE, samples));

    // The sample entry, with STREAMINFO in `dfLa`
    let stbl = find(&whole, "moov/trak/mdia/minf/stbl");
    let stsd = find(stbl, "stsd");
    let entry = find(&stsd[8..], "fLaC");
    assert_eq!(u16::from_be_bytes([entry[16], entry[17]]), 2);
    assert_eq!(u16::from_be_bytes([entry[18], entry[19]]), 16);
    assert_eq!(u32_at(entry, 24) >> 16, RATE);
    let dfla = find(&entry[28..], "dfLa");
    assert_eq!(&dfla[4..8], &[0x80, 0, 0, 34]);
    assert_eq!(&dfla[8..], &built.streaminfo);

    // A sample per frame, with its size and samples
    let stts = find(stbl, "stts");
    assert_eq!(
        (u32_at(stts, 4), u32_at(stts, 8), u32_at(stts, 12)),
        (1, 25, 4096)
    );
    let stsz = find(stbl, "stsz");
    assert_eq!(u32_at(stsz, 8), 25);
    let sizes: Vec<u32> = (0..25).map(|n| u32_at(stsz, 12 + 4 * n)).collect();
    let expected: Vec<u32> = built.frames.iter().map(|(s, e)| (e - s) as u32).collect();
    assert_eq!(sizes, expected);

    // Chunks of about a second: 11 frames of 4096 at 44.1 kHz, so 11, 11, 3
    let stsc = find(stbl, "stsc");
    assert_eq!(u32_at(stsc, 4), 2);
    assert_eq!((u32_at(stsc, 8), u32_at(stsc, 12)), (1, 11));
    assert_eq!((u32_at(stsc, 20), u32_at(stsc, 24)), (3, 3));
    let stco = find(stbl, "stco");
    assert_eq!(u32_at(stco, 4), 3);
    for (chunk, first_frame) in [0, 11, 22].into_iter().enumerate() {
        let offset = u32_at(stco, 8 + 4 * chunk) as usize;
        let (start, end) = built.frames[first_frame];
        assert_eq!(
            &whole[offset..offset + (end - start)],
            &built.bytes[start..end]
        );
    }

    // `mdat` is the frames, byte for byte, right after the header
    let mdat = find(&whole, "mdat");
    let frames_start = built.frames[0].0;
    assert_eq!(mdat, &built.bytes[frames_start..]);
    assert_eq!(mp4.header().len() + mdat.len(), whole.len());
}

#[test]
fn ranges_are_read_across_the_header_and_the_frames() {
    let frames: Vec<Frame> = (0..3).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.flac");
    fs::write(&path, &built.bytes).unwrap();
    let mp4 = FlacMp4::new(&path).unwrap();
    let whole = mp4.read(0, mp4.len() - 1).unwrap();
    let header = mp4.header().len() as u64;
    let last = mp4.len() - 1;
    for (start, end) in [
        (0, 1),
        (10, header - 1),
        (header - 5, header + 5),
        (header, header),
        (header + 100, last),
        (last, last),
        (0, last),
    ] {
        assert_eq!(
            mp4.read(start, end).unwrap(),
            &whole[start as usize..=end as usize],
            "{start}-{end}"
        );
    }
    assert!(mp4.read(0, mp4.len()).is_err());
    assert!(mp4.read(5, 4).is_err());
}

#[test]
fn large_files_get_64_bit_offsets_and_lengths() {
    // 1.1 million frames of 4,000 bytes and 4,096 samples: 4.4 GB, and more
    // samples than 32 bits hold
    let count = 1_100_000;
    let frames = FlacFrames {
        streaminfo: [0; 34],
        sample_rate: RATE,
        channels: 2,
        bits_per_sample: 16,
        start: 8_192,
        sizes: vec![4_000; count],
        blocks: vec![(count as u32, 4096)],
    };
    let bytes = header(&frames);
    let moov = find(&bytes, "moov");
    // Version 1: 64-bit times and lengths
    let mvhd = find(moov, "mvhd");
    assert_eq!(mvhd[0], 1);
    assert_eq!(
        u64::from_be_bytes(mvhd[24..32].try_into().unwrap()),
        frames.samples()
    );
    let mdhd = find(moov, "trak/mdia/mdhd");
    assert_eq!(mdhd[0], 1);
    let stbl = find(moov, "trak/mdia/minf/stbl");
    let co64 = find(stbl, "co64");
    let chunks = u32_at(co64, 4) as usize;
    let last = u64::from_be_bytes(
        co64[8 + 8 * (chunks - 1)..16 + 8 * (chunks - 1)]
            .try_into()
            .unwrap(),
    );
    assert!(last > u64::from(u32::MAX));
    // The last chunk ends where the MP4 does
    let frames_in_last = count - 11 * (chunks - 1);
    assert_eq!(
        last + 4_000 * frames_in_last as u64,
        bytes.len() as u64 + 4_000 * count as u64
    );
    // `mdat` has a 64-bit size
    let mdat = &bytes[bytes.len() - 16..];
    assert_eq!(&mdat[..8], &[0, 0, 0, 1, b'm', b'd', b'a', b't']);
    assert_eq!(
        u64::from_be_bytes(mdat[8..].try_into().unwrap()),
        16 + 4_000 * count as u64
    );
}

#[test]
fn high_sample_rates_are_halved_in_the_sample_entry() {
    let mut frames = FlacFrames {
        streaminfo: [0; 34],
        sample_rate: 192_000,
        channels: 2,
        bits_per_sample: 24,
        start: 8_192,
        sizes: vec![10_000; 100],
        blocks: vec![(100, 4096)],
    };
    for (rate, in_entry) in [
        (192_000, 48_000),
        (96_000, 48_000),
        (176_400, 44_100),
        (48_000, 48_000),
    ] {
        frames.sample_rate = rate;
        let bytes = header(&frames);
        let stsd = find(&bytes, "moov/trak/mdia/minf/stbl/stsd");
        let entry = find(&stsd[8..], "fLaC");
        assert_eq!(u32_at(entry, 24) >> 16, in_entry, "{rate}");
        let mdhd = find(&bytes, "moov/trak/mdia/mdhd");
        assert_eq!(u32_at(mdhd, 12), rate);
    }
}

/// A library in a temporary folder, with its media context.
struct Fixture {
    dir: tempfile::TempDir,
    media: Media,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().canonicalize().unwrap().join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let store = Arc::new(Store::new(connection::open(&db).unwrap()));
        let media = Media::new(store, Library::new(&db));
        Fixture { dir, media }
    }

    /// Writes a file and registers it, and returns its id.
    fn add(&self, path: &str, bytes: &[u8]) -> i64 {
        fs::write(self.dir.path().join(path), bytes).unwrap();
        self.media.register_files(&[path.to_string()]).unwrap()[0].id
    }

    fn source(&self, id: i64) -> (Option<AudioSource>, Vec<String>) {
        let mut failures = Vec::new();
        let source = self
            .media
            .audio_source(id, |_, error| failures.push(error.to_string()))
            .unwrap();
        (source, failures)
    }
}

fn mp4(source: Option<AudioSource>) -> Arc<FlacMp4> {
    match source {
        Some(AudioSource::Mp4(mp4)) => mp4,
        other => panic!("expected an MP4, got {other:?}"),
    }
}

#[test]
fn flac_files_are_served_as_mp4_and_kept() {
    let fixture = Fixture::new();
    let frames: Vec<Frame> = (0..3).map(|n| frame(4096, n)).collect();
    let id = fixture.add("a.flac", &build(false, &frames).bytes);

    let first = mp4(fixture.source(id).0);
    let again = mp4(fixture.source(id).0);
    assert!(Arc::ptr_eq(&first, &again), "made once");

    // Changed: made again
    let more: Vec<Frame> = (0..4).map(|n| frame(4096, n)).collect();
    fs::write(fixture.dir.path().join("a.flac"), build(false, &more).bytes).unwrap();
    let changed = mp4(fixture.source(id).0);
    assert!(!Arc::ptr_eq(&first, &changed));
    assert!(changed.len() > first.len());
}

#[test]
fn requests_at_once_wait_for_one_mp4() {
    let fixture = Fixture::new();
    let frames: Vec<Frame> = (0..200).map(|n| frame(4096, n)).collect();
    let id = fixture.add("a.flac", &build(false, &frames).bytes);
    let made: Vec<Arc<FlacMp4>> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| mp4(fixture.source(id).0)))
            .collect();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect()
    });
    assert!(made.iter().all(|mp4| Arc::ptr_eq(mp4, &made[0])));
}

#[test]
fn other_files_are_served_as_they_are() {
    let fixture = Fixture::new();
    let wav = fixture.add("a.wav", b"RIFF....WAVEfmt ");
    assert!(
        matches!(fixture.source(wav), (Some(AudioSource::File(path)), failures)
        if path.ends_with("a.wav") && failures.is_empty())
    );

    // A FLAC file cut off: as it is, and why is said once
    let frames: Vec<Frame> = (0..3).map(|n| frame(4096, n)).collect();
    let built = build(false, &frames);
    let cut = fixture.add("cut.flac", &built.bytes[..built.bytes.len() - 100]);
    let (source, failures) = fixture.source(cut);
    assert!(matches!(source, Some(AudioSource::File(_))));
    assert_eq!(failures, vec!["its last frame is cut off"]);
    assert_eq!(fixture.source(cut).1, Vec::<String>::new());

    // Missing: as it is, so it's a 404
    let missing = fixture.add("gone.flac", &built.bytes);
    fs::remove_file(fixture.dir.path().join("gone.flac")).unwrap();
    assert!(matches!(
        fixture.source(missing).0,
        Some(AudioSource::File(_))
    ));

    // Unknown
    assert!(fixture.source(999).0.is_none());
}

/// Core Audio decodes the MP4 to the same audio as the FLAC file, byte for
/// byte: a FLAC file built here, and one its own encoder made.
#[cfg(target_os = "macos")]
mod core_audio {
    use std::process::Command;

    use super::*;

    fn afconvert(args: &[&Path]) -> bool {
        Command::new("/usr/bin/afconvert")
            .args(["-f", "WAVE", "-d", "LEI16"])
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
    }

    /// A WAV file's audio: its `data` chunk.
    fn pcm(path: &Path) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        let mut at = 12;
        while at + 8 <= bytes.len() {
            let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            if &bytes[at..at + 4] == b"data" {
                return bytes[at + 8..at + 8 + size].to_vec();
            }
            at += 8 + size + size % 2;
        }
        panic!("no data chunk in {}", path.display());
    }

    /// Wraps `flac` in an MP4, decodes both, and compares.
    fn decodes_the_same(flac: &Path) -> Vec<u8> {
        let dir = flac.parent().unwrap();
        let wrapped = FlacMp4::new(flac).unwrap();
        let mp4 = dir.join("wrapped.m4a");
        fs::write(&mp4, wrapped.read(0, wrapped.len() - 1).unwrap()).unwrap();
        let (from_flac, from_mp4) = (dir.join("from-flac.wav"), dir.join("from-mp4.wav"));
        assert!(
            afconvert(&[flac, &from_flac]),
            "afconvert decodes the FLAC file"
        );
        assert!(afconvert(&[&mp4, &from_mp4]), "afconvert decodes the MP4");
        let audio = pcm(&from_flac);
        assert!(!audio.is_empty());
        assert!(audio == pcm(&from_mp4), "the same audio, byte for byte");
        audio
    }

    #[test]
    fn a_built_flac_file() {
        let dir = tempfile::tempdir().unwrap();
        let frames: Vec<Frame> = (0..30).map(|n| frame(4096, n)).collect();
        let built = build(false, &frames);
        let flac = dir.path().join("built.flac");
        fs::write(&flac, &built.bytes).unwrap();
        let audio = decodes_the_same(&flac);
        // Verbatim frames' samples are big-endian, the WAV's little-endian,
        // and the channels interleaved
        assert_eq!(audio.len(), 30 * 4096 * 4);
        let first = &frames[0].data;
        assert_eq!(&audio[0..2], &[first[1], first[0]]);
        assert_eq!(&audio[2..4], &[first[4096 * 2 + 1], first[4096 * 2]]);
    }

    #[test]
    fn a_flac_file_core_audio_encoded() {
        let dir = tempfile::tempdir().unwrap();
        // Ten seconds of noise, stereo, 16 bits, at 44.1 kHz
        let data = noise(441_000 * 4, 7);
        let mut wav = b"RIFF".to_vec();
        wav.extend((36 + data.len() as u32).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(RATE.to_le_bytes());
        wav.extend((RATE * 4).to_le_bytes());
        wav.extend(4u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((data.len() as u32).to_le_bytes());
        wav.extend(&data);
        let source = dir.path().join("noise.wav");
        fs::write(&source, wav).unwrap();
        let flac = dir.path().join("encoded.flac");
        let encoded = Command::new("/usr/bin/afconvert")
            .args(["-f", "flac", "-d", "flac"])
            .args([&source, &flac])
            .status()
            .unwrap();
        assert!(encoded.success());

        assert_eq!(decodes_the_same(&flac), data);
        // As many frames as Core Audio counts packets
        let info = Command::new("/usr/bin/afinfo").arg(&flac).output().unwrap();
        let info = String::from_utf8_lossy(&info.stdout);
        let packets: usize = info
            .lines()
            .find_map(|line| line.trim().strip_prefix("audio packets: "))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(read_frames(&flac).unwrap().sizes.len(), packets);
    }
}
