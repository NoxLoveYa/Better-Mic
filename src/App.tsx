import { useEffect, useRef, useState } from "react";
import { useStore } from "./store";
import { AddFilter } from "./components/AddFilter";
import { FilterCard } from "./components/FilterCard";
import { Icon } from "./components/Icon";
import { Meters } from "./components/Meters";
import { NvidiaPrompt } from "./components/NvidiaPrompt";
import { PresetMenu } from "./components/PresetMenu";
import { Select } from "./components/Select";
import { TitleBar } from "./components/TitleBar";
import { Toast } from "./components/Toast";

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
  const allFolded = s.chain.length > 0 && s.chain.every((f) => s.collapsed[f.id]);

  return (
    <>
      <TitleBar />
      <NvidiaPrompt />
      <div className="toolbar">
        <PresetMenu />
        <span className="spacer" />
        <button className="ghost" onClick={() => s.setAllCollapsed(!allFolded)}>
          {allFolded ? "Expand all" : "Collapse all"}
        </button>
        <AddFilter />
      </div>
      <div className="scroll">
        <div className="app">
          {s.error && (
            <div className="banner error" onClick={s.dismissError}>
              {s.error}
            </div>
          )}
          {s.devices && !cableFound && (
            <div className="banner warn">
              <span>
                VB-Cable (virtual microphone driver) isn't installed. It's donationware from vb-audio.com; the installer
                asks for administrator rights.
              </span>
              <button onClick={s.installCable} disabled={s.installingCable}>
                {s.installingCable ? "Installing…" : "Download & install VB-Cable"}
              </button>
            </div>
          )}

          <section className="panel devices">
            <label>
              <span className="label">Microphone</span>
              <Select
                disabled={s.running}
                value={s.input ?? ""}
                options={(s.devices?.inputs ?? []).map((d) => ({ value: d.id, label: d.name }))}
                onChange={s.setInput}
                placeholder={s.devices ? "No microphone found" : "Loading…"}
              />
            </label>
            <label>
              <span className="label">Output (virtual cable)</span>
              <Select
                disabled={s.running}
                value={s.output ?? ""}
                options={(s.devices?.outputs ?? []).map((d) => ({ value: d.id, label: d.name }))}
                onChange={s.setOutput}
                placeholder={s.devices ? "No output device found" : "Loading…"}
              />
            </label>
            <button onClick={() => s.refreshDevices()} disabled={s.running} title="Rescan audio devices" aria-label="Refresh devices">
              <Icon name="reset" />
              <span className="btn-text">Refresh</span>
            </button>
          </section>

          <span className="label chain-label">Signal chain · applied top to bottom</span>

          <ol className="chain">
            <li className="terminal">Microphone in</li>
            {s.chain.map((f, i) => (
              <FilterCard
                key={f.id}
                filter={f}
                index={i}
                count={s.chain.length}
                onDragStart={() => setDragFrom(i)}
                onDrop={() => {
                  if (dragFrom !== null && dragFrom !== i) s.moveFilter(dragFrom, i);
                  setDragFrom(null);
                }}
              />
            ))}
            <li className="terminal">Virtual cable out</li>
          </ol>
        </div>
      </div>
      <footer className="footer">
        <Toast />
        <Meters />
      </footer>
    </>
  );
}
