import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { useStore } from "../../app/store";
import type { PastImport } from "./types";

/** „Cofnij import” takes a whole batch out of the data, so it asks first. The undo is an unsaved change like any
 *  other: until Zapisz, Ctrl Z brings the import back. */
export function askUndoImport(batch: string, run: () => void) {
  useStore.getState().setAsk({
    title: `Cofnąć import „${batch}”?`,
    text: "Nowe osoby z tej paczki znikną, a połączone wrócą do stanu sprzed importu. Skopiowane pliki zostaną w folderze media/. Zmiana czeka na zapis — do tego czasu można ją cofnąć (Ctrl Z).",
    icon: "warn",
    buttons: [
      { label: "Anuluj", kind: "ghost" },
      { label: "Cofnij import", kind: "danger", run },
    ],
  });
}

/** Takes the batch back (after askUndoImport and „Kto edytuje?”). An import already taken back — from the toast or the
 *  Done screen — says so, instead of reporting a change that did nothing. */
export async function undoImport(batch: string): Promise<void> {
  const { changed, notify } = useStore.getState();
  try {
    const past = await call<PastImport[]>("import.history");
    const entry = past.find((h) => h.name === batch);
    if (entry?.active === false) {
      notify(entry.undone ? "Ten import jest już cofnięty." : "Nie ma czego cofnąć — wszystko z tego importu zmieniono później.");
      return;
    }
    const status = await call<ArchiveStatus>("history.undo", { batch });
    changed(status);
    notify("Import cofnięty — zmiana czeka na zapis.");
  } catch (e) {
    notify((e as ApiError).message, { kind: "err" });
  }
}
