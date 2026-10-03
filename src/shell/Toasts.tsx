import { CircleAlert, CircleCheck, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore, type Toast } from "../app/store";

/** Short messages (design 17f): bottom right, newest at the bottom, at most 3. Without an action a message goes after
 *  6 s, with one after 10 s; pointing at it or focusing it stops the clock. Read out politely by screen readers. */
export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <ToastItem key={t.id} toast={t} />
      ))}
    </div>
  );
}

function ToastItem({ toast: t }: { toast: Toast }) {
  const dismiss = useStore((s) => s.dismissToast);
  const actions = [...(t.action ? [t.action] : []), ...(t.actions ?? [])];
  const [paused, setPaused] = useState(false);
  const left = useRef(actions.length ? 10_000 : 6_000);
  // Two buttons (or a detail line and a button) would squeeze the text into a narrow column: they go under it.
  const stacked = actions.length >= 2 || (!!t.detail && actions.length > 0);

  useEffect(() => {
    if (paused) return;
    const started = Date.now();
    const timer = window.setTimeout(() => dismiss(t.id), left.current);
    return () => {
      window.clearTimeout(timer);
      left.current -= Date.now() - started;
    };
  }, [paused, dismiss, t.id]);

  return (
    <div
      role="status"
      className={`toast${t.kind === "err" ? " err" : ""}${stacked ? " stacked" : ""}`}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      {t.kind === "err" ? <CircleAlert size={17} /> : <CircleCheck size={17} />}
      <div className="toast-body">
        <span className="toast-title">{t.text}</span>
        {t.detail && <span className="toast-detail">{t.detail}</span>}
      </div>
      <div className="toast-actions">
        {actions.map((a) => (
          <button
            key={a.label}
            className="toast-action"
            onClick={() => {
              a.run();
              dismiss(t.id);
            }}
          >
            {a.label}
          </button>
        ))}
      </div>
      <button className="toast-close" onClick={() => dismiss(t.id)} aria-label="Zamknij" title="Zamknij">
        <X size={15} />
      </button>
    </div>
  );
}
