//! Pure audio helpers: everything here works on 16 kHz mono f32 unless stated.
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

pub const SAMPLE_RATE: u32 = 16_000;
const FRAME: usize = 480; // 30 ms
const PAD: usize = 3_200; // 200 ms

pub fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved.chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32).collect()
}

/// Linear resampling to 16 kHz.
/// shortcut: linear interpolation, no low-pass; switch to rubato if accuracy suffers on 44.1/48 kHz mics.
pub fn resample(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == SAMPLE_RATE || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / SAMPLE_RATE as f64;
    let n = (samples.len() as f64 / ratio).round() as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let j = pos as usize;
            let frac = (pos - j as f64) as f32;
            let a = samples[j.min(samples.len() - 1)];
            let b = samples[(j + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Drop leading/trailing silence (energy gate on 30 ms frames, 200 ms padding).
/// Returns an empty slice if nothing sounds like speech.
/// shortcut: energy gate, not a VAD model; swap in Silero VAD if it cuts words or passes noise.
pub fn trim_silence(samples: &[f32]) -> &[f32] {
    let levels: Vec<f32> = samples.chunks(FRAME).map(rms).collect();
    if levels.is_empty() {
        return &[];
    }
    let mut sorted = levels.clone();
    sorted.sort_by(f32::total_cmp);
    let noise_floor = sorted[sorted.len() / 10];
    let loudest = sorted[sorted.len() - 1];
    // Capped at half the loudest frame so a recording with no silence at all is kept.
    let threshold = (3.0 * noise_floor).min(0.5 * loudest).max(0.01);
    let Some(first) = levels.iter().position(|l| *l > threshold) else { return &[] };
    let last = levels.iter().rposition(|l| *l > threshold).unwrap();
    let start = (first * FRAME).saturating_sub(PAD);
    let end = ((last + 1) * FRAME + PAD).min(samples.len());
    &samples[start..end]
}

/// 16-bit mono WAV writer that can be flushed so a crash keeps the audio so far.
pub struct WavWriter(hound::WavWriter<BufWriter<File>>);

impl WavWriter {
    pub fn create(path: &Path) -> hound::Result<Self> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        Ok(Self(hound::WavWriter::create(path, spec)?))
    }

    pub fn write(&mut self, samples: &[f32]) -> hound::Result<()> {
        for s in samples {
            self.0.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        Ok(())
    }

    /// Writes buffered samples and updates the header.
    pub fn flush(&mut self) -> hound::Result<()> {
        self.0.flush()
    }

    pub fn finish(self) -> hound::Result<()> {
        self.0.finalize()
    }
}

pub fn read_wav(path: &Path) -> hound::Result<Vec<f32>> {
    let mut r = hound::WavReader::open(path)?;
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => r.samples::<i16>().map(|s| s.map(|v| v as f32 / i16::MAX as f32)).collect::<Result<_, _>>()?,
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
    };
    Ok(resample(&downmix(&samples, spec.channels as usize), spec.sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize, amp: f32) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.1).sin()).collect()
    }

    #[test]
    fn trim_all_silence_returns_empty() {
        assert!(trim_silence(&vec![0.0; 16_000]).is_empty());
        assert!(trim_silence(&vec![0.002; 16_000]).is_empty());
        assert!(trim_silence(&[]).is_empty());
    }

    #[test]
    fn trim_keeps_speech_with_padding() {
        let mut s = vec![0.0f32; 16_000];
        s.extend(sine(8_000, 0.3));
        s.extend(vec![0.0f32; 16_000]);
        let t = trim_silence(&s);
        // Speech is 8000 samples; 200 ms padding = 3200 samples each side (frame-aligned, ±1 frame).
        assert!(t.len() >= 8_000 + 2 * 3_200 - 480 && t.len() <= 8_000 + 2 * 3_200 + 960, "{}", t.len());
    }

    #[test]
    fn trim_keeps_continuous_speech() {
        let s = sine(32_000, 0.05);
        assert_eq!(trim_silence(&s).len(), s.len());
    }

    #[test]
    fn downmix_stereo() {
        assert_eq!(downmix(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
        assert_eq!(downmix(&[0.1, 0.2], 1), vec![0.1, 0.2]);
    }

    #[test]
    fn resample_48k_to_16k_length() {
        let out = resample(&vec![0.5; 48_000], 48_000);
        assert_eq!(out.len(), 16_000);
        assert!((out[100] - 0.5).abs() < 1e-6);
        assert_eq!(resample(&[0.1, 0.2], 16_000), vec![0.1, 0.2]);
    }

    #[test]
    fn rms_level() {
        assert_eq!(rms(&[]), 0.0);
        assert!((rms(&[0.5, -0.5]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn wav_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        let mut w = WavWriter::create(&p).unwrap();
        w.write(&[0.0, 0.5, -0.5]).unwrap();
        w.flush().unwrap();
        w.write(&[0.25]).unwrap();
        w.finish().unwrap();
        let r = read_wav(&p).unwrap();
        assert_eq!(r.len(), 4);
        assert!((r[1] - 0.5).abs() < 1e-3 && (r[3] - 0.25).abs() < 1e-3);
    }

    #[test]
    fn wav_survives_without_finish() {
        // A crash after flush must still leave a readable file.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.wav");
        let mut w = WavWriter::create(&p).unwrap();
        w.write(&[0.5; 100]).unwrap();
        w.flush().unwrap();
        std::mem::forget(w);
        assert_eq!(read_wav(&p).unwrap().len(), 100);
    }
}
