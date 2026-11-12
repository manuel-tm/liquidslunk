//! A FLAC file as an MP4 (backend.md, "Serving FLAC as MP4"): a header made
//! from the file's frames, then the frames, byte for byte. WebKit seeks FLAC
//! by estimate, and an MP4 exactly, by its table of samples. Nothing is
//! decoded, and nothing is written: the MP4 is never whole, only its header
//! is kept, and the rest is read from the file as it's asked for.
//!
//! It follows Xiph's encapsulation of FLAC in the ISO base media file format,
//! as Core Audio writes it: one audio track, each FLAC frame one sample, with
//! the `fLaC` sample entry, and STREAMINFO in its `dfLa` box.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::flac::{FlacError, FlacFrames, read_frames};

/// A FLAC file, served as an MP4 that holds its frames.
#[derive(Debug)]
pub struct FlacMp4 {
    path: PathBuf,
    header: Vec<u8>,
    /// Where the frames are in the file
    frames_start: u64,
    frames_length: u64,
}

impl FlacMp4 {
    /// Reads the file's frames, and makes the MP4's header from them.
    pub fn new(path: &Path) -> Result<FlacMp4, FlacError> {
        let frames = read_frames(path)?;
        Ok(FlacMp4 {
            path: path.to_path_buf(),
            header: header(&frames),
            frames_start: frames.start,
            frames_length: frames.end() - frames.start,
        })
    }

    /// The MP4's length.
    pub fn len(&self) -> u64 {
        self.header.len() as u64 + self.frames_length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The MP4's header, up to its frames.
    pub fn header(&self) -> &[u8] {
        &self.header
    }

    /// The MP4's bytes from `start` to `end`, both included: the header's,
    /// then the frames', read from the file.
    pub fn read(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        if start > end || end >= self.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the range is outside the MP4",
            ));
        }
        let mut body = Vec::with_capacity((end - start + 1) as usize);
        let header = self.header.len() as u64;
        if start < header {
            body.extend_from_slice(&self.header[start as usize..=end.min(header - 1) as usize]);
        }
        if end >= header {
            let from = start.max(header) - header;
            let mut frames = vec![0; (end - header - from + 1) as usize];
            let mut file = File::open(&self.path)?;
            file.seek(SeekFrom::Start(self.frames_start + from))?;
            file.read_exact(&mut frames)?;
            body.extend_from_slice(&frames);
        }
        Ok(body)
    }
}

/// About how long a chunk of frames is: a second.
const CHUNK_SECONDS: u64 = 1;

/// The MP4's header for these frames: `ftyp`, `moov`, and `mdat`'s own
/// header, which the frames follow.
pub fn header(frames: &FlacFrames) -> Vec<u8> {
    let chunks = chunks(frames);
    let ftyp = boxed(b"ftyp", &[&b"M4A "[..], &u32b(0), b"M4A mp42isom"].concat());
    let frames_length = frames.end() - frames.start;
    // 64-bit sizes and offsets only when they're needed
    let large_mdat = frames_length + 8 > u64::from(u32::MAX);
    let mdat_header = if large_mdat { 16 } else { 8 };
    let without_offsets = moov(frames, &chunks, 0, false).len() as u64;
    let large_offsets =
        ftyp.len() as u64 + without_offsets + mdat_header + frames_length > u64::from(u32::MAX);
    let moov_length = moov(frames, &chunks, 0, large_offsets).len() as u64;
    let frames_at = ftyp.len() as u64 + moov_length + mdat_header;

    let mut header = ftyp;
    header.extend(moov(frames, &chunks, frames_at, large_offsets));
    if large_mdat {
        header.extend(u32b(1));
        header.extend(b"mdat");
        header.extend((frames_length + 16).to_be_bytes());
    } else {
        header.extend(u32b((frames_length + 8) as u32));
        header.extend(b"mdat");
    }
    header
}

/// The chunks: runs of frames of about `CHUNK_SECONDS` each, as `(first
/// frame, frames)`.
fn chunks(frames: &FlacFrames) -> Vec<(usize, u32)> {
    let per_chunk = u64::from(frames.sample_rate) * CHUNK_SECONDS;
    let mut chunks = Vec::new();
    let (mut first, mut count, mut samples) = (0, 0u32, 0u64);
    for (index, block) in frame_samples(frames).enumerate() {
        if count == 0 {
            first = index;
        }
        count += 1;
        samples += u64::from(block);
        if samples >= per_chunk {
            chunks.push((first, count));
            (count, samples) = (0, 0);
        }
    }
    if count > 0 {
        chunks.push((first, count));
    }
    chunks
}

/// Each frame's samples, in order.
fn frame_samples(frames: &FlacFrames) -> impl Iterator<Item = u32> + '_ {
    frames
        .blocks
        .iter()
        .flat_map(|&(count, samples)| std::iter::repeat_n(samples, count as usize))
}

/// `moov`, with the frames at `frames_at` in the MP4.
fn moov(
    frames: &FlacFrames,
    chunks: &[(usize, u32)],
    frames_at: u64,
    large_offsets: bool,
) -> Vec<u8> {
    let samples = frames.samples();
    // Lengths and times in 64 bits only when they're needed
    let long = samples > u64::from(u32::MAX);
    let rate = frames.sample_rate;
    let times = |version_one: bool| -> Vec<u8> {
        if version_one {
            [u64b(0), u64b(0)].concat()
        } else {
            [u32b(0), u32b(0)].concat()
        }
    };
    let duration = |version_one: bool| -> Vec<u8> {
        if version_one {
            u64b(samples)
        } else {
            u32b(samples as u32)
        }
    };
    let version = u8::from(long);
    const MATRIX: [u32; 9] = [0x1_0000, 0, 0, 0, 0x1_0000, 0, 0, 0, 0x4000_0000];
    let matrix: Vec<u8> = MATRIX.iter().flat_map(|value| u32b(*value)).collect();

    // Every timescale is the sample rate. WebKit takes the length from
    // `mvhd` and `tkhd`, so in milliseconds, it would be cut to one.
    let mvhd = full_box(
        b"mvhd",
        version,
        0,
        &[
            times(long),
            u32b(rate),
            duration(long),
            u32b(0x1_0000), // rate: 1.0
            u16b(0x0100),   // volume: 1.0
            vec![0; 10],
            matrix.clone(),
            vec![0; 24],
            u32b(2), // the next track's id
        ]
        .concat(),
    );
    let tkhd = full_box(
        b"tkhd",
        version,
        0x7, // enabled, in the movie, in its preview
        &[
            times(long),
            u32b(1), // its id
            u32b(0),
            duration(long),
            vec![0; 8],
            u16b(0), // layer
            u16b(0), // alternate group
            u16b(0x0100),
            u16b(0),
            matrix,
            u32b(0), // width
            u32b(0), // height
        ]
        .concat(),
    );
    let mdhd = full_box(
        b"mdhd",
        version,
        0,
        &[
            times(long),
            u32b(rate),
            duration(long),
            u16b(0x55c4),
            u16b(0),
        ]
        .concat(), // "und"
    );
    let hdlr = full_box(
        b"hdlr",
        0,
        0,
        &[&u32b(0)[..], b"soun", &[0; 12], b"SoundHandler\0"].concat(),
    );
    let minf = boxed(
        b"minf",
        &[
            full_box(b"smhd", 0, 0, &[u16b(0), u16b(0)].concat()),
            boxed(
                b"dinf",
                &full_box(
                    b"dref",
                    0,
                    0,
                    &[u32b(1), full_box(b"url ", 0, 1, &[])].concat(),
                ),
            ),
            stbl(frames, chunks, frames_at, large_offsets),
        ]
        .concat(),
    );
    let mdia = boxed(b"mdia", &[mdhd, hdlr, minf].concat());
    let trak = boxed(b"trak", &[tkhd, mdia].concat());
    boxed(b"moov", &[mvhd, trak].concat())
}

/// The table of samples: one per frame.
fn stbl(
    frames: &FlacFrames,
    chunks: &[(usize, u32)],
    frames_at: u64,
    large_offsets: bool,
) -> Vec<u8> {
    // The sample entry's rate is 16.16 fixed point. A higher rate is halved
    // until it fits, as Core Audio writes it: the real one is STREAMINFO's,
    // and `mdhd`'s timescale.
    let mut entry_rate = frames.sample_rate;
    while entry_rate > u32::from(u16::MAX) {
        entry_rate /= 2;
    }
    let dfla = full_box(
        b"dfLa",
        0,
        0,
        // STREAMINFO, as the last metadata block
        &[&[0x80, 0, 0, 34][..], &frames.streaminfo].concat(),
    );
    let flac = boxed(
        b"fLaC",
        &[
            vec![0; 6],
            u16b(1), // the data reference
            vec![0; 8],
            u16b(u16::from(frames.channels)),
            u16b(u16::from(frames.bits_per_sample)),
            u16b(0),
            u16b(0),
            u32b(entry_rate << 16),
            dfla,
        ]
        .concat(),
    );
    let stsd = full_box(b"stsd", 0, 0, &[u32b(1), flac].concat());

    let stts_entries: Vec<u8> = frames
        .blocks
        .iter()
        .flat_map(|&(count, samples)| [u32b(count), u32b(samples)].concat())
        .collect();
    let stts = full_box(
        b"stts",
        0,
        0,
        &[u32b(frames.blocks.len() as u32), stts_entries].concat(),
    );

    // Runs of chunks with as many frames
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for (index, &(_, count)) in chunks.iter().enumerate() {
        if runs.last().is_none_or(|&(_, last)| last != count) {
            runs.push((index as u32 + 1, count));
        }
    }
    let stsc_entries: Vec<u8> = runs
        .iter()
        .flat_map(|&(first, count)| [u32b(first), u32b(count), u32b(1)].concat())
        .collect();
    let stsc = full_box(
        b"stsc",
        0,
        0,
        &[u32b(runs.len() as u32), stsc_entries].concat(),
    );

    let sizes: Vec<u8> = frames.sizes.iter().flat_map(|&size| u32b(size)).collect();
    let stsz = full_box(
        b"stsz",
        0,
        0,
        &[u32b(0), u32b(frames.sizes.len() as u32), sizes].concat(),
    );

    // Each chunk's offset in the MP4: where its first frame is
    let mut offsets = Vec::with_capacity(chunks.len());
    let mut offset = frames_at;
    let mut next = 0;
    for &(first, _) in chunks {
        while next < first {
            offset += u64::from(frames.sizes[next]);
            next += 1;
        }
        offsets.push(offset);
    }
    let chunk_offsets = if large_offsets {
        let entries: Vec<u8> = offsets.iter().flat_map(|&offset| u64b(offset)).collect();
        full_box(
            b"co64",
            0,
            0,
            &[u32b(offsets.len() as u32), entries].concat(),
        )
    } else {
        let entries: Vec<u8> = offsets
            .iter()
            .flat_map(|&offset| u32b(offset as u32))
            .collect();
        full_box(
            b"stco",
            0,
            0,
            &[u32b(offsets.len() as u32), entries].concat(),
        )
    };

    boxed(b"stbl", &[stsd, stts, stsc, stsz, chunk_offsets].concat())
}

fn boxed(kind: &[u8; 4], content: &[u8]) -> Vec<u8> {
    [&u32b((content.len() + 8) as u32)[..], kind, content].concat()
}

fn full_box(kind: &[u8; 4], version: u8, flags: u32, content: &[u8]) -> Vec<u8> {
    boxed(
        kind,
        &[&u32b((u32::from(version) << 24) | flags)[..], content].concat(),
    )
}

fn u16b(value: u16) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

fn u32b(value: u32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

fn u64b(value: u64) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}
