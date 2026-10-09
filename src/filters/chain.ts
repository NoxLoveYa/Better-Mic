import { FILTERS, defaultChain, defaultParams, type FilterCfg } from "./schema";

/** Fingerprint of what a chain does, ignoring the random ids. Equal fingerprints mean "no unsaved changes". */
export const signature = (chain: FilterCfg[]): string =>
  JSON.stringify(chain.map((f) => [f.kind, f.enabled, Object.entries(f.params).sort(([a], [b]) => (a < b ? -1 : 1))]));

export const DEFAULT_SIGNATURE = signature(defaultChain());

/** Makes a chain read from disk safe to use: unknown filters dropped, missing params defaulted, fresh ids. */
export const normalizeChain = (raw: FilterCfg[]): FilterCfg[] =>
  raw
    .filter((f) => f.kind in FILTERS)
    .map((f) => ({
      id: crypto.randomUUID(),
      kind: f.kind,
      enabled: f.enabled !== false,
      params: { ...defaultParams(f.kind), ...f.params },
    }));

/** Same rule the backend enforces for preset file names. */
export const PRESET_NAME = /^[\p{L}\p{N} _-]+$/u;
