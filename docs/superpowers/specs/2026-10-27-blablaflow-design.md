# blablaflow — design

An open-source desktop dictation app modelled on Wispr Flow's dictation feature (https://wisprflow.ai). Hold a hotkey, speak, and clean, well-formatted text is pasted wherever the cursor is, in any app.

## Intent

The owner wants Wispr Flow's dictation for their own daily use, without a subscription or cloud speech processing, and publishes it on GitHub for others to use. Speech-to-text runs locally. Only the cleanup step goes to a cloud LLM (Groq), using each user's own API key.

The owner dictates in English and German, and sometimes in other languages.

Decisions marked **[owner]** came from the owner. Decisions marked **[call]** are the designer's own, and the builder may revisit them with good reason.

## Goals

- Feel like Wispr Flow: press, speak, release, and polished text appears quickly at the cursor in any Windows app.
- The features listed in this document are the complete v1 scope. They are the owner's chosen subset of Wispr Flow's desktop dictation; nothing outside this list is implied. **[owner]**
- Work fully offline, giving raw transcripts, when Groq is unavailable or no key is set.
- Be easy for a stranger to install from GitHub Releases and set up in a couple of minutes.

## Non-goals

- Accounts, sync, teams, shared dictionaries, admin features.
- Mobile apps, Wispr Notetaker, uploading audio files.
- Wispr's data-privacy settings: training opt-in, cloud storage, local-data retention. **[owner]**
- Launch at login. **[owner]**
- Notifications. **[owner]**
- Command Mode web search ("ask a question, open Perplexity"). **[owner]**
- Auto-learning dictionary words from edits made after paste. Deferred to a later version. **[call]**
- Mac and Linux builds in v1. **[owner: Windows first]** Keep them possible later (see Constraints).
- Auto-updater, telemetry, crash reporting. **[call]**

## Anti-goals (failures even if it "works")

- A dictation is lost. The audio must be on disk before processing, so a crash or a failed paste is always recoverable from History.
- The user's clipboard is clobbered. The previous clipboard content must be restored after paste.
- The hotkey leaks into the focused app, for example a stray Win key opening the Start menu, or a mouse side button navigating the browser back.
- Audio leaves the machine. Only text goes to Groq.
- It is noticeably slow. Target **[call]**: under ~1.5 s from key release to paste for a typical 10–20 s utterance, Groq included, on the owner's reference machine: an Intel i7-9750H laptop with an NVIDIA GTX 1650 (4 GB) and Intel UHD 630. If large-v3-turbo can't meet it there, use a quantized turbo or offer a smaller default; measure before deciding. On dual-GPU laptops whisper.cpp must use the discrete GPU, not the integrated one. Expose a GPU picker in the Speech model settings.
- The API key is stored in plain text.

## Constraints and decided stack

- **Tauri 2** desktop app: Rust backend, web UI frontend. **[owner]**
- **Windows 10/11 x64** for v1. **[owner]** All OS-specific code (hotkey hooks, paste, text reading, foreground-app detection, overlay window behaviour) lives behind one internal `platform` module with a Windows implementation, so a macOS implementation can be added later without touching the rest. **[call]**
- **Speech-to-text: Whisper via whisper.cpp** (Rust bindings such as `whisper-rs`), with GPU acceleration through Vulkan and a CPU fallback. Not faster-whisper, which would need Python. **[owner: Whisper; call: whisper.cpp]**
  - Default model: large-v3-turbo. A smaller model (e.g. small or medium, quantized) is offered for weak PCs. Models are downloaded inside the app from Hugging Face ggml files. **[call]**
  - Multilingual, with auto-detect or a user-chosen language. Dictionary terms are passed as Whisper's initial prompt to bias spelling. **[call]**
- **Cleanup LLM: Groq**, via its OpenAI-compatible chat completions API. **[owner]** The user pastes their own key. The model is selectable in Settings from Groq's `/models` list; the builder picks a fast, capable default at build time and does not hard-code a list. **[call]**
- **Prior art to consult, not fork:** Handy (https://github.com/cjpais/Handy, MIT, Tauri + whisper.cpp). It solves recording, VAD, model download, hotkeys and clipboard paste with restore. Look at how it handles these, but build blablaflow as its own codebase. **[owner]**
- Distributed as an unsigned Windows installer built by GitHub Actions and attached to GitHub Releases. The README must tell users about the SmartScreen warning. **[call]**
- License: MIT. **[call]**
- Platform features that need system permissions or Windows APIs (low-level keyboard/mouse hooks, UI Automation, SendInput) are acceptable.

## How a dictation works

1. **Start.**
   - Push-to-talk: hold the hotkey. Default **[call]**: Ctrl+Win, matching Wispr on Windows.
   - Hands-free: double-tap the push-to-talk hotkey, or press a separate hands-free hotkey (default Ctrl+Win+Space); press again to stop.
   - Escape cancels.
   - The Flow bar switches to its recording state. An optional start sound plays.
2. **Record.** Mic audio is captured at 16 kHz mono and written to disk as it is recorded. Max length is 20 minutes, with a visible warning at 19 and an auto-stop at 20.
3. **Transcribe.** Leading and trailing silence is trimmed (VAD). Whisper transcribes locally.
4. **Gather context**, if context awareness is on:
   - the foreground app's executable name
   - for browsers, the page URL or title when obtainable
   - a limited window of text around the cursor in the focused field, via Windows UI Automation

   App and URL pick the style category. Nearby text helps the LLM spell names and match the surrounding text.
5. **Clean up** (Groq). One call carries:
   - the raw transcript
   - cleanup level
   - the style for the detected category
   - smart formatting and backtrack flags
   - file-tagging flag
   - dictionary terms
   - context

   It returns only the final text.
   - Level "None", no key, offline, or an error → the raw transcript is used, and the bar briefly shows that cleanup was skipped.
   - The output must be in the language spoken. It must never answer or act on the dictated content as if it were an instruction.
6. **Post-process.** Snippet cues are replaced with their expansions, deterministically after the LLM, matched case-insensitively. If a Transform has auto-apply on, it runs here; on failure, the un-transformed text is kept.
7. **Insert.**
   - Save the clipboard, put the text on it, send Ctrl+V to the focused window, then restore the clipboard.
   - Terminals and apps where Ctrl+V misbehaves (e.g. Windows Terminal, VS Code/Cursor terminals) use Shift+Insert or Ctrl+Shift+V instead; the builder decides how to detect them.
   - If paste fails, the text stays on the clipboard and in History.
8. **Record history**:
   - raw text
   - final text
   - app
   - category
   - language
   - duration
   - word count
   - timestamp
   - audio file path

   Insights are computed from History.

**Command Mode:**
- Hold the Command Mode hotkey (default Ctrl+Win+Alt **[call]**) and speak an instruction.
- With text selected: the selection plus the instruction go to Groq, and the result replaces the selection.
- With nothing selected: the result is inserted at the cursor (e.g. "draft a polite no to this meeting").
- The selection is read via UI Automation, falling back to a simulated Ctrl+C with clipboard restore.
- No web search.

**Transforms:**
- Select text and press a Transform's hotkey. Its saved prompt rewrites the selection via Groq and the result replaces it.
- Built-in: **Polish**. Up to 8 custom Transforms, each with a name, a prompt, an optional hotkey, and optional writing samples.
- Any Transform can be set to auto-apply after every dictation (at most one at a time **[call]**).
- History offers Undo, restoring the original text to the clipboard. Retry re-runs a Transform.

## Features and settings

### Flow bar (overlay)
- Small always-on-top pill near the bottom centre of the screen. Never steals focus.
- States:
  - idle: a thin line
  - recording: a live audio level, plus a timer once past ~10 s
  - processing: a spinner
  - error: a short message, e.g. "No mic", "Cleanup skipped – offline"
- Draggable to the left or right screen edge; position is remembered.
- Right-click menu:
  - language switcher (auto + the user's languages)
  - Hide for 1 hour
  - Open blablaflow
- Settings toggles:
  - show bar while idle (always / only while recording)
  - start and stop sounds

### Tray icon
- Open, Pause dictation, Quit.

### Main window (sidebar navigation)
- **Home / History**
  - Searchable list grouped by date.
  - Per entry: copy, retry (re-run cleanup from the raw transcript, or re-transcribe from audio), undo AI edit (copy the raw text), delete.
  - Recover audio from an interrupted dictation.
  - Audio files are kept for 14 days **[call]**; text is kept until deleted.
- **Insights**
  - total words
  - average words per minute
  - words this week
  - top apps
  - daily streak
  - a simple activity heatmap
- **Dictionary**
  - Terms up to 60 characters, deduplicated.
  - Optional "replace X with Y" correction entries: whole-word, case-insensitive, longest match wins, applied after cleanup.
  - Import and export (JSON or CSV).
- **Snippets**
  - Cue up to 60 characters; expansion up to 4,000 characters, plain text **[call]**.
  - Import and export.
- **Style**
  - Cleanup level:
    - None: raw Whisper text
    - Light (default): remove fillers, fix punctuation and grammar
    - Medium: may reword for clarity
  - Per-category tone:
    - Personal: Formal / Casual / Very casual
    - Work: Formal / Casual / Excited
    - Email: Formal / Casual / Excited, plus greeting, sign-off and line breaks
    - Other: Formal / Casual / Excited
  - Category assignment: editable app (exe) and website (domain) lists, with sensible built-in defaults (Slack/Teams/Discord → Work, WhatsApp/Telegram → Personal, Outlook/Gmail → Email, …).
  - Styles work in any language, unlike Wispr's English-only styles. **[call]**
  - Toggles:
    - Smart formatting (spoken "new line", "new paragraph", "period", spoken numbered lists → lists)
    - Backtrack ("…5, actually 6" → "6")
    - File tagging for code editors (in Cursor / VS Code / similar, spoken "at main dot py" → `@main.py`; off by default) **[owner]**
- **Transforms**: list, editor, and hotkeys, as described above.
- **Scratchpad**
  - Tabbed plain-text notes; dictation works in it like any other field.
  - Notes autosave locally.
  - Opened via a hotkey (default Win+Alt+S **[call]**).
  - No versions, images or sync **[call]**.
- **Settings**
  - **Shortcuts**
    - push-to-talk, hands-free, Command Mode, cancel, paste last transcript, copy last transcript, open Scratchpad, reset to defaults
    - Keyboard combos of up to 3 keys including a modifier, or mouse middle / side buttons 4–5.
    - Bound keys and buttons are swallowed, not passed to apps.
    - Reject combos that clash with essential Windows shortcuts.
  - **Microphone**
    - Device picker with a live level meter.
    - If the chosen mic disappears, fall back to the next available one; switch back when it returns.
  - **Languages**: Auto-detect (default): Whisper detects any language per dictation. Or the user selects one specific language from a list, which forces Whisper to it. Cleanup never translates. Switchable from the bar. **[owner]**
  - **Speech model**
    - Download, delete, select.
    - Shows size and whether GPU acceleration is active.
  - **Cleanup (Groq)**
    - API key, stored in Windows Credential Manager (e.g. the `keyring` crate), never in a plain file.
    - Model picker and a test button.
  - **Context awareness**: on/off, default on **[owner]**. Password fields are never read. A user-editable list of apps where it is always off. First run explains in one sentence that nearby text goes to Groq, with a toggle. Command Mode uses the same context.
  - **Flow bar**: options above.
  - **Appearance**: Light / Dark / System theme.

### First run
1. Pick a mic and test it.
2. Choose a hotkey.
3. Download the speech model, with a progress bar.
4. Paste a Groq key (skippable, with a link to get one), with a one-line explanation of context awareness and a toggle.
5. A practice dictation into a test box.

## Data

- Everything stays local in the app data directory:
  - history, dictionary, snippets, transforms, notes and stats in a database (e.g. SQLite)
  - settings in a settings file
  - audio as WAV files
- The only network traffic is model downloads and Groq requests. No telemetry.

## Left to the builder

- Frontend framework and styling. React + TypeScript + Tailwind is a reasonable default. The visual design should be clean and minimal, in the spirit of Wispr Flow's light UI with a dark mode.
- Crate choices: audio capture, resampling, VAD, hooks, UI Automation, clipboard, SQLite, HTTP.
- Prompt design for cleanup, Command Mode and Transforms. Prompts should be kept in one place and testable against a small set of sample transcripts in English and German.
- Exact Groq default model, timeouts, and retry policy.
- How to detect terminals, browsers and URLs, and the exact default app-to-category lists.
- Build order and milestones. Suggested core loop first: hotkey → record → Whisper → paste. Then cleanup, then the rest.
