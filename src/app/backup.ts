// „Utwórz kopię zapasową” (Start, Settings) and „Przywróć z kopii…” (DESIGN A.7). The backup goes into the backup
// folder under a new name; the restore unpacks into a new folder and opens that archive.

import { call, type ApiError } from "../api/transport";
import type { ArchiveStatus, BackupResult } from "../api/types";
import { pickFiles, pickFolder, revealPath } from "../lib/native";
import { CANCELLED_TEXT, doneToast } from "../screens/archive/backupText";
import { withProgress } from "./progress";
import { useStore } from "./store";

const store = () => useStore.getState();

const showInFolder = (path: string) => ({ label: "Pokaż w folderze", run: () => void revealPath(path).catch(() => {}) });

/** The automatic backup of the archive as the user sees it (in „Nie ruszaj oryginału” their version). Only saved
 *  data is in it, so unsaved changes are saved (or dropped) first. `then` runs after a backup was made. */
export function makeBackup(then?: (result: BackupResult) => void) {
  store().whenSaved(async () => {
    try {
      const result = await withProgress("Tworzę kopię zapasową…", call<BackupResult>("archive.backupAuto", { scope: "archive" }), { cancellable: true });
      store().notify(doneToast(result), { action: showInFolder(result.path) });
      then?.(result);
    } catch (e) {
      const error = e as ApiError;
      store().notify(error.code === "cancelled" ? CANCELLED_TEXT : error.message, { kind: error.code === "cancelled" ? "info" : "err" });
    }
  }, "Kopia obejmuje tylko zapisane dane");
}

/** „Przywróć z kopii…”: restoring opens the restored archive in place of the open one, so unsaved changes come first.
 *  The folder is next to the open archive; where that can't be written, the user picks one. */
export function restoreFromBackup() {
  store().whenSaved(async () => {
    const zip = (await pickFiles("Wybierz kopię zapasową", [{ name: "Kopia zapasowa (.zip)", extensions: ["zip"] }], false))[0];
    if (!zip) return;
    const restore = (folder?: string) =>
      withProgress("Przywracam archiwum z kopii…", call<ArchiveStatus>("archive.restore", folder ? { zip, folder } : { zip }), {
        note: "Przy wielu zdjęciach może to chwilę potrwać.",
      });
    try {
      let status: ArchiveStatus;
      try {
        status = await restore();
      } catch (e) {
        if ((e as ApiError).code !== "permission") throw e;
        store().notify("W folderze obok archiwum nie da się nic zapisać. Wybierz inne miejsce.", { kind: "err" });
        const folder = await pickFolder("Gdzie umieścić przywrócone archiwum?");
        if (!folder) return;
        status = await restore(folder);
      }
      // The backend has installed it; opening it the usual way shows Start and refreshes everything.
      if (await store().openArchive(status.root)) store().notify(status.notice ?? "Przywrócono archiwum z kopii.");
    } catch (e) {
      store().notify((e as ApiError).message, { kind: "err" });
    }
  }, "Przywrócenie otworzy inne archiwum");
}
