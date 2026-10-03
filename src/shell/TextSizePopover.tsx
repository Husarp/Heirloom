import { useEffect, useRef, type RefObject } from "react";
import { call, type ApiError } from "../api/transport";
import type { AppState } from "../api/types";
import { useStore } from "../app/store";

const SIZES = [100, 110, 125, 150];

/** Text size 100–150 % (Ustawienia › Wygląd), also reachable from the top bar. `toggle` is the button that opens
 *  it: a click there closes it through that button, not through the click outside. */
export function TextSizePopover({ onClose, toggle }: { onClose: () => void; toggle: RefObject<HTMLElement | null> }) {
  const size = useStore((s) => s.app?.appearance.textSize ?? 100);
  const refreshApp = useStore((s) => s.refreshApp);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (ref.current && !ref.current.contains(target) && !toggle.current?.contains(target)) onClose();
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [onClose, toggle]);

  const choose = async (textSize: number) => {
    try {
      await call<AppState>("app.setAppearance", { textSize });
      await refreshApp();
    } catch (e) {
      useStore.getState().notify((e as ApiError).message, { kind: "err" });
    }
  };

  // Wide enough for its four buttons at any text size (it grows to the left, anchored at the right).
  return (
    <div ref={ref} className="popover" style={{ right: 0, top: 38, width: "max-content", minWidth: 230, padding: 12 }}>
      <div className="label-caps" style={{ marginBottom: 8 }}>
        Rozmiar tekstu
      </div>
      <div className="seg neutral full">
        {SIZES.map((s) => (
          <button key={s} className={s === size ? "on" : ""} onClick={() => choose(s)}>
            {s}%
          </button>
        ))}
      </div>
      {/* The slider in Ustawienia › Wygląd also allows sizes in between (105, 115…). */}
      {!SIZES.includes(size) && <div style={{ marginTop: 8, fontSize: 12, color: "var(--text3)" }}>Teraz: {size}%</div>}
    </div>
  );
}
