//! Converting audio (backend.md, "Syncing a folder library"): what each
//! source becomes on a destination, and Core Audio's `afconvert`, which does
//! every conversion. Everything liquidslunk encodes goes through Apple's
//! encoders, and the source file is only read.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, FileType};
use lofty::mp4::{Mp4Codec, Mp4File};
use lofty::probe::Probe;

use super::{ExternalKind, OutputFormat, Profile};
use crate::{Error, Result};

const AFCONVERT: &str = "/usr/bin/afconvert";

/// Bumped when liquidslunk changes how it converts, so a sync converts again
/// what the old way made. It's part of each file's audio key.
pub const CONVERSION_VERSION: u32 = 1;

/// What a source file holds, read from the file itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceAudio {
    pub codec: Codec,
    /// Bits per sample, for lossless audio
    pub bits: Option<u8>,
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Flac,
    Alac,
    Wav,
    Aiff,
    Mp3,
    Aac,
    Vorbis,
    Opus,
    Ape,
    WavPack,
    Musepack,
    Dsd,
    Wma,
    /// A file liquidslunk can't read as audio
    Unknown,
}

impl Codec {
    fn is_lossless(self) -> bool {
        matches!(self, Codec::Flac | Codec::Alac | Codec::Wav | Codec::Aiff)
    }

    /// Its name, as a reason for leaving it out says it.
    fn name(self) -> &'static str {
        match self {
            Codec::Flac => "FLAC",
            Codec::Alac => "Apple Lossless",
            Codec::Wav => "WAV",
            Codec::Aiff => "AIFF",
            Codec::Mp3 => "MP3",
            Codec::Aac => "AAC",
            Codec::Vorbis => "Ogg Vorbis",
            Codec::Opus => "Opus",
            Codec::Ape => "APE",
            Codec::WavPack => "WavPack",
            Codec::Musepack => "Musepack",
            Codec::Dsd => "DSD",
            Codec::Wma => "WMA",
            Codec::Unknown => "this format",
        }
    }
}

/// Reads what a source file holds: its codec from its content, falling back
/// on its extension, and a lossless file's bits and sample rate. Only its
/// headers are read.
pub fn read_source(path: &Path) -> SourceAudio {
    let by_extension = |codec| SourceAudio {
        codec,
        bits: None,
        sample_rate: None,
    };
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "dsf" | "dff" => return by_extension(Codec::Dsd),
        "wma" => return by_extension(Codec::Wma),
        _ => {}
    }
    read_with_lofty(path).unwrap_or(by_extension(Codec::Unknown))
}

fn read_with_lofty(path: &Path) -> Option<SourceAudio> {
    let options = ParseOptions::new().read_tags(false).read_cover_art(false);
    let probe = Probe::open(path)
        .ok()?
        .options(options)
        .guess_file_type()
        .ok()?;
    let file_type = probe.file_type()?;
    if file_type == FileType::Mp4 {
        let mut reader = probe.into_inner();
        let file = Mp4File::read_from(&mut reader, options).ok()?;
        let properties = file.properties();
        let codec = match properties.codec() {
            Some(Mp4Codec::AAC) => Codec::Aac,
            Some(Mp4Codec::ALAC) => Codec::Alac,
            Some(Mp4Codec::FLAC) => Codec::Flac,
            Some(Mp4Codec::MP3) => Codec::Mp3,
            _ => Codec::Unknown,
        };
        return Some(SourceAudio {
            codec,
            bits: properties.bit_depth(),
            sample_rate: properties.sample_rate(),
        });
    }
    let codec = match file_type {
        FileType::Flac => Codec::Flac,
        FileType::Wav => Codec::Wav,
        FileType::Aiff => Codec::Aiff,
        FileType::Mpeg => Codec::Mp3,
        FileType::Aac => Codec::Aac,
        FileType::Vorbis => Codec::Vorbis,
        FileType::Opus => Codec::Opus,
        FileType::Ape => Codec::Ape,
        FileType::WavPack => Codec::WavPack,
        FileType::Mpc => Codec::Musepack,
        _ => Codec::Unknown,
    };
    let file = probe.read().ok()?;
    let properties = file.properties();
    Some(SourceAudio {
        codec,
        bits: properties.bit_depth(),
        sample_rate: properties.sample_rate(),
    })
}

/// What a source becomes on a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Becomes {
    /// Copied as it is, then tagged
    Copy,
    Convert(Encoding),
}

/// A conversion with `afconvert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding {
    pub format: Encoded,
    /// AAC's bitrate, in kbps
    pub kbps: Option<i64>,
    /// A lossless file's bits per sample, which `afconvert` has to be told:
    /// otherwise it makes 32-bit Apple Lossless from a 16-bit FLAC
    pub bits: Option<u8>,
    /// The sample rate, when it changes
    pub sample_rate: Option<u32>,
    /// Dithered, since it cuts bits or changes the sample rate of lossless
    /// audio: `afconvert` only rounds unless it's asked to dither
    pub dither: bool,
    /// With Sound Check data, for an iPod (ipod.md, "Gapless and Sound Check")
    pub sound_check: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoded {
    Flac,
    Alac,
    Aac,
    /// A WAV at CD resolution, in a folder library that keeps the originals
    Wav,
    /// An AIFF at CD resolution, likewise
    Aiff,
}

impl Becomes {
    /// What it's written as, for the manifest's audio key: "copy",
    /// "aac 256 kbps", "alac 16-bit 44100 Hz".
    pub fn key(&self) -> String {
        match self {
            Becomes::Copy => "copy".into(),
            Becomes::Convert(encoding) => {
                let mut key = match encoding.format {
                    Encoded::Flac => "flac",
                    Encoded::Alac => "alac",
                    Encoded::Aac => "aac",
                    Encoded::Wav => "wav",
                    Encoded::Aiff => "aiff",
                }
                .to_string();
                if let Some(kbps) = encoding.kbps {
                    key.push_str(&format!(" {kbps} kbps"));
                }
                if let Some(bits) = encoding.bits {
                    key.push_str(&format!(" {bits}-bit"));
                }
                if let Some(rate) = encoding.sample_rate {
                    key.push_str(&format!(" {rate} Hz"));
                }
                key
            }
        }
    }

    /// What the written file will take, estimated: AAC from its duration at
    /// its bitrate; lossless from its source's size, in proportion to the
    /// bits and the sample rate it keeps; a copy, its source's size.
    pub fn estimated_bytes(
        &self,
        source: &SourceAudio,
        size: i64,
        duration_ms: Option<i64>,
    ) -> i64 {
        let Becomes::Convert(encoding) = self else {
            return size;
        };
        match (encoding.kbps, duration_ms) {
            // kbps is bits per millisecond
            (Some(kbps), Some(ms)) => ms * kbps / 8,
            (Some(_), None) => size,
            (None, _) => {
                let ratio = |out: Option<u32>, of: Option<u32>| match (out, of) {
                    (Some(out), Some(of)) if of > 0 => f64::from(out) / f64::from(of),
                    _ => 1.0,
                };
                let bits = ratio(encoding.bits.map(u32::from), source.bits.map(u32::from));
                let rate = ratio(encoding.sample_rate, source.sample_rate);
                (size as f64 * bits.min(1.0) * rate.min(1.0)) as i64
            }
        }
    }

    /// The written file's extension, in lowercase, from its source's.
    pub fn extension(&self, source_extension: &str) -> String {
        let source = source_extension.to_lowercase();
        match self {
            Becomes::Copy => source,
            Becomes::Convert(encoding) => match encoding.format {
                Encoded::Flac => "flac".into(),
                Encoded::Alac | Encoded::Aac => "m4a".into(),
                Encoded::Wav => "wav".into(),
                Encoded::Aiff if source == "aif" => source,
                Encoded::Aiff => "aiff".into(),
            },
        }
    }
}

/// What a source becomes on a destination of this kind, with this profile,
/// or why it's left out.
/// - Lossless sources become the profile's format, and a file already in it
///   is copied. At CD resolution, which an iPod always has, a file above 16
///   bits or 48 kHz is converted even when its format stays the same.
/// - MP3 and AAC are copied. So are Ogg Vorbis and Opus, except to an iPod,
///   which gets AAC.
/// - APE, WavPack and Musepack are only copied, to a folder library that
///   keeps the originals: Core Audio can't decode them. DSD and WMA are left
///   out, since lofty can't tag them.
pub fn becomes(
    source: &SourceAudio,
    kind: ExternalKind,
    profile: &Profile,
) -> std::result::Result<Becomes, String> {
    let ipod = kind == ExternalKind::Ipod;
    let aac = |rate: Option<u32>| Encoding {
        format: Encoded::Aac,
        kbps: Some(profile.aac_kbps),
        bits: None,
        // Apple's AAC encoder takes at most 48 kHz
        sample_rate: rate.and_then(|rate| (rate > 48_000).then(|| cd_rate(rate))),
        dither: false,
        sound_check: ipod,
    };
    match source.codec {
        codec if codec.is_lossless() => {
            let cd = ipod || profile.cd_resolution;
            let bits = source.bits.unwrap_or(16);
            let rate = source.sample_rate.unwrap_or(44_100);
            let above = cd && (bits > 16 || rate > 48_000);
            let lossless = |format| {
                let out_bits = if cd { 16 } else { bits.clamp(16, 24) };
                let out_rate = if cd { cd_rate(rate) } else { rate };
                Encoding {
                    format,
                    kbps: None,
                    bits: Some(out_bits),
                    sample_rate: (out_rate != rate).then_some(out_rate),
                    dither: out_bits < bits || out_rate != rate,
                    sound_check: ipod,
                }
            };
            let own = match codec {
                Codec::Flac => Encoded::Flac,
                Codec::Alac => Encoded::Alac,
                Codec::Wav => Encoded::Wav,
                _ => Encoded::Aiff,
            };
            let target = match profile.format {
                OutputFormat::Original => own,
                OutputFormat::Flac => Encoded::Flac,
                OutputFormat::Alac => Encoded::Alac,
                OutputFormat::Aac => return Ok(Becomes::Convert(aac(Some(rate)))),
            };
            Ok(if target == own && !above {
                Becomes::Copy
            } else {
                Becomes::Convert(lossless(target))
            })
        }
        Codec::Mp3 | Codec::Aac => Ok(Becomes::Copy),
        Codec::Vorbis | Codec::Opus if ipod => Ok(Becomes::Convert(aac(source.sample_rate))),
        Codec::Vorbis | Codec::Opus => Ok(Becomes::Copy),
        Codec::Ape | Codec::WavPack | Codec::Musepack => {
            if !ipod && profile.format == OutputFormat::Original {
                Ok(Becomes::Copy)
            } else {
                Err(format!(
                    "Core Audio can't convert {}, so it's only copied, to a folder library that keeps the originals",
                    source.codec.name()
                ))
            }
        }
        Codec::Dsd | Codec::Wma => Err(format!(
            "liquidslunk can't tag {} files",
            source.codec.name()
        )),
        _ => Err("liquidslunk can't read this file's format".into()),
    }
}

/// CD resolution's sample rate for a source's: 44.1 kHz for 88.2, 176.4 and
/// 352.8 kHz, 48 kHz for anything else above 48 kHz, and the source's own
/// otherwise, since a file is never resampled upwards.
fn cd_rate(rate: u32) -> u32 {
    match rate {
        rate if rate <= 48_000 => rate,
        rate if rate % 44_100 == 0 => 44_100,
        _ => 48_000,
    }
}

/// AAC bitrates to step down to, in kbps, when Apple's encoder refuses the
/// profile's for a source's sample rate and channels: 8 kHz mono takes at
/// most 24 kbps, and 44.1 kHz mono 256
const AAC_KBPS: [i64; 12] = [320, 256, 192, 160, 128, 96, 64, 48, 32, 24, 16, 8];

/// Converts `source` into `output` with `afconvert`, which only reads the
/// source. It's started directly, never through a shell. AAC is at the
/// profile's bitrate, or else the highest of `AAC_KBPS` below it that the
/// encoder takes for this source.
pub fn convert(source: &Path, encoding: &Encoding, output: &Path) -> Result<()> {
    if encoding.format != Encoded::Aac {
        return run(source, encoding, output);
    }
    let asked = encoding.kbps.unwrap_or(super::DEFAULT_AAC_KBPS);
    let lower = AAC_KBPS.iter().copied().filter(|kbps| *kbps < asked);
    let mut refused = None;
    for kbps in std::iter::once(asked).chain(lower) {
        let at = Encoding {
            kbps: Some(kbps),
            ..*encoding
        };
        match run(source, &at, output) {
            Err(Error::Invalid(message)) if message.contains("'!dat'") => refused = Some(message),
            result => return result,
        }
    }
    Err(Error::Invalid(refused.unwrap_or_default()))
}

fn run(source: &Path, encoding: &Encoding, output: &Path) -> Result<()> {
    let at_rate = encoding
        .sample_rate
        .map(|rate| format!("@{rate}"))
        .unwrap_or_default();
    // Apple's format flags for lossless: 1 is 16-bit, 2 20-bit, 3 24-bit
    let flags = match encoding.bits {
        Some(16) | None => 1,
        Some(20) => 2,
        Some(_) => 3,
    };
    let bits = encoding.bits.unwrap_or(16);
    let mut command = Command::new(AFCONVERT);
    match encoding.format {
        Encoded::Flac => command.args(["-f", "flac", "-d", &format!("flac{at_rate}/{flags}")]),
        Encoded::Alac => command.args(["-f", "m4af", "-d", &format!("alac{at_rate}/{flags}")]),
        Encoded::Aac => command.args([
            "-f",
            "m4af",
            "-d",
            &format!("aac{at_rate}"),
            "-b",
            &(encoding.kbps.unwrap_or(super::DEFAULT_AAC_KBPS) * 1000).to_string(),
            // Constrained VBR, at the encoder's best quality
            "-s",
            "2",
            "-q",
            "127",
        ]),
        Encoded::Wav => command.args(["-f", "WAVE", "-d", &format!("LEI{bits}{at_rate}")]),
        Encoded::Aiff => command.args(["-f", "AIFF", "-d", &format!("BEI{bits}{at_rate}")]),
    };
    if encoding.dither {
        // TPDF
        command.args(["--dither", "1"]);
    }
    if encoding.sound_check {
        command.arg("--soundcheck-generate");
    }
    command.arg(source).arg(output);
    let result = command.output().map_err(Error::io(Path::new(AFCONVERT)))?;
    if !result.status.success() {
        let _ = fs::remove_file(output);
        let said = String::from_utf8_lossy(&result.stderr);
        let said = said
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        return Err(Error::Invalid(format!(
            "Core Audio couldn't convert it ({}){}",
            result.status,
            if said.is_empty() {
                String::new()
            } else {
                format!(": {}", said.trim())
            }
        )));
    }
    Ok(())
}

/// Which encoder converts: macOS's version, since `afconvert` and its
/// encoders come with it. The manifest records it, without comparing it, so
/// a macOS update doesn't convert everything again.
pub fn encoder() -> &'static str {
    static ENCODER: OnceLock<String> = OnceLock::new();
    ENCODER.get_or_init(|| {
        let plist = fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist")
            .unwrap_or_default();
        let value = |key: &str| {
            let after = plist.split(&format!("<key>{key}</key>")).nth(1)?;
            let start = after.find("<string>")? + "<string>".len();
            let end = after.find("</string>")?;
            after.get(start..end).map(str::to_string)
        };
        match (value("ProductVersion"), value("ProductBuildVersion")) {
            (Some(version), Some(build)) => format!("Core Audio, macOS {version} ({build})"),
            (Some(version), None) => format!("Core Audio, macOS {version}"),
            _ => "Core Audio".into(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(format: OutputFormat, cd_resolution: bool) -> Profile {
        Profile {
            format,
            aac_kbps: 256,
            cd_resolution,
            path_template: None,
            embed_covers: true,
            cover_px: 600,
            cover_file: false,
        }
    }

    fn source(codec: Codec, bits: u8, rate: u32) -> SourceAudio {
        SourceAudio {
            codec,
            bits: Some(bits),
            sample_rate: Some(rate),
        }
    }

    fn key(source: SourceAudio, kind: ExternalKind, profile: &Profile) -> String {
        becomes(&source, kind, profile).map_or_else(|reason| reason, |becomes| becomes.key())
    }

    #[test]
    fn each_source_becomes_what_the_profile_says() {
        use ExternalKind::{Folder, Ipod};
        use OutputFormat::{Aac, Alac, Flac, Original};
        let cd = source(Codec::Flac, 16, 44_100);
        let hi = source(Codec::Flac, 24, 96_000);
        assert_eq!(key(cd, Folder, &profile(Original, false)), "copy");
        assert_eq!(key(cd, Folder, &profile(Flac, false)), "copy");
        assert_eq!(key(cd, Folder, &profile(Alac, false)), "alac 16-bit");
        assert_eq!(key(cd, Folder, &profile(Aac, false)), "aac 256 kbps");
        // Lossless keeps its resolution, unless it's at CD resolution
        assert_eq!(key(hi, Folder, &profile(Alac, false)), "alac 24-bit");
        assert_eq!(key(hi, Folder, &profile(Flac, false)), "copy");
        assert_eq!(
            key(hi, Folder, &profile(Flac, true)),
            "flac 16-bit 48000 Hz"
        );
        assert_eq!(
            key(hi, Folder, &profile(Original, true)),
            "flac 16-bit 48000 Hz"
        );
        assert_eq!(
            key(source(Codec::Alac, 24, 88_200), Ipod, &profile(Alac, true)),
            "alac 16-bit 44100 Hz"
        );
        assert_eq!(
            key(source(Codec::Alac, 16, 44_100), Ipod, &profile(Alac, true)),
            "copy"
        );
        // AAC takes at most 48 kHz
        assert_eq!(key(hi, Ipod, &profile(Aac, true)), "aac 256 kbps 48000 Hz");
        assert_eq!(
            key(
                source(Codec::Wav, 24, 176_400),
                Folder,
                &profile(Aac, false)
            ),
            "aac 256 kbps 44100 Hz"
        );
        // Lossy sources are copied, except Ogg to an iPod
        let mp3 = SourceAudio {
            codec: Codec::Mp3,
            bits: None,
            sample_rate: Some(44_100),
        };
        assert_eq!(key(mp3, Ipod, &profile(Aac, true)), "copy");
        let vorbis = SourceAudio {
            codec: Codec::Vorbis,
            ..mp3
        };
        assert_eq!(key(vorbis, Folder, &profile(Alac, false)), "copy");
        assert_eq!(key(vorbis, Ipod, &profile(Alac, true)), "aac 256 kbps");
        // What Core Audio can't decode is only kept as it is
        let ape = SourceAudio {
            codec: Codec::Ape,
            ..cd
        };
        assert_eq!(key(ape, Folder, &profile(Original, false)), "copy");
        assert!(becomes(&ape, Folder, &profile(Flac, false)).is_err());
        let dsd = SourceAudio {
            codec: Codec::Dsd,
            ..cd
        };
        assert_eq!(
            becomes(&dsd, Folder, &profile(Original, false)),
            Err("liquidslunk can't tag DSD files".into())
        );
    }

    #[test]
    fn lossless_is_dithered_only_when_it_loses_resolution() {
        let encoding =
            |source, profile: &Profile| match becomes(&source, ExternalKind::Folder, profile) {
                Ok(Becomes::Convert(encoding)) => encoding,
                other => panic!("{other:?}"),
            };
        let flac_to_alac = encoding(
            source(Codec::Flac, 16, 44_100),
            &profile(OutputFormat::Alac, true),
        );
        assert!(!flac_to_alac.dither);
        let cut = encoding(
            source(Codec::Flac, 24, 48_000),
            &profile(OutputFormat::Alac, true),
        );
        assert_eq!(
            (cut.bits, cut.sample_rate, cut.dither),
            (Some(16), None, true)
        );
    }

    #[test]
    fn rates_go_down_to_their_family() {
        assert_eq!(cd_rate(32_000), 32_000);
        assert_eq!(cd_rate(48_000), 48_000);
        assert_eq!(cd_rate(88_200), 44_100);
        assert_eq!(cd_rate(352_800), 44_100);
        assert_eq!(cd_rate(96_000), 48_000);
        assert_eq!(cd_rate(192_000), 48_000);
    }

    #[test]
    fn extensions_follow_the_format() {
        let convert = |format| {
            Becomes::Convert(Encoding {
                format,
                kbps: None,
                bits: None,
                sample_rate: None,
                dither: false,
                sound_check: false,
            })
        };
        assert_eq!(Becomes::Copy.extension("MP3"), "mp3");
        assert_eq!(convert(Encoded::Aac).extension("flac"), "m4a");
        assert_eq!(convert(Encoded::Flac).extension("m4a"), "flac");
        assert_eq!(convert(Encoded::Aiff).extension("aif"), "aif");
    }
}
