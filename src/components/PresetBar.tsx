import { useState } from "react";
import { useStore } from "../store";

export function PresetBar() {
  const { presets, savePreset, loadPreset, deletePreset } = useStore();
  const [name, setName] = useState("");
  const [selected, setSelected] = useState("");

  return (
    <section className="presets">
      <select value={selected} onChange={(e) => setSelected(e.target.value)}>
        <option value="">Presets…</option>
        {presets.map((p) => (
          <option key={p}>{p}</option>
        ))}
      </select>
      <button disabled={!selected} onClick={() => loadPreset(selected)}>
        Load
      </button>
      <button
        disabled={!selected}
        onClick={() => {
          deletePreset(selected);
          setSelected("");
        }}
      >
        Delete
      </button>
      <input placeholder="New preset name" value={name} onChange={(e) => setName(e.target.value)} />
      <button
        disabled={!name.trim()}
        onClick={() => {
          savePreset(name.trim());
          setName("");
        }}
      >
        Save current
      </button>
    </section>
  );
}
