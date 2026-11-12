//! A FLAC file's frames, found in one pass over the file, so it can be
//! served as an MP4 that holds them (backend.md, "Serving FLAC as MP4"). It
//! only reads the file.
//!
//! A FLAC file is an optional ID3v2 tag, `fLaC`, its metadata blocks, then
//! its frames: each a header, a block of samples, and a CRC-16 of the whole
//! frame. A frame starts with a sync code, `0xFFF8`, or `0xFFF9` with
//! variable block sizes, and the same two bytes turn up inside frames too. So
//! a sync code starts the next frame only when its header is valid with a
//! matching CRC-8, its number follows the last frame's, and the bytes before
//! it end the last frame with a matching CRC-16.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use super::durations::skip_id3v2;

/// What the MP4 needs of a FLAC file: its STREAMINFO, and where each frame
/// is, with how many samples it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlacFrames {
    /// The STREAMINFO block's 34 bytes, as the file has them
    pub streaminfo: [u8; 34],
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    /// Where the first frame starts, in the file
    pub start: u64,
    /// Each frame's size, in order. They follow one another from `start`.
    pub sizes: Vec<u32>,
    /// Each frame's samples, in runs of frames with as many: `(frames,
    /// samples in each)`
    pub blocks: Vec<(u32, u32)>,
}

impl FlacFrames {
    /// Where the last frame ends, in the file.
    pub fn end(&self) -> u64 {
        self.start + self.sizes.iter().map(|&size| u64::from(size)).sum::<u64>()
    }

    /// The samples in every frame, per channel.
    pub fn samples(&self) -> u64 {
        self.blocks
            .iter()
            .map(|&(frames, samples)| u64::from(frames) * u64::from(samples))
            .sum()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FlacError {
    /// It doesn't start with `fLaC`, after any ID3v2 tags.
    #[error("it isn't a FLAC file")]
    NotFlac,
    /// It's a FLAC file whose frames can't be found.
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn invalid(message: impl Into<String>) -> FlacError {
    FlacError::Invalid(message.into())
}

/// Finds a FLAC file's frames.
pub fn read_frames(path: &Path) -> Result<FlacFrames, FlacError> {
    read_frames_from(BufReader::with_capacity(1 << 16, File::open(path)?))
}

/// Finds the frames of a FLAC stream that starts where `reader` is.
pub fn read_frames_from<R: Read + Seek>(mut reader: R) -> Result<FlacFrames, FlacError> {
    skip_id3v2(&mut reader).ok_or(FlacError::NotFlac)?;
    let mut marker = [0; 4];
    if reader.read_exact(&mut marker).is_err() || &marker != b"fLaC" {
        return Err(FlacError::NotFlac);
    }
    let streaminfo = read_metadata(&mut reader)?;
    let info = StreamInfo::parse(&streaminfo);
    if info.sample_rate == 0 {
        return Err(invalid("its STREAMINFO has no sample rate"));
    }
    let start = reader.stream_position()?;
    let mut frames = Scan::new(info, start).run(reader)?;
    if info.total_samples != 0 && frames.samples() != info.total_samples {
        return Err(invalid(format!(
            "its frames hold {} samples, and its STREAMINFO says {}",
            frames.samples(),
            info.total_samples
        )));
    }
    frames.streaminfo = streaminfo;
    Ok(frames)
}

/// Reads the metadata blocks, and returns STREAMINFO's, leaving `reader` at
/// the first frame.
fn read_metadata<R: Read + Seek>(reader: &mut R) -> Result<[u8; 34], FlacError> {
    let mut streaminfo = None;
    loop {
        let mut header = [0; 4];
        reader
            .read_exact(&mut header)
            .map_err(|_| invalid("its metadata is cut off"))?;
        let last = header[0] & 0x80 != 0;
        let kind = header[0] & 0x7f;
        let length = u32::from_be_bytes([0, header[1], header[2], header[3]]);
        match kind {
            0 if length == 34 && streaminfo.is_none() => {
                let mut block = [0; 34];
                reader
                    .read_exact(&mut block)
                    .map_err(|_| invalid("its metadata is cut off"))?;
                streaminfo = Some(block);
            }
            0 => return Err(invalid("its STREAMINFO isn't valid")),
            127 => return Err(invalid("a metadata block isn't valid")),
            _ => {
                reader.seek(SeekFrom::Current(i64::from(length)))?;
            }
        }
        if last {
            return streaminfo.ok_or_else(|| invalid("it has no STREAMINFO"));
        }
    }
}

/// What a frame's header is checked against.
#[derive(Debug, Clone, Copy)]
struct StreamInfo {
    sample_rate: u32,
    channels: u8,
    bits_per_sample: u8,
    /// 0 when the encoder didn't know
    total_samples: u64,
}

impl StreamInfo {
    fn parse(block: &[u8; 34]) -> StreamInfo {
        // After 10 bytes of block and frame sizes: the sample rate (20 bits),
        // channels - 1 (3), bits per sample - 1 (5), and samples (36)
        let packed = u64::from_be_bytes(block[10..18].try_into().unwrap_or_default());
        StreamInfo {
            sample_rate: (packed >> 44) as u32,
            channels: ((packed >> 41) & 0x7) as u8 + 1,
            bits_per_sample: ((packed >> 36) & 0x1f) as u8 + 1,
            total_samples: packed & 0xf_ffff_ffff,
        }
    }
}

/// A frame's header, as far as finding frames needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Header {
    /// Variable block sizes: `number` is the first sample's, not the frame's
    variable: bool,
    number: u64,
    samples: u32,
}

impl Header {
    /// Whether `next` is the frame after this one.
    fn followed_by(&self, next: &Header) -> bool {
        next.variable == self.variable
            && if self.variable {
                next.number == self.number + u64::from(self.samples)
            } else {
                next.number == self.number + 1
            }
    }
}

/// The longest a frame's header is: sync code and codes (4), the number (up
/// to 7), an uncommon block size (2) and sample rate (2), and its CRC-8.
const LONGEST_HEADER: usize = 16;

/// Reads the frame header that `bytes` starts with, when it's valid, its
/// CRC-8 matches, and it agrees with STREAMINFO.
fn read_header(bytes: &[u8], info: &StreamInfo) -> Option<Header> {
    if bytes.len() < 6 || bytes[0] != 0xff || bytes[1] & 0xfe != 0xf8 {
        return None;
    }
    let variable = bytes[1] & 1 == 1;
    let block_code = bytes[2] >> 4;
    let rate_code = bytes[2] & 0x0f;
    let channel_code = bytes[3] >> 4;
    let depth_code = (bytes[3] >> 1) & 0x7;
    if block_code == 0 || rate_code == 0xf || channel_code > 10 || depth_code == 3 {
        return None;
    }
    if bytes[3] & 1 != 0 {
        return None;
    }
    let channels = if channel_code < 8 {
        channel_code + 1
    } else {
        2
    };
    if channels != info.channels {
        return None;
    }
    let depth = [0, 8, 12, 0, 16, 20, 24, 32][usize::from(depth_code)];
    if depth != 0 && depth != info.bits_per_sample {
        return None;
    }
    let rate = [
        0, 88_200, 176_400, 192_000, 8_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000, 96_000,
    ]
    .get(usize::from(rate_code))
    .copied();
    if let Some(rate) = rate
        && rate != 0
        && rate != info.sample_rate
    {
        return None;
    }

    // The number, coded as UTF-8 is, up to 7 bytes
    let mut at = 4;
    let lead = bytes[at];
    let (mut number, more) = match lead {
        0x00..=0x7f => (u64::from(lead), 0),
        0xc0..=0xdf => (u64::from(lead & 0x1f), 1),
        0xe0..=0xef => (u64::from(lead & 0x0f), 2),
        0xf0..=0xf7 => (u64::from(lead & 0x07), 3),
        0xf8..=0xfb => (u64::from(lead & 0x03), 4),
        0xfc..=0xfd => (u64::from(lead & 0x01), 5),
        0xfe => (0, 6),
        _ => return None,
    };
    if !variable && more > 5 {
        return None;
    }
    at += 1;
    for _ in 0..more {
        let byte = *bytes.get(at)?;
        if byte & 0xc0 != 0x80 {
            return None;
        }
        number = (number << 6) | u64::from(byte & 0x3f);
        at += 1;
    }

    let samples = match block_code {
        1 => 192,
        2..=5 => 576 << (block_code - 2),
        6 => {
            at += 1;
            u32::from(*bytes.get(at - 1)?) + 1
        }
        7 => {
            at += 2;
            u32::from(u16::from_be_bytes([
                *bytes.get(at - 2)?,
                *bytes.get(at - 1)?,
            ])) + 1
        }
        _ => 256 << (block_code - 8),
    };
    match rate_code {
        0xc => at += 1,
        0xd | 0xe => at += 2,
        _ => {}
    }
    let crc = *bytes.get(at)?;
    (crc8(&bytes[..at]) == crc).then_some(Header {
        variable,
        number,
        samples,
    })
}

/// The pass over the frames. Each sync code that could start the next frame
/// is checked by its header first, then by the CRC-16 of the bytes since the
/// frame being read started, which is brought up to there only then.
struct Scan {
    info: StreamInfo,
    /// Where the frame being read started
    frame_start: u64,
    /// Its header, once the first frame's is read
    current: Option<Header>,
    /// The CRC-16 of the bytes from `frame_start` up to `crc_to`
    crc: u16,
    crc_to: u64,
    frames: FlacFrames,
}

/// How much is read at once.
const CHUNK: usize = 1 << 20;

impl Scan {
    fn new(info: StreamInfo, start: u64) -> Scan {
        Scan {
            info,
            frame_start: start,
            current: None,
            crc: 0,
            crc_to: start,
            frames: FlacFrames {
                streaminfo: [0; 34],
                sample_rate: info.sample_rate,
                channels: info.channels,
                bits_per_sample: info.bits_per_sample,
                start,
                sizes: Vec::new(),
                blocks: Vec::new(),
            },
        }
    }

    fn run<R: Read + Seek>(mut self, mut reader: R) -> Result<FlacFrames, FlacError> {
        let mut buffer: Vec<u8> = Vec::with_capacity(CHUNK + LONGEST_HEADER);
        // Where `buffer[0]` is in the file
        let mut base = self.frame_start;
        let mut ended = false;
        loop {
            // Keeps the bytes a header at the end of the last chunk may need
            let mut filled = buffer.len();
            buffer.resize(CHUNK + LONGEST_HEADER, 0);
            while !ended && filled < buffer.len() {
                match reader.read(&mut buffer[filled..]) {
                    Ok(0) => ended = true,
                    Ok(read) => filled += read,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error.into()),
                }
            }
            buffer.truncate(filled);
            if self.current.is_none() {
                let header = read_header(&buffer, &self.info)
                    .ok_or_else(|| invalid("there's no frame where its metadata ends"))?;
                self.current = Some(header);
            }
            let scanned = if ended {
                buffer.len()
            } else {
                buffer.len() - LONGEST_HEADER
            };
            self.scan(&buffer, scanned, base)?;
            if ended {
                break;
            }
            buffer.drain(..scanned);
            base += scanned as u64;
        }
        let end = base + buffer.len() as u64;
        self.finish(&mut reader, end)
    }

    /// Goes through `buffer[..scanned]`, which starts at `base` in the file.
    /// What's after `scanned` is only there for a header to be read whole.
    fn scan(&mut self, buffer: &[u8], scanned: usize, base: u64) -> Result<(), FlacError> {
        let mut from = 0;
        while let Some(found) = find_ff(&buffer[from..scanned]) {
            let index = from + found;
            from = index + 1;
            let at = base + index as u64;
            if at <= self.frame_start
                || buffer.get(index + 1).is_none_or(|next| next & 0xfe != 0xf8)
            {
                continue;
            }
            let Some(header) = read_header(&buffer[index..], &self.info) else {
                continue;
            };
            if !self
                .current
                .is_some_and(|current| current.followed_by(&header))
            {
                continue;
            }
            self.crc = crc16(self.crc, &buffer[(self.crc_to - base) as usize..index]);
            self.crc_to = at;
            if self.crc == 0 {
                self.push(at)?;
                self.current = Some(header);
            }
        }
        // The CRC is brought up to the end, before these bytes are dropped
        self.crc = crc16(self.crc, &buffer[(self.crc_to - base) as usize..scanned]);
        self.crc_to = base + scanned as u64;
        Ok(())
    }

    /// Ends the frame being read at `end`, where the next one starts.
    fn push(&mut self, end: u64) -> Result<(), FlacError> {
        let size =
            u32::try_from(end - self.frame_start).map_err(|_| invalid("a frame is too large"))?;
        self.frames.sizes.push(size);
        let samples = self.current.map_or(0, |current| current.samples);
        match self.frames.blocks.last_mut() {
            Some((frames, last)) if *last == samples => *frames += 1,
            _ => self.frames.blocks.push((1, samples)),
        }
        self.frame_start = end;
        self.crc = 0;
        self.crc_to = end;
        Ok(())
    }

    /// The last frame ends at `end`, the end of the file, or right before
    /// an ID3v1 tag that ends it, where its CRC-16 matches. Otherwise it's
    /// cut off, or followed by something unknown, which could be mistaken
    /// for its end: a CRC-16 comes to 0 by chance about every 64 KB.
    fn finish<R: Read + Seek>(mut self, reader: &mut R, end: u64) -> Result<FlacFrames, FlacError> {
        if end > self.frame_start && self.crc == 0 {
            self.push(end)?;
            return Ok(self.frames);
        }
        if end >= self.frame_start + 128 {
            let mut tag = [0; 128];
            reader.seek(SeekFrom::Start(end - 128))?;
            reader.read_exact(&mut tag)?;
            if &tag[..3] == b"TAG" {
                let mut last = vec![0; (end - 128 - self.frame_start) as usize];
                reader.seek(SeekFrom::Start(self.frame_start))?;
                reader.read_exact(&mut last)?;
                if !last.is_empty() && crc16(0, &last) == 0 {
                    self.push(end - 128)?;
                    return Ok(self.frames);
                }
            }
        }
        Err(invalid("its last frame is cut off"))
    }
}

/// Where the first `0xFF` is, looked for eight bytes at a time.
fn find_ff(bytes: &[u8]) -> Option<usize> {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const HIGHS: u64 = 0x8080_8080_8080_8080;
    let (words, rest) = bytes.as_chunks::<8>();
    for (index, word) in words.iter().enumerate() {
        // A byte of 0xFF is a byte of 0 in the inverse. The lowest flag is
        // the first such byte, since bytes are read little-endian.
        let inverse = !u64::from_le_bytes(*word);
        let zeros = inverse.wrapping_sub(ONES) & !inverse & HIGHS;
        if zeros != 0 {
            return Some(index * 8 + (zeros.trailing_zeros() / 8) as usize);
        }
    }
    let at = bytes.len() - rest.len();
    rest.iter()
        .position(|&byte| byte == 0xff)
        .map(|index| at + index)
}

const fn crc8_table() -> [u8; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = index as u8;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
}

/// Tables for the CRC-16, eight bytes at a time: `CRC16[k][byte]` is what
/// `byte` adds to the CRC with `k` bytes after it.
const fn crc16_tables() -> [[u16; 256]; 8] {
    let mut tables = [[0; 256]; 8];
    let mut index = 0;
    while index < 256 {
        let mut crc = (index as u16) << 8;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x8005
            } else {
                crc << 1
            };
            bit += 1;
        }
        tables[0][index] = crc;
        index += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut index = 0;
        while index < 256 {
            let previous = tables[k - 1][index];
            tables[k][index] = (previous << 8) ^ tables[0][(previous >> 8) as usize];
            index += 1;
        }
        k += 1;
    }
    tables
}

const CRC8: [u8; 256] = crc8_table();
const CRC16: [[u16; 256]; 8] = crc16_tables();

/// A frame header's CRC: polynomial 0x07, from 0.
pub fn crc8(bytes: &[u8]) -> u8 {
    bytes
        .iter()
        .fold(0, |crc, &byte| CRC8[usize::from(crc ^ byte)])
}

/// A frame's CRC, carried on over `bytes`: polynomial 0x8005, from 0. Over a
/// whole frame, its own CRC included, it comes to 0.
pub fn crc16(mut crc: u16, bytes: &[u8]) -> u16 {
    let (words, rest) = bytes.as_chunks::<8>();
    for word in words {
        let [high, low] = crc.to_be_bytes();
        crc = CRC16[7][usize::from(word[0] ^ high)]
            ^ CRC16[6][usize::from(word[1] ^ low)]
            ^ CRC16[5][usize::from(word[2])]
            ^ CRC16[4][usize::from(word[3])]
            ^ CRC16[3][usize::from(word[4])]
            ^ CRC16[2][usize::from(word[5])]
            ^ CRC16[1][usize::from(word[6])]
            ^ CRC16[0][usize::from(word[7])];
    }
    for &byte in rest {
        crc = (crc << 8) ^ CRC16[0][usize::from((crc >> 8) as u8 ^ byte)];
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> StreamInfo {
        StreamInfo {
            sample_rate: 44_100,
            channels: 2,
            bits_per_sample: 16,
            total_samples: 0,
        }
    }

    /// A header: fixed blocks of 4096, 44.1 kHz, stereo, 16 bits.
    fn header(number: u8) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xf8, 0xc9, 0x18, number];
        bytes.push(crc8(&bytes));
        bytes
    }

    #[test]
    fn headers_are_read_with_their_crc() {
        let bytes = header(5);
        assert_eq!(
            read_header(&bytes, &info()),
            Some(Header {
                variable: false,
                number: 5,
                samples: 4096
            })
        );
        let mut wrong = bytes.clone();
        *wrong.last_mut().unwrap() ^= 1;
        assert_eq!(read_header(&wrong, &info()), None);
    }

    #[test]
    fn headers_must_agree_with_streaminfo() {
        let mono = StreamInfo {
            channels: 1,
            ..info()
        };
        assert_eq!(read_header(&header(0), &mono), None);
        let rate = StreamInfo {
            sample_rate: 48_000,
            ..info()
        };
        assert_eq!(read_header(&header(0), &rate), None);
    }

    #[test]
    fn numbers_are_coded_as_utf8() {
        // Frame 200: two bytes, 0xc3 0x88
        let mut bytes = vec![0xff, 0xf8, 0xc9, 0x18, 0xc3, 0x88];
        bytes.push(crc8(&bytes));
        assert_eq!(read_header(&bytes, &info()).map(|h| h.number), Some(200));
    }

    #[test]
    fn crc16_is_the_same_eight_bytes_at_a_time() {
        let bytes: Vec<u8> = (0..1000u32).map(|n| (n * 7919 % 251) as u8).collect();
        let one_by_one = bytes.iter().fold(0, |crc, &byte| crc16(crc, &[byte]));
        assert_eq!(crc16(0, &bytes), one_by_one);
        // In pieces that don't line up with the eights
        let (first, rest) = bytes.split_at(13);
        assert_eq!(crc16(crc16(0, first), rest), one_by_one);
    }

    #[test]
    fn ff_is_found_eight_bytes_at_a_time() {
        let mut bytes = vec![0u8; 40];
        assert_eq!(find_ff(&bytes), None);
        for at in [0, 7, 8, 15, 21, 39] {
            bytes.fill(0xfe);
            bytes[at] = 0xff;
            assert_eq!(find_ff(&bytes), Some(at));
        }
        bytes[3] = 0xff;
        assert_eq!(find_ff(&bytes), Some(3));
    }

    #[test]
    fn crc16_comes_to_zero_over_a_frame_and_its_crc() {
        let bytes = b"some frame";
        let crc = crc16(0, bytes);
        let whole = [&bytes[..], &crc.to_be_bytes()].concat();
        assert_eq!(crc16(0, &whole), 0);
    }
}
