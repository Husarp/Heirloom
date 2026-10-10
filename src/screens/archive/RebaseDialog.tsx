import { ArrowRight, FolderSync } from "lucide-react";
import { useState } from "react";
import { create } from "zustand";
import type { FieldChange, RebaseConflict, RebaseResult } from "../../api/types";
import { Dialog } from "../../components/Dialog";
import { REBASE_TITLE, conflictsText, defaultChoices, rebaseText, sideNote, type Choice } from "./writeBack";

type Preview = Extract<RebaseResult["result"], { merged: false }>;

interface Asked {
  preview: Preview;
  answer: (choices: Record<string, Choice> | null) => void;
}

const useRebaseDialog = create<{ asked: Asked | null }>(() => ({ asked: null }));

/** „Archiwum zmieniło się” (DESIGN A.9): what will be merged, and which version to keep where both sides changed the
 *  same record. Resolves with the choice of every conflict ({} without conflicts), or null for „Później”. */
export function askRebase(preview: Preview): Promise<Record<string, Choice> | null> {
  return new Promise((resolve) => {
    useRebaseDialog.setState({
      asked: {
        preview,
        answer: (choices) => {
          useRebaseDialog.setState({ asked: null });
          resolve(choices);
        },
      },
    });
  });
}

export function RebaseDialog() {
  const asked = useRebaseDialog((s) => s.asked);
  if (!asked) return null;
  return <Body key={asked.preview.since + asked.preview.conflicts.length} {...asked} />;
}

function Body({ preview, answer }: Asked) {
  const conflicts = preview.conflicts;
  const [chosen, setChosen] = useState<Record<string, Choice>>(() => defaultChoices(conflicts));
  const [shown, setShown] = useState<string | null>(null);
  const later = () => answer(null);
  return (
    <Dialog width={conflicts.length > 0 ? 660 : 500} onClose={later} labelledBy="rebase-title">
      <div className="dialog-body">
        <div className="dialog-icon" style={{ background: "var(--accent-soft)", color: "var(--accent-text)" }}>
          <FolderSync size={19} />
        </div>
        <div className="dialog-title" id="rebase-title">
          {REBASE_TITLE}
        </div>
        <div className="dialog-text">{rebaseText(preview)}</div>
        {conflicts.length > 0 && (
          <>
            <div className="dialog-text" style={{ color: "var(--text)" }}>
              {conflictsText(conflicts)}
            </div>
            <div className="card rebase-list">
              {conflicts.map((c) => (
                <ConflictRow
                  key={c.key}
                  conflict={c}
                  choice={chosen[c.key] ?? c.default}
                  open={shown === c.key}
                  onChoose={(side) => setChosen({ ...chosen, [c.key]: side })}
                  onToggle={() => setShown(shown === c.key ? null : c.key)}
                />
              ))}
            </div>
          </>
        )}
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={later}>
          Później
        </button>
        <button className="btn primary" onClick={() => answer(chosen)} autoFocus>
          Połącz
        </button>
      </div>
    </Dialog>
  );
}

function ConflictRow({ conflict: c, choice, open, onChoose, onToggle }: { conflict: RebaseConflict; choice: Choice; open: boolean; onChoose: (side: Choice) => void; onToggle: () => void }) {
  const option = (side: Choice, label: string) => {
    const note = sideNote(c, side);
    return (
      <label className="rebase-choice">
        <input type="radio" name={`rebase-${c.key}`} checked={choice === side} onChange={() => onChoose(side)} />
        <span>{label}</span>
        {note && <span className="rebase-note">({note})</span>}
      </label>
    );
  };
  return (
    <div className="rebase-row">
      <div className="rebase-head">
        <span className="rebase-name ellipsis">{c.name}</span>
        <div className="rebase-choices" role="radiogroup" aria-label={`Którą wersję zostawić: ${c.name}`}>
          {option("mine", "Twoja wersja")}
          {option("theirs", "Z archiwum")}
        </div>
        <button className="btn secondary xs" aria-expanded={open} onClick={onToggle}>
          {open ? "Ukryj różnice" : "Pokaż różnice"}
        </button>
      </div>
      {open && (
        <div className="rebase-diff">
          <Column title="U Ciebie" changes={c.mineChanges} />
          <Column title="W archiwum" changes={c.theirChanges} />
        </div>
      )}
    </div>
  );
}

function Column({ title, changes }: { title: string; changes: FieldChange[] }) {
  return (
    <div className="rebase-col">
      <div className="label-caps">{title}</div>
      {changes.length === 0 && <div className="rebase-none">bez zmian w polach</div>}
      {changes.map((l, i) => (
        <div key={i} className="rebase-line">
          <span className="rebase-field">{l.field}</span>
          <span className="rebase-values">
            <span style={{ color: "var(--text3)" }}>{l.old ?? "—"}</span>
            <ArrowRight size={12} color="var(--text3)" style={{ flex: "none" }} />
            <span style={{ fontWeight: 500 }}>{l.new ?? "—"}</span>
          </span>
        </div>
      ))}
    </div>
  );
}
