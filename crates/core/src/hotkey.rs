use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Variant order is the canonical display order ("Ctrl+Shift+Win+Alt+…").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Key {
    Ctrl,
    Shift,
    Win,
    Alt,
    Space,
    Escape,
    CapsLock,
    Char(char),
    F(u8),
    /// 1 = left, 2 = right, 3 = middle, 4/5 = side buttons.
    Mouse(u8),
    /// Any other Windows virtual-key code.
    Other(u16),
}

impl Key {
    /// Maps a Windows virtual-key code (left/right variants merged).
    pub fn from_vk(vk: u16) -> Key {
        match vk {
            0x10 | 0xA0 | 0xA1 => Key::Shift,
            0x11 | 0xA2 | 0xA3 => Key::Ctrl,
            0x12 | 0xA4 | 0xA5 => Key::Alt,
            0x5B | 0x5C => Key::Win,
            0x20 => Key::Space,
            0x1B => Key::Escape,
            0x14 => Key::CapsLock,
            0x30..=0x39 | 0x41..=0x5A => Key::Char(vk as u8 as char),
            0x70..=0x87 => Key::F((vk - 0x6F) as u8),
            _ => Key::Other(vk),
        }
    }

    /// The Windows virtual-key code (left variant for modifiers).
    pub fn to_vk(self) -> Option<u16> {
        Some(match self {
            Key::Shift => 0x10,
            Key::Ctrl => 0x11,
            Key::Alt => 0x12,
            Key::Win => 0x5B,
            Key::Space => 0x20,
            Key::Escape => 0x1B,
            Key::CapsLock => 0x14,
            Key::Char(c) => c as u16,
            Key::F(n) => 0x6F + n as u16,
            Key::Mouse(n) => *[0x01, 0x02, 0x04, 0x05, 0x06].get(n.checked_sub(1)? as usize)?,
            Key::Other(vk) => vk,
        })
    }

    pub fn is_modifier(self) -> bool {
        matches!(self, Key::Ctrl | Key::Shift | Key::Win | Key::Alt)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => write!(f, "{c}"),
            Key::F(n) => write!(f, "F{n}"),
            Key::Mouse(n) => write!(f, "Mouse{n}"),
            Key::Other(vk) => write!(f, "VK{vk}"),
            k => write!(f, "{k:?}"),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HotkeyError {
    #[error("unknown key: {0}")]
    UnknownKey(String),
    #[error("use 1 to 3 keys")]
    Length,
    #[error("add a modifier (Ctrl, Shift, Alt or Win)")]
    NeedsModifier,
    #[error("left and right mouse buttons can't be used")]
    MouseButton,
    #[error("reserved by Windows")]
    Reserved,
}

/// A set of keys pressed together; order does not matter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey(pub Vec<Key>);

impl Hotkey {
    pub fn parse(s: &str) -> Result<Hotkey, HotkeyError> {
        let mut keys = BTreeSet::new();
        for part in s.split('+').map(str::trim) {
            let lower = part.to_lowercase();
            let key = match lower.as_str() {
                "ctrl" | "control" => Key::Ctrl,
                "shift" => Key::Shift,
                "win" | "super" | "meta" | "cmd" => Key::Win,
                "alt" | "option" => Key::Alt,
                "space" => Key::Space,
                "escape" | "esc" => Key::Escape,
                "capslock" => Key::CapsLock,
                "middle" => Key::Mouse(3),
                _ => {
                    let num = |p: &str| lower.strip_prefix(p).and_then(|n| n.parse::<u16>().ok());
                    let mut chars = part.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() => {
                            Key::Char(c.to_ascii_uppercase())
                        }
                        _ => match (num("mouse"), num("f"), num("vk")) {
                            (Some(n), _, _) if (1..=5).contains(&n) => Key::Mouse(n as u8),
                            (_, Some(n), _) if (1..=24).contains(&n) => Key::F(n as u8),
                            (_, _, Some(vk)) => Key::Other(vk),
                            _ => return Err(HotkeyError::UnknownKey(part.to_string())),
                        },
                    }
                }
            };
            keys.insert(key);
        }
        Ok(Hotkey(keys.into_iter().collect()))
    }

    fn set(&self) -> BTreeSet<Key> {
        self.0.iter().copied().collect()
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.0.iter().map(Key::to_string).collect();
        f.write_str(&parts.join("+"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    PushToTalk,
    HandsFree,
    Command,
    Cancel,
    PasteLast,
    CopyLast,
    Scratchpad,
    Transform(i64),
}

/// Keys Windows reserves when pressed with Win and no other modifier.
const WIN_RESERVED: &[Key] = &[
    Key::Char('E'), Key::Char('R'), Key::Char('L'), Key::Char('D'), Key::Other(0x09 /*Tab*/), Key::Char('I'),
    Key::Char('S'), Key::Char('X'), Key::Char('P'), Key::Other(0x26 /*Up*/), Key::Other(0x28 /*Down*/), Key::Char('Q'),
];

pub fn validate(h: &Hotkey, action: Action) -> Result<(), HotkeyError> {
    let keys = h.set();
    if keys.is_empty() || keys.len() > 3 {
        return Err(HotkeyError::Length);
    }
    if keys.contains(&Key::Mouse(1)) || keys.contains(&Key::Mouse(2)) {
        return Err(HotkeyError::MouseButton);
    }
    let mods: Vec<Key> = keys.iter().copied().filter(|k| k.is_modifier()).collect();
    if mods == [Key::Win] && (keys.len() == 1 || keys.iter().any(|k| WIN_RESERVED.contains(k))) {
        return Err(HotkeyError::Reserved);
    }
    if keys.contains(&Key::Other(0x2E /*Delete*/)) && keys.contains(&Key::Ctrl) && keys.contains(&Key::Alt) {
        return Err(HotkeyError::Reserved);
    }
    if mods.is_empty() {
        let only = *keys.iter().next().unwrap();
        let allowed = keys.len() == 1
            && match only {
                Key::Mouse(_) => true,
                Key::CapsLock => matches!(action, Action::PushToTalk | Action::HandsFree),
                Key::Escape => action == Action::Cancel,
                _ => false,
            };
        if !allowed {
            return Err(HotkeyError::NeedsModifier);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    PushToTalk,
    HandsFree,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Start(Mode),
    Stop,
    Cancel,
    Fire(Action),
}

#[derive(Debug, Default, PartialEq)]
pub struct MatchResult {
    pub events: Vec<Trigger>,
    pub swallow: bool,
}

/// Presses shorter than this count as taps (a tap cancels; two taps = hands-free).
const TAP_MS: u64 = 300;

#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    /// Released a short push-to-talk tap at `at`; a second press soon after enters hands-free.
    TapWait { at: u64 },
    /// Hold-to-talk with `keys`, started at `since`.
    Held { mode: Mode, keys: BTreeSet<Key>, since: u64 },
    /// Hands-free; ignores releases until the next full press of a talk binding.
    HandsFree,
    /// Just stopped or cancelled by a key press: ignore the rest of that chord until all keys are up,
    /// so finishing it (e.g. the Space of Ctrl+Win+Space) doesn't start a new recording.
    WaitRelease,
}

pub struct HotkeyMatcher {
    bindings: Vec<(BTreeSet<Key>, Action)>,
    double_tap_ms: u64,
    held: BTreeSet<Key>,
    swallowed: BTreeSet<Key>,
    state: State,
}

impl HotkeyMatcher {
    pub fn new(bindings: Vec<(Hotkey, Action)>, double_tap_ms: u64) -> Self {
        Self {
            bindings: bindings.iter().map(|(h, a)| (h.set(), *a)).collect(),
            double_tap_ms,
            held: BTreeSet::new(),
            swallowed: BTreeSet::new(),
            state: State::Idle,
        }
    }

    /// Replaces the bindings without resetting held keys or a running recording.
    pub fn set_bindings(&mut self, bindings: Vec<(Hotkey, Action)>) {
        self.bindings = bindings.iter().map(|(h, a)| (h.set(), *a)).collect();
    }

    /// Drops held keys for which `still_down` is false. Key-ups that happen on the secure
    /// desktop (Ctrl+Alt+Del, Win+L, UAC) never reach the hook and would otherwise stick.
    pub fn retain_held(&mut self, still_down: impl Fn(Key) -> bool) {
        self.held.retain(|k| still_down(*k));
        self.swallowed.retain(|k| still_down(*k));
    }

    fn matching(&self) -> Option<Action> {
        self.bindings.iter().find(|(k, _)| *k == self.held).map(|(_, a)| *a)
    }

    pub fn on_event(&mut self, key: Key, down: bool, now_ms: u64) -> MatchResult {
        if down {
            if self.state == State::WaitRelease {
                self.held.insert(key);
                let swallow = !key.is_modifier() && self.matching().is_some();
                if swallow {
                    self.swallowed.insert(key);
                }
                return MatchResult { events: vec![], swallow };
            }
            if !self.held.insert(key) {
                // Auto-repeat of a held key.
                return MatchResult { events: vec![], swallow: self.swallowed.contains(&key) };
            }
            let events = self.on_down(now_ms);
            // Only keys that complete a binding are hidden from apps; a shortcut typed over a
            // hold-to-talk chord cancels the recording but still reaches Windows.
            let swallow = !events.is_empty() && !key.is_modifier() && self.matching().is_some();
            if swallow {
                self.swallowed.insert(key);
            }
            MatchResult { events, swallow }
        } else {
            self.held.remove(&key);
            let swallow = self.swallowed.remove(&key);
            if self.state == State::WaitRelease {
                if self.held.is_empty() {
                    self.state = State::Idle;
                }
                return MatchResult { events: vec![], swallow };
            }
            MatchResult { events: self.on_up(key, now_ms), swallow }
        }
    }

    fn on_down(&mut self, now: u64) -> Vec<Trigger> {
        use Trigger::*;
        let action = self.matching();
        match (self.state.clone(), action) {
            (State::Idle | State::TapWait { .. }, Some(Action::PushToTalk)) => {
                if let State::TapWait { at } = self.state
                    && now.saturating_sub(at) <= self.double_tap_ms
                {
                    self.state = State::HandsFree;
                    return vec![Start(Mode::HandsFree)];
                }
                self.state = State::Held { mode: Mode::PushToTalk, keys: self.held.clone(), since: now };
                vec![Start(Mode::PushToTalk)]
            }
            (State::Idle | State::TapWait { .. }, Some(Action::HandsFree)) => {
                self.state = State::HandsFree;
                vec![Start(Mode::HandsFree)]
            }
            (State::Idle | State::TapWait { .. }, Some(Action::Command)) => {
                self.state = State::Held { mode: Mode::Command, keys: self.held.clone(), since: now };
                vec![Start(Mode::Command)]
            }
            (State::Idle | State::TapWait { .. }, Some(Action::Cancel)) | (_, None) if !self.recording() => vec![],
            (State::Idle | State::TapWait { .. }, Some(a)) => {
                self.state = State::Idle;
                vec![Fire(a)]
            }
            (_, Some(Action::Cancel)) => {
                self.state = State::WaitRelease;
                vec![Cancel]
            }
            // The push-to-talk chord grew into a longer binding: restart in that mode.
            (State::Held { mode: Mode::PushToTalk, .. }, Some(Action::HandsFree)) => {
                self.state = State::HandsFree;
                vec![Cancel, Start(Mode::HandsFree)]
            }
            (State::Held { mode: Mode::PushToTalk, .. }, Some(Action::Command)) => {
                self.state = State::Held { mode: Mode::Command, keys: self.held.clone(), since: now };
                vec![Cancel, Start(Mode::Command)]
            }
            (State::HandsFree, Some(Action::PushToTalk | Action::HandsFree)) => {
                self.state = State::WaitRelease;
                vec![Stop]
            }
            // Another shortcut is being typed over a hold-to-talk chord.
            (State::Held { .. }, _) => {
                self.state = State::WaitRelease;
                vec![Cancel]
            }
            _ => vec![],
        }
    }

    fn on_up(&mut self, key: Key, now: u64) -> Vec<Trigger> {
        match &self.state {
            State::Held { mode, keys, since } if keys.contains(&key) => {
                let tap = now.saturating_sub(*since) < TAP_MS;
                let mode = *mode;
                self.state = if tap && mode == Mode::PushToTalk { State::TapWait { at: now } } else { State::Idle };
                vec![if tap { Trigger::Cancel } else { Trigger::Stop }]
            }
            _ => vec![],
        }
    }

    fn recording(&self) -> bool {
        matches!(self.state, State::Held { .. } | State::HandsFree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Key::*;

    fn hk(s: &str) -> Hotkey {
        Hotkey::parse(s).unwrap()
    }

    fn matcher() -> HotkeyMatcher {
        HotkeyMatcher::new(
            vec![
                (hk("Ctrl+Win"), Action::PushToTalk),
                (hk("Ctrl+Win+Space"), Action::HandsFree),
                (hk("Ctrl+Win+Alt"), Action::Command),
                (hk("Escape"), Action::Cancel),
                (hk("Shift+Alt+Z"), Action::PasteLast),
                (hk("Win+Alt+1"), Action::Transform(7)),
                (hk("Mouse4"), Action::PushToTalk),
            ],
            500,
        )
    }

    /// Feeds (key, down, time) events and collects all triggers.
    fn run(m: &mut HotkeyMatcher, evs: &[(Key, bool, u64)]) -> Vec<Trigger> {
        evs.iter().flat_map(|&(k, d, t)| m.on_event(k, d, t).events).collect()
    }

    #[test]
    fn parse_display_roundtrip() {
        for s in ["Ctrl+Win", "Ctrl+Win+Space", "Shift+Alt+Z", "Win+Alt+S", "Ctrl+Win+Alt", "Escape", "Mouse4", "Ctrl+F5"] {
            assert_eq!(hk(s).to_string(), s);
        }
        assert_eq!(hk("space + win + control").to_string(), "Ctrl+Win+Space");
        assert_eq!(hk("alt+shift+z"), hk("Shift+Alt+Z"));
        assert!(Hotkey::parse("Ctrl+Banana").is_err());
        assert!(Hotkey::parse("").is_err());
    }

    #[test]
    fn validate_rejects_reserved() {
        for s in ["Win+L", "Win+D", "Win", "Ctrl+Alt+Delete", "A", "Mouse1", "Mouse2", "Ctrl+Shift+Alt+Win"] {
            assert!(validate(&Hotkey::parse(s).unwrap_or(Hotkey(vec![Mouse(1)])), Action::PushToTalk).is_err(), "{s}");
        }
        assert!(validate(&hk("Win+Alt+L"), Action::Scratchpad).is_ok());
        assert!(validate(&hk("Ctrl+Win"), Action::PushToTalk).is_ok());
        assert!(validate(&hk("Mouse5"), Action::PushToTalk).is_ok());
    }

    #[test]
    fn validate_allows_capslock_for_ptt_only() {
        assert!(validate(&hk("CapsLock"), Action::PushToTalk).is_ok());
        assert!(validate(&hk("CapsLock"), Action::HandsFree).is_ok());
        assert!(validate(&hk("CapsLock"), Action::PasteLast).is_err());
        assert!(validate(&hk("Escape"), Action::Cancel).is_ok());
        assert!(validate(&hk("Escape"), Action::PushToTalk).is_err());
    }

    #[test]
    fn ptt_hold_release() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Win, false, 2000), (Ctrl, false, 2010)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Stop]);
    }

    #[test]
    fn key_repeat_is_ignored() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Win, true, 40), (Ctrl, true, 50), (Win, false, 2000)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Stop]);
    }

    #[test]
    fn longer_binding_wins() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Space, true, 60)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Cancel, Trigger::Start(Mode::HandsFree)]);
        // Releasing the chord keeps hands-free running; the next full press stops it.
        let t = run(&mut m, &[(Space, false, 100), (Win, false, 110), (Ctrl, false, 120), (Ctrl, true, 5000), (Win, true, 5010)]);
        assert_eq!(t, vec![Trigger::Stop]);
    }

    #[test]
    fn command_chord_upgrades_from_ptt() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Alt, true, 50), (Alt, false, 3000)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Cancel, Trigger::Start(Mode::Command), Trigger::Stop]);
    }

    #[test]
    fn unrelated_key_during_ptt_cancels() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Char('D'), true, 80)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Cancel]);
    }

    #[test]
    fn double_tap_enters_hands_free_and_next_press_stops() {
        let mut m = matcher();
        let t = run(
            &mut m,
            &[
                (Ctrl, true, 0), (Win, true, 10), (Win, false, 100), (Ctrl, false, 110),
                (Ctrl, true, 300), (Win, true, 310), (Win, false, 400), (Ctrl, false, 410),
            ],
        );
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Cancel, Trigger::Start(Mode::HandsFree)]);
        let t = run(&mut m, &[(Ctrl, true, 9000), (Win, true, 9010), (Win, false, 9100)]);
        assert_eq!(t, vec![Trigger::Stop]);
    }

    #[test]
    fn short_tap_cancels() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Win, false, 100), (Ctrl, false, 110)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk), Trigger::Cancel]);
        // A second press after the window is a fresh push-to-talk.
        let t = run(&mut m, &[(Ctrl, true, 1000), (Win, true, 1010)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk)]);
    }

    #[test]
    fn hands_free_binding_toggles() {
        let mut m = matcher();
        let t = run(&mut m, &[(Ctrl, true, 0), (Win, true, 5), (Space, true, 10), (Space, false, 50)]);
        assert_eq!(t.last(), Some(&Trigger::Start(Mode::HandsFree)));
        let t = run(&mut m, &[(Space, true, 3000)]); // Ctrl+Win still held
        assert_eq!(t, vec![Trigger::Stop]);
    }

    #[test]
    fn escape_cancels_only_while_recording() {
        let mut m = matcher();
        let r = m.on_event(Escape, true, 0);
        assert!(r.events.is_empty() && !r.swallow);
        m.on_event(Escape, false, 1);
        run(&mut m, &[(Ctrl, true, 10), (Win, true, 20), (Space, true, 30), (Space, false, 40), (Win, false, 50), (Ctrl, false, 60)]);
        let r = m.on_event(Escape, true, 2000);
        assert_eq!(r.events, vec![Trigger::Cancel]);
        assert!(r.swallow);
        assert!(m.on_event(Escape, false, 2010).swallow);
    }

    #[test]
    fn mouse_button_swallowed() {
        let mut m = matcher();
        let r = m.on_event(Mouse(4), true, 0);
        assert_eq!(r.events, vec![Trigger::Start(Mode::PushToTalk)]);
        assert!(r.swallow);
        let r = m.on_event(Mouse(4), false, 2000);
        assert_eq!(r.events, vec![Trigger::Stop]);
        assert!(r.swallow);
        assert!(!m.on_event(Mouse(5), true, 3000).swallow);
    }

    #[test]
    fn transform_fires() {
        let mut m = matcher();
        let t = run(&mut m, &[(Win, true, 0), (Alt, true, 10)]);
        assert!(t.is_empty());
        let r = m.on_event(Char('1'), true, 20);
        assert_eq!(r.events, vec![Trigger::Fire(Action::Transform(7))]);
        assert!(r.swallow);
        let r = m.on_event(Char('1'), false, 30);
        assert!(r.events.is_empty() && r.swallow);
    }

    #[test]
    fn modifiers_never_swallowed() {
        let mut m = matcher();
        assert!(!m.on_event(Ctrl, true, 0).swallow);
        assert!(!m.on_event(Win, true, 10).swallow);
    }
}

#[cfg(test)]
mod vk_tests {
    use super::*;

    #[test]
    fn vk_to_key_mapping() {
        assert_eq!(Key::from_vk(0x5B), Key::Win); // VK_LWIN
        assert_eq!(Key::from_vk(0x5C), Key::Win); // VK_RWIN
        assert_eq!(Key::from_vk(0xA2), Key::Ctrl); // VK_LCONTROL
        assert_eq!(Key::from_vk(0x11), Key::Ctrl); // VK_CONTROL
        assert_eq!(Key::from_vk(0xA5), Key::Alt); // VK_RMENU
        assert_eq!(Key::from_vk(0xA0), Key::Shift);
        assert_eq!(Key::from_vk(0x20), Key::Space);
        assert_eq!(Key::from_vk(0x1B), Key::Escape);
        assert_eq!(Key::from_vk(0x14), Key::CapsLock);
        assert_eq!(Key::from_vk(0x41), Key::Char('A'));
        assert_eq!(Key::from_vk(0x31), Key::Char('1'));
        assert_eq!(Key::from_vk(0x74), Key::F(5));
        assert_eq!(Key::from_vk(0x2E), Key::Other(0x2E));
    }

    #[test]
    fn vk_keys_match_parsed_hotkeys() {
        let h = Hotkey::parse("Win+Alt+1").unwrap();
        let mut v = vec![Key::from_vk(0x5B), Key::from_vk(0xA4), Key::from_vk(0x31)];
        v.sort();
        assert_eq!(h.0, v);
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use Key::*;

    fn m() -> HotkeyMatcher {
        HotkeyMatcher::new(
            vec![
                (Hotkey::parse("Ctrl+Win").unwrap(), Action::PushToTalk),
                (Hotkey::parse("Ctrl+Win+Space").unwrap(), Action::HandsFree),
                (Hotkey::parse("Escape").unwrap(), Action::Cancel),
            ],
            500,
        )
    }

    #[test]
    fn shortcut_typed_over_ptt_cancels_but_passes_through() {
        let mut m = m();
        m.on_event(Ctrl, true, 0);
        m.on_event(Win, true, 10);
        let r = m.on_event(Char('D'), true, 80);
        assert_eq!(r.events, vec![Trigger::Cancel]);
        assert!(!r.swallow, "Ctrl+Win+D must still reach Windows");
        assert!(!m.on_event(Char('D'), false, 90).swallow);
    }

    #[test]
    fn stale_held_keys_can_be_dropped() {
        let mut m = m();
        // Ctrl+Alt+Del: key-ups happen on the secure desktop and never arrive.
        m.on_event(Ctrl, true, 0);
        m.on_event(Alt, true, 5);
        m.on_event(Other(0x2E), true, 10);
        m.retain_held(|_| false);
        let t: Vec<_> = [(Ctrl, true, 1000), (Win, true, 1010)].iter().flat_map(|&(k, d, t)| m.on_event(k, d, t).events).collect();
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk)]);
    }

    #[test]
    fn rebinding_keeps_recording_state() {
        let mut m = m();
        for (k, t) in [(Ctrl, 0), (Win, 5), (Space, 10)] {
            m.on_event(k, true, t);
        }
        for (k, t) in [(Space, 20), (Win, 30), (Ctrl, 40)] {
            m.on_event(k, false, t);
        }
        m.set_bindings(vec![
            (Hotkey::parse("Ctrl+Win").unwrap(), Action::PushToTalk),
            (Hotkey::parse("Ctrl+Win+Space").unwrap(), Action::HandsFree),
        ]);
        let t: Vec<_> = [(Ctrl, true, 5000), (Win, true, 5010)].iter().flat_map(|&(k, d, t)| m.on_event(k, d, t).events).collect();
        assert_eq!(t, vec![Trigger::Stop], "hands-free survives a settings change");
    }

    #[test]
    fn to_vk_roundtrips() {
        for k in [Win, Ctrl, Alt, Shift, Space, Escape, CapsLock, Char('A'), Char('7'), F(5), Other(0x2E)] {
            assert_eq!(Key::from_vk(k.to_vk().unwrap()), k);
        }
        assert_eq!(Mouse(3).to_vk(), Some(0x04));
        assert_eq!(Mouse(4).to_vk(), Some(0x05));
    }
}

#[cfg(test)]
mod stop_tests {
    use super::*;
    use Key::*;

    fn m() -> HotkeyMatcher {
        HotkeyMatcher::new(
            vec![
                (Hotkey::parse("Ctrl+Win").unwrap(), Action::PushToTalk),
                (Hotkey::parse("Ctrl+Win+Space").unwrap(), Action::HandsFree),
            ],
            500,
        )
    }

    fn run(m: &mut HotkeyMatcher, evs: &[(Key, bool, u64)]) -> Vec<Trigger> {
        evs.iter().flat_map(|&(k, d, t)| m.on_event(k, d, t).events).collect()
    }

    #[test]
    fn hands_free_stop_with_same_chord_does_not_restart() {
        let mut m = m();
        run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Space, true, 20), (Space, false, 80), (Win, false, 90), (Ctrl, false, 100)]);
        let t = run(&mut m, &[(Ctrl, true, 5000), (Win, true, 5010), (Space, true, 5020), (Space, false, 5080), (Win, false, 5090), (Ctrl, false, 5100)]);
        assert_eq!(t, vec![Trigger::Stop]);
        // After everything is released, the next press starts normally.
        let t = run(&mut m, &[(Ctrl, true, 9000), (Win, true, 9010)]);
        assert_eq!(t, vec![Trigger::Start(Mode::PushToTalk)]);
    }

    #[test]
    fn stopping_swallows_the_rest_of_the_chord() {
        let mut m = m();
        run(&mut m, &[(Ctrl, true, 0), (Win, true, 10), (Space, true, 20), (Space, false, 80), (Win, false, 90), (Ctrl, false, 100)]);
        run(&mut m, &[(Ctrl, true, 5000), (Win, true, 5010)]);
        let r = m.on_event(Space, true, 5020);
        assert!(r.events.is_empty());
        assert!(r.swallow, "the Space that finishes the stop chord must not type a space");
        assert!(m.on_event(Space, false, 5030).swallow);
    }
}
