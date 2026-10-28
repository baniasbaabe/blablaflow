//! The running app: hotkey triggers → recording → background job → paste → history.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use blablaflow_core::audio::read_wav;
use blablaflow_core::category::{Category, Context, classify, default_rules};
use blablaflow_core::db::{Db, HistoryEntry, Kind, Status};
use blablaflow_core::groq::Message;
use blablaflow_core::hotkey::{Action, Hotkey, HotkeyMatcher, Mode, Trigger};
use blablaflow_core::pipeline::{Engine, Inputs, Outcome, clean_text, run_command, run_dictation, run_transform};
use blablaflow_core::settings::Settings;
use blablaflow_core::text::word_count;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::audio::Recorder;
use crate::platform::{self, Foreground, HookReply, InsertResult};
use crate::stt::Stt;
use crate::llm;

const DOUBLE_TAP_MS: u64 = 500;
const CONTEXT_CHARS: usize = 1500;
const WARN_AFTER: Duration = Duration::from_secs(19 * 60);
const MAX_RECORDING: Duration = Duration::from_secs(20 * 60);
const AUDIO_RETENTION_MS: i64 = 14 * 86_400_000;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state")]
pub enum BarState {
    Idle,
    Recording { mode: Mode, elapsed_ms: u64 },
    Processing,
    Message { text: String, error: bool },
}

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}

struct Active {
    recorder: Recorder,
    mode: Mode,
    history_id: i64,
    fg: Foreground,
    ctx: Context,
    started: Instant,
    wav: PathBuf,
    warned: bool,
}

enum Job {
    Dictation { mode: Mode, history_id: i64, samples: Vec<f32>, fg: Foreground, ctx: Context, duration_ms: i64 },
    Transform(i64),
    Retry(i64),
}

pub struct Shared {
    pub app: AppHandle,
    pub data_dir: PathBuf,
    pub db: Mutex<Db>,
    pub settings: RwLock<Settings>,
    pub paused: AtomicBool,
    /// True while the microphone is recording; the worker waits before pasting so it never types into
    /// a chord the user is holding for the next dictation.
    recording: AtomicBool,
    matcher: Mutex<HotkeyMatcher>,
    stt: Mutex<Option<(String, Arc<Stt>)>>,
    triggers: Sender<Trigger>,
    jobs: Sender<Job>,
}

impl Shared {
    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn save_settings(&self, s: Settings) -> Result<()> {
        s.save(&self.settings_path())?;
        *self.settings.write().unwrap() = s;
        self.rebuild_matcher();
        Ok(())
    }

    /// Re-reads hotkeys from settings and transforms; invalid strings are skipped.
    pub fn rebuild_matcher(&self) {
        let s = self.settings();
        let h = &s.hotkeys;
        let mut bindings: Vec<(Hotkey, Action)> = Vec::new();
        let mut add = |list: &[String], a: Action| {
            for k in list {
                match Hotkey::parse(k) {
                    Ok(hk) => bindings.push((hk, a)),
                    Err(e) => log::warn!("bad hotkey {k:?}: {e}"),
                }
            }
        };
        add(&h.push_to_talk, Action::PushToTalk);
        add(&h.hands_free, Action::HandsFree);
        add(&h.command, Action::Command);
        add(&h.cancel, Action::Cancel);
        add(&h.paste_last, Action::PasteLast);
        add(&h.copy_last, Action::CopyLast);
        add(&h.scratchpad, Action::Scratchpad);
        if let Ok(ts) = self.db.lock().unwrap().list_transforms() {
            for t in ts {
                if let Some(k) = &t.hotkey {
                    add(std::slice::from_ref(k), Action::Transform(t.id));
                }
            }
        }
        self.matcher.lock().unwrap().set_bindings(bindings);
    }

    pub fn bar(&self, state: BarState) {
        let _ = self.app.emit("bar", &state);
        let Some(w) = self.app.get_webview_window("overlay") else { return };
        let show = !matches!(state, BarState::Idle) || self.settings.read().unwrap().bar_always_visible;
        if show && !BAR_HIDDEN.load(Ordering::SeqCst) {
            platform::show_no_activate(&w);
        } else if !show {
            let _ = w.hide();
        }
    }

    pub fn message(&self, text: impl Into<String>, error: bool) {
        self.bar(BarState::Message { text: text.into(), error });
    }

    pub fn retry(&self, history_id: i64) {
        let _ = self.jobs.send(Job::Retry(history_id));
    }

    /// Loads (or reuses) the speech model selected in settings.
    fn stt(&self) -> Result<Arc<Stt>, String> {
        let s = self.settings();
        let key = format!("{}|{}|{}", s.model, s.use_gpu, s.gpu_device);
        let mut slot = self.stt.lock().unwrap();
        if let Some((k, stt)) = slot.as_ref()
            && *k == key
        {
            return Ok(stt.clone());
        }
        let path = self.models_dir().join(&s.model);
        if !path.exists() {
            return Err("Speech model not downloaded – open blablaflow → Settings".into());
        }
        let stt = Arc::new(Stt::load(&path, s.use_gpu, s.gpu_device).map_err(|e| e.to_string())?);
        *slot = Some((key, stt.clone()));
        Ok(stt)
    }

    /// Loads the model in the background and runs it once, so the first dictation is fast:
    /// the first GPU run prepares its shaders, which can take several seconds.
    pub fn preload(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            let st = me.settings();
            if !me.models_dir().join(&st.model).exists() {
                return;
            }
            let gpu = st.use_gpu && !crate::stt::list_gpus().is_empty();
            if st.use_gpu && !gpu {
                me.message("No GPU found – using the CPU, dictation will be slower", false);
            } else if gpu {
                me.message("Warming up GPU…", false);
            }
            let warm = me.stt().and_then(|stt| stt.transcribe(&[0.0; 16_000], Some("en"), "").map_err(|e| e.to_string()));
            match warm {
                Err(e) => me.message(e, true),
                Ok(_) if gpu => me.bar(BarState::Idle),
                Ok(_) => {}
            }
        });
    }
}

/// "Hide for 1 hour" from the bar's menu.
pub static BAR_HIDDEN: AtomicBool = AtomicBool::new(false);

struct AppEngine {
    stt: Option<Arc<Stt>>,
    key: Option<String>,
    model: String,
}

impl Engine for AppEngine {
    fn transcribe(&self, samples: &[f32], language: Option<&str>, hint: &str) -> Result<(String, String), String> {
        let stt = self.stt.as_ref().ok_or("Speech model not loaded")?;
        stt.transcribe(samples, language, hint).map_err(|e| e.to_string())
    }

    fn chat(&self, messages: Vec<Message>) -> Result<String, String> {
        let key = self.key.as_deref().ok_or("no Groq API key")?;
        tauri::async_runtime::block_on(llm::chat(key, &self.model, messages))
    }
}

pub fn start(app: AppHandle, data_dir: PathBuf) -> Result<Arc<Shared>> {
    std::fs::create_dir_all(data_dir.join("audio"))?;
    let db = Db::open(&data_dir.join("blablaflow.db"))?;
    db.mark_interrupted()?;
    for (id, path) in db.expired_audio(now_ms() - AUDIO_RETENTION_MS)? {
        let _ = std::fs::remove_file(data_dir.join(path));
        db.clear_audio(id)?;
    }
    let settings = Settings::load(&data_dir.join("settings.json"));
    let (triggers, trigger_rx) = mpsc::channel();
    let (jobs, job_rx) = mpsc::channel();
    let shared = Arc::new(Shared {
        app,
        data_dir,
        db: Mutex::new(db),
        settings: RwLock::new(settings),
        paused: AtomicBool::new(false),
        recording: AtomicBool::new(false),
        matcher: Mutex::new(HotkeyMatcher::new(vec![], DOUBLE_TAP_MS)),
        stt: Mutex::new(None),
        triggers,
        jobs,
    });
    shared.rebuild_matcher();

    let s = shared.clone();
    platform::start_hooks(move |key, down| {
        if s.paused.load(Ordering::SeqCst) {
            return HookReply::default();
        }
        let mut m = s.matcher.lock().unwrap();
        if down {
            m.retain_held(|k| k == key || platform::is_key_down(k));
        }
        let r = m.on_event(key, down, now_ms() as u64);
        drop(m);
        for t in &r.events {
            let _ = s.triggers.send(*t);
        }
        HookReply { swallow: r.swallow, fired: !r.events.is_empty() }
    })?;

    let s = shared.clone();
    std::thread::Builder::new().name("control".into()).spawn(move || control_loop(&s, trigger_rx))?;
    let s = shared.clone();
    std::thread::Builder::new().name("worker".into()).spawn(move || worker_loop(&s, job_rx))?;
    shared.preload();
    Ok(shared)
}

fn context_for(s: &Shared, fg: &Foreground, read_nearby: bool) -> Context {
    let st = s.settings();
    let mut rules = st.category_rules.clone();
    rules.extend(default_rules());
    let excluded = st.context_excluded_apps.iter().any(|a| a.eq_ignore_ascii_case(&fg.exe));
    let nearby_text =
        if read_nearby && st.context_awareness && !excluded { platform::read_context(CONTEXT_CHARS) } else { None };
    Context { exe: fg.exe.clone(), title: fg.title.clone(), category: classify(&fg.exe, &fg.title, &rules), nearby_text }
}

fn begin(s: &Arc<Shared>, mode: Mode) -> Result<Active> {
    let fg = platform::foreground();
    let created_at = now_ms();
    let rel = format!("audio/{created_at}.wav");
    let wav = s.data_dir.join(&rel);
    let app = s.app.clone();
    let recorder = Recorder::start(s.settings().mic.as_deref(), &wav, move |level| {
        let _ = app.emit("level", level);
    })?;
    let history_id = s.db.lock().unwrap().insert_history(&HistoryEntry {
        id: 0,
        created_at,
        status: Status::Recording,
        raw_text: String::new(),
        final_text: String::new(),
        app: fg.exe.clone(),
        category: String::new(),
        language: String::new(),
        duration_ms: 0,
        words: 0,
        audio_path: Some(rel),
        kind: if mode == Mode::Command { Kind::Command } else { Kind::Dictation },
    })?;
    s.recording.store(true, Ordering::SeqCst);
    s.bar(BarState::Recording { mode, elapsed_ms: 0 });
    // Audio is already flowing; reading nearby text (up to 200 ms) doesn't delay the start.
    // Command Mode reads the selection after release instead.
    let ctx = context_for(s, &fg, mode != Mode::Command);
    Ok(Active { recorder, mode, history_id, fg, ctx, started: Instant::now(), wav, warned: false })
}

fn discard(s: &Shared, a: Active) {
    s.recording.store(false, Ordering::SeqCst);
    let _ = a.recorder.stop();
    let _ = std::fs::remove_file(&a.wav);
    let _ = s.db.lock().unwrap().delete_history(a.history_id);
    s.bar(BarState::Idle);
}

fn finish(s: &Shared, a: Active) {
    s.recording.store(false, Ordering::SeqCst);
    let duration_ms = a.started.elapsed().as_millis() as i64;
    match a.recorder.stop() {
        Ok(samples) => {
            {
                let db = s.db.lock().unwrap();
                if let Ok(Some(mut e)) = db.get_history(a.history_id) {
                    e.status = Status::Processing;
                    let _ = db.update_history(&e);
                }
            }
            s.bar(BarState::Processing);
            let _ = s.jobs.send(Job::Dictation {
                mode: a.mode,
                history_id: a.history_id,
                samples,
                fg: a.fg,
                ctx: a.ctx,
                duration_ms,
            });
        }
        Err(e) => s.message(format!("Recording failed: {e}"), true),
    }
}

fn control_loop(s: &Arc<Shared>, rx: Receiver<Trigger>) {
    let mut active: Option<Active> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Trigger::Start(mode)) => {
                if let Some(a) = active.take() {
                    discard(s, a);
                }
                match begin(s, mode) {
                    Ok(a) => active = Some(a),
                    Err(e) => s.message(e.to_string(), true),
                }
            }
            Ok(Trigger::Stop) => {
                if let Some(a) = active.take() {
                    finish(s, a);
                }
            }
            Ok(Trigger::Cancel) => {
                if let Some(a) = active.take() {
                    discard(s, a);
                }
            }
            Ok(Trigger::Fire(action)) => fire(s, action),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if let Some(a) = active.as_mut() {
            let elapsed = a.started.elapsed();
            if elapsed >= MAX_RECORDING {
                let a = active.take().unwrap();
                finish(s, a);
            } else if elapsed >= WARN_AFTER && !a.warned {
                a.warned = true;
                s.message("1 minute left – recording stops at 20 minutes", false);
            } else {
                s.bar(BarState::Recording { mode: a.mode, elapsed_ms: elapsed.as_millis() as u64 });
            }
        }
    }
}

fn fire(s: &Arc<Shared>, action: Action) {
    match action {
        Action::PasteLast | Action::CopyLast => {
            let last = s.db.lock().unwrap().last_done().ok().flatten();
            let Some(e) = last else { return s.message("Nothing dictated yet", false) };
            if action == Action::CopyLast {
                let _ = platform::copy_text(&e.final_text);
                s.message("Copied", false);
            } else {
                let fg = platform::foreground();
                if let Ok(InsertResult::LeftOnClipboard) = platform::insert_text(&e.final_text, &fg.exe) {
                    s.message("Copied – press Ctrl+V", false);
                }
            }
        }
        Action::Scratchpad => {
            if let Some(w) = s.app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
                let _ = s.app.emit("navigate", "scratchpad");
            }
        }
        Action::Transform(id) => {
            let _ = s.jobs.send(Job::Transform(id));
        }
        _ => {}
    }
}

fn engine(s: &Shared, need_stt: bool) -> Result<AppEngine, String> {
    let st = s.settings();
    Ok(AppEngine { stt: if need_stt { Some(s.stt()?) } else { None }, key: llm::get_key(), model: st.groq_model })
}

/// Blocks while a new recording is running (the user may be holding its hotkey).
fn wait_not_recording(s: &Shared) {
    while s.recording.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn paste(s: &Shared, text: &str, exe: &str, notice: Option<String>) {
    wait_not_recording(s);
    match platform::insert_text(text, exe) {
        Ok(InsertResult::Pasted) => match notice {
            Some(n) => s.message(n, false),
            None => s.bar(BarState::Idle),
        },
        Ok(InsertResult::LeftOnClipboard) => s.message("Copied – press Ctrl+V", false),
        Err(e) => s.message(format!("Paste failed: {e}"), true),
    }
}

fn worker_loop(s: &Arc<Shared>, rx: Receiver<Job>) {
    while let Ok(job) = rx.recv() {
        let result = match job {
            Job::Dictation { mode, history_id, samples, fg, ctx, duration_ms } => {
                process_dictation(s, mode, history_id, &samples, &fg, &ctx, duration_ms)
            }
            Job::Transform(id) => process_transform(s, id),
            Job::Retry(id) => process_retry(s, id),
        };
        if let Err(e) = result {
            s.message(e, true);
        }
    }
}

fn process_dictation(
    s: &Shared,
    mode: Mode,
    history_id: i64,
    samples: &[f32],
    fg: &Foreground,
    ctx: &Context,
    duration_ms: i64,
) -> Result<(), String> {
    let fail = |e: String| {
        let db = s.db.lock().unwrap();
        if let Ok(Some(mut h)) = db.get_history(history_id) {
            h.status = Status::Failed;
            let _ = db.update_history(&h);
        }
        e
    };
    let eng = engine(s, true).map_err(fail)?;
    let st = s.settings();
    let (dict, snippets, auto) = {
        let db = s.db.lock().unwrap();
        let auto = st.auto_transform.and_then(|id| db.list_transforms().ok()?.into_iter().find(|t| t.id == id));
        (db.list_dict().unwrap_or_default(), db.list_snippets().unwrap_or_default(), auto)
    };
    let inp = Inputs {
        settings: &st,
        dict: &dict,
        snippets: &snippets,
        auto_transform: auto.as_ref(),
        ctx,
        has_key: eng.key.is_some(),
    };
    let outcome = if mode == Mode::Command {
        wait_not_recording(s);
        let selection = platform::read_selection(&fg.exe);
        run_command(samples, selection.as_deref(), &inp, &eng)
    } else {
        run_dictation(samples, &inp, &eng)
    }
    .map_err(fail)?;
    let Outcome::Done { raw, text, language, notice } = outcome else {
        // Nothing was said: leave no trace.
        {
            let db = s.db.lock().unwrap();
            if let Ok(Some(h)) = db.get_history(history_id) {
                if let Some(p) = &h.audio_path {
                    let _ = std::fs::remove_file(s.data_dir.join(p));
                }
                let _ = db.delete_history(history_id);
            }
        }
        s.message("No speech detected", false);
        return Ok(());
    };
    paste(s, &text, &fg.exe, notice);
    let db = s.db.lock().unwrap();
    if let Ok(Some(mut h)) = db.get_history(history_id) {
        h.status = Status::Done;
        h.words = word_count(&text);
        h.raw_text = raw;
        h.final_text = text;
        h.language = language;
        h.category = format!("{:?}", ctx.category);
        h.duration_ms = duration_ms;
        let _ = db.update_history(&h);
    }
    let _ = s.app.emit("history-changed", ());
    Ok(())
}

fn process_transform(s: &Shared, id: i64) -> Result<(), String> {
    let t = s.db.lock().unwrap().list_transforms().map_err(|e| e.to_string())?.into_iter().find(|t| t.id == id);
    let t = t.ok_or("Transform not found")?;
    let fg = platform::foreground();
    let Some(selection) = platform::read_selection(&fg.exe) else { return Err("Select some text first".into()) };
    s.bar(BarState::Processing);
    let eng = engine(s, false)?;
    if eng.key.is_none() {
        return Err("Transforms need a Groq API key".into());
    }
    let out = run_transform(&selection, &t, &eng)?;
    paste(s, &out, &fg.exe, None);
    let _ = s.db.lock().unwrap().insert_history(&HistoryEntry {
        id: 0,
        created_at: now_ms(),
        status: Status::Done,
        raw_text: selection,
        words: word_count(&out),
        final_text: out,
        app: fg.exe,
        category: t.name,
        language: String::new(),
        duration_ms: 0,
        audio_path: None,
        kind: Kind::Transform,
    });
    let _ = s.app.emit("history-changed", ());
    Ok(())
}

/// Re-runs a history entry (re-transcribing if its audio is kept) and copies the result.
fn process_retry(s: &Shared, id: i64) -> Result<(), String> {
    let mut h = s.db.lock().unwrap().get_history(id).map_err(|e| e.to_string())?.ok_or("Entry not found")?;
    s.bar(BarState::Processing);
    let audio = h.audio_path.as_ref().map(|p| s.data_dir.join(p)).filter(|p| p.exists());
    let eng = engine(s, audio.is_some())?;
    let st = s.settings();
    let (dict, snippets) = {
        let db = s.db.lock().unwrap();
        (db.list_dict().unwrap_or_default(), db.list_snippets().unwrap_or_default())
    };
    let category = match h.category.as_str() {
        "Personal" => Category::Personal,
        "Work" => Category::Work,
        "Email" => Category::Email,
        _ => Category::Other,
    };
    let ctx = Context { exe: h.app.clone(), title: String::new(), category, nearby_text: None };
    let inp = Inputs { settings: &st, dict: &dict, snippets: &snippets, auto_transform: None, ctx: &ctx, has_key: eng.key.is_some() };
    let (raw, text) = match audio {
        Some(path) => {
            let samples = read_wav(&path).map_err(|e| e.to_string())?;
            match run_dictation(&samples, &inp, &eng)? {
                Outcome::Done { raw, text, language, .. } => {
                    h.language = language;
                    (raw, text)
                }
                Outcome::Empty => return Err("No speech found in the recording".into()),
            }
        }
        None if !h.raw_text.is_empty() => (h.raw_text.clone(), clean_text(&h.raw_text, &inp, &eng).0),
        None => return Err("Nothing to retry: the audio has expired".into()),
    };
    h.status = Status::Done;
    h.words = word_count(&text);
    h.raw_text = raw;
    h.final_text = text.clone();
    s.db.lock().unwrap().update_history(&h).map_err(|e| e.to_string())?;
    platform::copy_text(&text).map_err(|e| e.to_string())?;
    s.message("Retried – result copied", false);
    let _ = s.app.emit("history-changed", ());
    Ok(())
}
