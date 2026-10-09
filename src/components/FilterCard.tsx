import { useStore } from "../store";
import { FILTERS, isSelect, type FilterCfg } from "../filters/schema";

interface Props {
  filter: FilterCfg;
  onDragStart: () => void;
  onDrop: () => void;
}

export function FilterCard({ filter, onDragStart, onDrop }: Props) {
  const { toggleFilter, removeFilter, setParam, nvidia } = useStore();
  const def = FILTERS[filter.kind];

  return (
    <div className={`card${filter.enabled ? "" : " off"}`} onDragOver={(e) => e.preventDefault()} onDrop={onDrop}>
      <div className="card-head">
        <span className="grip" title="Drag to reorder" draggable onDragStart={onDragStart}>
          ⠿
        </span>
        <h3>{def.label}</h3>
        <button className={filter.enabled ? "on" : ""} onClick={() => toggleFilter(filter.id)}>
          {filter.enabled ? "On" : "Bypassed"}
        </button>
        <button onClick={() => removeFilter(filter.id)} title="Remove">
          ✕
        </button>
      </div>
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
          return (
            <label key={p.key}>
              <span>
                {p.label} <b>{Number(value).toFixed(p.step < 1 ? (p.step < 0.1 ? 2 : 1) : 0)}{p.unit ? ` ${p.unit}` : ""}</b>
              </span>
              <input
                type="range"
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
    </div>
  );
}
