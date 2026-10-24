import { useEffect, useLayoutEffect, useState } from "react";
import { inTauri } from "../api/transport";
import { knownAccent, knownScheme } from "./colorLooks";
import { useStore } from "./store";

/** The branch colour palette of the open archive (Ustawienia › Drzewo › Kolory gałęzi): "light" or "bright"; the
 *  default, „Stonowane”, is no `data-palette` at all. */
export function useBranchPalette(): "light" | "bright" | undefined {
  const palette = useStore((s) => s.archive?.display?.branchPalette);
  return palette === "light" || palette === "bright" ? palette : undefined;
}

/** The colour scheme and the accent colour (Ustawienia › Wygląd) as one key, for what has to be rebuilt when either
 *  changes (a canvas that bakes the colours in). */
export function useColorLook(): string {
  const scheme = useStore((s) => knownScheme(s.app?.appearance.scheme));
  const accent = useStore((s) => knownAccent(s.app?.appearance.accent));
  return `${scheme}/${accent}`;
}

/** Applies the theme, colour scheme, accent, text size, density and animation settings to the whole window. */
export function useAppearance() {
  const appearance = useStore((s) => s.app?.appearance);
  const palette = useBranchPalette();
  const scheme = useStore((s) => knownScheme(s.app?.appearance.scheme));
  const accent = useStore((s) => knownAccent(s.app?.appearance.accent));
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

  // Before any effect of the screens below, so the tree can read the new colours when it recolours itself. The default
  // scheme and accent are no attribute at all (tokens.css).
  useLayoutEffect(() => {
    const root = document.documentElement;
    if (palette) root.dataset.palette = palette;
    else delete root.dataset.palette;
    if (scheme !== "museum") root.dataset.scheme = scheme;
    else delete root.dataset.scheme;
    if (accent !== "default") root.dataset.accent = accent;
    else delete root.dataset.accent;
  }, [palette, scheme, accent]);

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
