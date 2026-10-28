//! Rules for pasting text into the focused app.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteCombo {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
}

/// Terminals ignore or misread Ctrl+V; use what each one expects.
pub fn paste_combo(exe: &str) -> PasteCombo {
    match exe.to_lowercase().as_str() {
        "windowsterminal.exe" | "wt.exe" | "alacritty.exe" | "wezterm-gui.exe" => PasteCombo::CtrlShiftV,
        "mintty.exe" | "conhost.exe" => PasteCombo::ShiftInsert,
        _ => PasteCombo::CtrlV,
    }
}

/// Simulating Ctrl+C to read a selection is unsafe where Ctrl+C means "interrupt" (terminals).
pub fn copy_fallback_allowed(exe: &str) -> bool {
    paste_combo(exe) == PasteCombo::CtrlV
}

/// Restore the user's clipboard only if the paste surely happened; otherwise the text stays
/// on the clipboard so the user can paste it by hand.
pub fn paste_restore_policy(send_ok: bool, fg_before: isize, fg_after: isize) -> bool {
    send_ok && fg_before == fg_after
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_combo_for_exe() {
        assert_eq!(paste_combo("WindowsTerminal.exe"), PasteCombo::CtrlShiftV);
        assert_eq!(paste_combo("alacritty.exe"), PasteCombo::CtrlShiftV);
        assert_eq!(paste_combo("mintty.exe"), PasteCombo::ShiftInsert);
        assert_eq!(paste_combo("conhost.exe"), PasteCombo::ShiftInsert);
        assert_eq!(paste_combo("slack.exe"), PasteCombo::CtrlV);
    }

    #[test]
    fn paste_restore_policy_cases() {
        assert!(paste_restore_policy(true, 1, 1));
        assert!(!paste_restore_policy(false, 1, 1), "SendInput reported failure");
        assert!(!paste_restore_policy(true, 1, 2), "focus moved during paste");
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn no_ctrl_c_in_terminals() {
        assert!(!copy_fallback_allowed("WindowsTerminal.exe"), "Ctrl+C would interrupt the running program");
        assert!(!copy_fallback_allowed("conhost.exe"));
        assert!(copy_fallback_allowed("notepad.exe"));
    }
}
