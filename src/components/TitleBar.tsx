import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useStore } from "../store";

const win = getCurrentWindow();

const Icon = ({ d }: { d: string }) => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
    <path d={d} />
  </svg>
);

export function TitleBar() {
  const { running, muted, toggleRun, toggleMute } = useStore();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const sync = () => win.isMaximized().then(setMaximized);
    sync();
    const off = win.onResized(sync);
    return () => {
      off.then((f) => f());
    };
  }, []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="brand">
        <span className={`dot${running ? " live" : ""}`} />
        <span className="name">Better Mic</span>
        <span className={`status${running ? " live-text" : ""}`}>{running ? "Live" : "Idle"}</span>
      </div>
      <div className="actions">
        <button className={muted ? "active" : ""} onClick={toggleMute}>
          {muted ? "Muted" : "Mute"}
        </button>
        <button className={running ? "danger" : "primary"} onClick={toggleRun}>
          {running ? "Stop" : "Start"}
        </button>
      </div>
      <div className="wc">
        <button aria-label="Minimize" onClick={() => win.minimize()}>
          <Icon d="M0 5h10" />
        </button>
        <button aria-label={maximized ? "Restore" : "Maximize"} onClick={() => win.toggleMaximize()}>
          <Icon d={maximized ? "M2.5 2.5v-2h7v7h-2M.5 2.5h7v7h-7z" : "M.5.5h9v9h-9z"} />
        </button>
        <button className="close" aria-label="Close" onClick={() => win.close()}>
          <Icon d="M0 0l10 10M10 0L0 10" />
        </button>
      </div>
    </div>
  );
}
