import { create } from "zustand";
import { api, type Devices, type Meter, type NvidiaStatus } from "./api";
import { defaultChain, makeFilter, type FilterCfg, type FilterKind, type ParamValue } from "./filters/schema";

const STORAGE_KEY = "better-mic";

interface Saved {
  input: string | null;
  output: string | null;
  chain: FilterCfg[];
}

const loadSaved = (): Saved => {
  try {
    const s = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "");
    if (Array.isArray(s.chain)) return s;
  } catch {
    /* first run or corrupt data */
  }
  return { input: null, output: null, chain: defaultChain() };
};

interface State extends Saved {
  devices: Devices | null;
  running: boolean;
  muted: boolean;
  meter: Meter | null;
  error: string | null;
  notice: string | null;
  installingCable: boolean;
  presets: string[];
  nvidia: NvidiaStatus | null;

  init: () => Promise<void>;
  installCable: () => Promise<void>;
  refreshDevices: () => Promise<void>;
  setInput: (id: string) => void;
  setOutput: (id: string) => void;
  toggleRun: () => Promise<void>;
  toggleMute: () => void;
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
    const { input, output, chain } = get();
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ input, output, chain }));
  };

  const setChain = (chain: FilterCfg[]) => {
    set({ chain });
    persist();
    if (get().running) api.setChain(chain).catch((e) => set({ error: String(e) }));
  };

  const fail = (e: unknown) => set({ error: String(e) });

  return {
    ...loadSaved(),
    devices: null,
    running: false,
    muted: false,
    meter: null,
    error: null,
    notice: null,
    installingCable: false,
    presets: [],
    nvidia: null,

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
        set({ error, running: false, meter: null });
      });
      await Promise.all([get().refreshDevices(), get().refreshPresets(), api.nvidiaStatus().then((nvidia) => set({ nvidia }))]).catch(fail);
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
          set({ running: false, meter: null });
        } else {
          await api.start(input, output, chain);
          set({ running: true, error: null });
        }
      } catch (e) {
        fail(e);
      }
    },

    toggleMute: () => {
      const muted = !get().muted;
      set({ muted });
      api.setMute(muted).catch(fail);
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
