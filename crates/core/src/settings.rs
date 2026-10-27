use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::category::Category;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CleanupLevel {
    None,
    #[default]
    Light,
    Medium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tone {
    Formal,
    #[default]
    Casual,
    VeryCasual,
    Excited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Styles {
    pub personal: Tone,
    pub work: Tone,
    pub email: Tone,
    pub other: Tone,
}

impl Default for Styles {
    fn default() -> Self {
        Self { personal: Tone::Casual, work: Tone::Casual, email: Tone::Formal, other: Tone::Casual }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryRule {
    pub pattern: String,
    pub category: Category,
}

/// Each action holds up to 4 bindings in canonical `Hotkey` string form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub push_to_talk: Vec<String>,
    pub hands_free: Vec<String>,
    pub command: Vec<String>,
    pub cancel: Vec<String>,
    pub paste_last: Vec<String>,
    pub copy_last: Vec<String>,
    pub scratchpad: Vec<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        let v = |s: &str| vec![s.to_string()];
        Self {
            push_to_talk: v("Ctrl+Win"),
            hands_free: v("Ctrl+Win+Space"),
            command: v("Ctrl+Win+Alt"),
            cancel: v("Escape"),
            paste_last: v("Shift+Alt+Z"),
            copy_last: v("Shift+Alt+X"),
            scratchpad: v("Win+Alt+S"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkeys: Hotkeys,
    pub mic: Option<String>,
    /// `None` = auto-detect.
    pub language: Option<String>,
    pub model: String,
    pub use_gpu: bool,
    pub gpu_device: i32,
    pub groq_model: String,
    pub cleanup: CleanupLevel,
    pub styles: Styles,
    pub category_rules: Vec<CategoryRule>,
    pub smart_formatting: bool,
    pub backtrack: bool,
    pub file_tagging: bool,
    pub context_awareness: bool,
    pub context_excluded_apps: Vec<String>,
    pub bar_always_visible: bool,
    pub sounds: bool,
    pub bar_position: Option<(i32, i32)>,
    pub auto_transform: Option<i64>,
    pub theme: Theme,
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkeys: Hotkeys::default(),
            mic: None,
            language: None,
            model: "ggml-large-v3-turbo-q5_0.bin".into(),
            use_gpu: true,
            gpu_device: 0,
            groq_model: String::new(),
            cleanup: CleanupLevel::Light,
            styles: Styles::default(),
            category_rules: Vec::new(),
            smart_formatting: true,
            backtrack: true,
            file_tagging: false,
            context_awareness: true,
            context_excluded_apps: Vec::new(),
            bar_always_visible: true,
            sounds: true,
            bar_position: None,
            auto_transform: None,
            theme: Theme::System,
            onboarded: false,
        }
    }
}

impl Settings {
    /// Missing file → defaults. Corrupt file → renamed to `.bak`, then defaults.
    pub fn load(path: &Path) -> Settings {
        let Ok(text) = std::fs::read_to_string(path) else { return Settings::default() };
        serde_json::from_str(&text).unwrap_or_else(|_| {
            let _ = std::fs::rename(path, path.with_extension("json.bak"));
            Settings::default()
        })
    }

    /// Atomic write: temp file, then rename over the original.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let s = Settings::default();
        assert_eq!(s.cleanup, CleanupLevel::Light);
        assert_eq!(s.hotkeys.push_to_talk, vec!["Ctrl+Win"]);
        assert_eq!(s.hotkeys.hands_free, vec!["Ctrl+Win+Space"]);
        assert_eq!(s.hotkeys.command, vec!["Ctrl+Win+Alt"]);
        assert_eq!(s.hotkeys.cancel, vec!["Escape"]);
        assert_eq!(s.hotkeys.scratchpad, vec!["Win+Alt+S"]);
        assert!(s.context_awareness && s.smart_formatting && s.backtrack && s.use_gpu);
        assert!(!s.file_tagging && !s.onboarded);
        assert_eq!(s.styles.email, Tone::Formal);
        assert_eq!(s.model, "ggml-large-v3-turbo-q5_0.bin");
        assert_eq!(s.language, None);
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        let mut s = Settings::default();
        s.sounds = false;
        s.language = Some("de".into());
        s.save(&p).unwrap();
        assert_eq!(Settings::load(&p), s);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, r#"{"sounds":false,"hotkeys":{"cancel":["Ctrl+Q"]}}"#).unwrap();
        let s = Settings::load(&p);
        assert!(!s.sounds);
        assert_eq!(s.hotkeys.cancel, vec!["Ctrl+Q"]);
        assert_eq!(s.hotkeys.push_to_talk, vec!["Ctrl+Win"]);
        assert_eq!(s.cleanup, CleanupLevel::Light);
    }

    #[test]
    fn corrupt_file_backs_up_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, "{not json").unwrap();
        assert_eq!(Settings::load(&p), Settings::default());
        assert_eq!(std::fs::read_to_string(dir.path().join("settings.json.bak")).unwrap(), "{not json");
    }

    #[test]
    fn missing_file_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(&dir.path().join("nope.json")), Settings::default());
    }
}
