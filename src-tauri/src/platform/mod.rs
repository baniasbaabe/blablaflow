//! Everything that talks to the OS. v1 implements Windows only; a macOS
//! implementation would provide the same functions.
#[cfg(windows)]
mod win;
#[cfg(windows)]
pub use win::*;

/// What a global key hook callback tells the platform layer.
#[derive(Debug, Clone, Copy, Default)]
pub struct HookReply {
    /// Keep the key from reaching other apps.
    pub swallow: bool,
    /// A binding fired; used to stop a held Win/Alt from opening the Start menu or a menu bar on release.
    pub fired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertResult {
    Pasted,
    LeftOnClipboard,
}

#[derive(Debug, Clone, Default)]
pub struct Foreground {
    /// Lowercased exe file name, e.g. "slack.exe".
    pub exe: String,
    pub title: String,
    pub hwnd: isize,
}
