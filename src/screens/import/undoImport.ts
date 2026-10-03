import { useStore } from "../../app/store";

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
