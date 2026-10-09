import { useEffect, useRef, useState } from "react";
import { useStore } from "./store";
import { FILTERS, type FilterKind } from "./filters/schema";
import { FilterCard } from "./components/FilterCard";
import { Meters } from "./components/Meters";
import { PresetBar } from "./components/PresetBar";

export default function App() {
  const s = useStore();
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const init = useRef(false);

  useEffect(() => {
    if (init.current) return;
    init.current = true;
    useStore.getState().init();
  }, []);

  const cableFound = s.devices?.outputs.some((d) => /CABLE Input/i.test(d.name));

  return (
    <div className="app">
      <header>
        <h1>Better Mic</h1>
        <button className={s.running ? "stop" : "go"} onClick={s.toggleRun}>
          {s.running ? "Stop" : "Start"}
        </button>
        <button className={s.muted ? "on" : ""} onClick={s.toggleMute}>
          {s.muted ? "Muted" : "Mute"}
        </button>
      </header>

      {s.error && (
        <div className="banner error" onClick={s.dismissError}>
          {s.error}
        </div>
      )}
      {s.notice && (
        <div className="banner info" onClick={s.dismissError}>
          {s.notice}
        </div>
      )}
      {s.devices && !cableFound && (
        <div className="banner warn">
          <span>
            VB-Cable (virtual microphone driver) isn't installed. It's donationware from vb-audio.com; the installer asks
            for administrator rights.
          </span>
          <button onClick={s.installCable} disabled={s.installingCable}>
            {s.installingCable ? "Installing…" : "Download & install VB-Cable"}
          </button>
        </div>
      )}

      <section className="devices">
        <label>
          Microphone
          <select disabled={s.running} value={s.input ?? ""} onChange={(e) => s.setInput(e.target.value)}>
            {s.devices?.inputs.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          Output (virtual cable)
          <select disabled={s.running} value={s.output ?? ""} onChange={(e) => s.setOutput(e.target.value)}>
            {s.devices?.outputs.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </select>
        </label>
        <button onClick={() => s.refreshDevices()} disabled={s.running}>
          Refresh
        </button>
      </section>

      <Meters />
      <PresetBar />

      <main>
        {s.chain.map((f, i) => (
          <FilterCard
            key={f.id}
            filter={f}
            onDragStart={() => setDragFrom(i)}
            onDrop={() => {
              if (dragFrom !== null && dragFrom !== i) s.moveFilter(dragFrom, i);
              setDragFrom(null);
            }}
          />
        ))}
        <select
          className="add"
          value=""
          onChange={(e) => e.target.value && s.addFilter(e.target.value as FilterKind)}
        >
          <option value="">+ Add filter…</option>
          {(Object.keys(FILTERS) as FilterKind[]).map((k) => (
            <option key={k} value={k}>
              {FILTERS[k].label}
            </option>
          ))}
        </select>
      </main>
    </div>
  );
}
