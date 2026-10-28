# blablaflow

Hold a hotkey, speak, let go – clean, punctuated text appears wherever your cursor is, in any Windows app.

An open-source take on [Wispr Flow](https://wisprflow.ai)'s dictation. Speech recognition runs **on your computer** with [Whisper](https://github.com/ggml-org/whisper.cpp); only the text is sent to [Groq](https://groq.com) to remove filler words, fix punctuation and match your style – with your own free API key.

## Features

- **Push-to-talk** (hold `Ctrl+Win`) or **hands-free** (double-tap it, or `Ctrl+Win+Space`); `Esc` cancels. Mouse side/middle buttons work too.
- **Cleanup levels** – none (raw Whisper), light (fillers, punctuation, grammar) or medium (may rephrase).
- **Smart formatting** – "new paragraph", "period", spoken lists.
- **Backtrack** – "let's meet at 5, actually 6" → "Let's meet at 6."
- **Styles per app** – formal, casual, very casual or excited for personal messages, work chat, email and everything else.
- **Dictionary** – names and jargon, plus "replace X with Y" corrections.
- **Snippets** – say "my calendar link", get the full URL.
- **Command Mode** – select text, hold `Ctrl+Win+Alt`, say "make this shorter".
- **Transforms** – saved rewrite prompts on a hotkey (`Win+Alt+1` = Polish), optionally applied to every dictation.
- **Context awareness** – reads a little text around your cursor so names and tone match (never password fields).
- **File tagging** for Cursor / VS Code – "at main dot py" → `@main.py`.
- **History** with retry and "undo AI edit", **Insights** (words per minute, streaks), a **Scratchpad**, 100+ languages with auto-detect.
- Long dictations (up to 20 minutes); audio is saved as you speak, so a crash never loses a dictation.

## Install

1. Download `blablaflow_x.y.z_x64-setup.exe` from [Releases](../../releases) and run it.
   The installer isn't code-signed, so Windows SmartScreen may warn you: click **More info → Run anyway**.
2. The setup guide walks you through your microphone, shortcut, the speech model download (~575 MB) and your Groq key.
3. Get a free Groq API key at [console.groq.com/keys](https://console.groq.com/keys). Without one you get the raw Whisper transcript.

Requires Windows 10/11 (x64). A GPU (NVIDIA, AMD or Intel, via Vulkan) makes transcription much faster; the CPU works too – pick the *Small* model on older machines.

## Default shortcuts

| Action | Shortcut |
| --- | --- |
| Push to talk (double-tap = hands-free) | `Ctrl+Win` |
| Hands-free start/stop | `Ctrl+Win+Space` |
| Command Mode | `Ctrl+Win+Alt` |
| Cancel | `Esc` |
| Paste / copy last transcript | `Shift+Alt+Z` / `Shift+Alt+X` |
| Scratchpad | `Win+Alt+S` |
| Polish selected text | `Win+Alt+1` |

All of them can be changed in Settings.

## Privacy

- Audio is recorded only while you hold the shortcut and is transcribed locally. It never leaves your computer.
- With cleanup on, the transcript is sent to Groq. With context awareness on, up to 1,500 characters of text around your cursor are sent too. Turn either off in Settings, or exclude specific apps.
- History, dictionary, snippets and notes live in a local database in `%APPDATA%\com.blablaflow.app`. Recordings are deleted after 14 days.
- Your Groq key is stored in Windows Credential Manager. No telemetry.

## Build from source

Needs Rust (stable, MSVC), Node 22, the Visual Studio C++ build tools and – for GPU support – the [Vulkan SDK](https://vulkan.lunarg.com/).

```powershell
npm ci
npx tauri dev --features vulkan     # run
npx tauri build --features vulkan   # installer in target/release/bundle/nsis
cargo test --workspace              # tests
```

`crates/core` holds all platform-independent logic (settings, storage, hotkey matching, prompts, text processing) and is tested on any OS. `src-tauri` is the Windows app; `src` is the React UI.

## License

MIT
