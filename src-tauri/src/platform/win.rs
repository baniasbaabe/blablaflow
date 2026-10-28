use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use blablaflow_core::hotkey::Key;
use blablaflow_core::insert::{PasteCombo, paste_combo, paste_restore_policy};
use uiautomation::UIAutomation;
use uiautomation::patterns::{UITextPattern, UIValuePattern};
use uiautomation::types::TextPatternRangeEndpoint as End;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VIRTUAL_KEY,
    VK_CONTROL, VK_INSERT, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GWL_EXSTYLE, GetForegroundWindow, GetMessageW, GetWindowLongPtrW, GetWindowTextW,
    GetWindowThreadProcessId, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLMHF_INJECTED, MSG, MSLLHOOKSTRUCT, SetWindowLongPtrW,
    SetWindowsHookExW, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_SYSKEYDOWN,
    WM_XBUTTONDOWN, WM_XBUTTONUP, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{HSTRING, PWSTR};

use super::{Foreground, HookReply, InsertResult};

type HookFn = Box<dyn Fn(Key, bool) -> HookReply + Send + Sync>;
static HOOK: OnceLock<HookFn> = OnceLock::new();
/// A binding fired while Win/Alt was held; mask their release (see `mask_release`).
static WIN_USED: AtomicBool = AtomicBool::new(false);
static ALT_USED: AtomicBool = AtomicBool::new(false);
/// Unassigned virtual key: tapping it makes Windows treat Win/Alt as part of a chord.
const VK_MASK: u16 = 0xE8;

fn key_input(vk: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Presses `keys` in order and releases them in reverse. Returns true if every event was injected.
fn send_chord(keys: &[u16]) -> bool {
    let mut inputs: Vec<INPUT> = keys.iter().map(|k| key_input(*k, false)).collect();
    inputs.extend(keys.iter().rev().map(|k| key_input(*k, true)));
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    sent as usize == inputs.len()
}

fn mask_release(key: Key) {
    let flag = match key {
        Key::Win => &WIN_USED,
        Key::Alt => &ALT_USED,
        _ => return,
    };
    if flag.swap(false, Ordering::SeqCst) {
        send_chord(&[VK_MASK]);
    }
}

fn dispatch(key: Key, down: bool) -> bool {
    let Some(cb) = HOOK.get() else { return false };
    if !down {
        mask_release(key);
    }
    let reply = cb(key, down);
    if reply.fired {
        let held = |vk: VIRTUAL_KEY| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0;
        if held(VK_LWIN) || held(VK_RWIN) {
            WIN_USED.store(true, Ordering::SeqCst);
        }
        if held(VK_MENU) {
            ALT_USED.store(true, Ordering::SeqCst);
        }
    }
    reply.swallow
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if !k.flags.contains(LLKHF_INJECTED) {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            if dispatch(Key::from_vk(k.vkCode as u16), down) {
                return LRESULT(1);
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let m = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        let msg = wparam.0 as u32;
        let button = match msg {
            WM_MBUTTONDOWN | WM_MBUTTONUP => Some(3),
            WM_XBUTTONDOWN | WM_XBUTTONUP => Some(if (m.mouseData >> 16) == 1 { 4 } else { 5 }),
            _ => None,
        };
        if let Some(b) = button
            && m.flags & LLMHF_INJECTED == 0
            && dispatch(Key::Mouse(b), matches!(msg, WM_MBUTTONDOWN | WM_XBUTTONDOWN))
        {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Installs global keyboard and mouse hooks on a dedicated thread. `on_event` must return fast (< 1 ms).
pub fn start_hooks(on_event: impl Fn(Key, bool) -> HookReply + Send + Sync + 'static) -> Result<()> {
    HOOK.set(Box::new(on_event)).map_err(|_| anyhow!("hooks already started"))?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("hooks".into()).spawn(move || unsafe {
        let module = GetModuleHandleW(None).ok().map(|m| m.into());
        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
        let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
        let _ = tx.send(kb.and(ms).map(|_| ()).map_err(|e| anyhow!("installing hooks: {e}")));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    })?;
    rx.recv()?
}

pub fn foreground() -> Foreground {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe = String::new();
        if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut path = [0u16; 1024];
            let mut len = path.len() as u32;
            if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut len).is_ok() {
                let full = String::from_utf16_lossy(&path[..len as usize]);
                exe = full.rsplit('\\').next().unwrap_or_default().to_lowercase();
            }
            let _ = windows::Win32::Foundation::CloseHandle(h);
        }
        Foreground { exe, title, hwnd: hwnd.0 as isize }
    }
}

/// Runs `f` on its own thread and gives up after `timeout` (UIA calls can hang on busy apps).
fn with_timeout<T: Send + 'static>(timeout: Duration, f: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(timeout).ok().flatten()
}

/// Text around the cursor in the focused field; never reads password fields.
pub fn read_context(max_chars: usize) -> Option<String> {
    with_timeout(Duration::from_millis(200), move || {
        let uia = UIAutomation::new().ok()?;
        let el = uia.get_focused_element().ok()?;
        if el.is_password().unwrap_or(true) {
            return None;
        }
        let half = max_chars / 2;
        if let Ok(tp) = el.get_pattern::<UITextPattern>() {
            let sel = tp.get_selection().ok()?.into_iter().next()?;
            let before = tp.get_document_range().ok()?;
            before.move_endpoint_by_range(End::End, &sel, End::Start).ok()?;
            let after = tp.get_document_range().ok()?;
            after.move_endpoint_by_range(End::Start, &sel, End::End).ok()?;
            let b = before.get_text(-1).ok()?;
            let a = after.get_text(half as i32).ok()?;
            let b: String = b.chars().rev().take(half).collect::<Vec<_>>().into_iter().rev().collect();
            let text = format!("{b}[cursor]{a}");
            return (text.len() > "[cursor]".len()).then_some(text);
        }
        let v = el.get_pattern::<UIValuePattern>().ok()?.get_value().ok()?;
        let v: String = v.chars().rev().take(max_chars).collect::<Vec<_>>().into_iter().rev().collect();
        (!v.trim().is_empty()).then_some(v)
    })
}

/// Waits (up to 1 s) for the user to let go of modifiers so our paste isn't read as e.g. Win+Ctrl+V.
fn wait_modifiers_released() {
    let deadline = Instant::now() + Duration::from_secs(1);
    let held = || {
        [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
    };
    while held() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Selected text: UI Automation first, then a simulated Ctrl+C with the clipboard restored.
pub fn read_selection() -> Option<String> {
    let uia = with_timeout(Duration::from_millis(300), || {
        let el = UIAutomation::new().ok()?.get_focused_element().ok()?;
        let tp = el.get_pattern::<UITextPattern>().ok()?;
        let text = tp.get_selection().ok()?.into_iter().next()?.get_text(-1).ok()?;
        (!text.is_empty()).then_some(text)
    });
    if uia.is_some() {
        return uia;
    }
    wait_modifiers_released();
    let mut cb = arboard::Clipboard::new().ok()?;
    let prev = cb.get_text().ok();
    let seq = unsafe { GetClipboardSequenceNumber() };
    send_chord(&[VK_CONTROL.0, b'C' as u16]);
    let deadline = Instant::now() + Duration::from_millis(150);
    while unsafe { GetClipboardSequenceNumber() } == seq && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if unsafe { GetClipboardSequenceNumber() } == seq {
        return None;
    }
    let text = cb.get_text().ok();
    if let Some(p) = prev {
        let _ = cb.set_text(p);
    }
    text.filter(|t| !t.is_empty())
}

/// Pastes `text` into the focused app and restores the previous clipboard text.
/// shortcut: only text clipboard content is restored (images/files are lost); enumerate formats to keep them.
pub fn insert_text(text: &str, exe: &str) -> Result<InsertResult> {
    wait_modifiers_released();
    let mut cb = arboard::Clipboard::new()?;
    let prev = cb.get_text().ok();
    cb.set_text(text)?;
    let before = unsafe { GetForegroundWindow() }.0 as isize;
    let keys: &[u16] = match paste_combo(exe) {
        PasteCombo::CtrlV => &[VK_CONTROL.0, b'V' as u16],
        PasteCombo::CtrlShiftV => &[VK_CONTROL.0, VK_SHIFT.0, b'V' as u16],
        PasteCombo::ShiftInsert => &[VK_SHIFT.0, VK_INSERT.0],
    };
    let sent = send_chord(keys);
    // The target app reads the clipboard asynchronously; give it time before restoring.
    std::thread::sleep(Duration::from_millis(150));
    let after = unsafe { GetForegroundWindow() }.0 as isize;
    if !paste_restore_policy(sent, before, after) {
        return Ok(InsertResult::LeftOnClipboard);
    }
    match prev {
        Some(p) => cb.set_text(p)?,
        None => cb.clear()?,
    }
    Ok(InsertResult::Pasted)
}

/// Copies text to the clipboard without pasting.
pub fn copy_text(text: &str) -> Result<()> {
    arboard::Clipboard::new()?.set_text(text)?;
    Ok(())
}

/// Keeps a window (the Flow bar) from ever taking focus or showing in Alt+Tab.
pub fn set_no_activate(window: &tauri::WebviewWindow) -> Result<()> {
    let hwnd = HWND(window.hwnd()?.0);
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize);
    }
    Ok(())
}

pub fn free_space(dir: &Path) -> Option<u64> {
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(dir), Some(&mut free), None, None) }.ok()?;
    Some(free)
}


/// Shows a window without activating it, so the focused app keeps focus.
pub fn show_no_activate(window: &tauri::WebviewWindow) {
    if let Ok(h) = window.hwnd() {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(HWND(h.0), windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE);
        }
    }
}
