//! Resampling, for a file whose rate isn't the output's (backend.md, "Joining
//! tracks"). One converter runs across consecutive tracks at the same rate,
//! so nothing is flushed at a join between them. Its delay is trimmed from
//! the start, so output frame `k` is input time `k / ratio`, and a join's
//! place in the output is arithmetic.

use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

use crate::Error;

/// Input frames the resampler takes at a time.
const CHUNK: usize = 1024;
const CHANNELS: usize = 2;

/// A resampler from one rate to another, for interleaved stereo.
pub struct Converter {
    from: u32,
    to: u32,
    resampler: Fft<f32>,
    /// Input not yet a whole chunk
    pending: Vec<f32>,
    /// The resampler's output, before it's trimmed
    scratch: Vec<f32>,
    /// Output frames still to drop: the resampler's delay
    trim: usize,
    taken: u64,
    given: u64,
}

impl Converter {
    pub fn new(from: u32, to: u32) -> Result<Self, Error> {
        let resampler = Fft::<f32>::new(
            from as usize,
            to as usize,
            CHUNK,
            CHANNELS,
            FixedSync::Input,
        )
        .map_err(|error| Error::Unsupported(format!("can't resample it: {error}")))?;
        let scratch = vec![0.0; resampler.output_frames_max() * CHANNELS];
        let trim = resampler.output_delay();
        Ok(Self {
            from,
            to,
            resampler,
            pending: Vec::with_capacity(CHUNK * CHANNELS * 2),
            scratch,
            trim,
            taken: 0,
            given: 0,
        })
    }

    /// The input rate.
    pub fn from(&self) -> u32 {
        self.from
    }

    /// Input frames taken so far.
    pub fn taken(&self) -> u64 {
        self.taken
    }

    /// The output frame that input frame `input` comes to.
    pub fn output_at(&self, input: u64) -> u64 {
        ((u128::from(input) * u128::from(self.to) + u128::from(self.from) / 2)
            / u128::from(self.from)) as u64
    }

    /// Takes interleaved stereo, and appends what's resampled of it so far.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.taken += (input.len() / CHANNELS) as u64;
        self.pending.extend_from_slice(input);
        while self.pending.len() >= CHUNK * CHANNELS {
            self.process(CHUNK, out);
            self.pending.drain(..CHUNK * CHANNELS);
        }
    }

    /// Resamples what's left, and appends it, up to the output frame the
    /// last input frame comes to.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        let end = self.output_at(self.taken);
        let left = self.pending.len() / CHANNELS;
        if left > 0 {
            self.process(left, out);
            self.pending.clear();
        }
        // The resampler's delay holds the end back: silence pushes it out
        while self.given < end {
            self.process(0, out);
        }
        let over = (self.given - end) as usize;
        out.truncate(out.len() - over * CHANNELS);
        self.given = end;
    }

    /// Resamples one chunk of `frames` pending frames, padded with silence.
    fn process(&mut self, frames: usize, out: &mut Vec<f32>) {
        let mut padded;
        let input: &[f32] = if frames == CHUNK {
            &self.pending[..CHUNK * CHANNELS]
        } else {
            padded = vec![0.0; CHUNK * CHANNELS];
            padded[..frames * CHANNELS].copy_from_slice(&self.pending[..frames * CHANNELS]);
            &padded
        };
        let output_frames = self.resampler.output_frames_next();
        let (Ok(input), Ok(mut output)) = (
            InterleavedSlice::new(input, CHANNELS, CHUNK),
            InterleavedSlice::new_mut(&mut self.scratch, CHANNELS, output_frames),
        ) else {
            return;
        };
        let indexing = Indexing {
            input_offset: 0,
            output_offset: 0,
            partial_len: Some(frames),
            active_channels_mask: None,
        };
        let Ok((_, produced)) =
            self.resampler
                .process_into_buffer(&input, &mut output, Some(&indexing))
        else {
            return;
        };
        let dropped = self.trim.min(produced);
        self.trim -= dropped;
        out.extend_from_slice(&self.scratch[dropped * CHANNELS..produced * CHANNELS]);
        self.given += (produced - dropped) as u64;
    }
}
