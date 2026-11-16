//! The output: the system's default device, through cpal (backend.md, "The
//! output"). Its stream follows the default device as it changes, at the
//! rate it opened at: the device's own, which the engine never changes.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{ErrorKind, FromSample, OutputCallbackInfo, SampleFormat, SizedSample, StreamConfig};

use crate::Error;
use crate::render::Renderer;

/// Frames rendered at a time, for an output that asks for more at once.
const BLOCK: usize = 4096;

/// What the default output device runs at: its rate and channels.
pub fn default_config() -> Result<(u32, u16), Error> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| Error::Output("there's no output device".into()))?;
    let config = device
        .default_output_config()
        .map_err(|error| Error::Output(error.to_string()))?;
    Ok((config.sample_rate(), config.channels()))
}

/// A stream on the default output device, playing a renderer.
pub struct Output {
    stream: cpal::Stream,
    errors: Receiver<OutputError>,
}

/// What the output reported going wrong, or changing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputError {
    /// The stream can't play any more: its device is gone, with none to
    /// follow
    pub lost: bool,
    pub message: String,
}

impl Output {
    /// Opens the default output at its own rate and plays the renderer,
    /// which must be for that rate.
    pub fn start(renderer: Renderer) -> Result<Self, Error> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| Error::Output("there's no output device".into()))?;
        let supported = device
            .default_output_config()
            .map_err(|error| Error::Output(error.to_string()))?;
        if supported.sample_rate() != renderer.rate() {
            return Err(Error::Output(format!(
                "the output runs at {} Hz, not {} Hz",
                supported.sample_rate(),
                renderer.rate()
            )));
        }
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let (errors_in, errors) = mpsc::channel();
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, renderer, errors_in),
            SampleFormat::I16 => build::<i16>(&device, config, renderer, errors_in),
            SampleFormat::I32 => build::<i32>(&device, config, renderer, errors_in),
            SampleFormat::U16 => build::<u16>(&device, config, renderer, errors_in),
            format => Err(Error::Output(format!("it takes {format} samples"))),
        }?;
        stream
            .play()
            .map_err(|error| Error::Output(error.to_string()))?;
        Ok(Self { stream, errors })
    }

    /// What the output has reported going wrong, or changing, since asked:
    /// the default device changing, a buffer it couldn't fill in time.
    pub fn errors(&self) -> Vec<OutputError> {
        self.errors.try_iter().collect()
    }

    /// Stops calling the renderer, so the device rests while nothing plays.
    pub fn pause(&self) -> Result<(), Error> {
        self.stream
            .pause()
            .map_err(|error| Error::Output(error.to_string()))
    }

    /// Calls the renderer again, after `pause()`.
    pub fn resume(&self) -> Result<(), Error> {
        self.stream
            .play()
            .map_err(|error| Error::Output(error.to_string()))
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut renderer: Renderer,
    errors: mpsc::Sender<OutputError>,
) -> Result<cpal::Stream, Error>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut scratch = vec![0.0f32; BLOCK * channels];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &OutputCallbackInfo| {
                let timestamp = info.timestamp();
                if let Some(latency) = timestamp
                    .playback
                    .checked_duration_since(timestamp.callback)
                {
                    renderer
                        .shared()
                        .latency
                        .store(latency.as_nanos() as u64, Ordering::Relaxed);
                }
                for block in data.chunks_mut(BLOCK * channels) {
                    let rendered = &mut scratch[..block.len()];
                    renderer.render(rendered, channels);
                    for (sample, &value) in block.iter_mut().zip(rendered.iter()) {
                        *sample = T::from_sample(value);
                    }
                }
            },
            move |error| {
                let lost = matches!(
                    error.kind(),
                    ErrorKind::DeviceNotAvailable | ErrorKind::StreamInvalidated
                );
                let _ = errors.send(OutputError {
                    lost,
                    message: error.to_string(),
                });
            },
            None,
        )
        .map_err(|error| Error::Output(error.to_string()))
}
