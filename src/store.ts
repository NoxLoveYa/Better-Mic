import { create } from "zustand";
import { api, type Devices, type Meter, type NvidiaGpu, type NvidiaInstallProgress, type NvidiaStatus } from "./api";
import { defaultChain, makeFilter, type FilterCfg, type FilterKind, type ParamValue } from "./filters/schema";

const STORAGE_KEY = "better-mic";

interface Saved {
  input: string | null;
  output: string | null;
  chain: FilterCfg[];
  closeToTray: boolean;
  /** Whether the "launch on startup" default has been applied once; after that the OS entry is the truth. */
  autostartDefaulted: boolean;
  /** Set once the NVIDIA install popup was closed, so it doesn't reappear on every launch. */
  nvidiaPromptDismissed: boolean;
}

const loadSaved = (): Saved => {
  const fresh: Saved = {
    input: null,
    output: null,
    chain: defaultChain(),
    closeToTray: false,
    autostartDefaulted: false,
    nvidiaPromptDismissed: false,
  };
  try {
    const s = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "");
    if (Array.isArray(s.chain)) return { ...fresh, ...s };
  } catch {
    /* first run or corrupt data */
  }
  return fresh;
};

interface State extends Saved {
  autostart: boolean;
  devices: Devices | null;
  running: boolean;
  preview: boolean;
  muted: boolean;
  meter: Meter | null;
  error: string | null;
  notice: string | null;
  installingCable: boolean;
  presets: string[];
  nvidia: NvidiaStatus | null;
  gpu: NvidiaGpu | null;
  nvidiaPrompt: boolean;
  nvidiaInstall: NvidiaInstallProgress | null;
  nvidiaError: string | null;

  init: () => Promise<void>;
  installCable: () => Promise<void>;
  openNvidiaPrompt: () => void;
  closeNvidiaPrompt: () => void;
  installNvidia: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  setInput: (id: string) => void;
  setOutput: (id: string) => void;
  toggleRun: () => Promise<void>;
  togglePreview: () => Promise<void>;
  toggleMute: () => void;
  setAutostart: (on: boolean) => Promise<void>;
  setCloseToTray: (on: boolean) => void;
  dismissError: () => void;

  addFilter: (kind: FilterKind) => void;
  removeFilter: (id: string) => void;
  moveFilter: (from: number, to: number) => void;
  toggleFilter: (id: string) => void;
  setParam: (id: string, key: string, value: ParamValue) => void;

  refreshPresets: () => Promise<void>;
  savePreset: (name: string) => Promise<void>;
  loadPreset: (name: string) => Promise<void>;
  deletePreset: (name: string) => Promise<void>;
}

export const useStore = create<State>((set, get) => {
  const persist = () => {
    const { input, output, chain, closeToTray, autostartDefaulted, nvidiaPromptDismissed } = get();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ input, output, chain, closeToTray, autostartDefaulted, nvidiaPromptDismissed }),
    );
  };

  const setChain = (chain: FilterCfg[]) => {
    set({ chain });
    persist();
    if (get().running) api.setChain(chain).catch((e) => set({ error: String(e) }));
  };

  const fail = (e: unknown) => set({ error: String(e) });

  return {
    ...loadSaved(),
    autostart: false,
    devices: null,
    running: false,
    preview: false,
    muted: false,
    meter: null,
    error: null,
    notice: null,
    installingCable: false,
    presets: [],
    nvidia: null,
    gpu: null,
    nvidiaPrompt: false,
    nvidiaInstall: null,
    nvidiaError: null,

    openNvidiaPrompt: () => set({ nvidiaPrompt: true, nvidiaError: null }),
    closeNvidiaPrompt: () => {
      set({ nvidiaPrompt: false, nvidiaPromptDismissed: true });
      persist();
    },

    installNvidia: async () => {
      const { gpu } = get();
      if (!gpu) return;
      set({ nvidiaInstall: { stage: "download", done: 0, total: 0 }, nvidiaError: null });
      try {
        await api.installNvidiaSdk(gpu.arch);
        const nvidia = await api.nvidiaStatus();
        set({
          nvidia,
          nvidiaPrompt: !nvidia.available,
          notice: nvidia.available
            ? "NVIDIA noise removal installed. Pick it under Noise suppression → Method."
            : null,
          nvidiaError: nvidia.available ? null : "The installer finished, but the SDK still isn't found. Restart Better Mic and check again.",
        });
      } catch (e) {
        set({ nvidiaError: String(e) });
      } finally {
        set({ nvidiaInstall: null });
      }
    },

    installCable: async () => {
      set({ installingCable: true, error: null, notice: null });
      try {
        await api.installVbCable();
        await get().refreshDevices();
        const found = get().devices?.outputs.some((d) => /CABLE Input/i.test(d.name));
        set({
          notice: found
            ? "VB-Cable installed. In your apps, choose “CABLE Output” as the microphone."
            : "Installer finished. If CABLE doesn't show up, restart Windows and press Refresh.",
        });
      } catch (e) {
        fail(e);
      } finally {
        set({ installingCable: false });
      }
    },

    init: async () => {
      await api.onMeter((meter) => set({ meter }));
      await api.onError((error) => {
        api.stop().catch(() => {});
        set({ error, running: false, preview: false, meter: null });
      });
      await api.onPreviewError((error) => {
        api.setPreview(false).catch(() => {});
        set({ error, preview: false });
      });
      await api.onNvidiaInstall((p) => get().nvidiaInstall && set({ nvidiaInstall: p }));
      await Promise.all([
        get().refreshDevices(),
        get().refreshPresets(),
        api.nvidiaStatus().then((nvidia) => set({ nvidia })),
        api.nvidiaGpu().then((gpu) => set({ gpu })),
      ]).catch(fail);
      const { gpu, nvidia, nvidiaPromptDismissed } = get();
      if (gpu && nvidia && !nvidia.available && !nvidiaPromptDismissed) set({ nvidiaPrompt: true });

      // Re-check when returning to the window, so installing the NVIDIA SDK doesn't need an app restart.
      window.addEventListener("focus", () => api.nvidiaStatus().then((nvidia) => set({ nvidia })).catch(() => {}));

      try {
        if (!get().autostartDefaulted) {
          await api.setAutostart(true);
          set({ autostartDefaulted: true });
          persist();
        }
        set({ autostart: await api.getAutostart() });
        await api.setCloseToTray(get().closeToTray);
      } catch (e) {
        fail(e);
      }

      if (await api.launchedAtStartup().catch(() => false)) {
        // Right after login the virtual cable may not be up yet, so keep trying for a while.
        for (let i = 0; i < 10 && !get().running; i++) {
          await get().refreshDevices().catch(() => {});
          await get().toggleRun();
          if (!get().running) await new Promise((r) => setTimeout(r, 3000));
        }
      }
    },

    refreshDevices: async () => {
      const devices = await api.listDevices();
      const { input, output } = get();
      const has = (list: Devices["inputs"], id: string | null) => id && list.some((d) => d.id === id);
      set({
        devices,
        input: has(devices.inputs, input) ? input : devices.default_input,
        output: has(devices.outputs, output)
          ? output
          : (devices.outputs.find((d) => /CABLE Input/i.test(d.name))?.id ?? devices.default_output),
      });
    },

    setInput: (input) => {
      set({ input });
      persist();
    },
    setOutput: (output) => {
      set({ output });
      persist();
    },

    toggleRun: async () => {
      const { running, input, output, chain } = get();
      try {
        if (running) {
          await api.stop();
          set({ running: false, preview: false, meter: null });
        } else {
          await api.start(input, output, chain);
          set({ running: true, error: null });
        }
      } catch (e) {
        fail(e);
      }
    },

    togglePreview: async () => {
      const preview = !get().preview;
      try {
        await api.setPreview(preview);
        set({ preview });
      } catch (e) {
        fail(e);
      }
    },

    toggleMute: () => {
      const muted = !get().muted;
      set({ muted });
      api.setMute(muted).catch(fail);
    },

    setAutostart: async (on) => {
      try {
        await api.setAutostart(on);
        set({ autostart: on });
      } catch (e) {
        fail(e);
      }
    },

    setCloseToTray: (closeToTray) => {
      set({ closeToTray });
      persist();
      api.setCloseToTray(closeToTray).catch(fail);
    },

    dismissError: () => set({ error: null, notice: null }),

    addFilter: (kind) => setChain([...get().chain, makeFilter(kind)]),
    removeFilter: (id) => setChain(get().chain.filter((f) => f.id !== id)),
    moveFilter: (from, to) => {
      const chain = [...get().chain];
      chain.splice(to, 0, chain.splice(from, 1)[0]);
      setChain(chain);
    },
    toggleFilter: (id) => setChain(get().chain.map((f) => (f.id === id ? { ...f, enabled: !f.enabled } : f))),
    setParam: (id, key, value) =>
      setChain(get().chain.map((f) => (f.id === id ? { ...f, params: { ...f.params, [key]: value } } : f))),

    refreshPresets: async () => set({ presets: await api.listPresets() }),
    savePreset: async (name) => {
      await api.savePreset(name, get().chain).catch(fail);
      await get().refreshPresets();
    },
    loadPreset: async (name) => {
      try {
        const loaded = await api.loadPreset(name);
        setChain(loaded.map((f) => ({ ...f, id: crypto.randomUUID() })));
      } catch (e) {
        fail(e);
      }
    },
    deletePreset: async (name) => {
      await api.deletePreset(name).catch(fail);
      await get().refreshPresets();
    },
  };
});
