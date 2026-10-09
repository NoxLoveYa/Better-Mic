import type { CSSProperties } from "react";
import { useStore } from "../store";
import { FILTERS, isSelect, type FilterCfg } from "../filters/schema";
import { Icon } from "./Icon";
import { Select } from "./Select";

interface Props {
  filter: FilterCfg;
  index: number;
  count: number;
  onDragStart: () => void;
  onDrop: () => void;
}

export function FilterCard({ filter, index, count, onDragStart, onDrop }: Props) {
  const { toggleFilter, removeFilter, moveFilter, setParam, nvidia, gpu, openNvidiaPrompt, collapsed, toggleCollapsed } =
    useStore();
  const def = FILTERS[filter.kind];
  const folded = !!collapsed[filter.id];

  return (
    <li className={`step${filter.enabled ? " on" : ""}`} onDragOver={(e) => e.preventDefault()} onDrop={onDrop}>
      <span className="num" title={`Step ${index + 1} of ${count}`}>
        {String(index + 1).padStart(2, "0")}
      </span>
      <div className="card">
        <div className="card-head">
          <span className="grip" title="Drag to reorder" draggable onDragStart={onDragStart}>
            <Icon name="grip" />
          </span>
          <h3 className={`card-title${folded ? " folded" : ""}`}>
            <button aria-expanded={!folded} title={folded ? "Expand" : "Collapse"} onClick={() => toggleCollapsed(filter.id)}>
              <Icon name="chevron" />
              {def.label}
            </button>
          </h3>
          {!filter.enabled && <span className="tag">Bypassed</span>}
          <span className="fill" />
          <button className="ghost icon" disabled={index === 0} onClick={() => moveFilter(index, index - 1)} title="Apply earlier" aria-label="Move up">
            <Icon name="up" />
          </button>
          <button className="ghost icon" disabled={index === count - 1} onClick={() => moveFilter(index, index + 1)} title="Apply later" aria-label="Move down">
            <Icon name="down" />
          </button>
          <button
            role="switch"
            aria-checked={filter.enabled}
            aria-label={`${def.label} enabled`}
            className="switch"
            title={filter.enabled ? "Bypass" : "Enable"}
            onClick={() => toggleFilter(filter.id)}
          />
          <button className="ghost icon" onClick={() => removeFilter(filter.id)} title="Remove" aria-label="Remove">
            <Icon name="close" />
          </button>
        </div>
        {def.params.length > 0 && !folded && (
          <div className="params">
            {def.params.map((p) => {
              const value = filter.params[p.key] ?? p.def;
              if (isSelect(p)) {
                return (
                  <label key={p.key}>
                    {p.label}
                    <Select
                      value={String(value)}
                      options={p.options.map((o) => ({ ...o, disabled: o.value === "nvidia" && nvidia?.available === false }))}
                      onChange={(v) => setParam(filter.id, p.key, v)}
                    />
                    {filter.kind === "denoise" && nvidia && (!nvidia.available || value === "nvidia") && (
                      <small>
                        {nvidia.detail}
                        {gpu && !nvidia.available && (
                          <button className="link" onClick={openNvidiaPrompt}>
                            Install it now
                          </button>
                        )}
                      </small>
                    )}
                  </label>
                );
              }
              const fill = ((Number(value) - p.min) / (p.max - p.min)) * 100;
              return (
                <label key={p.key}>
                  <span>
                    {p.label}
                    <b>
                      {Number(value).toFixed(p.step < 1 ? (p.step < 0.1 ? 2 : 1) : 0)}
                      {p.unit ? ` ${p.unit}` : ""}
                    </b>
                  </span>
                  <input
                    type="range"
                    style={{ "--p": `${fill}%` } as CSSProperties}
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    value={Number(value)}
                    onChange={(e) => setParam(filter.id, p.key, Number(e.target.value))}
                    onDoubleClick={() => setParam(filter.id, p.key, p.def)}
                  />
                </label>
              );
            })}
          </div>
        )}
      </div>
    </li>
  );
}
