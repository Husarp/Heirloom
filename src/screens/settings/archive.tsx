// Ustawienia › Archiwum and the sections under it: settings files, backups, export and tools (spec §4.34).

import { FolderOpen, HardDriveDownload } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "../../api/transport";
import type { AppState, ArchiveStatus, OverlayInfo } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Segmented, SettingRow, Spinner, Toggle } from "../../components/bits";
import { bytes, count, displayPath } from "../../lib/format";
import { openPath, pickFiles, pickSavePath } from "../../lib/native";
import { discardMine, switchToInPlace, switchToOverlay } from "../../app/workMode";
import { lastWriteBackText, waitingText } from "../archive/workModes";
import { BusyDialog, CheckDialog, type CheckProblem } from "./dialogs";
import { failed, fileSafe, Keycap, saveSettings, Section, Select, showInFolder, today, whenDone } from "./parts";

interface Storage {
  photos: number;
  documents: number;
  backups: number;
  cache: number;
  data: number;
  total: number;
  disk: { name: string | null; free: number | null };
}

type Busy = { text: string; note?: string } | null;

export function ArchiveSections({ archive }: { archive: ArchiveStatus }) {
  const [busy, setBusy] = useState<Busy>(null);
  // Bumped after the cache is emptied, so the storage block is measured again.
  const [measured, setMeasured] = useState(0);
  return (
    <>
      <ArchiveSection archive={archive} measured={measured} />
      {archive.overlay && <OverlaySection archive={archive} />}
      <SettingsFilesSection archive={archive} />
      <BackupSection archive={archive} setBusy={setBusy} />
      <ExportSection archive={archive} setBusy={setBusy} />
      <ToolsSection onRebuilt={() => setMeasured((n) => n + 1)} />
      {busy && <BusyDialog text={busy.text} note={busy.note} />}
    </>
  );
}

function ArchiveSection({ archive, measured }: { archive: ArchiveStatus; measured: number }) {
  const whenSaved = useStore((s) => s.whenSaved);
  const stopEditing = useStore((s) => s.stopEditing);
  const startIn = useStore((s) => s.app?.appearance.startIn ?? "start");
  const version = archive.gedcomVersion;

  // A choice of this computer (aplikacja.json), like the appearance: on a shared archive everyone keeps their own.
  const setStartIn = async (value: "start" | "last") => {
    try {
      useStore.setState({ app: await call<AppState>("app.setAppearance", { startIn: value }) });
    } catch (e) {
      failed(e);
    }
  };

  const setMode = (mode: "edit" | "readOnly") => {
    if (mode === "edit") {
      saveSettings({ readOnly: false });
      return;
    }
    // Unsaved changes are saved (or dropped) first: a read-only archive can't be saved into.
    whenSaved(async () => {
      if (await saveSettings({ readOnly: true })) stopEditing();
    }, "Przełączasz archiwum na tylko do odczytu");
  };

  return (
    <Section id="archive" title="Archiwum">
      <SettingRow
        label="Sposób pracy"
        note={archive.workMode === "overlay" ? "Zmiany zapisują się u Ciebie, archiwum zostaje bez zmian." : "Zmiany zapisują się w tym archiwum."}
      >
        <Segmented
          size={28}
          value={archive.workMode === "overlay" ? "overlay" : "inPlace"}
          onChange={(mode) => {
            if (mode === (archive.workMode === "overlay" ? "overlay" : "inPlace")) return;
            if (mode === "overlay") switchToOverlay();
            else switchToInPlace();
          }}
          options={[
            { value: "inPlace", label: "Pracuj na archiwum" },
            { value: "overlay", label: "Nie ruszaj oryginału" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Po otwarciu archiwum" note="Ekran, widok drzewa i powiększenie zapamiętane osobno dla każdego archiwum, na tym komputerze">
        <Segmented
          size={28}
          value={startIn}
          onChange={setStartIn}
          options={[
            { value: "start", label: "Start" },
            { value: "last", label: "Ostatnie miejsce" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Folder archiwum" note={<span className="selectable">{displayPath(archive.root)}</span>}>
        <button className="btn secondary set-btn" onClick={() => openPath(archive.root).catch(failed)}>
          <FolderOpen size={15} />
          Otwórz folder
        </button>
      </SettingRow>
      <SettingRow label="Plik danych" note={archive.dataBytes != null ? `${archive.dataFile} · ${bytes(archive.dataBytes)}` : archive.dataFile}>
        <Keycap>{version?.startsWith("7") ? "GEDCOM 7" : version ? `GEDCOM ${version}` : "GEDCOM"}</Keycap>
      </SettingRow>
      {/* „Nie ruszaj oryginału”: the user's folder starts unlocked, and a lock on the archive stops the write-back. */}
      {!archive.overlay && (
        <SettingRow label="Tylko do odczytu" note="Nikt nie zmieni niczego w tym archiwum (ustawienie zapisane w archiwum)">
          <Toggle on={archive.readOnly} onChange={(on) => setMode(on ? "readOnly" : "edit")} />
        </SettingRow>
      )}
      <StorageBlock dataFile={archive.dataFile} measured={measured} />
    </Section>
  );
}

/** „Twoje zmiany” (only with „Nie ruszaj oryginału”): where they are, what waits to go into the archive. */
function OverlaySection({ archive }: { archive: ArchiveStatus }) {
  const startWriteBack = useStore((s) => s.startWriteBack);
  const startRebase = useStore((s) => s.startRebase);
  const undoWriteBack = useStore((s) => s.undoWriteBack);
  const { data: info, error } = useApi<OverlayInfo>("archive.overlayInfo");
  useEffect(() => {
    if (error) failed(error);
  }, [error]);
  const overlay = archive.overlay!;
  const nothing = info != null && info.pending === 0 && info.newFiles === 0;
  // Why moving the changes into the archive isn't possible now (the changes stay here).
  const blocked = !info
    ? null
    : info.originalMissing
      ? "Archiwum jest teraz niedostępne — przenieść zmiany można, gdy będzie dostępne."
      : info.lockedOriginal
        ? "Właściciel archiwum zablokował zmiany (Tylko do odczytu). Twoje zmiany zostają u Ciebie."
        : !info.writable
          ? "W archiwum nie da się nic zapisać (np. płyta lub zablokowany pendrive). Twoje zmiany zostają u Ciebie."
          : null;
  return (
    <Section id="overlay" title="Twoje zmiany">
      <SettingRow label="Folder" note={<span className="selectable">{displayPath(overlay.folder)}</span>}>
        <button className="btn secondary set-btn" onClick={() => openPath(overlay.folder).catch(failed)}>
          <FolderOpen size={15} />
          Otwórz folder
        </button>
      </SettingRow>
      <SettingRow
        label="Czeka na przeniesienie"
        note={
          <>
            {info ? waitingText(info) : "Sprawdzam…"}
            {blocked && (
              <>
                <br />
                {blocked}
              </>
            )}
          </>
        }
      >
        <button className="btn primary set-btn" disabled={!info || nothing || !!blocked} onClick={() => startWriteBack()}>
          Przenieś zmiany do archiwum…
        </button>
      </SettingRow>
      {overlay.originalChanged && !overlay.originalMissing && (
        <SettingRow label="Ktoś zmienił archiwum" note="Od ostatniego razu — wczytaj te zmiany, zanim przeniesiesz swoje.">
          <button className="btn secondary set-btn" onClick={startRebase}>
            Wczytaj zmiany
          </button>
        </SettingRow>
      )}
      <SettingRow label="Ostatnie przeniesienie" note={info ? lastWriteBackText(info.lastWriteBack) : "…"}>
        {info?.canUndoWriteBack && (
          <button className="btn secondary set-btn" onClick={undoWriteBack}>
            Cofnij
          </button>
        )}
      </SettingRow>
      <SettingRow label="Zacznij od nowa" note="Twoja wersja wraca do tego, co jest teraz w archiwum. Stary folder zostaje, tylko pod inną nazwą." last>
        <button className="btn secondary set-btn" disabled={!!overlay.originalMissing} onClick={discardMine}>
          Porzuć moje zmiany…
        </button>
      </SettingRow>
    </Section>
  );
}

/** „Zajęte miejsce”: a bar and a legend (spec §4.34, §5.20). */
function StorageBlock({ dataFile, measured }: { dataFile: string; measured: number }) {
  const { data, error } = useApi<Storage>("archive.storage", { measured });
  useEffect(() => {
    if (error) failed(error);
  }, [error]);
  if (!data) {
    return (
      <div className="storage">
        <span className="skel" style={{ width: 260, height: 14 }} />
        <span className="skel" style={{ height: 10 }} />
        <span className="skel" style={{ width: "70%", height: 12 }} />
      </div>
    );
  }
  const parts = [
    { label: "Zdjęcia (media/)", size: data.photos, color: "var(--accent)" },
    { label: "Dokumenty", size: data.documents, color: "var(--b4)" },
    { label: "Dane podręczne", size: data.cache, color: "var(--b5)" },
    { label: "Kopie zapasowe", size: data.backups, color: "var(--b3)" },
    { label: dataFile, size: data.data, color: "var(--text2)" },
  ];
  const { name, free } = data.disk;
  return (
    <div className="storage">
      <div style={{ fontSize: 13, color: "var(--text2)" }}>
        <b style={{ color: "var(--text)", fontWeight: 600 }}>Zajęte miejsce</b> {bytes(data.total)}
        {(name || free != null) && ` · ${name ? `dysk ${name} ` : ""}${free != null ? `(wolne ${bytes(free)})` : ""}`}
      </div>
      <div className="storage-bar">
        {parts
          .filter((p) => p.size > 0)
          .map((p) => (
            <span key={p.label} style={{ flexGrow: p.size, background: p.color }} title={`${p.label}: ${bytes(p.size)}`} />
          ))}
      </div>
      <div className="storage-legend">
        {parts.map((p) => (
          <span key={p.label}>
            <i style={{ background: p.color }} />
            {p.label} · <b>{bytes(p.size)}</b>
          </span>
        ))}
      </div>
    </div>
  );
}

function SettingsFilesSection({ archive }: { archive: ArchiveStatus }) {
  const notify = useStore((s) => s.notify);

  const exportSettings = async () => {
    const path = await pickSavePath(`Heirloom-ustawienia-${fileSafe(archive.name)}.json`, [{ name: "Ustawienia Heirlooma", extensions: ["json"] }]);
    if (!path) return;
    try {
      const result = await call<{ path: string }>("settings.export", { path });
      notify("Zapisano ustawienia w pliku.", { action: showInFolder(result.path) });
    } catch (e) {
      failed(e);
    }
  };

  const importSettings = async () => {
    const [path] = await pickFiles("Wczytaj ustawienia", [{ name: "Ustawienia Heirlooma", extensions: ["json"] }], false);
    if (!path) return;
    try {
      const app = await call<AppState>("settings.import", { path });
      useStore.setState((s) => ({ app, archive: app.archive ?? s.archive }));
      notify("Wczytano ustawienia.");
    } catch (e) {
      failed(e);
    }
  };

  return (
    <Section id="settingsFiles" title="Pliki ustawień Heirlooma">
      <SettingRow label="Ustawienia wyglądu" note="Wygląd programu i wybory z tej strony w jednym pliku">
        <button className="btn secondary set-btn" onClick={exportSettings}>
          Eksportuj do pliku
        </button>
      </SettingRow>
      <SettingRow label="Wczytaj ustawienia" last>
        <button className="btn secondary set-btn" onClick={importSettings}>
          Wczytaj z pliku…
        </button>
      </SettingRow>
    </Section>
  );
}

const KEEP = [5, 10, 20, 50, 100];

function BackupSection({ archive, setBusy }: { archive: ArchiveStatus; setBusy: (busy: Busy) => void }) {
  const notify = useStore((s) => s.notify);
  const whenSaved = useStore((s) => s.whenSaved);
  const keep = [...new Set([...KEEP, archive.backupsToKeep])].sort((a, b) => a - b);

  const createBackup = async () => {
    const path = await pickSavePath(`Heirloom-${fileSafe(archive.name)}-${today()}.zip`, [{ name: "Archiwum ZIP", extensions: ["zip"] }]);
    if (!path) return;
    setBusy({ text: "Tworzę kopię…", note: "Przy wielu zdjęciach może to chwilę potrwać." });
    try {
      const result = await call<{ path: string; bytes: number; files: number }>("archive.backup", { path });
      notify(`Utworzono kopię: ${count(result.files, "plik", "pliki", "plików")}, ${bytes(result.bytes)}.`, { action: showInFolder(result.path) });
    } catch (e) {
      failed(e);
    } finally {
      setBusy(null);
    }
  };

  return (
    <Section id="backup" title="Kopie zapasowe">
      <SettingRow label="Kopia całego archiwum" note="Plik .zip z danymi i zdjęciami">
        <button className="btn secondary set-btn" onClick={() => whenSaved(createBackup, "Kopia obejmuje tylko zapisane dane")}>
          <HardDriveDownload size={15} />
          Utwórz kopię (.zip)
        </button>
      </SettingRow>
      <SettingRow
        label="Kopia pliku danych przed każdym zapisem"
        note={archive.lastBackup ? `Ostatnia: ${whenDone(archive.lastBackup)}` : "Jeszcze nie było kopii"}
      >
        <Toggle on={archive.backupBeforeSave} onChange={(backupBeforeSave) => saveSettings({ backupBeforeSave })} />
      </SettingRow>
      <SettingRow label="Ile kopii trzymać" note="Starsze kopie pliku danych są usuwane" last>
        <Select
          label="Ile kopii trzymać"
          value={String(archive.backupsToKeep)}
          options={keep.map((n) => ({ value: String(n), label: String(n) }))}
          onChange={(value) => saveSettings({ backupsToKeep: Number(value) })}
        />
      </SettingRow>
    </Section>
  );
}

function ExportSection({ archive, setBusy }: { archive: ArchiveStatus; setBusy: (busy: Busy) => void }) {
  const notify = useStore((s) => s.notify);
  const whenSaved = useStore((s) => s.whenSaved);

  // The saved file is exported: unsaved changes are saved (or dropped) first.
  const exportGedzip = () =>
    whenSaved(async () => {
      const path = await pickSavePath(`${fileSafe(archive.name)}.gdz`, [{ name: "GEDZIP", extensions: ["gdz"] }]);
      if (!path) return;
      setBusy({ text: "Eksportuję GEDZIP…", note: "Dane i zdjęcia trafiają do jednego pliku." });
      try {
        const result = await call<{ path: string; bytes: number; files: number; missing: string[] }>("archive.exportGedzip", { path });
        const skipped = result.missing.length ? ` Brakuje ${count(result.missing.length, "pliku", "plików", "plików")} — pominięto.` : "";
        notify(`Wyeksportowano GEDZIP: dane i ${count(result.files, "plik", "pliki", "plików")}, ${bytes(result.bytes)}.${skipped}`, {
          action: showInFolder(result.path),
        });
      } catch (e) {
        failed(e);
      } finally {
        setBusy(null);
      }
    }, "Przed eksportem");

  return (
    <Section id="export" title="Eksport">
      <SettingRow label="GEDCOM 5.5.1" note="dla MyHeritage i Ancestry" soon>
        <button className="btn secondary set-btn">Eksportuj…</button>
      </SettingRow>
      <SettingRow label="GEDZIP" note="GEDCOM 7 + zdjęcia w jednym pliku" last>
        <button className="btn secondary set-btn" onClick={exportGedzip}>
          Eksportuj…
        </button>
      </SettingRow>
    </Section>
  );
}

function ToolsSection({ onRebuilt }: { onRebuilt: () => void }) {
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const { data: missing } = useApi<{ rows: unknown[] }>("media.missing");
  const [checking, setChecking] = useState(false);
  const [problems, setProblems] = useState<CheckProblem[] | null>(null);
  const [rebuilding, setRebuilding] = useState(false);
  const n = missing?.rows.length;

  const check = async () => {
    setChecking(true);
    try {
      setProblems(await call<CheckProblem[]>("archive.check"));
    } catch (e) {
      failed(e);
    } finally {
      setChecking(false);
    }
  };

  const rebuild = async () => {
    setRebuilding(true);
    try {
      const result = await call<{ files: number; bytes: number }>("archive.rebuildCache");
      notify(
        result.files > 0
          ? `Odbudowano dane podręczne: usunięto ${count(result.files, "miniaturę", "miniatury", "miniatur")} (${bytes(result.bytes)}), powstaną na nowo.`
          : "Odbudowano dane podręczne.",
      );
      onRebuilt();
    } catch (e) {
      failed(e);
    } finally {
      setRebuilding(false);
    }
  };

  return (
    <Section id="tools" title="Narzędzia">
      <SettingRow
        label="Brakujące pliki"
        note={n == null ? "Sprawdzam…" : n === 0 ? "Wszystkie pliki są na miejscu" : `${count(n, "pliku", "plików", "plików")} nie ma na dysku`}
      >
        <button className="btn secondary set-btn" disabled={!n} onClick={() => go({ name: "missingFiles" })}>
          Otwórz
        </button>
      </SettingRow>
      <SettingRow label="Zmniejsz zdjęcia bez utraty jakości" note="Nic nie dzieje się samo" soon>
        <button className="btn secondary set-btn">Zmniejsz…</button>
      </SettingRow>
      <SettingRow label="Sprawdź archiwum" note="Spójność danych i plików">
        <button className="btn secondary set-btn" disabled={checking} onClick={check}>
          {checking && <Spinner size={14} />}
          Sprawdź
        </button>
      </SettingRow>
      <SettingRow label="Odbuduj dane podręczne" note="Miniatury i indeks wyszukiwania" last>
        <button className="btn secondary set-btn" disabled={rebuilding} onClick={rebuild}>
          {rebuilding && <Spinner size={14} />}
          Odbuduj
        </button>
      </SettingRow>
      {problems && <CheckDialog problems={problems} onClose={() => setProblems(null)} />}
    </Section>
  );
}
