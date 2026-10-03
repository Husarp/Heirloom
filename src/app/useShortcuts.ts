import { useEffect, useRef } from "react";
import { useStore } from "./store";

function typing(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  return el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName);
}

// A screen with its own form (the person editor) handles Ctrl S itself; otherwise Ctrl S saves the archive.
let saveHandler: (() => void) | null = null;

export function useSaveHandler(handler: () => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const run = () => ref.current();
    saveHandler = run;
    return () => {
      if (saveHandler === run) saveHandler = null;
    };
  }, []);
}

/** App-wide keys (Settings › O programie lists them): Ctrl K search, Ctrl E edit mode, Ctrl S save, Ctrl Z / Ctrl Y
 *  undo and redo, Alt ← / → back and forward. Screens add their own (the tree's arrows). */
export function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      if (s.phase !== "ready") return;
      const ctrl = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (ctrl && key === "k") {
        e.preventDefault();
        s.setPalette(!s.paletteOpen);
      } else if (ctrl && key === "e") {
        e.preventDefault();
        if (s.mode === "edit") s.whenSaved(s.stopEditing, "Kończysz edycję");
        else s.requireEdit();
      } else if (ctrl && key === "s") {
        e.preventDefault();
        if (saveHandler) saveHandler();
        else if (s.mode === "edit" && (s.archive?.unsavedChanges ?? 0) > 0) s.save();
      } else if (ctrl && key === "z" && !e.shiftKey && !typing(e.target)) {
        if (s.mode === "edit" && s.archive?.canUndo) {
          e.preventDefault();
          s.undo();
        }
      } else if (ctrl && (key === "y" || (key === "z" && e.shiftKey)) && !typing(e.target)) {
        if (s.mode === "edit" && s.archive?.canRedo) {
          e.preventDefault();
          s.redo();
        }
      } else if (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        // Not from under a dialog (a question, „Kto edytuje?”): it would be answered on another screen.
        if (s.ask || s.whoEditsOpen || s.conflictOpen || s.foreignConfirmOpen || s.paletteOpen) return;
        e.preventDefault();
        if (e.key === "ArrowLeft") s.goBack();
        else s.goForward();
      }
    };
    // Mouse back/forward buttons.
    const onMouse = (e: MouseEvent) => {
      const s = useStore.getState();
      if (s.ask || s.whoEditsOpen || s.conflictOpen || s.foreignConfirmOpen || s.paletteOpen) return;
      if (e.button === 3) s.goBack();
      if (e.button === 4) s.goForward();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mouseup", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mouseup", onMouse);
    };
  }, []);
}
