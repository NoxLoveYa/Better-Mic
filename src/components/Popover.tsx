import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface Props {
  open: boolean;
  onClose: () => void;
  /** The element that opens the menu; stays visible while it's open. */
  anchor: ReactNode;
  children: ReactNode;
  align?: "left" | "right";
  /** Make the panel at least as wide as the anchor (for dropdowns). */
  matchWidth?: boolean;
  role?: "menu" | "listbox";
  className?: string;
}

/**
 * A dropdown panel under its anchor that closes on outside click, Escape, resize or page scroll.
 * It is positioned against the window and capped to the room below the anchor, and rendered at the top of the page,
 * so a scrolling or dimmed parent can neither clip it nor affect its backdrop blur.
 */
export function Popover({ open, onClose, anchor, children, align = "left", matchWidth, role = "menu", className = "" }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  const [place, setPlace] = useState<CSSProperties>({});

  useLayoutEffect(() => {
    if (!open || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    setPlace({
      top: r.bottom + 6,
      ...(align === "left" ? { left: r.left } : { right: window.innerWidth - r.right }),
      maxHeight: Math.max(120, window.innerHeight - r.bottom - 14),
      ...(matchWidth && { minWidth: r.width }),
    });
  }, [open, align, matchWidth]);

  useEffect(() => {
    if (!open) return;
    const inside = (t: EventTarget | null) => ref.current?.contains(t as Node) || menu.current?.contains(t as Node);
    const away = (e: MouseEvent) => !inside(e.target) && close.current();
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close.current();
    const scrolled = (e: Event) => !inside(e.target) && close.current();
    const resized = () => close.current();
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    window.addEventListener("scroll", scrolled, true);
    window.addEventListener("resize", resized);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", esc);
      window.removeEventListener("scroll", scrolled, true);
      window.removeEventListener("resize", resized);
    };
  }, [open]);

  return (
    <div className="pop" ref={ref}>
      {anchor}
      {open &&
        createPortal(
          <div className={`pop-menu ${className}`} style={place} role={role} ref={menu}>
            {children}
          </div>,
          document.body,
        )}
    </div>
  );
}
