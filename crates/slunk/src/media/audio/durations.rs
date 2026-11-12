//! Durations lofty works out differently from music-metadata, worked out as
//! music-metadata did, so they match slunk's to the millisecond:
//!
//! - FLAC and AIFF: lofty cuts them to the millisecond, and music-metadata
//!   rounded the sample count over the rate.
//! - MP3: lofty estimates one without an Info or Xing header from the
//!   bitrate. music-metadata read four frames: at a constant bitrate, it
//!   estimated from the file's size and the fourth frame's, and otherwise it
//!   counted every frame.
//! - AAC in ADTS (`.aac`): music-metadata read it with the same parser, and
//!   counted every frame.

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// As music-metadata worked it out: `Math.round(samples / rate * 1000)`.
fn rounded(samples: f64, rate: f64) -> Option<i64> {
    (rate > 0.0).then(|| ((samples / rate) * 1000.0).round() as i64)
}

/// A FLAC file's duration, from its STREAMINFO block, after any ID3v2 tags.
pub fn flac(path: &Path) -> Option<i64> {
    let mut file = BufReader::new(File::open(path).ok()?);
    skip_id3v2(&mut file)?;
    let mut header = [0; 4 + 4 + 18];
    file.read_exact(&mut header).ok()?;
    if &header[..4] != b"fLaC" || header[4] & 0x7f != 0 {
        return None;
    }
    // After the block's header: 10 bytes of sizes, then the sample rate (20
    // bits), channels (3), bits per sample (5) and samples (36)
    let info = &header[8 + 10..];
    let rate = (u32::from(info[0]) << 12) | (u32::from(info[1]) << 4) | (u32::from(info[2]) >> 4);
    let samples = (u64::from(info[3] & 0x0f) << 32)
        | (u64::from(info[4]) << 24)
        | (u64::from(info[5]) << 16)
        | (u64::from(info[6]) << 8)
        | u64::from(info[7]);
    rounded(samples as f64, f64::from(rate))
}

/// Skips the ID3v2 tags at the start of a file, as music-metadata did.
fn skip_id3v2<R: Read + Seek>(file: &mut R) -> Option<()> {
    loop {
        let mut header = [0; 10];
        let start = file.stream_position().ok()?;
        if file.read_exact(&mut header).is_err() || &header[..3] != b"ID3" {
            file.seek(SeekFrom::Start(start)).ok()?;
            return Some(());
        }
        let size = header[6..10]
            .iter()
            .fold(0u64, |size, byte| (size << 7) | u64::from(byte & 0x7f));
        let footer = if header[5] & 0x10 != 0 { 10 } else { 0 };
        file.seek(SeekFrom::Current((size + footer) as i64)).ok()?;
    }
}

/// An AIFF file's duration, from its COMM chunk.
pub fn aiff(path: &Path) -> Option<i64> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let mut form = [0; 12];
    file.read_exact(&mut form).ok()?;
    if &form[..4] != b"FORM" || !matches!(&form[8..], b"AIFF" | b"AIFC") {
        return None;
    }
    loop {
        let mut chunk = [0; 8];
        file.read_exact(&mut chunk).ok()?;
        let size = u32::from_be_bytes(chunk[4..].try_into().ok()?);
        if &chunk[..4] == b"COMM" {
            let mut comm = [0; 18];
            file.read_exact(&mut comm).ok()?;
            let frames = u32::from_be_bytes(comm[2..6].try_into().ok()?);
            let rate = extended(comm[8..18].try_into().ok()?);
            return rounded(f64::from(frames), rate);
        }
        // Chunks are padded to an even size
        file.seek(SeekFrom::Current(i64::from(size + (size & 1))))
            .ok()?;
    }
}

/// An 80-bit IEEE 754 extended float, as AIFF stores its sample rate.
fn extended(bytes: [u8; 10]) -> f64 {
    let sign = if bytes[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    let exponent = (i32::from(bytes[0] & 0x7f) << 8) | i32::from(bytes[1]);
    let mantissa = u64::from_be_bytes(bytes[2..].try_into().expect("8 bytes"));
    if exponent == 0 && mantissa == 0 {
        return 0.0;
    }
    sign * mantissa as f64 * 2f64.powi(exponent - 16383 - 63)
}

/// An MPEG audio frame's header, as music-metadata read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameHeader {
    bitrate: u32,
    sample_rate: u32,
    samples_per_frame: u32,
    /// Including the header
    size: u64,
    side_info: u64,
    protected: bool,
}

impl FrameHeader {
    /// The header in `bytes`, which start with 0xFF, or `None` when it has a
    /// reserved value, or a bitrate music-metadata didn't know.
    fn parse(bytes: [u8; 4]) -> Option<FrameHeader> {
        // MPEG 2.5, reserved, MPEG 2, MPEG 1
        let version = match (bytes[1] >> 3) & 3 {
            0 => 25,
            2 => 20,
            3 => 10,
            _ => return None,
        };
        let layer = match (bytes[1] >> 1) & 3 {
            1 => 3,
            2 => 2,
            3 => 1,
            _ => return None,
        };
        let bitrate_index = usize::from(bytes[2] >> 4);
        let rate_index = usize::from((bytes[2] >> 2) & 3);
        let padding = (bytes[2] >> 1) & 1 == 1;
        let mono = bytes[3] >> 6 == 3;
        // In kbps, by index from 1: MPEG 1 layers I, II and III, then MPEG 2
        // and 2.5's layer I, then their layers II and III
        const BITRATES: [[u32; 4]; 14] = [
            [32, 32, 32, 32],
            [64, 48, 40, 48],
            [96, 56, 48, 56],
            [128, 64, 56, 64],
            [160, 80, 64, 80],
            [192, 96, 80, 96],
            [224, 112, 96, 112],
            [256, 128, 112, 128],
            [288, 160, 128, 144],
            [320, 192, 160, 160],
            [352, 224, 192, 176],
            [384, 256, 224, 192],
            [416, 320, 256, 224],
            [448, 384, 320, 256],
        ];
        const LOWER: [u32; 14] = [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
        let row = bitrate_index.checked_sub(1).filter(|row| *row < 14)?;
        let kbps = match (version, layer) {
            (10, layer) => BITRATES[row][layer - 1],
            (_, 1) => BITRATES[row][3],
            _ => LOWER[row],
        };
        let sample_rate = match (version, rate_index) {
            (10, 0) => 44100,
            (10, 1) => 48000,
            (10, 2) => 32000,
            (20, 0) => 22050,
            (20, 1) => 24000,
            (20, 2) => 16000,
            (25, 0) => 11025,
            (25, 1) => 12000,
            (25, 2) => 8000,
            _ => return None,
        };
        let samples_per_frame = match (layer, version) {
            (1, _) => 384,
            (2, _) | (3, 10) => 1152,
            _ => 576,
        };
        let bitrate = kbps * 1000;
        let slot = if layer == 1 { 4.0 } else { 1.0 };
        let size = (f64::from(samples_per_frame) / 8.0 * f64::from(bitrate)
            / f64::from(sample_rate)
            + if padding { slot } else { 0.0 })
        .floor() as u64;
        let side_info = match (layer, version, mono) {
            (1 | 2, _, _) => 2,
            (_, 10, true) => 17,
            (_, 10, false) => 32,
            (_, _, true) => 9,
            (_, _, false) => 17,
        };
        Some(FrameHeader {
            bitrate,
            sample_rate,
            samples_per_frame,
            size,
            side_info,
            protected: bytes[1] & 1 == 0,
        })
    }
}

/// An ADTS frame's sample rates, by index. The rest are reserved.
const ADTS_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// How far ahead music-metadata looked for a frame's sync.
const PEEK: usize = 1024;

/// An MP3 or ADTS file's duration, as music-metadata's MPEG parser worked
/// it out with its `duration` option on, as slunk asked for it.
pub fn mpeg(path: &Path) -> Option<i64> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let size = file.get_ref().metadata().ok()?.len();
    let has_id3v1 = has_id3v1(&mut file, size);
    skip_id3v2(&mut file)?;
    let mut parser = Mpeg {
        file,
        frame_count: 0,
        sync_frame_count: None,
        mpeg_offset: 0,
        frame: None,
        first_bitrate: None,
        constant_bitrate: true,
        samples_per_frame: None,
        sample_rate: None,
        count_to_end: false,
    };
    match parser.run() {
        Ending::Xing(samples, rate) => rounded(samples, rate),
        Ending::Constant => {
            // `finalize()`: the size of the stream, from the first frame,
            // over the fourth frame's size
            let frame = parser.frame?;
            let stream = size.checked_sub(parser.mpeg_offset + if has_id3v1 { 128 } else { 0 })?;
            let frames = (stream as f64 / frame.size as f64).round();
            let samples = frames * f64::from(frame.samples_per_frame);
            rounded(samples, f64::from(frame.sample_rate))
        }
        Ending::EndOfStream if parser.count_to_end => {
            let samples = parser.frame_count as f64 * f64::from(parser.samples_per_frame?);
            rounded(samples, f64::from(parser.sample_rate?))
        }
        Ending::EndOfStream => None,
    }
}

fn has_id3v1(file: &mut BufReader<File>, size: u64) -> bool {
    let mut tag = [0; 3];
    let found = size >= 128
        && file.seek(SeekFrom::Start(size - 128)).is_ok()
        && file.read_exact(&mut tag).is_ok()
        && &tag == b"TAG";
    let _ = file.seek(SeekFrom::Start(0));
    found
}

/// How the parser stopped.
enum Ending {
    /// An Info or Xing header gave the frames: samples and rate
    Xing(f64, f64),
    /// Four frames of the same bitrate
    Constant,
    EndOfStream,
}

/// music-metadata's MpegParser, for the duration only.
struct Mpeg {
    file: BufReader<File>,
    frame_count: u64,
    sync_frame_count: Option<u64>,
    /// Where the first frame starts
    mpeg_offset: u64,
    /// The last frame read
    frame: Option<FrameHeader>,
    first_bitrate: Option<u32>,
    constant_bitrate: bool,
    samples_per_frame: Option<u32>,
    /// The last frame's
    sample_rate: Option<u32>,
    count_to_end: bool,
}

impl Mpeg {
    fn run(&mut self) -> Ending {
        loop {
            let Some(header) = self.sync() else {
                return Ending::EndOfStream;
            };
            if self.frame_count == 0 {
                self.mpeg_offset = self.position() - 1;
            }
            let mut tail = [0; 3];
            if self.peek(&mut tail) < 3 {
                return Ending::EndOfStream;
            }
            let bytes = [0xff, header, tail[1], tail[2]];
            if bytes[1] & 0x16 == 0x10 {
                self.skip(3);
                self.frame_count += 1;
                self.adts(bytes);
                continue;
            }
            let Some(frame) = FrameHeader::parse(bytes) else {
                // `parseFrameHeader()` skips a byte and syncs again
                self.skip(1);
                continue;
            };
            self.skip(3);
            self.frame_count += 1;
            match self.frame(frame) {
                Some(ending) => return ending,
                None => continue,
            }
        }
    }

    /// `parseAdts()`: skips the frame, whose length is in its header, and
    /// counts frames to the end from the third.
    fn adts(&mut self, bytes: [u8; 4]) {
        self.sample_rate = ADTS_RATES.get(usize::from((bytes[2] >> 2) & 0xf)).copied();
        let mut rest = [0; 3];
        self.read(&mut rest);
        let length = (u64::from(bytes[3] & 3) << 11)
            + ((u64::from(rest[0]) << 3) | (u64::from(rest[1]) >> 5));
        self.skip(if length > 7 { length - 7 } else { 1 });
        self.samples_per_frame = Some(1024);
        if self.frame_count == 3 {
            self.count_to_end = true;
        }
    }

    /// `parseMpegFrame()`: `Some` when the parser stops.
    fn frame(&mut self, frame: FrameHeader) -> Option<Ending> {
        self.frame = Some(frame);
        self.sample_rate = Some(frame.sample_rate);
        let first = *self.first_bitrate.get_or_insert(frame.bitrate);
        self.constant_bitrate &= frame.bitrate == first;
        if self.frame_count == 1 {
            // The Info or Xing header follows the side information, even
            // when the frame has a CRC
            self.skip(frame.side_info);
            return self.xing(frame, 4 + frame.side_info);
        }
        if self.frame_count == 4 {
            if self.constant_bitrate {
                self.samples_per_frame = Some(frame.samples_per_frame);
                return Some(Ending::Constant);
            }
            self.samples_per_frame = Some(frame.samples_per_frame);
            self.count_to_end = true;
        }
        let mut offset = 4;
        if frame.protected {
            self.skip(2);
            offset += 2;
        }
        self.skip(frame.side_info);
        self.xing(frame, offset + frame.side_info)
    }

    /// `readXtraInfoHeader()`: reads the four bytes after the side
    /// information, and an Info or Xing header if that's what they are.
    fn xing(&mut self, frame: FrameHeader, offset: u64) -> Option<Ending> {
        let mut tag = [0; 4];
        if self.read(&mut tag) < 4 {
            return None;
        }
        let offset = offset + 4;
        match &tag {
            b"Info" | b"Xing" => {
                let mut flags = [0; 4];
                self.read(&mut flags);
                let flags = u32::from_be_bytes(flags);
                let mut field = || {
                    let mut value = [0; 4];
                    self.read(&mut value);
                    u32::from_be_bytes(value)
                };
                let frames = (flags & 1 != 0).then(&mut field);
                let bytes = (flags & 2 != 0).then(&mut field);
                // The table of contents and the quality, read past as
                // music-metadata did, so the next sync starts where it did
                if flags & 4 != 0 {
                    self.skip(100);
                }
                if flags & 8 != 0 {
                    self.skip(4);
                }
                // Only with both, which LAME always writes
                if let (Some(frames), Some(bytes)) = (frames, bytes)
                    && bytes != 0
                {
                    let samples = f64::from(frames) * f64::from(frame.samples_per_frame);
                    return Some(Ending::Xing(samples, f64::from(frame.sample_rate)));
                }
                None
            }
            b"LAME" => {
                // The rest of the frame, after the encoder's version
                self.skip(frame.size.saturating_sub(offset));
                None
            }
            _ => {
                self.skip(frame.size.saturating_sub(offset));
                None
            }
        }
    }

    /// `sync()`: the second byte of the next frame's header, after a 0xFF,
    /// with the position on it. `None` at the end of the stream, which is
    /// within 163 bytes of the end of the file.
    fn sync(&mut self) -> Option<u8> {
        let mut first_sync = false;
        loop {
            let mut buffer = [0; PEEK];
            let length = self.peek(&mut buffer);
            if length <= 163 {
                return None;
            }
            let mut at = 0;
            loop {
                if first_sync && at < length && buffer[at] & 0xe0 == 0xe0 {
                    self.skip(at as u64);
                    if self.sync_frame_count == Some(self.frame_count) {
                        // Synced again without a frame between: start over
                        self.frame_count = 0;
                    }
                    self.sync_frame_count = Some(self.frame_count);
                    return Some(buffer[at]);
                }
                first_sync = false;
                match buffer[at.min(length)..length]
                    .iter()
                    .position(|byte| *byte == 0xff)
                {
                    Some(found) => {
                        at += found + 1;
                        first_sync = true;
                    }
                    None => {
                        if length < PEEK {
                            return None;
                        }
                        self.skip(length as u64);
                        break;
                    }
                }
            }
        }
    }

    fn position(&mut self) -> u64 {
        self.file.stream_position().unwrap_or(0)
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        let start = self.position();
        let length = self.read(buffer);
        let _ = self.file.seek(SeekFrom::Start(start));
        length
    }

    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let mut length = 0;
        while length < buffer.len() {
            match self.file.read(&mut buffer[length..]) {
                Ok(0) | Err(_) => break,
                Ok(read) => length += read,
            }
        }
        length
    }

    fn skip(&mut self, bytes: u64) {
        let _ = self.file.seek_relative(bytes as i64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_floats_decode() {
        // 44100 and 48000, as AIFF stores them
        assert_eq!(
            extended([0x40, 0x0e, 0xac, 0x44, 0, 0, 0, 0, 0, 0]),
            44100.0
        );
        assert_eq!(
            extended([0x40, 0x0e, 0xbb, 0x80, 0, 0, 0, 0, 0, 0]),
            48000.0
        );
    }

    #[test]
    fn frame_headers_read_as_music_metadata_read_them() {
        // MPEG 1 layer III, 128 kbps, 44.1 kHz, padded, stereo
        let frame = FrameHeader::parse([0xff, 0xfb, 0x92, 0x00]).unwrap();
        assert_eq!(
            frame,
            FrameHeader {
                bitrate: 128_000,
                sample_rate: 44100,
                samples_per_frame: 1152,
                size: 418,
                side_info: 32,
                protected: false,
            }
        );
        // MPEG 2 layer III, 64 kbps, 22.05 kHz, mono
        let frame = FrameHeader::parse([0xff, 0xf3, 0x80, 0xc0]).unwrap();
        assert_eq!(
            (frame.bitrate, frame.samples_per_frame, frame.side_info),
            (64_000, 576, 9)
        );
        // A free or bad bitrate, and a reserved version
        assert_eq!(FrameHeader::parse([0xff, 0xfb, 0x00, 0x00]), None);
        assert_eq!(FrameHeader::parse([0xff, 0xfb, 0xf0, 0x00]), None);
        assert_eq!(FrameHeader::parse([0xff, 0xeb, 0x90, 0x00]), None);
    }
}
