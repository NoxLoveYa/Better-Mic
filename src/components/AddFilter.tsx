import { useState } from "react";
import { useStore } from "../store";
import { FILTERS, FILTER_HINTS, type FilterKind } from "../filters/schema";
import { Icon } from "./Icon";
import { Popover } from "./Popover";

export function AddFilter() {
  const addFilter = useStore((s) => s.addFilter);
  const [open, setOpen] = useState(false);

  return (
    <Popover
      open={open}
      onClose={() => setOpen(false)}
      align="right"
      className="add-menu"
      anchor={
        <button aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>
          <Icon name="plus" /> Add filter
        </button>
      }
    >
      {(Object.keys(FILTERS) as FilterKind[]).map((kind) => (
        <button
          key={kind}
          className="pm-item two"
          onClick={() => {
            addFilter(kind);
            setOpen(false);
          }}
        >
          <b>{FILTERS[kind].label}</b>
          <small>{FILTER_HINTS[kind]}</small>
        </button>
      ))}
    </Popover>
  );
}
