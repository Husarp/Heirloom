import { Archive, Copy, FileDiff, FileInput, FileWarning, Info, Pencil, Plus, RefreshCw, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { call } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { useStore } from "../app/store";
import { Dialog } from "../components/Dialog";
import { count, displayPath, shortWhen } from "../lib/format";
import { pickFolder } from "../lib/native";
import { useWizard } from "../screens/import/types";

/** „Kto edytuje?” (spec §4.20): a name from the archive's list, or a new one. No passwords. */
export function WhoEditsDialog() {
  const archive = useStore((s) => s.archive);
  const lastEditor = useStore((s) => s.app?.lastEditor ?? null);
  const startEditing = useStore((s) => s.startEditing);
  const cancel = useStore((s) => s.cancelWhoEdits);
  const editors = archive?.editors ?? [];
  const [chosen, setChosen] = useState<string | null>(
    editors.find((e) => e.name === lastEditor)?.name ?? editors[0]?.name ?? null,
  );
  const [adding, setAdding] = useState(editors.length === 0);
  const [newName, setNewName] = useState("");

  const name = adding ? newName.trim() : chosen;

  return (
    <Dialog width={440} onClose={cancel}>
      <div style={{ padding: "22px 24px 10px", display: "flex", flexDirection: "column", gap: 4 }}>
        <div className="serif" style={{ fontSize: 28, lineHeight: 1.15, fontWeight: 600 }}>
          Kto edytuje?
        </div>
        <div style={{ fontSize: 14, lineHeight: 1.5, color: "var(--text2)" }}>
          Imię zapisze się w historii zmian przy każdej zmianie. Bez haseł.
        </div>
      </div>
      <div style={{ padding: "6px 14px 10px", display: "flex", flexDirection: "column", gap: 2 }}>
        {editors.map((editor) => {
          const on = !adding && chosen === editor.name;
          const words = editor.name.trim().split(/\s+/);
          return (
            <button
              key={editor.name}
              className="row"
              style={{
                height: 52,
                padding: "0 12px",
                gap: 12,
                borderRadius: "var(--r-ctl)",
                background: on ? "var(--accent-soft)" : undefined,
                textAlign: "left",
              }}
              onClick={() => {
                setAdding(false);
                setChosen(editor.name);
              }}
              onDoubleClick={() => startEditing(editor.name)}
            >
              <span className={`radio${on ? " on" : ""}`} style={{ width: 18, height: 18 }} />
              <span className="avatar" style={{ width: 32, height: 32, fontSize: 11 }}>
                {(words[words.length - 1]?.[0] ?? "?").toUpperCase()}
              </span>
              <span className="col grow">
                <span style={{ fontSize: 15, fontWeight: 600 }}>{editor.name}</span>
                <span style={{ fontSize: 12, color: "var(--text3)" }}>
                  {editor.lastEdited ? `ostatnio: ${shortWhen(editor.lastEdited)}` : "jeszcze nie edytował(a)"}
                </span>
              </span>
            </button>
          );
        })}
        {adding ? (
          <input
            className="input"
            autoFocus
            placeholder="Imię, np. Ewa albo Ciocia Helena"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && newName.trim() && startEditing(newName.trim())}
            style={{ marginTop: 4 }}
          />
        ) : (
          <button
            className="row"
            style={{
              height: 44,
              marginTop: 4,
              gap: 10,
              padding: "0 12px",
              border: "1px dashed var(--border)",
              borderRadius: "var(--r-ctl)",
              fontSize: 14,
              color: "var(--text3)",
            }}
            onClick={() => setAdding(true)}
          >
            <Plus size={15} />
            Dodaj imię…
          </button>
        )}
      </div>
      <div className="dialog-foot">
        <span style={{ flex: 1, fontSize: 12, color: "var(--text3)" }}>Zapamiętam do zamknięcia programu</span>
        <button className="btn secondary" onClick={cancel}>
          Anuluj
        </button>
        <button className="btn primary" disabled={!name} onClick={() => name && startEditing(name)}>
          <Pencil size={14} />
          Włącz edycję
        </button>
      </div>
    </Dialog>
  );
}

/** „Plik … został zmieniony w innym programie” (spec §4.25). */
export function ConflictDialog() {
  const archive = useStore((s) => s.archive);
  const editor = useStore((s) => s.editor);
  const close = useStore((s) => s.closeConflict);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const unsaved = archive?.unsavedChanges ?? 0;
  const [busy, setBusy] = useState(false);
  const changes = count(unsaved, "niezapisaną zmianę", "niezapisane zmiany", "niezapisanych zmian");

  const reload = async () => {
    setBusy(true);
    try {
      const status = await call<ArchiveStatus>("archive.reload");
      // The undo steps and the last save's changes belonged to the old version.
      useStore.setState({ section: null, lastSave: null, leaveGuard: null });
      changed(status);
      close();
      notify("Wczytano nową wersję pliku.");
    } catch (e) {
      notify((e as Error).message, { kind: "err" });
    } finally {
      setBusy(false);
    }
  };

  const saveCopy = async () => {
    setBusy(true);
    try {
      const { path } = await call<{ path: string }>("archive.saveAsCopy");
      close();
      notify(`Zapisano kopię: ${path}`);
    } catch (e) {
      notify((e as Error).message, { kind: "err" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog width={500}>
      <div className="dialog-body">
        <div className="dialog-icon">
          <FileDiff size={19} />
        </div>
        <div className="dialog-title">Plik {archive?.dataFile} został zmieniony w innym programie.</div>
        <div className="dialog-text">
          Ktoś zmienił go po otwarciu archiwum w Heirloom. Masz {changes}
          {editor ? ` (${editor})` : ""}.
        </div>
      </div>
      <div style={{ padding: "0 24px 16px", display: "flex", flexDirection: "column", gap: 8, fontSize: 13, lineHeight: 1.45 }}>
        <div className="row" style={{ gap: 10, padding: "10px 12px", borderRadius: "var(--r-ctl)", border: "1px solid var(--border)" }}>
          <RefreshCw size={15} color="var(--text2)" style={{ flex: "none" }} />
          <span>
            <b>Wczytaj nową wersję</b> — Twoje zmiany ({unsaved}) zostaną odrzucone.
          </span>
        </div>
        <div
          className="row"
          style={{
            gap: 10,
            padding: "10px 12px",
            borderRadius: "var(--r-ctl)",
            border: "1.5px solid var(--accent)",
            background: "var(--accent-soft)",
          }}
        >
          <Copy size={15} color="var(--accent-text)" style={{ flex: "none" }} />
          <span>
            <b>Zapisz moje zmiany jako kopię</b> — powstanie nowy plik obok oryginału; oryginał zostaje bez zmian.
          </span>
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" disabled={busy} onClick={reload}>
          Wczytaj nową wersję
        </button>
        <button className="btn primary" disabled={busy} onClick={saveCopy}>
          Zapisz moje zmiany jako kopię
        </button>
      </div>
    </Dialog>
  );
}

/** The first save into a file made by another program asks first (design 17b, PLAN §11.2 rule 4); once per archive.
 *  „Anuluj” stands apart on the left, so it isn't read as one of the two choices. */
export function ForeignConfirmDialog() {
  const archive = useStore((s) => s.archive);
  const close = useStore((s) => s.closeForeignConfirm);
  const save = useStore((s) => s.save);
  const notify = useStore((s) => s.notify);
  const refreshApp = useStore((s) => s.refreshApp);
  const [busy, setBusy] = useState(false);
  if (!archive) return null;
  const backups = `${displayPath(archive.sidecar)}\\kopie`;

  const saveAsNew = async () => {
    const folder = await pickFolder("Nowe archiwum: wybierz pusty folder");
    if (!folder) return;
    setBusy(true);
    try {
      const result = await call<{ status: ArchiveStatus; copied: number; failed: number; missing: number }>("archive.saveAsNew", { folder });
      close();
      // A section still open stays open; its undo steps start again with the new archive.
      const open = useStore.getState().section;
      useStore.setState({ lastSave: null, section: open ? { ...open, base: result.status.undoDepth } : null });
      // The same family goes on in the new archive, and so does an import in progress (the backend keeps it too).
      if (useWizard.getState().archiveId === archive.archiveId) useWizard.getState().set({ archiveId: result.status.archiveId });
      useStore.getState().changed(result.status);
      await refreshApp();
      notify(`Utworzono nowe archiwum w ${folder}.`, {
        detail: [
          `Plik ${archive.dataFile} został bez zmian.`,
          result.copied ? `Skopiowano ${count(result.copied, "plik", "pliki", "plików")}.` : "",
          result.failed ? `${count(result.failed, "pliku", "plików", "plików")} nie udało się skopiować — zostały tam, gdzie były.` : "",
          result.missing ? `Brakuje ${count(result.missing, "pliku", "plików", "plików")}.` : "",
        ]
          .filter(Boolean)
          .join(" "),
      });
    } catch (e) {
      notify((e as Error).message, { kind: "err" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog width={540} onClose={close} labelledBy="foreign-title">
      <div style={{ padding: "22px 24px 16px", display: "flex", gap: 14 }}>
        <div className="dialog-icon" style={{ flex: "none" }}>
          <FileInput size={19} />
        </div>
        <div className="col" style={{ gap: 8 }}>
          <div id="foreign-title" className="dialog-title">
            {archive.origin ? `Ten plik pochodzi z programu ${archive.origin}.` : "Ten plik pochodzi z innego programu."}
          </div>
          <div className="dialog-text">
            Zmiany zapiszą się bezpośrednio w nim: <b style={{ fontWeight: 600, color: "var(--text)" }}>{archive.dataFile}</b>. Najpierw zrobimy kopię zapasową.
          </div>
        </div>
      </div>
      <div className="col" style={{ margin: "0 24px 16px 78px", gap: 6, fontSize: 13, lineHeight: 1.5, color: "var(--text2)" }}>
        <span className="row" style={{ gap: 8, alignItems: "flex-start" }}>
          <Archive size={14} style={{ marginTop: 3, flex: "none" }} />
          <span>
            Kopia trafi do <span className="mono selectable">{backups}</span>. Przywrócisz ją w Ustawienia › Kopie zapasowe.
          </span>
        </span>
        <span className="row" style={{ gap: 8, alignItems: "flex-start" }}>
          <ShieldCheck size={14} style={{ marginTop: 3, flex: "none" }} />
          Pola, których Heirloom nie zna, zostaną w pliku bez zmian.
        </span>
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" disabled={busy} onClick={close}>
          Anuluj
        </button>
        <span className="grow" />
        <button className="btn secondary" disabled={busy} onClick={saveAsNew} title="Plik z innego programu zostanie bez zmian">
          Zapisz jako nowe archiwum
        </button>
        <button
          className="btn primary"
          disabled={busy}
          onClick={async () => {
            close();
            await save(true);
          }}
        >
          Zapisz w tym pliku
        </button>
      </div>
    </Dialog>
  );
}

/** A generic question from the store (e.g. unsaved changes). */
export function AskDialog() {
  const ask = useStore((s) => s.ask);
  const setAsk = useStore((s) => s.setAsk);
  const notify = useStore((s) => s.notify);
  const [busy, setBusy] = useState(false);
  if (!ask) return null;
  return (
    <Dialog width={480} onClose={() => setAsk(null)}>
      <div className="dialog-body">
        {ask.icon && (
          <div className="dialog-icon" style={ask.icon === "info" ? { background: "var(--accent-soft)", color: "var(--accent-text)" } : undefined}>
            {ask.icon === "info" ? <Info size={19} /> : <FileWarning size={19} />}
          </div>
        )}
        <div className="dialog-title">{ask.title}</div>
        {ask.text && <div className="dialog-text">{ask.text}</div>}
      </div>
      <div className="dialog-foot">
        {ask.buttons.map((b) => (
          <button
            key={b.label}
            className={`btn ${b.kind ?? "secondary"}`}
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                setAsk(null);
                await b.run?.();
              } catch (e) {
                // The dialog is already gone, so a failure (e.g. „Odrzuć zmiany” when reloading fails) must show.
                notify((e as Error).message, { kind: "err" });
              } finally {
                setBusy(false);
              }
            }}
          >
            {b.label}
          </button>
        ))}
      </div>
    </Dialog>
  );
}
