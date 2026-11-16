//! A track's decoder: a file, decoded to interleaved stereo, every sample of
//! its music and nothing else, from its start or from any sample of it
//! (backend.md, "Decoding", and "Seeking").

use std::borrow::Cow;
use std::fs::File;
use std::path::{Path, PathBuf};

use lofty::config::ParseOptions;
use lofty::file::AudioFile;
use lofty::mp4::{AtomData, AtomIdent, Mp4File};
use symphonia::core::audio::{Channels, Position};
use symphonia::core::codecs::audio::well_known::{CODEC_ID_AAC, CODEC_ID_MP3, CODEC_ID_VORBIS};
use symphonia::core::codecs::audio::{AudioCodecId, AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::{Error as SymphoniaError, SeekErrorKind};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{TimeBase, Timestamp};

use crate::Error;

/// A file being decoded.
pub struct TrackDecoder {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    codec: AudioCodecId,
    track_id: u32,
    time_base: Option<TimeBase>,
    rate: u32,
    /// An AAC file's encoder delay, before its music: what its samples are
    /// counted from
    delay: u64,
    /// Its samples, when the file says how many it holds: an AAC file's,
    /// without its encoder's padding
    samples: Option<u64>,
    /// Frames still to drop from the start: an AAC file's encoder delay
    skip: u64,
    /// Frames left to give, when the file says how many it holds
    left: Option<u64>,
    /// After a seek, the frame of the file to give from: what's decoded
    /// before it is dropped
    until: Option<i64>,
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
        let time_base = track.time_base;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| Error::Unsupported("it has no audio".into()))?;
        let rate = params.sample_rate.unwrap_or(0);
        let codec = params.codec;
        let is_aac = codec == CODEC_ID_AAC;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(unsupported)?;
        // symphonia doesn't trim AAC in MP4: its encoder's delay and padding
        // are in iTunSMPB, as an iPod sync reads them
        let (delay, samples) = match is_aac.then(|| itunsmpb(path)).flatten() {
            Some((delay, samples)) => (delay, Some(samples)),
            None => (0, None),
        };
        Ok(Self {
            path: path.to_path_buf(),
            format,
            decoder,
            codec,
            track_id,
            time_base,
            rate,
            delay,
            samples,
            skip: delay,
            left: samples,
            until: None,
            scratch: Vec::new(),
        })
    }

    /// Goes to frame `frame` of the music, at the file's rate: what's
    /// decoded next starts there, sample for sample as decoding from the
    /// start would have it. False when the music ends before it.
    ///
    /// The file's reader finds the packet, which is exact in every format
    /// it knows, and the decoder starts a little before it, for the codecs
    /// whose packets lean on the ones before, and drops what comes before
    /// the frame. A file that can't seek is decoded again from its start.
    pub fn seek(&mut self, frame: u64) -> Result<bool, Error> {
        if self.samples.is_some_and(|samples| frame >= samples) {
            self.left = Some(0);
            return Ok(false);
        }
        let target = frame + self.delay;
        let from = target.saturating_sub(preroll(self.codec));
        let to = SeekTo::Timestamp {
            ts: Timestamp::new(self.ts_of(from as i64)),
            track_id: self.track_id,
        };
        match self.format.seek(SeekMode::Accurate, to) {
            Ok(_) => {
                self.decoder.reset();
                self.skip = 0;
                self.left = self.samples.map(|samples| samples - frame);
                self.until = Some(target as i64);
                Ok(true)
            }
            Err(SymphoniaError::SeekError(SeekErrorKind::OutOfRange)) => {
                self.left = Some(0);
                Ok(false)
            }
            // FLAC's reader looks for a frame at the music's end, and finds
            // the file's
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                self.left = Some(0);
                Ok(false)
            }
            Err(_) => {
                let mut again = Self::open(&self.path)?;
                again.skip += frame;
                again.left = again.left.map(|left| left.saturating_sub(frame));
                *self = again;
                Ok(true)
            }
        }
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
            if let Some(until) = self.until {
                // Where what's decoded starts: the packet's first frame,
                // after what the decoder trims of an encoder's delay
                let at = self.frames_of(packet.pts.get())
                    + self.frames_of(packet.trim_start.get() as i64);
                let dropped = (until - at).clamp(0, frames as i64) as usize;
                if at + frames as i64 >= until {
                    self.until = None;
                }
                first = dropped;
                frames -= dropped;
            }
            if self.skip > 0 {
                let dropped = (self.skip as usize).min(frames);
                self.skip -= dropped as u64;
                first += dropped;
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

impl TrackDecoder {
    /// A timestamp of the track, in frames at its rate: they're the same
    /// unless its time base isn't one over its rate.
    fn frames_of(&self, ts: i64) -> i64 {
        match self.time_base {
            Some(base)
                if self.rate > 0 && (base.numer.get(), base.denom.get()) != (1, self.rate) =>
            {
                (i128::from(ts) * i128::from(base.numer.get()) * i128::from(self.rate)
                    / i128::from(base.denom.get())) as i64
            }
            _ => ts,
        }
    }

    /// Frames at its rate, as a timestamp of the track.
    fn ts_of(&self, frames: i64) -> i64 {
        match self.time_base {
            Some(base)
                if self.rate > 0 && (base.numer.get(), base.denom.get()) != (1, self.rate) =>
            {
                (i128::from(frames) * i128::from(base.denom.get())
                    / (i128::from(base.numer.get()) * i128::from(self.rate))) as i64
            }
            _ => frames,
        }
    }
}

/// How far before a seek's frame the decoder starts, so the frame decodes as
/// it would from the start: a codec whose packets overlap needs the one
/// before. MP3's reader already goes back to the frames whose bytes a frame
/// takes; three more fill its filter bank. Vorbis's longest block is 8,192
/// frames.
fn preroll(codec: AudioCodecId) -> u64 {
    match codec {
        CODEC_ID_AAC => 1024,
        CODEC_ID_MP3 => 3 * 1152,
        CODEC_ID_VORBIS => 8192,
        _ => 0,
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
