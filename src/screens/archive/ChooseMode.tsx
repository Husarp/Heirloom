import { Check, FolderCog, FolderOpen, Info, PenLine, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { type ApiError } from "../../api/transport";
import type { ModeInfo } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { applyWorkMode, askPendingSwitch } from "../../app/workMode";
import { displayPath } from "../../lib/format";
import { pickFolder } from "../../lib/native";
import { changesFolderShown, defaultMode, editorsNote, modeNote, type WorkMode } from "./workModes";
import "./archive.css";

/** „Jak chcesz pracować z tym archiwum?” (DESIGN A.5): asked once for every archive Heirloom didn't make on this
 *  computer, before anything is written. „Dalej” goes on to the first-open page or the archive. */
export function ChooseMode() {
  const archive = useStore((s) => s.archive);
  const finish = useStore((s) => s.finishChooseMode);
  const closeArchive = useStore((s) => s.closeArchive);
  const { data: info, error } = useApi<ModeInfo>("archive.modeInfo");
  const [picked, setPicked] = useState<WorkMode | null>(null);
  const [parent, setParent] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [working, setWorking] = useState(false);

  const mode = info ? (info.writable ? (picked ?? defaultMode(info)) : "overlay") : null;
  const notes = info ? [modeNote(info), editorsNote(info.editors)].filter((n): n is string => !!n) : [];

  const choose = async () => {
    if (!mode) return;
    setProblem(null);
    setWorking(true);
    try {
      const status = await applyWorkMode(mode, mode === "overlay" ? parent : null);
      finish(status);
    } catch (e) {
      const err = e as ApiError;
      if (err.code === "pending_changes") await askPendingSwitch();
      else setProblem(err.message);
      // A folder that can't be written has just been found out: only the other way is left.
      if (err.code === "not_writable") setPicked("overlay");
    } finally {
      setWorking(false);
    }
  };

  const changeFolder = async () => {
    const folder = await pickFolder("Gdzie trzymać Twoje zmiany?");
    if (folder) setParent(folder);
  };

  return (
    <div className="fullscreen">
      <div className="first-open choose-mode">
        <div className="col" style={{ gap: 4 }}>
          <span className="mono" style={{ fontSize: 13, color: "var(--text3)" }}>
            {archive?.name} · {archive ? displayPath(archive.root) : ""} · {archive?.dataFile}
          </span>
          <h1 className="serif" style={{ fontSize: 36, lineHeight: 1.15, fontWeight: 500 }}>
            Jak chcesz pracować z tym archiwum?
          </h1>
        </div>

        {error && <div className="banner err">{error.message}</div>}

        <div className="mode-cards" role="radiogroup" aria-label="Sposób pracy">
          <ModeCard
            selected={mode === "inPlace"}
            disabled={info != null && !info.writable}
            onPick={() => setPicked("inPlace")}
            icon={<PenLine size={18} />}
            title="Pracuj na archiwum"
            chip="Gdy to Twoje archiwum"
          >
            <p>
              Zmiany zapisują się w tym archiwum: w pliku {archive?.dataFile} i w folderze media. Przed każdym zapisem Heirloom zachowuje kopię pliku danych, a
              swoje pliki (ustawienia, historię zmian, kopie) trzyma w jednym folderze .heirloom.
            </p>
          </ModeCard>
          <ModeCard
            selected={mode === "overlay"}
            onPick={() => setPicked("overlay")}
            icon={<FolderCog size={18} />}
            title="Nie ruszaj oryginału"
            chip="Gdy archiwum jest czyjeś, wspólne albo na pendrivie"
          >
            <p>Archiwum zostanie dokładnie takie, jakie jest. Twoje zmiany, nowe zdjęcia i importy Heirloom zapisze osobno, w folderze:</p>
            <div className="mode-folder">
              <FolderOpen size={15} style={{ flex: "none" }} />
              <span className="grow selectable" style={{ minWidth: 0, overflowWrap: "anywhere" }}>
                {info ? changesFolderShown(info, parent) : "…"}
              </span>
              <button
                className="btn secondary sm"
                onClick={(e) => {
                  e.stopPropagation();
                  void changeFolder();
                }}
              >
                Zmień…
              </button>
            </div>
            <p>Kiedy zechcesz, przeniesiesz je do archiwum.</p>
          </ModeCard>
        </div>

        {notes.map((note) => (
          <div key={note} className="banner info">
            {info && !info.writable && note === notes[0] ? <TriangleAlert size={16} color="var(--warn)" style={{ flex: "none" }} /> : <Info size={16} color="var(--text2)" style={{ flex: "none" }} />}
            <span>{note}</span>
          </div>
        ))}
        {problem && (
          <div className="banner err" role="alert">
            <TriangleAlert size={16} color="var(--err)" style={{ flex: "none" }} />
            <span>{problem}</span>
          </div>
        )}

        <div className="row" style={{ gap: 12, marginTop: 4, alignItems: "center" }}>
          <span className="grow" style={{ fontSize: 13, color: "var(--text3)" }}>
            Możesz to później zmienić w Ustawienia › Archiwum.
          </span>
          <button className="link" style={{ fontSize: 13 }} disabled={working} onClick={() => void closeArchive()}>
            Nie teraz — wróć do listy archiwów
          </button>
          <button className="btn primary" disabled={!info || working} onClick={() => void choose()}>
            Dalej
          </button>
        </div>
      </div>
    </div>
  );
}

function ModeCard({
  selected,
  disabled,
  onPick,
  icon,
  title,
  chip,
  children,
}: {
  selected: boolean;
  disabled?: boolean;
  onPick: () => void;
  icon: React.ReactNode;
  title: string;
  chip: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={`mode-card${selected ? " selected" : ""}${disabled ? " disabled" : ""}`}
      role="radio"
      aria-checked={selected}
      aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      onClick={() => !disabled && onPick()}
      onKeyDown={(e) => {
        if (!disabled && (e.key === " " || e.key === "Enter") && e.target === e.currentTarget) {
          e.preventDefault();
          onPick();
        }
      }}
    >
      <div className="row" style={{ gap: 10 }}>
        <span className="icon-tile" style={{ width: 34, height: 34 }}>
          {icon}
        </span>
        <span className="serif grow" style={{ fontSize: 19, fontWeight: 600 }}>
          {title}
        </span>
        <span className="mode-check" aria-hidden>
          {selected && <Check size={14} />}
        </span>
      </div>
      <span className="badge mode-chip">{chip}</span>
      {children}
    </div>
  );
}

