import { useCallback, useEffect, useState, type FormEvent } from "react";
import { useStore } from "../store";
import { DEFAULT_SIGNATURE, PRESET_NAME, signature } from "../filters/chain";
import { Icon } from "./Icon";
import { Popover } from "./Popover";

export function PresetMenu() {
  const presets = useStore((s) => s.presets);
  const activePreset = useStore((s) => s.activePreset);
  const baseline = useStore((s) => s.presetBaseline);
  const sig = useStore((s) => signature(s.chain));
  const { loadPreset, savePreset, saveActivePreset, deletePreset, resetChain } = useStore.getState();

  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    setSaving(false);
    setName("");
    setConfirmDelete(null);
  }, []);

  const dirty = activePreset ? sig !== baseline : sig !== DEFAULT_SIGNATURE;
  const label = activePreset ?? (sig === DEFAULT_SIGNATURE ? "Default" : "Unsaved");

  // Save overwrites the active preset; with none active it asks for a name instead.
  const save = useCallback(() => {
    if (!activePreset) {
      setOpen(true);
      setSaving(true);
    } else if (dirty) {
      void saveActivePreset();
    }
  }, [activePreset, dirty, saveActivePreset]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        save();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [save]);

  const trimmed = name.trim();
  const existing = presets.find((p) => p.toLowerCase() === trimmed.toLowerCase());
  const invalid = trimmed !== "" && !PRESET_NAME.test(trimmed);
  const hint = invalid
    ? "Use letters, numbers, spaces, - and _ only."
    : existing
      ? `“${existing}” already exists. Saving replaces it.`
      : "";

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!trimmed || invalid) return;
    void savePreset(existing ?? trimmed).then(close);
  };

  return (
    <>
      <Popover
        open={open}
        onClose={close}
        className="presets-menu"
        anchor={
          <button className="preset-trigger" aria-haspopup="menu" aria-expanded={open} onClick={() => (open ? close() : setOpen(true))}>
            <span className="label">Preset</span>
            <span className="pt-name">{label}</span>
            {activePreset && dirty && <span className="pt-mod">Modified</span>}
            <Icon name="chevron" />
          </button>
        }
      >
        <div className="pm-list">
          {presets.length === 0 && <p className="pm-empty">No saved presets yet.</p>}
          {presets.map((p) =>
            confirmDelete === p ? (
              <div className="pm-row confirm" key={p}>
                <span>Delete “{p}”?</span>
                <button
                  className="danger"
                  onClick={() => {
                    setConfirmDelete(null);
                    void deletePreset(p);
                  }}
                >
                  Delete
                </button>
                <button className="ghost" onClick={() => setConfirmDelete(null)}>
                  Cancel
                </button>
              </div>
            ) : (
              <div className={`pm-row${p === activePreset ? " active" : ""}`} key={p} role="menuitem">
                <button
                  className="pm-load"
                  onClick={() => {
                    void loadPreset(p);
                    close();
                  }}
                >
                  <span className="pm-check">{p === activePreset && <Icon name="check" />}</span>
                  <span className="pm-name">{p}</span>
                </button>
                <button className="ghost icon" aria-label={`Delete ${p}`} title="Delete" onClick={() => setConfirmDelete(p)}>
                  <Icon name="trash" />
                </button>
              </div>
            ),
          )}
        </div>
        <div className="pm-sep" />
        {saving ? (
          <form className="pm-save" onSubmit={submit}>
            <input
              autoFocus
              placeholder="Name this preset"
              maxLength={40}
              value={name}
              onChange={(e) => setName(e.target.value)}
              aria-label="Preset name"
            />
            <button className="primary" type="submit" disabled={!trimmed || invalid}>
              {existing ? "Replace" : "Save"}
            </button>
            {hint && <small className={invalid ? "bad" : ""}>{hint}</small>}
          </form>
        ) : (
          <button className="pm-item" onClick={() => setSaving(true)}>
            <Icon name="plus" /> Save current chain as new preset…
          </button>
        )}
        <button
          className="pm-item"
          onClick={() => {
            resetChain();
            close();
          }}
        >
          <Icon name="reset" /> Reset to default chain
        </button>
      </Popover>
      <button
        className={dirty ? "toggled" : ""}
        disabled={!!activePreset && !dirty}
        onClick={save}
        title={activePreset ? `Save changes to “${activePreset}” (Ctrl+S)` : "Save as a preset (Ctrl+S)"}
      >
        {activePreset ? "Save" : "Save as…"}
      </button>
    </>
  );
}
