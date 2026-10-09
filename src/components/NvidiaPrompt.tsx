import { useEffect } from "react";
import { useStore } from "../store";

const mb = (n: number) => `${Math.round(n / 1048576)} MB`;

const STATUS = {
  verify: "Checking NVIDIA's signature…",
  install: "Waiting for the NVIDIA installer. Finish it in its window.",
} as const;

export function NvidiaPrompt() {
  const { nvidiaPrompt, gpu, nvidia, nvidiaInstall, nvidiaError, installNvidia, closeNvidiaPrompt } = useStore();
  const busy = nvidiaInstall !== null;

  useEffect(() => {
    if (!nvidiaPrompt || busy) return;
    const esc = (e: KeyboardEvent) => e.key === "Escape" && closeNvidiaPrompt();
    document.addEventListener("keydown", esc);
    return () => document.removeEventListener("keydown", esc);
  }, [nvidiaPrompt, busy, closeNvidiaPrompt]);

  if (!nvidiaPrompt || !gpu || nvidia?.available) return null;

  const downloading = nvidiaInstall?.stage === "download";
  const pct = downloading && nvidiaInstall.total > 0 ? (nvidiaInstall.done / nvidiaInstall.total) * 100 : 0;

  return (
    <div className="backdrop" onMouseDown={() => !busy && closeNvidiaPrompt()}>
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="nv-title" onMouseDown={(e) => e.stopPropagation()}>
        <h2 id="nv-title">Install NVIDIA noise removal</h2>
        <p>
          <b>{gpu.name}</b> detected. NVIDIA's AI noise removal needs its Audio Effects runtime, which isn't installed yet. It's
          the same package OBS uses.
        </p>
        <ul>
          <li>Downloads about 600–800 MB from nvidia.com.</li>
          <li>Checks that the installer is signed by NVIDIA, then asks for administrator rights.</li>
        </ul>

        {busy && (
          <div className="progress-block">
            <div className="progress" aria-hidden>
              <div className={downloading ? "" : "indeterminate"} style={downloading ? { width: `${pct}%` } : undefined} />
            </div>
            <small>
              {downloading
                ? `Downloading… ${mb(nvidiaInstall.done)}${nvidiaInstall.total ? ` of ${mb(nvidiaInstall.total)}` : ""}`
                : STATUS[nvidiaInstall.stage as keyof typeof STATUS]}
            </small>
          </div>
        )}
        {nvidiaError && <div className="modal-error">{nvidiaError}</div>}

        <div className="modal-actions">
          <button disabled={busy} onClick={closeNvidiaPrompt}>
            Not now
          </button>
          <button className="primary" disabled={busy} onClick={installNvidia}>
            {busy ? "Installing…" : nvidiaError ? "Try again" : "Download & install"}
          </button>
        </div>
      </div>
    </div>
  );
}
