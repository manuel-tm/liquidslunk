//! A track's decoder: a file, decoded to interleaved stereo, every sample of
//! its music and nothing else (backend.md, "Decoding").

use std::borrow::Cow;
use std::fs::File;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::AudioFile;
use lofty::mp4::{AtomData, AtomIdent, Mp4File};
use symphonia::core::audio::{Channels, Position};
use symphonia::core::codecs::audio::well_known::CODEC_ID_AAC;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::Error;

/// A file being decoded.
pub struct TrackDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    /// Frames still to drop from the start: an AAC file's encoder delay
    skip: u64,
    /// Frames left to give, when the file says how many it holds: an AAC
    /// file's samples, without its encoder's padding
    left: Option<u64>,
    /// The decoder's output, interleaved as decoded
    scratch: Vec<f32>,
}

impl TrackDecoder {
    /// Opens a file and its audio track.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let file = File::open(path).map_err(|error| Error::Open(error.to_string()))?;
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(unsupported)?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| Error::Unsupported("it has no audio".into()))?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| Error::Unsupported("it has no audio".into()))?;
        let rate = params.sample_rate.unwrap_or(0);
        let is_aac = params.codec == CODEC_ID_AAC;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(unsupported)?;
        // symphonia doesn't trim AAC in MP4: its encoder's delay and padding
        // are in iTunSMPB, as an iPod sync reads them
        let (skip, left) = match is_aac.then(|| itunsmpb(path)).flatten() {
            Some((delay, samples)) => (delay, Some(samples)),
            None => (0, None),
        };
        Ok(Self {
            format,
            decoder,
            track_id,
            rate,
            skip,
            left,
            scratch: Vec::new(),
        })
    }

    /// The file's sample rate, once known: AAC in MP4 may say only as it
    /// decodes.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Decodes the next packet, and appends its frames to `out`, as
    /// interleaved stereo. False at the end of the file.
    pub fn next(&mut self, out: &mut Vec<f32>) -> Result<bool, Error> {
        loop {
            if self.left == Some(0) {
                return Ok(false);
            }
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => return Ok(false),
                Err(SymphoniaError::ResetRequired) => return Ok(false),
                Err(error) => return Err(Error::Decode(error.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                // A packet that can't be decoded is left out, as players do
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(SymphoniaError::Unsupported(what)) => {
                    return Err(Error::Unsupported(what.into()));
                }
                Err(error) => return Err(Error::Decode(error.to_string())),
            };
            let spec = decoded.spec().clone();
            self.rate = spec.rate();
            decoded.copy_to_vec_interleaved(&mut self.scratch);
            let channels = spec.channels().count().max(1);
            let mut frames = self.scratch.len() / channels;
            let mut first = 0;
            if self.skip > 0 {
                let dropped = (self.skip as usize).min(frames);
                self.skip -= dropped as u64;
                first = dropped;
                frames -= dropped;
            }
            if let Some(left) = self.left.as_mut() {
                let kept = (*left as usize).min(frames);
                *left -= kept as u64;
                frames = kept;
            }
            if frames == 0 {
                continue;
            }
            let samples = &self.scratch[first * channels..(first + frames) * channels];
            to_stereo(samples, spec.channels(), out);
            return Ok(true);
        }
    }
}

fn unsupported(error: SymphoniaError) -> Error {
    match error {
        SymphoniaError::IoError(error) => Error::Open(error.to_string()),
        SymphoniaError::Unsupported(what) => Error::Unsupported(what.into()),
        error => Error::Unsupported(error.to_string()),
    }
}

/// An AAC file's encoder delay and samples, from its `iTunSMPB` atom:
/// " 00000000 00000840 0000026C 0000000000035D54 …", in hex, a zero, the
/// delay, the padding and the samples.
fn itunsmpb(path: &Path) -> Option<(u64, u64)> {
    let mut file = File::open(path).ok()?;
    let parsed = Mp4File::read_from(&mut file, ParseOptions::new().read_cover_art(false)).ok()?;
    let ident = AtomIdent::Freeform {
        mean: Cow::Borrowed("com.apple.iTunes"),
        name: Cow::Borrowed("iTunSMPB"),
    };
    let text = parsed
        .ilst()?
        .get(&ident)?
        .data()
        .find_map(|data| match data {
            AtomData::UTF8(text) => Some(text.clone()),
            _ => None,
        })?;
    let fields: Vec<&str> = text.split_whitespace().collect();
    let delay = u64::from_str_radix(fields.get(1)?, 16).ok()?;
    let samples = u64::from_str_radix(fields.get(3)?, 16).ok()?;
    (samples > 0).then_some((delay, samples))
}

/// Appends interleaved samples as stereo: mono on both channels, stereo as it
/// is, and more channels mixed down by their positions, as ITU-R BS.775 does,
/// scaled so they can't clip.
fn to_stereo(samples: &[f32], channels: &Channels, out: &mut Vec<f32>) {
    let count = channels.count().max(1);
    match count {
        1 => {
            for &sample in samples {
                out.extend_from_slice(&[sample, sample]);
            }
        }
        2 => out.extend_from_slice(samples),
        _ => {
            let weights = downmix(channels, count);
            for frame in samples.chunks_exact(count) {
                let (mut left, mut right) = (0.0, 0.0);
                for (sample, (to_left, to_right)) in frame.iter().zip(&weights) {
                    left += sample * to_left;
                    right += sample * to_right;
                }
                out.extend_from_slice(&[left, right]);
            }
        }
    }
}

/// Each channel's weight on the left and on the right.
fn downmix(channels: &Channels, count: usize) -> Vec<(f32, f32)> {
    const HALF_POWER: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let mut weights = match channels {
        Channels::Positioned(positions) => (0..64)
            .map(|bit| Position::from_bits_retain(1 << bit))
            .filter(|position| positions.contains(*position))
            .map(|position| {
                if position == Position::FRONT_LEFT {
                    (1.0, 0.0)
                } else if position == Position::FRONT_RIGHT {
                    (0.0, 1.0)
                } else if position == Position::FRONT_CENTER {
                    (HALF_POWER, HALF_POWER)
                } else if position == Position::LFE1 {
                    (0.0, 0.0)
                } else if position.intersects(
                    Position::REAR_LEFT
                        | Position::SIDE_LEFT
                        | Position::FRONT_LEFT_CENTER
                        | Position::FRONT_LEFT_WIDE,
                ) {
                    (HALF_POWER, 0.0)
                } else if position.intersects(
                    Position::REAR_RIGHT
                        | Position::SIDE_RIGHT
                        | Position::FRONT_RIGHT_CENTER
                        | Position::FRONT_RIGHT_WIDE,
                ) {
                    (0.0, HALF_POWER)
                } else {
                    (HALF_POWER / 2.0, HALF_POWER / 2.0)
                }
            })
            .collect(),
        // Without positions, the first two are left and right
        _ => (0..count)
            .map(|index| match index {
                0 => (1.0, 0.0),
                1 => (0.0, 1.0),
                _ => (0.0, 0.0),
            })
            .collect::<Vec<_>>(),
    };
    weights.resize(count, (0.0, 0.0));
    let (left, right) = weights
        .iter()
        .fold((0.0, 0.0), |(left, right), (l, r)| (left + l, right + r));
    let loudest: f32 = f32::max(left, right);
    if loudest > 1.0 {
        for weight in &mut weights {
            *weight = (weight.0 / loudest, weight.1 / loudest);
        }
    }
    weights
}
