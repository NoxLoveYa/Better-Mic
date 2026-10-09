import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { FilterCfg } from "./filters/schema";

export interface DeviceInfo {
  id: string;
  name: string;
}

export interface Devices {
  inputs: DeviceInfo[];
  outputs: DeviceInfo[];
  default_input: string | null;
  default_output: string | null;
}

export interface Meter {
  in_peak: number;
  in_rms: number;
  out_peak: number;
  out_rms: number;
  buffered_ms: number;
  exclusive: boolean;
}

export interface NvidiaStatus {
  available: boolean;
  detail: string;
}

export interface NvidiaGpu {
  name: string;
  arch: "turing" | "ampere" | "ada" | "blackwell";
}

export interface NvidiaInstallProgress {
  stage: "download" | "verify" | "install";
  done: number;
  total: number;
}

export const api = {
  listDevices: () => invoke<Devices>("list_devices"),
  installVbCable: () => invoke<void>("install_vbcable"),
  nvidiaStatus: () => invoke<NvidiaStatus>("nvidia_status"),
  nvidiaGpu: () => invoke<NvidiaGpu | null>("nvidia_gpu"),
  installNvidiaSdk: (arch: NvidiaGpu["arch"]) => invoke<void>("install_nvidia_sdk", { arch }),
  onNvidiaInstall: (cb: (p: NvidiaInstallProgress) => void) =>
    listen<NvidiaInstallProgress>("nvidia-install", (e) => cb(e.payload)),
  start: (input: string | null, output: string | null, chain: FilterCfg[]) =>
    invoke<void>("start_engine", { input, output, chain }),
  stop: () => invoke<void>("stop_engine"),
  setChain: (chain: FilterCfg[]) => invoke<void>("set_chain", { chain }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (on: boolean) => invoke<void>("set_autostart", { on }),
  launchedAtStartup: () => invoke<boolean>("launched_at_startup"),
  setCloseToTray: (on: boolean) => invoke<void>("set_close_to_tray", { on }),
  setPreview: (on: boolean) => invoke<void>("set_preview", { on }),
  setMute: (mute: boolean) => invoke<void>("set_mute", { mute }),
  listPresets: () => invoke<string[]>("list_presets"),
  savePreset: (name: string, chain: FilterCfg[]) => invoke<void>("save_preset", { name, chain }),
  loadPreset: (name: string) => invoke<FilterCfg[]>("load_preset", { name }),
  deletePreset: (name: string) => invoke<void>("delete_preset", { name }),
  onMeter: (cb: (m: Meter) => void) => listen<Meter>("meter", (e) => cb(e.payload)),
  onError: (cb: (msg: string) => void) => listen<string>("engine-error", (e) => cb(e.payload)),
  onPreviewError: (cb: (msg: string) => void) => listen<string>("preview-error", (e) => cb(e.payload)),
};
