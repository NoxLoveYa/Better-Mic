import type { CSSProperties } from "react";
import { useStore } from "../store";
import { FILTERS, isSelect, type FilterCfg } from "../filters/schema";

interface Props {
  filter: FilterCfg;
  index: number;
  count: number;
  onDragStart: () => void;
  onDrop: () => void;
}

export function FilterCard({ filter, index, count, onDragStart, onDrop }: Props) {
  const { toggleFilter, removeFilter, moveFilter, setParam, nvidia } = useStore();
  const def = FILTERS[filter.kind];

  return (
    <li className={`step${filter.enabled ? " on" : ""}`} onDragOver={(e) => e.preventDefault()} onDrop={onDrop}>
      <span className="num" title={`Step ${index + 1} of ${count}`}>
        {String(index + 1).padStart(2, "0")}
      </span>
      <div className="card">
        <div className="card-head">
          <span className="grip" title="Drag to reorder" draggable onDragStart={onDragStart}>
            ⋮⋮
          </span>
          <h3>{def.label}</h3>
          {!filter.enabled && <span className="tag">Bypassed</span>}
          <span className="fill" />
          <button className="ghost" disabled={index === 0} onClick={() => moveFilter(index, index - 1)} title="Apply earlier">
            ↑
          </button>
          <button className="ghost" disabled={index === count - 1} onClick={() => moveFilter(index, index + 1)} title="Apply later">
            ↓
          </button>
          <button
            role="switch"
            aria-checked={filter.enabled}
            aria-label={`${def.label} enabled`}
            className="switch"
            title={filter.enabled ? "Bypass" : "Enable"}
            onClick={() => toggleFilter(filter.id)}
          />
          <button className="ghost" onClick={() => removeFilter(filter.id)} title="Remove">
            ✕
          </button>
        </div>
        {def.params.length > 0 && (
          <div className="params">
            {def.params.map((p) => {
              const value = filter.params[p.key] ?? p.def;
              if (isSelect(p)) {
                return (
                  <label key={p.key}>
                    {p.label}
                    <select value={String(value)} onChange={(e) => setParam(filter.id, p.key, e.target.value)}>
                      {p.options.map((o) => (
                        <option key={o.value} value={o.value} disabled={o.value === "nvidia" && nvidia?.available === false}>
                          {o.label}
                        </option>
                      ))}
                    </select>
                    {filter.kind === "denoise" && value === "nvidia" && nvidia && <small>{nvidia.detail}</small>}
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
