//! Microphone capture: any device format → 16 kHz mono, streamed to a WAV file.
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use blablaflow_core::audio::{SAMPLE_RATE, WavWriter, downmix, resample, rms};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

const LEVEL_EVERY: usize = SAMPLE_RATE as usize / 20; // 50 ms
const FLUSH_EVERY: Duration = Duration::from_secs(1);

pub fn list_inputs() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|ds| ds.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

/// The named device, or the system default if it is gone (unplugged mic).
fn pick_device(name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        if let Some(d) = host.input_devices()?.find(|d| d.to_string() == name) {
            return Ok(d);
        }
        log::warn!("mic {name:?} not found, using default");
    }
    host.default_input_device().ok_or_else(|| anyhow!("No microphone found"))
}

struct Shared {
    samples: Vec<f32>,
    writer: Option<WavWriter>,
    last_flush: Instant,
    level_buf: Vec<f32>,
}

pub struct Recorder {
    stream: cpal::Stream,
    shared: Arc<Mutex<Shared>>,
}

impl Recorder {
    pub fn start(device: Option<&str>, wav_path: &Path, on_level: impl Fn(f32) + Send + 'static) -> Result<Recorder> {
        let device = pick_device(device)?;
        let cfg = device.default_input_config().context("mic config")?;
        let (rate, channels) = (cfg.sample_rate(), cfg.channels() as usize);
        let shared = Arc::new(Mutex::new(Shared {
            samples: Vec::with_capacity(SAMPLE_RATE as usize * 60),
            writer: Some(WavWriter::create(wav_path)?),
            last_flush: Instant::now(),
            level_buf: Vec::new(),
        }));

        let sh = shared.clone();
        let push = move |mono: Vec<f32>| {
            let chunk = resample(&mono, rate);
            let mut s = sh.lock().unwrap();
            if let Some(w) = s.writer.as_mut() {
                let _ = w.write(&chunk);
                if s.last_flush.elapsed() >= FLUSH_EVERY {
                    let _ = s.writer.as_mut().unwrap().flush();
                    s.last_flush = Instant::now();
                }
            }
            s.samples.extend_from_slice(&chunk);
            s.level_buf.extend_from_slice(&chunk);
            if s.level_buf.len() >= LEVEL_EVERY {
                on_level(rms(&s.level_buf));
                s.level_buf.clear();
            }
        };
        // A device error (e.g. unplugged mid-recording) just ends the data; stop() keeps what we have.
        let on_err = |e: cpal::Error| log::error!("mic stream error: {e}");
        let config = cfg.config();
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                config, move |d, _| push(downmix(d, channels)), on_err, None)?,
            cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
                config,
                move |d, _| {
                    let f: Vec<f32> = d.iter().map(|v| *v as f32 / i16::MAX as f32).collect();
                    push(downmix(&f, channels))
                },
                on_err,
                None,
            )?,
            f => return Err(anyhow!("unsupported mic sample format {f:?}")),
        };
        stream.play()?;
        Ok(Recorder { stream, shared })
    }

    /// Stops capture and returns all 16 kHz samples; the WAV file is finalized.
    pub fn stop(self) -> Result<Vec<f32>> {
        drop(self.stream);
        let mut s = self.shared.lock().unwrap();
        if let Some(w) = s.writer.take() {
            w.finish()?;
        }
        Ok(std::mem::take(&mut s.samples))
    }
}
