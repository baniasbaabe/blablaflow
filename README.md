# blablaflow

Hold a hotkey, speak, let go – clean, punctuated text appears wherever your cursor is, in any Windows app.

An open-source take on [Wispr Flow](https://wisprflow.ai)'s dictation. Speech recognition runs **on your computer** with [Whisper](https://github.com/ggml-org/whisper.cpp). Only the text is sent to [Groq](https://groq.com), with your own free API key, to remove filler words, fix punctuation and match your style.

## Install

1. Download `blablaflow_x.y.z_x64-setup.exe` from the [latest release](../../releases/latest) and run it.
   The installer isn't code-signed, so Windows SmartScreen may say "Windows protected your PC": click **More info → Run anyway**.
2. blablaflow opens a short setup:
   1. **Microphone** – pick yours (a mic close to your mouth works best; Bluetooth earbuds tend to cut off the first words).
   2. **Shortcut** – the push-to-talk key, `Ctrl+Win` by default.
   3. **Speech model** – download one once:
      - **Large v3 Turbo** (~575 MB) – best accuracy; needs a GPU to be fast.
      - **Small** (~190 MB) – fast on any PC, a bit less accurate.
   4. **Groq API key** – optional but recommended. Create a free key at [console.groq.com/keys](https://console.groq.com/keys) and paste it in. Without a key you get Whisper's raw transcript.
   5. **Try it** – a practice box to check everything works.
3. blablaflow now lives in the system tray (bottom-right, next to the clock). Closing the window keeps it running; quit it from the tray icon.

**Requirements:** Windows 10 or 11 (64-bit). A graphics card (NVIDIA, AMD or Intel) makes transcription much faster; without one, the CPU is used automatically.

## Use it

### Dictate

| What you want | How |
| --- | --- |
| Dictate a sentence | Click into any text field, **hold `Ctrl+Win`**, speak, **let go**. The text is pasted at your cursor. |
| Dictate hands-free | Press **`Ctrl+Win+Space`** (or quickly **double-tap `Ctrl+Win`**). Speak as long as you like (up to 20 minutes), then press the shortcut again to stop. |
| Cancel | Press **`Esc`** while recording. Nothing is pasted. |
| Paste the last text again | `Shift+Alt+Z` (copy it instead: `Shift+Alt+X`). |

Speak naturally. blablaflow:

- removes filler words ("um", "uh", "äh") and stutters, and fixes punctuation;
- understands corrections – "let's meet at 5, actually 6" → "Let's meet at 6.";
- turns spoken formatting into formatting – "new line", "new paragraph", "period", and spoken lists ("first… second…") become real lists;
- detects the language you speak (100+ languages) and never translates.

### The Flow bar

The small pill near the bottom of your screen shows what blablaflow is doing:

| Looks like | Means |
| --- | --- |
| Thin grey line | Ready |
| Bars moving with your voice | Recording |
| Three pulsing dots | Turning your speech into text |
| Short message | e.g. "Copied – press Ctrl+V" when the target app didn't accept the paste |

Drag it anywhere. **Right-click** it to switch language, hide it for an hour, or open the app. To only see it while recording, turn off *Settings → Show the bar when idle*.

### Edit text by voice

- **Command Mode** – select some text, **hold `Ctrl+Win+Alt`** and say what to do: "make this shorter", "translate to German", "turn this into bullet points". With nothing selected, it writes new text: "draft a polite reply saying I can't make it".
- **Transforms** – saved rewrite prompts on a shortcut. Select text and press `Win+Alt+1` to *Polish* it. Create up to 8 of your own under **Transforms** (e.g. "Rewrite as a friendly tweet"), optionally with samples of your writing so the result sounds like you, and optionally applied to every dictation automatically.

### Make it yours

| Page | What it does |
| --- | --- |
| **Style** | Cleanup level (None / Light / Medium), and a tone per kind of app: personal messages, work chat, email, everything else (Formal, Casual, Very casual, Excited). Add your own rules for which app counts as what. |
| **Dictionary** | Names and jargon blablaflow should spell right ("Kubernetes", "Siobhán"). Add a replacement to fix a word every time ("wisper" → "Wispr"). Import/export as JSON. |
| **Snippets** | Say a cue, get the full text: "my calendar link" → `https://cal.com/you`. Import/export as JSON. |
| **Home** | Everything you dictated, searchable. Copy, retry, or *Undo AI edit* to get Whisper's raw text back. |
| **Insights** | Words per minute, total words, streaks and a daily activity grid. |
| **Scratchpad** | Notes to dictate into, also with `Win+Alt+S`. |
| **Settings** | Shortcuts (keys or mouse side buttons), microphone, language, speech model and GPU, Groq key and model, context awareness, Flow bar, theme. |

**Code editors:** turn on *Style → File tagging* and saying "at main dot py" in Cursor or VS Code writes `@main.py`.

## Privacy

- Audio is recorded only while you hold or toggle the shortcut, and is transcribed **on your computer**. It never leaves it.
- With cleanup on, the **text** of your dictation is sent to Groq. With *context awareness* on, up to 1,500 characters of text around your cursor are sent too, so names and style match. Password fields are never read. Turn context off, or exclude specific apps, in *Settings → Privacy*.
- History, dictionary, snippets and notes are stored locally in `%APPDATA%\com.blablaflow.app`. Recordings are deleted after 14 days.
- Your Groq key is stored in Windows Credential Manager. There is no telemetry.

## Troubleshooting

| Problem | Fix |
| --- | --- |
| Nothing happens when I press `Ctrl+Win` | Check the tray icon isn't set to *Pause dictation*. Some keyboard tools (AutoHotkey, Logitech Options) remap keys in a way blablaflow can't see – try another shortcut in Settings. |
| Text doesn't appear in an app run as administrator | Windows blocks typing into admin windows from normal apps. The text is on your clipboard: press `Ctrl+V`. |
| The first dictation after starting is slow | The GPU is warming up ("Warming up GPU…" on the bar). Later dictations are fast. |
| Dictation is slow every time | Pick the *Small* model in Settings, or check that *Use GPU* is on and the right GPU is selected. |
| "Cleanup skipped" on the bar | Groq was unreachable or the key is wrong; you got the raw transcript. Test the key in Settings. |
| Words get cut off at the start | Use a wired or built-in mic instead of Bluetooth earbuds. |

To uninstall, use *Windows Settings → Apps*. Your data stays in `%APPDATA%\com.blablaflow.app` until you delete that folder.

## Build from source

Needs Rust (stable, MSVC), Node 22, the Visual Studio C++ build tools and – for GPU support – the [Vulkan SDK](https://vulkan.lunarg.com/).

Build in a short folder: whisper.cpp's GPU build creates paths longer than Windows' 260-character limit.

```powershell
$env:CARGO_TARGET_DIR = "C:\t"
npm ci
npx tauri dev --features vulkan     # run
npx tauri build --features vulkan   # installer in C:\t\release\bundle\nsis
cargo test --workspace              # tests
npm test
```

`crates/core` holds all platform-independent logic (settings, storage, hotkey matching, prompts, text processing) and is tested on any OS. `src-tauri` is the Windows app; `src` is the React UI. Releases are built by GitHub Actions when a `v*` tag is pushed.

## License

MIT
