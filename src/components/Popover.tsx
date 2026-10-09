import { useEffect, useRef, type ReactNode } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  /** The element that opens the menu; stays visible while it's open. */
  anchor: ReactNode;
  children: ReactNode;
  align?: "left" | "right";
  className?: string;
}

/** A dropdown panel under its anchor that closes on outside click or Escape. */
export function Popover({ open, onClose, anchor, children, align = "left", className = "" }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;

  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && close.current();
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close.current();
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div className="pop" ref={ref}>
      {anchor}
      {open && (
        <div className={`pop-menu ${align} ${className}`} role="menu">
          {children}
        </div>
      )}
    </div>
  );
}
