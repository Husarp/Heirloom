import { AppWindow, FileText, FileWarning, FolderOpen, FolderPlus, FolderX, Layers, X } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "../../api/transport";
import type { RecentArchive } from "../../api/types";
import { isSetPath, useStore } from "../../app/store";
import { BrandMark, rowButton } from "../../components/bits";
import { Dialog } from "../../components/Dialog";
import { count, people as peopleCount, shortWhen } from "../../lib/format";
import { pickFolder, pickGedcom } from "../../lib/native";
import { UpdateBanner } from "../../shell/UpdateBanner";
import { OpenTogether } from "./OpenTogether";
import "./archive.css";

/** Wybór archiwum (spec §4.15, design 17c), with „Utwórz archiwum z plików” as the main first-launch action
 *  („Uzupełnienie 3”); an empty archive is made the same way and filled by hand from its Start screen. */
export function ArchivePicker() {
  const app = useStore((s) => s.app);
  const archive = useStore((s) => s.archive);
  const openArchive = useStore((s) => s.openArchive);
  const openError = useStore((s) => s.openError);
  const refreshApp = useStore((s) => s.refreshApp);
  const openElsewhere = useStore((s) => s.openElsewhere);
  const [creating, setCreating] = useState(false);
  const [together, setTogether] = useState(false);
  const [errorShown, setErrorShown] = useState(false);
  const recent = app?.recent ?? [];

  useEffect(() => setErrorShown(openError != null), [openError]);

  const openFolder = async () => {
    const folder = await pickFolder("Otwórz folder archiwum");
    if (folder) await openArchive(folder);
  };
  const openFile = async () => {
    const file = await pickGedcom();
    if (file) await openArchive(file);
  };
  const forget = async (path: string) => {
    await call("recent.forget", { path });
    await refreshApp();
  };

  return (
    <div className="fullscreen">
      <UpdateBanner />
      <div className="picker">
        <div className="col" style={{ gap: 18, minWidth: 0 }}>
          <div className="row" style={{ gap: 12 }}>
            <BrandMark size={48} />
            <span className="serif" style={{ fontSize: 24, fontWeight: 500 }}>
              Heirloom
            </span>
          </div>
          <h1 className="serif" style={{ fontSize: 42, lineHeight: 1.1, fontWeight: 500 }}>
            {recent.length ? "Wybierz archiwum rodzinne" : "Zacznij archiwum rodzinne"}
          </h1>
          {recent.length > 0 ? (
            <div className="card">
              <div className="label-caps" style={{ padding: "12px 16px", borderBottom: "1px solid var(--border)" }}>
                Ostatnio otwierane
              </div>
              {recent.map((r) => (
                <RecentRow
                  key={r.path}
                  entry={r}
                  current={archive?.root === r.path || archive?.dataPath === r.path}
                  onOpen={() => openArchive(r.path)}
                  onWindow={() => openElsewhere(r.path)}
                  onForget={() => forget(r.path)}
                />
              ))}
            </div>
          ) : (
            <p style={{ fontSize: 17, lineHeight: 1.6, color: "var(--text2)", maxWidth: 560 }}>
              Masz teksty, zdjęcia i skany od osoby, która szuka w aktach? Zacznij od „Utwórz archiwum z plików” —
              Heirloom zrobi z nich drzewo rodziny. Masz już plik GEDCOM z innego programu? Otwórz go.
            </p>
          )}
        </div>
        <div className="col" style={{ gap: 10, paddingTop: recent.length ? 124 : 116 }}>
          <ActionCard main icon={<FolderPlus size={17} />} title="Utwórz archiwum z plików" text="wybierz folder, potem wczytaj odpowiedzi AI, zdjęcia i skany" onClick={() => setCreating(true)} />
          <ActionCard icon={<FolderOpen size={17} />} title="Otwórz folder…" text="z plikiem .ged i folderem media/" onClick={openFolder} />
          <ActionCard icon={<FileText size={17} />} title="Otwórz plik GEDCOM…" text=".ged · także z innych programów" onClick={openFile} />
          <ActionCard icon={<Layers size={17} />} title="Otwórz razem…" text="kilka archiwów jako jedno drzewo; pliki zostają osobno" onClick={() => setTogether(true)} />
          <p style={{ fontSize: 12, lineHeight: 1.55, color: "var(--text3)", marginTop: 6 }}>
            Twoje dane zostają w wybranym folderze: plik GEDCOM i folder media/. Heirloom działa bez internetu i nie wysyła
            niczego z archiwum. Z GitHubem łączy się tylko, gdy w Ustawieniach poprosisz o sprawdzenie aktualizacji.
          </p>
        </div>
      </div>
      {creating && <CreateDialog onClose={() => setCreating(false)} />}
      {together && <OpenTogether onClose={() => setTogether(false)} />}
      {errorShown && openError && <OpenErrorDialog message={openError.message} code={openError.code} onClose={() => setErrorShown(false)} onPick={openFolder} />}
    </div>
  );
}

function RecentRow({ entry, current, onOpen, onWindow, onForget }: { entry: RecentArchive; current: boolean; onOpen: () => void; onWindow: () => void; onForget: () => void }) {
  const isFile = entry.path.toLowerCase().endsWith(".ged");
  const isSet = entry.kind === "set" || isSetPath(entry.path);
  return (
    <div className={`list-row clickable recent${current ? " selected" : ""}`} {...rowButton(onOpen)} style={{ minHeight: 68, padding: "10px 16px", gap: 14 }}>
      <span className="icon-tile neutral" style={{ width: 40, height: 40 }}>
        {isSet ? <Layers size={18} /> : isFile ? <FileText size={18} /> : <FolderOpen size={18} />}
      </span>
      <span className="col grow" style={{ minWidth: 0 }}>
        <span className="row" style={{ gap: 8 }}>
          <span className="serif ellipsis" style={{ fontSize: 17, fontWeight: 600 }}>
            {entry.name}
          </span>
          {isSet && <span className="badge accent">zestaw{entry.archives ? ` · ${count(entry.archives, "archiwum", "archiwa", "archiwów")}` : ""}</span>}
        </span>
        <span className="mono ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
          {entry.path}
        </span>
      </span>
      <span className="col" style={{ alignItems: "flex-end", fontSize: 12, color: "var(--text2)" }}>
        <span>{entry.people ? peopleCount(entry.people) : "—"}</span>
        <span style={{ color: "var(--text3)" }}>otwarte {shortWhen(entry.openedAt)}</span>
      </span>
      <button
        className="icon-btn forget"
        title="Otwórz w nowym oknie"
        aria-label="Otwórz w nowym oknie"
        onClick={(e) => {
          e.stopPropagation();
          onWindow();
        }}
      >
        <AppWindow size={15} />
      </button>
      <button
        className="icon-btn forget"
        title="Usuń z listy (archiwum zostaje na dysku)"
        onClick={(e) => {
          e.stopPropagation();
          onForget();
        }}
      >
        <X size={15} />
      </button>
    </div>
  );
}

function ActionCard({ icon, title, text, onClick, main }: { icon: React.ReactNode; title: string; text: string; onClick: () => void; main?: boolean }) {
  return (
    <button className={`action-card${main ? " first-choice" : ""}`} onClick={onClick}>
      <span className="icon-tile">{icon}</span>
      <span className="col" style={{ textAlign: "left" }}>
        <span style={{ fontSize: 14, fontWeight: 600 }}>{title}</span>
        <span className="action-card-text">{text}</span>
      </span>
    </button>
  );
}

/** A new archive: a folder and a name, then the Import (an archive filled by hand starts the same way). */
function CreateDialog({ onClose }: { onClose: () => void }) {
  const createArchive = useStore((s) => s.createArchive);
  const openError = useStore((s) => s.openError);
  const [folder, setFolder] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  // The store's error may be left from opening another archive; only this dialog's own attempt is shown.
  const [tried, setTried] = useState(false);

  const choose = async () => {
    const picked = await pickFolder("Wybierz pusty folder na archiwum");
    if (picked) {
      setFolder(picked);
      if (!name) {
        const last = picked.split(/[\\/]/).filter(Boolean).pop() ?? "";
        setName(last);
      }
    }
  };

  const create = async () => {
    setBusy(true);
    setTried(true);
    const ok = await createArchive(folder, name || "Archiwum rodzinne");
    setBusy(false);
    if (ok) {
      onClose();
      useStore.getState().go({ name: "import" });
    }
  };

  return (
    <Dialog width={520} onClose={onClose}>
      <div className="dialog-body">
        <div className="dialog-title">Utwórz archiwum z plików</div>
        <div className="dialog-text">
          Najpierw wybierz pusty folder na archiwum rodziny: Heirloom utworzy w nim plik rodzina.ged i folder media/. Potem
          przejdziesz do Importu i upuścisz naraz odpowiedzi AI, zdjęcia, skany i notatki. Nic nie zapisze się bez Twojego
          zatwierdzenia. Wolisz dodawać osoby ręcznie? Pomiń import — ekran Start to umożliwia.
        </div>
        <label className="field">
          <span className="field-label">Folder archiwum</span>
          <div className="row" style={{ gap: 8 }}>
            <input className="input mono" style={{ fontSize: 13 }} value={folder} onChange={(e) => setFolder(e.target.value)} placeholder="np. D:\Heirloom\Rodzina Kowalskich" />
            <button className="btn secondary" onClick={choose}>
              Wybierz…
            </button>
          </div>
        </label>
        <label className="field">
          <span className="field-label">Nazwa archiwum</span>
          <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="np. Rodzina Kowalskich" />
        </label>
        {tried && openError && <div className="banner warn">{openError.message}</div>}
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" disabled={!folder.trim() || busy} onClick={create}>
          Utwórz i przejdź do importu
        </button>
      </div>
    </Dialog>
  );
}

/** Błąd otwierania (spec §4.16), with the variants the designer described. */
function OpenErrorDialog({ message, code, onClose, onPick }: { message: string; code: string; onClose: () => void; onPick: () => void }) {
  const title =
    code === "not_found"
      ? "Nie znaleziono archiwum"
      : code === "permission"
        ? "Brak dostępu do folderu"
        : code === "no_data_file"
          ? "To nie jest archiwum GEDCOM"
          : code === "several_data_files"
            ? "W folderze jest kilka plików GEDCOM"
            : code.startsWith("set_") || code === "not_a_set"
              ? "Nie udało się otworzyć zestawu"
              : "Nie udało się otworzyć archiwum";
  return (
    <Dialog width={480} onClose={onClose}>
      <div className="dialog-body">
        <div className="dialog-icon">{code === "permission" || code === "not_found" ? <FolderX size={19} /> : <FileWarning size={19} />}</div>
        <div className="dialog-title">{title}</div>
        <div className="dialog-text">{message}</div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onClose}>
          Zamknij
        </button>
        {(code === "permission" || code === "not_found" || code === "no_data_file" || code === "several_data_files") && (
          <button
            className="btn primary"
            onClick={() => {
              onClose();
              onPick();
            }}
          >
            Wybierz inny folder
          </button>
        )}
      </div>
    </Dialog>
  );
}
