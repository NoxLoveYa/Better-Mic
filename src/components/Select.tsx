import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Icon } from "./Icon";
import { Popover } from "./Popover";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface Props {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

/** A dropdown in the app's own style, replacing the browser's native <select> list. Keyboard: arrows, Home/End, Enter. */
export function Select({ value, options, onChange, disabled, placeholder = "Select…" }: Props) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const list = useRef<HTMLDivElement>(null);
  const isOpen = open && !disabled;
  const current = options.find((o) => o.value === value);

  const enabled = (i: number) => i >= 0 && i < options.length && !options[i].disabled;
  const step = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < options.length; i += dir) if (enabled(i)) return i;
    return from;
  };

  const show = () => {
    const i = options.findIndex((o) => o.value === value);
    setActive(enabled(i) ? i : step(-1, 1));
    setOpen(true);
  };

  const pick = (i: number) => {
    if (!enabled(i)) return;
    setOpen(false);
    if (options[i].value !== value) onChange(options[i].value);
  };

  useEffect(() => {
    if (isOpen) list.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: "nearest" });
  }, [isOpen, active]);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!isOpen) show();
      else setActive(step(active, e.key === "ArrowDown" ? 1 : -1));
    } else if (!isOpen) {
      return;
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      setActive(e.key === "Home" ? step(-1, 1) : step(options.length, -1));
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(active);
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  return (
    <Popover
      open={isOpen}
      onClose={() => setOpen(false)}
      matchWidth
      role="listbox"
      className="select-menu"
      anchor={
        <button
          type="button"
          role="combobox"
          className="select"
          aria-haspopup="listbox"
          aria-expanded={isOpen}
          aria-activedescendant={isOpen && active >= 0 ? `${id}-${active}` : undefined}
          disabled={disabled}
          onClick={() => (isOpen ? setOpen(false) : show())}
          onKeyDown={onKeyDown}
        >
          <span className={`select-value${current ? "" : " empty"}`}>{current?.label ?? placeholder}</span>
          <Icon name="chevron" />
        </button>
      }
    >
      {/* Keep focus on the trigger while clicking, and stop a surrounding <label> from re-clicking the trigger. */}
      <div ref={list} onMouseDown={(e) => e.preventDefault()} onClick={(e) => e.preventDefault()}>
        {options.length === 0 && <p className="pm-empty">Nothing to choose from.</p>}
        {options.map((o, i) => (
          <button
            type="button"
            key={o.value}
            id={`${id}-${i}`}
            role="option"
            className="opt"
            aria-selected={o.value === value}
            data-active={i === active}
            disabled={o.disabled}
            tabIndex={-1}
            title={o.label}
            onMouseEnter={() => enabled(i) && setActive(i)}
            onClick={() => pick(i)}
          >
            <span className="pm-check">{o.value === value && <Icon name="check" />}</span>
            <span className="pm-name">{o.label}</span>
          </button>
        ))}
      </div>
    </Popover>
  );
}
