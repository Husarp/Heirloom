import { Check, Lock, LockOpen, Pencil, Save, Undo2 } from "lucide-react";
import { call } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { useStore } from "../app/store";
import { clock, count, displayPath } from "../lib/format";

/** One bar on every screen in edit mode (design 17a): who edits, the file and folder, the unsaved changes with
 *  Cofnij / Anuluj / Zapisz, and after a save „Cofnij zapis” and „Zakończ edycję”. */
export function EditModeBar() {
  const editor = useStore((s) => s.editor);
  const archive = useStore((s) => s.archive);
  const lastSave = useStore((s) => s.lastSave);
  const save = useStore((s) => s.save);
  const undo = useStore((s) => s.undo);
  const undoSave = useStore((s) => s.undoSave);
  const discardChanges = useStore((s) => s.discardChanges);
  const whenSaved = useStore((s) => s.whenSaved);
  const stopEditing = useStore((s) => s.stopEditing);
  const setAsk = useStore((s) => s.setAsk);
  const notify = useStore((s) => s.notify);
  if (!archive) return null;
  const unsaved = archive.unsavedChanges;

  const cancel = () =>
    setAsk({
      title: `Odrzucić ${count(unsaved, "niezapisaną zmianę", "niezapisane zmiany", "niezapisanych zmian")}?`,
      text: `Plik ${archive.dataFile} zostanie taki, jak po ostatnim zapisie, a edycja się zakończy.`,
      icon: "warn",
      buttons: [
        { label: "Wróć do edycji", kind: "ghost" },
        { label: "Odrzuć zmiany", kind: "danger", run: discardChanges },
      ],
    });

  return (
    <div className="edit-bar" role="status">
      <Pencil size={15} color="var(--accent-text)" style={{ flex: "none" }} />
      <strong>Tryb edycji</strong>
      <span className="edit-bar-who">· edytuje: {editor} · zmiany zapisują się w:</span>
      <span className="edit-bar-file" title={displayPath(archive.dataPath)}>
        <b>{archive.dataFile}</b> <span className="mono">({displayPath(archive.root)})</span>
      </span>
      <span className="grow" />
      {unsaved > 0 ? (
        <>
          <span className="row edit-bar-count">
            <span className="dot" />
            {count(unsaved, "niezapisana zmiana", "niezapisane zmiany", "niezapisanych zmian")}
          </span>
          <button className="btn ghost bar-btn" disabled={!archive.canUndo} onClick={() => undo().catch((e) => notify((e as Error).message, { kind: "err" }))} title="Cofnij ostatnią zmianę">
            <Undo2 size={15} />
            Cofnij <span className="kbd-hint">Ctrl Z</span>
          </button>
          <button className="btn secondary bar-btn" onClick={cancel}>
            Anuluj
          </button>
          <button className="btn primary bar-btn" onClick={() => save()}>
            <Save size={14} />
            Zapisz <span className="kbd-hint">Ctrl S</span>
          </button>
        </>
      ) : (
        <>
          {lastSave ? (
            <>
              <span className="row edit-bar-saved">
                <Check size={14} />
                Zapisano o {clock(lastSave.at)}
              </span>
              <button
                className="btn ghost bar-btn strong"
                onClick={() => useStore.getState().outsideSection(() => void undoSave().catch((e) => notify((e as Error).message, { kind: "err" })))}
              >
                <Undo2 size={15} />
                Cofnij zapis
              </button>
            </>
          ) : (
            <span style={{ color: "var(--text2)" }}>Brak zmian</span>
          )}
          <button className="btn secondary bar-btn" onClick={() => whenSaved(stopEditing, "Kończysz edycję")}>
            Zakończ edycję
          </button>
        </>
      )}
    </div>
  );
}

/** Read-only is a switch the family turns on in Ustawienia › Archiwum (design 17c); off by default. */
export function ReadOnlyBar() {
  const setArchive = useStore((s) => s.setArchive);
  const notify = useStore((s) => s.notify);
  const turnOff = async () => {
    try {
      const status = await call<ArchiveStatus>("archive.setSettings", { readOnly: false });
      setArchive(status);
      notify("Tryb tylko do odczytu wyłączony — możesz edytować.");
    } catch (e) {
      notify((e as Error).message, { kind: "err" });
    }
  };
  return (
    <div className="readonly-bar">
      <Lock size={15} color="var(--text2)" style={{ flex: "none" }} />
      <span className="grow">
        Tryb tylko do odczytu jest włączony <span style={{ color: "var(--text2)" }}>(Ustawienia › Archiwum)</span>. Edycja jest wyłączona dla tego
        archiwum.
      </span>
      <button className="btn secondary" style={{ height: 32 }} onClick={turnOff}>
        <LockOpen size={14} />
        Wyłącz
      </button>
    </div>
  );
}
