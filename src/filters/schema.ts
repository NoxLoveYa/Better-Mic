export type FilterKind =
  | "denoise"
  | "gate"
  | "eq3"
  | "expander"
  | "compressor"
  | "upward"
  | "limiter"
  | "gain"
  | "polarity"
  | "delay";

export type ParamValue = number | string;

export interface FilterCfg {
  id: string;
  kind: FilterKind;
  enabled: boolean;
  params: Record<string, ParamValue>;
}

interface Slider {
  key: string;
  label: string;
  min: number;
  max: number;
  step: number;
  def: number;
  unit?: string;
}

interface Select {
  key: string;
  label: string;
  def: string;
  options: { value: string; label: string }[];
}

export type ParamDef = Slider | Select;

export const isSelect = (p: ParamDef): p is Select => "options" in p;

export interface FilterDef {
  label: string;
  params: ParamDef[];
}

const dynamics = (thr: number, ratio: number, att: number, rel: number, ratioMax = 32): ParamDef[] => [
  { key: "ratio", label: "Ratio", min: 1, max: ratioMax, step: 0.5, def: ratio, unit: ":1" },
  { key: "threshold", label: "Threshold", min: -60, max: 0, step: 0.5, def: thr, unit: "dB" },
  { key: "attack_time", label: "Attack", min: 1, max: 500, step: 1, def: att, unit: "ms" },
  { key: "release_time", label: "Release", min: 1, max: 1000, step: 1, def: rel, unit: "ms" },
  { key: "output_gain", label: "Output gain", min: -32, max: 32, step: 0.5, def: 0, unit: "dB" },
];

export const FILTERS: Record<FilterKind, FilterDef> = {
  denoise: {
    label: "Noise suppression",
    params: [
      {
        key: "method",
        label: "Method",
        def: "rnnoise",
        options: [
          { value: "rnnoise", label: "RNNoise (CPU)" },
          { value: "nvidia", label: "NVIDIA Noise Removal (RTX)" },
        ],
      },
      { key: "intensity", label: "Intensity", min: 0, max: 1, step: 0.01, def: 1 },
    ],
  },
  gate: {
    label: "Noise gate",
    params: [
      { key: "open_threshold", label: "Open threshold", min: -96, max: 0, step: 0.5, def: -26, unit: "dB" },
      { key: "close_threshold", label: "Close threshold", min: -96, max: 0, step: 0.5, def: -32, unit: "dB" },
      { key: "attack_time", label: "Attack", min: 1, max: 500, step: 1, def: 25, unit: "ms" },
      { key: "hold_time", label: "Hold", min: 1, max: 500, step: 1, def: 200, unit: "ms" },
      { key: "release_time", label: "Release", min: 1, max: 1000, step: 1, def: 150, unit: "ms" },
    ],
  },
  eq3: {
    label: "3-band equalizer",
    params: [
      { key: "low", label: "Low", min: -20, max: 20, step: 0.5, def: 0, unit: "dB" },
      { key: "mid", label: "Mid", min: -20, max: 20, step: 0.5, def: 0, unit: "dB" },
      { key: "high", label: "High", min: -20, max: 20, step: 0.5, def: 0, unit: "dB" },
    ],
  },
  expander: { label: "Expander", params: dynamics(-40, 4, 10, 50, 20) },
  compressor: { label: "Compressor", params: dynamics(-18, 10, 6, 60) },
  upward: { label: "Upward compressor", params: dynamics(-32, 2, 10, 100) },
  limiter: {
    label: "Limiter",
    params: [
      { key: "threshold", label: "Threshold", min: -60, max: 0, step: 0.5, def: -6, unit: "dB" },
      { key: "release_time", label: "Release", min: 1, max: 1000, step: 1, def: 60, unit: "ms" },
    ],
  },
  gain: {
    label: "Gain",
    params: [{ key: "gain", label: "Gain", min: -30, max: 30, step: 0.1, def: 0, unit: "dB" }],
  },
  polarity: { label: "Invert polarity", params: [] },
  delay: {
    label: "Sync offset (delay)",
    params: [{ key: "delay", label: "Delay", min: 0, max: 1900, step: 1, def: 0, unit: "ms" }],
  },
};

export const defaultParams = (kind: FilterKind): Record<string, ParamValue> =>
  Object.fromEntries(FILTERS[kind].params.map((p) => [p.key, p.def]));

export const makeFilter = (kind: FilterKind, enabled = true): FilterCfg => ({
  id: crypto.randomUUID(),
  kind,
  enabled,
  params: defaultParams(kind),
});

export const defaultChain = (): FilterCfg[] => [
  makeFilter("denoise"),
  makeFilter("gate", false),
  makeFilter("eq3", false),
  makeFilter("compressor", false),
  makeFilter("limiter"),
];
