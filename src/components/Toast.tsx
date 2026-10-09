import { useEffect } from "react";
import { useStore } from "../store";
import { Icon } from "./Icon";

export function Toast() {
  const notice = useStore((s) => s.notice);
  const seq = useStore((s) => s.noticeSeq);
  const canUndo = useStore((s) => s.undo !== null);
  const { dismissNotice, undoChain } = useStore.getState();

  useEffect(() => {
    if (!notice) return;
    const t = setTimeout(dismissNotice, 4000 + notice.length * 40);
    return () => clearTimeout(t);
  }, [notice, seq, dismissNotice]);

  if (!notice) return null;
  return (
    <div className="toast" role="status" key={seq}>
      <span>{notice}</span>
      {canUndo && (
        <button className="toast-undo" onClick={undoChain}>
          <Icon name="undo" /> Undo
        </button>
      )}
      <button className="ghost icon" aria-label="Dismiss" onClick={dismissNotice}>
        <Icon name="close" />
      </button>
    </div>
  );
}
