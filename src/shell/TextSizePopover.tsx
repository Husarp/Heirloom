import { useEffect, useRef } from "react";
import { call } from "../api/transport";
import type { AppState } from "../api/types";
import { useStore } from "../app/store";

const SIZES = [100, 110, 125, 150];

/** Text size 100–150 % (Ustawienia › Wygląd), also reachable from the top bar. */
export function TextSizePopover({ onClose }: { onClose: () => void }) {
  const size = useStore((s) => s.app?.appearance.textSize ?? 100);
  const refreshApp = useStore((s) => s.refreshApp);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [onClose]);

  const choose = async (textSize: number) => {
    await call<AppState>("app.setAppearance", { textSize });
    await refreshApp();
  };

  return (
    <div ref={ref} className="popover" style={{ right: 0, top: 38, width: 230, padding: 12 }}>
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
    </div>
  );
}
