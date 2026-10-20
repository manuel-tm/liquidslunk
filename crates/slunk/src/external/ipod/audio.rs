//! What the iPod's database needs to know of a file it gets
//! (ipod.md, "Gapless and Sound Check"), read from the file liquidslunk wrote:
//! its kind, length, bitrate and sample rate; its Sound Check, which
//! `afconvert` adds to what it makes; and what gapless playback needs, which
//! iTunes worked out as it copied a file.
//! - An AAC file's encoder delay and padding, and its samples, are in its
//!   `iTunSMPB` atom.
//! - An Apple Lossless file has no delay or padding: only its samples.
//! - A copied MP3's are in its LAME header, with `gapless_data`: the bytes
//!   from its first frame to the 8th before its last, as gtkpod worked it
//!   out.

use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::AudioFile;
use lofty::mp4::{AtomData, AtomIdent, Mp4Codec, Mp4File};
use lofty::mpeg::MpegFile;

use super::itunesdb::{FileKind, Gapless};

/// What a file's database record says of its audio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts {
    pub kind: Option<FileKind>,
    pub length_ms: u32,
    /// In kbps
    pub bitrate: u32,
    pub sample_rate: u32,
    pub sound_check: u32,
    pub gapless: Gapless,
}

/// Reads a file's facts, or why it can't go on an iPod.
pub fn read(path: &Path) -> Result<Facts, String> {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let unreadable = |error: &dyn std::fmt::Display| format!("Its file can't be read: {error}");
    let mut file = File::open(path).map_err(|error| unreadable(&error))?;
    let options = ParseOptions::new().read_cover_art(false);
    match extension.as_str() {
        "m4a" | "mp4" => {
            let parsed =
                Mp4File::read_from(&mut file, options).map_err(|error| unreadable(&error))?;
            let properties = parsed.properties();
            let kind = match properties.codec() {
                Some(Mp4Codec::AAC) => FileKind::Aac,
                Some(Mp4Codec::ALAC) => FileKind::Alac,
                _ => return Err("The iPod plays only AAC and Apple Lossless in an M4A".into()),
            };
            let freeform = |name: &'static str| -> Option<String> {
                let ident = AtomIdent::Freeform {
                    mean: Cow::Borrowed("com.apple.iTunes"),
                    name: Cow::Borrowed(name),
                };
                parsed
                    .ilst()?
                    .get(&ident)?
                    .data()
                    .find_map(|data| match data {
                        AtomData::UTF8(text) => Some(text.clone()),
                        _ => None,
                    })
            };
            let smpb = freeform("iTunSMPB").and_then(|text| itunsmpb(&text));
            let gapless = match (kind, smpb) {
                (FileKind::Aac, Some(gapless)) => gapless,
                (FileKind::Alac, Some(gapless)) => Gapless {
                    sample_count: gapless.sample_count,
                    ..Gapless::default()
                },
                _ => Gapless::default(),
            };
            let sample_rate = properties.sample_rate().unwrap_or(0);
            // The samples played, without the encoder's delay and padding,
            // which lofty counts in
            let length_ms = match gapless.sample_count {
                0 => properties.duration().as_millis() as u32,
                samples if sample_rate > 0 => (samples * 1000 / u64::from(sample_rate)) as u32,
                _ => properties.duration().as_millis() as u32,
            };
            // Apple Lossless records none: its average, from its size
            let bitrate = properties
                .audio_bitrate()
                .filter(|bitrate| *bitrate > 0)
                .or_else(|| {
                    let bits = std::fs::metadata(path).ok()?.len() * 8;
                    (length_ms > 0).then(|| (bits / u64::from(length_ms)) as u32)
                })
                .unwrap_or(0);
            Ok(Facts {
                kind: Some(kind),
                length_ms,
                bitrate,
                sample_rate,
                sound_check: freeform("iTunNORM")
                    .and_then(|text| itunnorm(&text))
                    .unwrap_or(0),
                gapless,
            })
        }
        "mp3" => {
            let parsed =
                MpegFile::read_from(&mut file, options).map_err(|error| unreadable(&error))?;
            let properties = parsed.properties();
            Ok(Facts {
                kind: Some(FileKind::Mp3),
                length_ms: properties.duration().as_millis() as u32,
                bitrate: properties.audio_bitrate(),
                sample_rate: properties.sample_rate(),
                sound_check: 0,
                gapless: mp3_gapless(path).unwrap_or_default(),
            })
        }
        _ => Err("The iPod plays only AAC, Apple Lossless and MP3".into()),
    }
}

/// `iTunSMPB`: " 00000000 00000840 0000026C 0000000000035D54 …", in hex:
/// a zero, the encoder's delay, its padding, and the samples.
fn itunsmpb(text: &str) -> Option<Gapless> {
    let fields: Vec<&str> = text.split_whitespace().collect();
    let hex32 = |index: usize| u32::from_str_radix(fields.get(index)?, 16).ok();
    Some(Gapless {
        pregap: hex32(1)?,
        postgap: hex32(2)?,
        sample_count: u64::from_str_radix(fields.get(3)?, 16).ok()?,
        data: 0,
    })
}

/// `iTunNORM`: ten numbers in hex, the first two the left and right
/// channels' Sound Check, which the iPod takes the louder of.
fn itunnorm(text: &str) -> Option<u32> {
    let fields: Vec<u32> = text
        .split_whitespace()
        .map(|field| u32::from_str_radix(field, 16).ok())
        .collect::<Option<_>>()?;
    Some(*fields.first()?.max(fields.get(1)?))
}

/// An MP3 frame's header: its length in bytes and its samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    length: usize,
    samples: usize,
    mpeg1: bool,
    mono: bool,
}

/// Reads a frame header, or `None` for bytes that aren't one. Only
/// Layer III, which is what MP3 is.
fn frame(header: [u8; 4]) -> Option<Frame> {
    if header[0] != 0xff || header[1] & 0xe0 != 0xe0 {
        return None;
    }
    let version = (header[1] >> 3) & 0b11;
    let layer = (header[1] >> 1) & 0b11;
    // Reserved, and only Layer III
    if version == 0b01 || layer != 0b01 {
        return None;
    }
    let mpeg1 = version == 0b11;
    const MPEG1: [u32; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const MPEG2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let bitrate = *(if mpeg1 { &MPEG1 } else { &MPEG2 }).get(usize::from(header[2] >> 4))?;
    let rate = match ((header[2] >> 2) & 0b11, version) {
        (0, 0b11) => 44_100,
        (1, 0b11) => 48_000,
        (2, 0b11) => 32_000,
        (0, 0b10) => 22_050,
        (1, 0b10) => 24_000,
        (2, 0b10) => 16_000,
        (0, 0b00) => 11_025,
        (1, 0b00) => 12_000,
        (2, 0b00) => 8_000,
        _ => return None,
    };
    if bitrate == 0 {
        return None;
    }
    let padding = u32::from((header[2] >> 1) & 1);
    let per_kbps = if mpeg1 { 144_000 } else { 72_000 };
    Some(Frame {
        length: (per_kbps * bitrate / rate + padding) as usize,
        samples: if mpeg1 { 1152 } else { 576 },
        mpeg1,
        mono: header[3] >> 6 == 0b11,
    })
}

/// A copied MP3's gapless data, from its LAME header and its frames.
fn mp3_gapless(path: &Path) -> Option<Gapless> {
    let mut file = BufReader::new(File::open(path).ok()?);
    let mut start = [0u8; 10];
    file.read_exact(&mut start).ok()?;
    // Past an ID3v2 tag, whose size is in 7-bit bytes
    let mut offset = 0u64;
    if &start[..3] == b"ID3" {
        let size = start[6..10]
            .iter()
            .fold(0u64, |size, byte| (size << 7) | u64::from(byte & 0x7f));
        offset = 10 + size + if start[5] & 0x10 != 0 { 10 } else { 0 };
    }
    // The first frame, which carries the Xing or Info header
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut header = [0u8; 4];
    let first = loop {
        file.read_exact(&mut header).ok()?;
        if let Some(found) = frame(header) {
            break found;
        }
        offset += 1;
        if offset > 64 * 1024 {
            return None;
        }
        file.seek(SeekFrom::Start(offset)).ok()?;
    };
    let mut body = vec![0u8; first.length.saturating_sub(4)];
    file.read_exact(&mut body).ok()?;
    let side_info = match (first.mpeg1, first.mono) {
        (true, false) => 32,
        (true, true) | (false, false) => 17,
        (false, true) => 9,
    };
    let xing = body.get(side_info..side_info + 8)?;
    let vbr = match &xing[..4] {
        b"Xing" => true,
        b"Info" => false,
        _ => return None,
    };
    let flags = u32::from_be_bytes(xing[4..8].try_into().ok()?);
    let mut lame = side_info + 8;
    for (flag, size) in [(1, 4), (2, 4), (4, 100), (8, 4)] {
        if flags & flag != 0 {
            lame += size;
        }
    }
    let tag = body.get(lame..lame + 0x18)?;
    if &tag[..4] != b"LAME" {
        return None;
    }
    let pregap = (u32::from(tag[0x15]) << 4) | (u32::from(tag[0x16]) >> 4);
    let postgap = (u32::from(tag[0x16] & 0x0f) << 8) | u32::from(tag[0x17]);

    // Every frame after the first, by its header alone
    let mut total = first.length as u64;
    let mut frames = 0u64;
    let mut last: [u64; 8] = [0; 8];
    loop {
        if file.read_exact(&mut header).is_err() {
            break;
        }
        let Some(next) = frame(header) else {
            break;
        };
        last[(frames % 8) as usize] = next.length as u64;
        total += next.length as u64;
        frames += 1;
        if file.seek_relative(next.length as i64 - 4).is_err() {
            break;
        }
    }
    // iTunes counts a frame more for a CBR file
    if !vbr {
        frames += 1;
    }
    let samples = (frames * first.samples as u64).checked_sub(u64::from(pregap + postgap))?;
    let data = total.checked_sub(last.iter().sum())?;
    Some(Gapless {
        pregap,
        postgap,
        sample_count: samples,
        data: u32::try_from(data).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_atoms_are_read() {
        assert_eq!(
            itunsmpb(
                " 00000000 00000840 0000026C 0000000000035D54 00000000 00000000 00000000 00000000"
            ),
            Some(Gapless {
                pregap: 2112,
                postgap: 620,
                sample_count: 220_500,
                data: 0,
            })
        );
        assert_eq!(itunsmpb("nonsense"), None);
        assert_eq!(
            itunnorm(
                " 00000515 00000600 00000515 00000515 000010B0 000010B0 00001F3F 00001F3F 00000000 00000000"
            ),
            Some(0x600)
        );
    }

    #[test]
    fn frame_headers_are_read() {
        // MPEG-1 Layer III, 128 kbps, 44.1 kHz, stereo
        let found = frame([0xff, 0xfb, 0x90, 0x00]).unwrap();
        assert_eq!((found.length, found.samples), (417, 1152));
        // With padding, mono
        let padded = frame([0xff, 0xfb, 0x92, 0xc0]).unwrap();
        assert_eq!((padded.length, padded.mono), (418, true));
        assert_eq!(frame([0x49, 0x44, 0x33, 0x04]), None);
    }

    /// A tone, converted as a sync converts for an iPod, has Sound Check,
    /// and AAC its encoder's delay and padding.
    #[cfg(target_os = "macos")]
    #[test]
    fn what_afconvert_makes_has_its_facts() {
        use crate::external::convert::{Encoded, Encoding, convert};
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("tone.wav");
        let samples: Vec<u8> = (0..44_100 * 2)
            .flat_map(|i| {
                let value =
                    (8000.0 * (i as f64 * 440.0 * std::f64::consts::TAU / 44_100.0).sin()) as i16;
                [value.to_le_bytes(), value.to_le_bytes()].concat()
            })
            .collect();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        for value in [16u32, 0x0002_0001, 44_100, 44_100 * 4, 0x0010_0004] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&samples);
        std::fs::write(&wav, bytes).unwrap();
        let encoding = |format, kbps| Encoding {
            format,
            kbps,
            bits: (format == Encoded::Alac).then_some(16),
            sample_rate: None,
            dither: false,
            sound_check: true,
        };

        let aac = dir.path().join("tone.m4a");
        convert(&wav, &encoding(Encoded::Aac, Some(256)), &aac).unwrap();
        let facts = read(&aac).unwrap();
        assert_eq!(facts.kind, Some(FileKind::Aac));
        assert!(facts.sound_check > 0, "{facts:?}");
        assert_eq!(facts.gapless.pregap, 2112);
        assert_eq!(facts.gapless.sample_count, 88_200);
        assert_eq!(facts.sample_rate, 44_100);
        assert_eq!(facts.length_ms, 2000);

        let alac = dir.path().join("tone-alac.m4a");
        convert(&wav, &encoding(Encoded::Alac, None), &alac).unwrap();
        let facts = read(&alac).unwrap();
        assert_eq!(facts.kind, Some(FileKind::Alac));
        assert!(facts.sound_check > 0 && facts.bitrate > 0, "{facts:?}");
        assert_eq!(
            facts.gapless,
            Gapless {
                sample_count: 88_200,
                ..Gapless::default()
            }
        );
    }
}
