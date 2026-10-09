import { useStore } from "../store";

const FLOOR = -60;
const pct = (db: number) => Math.max(0, Math.min(100, ((db - FLOOR) / -FLOOR) * 100));

function Bar({ label, peak, rms }: { label: string; peak: number; rms: number }) {
  return (
    <div className="meter">
      <span>{label}</span>
      <div className="track">
        <div className="mask" style={{ width: `${100 - pct(rms)}%` }} />
        <div className="peak" style={{ left: `${pct(peak)}%` }} />
      </div>
      <span className="db">{peak > FLOOR ? `${peak.toFixed(0)} dB` : "−∞"}</span>
    </div>
  );
}

export function Meters() {
  const m = useStore((s) => s.meter);
  return (
    <section className="panel meters">
      <div className="meters-head">
        <span className="label">Levels</span>
        {m && <small>{m.buffered_ms.toFixed(0)} ms buffered</small>}
      </div>
      <Bar label="In" peak={m?.in_peak ?? -120} rms={m?.in_rms ?? -120} />
      <Bar label="Out" peak={m?.out_peak ?? -120} rms={m?.out_rms ?? -120} />
    </section>
  );
}
