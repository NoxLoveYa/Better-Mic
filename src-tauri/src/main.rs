#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod dsp;

use std::sync::atomic::Ordering;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main() {
    let builder = tauri::Builder::default();
    // A relaunch while the app sits in the tray should surface that window, not start a second engine.
    // Debug builds skip this so `tauri dev` still works while an installed copy is running.
    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)));

    builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![commands::AUTOSTART_ARG])))
        .manage(commands::AppState::default())
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show Better Mic", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Better Mic")
                .menu(&Menu::with_items(app, &[&show, &quit])?)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // The window starts hidden; a login launch stays in the tray, a manual one shows it.
            if !commands::launched_at_startup() {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<commands::AppState>().close_to_tray.load(Ordering::Relaxed) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::nvidia_status,
            commands::install_vbcable,
            commands::get_autostart,
            commands::set_autostart,
            commands::launched_at_startup,
            commands::set_close_to_tray,
            commands::start_engine,
            commands::stop_engine,
            commands::set_chain,
            commands::set_preview,
            commands::set_mute,
            commands::list_presets,
            commands::save_preset,
            commands::load_preset,
            commands::delete_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Better Mic");
}
