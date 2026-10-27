# blablaflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Windows desktop dictation app (Tauri 2) that records on a global hotkey, transcribes locally with whisper.cpp, cleans up via Groq, and pastes at the cursor, plus the Wispr-style settings, history and tools from the spec.

**Architecture:** A cargo workspace has two crates:
- `crates/core` holds all pure logic: settings, SQLite storage, text post-processing, the hotkey state machine, prompt building, category detection, and Groq request/response shapes. It has no OS calls and is tested on Linux.
- `src-tauri` is the app. It holds audio, Whisper, HTTP, the Windows `platform` module and the dictation orchestrator, and is built and tested on Windows in GitHub Actions.

The React frontend has two entry points: `index.html` (main window) and `overlay.html` (Flow bar).

**Tech Stack:**
- Rust: Tauri 2.12 (`tray-icon`), whisper-rs 0.16 (`vulkan` feature on CI), cpal 0.18, hound, rusqlite 0.41 (bundled), reqwest 0.13 (rustls, json, stream), keyring 4, arboard 3, windows 0.62, uiautomation 0.25, serde, thiserror
- Frontend: React 19 + TypeScript + Vite + Tailwind 4

**Spec:** `docs/superpowers/specs/2026-10-27-blablaflow-design.md`

## Global Constraints

- **Platform:** Windows 10/11 x64 only for v1. All OS calls live in `src-tauri/src/platform/` (`mod.rs` defines the functions, `windows.rs` implements them).
- **Local disk (C: has ~4 GB free):** never build the Tauri app or install the Vulkan SDK locally. The app crate is built only in GitHub Actions (`windows-latest`). Only `crates/core` and the frontend are built and tested in WSL.
- **Audio never leaves the machine.** Only text goes to Groq. No telemetry.
- **Groq API key** lives in Windows Credential Manager via `keyring` (service `blablaflow`, user `groq`), never in `settings.json`.
- **Groq endpoint:** `https://api.groq.com/openai/v1/chat/completions` and `/models`. Pick the default model at build time from Groq's live list; the setting stores a free string.
- **Default model file:** `ggml-large-v3-turbo-q5_0.bin` from `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/`. Small option: `ggml-small-q5_1.bin`.
- **Hotkey defaults:**
  - push-to-talk `Ctrl+Win`
  - hands-free `Ctrl+Win+Space`
  - command `Ctrl+Win+Alt`
  - cancel `Escape`
  - paste last `Shift+Alt+Z`
  - copy last `Shift+Alt+X`
  - scratchpad `Win+Alt+S`
  - Polish transform `Win+Alt+1`
- **Hotkey timing:** double-tap window 500 ms.
- **Max recording:** 20 min (warning at 19 min).
- **Text limits:** dictionary term ≤ 60 chars; snippet cue ≤ 60 chars, expansion ≤ 4000; ≤ 8 custom transforms.
- **Audio retention:** files deleted after 14 days. Text kept until the user deletes it.
- **Licence and identity:** MIT. App identifier `com.blablaflow.app`, product name `blablaflow`.

## Review Focus

1. **Silent or near-empty recording:** a short tap, or holding the key without speaking. Expected: nothing is pasted, no Groq call is made, no history row, and Whisper's silence hallucinations ("Thank you.", "you") never appear. Tests: `core::text::tests::is_hallucination_*` (Task 4) and `audio::tests::trim_all_silence_returns_empty` (Task 7).
2. **The LLM answers or wraps the dictation instead of cleaning it.** Dictating "what's the capital of France" must paste the question, not "Paris"; output must not start with "Here is…". Expected: the system prompt forbids it and `sanitize_llm_output` strips wrappers. Output longer than `max(3 × raw_len, raw_len + 200)` chars falls back to the raw text. Tests: Task 6.
3. **Paste target is an elevated (admin) window, or focus changed during processing.** SendInput is silently blocked. Expected: the text stays on the clipboard (no restore) and the bar shows "Copied – press Ctrl+V". The rule is pinned by a test: `paste_restore_policy` (Task 9).
4. **A new hotkey press while the previous dictation is still processing.** Expected: the new recording starts, and the old one finishes and pastes in order, with no interleaving and no deadlock. Test: `dictation::tests::jobs_paste_in_order` (Task 10).
5. **Umlauts and case** in snippet cues and dictionary corrections ("Grüße" vs "grüße", "STRASSE"). Expected: case-insensitive matching that is Unicode-aware with whole-word boundaries. Tests: Task 4.

---

## File Structure

```
Cargo.toml                      workspace: members crates/core, src-tauri
crates/core/src/lib.rs          re-exports modules
crates/core/src/settings.rs     Settings struct, defaults, load/save JSON
crates/core/src/db.rs           SQLite schema + CRUD: history, dictionary, snippets, transforms, notes
crates/core/src/insights.rs     stats computed from history
crates/core/src/text.rs         snippets, dictionary corrections, word count, hallucination filter, output sanitizing
crates/core/src/hotkey.rs       Hotkey parse/validate + HotkeyMatcher state machine
crates/core/src/category.rs     app/title → Category
crates/core/src/prompt.rs       chat messages for cleanup, command mode, transforms
crates/core/src/groq.rs         request/response serde types (no HTTP)
src-tauri/src/main.rs           Tauri setup: windows, tray, state, plugins
src-tauri/src/commands.rs       #[tauri::command]s used by the frontend
src-tauri/src/audio.rs          cpal capture → 16 kHz mono, WAV writer, level, silence trim
src-tauri/src/stt.rs            whisper-rs wrapper + model download
src-tauri/src/llm.rs            reqwest calls to Groq using core::groq types; keyring access
src-tauri/src/dictation.rs      orchestrator: actions → record → transcribe → cleanup → insert → history
src-tauri/src/platform/mod.rs   pub use windows::*; function signatures documented here
src-tauri/src/platform/windows.rs hooks, paste, clipboard, foreground app, UIA context/selection, no-activate
src/main.tsx, src/App.tsx       main window shell with sidebar
src/pages/*.tsx                 Home, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad, Settings, Onboarding
src/overlay.tsx                 Flow bar
src/api.ts                      typed invoke()/listen() wrappers
.github/workflows/ci.yml        linux: core tests + frontend build; windows: app test + tauri build artifact
```

---

### Task 1: Scaffold, CI, and private GitHub repo

**Files:**
- Create: the workspace `Cargo.toml`
- Create: `crates/core/{Cargo.toml,src/lib.rs}`
- Create: `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/*,src/main.rs}`
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `overlay.html`, `src/main.tsx`, `src/overlay.tsx`, `src/index.css`
- Create: `.github/workflows/ci.yml`, `.gitignore`, `LICENSE`

**Interfaces:**
- Produces: two Tauri windows.
  - `main`: 1000×700, hidden at start; shown from the tray or on first run.
  - `overlay`: 220×48, transparent, no decorations, always on top, `skipTaskbar`, `focus: false`, `shadow: false`.
- Produces: a tray menu with Open / Pause dictation / Quit.
- Produces: the CI jobs `core` (ubuntu: `cargo test -p blablaflow-core`, `npm ci && npm run build`) and `app` (windows-latest: install the Vulkan SDK with `humbletim/install-vulkan-sdk@v1.2`, run `cargo test -p blablaflow --features vulkan`, then `npx tauri build`, then upload the NSIS installer as an artifact).
- `src-tauri` Cargo feature `vulkan = ["whisper-rs/vulkan"]`.

- [ ] Step 1: Run `git init -b main`. Scaffold the files above. `npm run build` must run `tsc && vite build`, with both HTML files as Vite inputs.
- [ ] Step 2: Run `cargo test -p blablaflow-core` and `npm ci && npm run build` in WSL. Expected: both pass (core has 0 tests).
- [ ] Step 3: Commit. Run `gh repo create baniasbaabe/blablaflow --private --source . --push`.
- [ ] Step 4: Watch CI with `gh run watch`. Expected: both jobs green, and the `app` job uploads `blablaflow_*_x64-setup.exe`.

### Task 2: Settings

**Files:** Create `crates/core/src/settings.rs`.

**Interfaces:**
- Produces:
  - `Settings` (serde, `#[serde(default)]` on every struct so old files keep loading), with these fields:
    - `hotkeys: Hotkeys` — fields `push_to_talk`, `hands_free`, `command`, `cancel`, `paste_last`, `copy_last`, `scratchpad`, each a `Vec<String>` of up to 4 bindings
    - `mic: Option<String>` (device name; `None` = system default)
    - `language: Option<String>` (`None` = auto)
    - `model: String` (model file name)
    - `use_gpu: bool` (default true)
    - `gpu_device: i32` (default 0)
    - `groq_model: String`
    - `cleanup: CleanupLevel` (`None`|`Light`|`Medium`, default `Light`)
    - `styles: Styles`, with `personal: Tone`, `work: Tone`, `email: Tone`, `other: Tone`, where `Tone` is `Formal`|`Casual`|`VeryCasual`|`Excited`. Defaults: `Casual`, `Casual`, `Formal`, `Casual`.
    - `category_rules: Vec<CategoryRule { pattern: String, category: Category }>`
    - `smart_formatting: bool` (default true)
    - `backtrack: bool` (default true)
    - `file_tagging: bool` (default false)
    - `context_awareness: bool` (default true)
    - `context_excluded_apps: Vec<String>`
    - `bar_always_visible: bool` (default true)
    - `sounds: bool` (default true)
    - `bar_position: Option<(i32, i32)>`
    - `auto_transform: Option<i64>`
    - `theme: Theme` (`System`|`Light`|`Dark`)
    - `onboarded: bool`
  - `Settings::load(path: &Path) -> Settings`: a missing or corrupt file returns the defaults. A corrupt file is renamed to `settings.json.bak` first.
  - `Settings::save(&self, path: &Path) -> std::io::Result<()>`: write to a `.tmp` file, then rename it over the original.

- [ ] Step 1: Write the tests:
  - `defaults_match_spec`: `cleanup == Light`, `hotkeys.push_to_talk == ["Ctrl+Win"]`, `context_awareness`
  - `roundtrip`
  - `partial_json_fills_defaults`: `{"sounds":false}` loads with the other fields at their defaults
  - `corrupt_file_backs_up_and_defaults`
- [ ] Step 2: Run `cargo test -p blablaflow-core settings`. Expected: FAIL.
- [ ] Step 3: Implement.
- [ ] Step 4: Run the tests. Expected: PASS. Commit with message `feat(core): settings`.

### Task 3: Database and insights

**Files:** Create `crates/core/src/db.rs` and `crates/core/src/insights.rs`.

**Interfaces:**
- Produces:
  - `Db::open(path: &Path) -> rusqlite::Result<Db>`: WAL mode; migrations driven by `PRAGMA user_version`.
  - `Db::open_in_memory()`.
- History:
  - `HistoryEntry { id, created_at: i64 /*unix ms*/, status: Status /*Recording|Processing|Done|Failed|Interrupted*/, raw_text, final_text, app, category, language, duration_ms: i64, words: i64, audio_path: Option<String>, kind: Kind /*Dictation|Command|Transform*/ }`
  - `Db::insert_history(&HistoryEntry) -> i64`
  - `Db::update_history(&HistoryEntry)`
  - `Db::list_history(query: &str, limit: i64, offset: i64) -> Vec<HistoryEntry>`: newest first; `LIKE` search over `raw_text` and `final_text`.
  - `Db::delete_history(id)`
  - `Db::last_done() -> Option<HistoryEntry>`
  - `Db::mark_interrupted() -> usize`: changes Recording/Processing rows to Interrupted. Called at startup.
  - `Db::expired_audio(before_ms) -> Vec<(i64, String)>` and `Db::clear_audio(id)`
- Dictionary:
  - `DictEntry { id, term, replacement: Option<String> }`
  - `add_dict(term, replacement) -> Result<i64, DbError>`: `DbError::TooLong` if over 60 chars; `DbError::Duplicate` if the term matches case-insensitively.
  - `list_dict`, `delete_dict`
- Snippets: `Snippet { id, cue, text }`, with `add_snippet` (cue ≤ 60, text ≤ 4000, unique cue), `update_snippet`, `list_snippets`, `delete_snippet`.
- Transforms:
  - `Transform { id, name, prompt, hotkey: Option<String>, samples: Vec<String>, builtin: bool }`
  - `list_transforms`, `save_transform` (insert or update; `DbError::Limit` past 8 non-builtin)
  - `delete_transform`: built-in transforms cannot be deleted.
  - The migration seeds Polish (`builtin = 1`, hotkey `Win+Alt+1`).
- Notes: `Note { id, title, body, updated_at }`, with `list_notes`, `save_note`, `delete_note`.
- `insights::compute(db: &Db, now_ms: i64) -> Insights { total_words, avg_wpm, words_this_week, streak_days, top_apps: Vec<(String, i64)>, daily: Vec<(String /*YYYY-MM-DD*/, i64)> }`:
  - Counts only `Done` dictations.
  - WPM is `words / (duration_ms / 60000)` over all of them.
  - The streak is consecutive local days ending today or yesterday.

- [ ] Step 1: Write the tests:
  - `history_crud_and_search`
  - `mark_interrupted_only_touches_open_rows`
  - `dict_rejects_dupes_case_insensitive` (`"Grüße"` vs `"GRÜSSE"` is not a dupe; `"grüße"` is)
  - `snippet_limits`
  - `transform_limit_8_and_builtin_undeletable`
  - `insights_streak_and_wpm`: fixed timestamps; 3 consecutive days, then a gap
- [ ] Step 2: Run `cargo test -p blablaflow-core db insights`. Expected: FAIL.
- [ ] Step 3: Implement. Store `samples` as a JSON text column.
- [ ] Step 4: Run the tests. Expected: PASS. Commit with message `feat(core): sqlite storage and insights`.

### Task 4: Text post-processing

**Files:** Create `crates/core/src/text.rs`.

**Interfaces:**
- Produces:
  - `expand_snippets(text: &str, snippets: &[Snippet]) -> String`: whole-word and case-insensitive (Unicode-aware lowercasing). Longest cue wins. Trailing punctuation the LLM added after a cue that took up the whole utterance is dropped.
  - `apply_corrections(text: &str, dict: &[DictEntry]) -> String`: only entries that have a `replacement`. Same matching rules as snippets.
  - `word_count(text: &str) -> i64`: Unicode whitespace split.
  - `is_hallucination(text: &str) -> bool`: true for empty text, punctuation only, or, after trimming and lowercasing, one of `["thank you.", "thanks for watching!", "you", "thank you for watching.", "vielen dank.", "untertitel im auftrag des zdf, 2017", "untertitel der amara.org-community"]` (strip trailing punctuation before comparing).
  - `sanitize_llm_output(raw: &str, out: &str) -> String`:
    - strips one leading line matching `^(here('s| is)|sure|okay)[^\n]*:\s*\n` (case-insensitive)
    - strips surrounding ``` fences and surrounding quotes
    - returns `raw` if the result is empty or longer than `max(3 × raw.chars().count(), raw.chars().count() + 200)`

- [ ] Step 1: Write the tests:
  - `snippet_whole_word_ci`: cue "my calendar" expands inside "Book a slot: My Calendar." but not inside "mycalendar"
  - `snippet_umlaut_case`: cue "grüße" matches "Grüße"
  - `snippet_longest_wins`
  - `corrections_apply`
  - `is_hallucination_cases`: true for `""`, `"..."`, `"Thank you."`, `"you"`, `"Vielen Dank."`; false for `"Thank you for the update, Marcus."`
  - `sanitize_strips_preamble`: `"Here is the cleaned text:\nHello."` → `"Hello."`
  - `sanitize_strips_fences_and_quotes`
  - `sanitize_falls_back_when_runaway`
- [ ] Step 2: Run `cargo test -p blablaflow-core text`. Expected: FAIL.
- [ ] Step 3: Implement. Use the `regex` crate with `(?iu)` and `\b`, with cues passed through `regex::escape`, longest first.
- [ ] Step 4: Run the tests. Expected: PASS. Commit with message `feat(core): text post-processing`.

### Task 5: Hotkeys

**Files:** Create `crates/core/src/hotkey.rs`.

**Interfaces:**
- Produces:
  - `enum Key { Ctrl, Shift, Alt, Win, Space, Escape, CapsLock, Char(char), F(u8), Mouse(u8) /*3=middle,4,5*/, Other(u16 /*VK*/) }`
  - `struct Hotkey(Vec<Key>)`: a set, so order is irrelevant.
  - `Hotkey::parse(s: &str) -> Result<Hotkey, HotkeyError>` and `Display` back to the canonical `"Ctrl+Win+Space"` form.
- `validate(h: &Hotkey, action: Action) -> Result<(), HotkeyError>` enforces these rules:
  - 1 to 3 keys
  - must include a modifier, unless it's a mouse button, CapsLock (push-to-talk/hands-free only) or Escape (cancel only)
  - Mouse 1 and 2 are not allowed
  - rejects Windows-reserved combos: Win alone, or Win + one of E R L D Tab I S X P Up Down Q with no other modifier; also Ctrl+Alt+Delete
- `enum Action { PushToTalk, HandsFree, Command, Cancel, PasteLast, CopyLast, Scratchpad, Transform(i64) }`
- `struct HotkeyMatcher`:
  - `new(bindings: Vec<(Hotkey, Action)>, double_tap_ms: u64)`
  - `on_event(&mut self, key: Key, down: bool, now_ms: u64) -> MatchResult { events: Vec<Trigger>, swallow: bool }`
  - `enum Trigger { Start(Mode), Stop, Cancel, Fire(Action) }`, with `Mode = PushToTalk | HandsFree | Command`
- Matcher semantics:
  - Exact match on the set of currently held keys. If a press is a superset of a binding, the longer binding wins (e.g. `Ctrl+Win+Space` over `Ctrl+Win`); `Start(PushToTalk)` is held back until it is unambiguous, i.e. until the next key event or release. Releasing a binding key while in push-to-talk emits `Stop`.
  - A push-to-talk press-and-release under 300 ms followed by another press within 500 ms switches to hands-free: the second press emits `Start(HandsFree)`, and the next full press emits `Stop`. A lone short tap emits `Start` then `Cancel`, so nothing is pasted.
  - Pressing the `HandsFree` binding toggles. `Cancel` fires only while recording.
  - `swallow` is true for non-modifier keys and mouse buttons that are part of a matched binding. Modifiers are never swallowed. The platform layer handles the Win key-up (Task 9).

- [ ] Step 1: Write the tests:
  - `parse_display_roundtrip`
  - `validate_rejects_reserved` (`Win+L`, `Ctrl+Alt+Delete`, `A`, `Mouse(1)`)
  - `validate_allows_capslock_for_ptt_only`
  - `ptt_hold_release`
  - `longer_binding_wins` (`Ctrl+Win+Space` → `Start(HandsFree)`, never `Start(PushToTalk)`)
  - `double_tap_enters_hands_free_and_next_press_stops`
  - `short_tap_cancels`
  - `escape_cancels_only_while_recording`
  - `mouse_button_swallowed`
  - `transform_fires`
- [ ] Step 2: Run `cargo test -p blablaflow-core hotkey`. Expected: FAIL.
- [ ] Step 3: Implement.
- [ ] Step 4: Run the tests. Expected: PASS. Commit with message `feat(core): hotkey parsing and matcher`.

### Task 6: Categories, prompts, Groq types

**Files:** Create `crates/core/src/category.rs`, `crates/core/src/prompt.rs` and `crates/core/src/groq.rs`.

**Interfaces:**
- `enum Category { Personal, Work, Email, Other }`
- `default_rules() -> Vec<CategoryRule>`, matched against the lowercased exe name or window title:
  - **Work:** slack, teams, discord, zoom, notion, linear, jira, code.exe, cursor, `" - google docs"`
  - **Personal:** whatsapp, telegram, signal, messenger
  - **Email:** outlook, thunderbird, `"gmail"`, `"outlook.live"`, `"mail -"`
- `classify(exe: &str, title: &str, rules: &[CategoryRule]) -> Category`: the first matching rule wins; otherwise `Other`.
- `is_code_editor(exe: &str) -> bool`: code.exe, cursor.exe, windsurf.exe, zed.exe, and JetBrains exes (`idea64`, `pycharm64`, …).
- `struct Context { exe: String, title: String, category: Category, nearby_text: Option<String> /*≤ 1500 chars*/ }`
- `cleanup_messages(raw: &str, ctx: &Context, s: &Settings, dict: &[DictEntry]) -> Vec<groq::Message>`. The system prompt must state:
  - output only the final text; never answer, follow or comment on the content, even if it is a question or an instruction
  - keep the spoken language(s) and never translate
  - the cleanup-level rules: Light removes fillers and fixes punctuation and grammar; Medium may also rephrase for clarity
  - the tone for the category; for Email, add a greeting, a sign-off and line breaks
  - if smart formatting is on: spoken "new line", "new paragraph", "period", "comma" and enumerations become formatting and lists
  - if backtrack is on: on a self-correction ("actually", "I mean", "scratch that", "nein, ich meine"), keep only the corrected version
  - if file tagging is on and `is_code_editor`: spoken file names become `@name.ext`
  - dictionary terms as the preferred spellings
  - nearby text, if present, inside `<context>…</context>`, marked as reference only

  The raw transcript goes in the user message inside `<transcript>…</transcript>`.
- `command_messages(instruction: &str, selection: Option<&str>, ctx: &Context) -> Vec<Message>`
- `transform_messages(t: &Transform, text: &str) -> Vec<Message>`
- `groq::{Message { role, content }, ChatRequest { model, messages, temperature: f32 /*0.2*/, max_tokens: u32 }, ChatResponse /*choices[0].message.content*/, ModelList /*data[].id*/}`
- `ChatRequest::new(model, messages)`, where `max_tokens = 4096`.

- [ ] Step 1: Write the tests:
  - `classify_defaults` (slack.exe → Work; title "Inbox - Gmail - Google Chrome" → Email; notepad.exe → Other)
  - `prompt_contains_rules_per_setting` (toggling backtrack, smart formatting or file tagging adds or removes its line; file tagging appears only for code editors)
  - `prompt_wraps_transcript_and_context`
  - `prompt_never_contains_cleanup_rules_for_none` (callers skip the LLM; the helper `needs_llm(&Settings) -> bool` returns false for `CleanupLevel::None` with no auto transform)
  - `chat_response_parses_groq_json` (fixture)
- [ ] Step 2: Run `cargo test -p blablaflow-core category prompt groq`. Expected: FAIL.
- [ ] Step 3: Implement. Keep the prompt strings as `const`s at the top of `prompt.rs`.
- [ ] Step 4: Run the tests. Expected: PASS. Commit with message `feat(core): categories, prompts, groq types`.

### Task 7: Audio capture

**Files:** Create `src-tauri/src/audio.rs`.

**Interfaces:**
- `list_inputs() -> Vec<String>`
- `Recorder::start(device: Option<&str>, wav_path: &Path, on_level: impl Fn(f32) + Send + 'static) -> Result<Recorder>`:
  - Takes the default config from cpal and downmixes to mono.
  - Resamples linearly to 16 kHz (a shortcut comment says to upgrade to rubato if quality suffers).
  - Writes 16-bit WAV through `hound` and flushes about once a second, so a crash keeps the audio.
  - Emits an RMS level about every 50 ms.
  - If the named device is missing, falls back to the default device and logs it.
- `Recorder::stop(self) -> Result<Vec<f32>>`: returns the 16 kHz samples. A device error mid-recording ends the stream, and `stop` still returns what was captured.
- `read_wav(path) -> Result<Vec<f32>>`: used for retry and recovery.
- `trim_silence(samples: &[f32]) -> &[f32]`: energy gate on 30 ms frames, threshold `max(0.01, 3 × noise floor)`, 200 ms padding. Returns an empty slice if no frame passes (a shortcut comment says to swap in Silero VAD if it misfires).

- [ ] Step 1: Write the tests:
  - `trim_all_silence_returns_empty`
  - `trim_keeps_speech_with_padding` (synthetic sine burst)
  - `resample_48k_to_16k_length`
  - `wav_roundtrip`
- [ ] Step 2: Implement. Push and check that the CI `app` job runs the tests and passes.
- [ ] Step 3: Commit with message `feat(app): audio capture`.

### Task 8: Whisper STT and model download

**Files:** Create `src-tauri/src/stt.rs`.

**Interfaces:**
- `const MODELS: &[ModelInfo { file: &str, label: &str, size_mb: u32 }]`:
  - turbo q5_0, "Large v3 Turbo (recommended)", 574
  - small q5_1, "Small (fast, weaker PCs)", 190
- `download_model(file, dir, on_progress: impl Fn(u64, u64)) -> Result<()>`:
  - Streams with reqwest to `file.part`, then renames it on completion.
  - Before starting, checks free disk space against `size × 1.1` and fails with "Not enough disk space".
- `Stt::load(path: &Path, use_gpu: bool, gpu_device: i32) -> Result<Stt>`
- `Stt::transcribe(&self, samples: &[f32], language: Option<&str>, hint: &str) -> Result<(String, String /*detected lang*/)>`:
  - greedy sampling
  - `set_initial_prompt(hint)`, where the hint is dictionary terms joined with ", " and capped at 200 chars
  - `set_no_context(true)`, `set_suppress_blank(true)`
  - threads = available parallelism − 1
- The `Stt` is cached in app state and reloaded when the model, GPU setting or device changes.

- [ ] Step 1: Write the test `transcribes_sample` behind the `test-with-tiny-model` feature, using whisper-rs's tiny model and a short WAV fixture from the whisper.cpp repo (`jfk.wav`). Assert the output contains "country".
- [ ] Step 2: Implement. Push. The CI `app` job runs the test. Expected: PASS.
- [ ] Step 3: Commit with message `feat(app): whisper stt and model download`.

### Task 9: Windows platform layer

**Files:** Create `src-tauri/src/platform/mod.rs` and `src-tauri/src/platform/windows.rs`.

**Interfaces:**
- `start_hooks(on_event: impl Fn(core::hotkey::Key, bool /*down*/) -> bool /*swallow*/ + Send + 'static)`:
  - A dedicated thread installs `WH_KEYBOARD_LL` and `WH_MOUSE_LL` and runs a message loop. Injected events (`LLKHF_INJECTED`) are ignored.
  - When a binding containing Win fires, send a dummy key `VK 0xE8` before the Win key-up so the Start menu doesn't open.
  - The callback must return quickly, under 1 ms. It only runs the matcher and posts to a channel.
- `foreground() -> (String /*exe file name, lowercased*/, String /*title*/, isize /*hwnd*/)`
- `read_context(max_chars: usize) -> Option<String>`:
  - Uses UIA on the focused element.
  - Returns `None` if the element `IsPassword`, or on any error or timeout (200 ms).
  - Tries TextPattern first: the text around the selection, ±max/2. Falls back to ValuePattern.
- `read_selection() -> Option<String>`: UIA TextPattern `GetSelection`. Fallback: save the clipboard, send Ctrl+C, wait up to 150 ms for a clipboard sequence-number change, read it, restore the clipboard.
- `insert_text(text: &str, exe: &str) -> InsertResult { Pasted, LeftOnClipboard }`:
  - Save clipboard text, set the new text, send the paste combo, wait 150 ms, then restore. Only text is restored, through arboard (shortcut comment: images and other formats are not restored; upgrade by enumerating the formats).
  - Paste combo: Ctrl+Shift+V for `windowsterminal.exe`, `wt.exe`, `alacritty.exe`, `wezterm-gui.exe`; Shift+Insert for `mintty.exe`, `conhost.exe`; Ctrl+V otherwise.
  - The decision is the pure function `paste_restore_policy(send_ok: bool, fg_before: isize, fg_after: isize) -> bool /*restore?*/`: restore only if SendInput reported all events sent and the foreground window is unchanged.
- `set_no_activate(window: &tauri::WebviewWindow)`: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`.
- `send_enter()` is not needed (no "press Enter" action in the spec).

- [ ] Step 1: Write the tests (unit, Windows CI):
  - `paste_restore_policy_cases`
  - `paste_combo_for_exe`
  - `vk_to_key_mapping` (VK_LWIN/VK_RWIN → Win, VK_LCONTROL → Ctrl, XBUTTON1 → Mouse(4))
- [ ] Step 2: Implement. Push. The CI `app` job passes.
- [ ] Step 3: Commit with message `feat(app): windows platform layer`.

### Task 10: Dictation orchestrator and Groq client

**Files:** Create `src-tauri/src/dictation.rs` and `src-tauri/src/llm.rs`.

**Interfaces:**
- `llm::get_key() -> Option<String>` and `llm::set_key(Option<&str>)`, using keyring.
- `llm::chat(key, req: &ChatRequest) -> Result<String>`: 15 s timeout, one retry on 429 or 5xx after the `retry-after` delay (capped at 2 s).
- `llm::list_models(key) -> Result<Vec<String>>`
- `BarState` (serialized, emitted as event `bar`): `Idle | Recording { mode, level, elapsed_ms } | Processing | Message { text, kind: Info|Error }`
- `Dictation::new(app: AppHandle) -> Arc<Dictation>` and `Dictation::on_trigger(Trigger)`
- Job pipeline (one worker thread, jobs in FIFO order so pastes happen in order):
  1. **Start:**
     - Capture `foreground()` and the context (if enabled, the app isn't excluded, and the mode isn't Command, which uses the selection instead).
     - Insert a history row with status `Recording` and an audio path under `audio/<unix_ms>.wav`.
     - Start the `Recorder` and play the start sound (the overlay emits it).
     - Start a timer: `Message` warning at 19 min, auto-`Stop` at 20 min.
  2. **Stop:** stop the recorder, set status `Processing`, and queue the job.
  3. **Cancel:** stop the recorder, delete the row and the WAV.
  4. **Worker:**
     - Trim silence. If the result is empty, delete the row and the WAV and show nothing.
     - Transcribe. If `is_hallucination`, delete the row and the WAV and show nothing.
     - Dictation: run cleanup if `needs_llm` and a key is set, then `sanitize_llm_output`. On LLM error, use the raw text and show `Message("Cleanup skipped – <reason>")`.
     - Then apply corrections, expand snippets, run the auto transform if set (on failure keep the text), and `insert_text`.
     - Command mode: `read_selection()`, then `command_messages`; the result replaces the selection (paste) or is inserted.
     - Set history to `Done` with the final text and the word count. On error set `Failed` and show `Message`.
     - `LeftOnClipboard` shows `Message("Copied – press Ctrl+V")`.
- `Dictation::run_transform(id)`: read the selection, call Groq, insert the result, add a history row with kind `Transform`.
- `Dictation::retry(history_id)`: re-transcribe from the WAV if there is one, otherwise re-clean from `raw_text`. Updates the row and copies the result to the clipboard; it does not paste.
- At startup: `db.mark_interrupted()` and delete audio older than 14 days.

- [ ] Step 1: Put the worker behind a trait `Pipeline { transcribe, clean, insert }`, so it can be tested with fakes. Write these tests:
  - `jobs_paste_in_order`: two jobs, the first with a slow fake transcribe; insert order is 1, 2
  - `silent_job_leaves_no_history`
  - `llm_error_falls_back_to_raw`
  - `cancel_removes_row_and_wav`
- [ ] Step 2: Implement. Push. The CI `app` job passes.
- [ ] Step 3: Commit with message `feat(app): dictation pipeline and groq client`.

### Task 11: App wiring: commands, tray, overlay

**Files:** Modify `src-tauri/src/main.rs`. Create `src-tauri/src/commands.rs`.

**Interfaces:**
- Commands (all return `Result<T, String>`):
  - `get_settings`, `save_settings(settings)`: re-registers the hotkeys in the matcher and reloads Stt if needed.
  - `list_mics`
  - `list_models_local`: `{file, label, size_mb, downloaded}`
  - `download_model(file)`: emits `model-progress {file, done, total}`
  - `delete_model(file)`
  - `get_groq_key_set -> bool`, `set_groq_key(key: Option<String>)`, `list_groq_models`, `test_groq -> String`
  - `history_list(query, offset)`, `history_delete(id)`, `history_retry(id)`, `history_copy(id, raw: bool)`
  - `insights`
  - dictionary, snippet, transform and note CRUD mirroring `Db`
  - `import_items(kind, json)` and `export_items(kind) -> String`: JSON arrays of `{term, replacement}` or `{cue, text}`
  - `validate_hotkey(s, action) -> Result<String /*canonical*/, String>`
  - `set_paused(bool)`, `hide_bar_for(minutes)`
- Startup:
  - Open the DB and settings in `app_data_dir`.
  - `start_hooks` → `HotkeyMatcher` → `Dictation::on_trigger`. `Fire(PasteLast | CopyLast | Scratchpad | Transform)` are handled directly.
  - The overlay is positioned at `bar_position` or bottom-centre of the primary monitor, 80 px above the bottom; `set_no_activate`.
  - If `!onboarded`, show `main`.
  - Single instance: a second launch focuses `main` (`tauri-plugin-single-instance`).
- The tray's Pause sets a flag that makes the matcher ignore everything.

- [ ] Step 1: Push. Expected: CI builds the installer.
- [ ] Step 2: Commit with message `feat(app): commands, tray, overlay wiring`.

### Task 12: Flow bar UI

**Files:** Create `src/overlay.tsx` and `src/api.ts` (shared).

**Interfaces:**
- Consumes the events `bar` and `model-progress`, and the commands from Task 11.

- [ ] Step 1: Implement the pill states.
  - Idle: a 40×6 line, hidden when `!bar_always_visible`.
  - Recording: 7 bars driven by `level`, and `m:ss` after 10 s.
  - Processing: a spinner.
  - Message: text that auto-hides after 3 s.
  - Start and stop sounds: WebAudio short sine blips, 660 Hz and 440 Hz, 60 ms, played only when `sounds` is on.
- [ ] Step 2: Dragging (mousedown → `startDragging()`) snaps to the left or right edge or the bottom-centre on release, and saves `bar_position`.
- [ ] Step 3: The right-click menu, rendered inside the overlay, offers: Language (Auto + common list), Hide for 1 hour, Open blablaflow.
- [ ] Step 4: `npm run build` passes. Commit with message `feat(ui): flow bar`.

### Task 13: Main window UI and onboarding

**Files:** Create `src/App.tsx` and `src/pages/{Home,Insights,Dictionary,Snippets,Style,Transforms,Scratchpad,Settings,Onboarding}.tsx`.

- [ ] Step 1: Build the shell: a sidebar with the 8 pages and a theme from settings (Tailwind `dark` class, with `System` following `prefers-color-scheme`). Clean, minimal, light-first.
- [ ] Step 2: Build the pages as listed in the spec's "Main window" section. Each page calls only the Task 11 commands.
  - Home: search box; entries grouped by date; actions Copy / Copy raw ("Undo AI edit") / Retry / Delete; "Interrupted" badge with Retry.
  - Insights: tiles plus a 12-week heatmap of `daily`.
  - Dictionary / Snippets: list, add form with inline validation errors, import / export.
  - Style: cleanup level, four tone selects, category rules editor, three toggles.
  - Transforms: list, editor (name, prompt, hotkey capture, up to 5 samples), auto-apply select.
  - Scratchpad: tabs (cap of 5 open), textarea autosaving after 500 ms of inactivity.
  - Settings: hotkey capture fields (record keydown in the webview → `validate_hotkey`), mic select with level meter, language, models with download progress and GPU toggle and device index, Groq key (password input) with model select and Test, context awareness with excluded apps, Flow bar options, theme.
- [ ] Step 3: Onboarding, a 5 step wizard shown when `!onboarded`: mic test, hotkey, model download, Groq key plus context-awareness sentence and toggle, practice box. Finishing sets `onboarded`.
- [ ] Step 4: `npm run build` passes. Commit with message `feat(ui): main window and onboarding`.

### Task 14: README and release

**Files:** Create `README.md` and `.github/workflows/release.yml`.

- [ ] Step 1: Write the README: what it is, install (download the setup .exe from Releases; the SmartScreen "More info → Run anyway" note), get a Groq key, default hotkeys, privacy (audio is local; text and nearby context go to Groq; how to turn context off), build from source, MIT.
- [ ] Step 2: The release workflow runs on a `v*` tag push: `tauri-apps/tauri-action` with `--features vulkan` creates a draft GitHub Release with the NSIS installer.
- [ ] Step 3: Commit with message `docs: readme and release workflow`. Push. CI green.
