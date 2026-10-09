import { create } from "zustand";
import { api, type Devices, type Meter, type NvidiaGpu, type NvidiaInstallProgress, type NvidiaStatus } from "./api";
import { FILTERS, defaultChain, makeFilter, type FilterCfg, type FilterKind, type ParamValue } from "./filters/schema";
import { normalizeChain, signature } from "./filters/chain";

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
  /** The preset the chain was last loaded from or saved as, and what it looked like then (see `signature`). */
  activePreset: string | null;
  presetBaseline: string | null;
}

/** What "Undo" restores after loading a preset or resetting. */
interface Snapshot {
  chain: FilterCfg[];
  activePreset: string | null;
  presetBaseline: string | null;
}

const loadSaved = (): Saved => {
  const fresh: Saved = {
    input: null,
    output: null,
    chain: defaultChain(),
    closeToTray: false,
    autostartDefaulted: false,
    nvidiaPromptDismissed: false,
    activePreset: null,
    presetBaseline: null,
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
  /** Bumped on every toast so repeating the same message restarts its timer. */
  noticeSeq: number;
  undo: Snapshot | null;
  collapsed: Record<string, boolean>;
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
  dismissNotice: () => void;
  undoChain: () => void;

  addFilter: (kind: FilterKind) => void;
  removeFilter: (id: string) => void;
  moveFilter: (from: number, to: number) => void;
  toggleFilter: (id: string) => void;
  setParam: (id: string, key: string, value: ParamValue) => void;
  toggleCollapsed: (id: string) => void;
  setAllCollapsed: (on: boolean) => void;

  refreshPresets: () => Promise<void>;
  /** Saves the current chain under `name` (replacing any preset with that name) and makes it the active preset. */
  savePreset: (name: string) => Promise<void>;
  saveActivePreset: () => Promise<void>;
  loadPreset: (name: string) => Promise<void>;
  deletePreset: (name: string) => Promise<void>;
  resetChain: () => void;
}

export const useStore = create<State>((set, get) => {
  const persist = () => {
    const { input, output, chain, closeToTray, autostartDefaulted, nvidiaPromptDismissed, activePreset, presetBaseline } = get();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ input, output, chain, closeToTray, autostartDefaulted, nvidiaPromptDismissed, activePreset, presetBaseline }),
    );
  };

  const setChain = (chain: FilterCfg[]) => {
    set({ chain });
    persist();
    if (get().running) api.setChain(chain).catch((e) => set({ error: String(e) }));
  };

  const fail = (e: unknown) => set({ error: String(e) });

  /** Shows a toast, optionally with an Undo that restores `undo`. */
  const say = (notice: string, undo: Snapshot | null = null) => set((s) => ({ notice, undo, noticeSeq: s.noticeSeq + 1 }));

  const snapshot = (): Snapshot => {
    const { chain, activePreset, presetBaseline } = get();
    return { chain, activePreset, presetBaseline };
  };

  /** Swaps in a whole chain (preset load, reset, undo) and records which preset it came from. */
  const replaceChain = (chain: FilterCfg[], activePreset: string | null, presetBaseline: string | null) => {
    set({ activePreset, presetBaseline });
    setChain(chain);
  };

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
    noticeSeq: 0,
    undo: null,
    collapsed: {},
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
          nvidiaError: nvidia.available ? null : "The installer finished, but the SDK still isn't found. Restart Better Mic and check again.",
        });
        if (nvidia.available) say("NVIDIA noise removal installed. Pick it under Noise suppression → Method.");
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
        say(
          found
            ? "VB-Cable installed. In your apps, choose “CABLE Output” as the microphone."
            : "Installer finished. If CABLE doesn't show up, restart Windows and press Refresh.",
        );
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

      // Work out which saved preset (if any) the restored chain still matches, so the toolbar shows the right name.
      const { presets } = get();
      if (get().activePreset && !presets.includes(get().activePreset!)) set({ activePreset: null, presetBaseline: null });
      if (!get().activePreset) {
        const sig = signature(get().chain);
        for (const name of presets) {
          const saved = await api.loadPreset(name).then(normalizeChain).catch(() => null);
          if (saved && signature(saved) === sig) {
            set({ activePreset: name, presetBaseline: sig });
            break;
          }
        }
      }
      persist();

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

    dismissError: () => set({ error: null, notice: null, undo: null }),
    dismissNotice: () => set({ notice: null, undo: null }),
    undoChain: () => {
      const u = get().undo;
      if (!u) return;
      replaceChain(u.chain, u.activePreset, u.presetBaseline);
      say("Undone");
    },

    addFilter: (kind) => {
      const prev = snapshot();
      setChain([...prev.chain, makeFilter(kind)]);
      say(`Added ${FILTERS[kind].label} as step ${prev.chain.length + 1}`, prev);
    },
    removeFilter: (id) => {
      const prev = snapshot();
      const gone = prev.chain.find((f) => f.id === id);
      setChain(prev.chain.filter((f) => f.id !== id));
      if (gone) say(`Removed ${FILTERS[gone.kind].label}`, prev);
    },
    moveFilter: (from, to) => {
      const chain = [...get().chain];
      chain.splice(to, 0, chain.splice(from, 1)[0]);
      setChain(chain);
    },
    toggleFilter: (id) => setChain(get().chain.map((f) => (f.id === id ? { ...f, enabled: !f.enabled } : f))),
    setParam: (id, key, value) =>
      setChain(get().chain.map((f) => (f.id === id ? { ...f, params: { ...f.params, [key]: value } } : f))),

    toggleCollapsed: (id) => set((s) => ({ collapsed: { ...s.collapsed, [id]: !s.collapsed[id] } })),
    setAllCollapsed: (on) => set((s) => ({ collapsed: Object.fromEntries(s.chain.map((f) => [f.id, on])) })),

    refreshPresets: async () => set({ presets: await api.listPresets() }),
    savePreset: async (name) => {
      const { chain } = get();
      try {
        await api.savePreset(name, chain);
        set({ activePreset: name, presetBaseline: signature(chain) });
        persist();
        await get().refreshPresets();
        say(`Saved “${name}”`);
      } catch (e) {
        fail(e);
      }
    },
    saveActivePreset: async () => {
      const name = get().activePreset;
      if (name) await get().savePreset(name);
    },
    loadPreset: async (name) => {
      try {
        const chain = normalizeChain(await api.loadPreset(name));
        const prev = snapshot();
        replaceChain(chain, name, signature(chain));
        say(`Loaded “${name}”`, signature(prev.chain) === signature(chain) ? null : prev);
      } catch (e) {
        fail(e);
      }
    },
    deletePreset: async (name) => {
      try {
        await api.deletePreset(name);
        if (get().activePreset === name) {
          set({ activePreset: null, presetBaseline: null });
          persist();
        }
        await get().refreshPresets();
        say(`Deleted “${name}”`);
      } catch (e) {
        fail(e);
      }
    },
    resetChain: () => {
      const prev = snapshot();
      replaceChain(defaultChain(), null, null);
      say("Reset to the default chain", prev);
    },
  };
});
