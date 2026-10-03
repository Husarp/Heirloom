import { useEffect, useState } from "react";
import { inTauri } from "../api/transport";
import { useStore } from "./store";

/** Applies the theme, text size, density and animation settings to the whole window. */
export function useAppearance() {
  const appearance = useStore((s) => s.app?.appearance);
  const [systemDark, setSystemDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const theme = appearance?.theme ?? "system";
  const dark = theme === "dark" || (theme === "system" && systemDark);
  const textSize = appearance?.textSize ?? 100;

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = dark ? "dark" : "light";
    root.classList.toggle("no-anim", appearance?.animations === false);
    root.classList.toggle("compact", appearance?.density === "compact");
  }, [dark, appearance?.animations, appearance?.density]);

  useEffect(() => {
    // The whole interface grows, like the browser zoom: the design is drawn in px at 100 %.
    if (inTauri) {
      import("@tauri-apps/api/webview")
        .then(({ getCurrentWebview }) => getCurrentWebview().setZoom(textSize / 100))
        .catch(() => {
          document.documentElement.style.zoom = String(textSize / 100);
        });
    } else {
      document.documentElement.style.zoom = String(textSize / 100);
    }
  }, [textSize]);
}

/** Whether the dark theme is on (for the canvas, which can't read CSS variables). */
export function useDark(): boolean {
  const theme = useStore((s) => s.app?.appearance.theme ?? "system");
  const [systemDark, setSystemDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setSystemDark(query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return theme === "dark" || (theme === "system" && systemDark);
}
