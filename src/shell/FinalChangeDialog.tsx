// „Zapisać import w archiwum?” and its kin (DESIGN A.7): before a change that can't be fully undone, the dialog says
// what will happen and offers a backup of the whole archive; a file made by another program is asked about here too.

import { FileWarning } from "lucide-react";
import { useState } from "react";
import { useStore, type FinalChange } from "../app/store";
import { Dialog } from "../components/Dialog";
import { displayPath } from "../lib/format";
import { FINAL_ACTIONS, FINAL_TITLES, finalChangeText, foreignLine, type FinalFacts } from "../screens/archive/workModes";
import { BackupBlock, useBackupBlock } from "./BackupBlock";
import { saveAsNewArchive } from "./Dialogs";

export const FINAL_COMMON = "Tej zmiany może nie dać się w całości cofnąć. Zalecamy najpierw zrobić kopię zapasową całego archiwum.";

export function FinalChangeDialog() {
  const final = useStore((s) => s.finalChange);
  const archive = useStore((s) => s.archive);
  if (!final || !archive) return null;
  return <Body kind={final.kind} run={final.run} extra={final.extra} />;
}

function Body({ kind, run, extra }: FinalChange) {
  const archive = useStore((s) => s.archive)!;
  const close = useStore((s) => s.closeFinalChange);
  // The backup of the archive as the user sees it; before a write-back the original itself.
  const bb = useBackupBlock(kind === "writeBack" ? "original" : "archive");
  const [busy, setBusy] = useState(false);

  const facts: FinalFacts = { name: archive.name, root: displayPath(archive.root), dataFile: archive.dataFile, ...extra };
  // An import into the archive's own file asks about another program's file here; the write-back brings its own facts.
  const foreign =
    "foreign" in (extra ?? {})
      ? (extra!.foreign ?? null)
      : kind === "import" && archive.foreign && !archive.foreignConfirmed
        ? { origin: archive.origin, version: archive.gedcomVersion, dataFile: archive.dataFile }
        : null;
  const running = bb.phase === "running";

  const go = () => {
    close();
    void run({ allowForeign: foreign != null });
  };
  const saveAsNew = async () => {
    setBusy(true);
    if (await saveAsNewArchive()) close();
    setBusy(false);
  };

  return (
    <Dialog width={foreign ? 680 : 580} onClose={running ? undefined : close} labelledBy="final-title">
      <div className="col" style={{ padding: "22px 24px 14px", gap: 8, overflow: "auto" }}>
        <div className="row" style={{ gap: 14, alignItems: "flex-start" }}>
          <div className="dialog-icon" style={{ flex: "none" }}>
            <FileWarning size={19} />
          </div>
          <div className="col" style={{ gap: 8 }}>
            <div id="final-title" className="dialog-title">
              {FINAL_TITLES[kind]}
            </div>
            <div className="dialog-text">{finalChangeText(kind, facts)}</div>
            {foreign && <div className="dialog-text">{foreignLine(foreign)}</div>}
            <div className="dialog-text">{FINAL_COMMON}</div>
          </div>
        </div>
      </div>
      <BackupBlock bb={bb} />
      <div className="dialog-foot">
        <button className="btn ghost" disabled={running || busy} onClick={close}>
          Anuluj
        </button>
        <span className="grow" />
        {foreign && (
          <button className="btn secondary" disabled={running || busy} onClick={saveAsNew} title="Plik z innego programu zostanie bez zmian">
            Zapisz jako nowe archiwum…
          </button>
        )}
        {bb.covered ? (
          <button className="btn primary" disabled={running || busy} onClick={go}>
            {FINAL_ACTIONS[kind]}
          </button>
        ) : (
          <>
            <button className="btn secondary" disabled={running || busy} onClick={go}>
              Dalej bez kopii
            </button>
            <button className="btn primary" disabled={running || busy || !bb.canStart} onClick={bb.start}>
              Zrób kopię zapasową
            </button>
          </>
        )}
      </div>
    </Dialog>
  );
}
