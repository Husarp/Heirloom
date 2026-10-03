import { useEffect, type ReactNode } from "react";

/** A modal dialog (spec §2.10). Esc closes it when `onClose` is given. */
export function Dialog({
  width = 480,
  onClose,
  labelledBy,
  children,
}: {
  width?: number;
  onClose?: () => void;
  /** The id of the title, read out when the dialog opens. */
  labelledBy?: string;
  children: ReactNode;
}) {
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose?.()}>
      <div className="dialog" style={{ width }} role="dialog" aria-modal="true" aria-labelledby={labelledBy}>
        {children}
      </div>
    </div>
  );
}
