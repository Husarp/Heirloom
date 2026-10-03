// Windows dialogs, links and files. In the app window they go through Tauri; in a normal browser (tests and
// design checks with heirloom-bridge) a simple prompt stands in for the file dialogs.

import { inTauri } from "../api/transport";

export async function pickFolder(title: string): Promise<string | null> {
  if (inTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const result = await open({ directory: true, title });
    return typeof result === "string" ? result : null;
  }
  return window.prompt(`${title}\n(ścieżka folderu)`)?.trim() || null;
}

export async function pickFiles(title: string, filters?: { name: string; extensions: string[] }[], multiple = true): Promise<string[]> {
  if (inTauri) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const result = await open({ multiple, title, filters });
    if (!result) return [];
    return Array.isArray(result) ? result : [result];
  }
  const typed = window.prompt(`${title}\n(ścieżki plików, rozdzielone średnikiem)`);
  return typed ? typed.split(";").map((s) => s.trim()).filter(Boolean) : [];
}

/** "Zapisz jako": where to write a new file (a backup, an export). */
export async function pickSavePath(defaultName: string, filters?: { name: string; extensions: string[] }[]): Promise<string | null> {
  if (inTauri) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    return (await save({ defaultPath: defaultName, filters })) ?? null;
  }
  return window.prompt(`Zapisz jako\n(pełna ścieżka pliku)`, defaultName)?.trim() || null;
}

export async function pickGedcom(): Promise<string | null> {
  const files = await pickFiles("Otwórz plik GEDCOM", [{ name: "GEDCOM", extensions: ["ged", "GED"] }], false);
  return files[0] ?? null;
}

/** Opens a web page in the system browser. */
export async function openUrl(url: string) {
  if (inTauri) {
    const { openUrl: open } = await import("@tauri-apps/plugin-opener");
    await open(url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}

/** Opens a file with its default program (e.g. a PDF reader). */
export async function openPath(path: string) {
  if (inTauri) {
    const { openPath: open } = await import("@tauri-apps/plugin-opener");
    await open(path);
  }
}

/** Shows a file in Explorer. */
export async function revealPath(path: string) {
  if (inTauri) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  }
}

/** Files dropped onto the window (Tauri gives real paths; a browser doesn't). */
export async function onFileDrop(handler: (paths: string[]) => void, hover?: (over: boolean) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  const unlisten = await getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "over" || payload.type === "enter") hover?.(true);
    else if (payload.type === "leave") hover?.(false);
    else if (payload.type === "drop") {
      hover?.(false);
      handler(payload.paths);
    }
  });
  return unlisten;
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
