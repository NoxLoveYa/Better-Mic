use crate::audio::{self, engine, engine::EngineHandle, Devices};
use crate::dsp::{self, FilterCfg};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

/// Passed to the app by the Windows startup entry, so a login launch can be told apart from a manual one.
pub const AUTOSTART_ARG: &str = "--autostarted";

#[derive(Default)]
pub struct AppState {
    engine: Mutex<Option<EngineHandle>>,
    mute: Arc<AtomicBool>,
    pub close_to_tray: AtomicBool,
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let res = if on {
        launcher.enable()
    } else if launcher.is_enabled().unwrap_or(false) {
        launcher.disable()
    } else {
        Ok(())
    };
    res.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launched_at_startup() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}

#[tauri::command]
pub fn set_close_to_tray(state: State<AppState>, on: bool) {
    state.close_to_tray.store(on, Ordering::Relaxed);
}

#[tauri::command]
pub fn list_devices() -> Result<Devices, String> {
    audio::list_devices()
}

const INSTALL_VBCABLE: &str = r#"
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$dir = Join-Path $env:TEMP 'better-mic-vbcable'
Remove-Item $dir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $dir | Out-Null
$zip = Join-Path $dir 'vbcable.zip'
Invoke-WebRequest 'https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack45.zip' -OutFile $zip -UseBasicParsing
Expand-Archive $zip -DestinationPath $dir -Force
$pat = if ([Environment]::Is64BitOperatingSystem) { '*x64*' } else { 'VBCABLE_Setup.exe' }
$exe = Get-ChildItem $dir -Filter 'VBCABLE_Setup*.exe' | Where-Object { $_.Name -like $pat } | Select-Object -First 1
if (-not $exe) { throw 'Installer not found in the downloaded archive.' }
Start-Process $exe.FullName -Verb RunAs -Wait
"#;

/// Downloads the official VB-Cable driver pack and runs its setup elevated (UAC prompt, then VB's own installer UI).
#[tauri::command]
pub async fn install_vbcable() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", INSTALL_VBCABLE])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("VB-Cable install failed").to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn nvidia_status() -> dsp::NvidiaStatus {
    dsp::nvidia_probe()
}

/// `nvidia-smi` can take a second or so, so keep it off the main thread.
#[tauri::command]
pub async fn nvidia_gpu() -> Option<dsp::NvidiaGpu> {
    tauri::async_runtime::spawn_blocking(dsp::nvidia_gpu).await.ok().flatten()
}

#[derive(serde::Serialize, Clone)]
struct InstallProgress {
    stage: String,
    done: u64,
    total: u64,
}

const INSTALL_NVIDIA: &str = include_str!("install_nvidia.ps1");

/// Downloads NVIDIA's Audio Effects redistributable for `arch`, checks its signature and runs it elevated.
/// Progress is reported through `nvidia-install` events.
#[tauri::command]
pub async fn install_nvidia_sdk(app: AppHandle, arch: String) -> Result<(), String> {
    use std::io::{BufRead, BufReader};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use tauri::Emitter;

    let url = dsp::nvidia_installer_url(&arch).ok_or("Unknown GPU generation.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &INSTALL_NVIDIA.replace("{URL}", &url)])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;

        let emit = |stage: &str, done: u64, total: u64| {
            let _ = app.emit("nvidia-install", InstallProgress { stage: stage.into(), done, total });
        };
        let mut error = None;
        for line in BufReader::new(child.stdout.take().ok_or("no installer output")?).lines().map_while(Result::ok) {
            match line.trim().split_once(' ') {
                Some(("PROGRESS", rest)) => {
                    let mut n = rest.split(' ').filter_map(|x| x.parse::<u64>().ok());
                    emit("download", n.next().unwrap_or(0), n.next().unwrap_or(0));
                }
                Some(("STAGE", stage)) => emit(stage, 0, 0),
                Some(("ERROR", msg)) => error = Some(msg.to_string()),
                _ => {}
            }
        }
        if child.wait().map_err(|e| e.to_string())?.success() {
            Ok(())
        } else {
            Err(error.unwrap_or_else(|| "The NVIDIA installer failed.".into()))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn start_engine(
    app: AppHandle,
    state: State<AppState>,
    input: Option<String>,
    output: Option<String>,
    chain: Vec<FilterCfg>,
) -> Result<(), String> {
    let mut slot = state.engine.lock().unwrap();
    if let Some(old) = slot.take() {
        old.stop();
    }
    *slot = Some(engine::start(app, input, output, chain, state.mute.clone())?);
    Ok(())
}

#[tauri::command]
pub fn stop_engine(state: State<AppState>) {
    if let Some(e) = state.engine.lock().unwrap().take() {
        e.stop();
    }
}

#[tauri::command]
pub fn set_preview(state: State<AppState>, on: bool) -> Result<(), String> {
    match state.engine.lock().unwrap().as_ref() {
        Some(e) => e.set_preview(on),
        None if on => Err("Start the engine first.".into()),
        None => Ok(()),
    }
}

#[tauri::command]
pub fn set_chain(state: State<AppState>, chain: Vec<FilterCfg>) {
    if let Some(e) = state.engine.lock().unwrap().as_ref() {
        e.send(engine::Msg::Chain(chain));
    }
}

#[tauri::command]
pub fn set_mute(state: State<AppState>, mute: bool) {
    state.mute.store(mute, Ordering::Relaxed);
}

fn preset_path(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let ok = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'));
    if !ok {
        return Err("preset names may only contain letters, digits, spaces, '-' and '_'".into());
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("presets");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{name}.json")))
}

#[tauri::command]
pub fn list_presets(app: AppHandle) -> Result<Vec<String>, String> {
    let dir = preset_path(&app, "x")?.parent().unwrap().to_path_buf();
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    v.sort();
    Ok(v)
}

#[tauri::command]
pub fn save_preset(app: AppHandle, name: String, chain: Vec<serde_json::Value>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&chain).map_err(|e| e.to_string())?;
    std::fs::write(preset_path(&app, &name)?, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_preset(app: AppHandle, name: String) -> Result<Vec<serde_json::Value>, String> {
    let s = std::fs::read_to_string(preset_path(&app, &name)?).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_preset(app: AppHandle, name: String) -> Result<(), String> {
    std::fs::remove_file(preset_path(&app, &name)?).map_err(|e| e.to_string())
}
