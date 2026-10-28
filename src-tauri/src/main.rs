#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod dictation;
mod llm;
mod platform;
mod stt;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, PhysicalPosition};

use dictation::{BarState, Shared};

const BAR_WIDTH: f64 = 220.0;
const BAR_HEIGHT: f64 = 48.0;
const BAR_BOTTOM_GAP: f64 = 80.0;

static PENDING_BAR_POS: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn place_overlay(app: &tauri::AppHandle, shared: &Shared) -> tauri::Result<()> {
    let Some(w) = app.get_webview_window("overlay") else { return Ok(()) };
    platform::set_no_activate(&w).ok();
    let pos = match shared.settings().bar_position {
        Some((x, y)) => PhysicalPosition::new(x, y),
        None => {
            let Some(m) = w.primary_monitor()? else { return Ok(()) };
            let scale = m.scale_factor();
            let (mp, ms) = (m.position(), m.size());
            PhysicalPosition::new(
                mp.x + ((ms.width as f64 - BAR_WIDTH * scale) / 2.0) as i32,
                mp.y + (ms.height as f64 - (BAR_HEIGHT + BAR_BOTTOM_GAP) * scale) as i32,
            )
        }
    };
    w.set_position(pos)?;
    shared.bar(BarState::Idle);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let shared = dictation::start(app.handle().clone(), data_dir)?;
            app.manage(shared.clone());
            // Windows are created only now: their pages call commands that need the managed state.
            for w in &app.config().app.windows {
                tauri::WebviewWindowBuilder::from_config(app.handle(), w)?.build()?;
            }
            place_overlay(app.handle(), &shared)?;

            let open = MenuItem::with_id(app, "open", "Open blablaflow", true, None::<&str>)?;
            let pause = CheckMenuItem::with_id(app, "pause", "Pause dictation", true, false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &pause, &quit])?;
            let tray_shared = shared.clone();
            let pause_item = pause.clone();
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("blablaflow")
                .menu(&menu)
                .on_menu_event(move |app, e| match e.id.as_ref() {
                    "open" => show_main(app),
                    "pause" => {
                        let paused = pause_item.is_checked().unwrap_or(false);
                        tray_shared.paused.store(paused, Ordering::SeqCst);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            if !shared.settings().onboarded {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the main window hides it; the app lives in the tray.
            tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            tauri::WindowEvent::Moved(pos) if window.label() == "overlay" => {
                // Moved fires continuously while dragging: remember the latest and save once it settles.
                let first = PENDING_BAR_POS.lock().unwrap().replace((pos.x, pos.y)).is_none();
                if first {
                    let shared = window.state::<Arc<Shared>>().inner().clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let Some(p) = PENDING_BAR_POS.lock().unwrap().take() else { return };
                        let mut st = shared.settings();
                        if st.bar_position != Some(p) {
                            st.bar_position = Some(p);
                            let _ = shared.save_settings(st);
                        }
                    });
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_mics,
            commands::list_models_local,
            commands::download_model,
            commands::delete_model,
            commands::list_gpus,
            commands::groq_key_set,
            commands::set_groq_key,
            commands::list_groq_models,
            commands::test_groq,
            commands::history_list,
            commands::history_delete,
            commands::history_retry,
            commands::history_copy,
            commands::insights,
            commands::dict_list,
            commands::dict_add,
            commands::dict_delete,
            commands::snippets_list,
            commands::snippet_add,
            commands::snippet_update,
            commands::snippet_delete,
            commands::transforms_list,
            commands::transform_save,
            commands::transform_delete,
            commands::notes_list,
            commands::note_save,
            commands::note_delete,
            commands::export_items,
            commands::import_items,
            commands::validate_hotkey,
            commands::set_paused,
            commands::hide_bar_for,
            commands::set_language,
            commands::show_main,
        ])
        .run(tauri::generate_context!())
        .expect("error while running blablaflow");
}
