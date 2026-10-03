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
          const unsaved = s.archive?.unsavedChanges ?? 0;
          // A section's draft (the personal data being typed) isn't a change in the archive yet, but it is work.
          const draft = s.leaveGuard;
          if (unsaved === 0 && !draft) return;
          event.preventDefault();
          const close = () => void win.destroy();
          s.setAsk({
            title: unsaved > 0 ? `Masz ${count(unsaved, "niezapisaną zmianę", "niezapisane zmiany", "niezapisanych zmian")}` : "Otwarta sekcja ma niezatwierdzone zmiany",
            text:
              unsaved > 0
                ? `Zamykasz Heirloom. Zapisać zmiany w pliku ${s.archive?.dataFile ?? ""}?${draft ? " To, co wpisano w otwartej sekcji, a nie zatwierdzono przyciskiem „Gotowe”, nie zostanie zapisane." : ""}`
                : "Zamykasz Heirloom. To, co wpisano w otwartej sekcji, nie zostanie zapamiętane.",
            icon: "warn",
            buttons: [
              { label: "Anuluj", kind: "ghost" },
              { label: "Zamknij bez zapisu", kind: unsaved > 0 ? "secondary" : "danger", run: close },
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
