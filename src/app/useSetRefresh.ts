// Archives opened together are edited in their own windows („Edytuj w jego archiwum”): when this window comes back,
// what was saved there (and decisions made in another window with the same set) is read again.

import { useEffect } from "react";
import { call } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { useStore } from "./store";

/** Mounted once, in App. */
export function useSetRefresh() {
  const combined = useStore((s) => s.archive?.combined != null);
  useEffect(() => {
    if (!combined) return;
    let busy = false;
    const back = async () => {
      if (busy || document.visibilityState !== "visible") return;
      busy = true;
      try {
        const before = useStore.getState().archive;
        const { changed, status } = await call<{ changed: string[]; status: ArchiveStatus }>("set.refresh");
        const linksChanged = before?.combined?.linked !== status.combined?.linked || before?.combined?.archives.length !== status.combined?.archives.length;
        if (changed.length === 0 && !linksChanged) {
          useStore.getState().setArchive(status);
          return;
        }
        useStore.getState().changed(status);
        useStore.getState().notify(changed.length ? `Odświeżono: ${changed.join(", ")} (zmiany z innego okna).` : "Odświeżono połączenia zapisane w innym oknie.");
      } catch {
        // The set was closed meanwhile; nothing to refresh.
      } finally {
        busy = false;
      }
    };
    document.addEventListener("visibilitychange", back);
    window.addEventListener("focus", back);
    return () => {
      document.removeEventListener("visibilitychange", back);
      window.removeEventListener("focus", back);
    };
  }, [combined]);
}
