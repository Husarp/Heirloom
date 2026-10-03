import { useEffect } from "react";
import { inTauri } from "../api/transport";
import { count } from "../lib/format";
import { useStore } from "./store";

/** Closing the window with unsaved changes asks first (saving is manual): „Zapisz i zamknij”, „Zamknij bez zapisu”
 *  or „Anuluj”. Without changes the window closes at once. */
export function useCloseGuard() {
  useEffect(() => {
    if (!inTauri) return;
    let unlisten: (() => void) | null = null;
    let gone = false;
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      const win = getCurrentWindow();
      return win
        .onCloseRequested((event) => {
          const s = useStore.getState();
          if ((s.archive?.unsavedChanges ?? 0) === 0 && !s.leaveGuard) return;
          event.preventDefault();
          closeWhenSaved(() => void win.destroy());
        })
        .then((u) => {
          if (gone) u();
          else unlisten = u;
        });
    });
    return () => {
      gone = true;
      unlisten?.();
    };
  }, []);
}

/** Runs `close` at once when closing loses nothing; otherwise asks first. `update`: Heirloom closes to install
 *  version `update`, and only „Zapisz i zamknij” or „Anuluj” are offered (with an open section's draft, only „Wróć”)
 *  — an update never drops work. */
export function closeWhenSaved(close: () => void, update?: string) {
  const s = useStore.getState();
  const unsaved = s.archive?.unsavedChanges ?? 0;
  // A section's draft (the personal data being typed) isn't a change in the archive yet, but it is work.
  const draft = s.leaveGuard;
  if (unsaved === 0 && !draft) {
    close();
    return;
  }
  const closing = update ? `Heirloom zamknie się, żeby zainstalować wersję ${update}.` : "Zamykasz Heirloom.";
  // An open section's draft can't be saved from here: for an update it is confirmed or cancelled first.
  if (update && draft) {
    s.setAsk({
      title: "Otwarta sekcja ma niezatwierdzone zmiany",
      text: `${closing} Najpierw zatwierdź otwartą sekcję przyciskiem „Gotowe” albo ją anuluj.`,
      icon: "warn",
      buttons: [{ label: "Wróć", kind: "primary" }],
    });
    return;
  }
  s.setAsk({
    title: unsaved > 0 ? `Masz ${count(unsaved, "niezapisaną zmianę", "niezapisane zmiany", "niezapisanych zmian")}` : "Otwarta sekcja ma niezatwierdzone zmiany",
    text:
      unsaved > 0
        ? `${closing} Zapisać zmiany w pliku ${s.archive?.dataFile ?? ""}?${draft ? " To, co wpisano w otwartej sekcji, a nie zatwierdzono przyciskiem „Gotowe”, nie zostanie zapisane." : ""}`
        : `${closing} To, co wpisano w otwartej sekcji, nie zostanie zapamiętane.`,
    icon: "warn",
    buttons: [
      { label: "Anuluj", kind: "ghost" },
      ...(update ? [] : [{ label: "Zamknij bez zapisu", kind: unsaved > 0 ? ("secondary" as const) : ("danger" as const), run: close }]),
      ...(unsaved > 0
        ? [
            {
              label: "Zapisz i zamknij",
              kind: "primary" as const,
              run: async () => {
                if (await useStore.getState().save()) close();
              },
            },
          ]
        : []),
    ],
  });
}
