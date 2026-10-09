#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod dsp;

fn main() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::nvidia_status,
            commands::install_vbcable,
            commands::start_engine,
            commands::stop_engine,
            commands::set_chain,
            commands::set_mute,
            commands::list_presets,
            commands::save_preset,
            commands::load_preset,
            commands::delete_preset,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Better Mic");
}
