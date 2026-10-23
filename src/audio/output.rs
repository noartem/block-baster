//! The audio device: the default output of the default host, at its default
//! configuration, pulling frames from the shared engine.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};

use super::engine::Engine;

/// An open output stream. `cpal::Stream` may not be `Send`; it stays with `Audio` on the main thread.
pub struct Output {
    stream: cpal::Stream,
}

/// Logs a problem with the audio device; the game carries on silently.
pub fn report(msg: &str) {
    #[cfg(target_os = "android")]
    log::warn!("audio: {msg}");
    #[cfg(not(target_os = "android"))]
    eprintln!("audio: {msg}");
}

impl Output {
    /// Opens the default device and starts playing; `make` builds the engine at the device's sample rate.
    pub fn open(
        make: impl FnOnce(f32) -> Arc<Mutex<Engine>>,
    ) -> Result<(Output, Arc<Mutex<Engine>>), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("no output device")?;
        let supported = device
            .default_output_config()
            .map_err(|e| format!("no output config: {e}"))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let engine = make(config.sample_rate as f32);
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, engine.clone()),
            SampleFormat::I16 => build::<i16>(&device, config, engine.clone()),
            SampleFormat::U16 => build::<u16>(&device, config, engine.clone()),
            SampleFormat::I32 => build::<i32>(&device, config, engine.clone()),
            SampleFormat::F64 => build::<f64>(&device, config, engine.clone()),
            SampleFormat::I8 => build::<i8>(&device, config, engine.clone()),
            SampleFormat::U8 => build::<u8>(&device, config, engine.clone()),
            other => return Err(format!("unsupported sample format {other}")),
        }?;
        stream
            .play()
            .map_err(|e| format!("can't start the stream: {e}"))?;
        Ok((Output { stream }, engine))
    }

    pub fn play(&self) {
        if let Err(e) = self.stream.play() {
            report(&format!("can't resume: {e}"));
        }
    }

    pub fn pause(&self) {
        if let Err(e) = self.stream.pause() {
            report(&format!("can't pause: {e}"));
        }
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    engine: Arc<Mutex<Engine>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let errors = |e: cpal::Error| report(&format!("stream error: {e}"));
    let stream = if T::FORMAT == SampleFormat::F32 {
        device.build_output_stream(
            config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                engine
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .render(data, channels)
            },
            errors,
            None,
        )
    } else {
        // Rendered in f32, then converted; the buffer grows only if the device asks for more.
        let mut scratch: Vec<f32> = Vec::with_capacity(8192 * channels);
        device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                scratch.resize(data.len(), 0.0);
                engine
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .render(&mut scratch, channels);
                for (d, &s) in data.iter_mut().zip(&scratch) {
                    *d = T::from_sample(s);
                }
            },
            errors,
            None,
        )
    };
    stream.map_err(|e| format!("can't open the stream: {e}"))
}
