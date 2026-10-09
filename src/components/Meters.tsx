import { useStore } from "../store";

const FLOOR = -60;
const pct = (db: number) => Math.max(0, Math.min(100, ((db - FLOOR) / -FLOOR) * 100));
const zone = (db: number) => (db > -9 ? "red" : db > -20 ? "amber" : "green");
const TICKS = Array.from({ length: 13 }, (_, i) => FLOOR + i * 5);

function Bar({ label, peak, rms }: { label: string; peak: number; rms: number }) {
  return (
    <div className="meter">
      <span>{label}</span>
      <div className="track">
        <div className="lit" style={{ clipPath: `inset(0 ${100 - pct(rms)}% 0 0)` }} />
        {peak > FLOOR && <div className={`peak ${zone(peak)}`} style={{ left: `${pct(peak)}%` }} />}
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
        {m && (
          <small
            title={
              m.exclusive
                ? "The cable is opened in exclusive mode, so Discord's screen-share audio can't pick up your voice a second time."
                : "The cable is on the shared mixer, so a whole-screen share can pick up your voice twice. Close any other app using the cable, then Stop and Start."
            }
          >
            {m.buffered_ms.toFixed(0)} ms buffered · {m.exclusive ? "exclusive" : "shared"} cable
          </small>
        )}
      </div>
      <Bar label="In" peak={m?.in_peak ?? -120} rms={m?.in_rms ?? -120} />
      <Bar label="Out" peak={m?.out_peak ?? -120} rms={m?.out_rms ?? -120} />
      <div className="meter scale" aria-hidden>
        <span />
        <div className="ticks">
          {TICKS.map((t) => (
            <span key={t} style={{ left: `${pct(t)}%` }}>
              {t}
            </span>
          ))}
        </div>
        <span />
      </div>
    </section>
  );
}
