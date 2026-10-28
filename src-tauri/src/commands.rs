//! Commands the frontend calls via `invoke`.
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use blablaflow_core::db::{DictEntry, HistoryEntry, Note, Snippet, Transform};
use blablaflow_core::hotkey::{Action, Hotkey, validate};
use blablaflow_core::insights::{Insights, compute};
use blablaflow_core::settings::Settings;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

use crate::dictation::{BAR_HIDDEN, BarState, Shared, now_ms};
use crate::{audio, llm, platform, stt};

type S<'a> = State<'a, Arc<Shared>>;
type R<T> = Result<T, String>;

fn e(err: impl ToString) -> String {
    err.to_string()
}

#[tauri::command]
pub fn get_settings(s: S) -> Settings {
    s.settings()
}

#[tauri::command]
pub fn save_settings(s: S, settings: Settings) -> R<()> {
    let before = s.settings();
    let reload = (&before.model, before.use_gpu, before.gpu_device) != (&settings.model, settings.use_gpu, settings.gpu_device);
    s.save_settings(settings).map_err(e)?;
    if reload {
        s.preload();
    }
    s.bar(BarState::Idle);
    Ok(())
}

#[tauri::command]
pub fn list_mics() -> Vec<String> {
    audio::list_inputs()
}

#[derive(Serialize)]
pub struct LocalModel {
    file: &'static str,
    label: &'static str,
    size_mb: u32,
    downloaded: bool,
}

#[tauri::command]
pub fn list_models_local(s: S) -> Vec<LocalModel> {
    stt::MODELS
        .iter()
        .map(|m| LocalModel { file: m.file, label: m.label, size_mb: m.size_mb, downloaded: s.models_dir().join(m.file).exists() })
        .collect()
}

#[derive(Clone, Serialize)]
struct Progress {
    file: String,
    done: u64,
    total: u64,
}

#[tauri::command]
pub async fn download_model(s: S<'_>, file: String) -> R<()> {
    let app = s.app.clone();
    let f = file.clone();
    stt::download_model(&file, &s.models_dir(), move |done, total| {
        let _ = app.emit("model-progress", Progress { file: f.clone(), done, total });
    })
    .await
    .map_err(e)?;
    s.preload();
    Ok(())
}

#[tauri::command]
pub fn delete_model(s: S, file: String) -> R<()> {
    if !stt::MODELS.iter().any(|m| m.file == file) {
        return Err("unknown model".into());
    }
    std::fs::remove_file(s.models_dir().join(file)).map_err(e)
}

#[tauri::command]
pub fn list_gpus() -> Vec<stt::Gpu> {
    stt::list_gpus()
}

#[tauri::command]
pub fn groq_key_set() -> bool {
    llm::get_key().is_some()
}

#[tauri::command]
pub fn set_groq_key(key: Option<String>) -> R<()> {
    llm::set_key(key.as_deref())
}

#[tauri::command]
pub async fn list_groq_models() -> R<Vec<String>> {
    let key = llm::get_key().ok_or("No Groq API key")?;
    llm::list_models(&key).await
}

/// Sends a tiny request; picks a default model first if none is chosen.
#[tauri::command]
pub async fn test_groq(s: S<'_>) -> R<String> {
    let key = llm::get_key().ok_or("No Groq API key")?;
    let mut st = s.settings();
    if st.groq_model.is_empty() {
        let models = llm::list_models(&key).await?;
        st.groq_model = llm::pick_default_model(&models).ok_or("Groq returned no models")?;
        s.save_settings(st.clone()).map_err(e)?;
    }
    let msgs = vec![
        blablaflow_core::groq::Message::system("Reply with exactly: OK"),
        blablaflow_core::groq::Message::user("ping"),
    ];
    llm::chat(&key, &st.groq_model, msgs).await?;
    Ok(st.groq_model)
}

#[tauri::command]
pub fn history_list(s: S, query: String, offset: i64) -> R<Vec<HistoryEntry>> {
    s.db.lock().unwrap().list_history(&query, 100, offset).map_err(e)
}

#[tauri::command]
pub fn history_delete(s: S, id: i64) -> R<()> {
    let db = s.db.lock().unwrap();
    if let Some(h) = db.get_history(id).map_err(e)?
        && let Some(p) = h.audio_path
    {
        let _ = std::fs::remove_file(s.data_dir.join(p));
    }
    db.delete_history(id).map_err(e)
}

#[tauri::command]
pub fn history_retry(s: S, id: i64) {
    s.retry(id);
}

#[tauri::command]
pub fn history_copy(s: S, id: i64, raw: bool) -> R<()> {
    let h = s.db.lock().unwrap().get_history(id).map_err(e)?.ok_or("not found")?;
    platform::copy_text(if raw { &h.raw_text } else { &h.final_text }).map_err(e)
}

/// `utc_offset_s` comes from the frontend so "today" matches the user's clock.
#[tauri::command]
pub fn insights(s: S, utc_offset_s: i64) -> R<Insights> {
    compute(&s.db.lock().unwrap(), now_ms(), utc_offset_s).map_err(e)
}

#[tauri::command]
pub fn dict_list(s: S) -> R<Vec<DictEntry>> {
    s.db.lock().unwrap().list_dict().map_err(e)
}

#[tauri::command]
pub fn dict_add(s: S, term: String, replacement: Option<String>) -> R<i64> {
    let replacement = replacement.filter(|r| !r.trim().is_empty());
    s.db.lock().unwrap().add_dict(&term, replacement.as_deref()).map_err(e)
}

#[tauri::command]
pub fn dict_delete(s: S, id: i64) -> R<()> {
    s.db.lock().unwrap().delete_dict(id).map_err(e)
}

#[tauri::command]
pub fn snippets_list(s: S) -> R<Vec<Snippet>> {
    s.db.lock().unwrap().list_snippets().map_err(e)
}

#[tauri::command]
pub fn snippet_add(s: S, cue: String, text: String) -> R<i64> {
    s.db.lock().unwrap().add_snippet(&cue, &text).map_err(e)
}

#[tauri::command]
pub fn snippet_update(s: S, snippet: Snippet) -> R<()> {
    s.db.lock().unwrap().update_snippet(&snippet).map_err(e)
}

#[tauri::command]
pub fn snippet_delete(s: S, id: i64) -> R<()> {
    s.db.lock().unwrap().delete_snippet(id).map_err(e)
}

#[tauri::command]
pub fn transforms_list(s: S) -> R<Vec<Transform>> {
    s.db.lock().unwrap().list_transforms().map_err(e)
}

#[tauri::command]
pub fn transform_save(s: S, transform: Transform) -> R<i64> {
    let id = s.db.lock().unwrap().save_transform(&transform).map_err(e)?;
    s.rebuild_matcher();
    Ok(id)
}

#[tauri::command]
pub fn transform_delete(s: S, id: i64) -> R<()> {
    s.db.lock().unwrap().delete_transform(id).map_err(e)?;
    let mut st = s.settings();
    if st.auto_transform == Some(id) {
        st.auto_transform = None;
        s.save_settings(st).map_err(e)?;
    }
    s.rebuild_matcher();
    Ok(())
}

#[tauri::command]
pub fn notes_list(s: S) -> R<Vec<Note>> {
    s.db.lock().unwrap().list_notes().map_err(e)
}

#[tauri::command]
pub fn note_save(s: S, note: Note) -> R<i64> {
    let note = Note { updated_at: now_ms(), ..note };
    s.db.lock().unwrap().save_note(&note).map_err(e)
}

#[tauri::command]
pub fn note_delete(s: S, id: i64) -> R<()> {
    s.db.lock().unwrap().delete_note(id).map_err(e)
}

#[derive(Serialize, Deserialize)]
struct DictItem {
    term: String,
    replacement: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SnippetItem {
    cue: String,
    text: String,
}

/// `kind` is "dictionary" or "snippets"; output is a JSON array.
#[tauri::command]
pub fn export_items(s: S, kind: String) -> R<String> {
    let db = s.db.lock().unwrap();
    match kind.as_str() {
        "dictionary" => {
            let items: Vec<DictItem> =
                db.list_dict().map_err(e)?.into_iter().map(|d| DictItem { term: d.term, replacement: d.replacement }).collect();
            serde_json::to_string_pretty(&items).map_err(e)
        }
        "snippets" => {
            let items: Vec<SnippetItem> =
                db.list_snippets().map_err(e)?.into_iter().map(|s| SnippetItem { cue: s.cue, text: s.text }).collect();
            serde_json::to_string_pretty(&items).map_err(e)
        }
        _ => Err("unknown kind".into()),
    }
}

/// Imports a JSON array; duplicates and invalid items are skipped. Returns how many were added.
#[tauri::command]
pub fn import_items(s: S, kind: String, json: String) -> R<usize> {
    const MAX_ITEMS: usize = 1000;
    let db = s.db.lock().unwrap();
    let added = match kind.as_str() {
        "dictionary" => {
            let items: Vec<DictItem> = serde_json::from_str(&json).map_err(|_| "Not a valid dictionary file")?;
            items.iter().take(MAX_ITEMS).filter(|i| db.add_dict(&i.term, i.replacement.as_deref()).is_ok()).count()
        }
        "snippets" => {
            let items: Vec<SnippetItem> = serde_json::from_str(&json).map_err(|_| "Not a valid snippets file")?;
            items.iter().take(MAX_ITEMS).filter(|i| db.add_snippet(&i.cue, &i.text).is_ok()).count()
        }
        _ => return Err("unknown kind".into()),
    };
    Ok(added)
}

/// Returns the canonical form ("Ctrl+Win+Space") or a readable error.
#[tauri::command]
pub fn validate_hotkey(hotkey: String, action: Action) -> R<String> {
    let h = Hotkey::parse(&hotkey).map_err(e)?;
    validate(&h, action).map_err(e)?;
    Ok(h.to_string())
}

#[tauri::command]
pub fn set_paused(s: S, paused: bool) {
    s.paused.store(paused, Ordering::SeqCst);
}

#[tauri::command]
pub fn hide_bar_for(s: S, minutes: u64) {
    BAR_HIDDEN.store(true, Ordering::SeqCst);
    if let Some(w) = s.app.get_webview_window("overlay") {
        let _ = w.hide();
    }
    let shared = s.inner().clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(minutes * 60));
        BAR_HIDDEN.store(false, Ordering::SeqCst);
        shared.bar(BarState::Idle);
    });
}

#[tauri::command]
pub fn set_language(s: S, language: Option<String>) -> R<()> {
    let mut st = s.settings();
    st.language = language;
    s.save_settings(st).map_err(e)
}

#[tauri::command]
pub fn show_main(s: S) {
    if let Some(w) = s.app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Set by the bar when the user starts dragging it, so only real drags are saved as its position
/// (programmatic moves at startup also fire `Moved`).
pub static BAR_DRAGGING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
pub fn bar_drag_start() {
    BAR_DRAGGING.store(true, Ordering::SeqCst);
}
