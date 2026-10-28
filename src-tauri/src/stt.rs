//! Local speech-to-text with whisper.cpp, plus model downloads.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    pub file: &'static str,
    pub label: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { file: "ggml-large-v3-turbo-q5_0.bin", label: "Large v3 Turbo (recommended)", size_mb: 574 },
    ModelInfo { file: "ggml-small-q5_1.bin", label: "Small (fast, weaker PCs)", size_mb: 190 },
];

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";
const HINT_MAX_CHARS: usize = 200;

/// Streams the model to `<file>.part`, then renames it, so a cut download never looks complete.
pub async fn download_model(file: &str, dir: &Path, on_progress: impl Fn(u64, u64)) -> Result<PathBuf> {
    let info = MODELS.iter().find(|m| m.file == file).ok_or_else(|| anyhow!("unknown model {file}"))?;
    tokio::fs::create_dir_all(dir).await?;
    let needed = info.size_mb as u64 * 1024 * 1024 * 11 / 10;
    if crate::platform::free_space(dir).is_some_and(|free| free < needed) {
        return Err(anyhow!("Not enough disk space: {} MB needed", needed / (1024 * 1024)));
    }
    let resp = reqwest::get(format!("{MODEL_BASE_URL}{file}")).await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    let part = dir.join(format!("{file}.part"));
    let mut out = tokio::fs::File::create(&part).await?;
    let mut done = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.write_all(&chunk).await?;
        done += chunk.len() as u64;
        on_progress(done, total);
    }
    out.flush().await?;
    drop(out);
    let dest = dir.join(file);
    tokio::fs::rename(&part, &dest).await?;
    Ok(dest)
}

#[derive(Debug, Clone, Serialize)]
pub struct Gpu {
    pub id: i32,
    pub name: String,
}

/// GPUs whisper.cpp can use; discrete GPUs come first, so 0 is the right default on laptops.
pub fn list_gpus() -> Vec<Gpu> {
    #[cfg(feature = "vulkan")]
    return whisper_rs::vulkan::list_devices().into_iter().map(|d| Gpu { id: d.id, name: d.name }).collect();
    #[cfg(not(feature = "vulkan"))]
    Vec::new()
}

pub struct Stt {
    ctx: WhisperContext,
}

impl Stt {
    pub fn load(path: &Path, use_gpu: bool, gpu_device: i32) -> Result<Stt> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(use_gpu).gpu_device(gpu_device);
        let ctx = WhisperContext::new_with_params(path, params).context("loading speech model")?;
        Ok(Stt { ctx })
    }

    /// Returns (text, detected language code). `language: None` = auto-detect.
    pub fn transcribe(&self, samples: &[f32], language: Option<&str>, hint: &str) -> Result<(String, String)> {
        let mut state = self.ctx.create_state()?;
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_language(Some(language.unwrap_or("auto")));
        let hint: String = hint.chars().take(HINT_MAX_CHARS).collect();
        if !hint.is_empty() {
            p.set_initial_prompt(&hint);
        }
        p.set_no_context(true);
        p.set_suppress_blank(true);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_special(false);
        p.set_print_timestamps(false);
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).saturating_sub(1).max(1);
        p.set_n_threads(threads as i32);
        state.full(p, samples)?;
        let mut text = String::new();
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                text.push_str(&seg.to_str_lossy()?);
            }
        }
        let lang = whisper_rs::get_lang_str(state.full_lang_id_from_state()).unwrap_or("").to_string();
        Ok((text.trim().to_string(), lang))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs BLABLAFLOW_TEST_MODEL (ggml-tiny.bin) and BLABLAFLOW_TEST_WAV (whisper.cpp's jfk.wav); CI provides both.
    #[test]
    #[ignore]
    fn transcribes_sample() {
        let model = std::env::var("BLABLAFLOW_TEST_MODEL").unwrap();
        let wav = std::env::var("BLABLAFLOW_TEST_WAV").unwrap();
        let samples = blablaflow_core::audio::read_wav(Path::new(&wav)).unwrap();
        let stt = Stt::load(Path::new(&model), false, 0).unwrap();
        let (text, lang) = stt.transcribe(&samples, None, "").unwrap();
        assert!(text.to_lowercase().contains("country"), "{text}");
        assert_eq!(lang, "en");
    }
}
