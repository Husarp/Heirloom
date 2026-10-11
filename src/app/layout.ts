// Part B actions (DESIGN B.4–B.7): the scan of media/, moving files between the two layouts, finishing or taking back
// a run. The backend does the work; this is the order of the questions in front of it: unsaved changes (whenSaved),
// „Kto edytuje?” (requireEdit), the final-change dialog with the backup, then the move behind a progress window.
// Nothing here moves a file without the user having said so in that dialog.

import { call, type ApiError } from "../api/transport";
import type { MediaLayout, MediaScan, RestructurePlan, RestructureResult, UndoRestructureResult } from "../api/types";
import { MOVING_BACK_LABEL, MOVING_LABEL, UNDO_ASK, movedToast, undoneToast } from "../screens/media/layoutText";
import type { FinalFacts } from "../screens/archive/workModes";
import { withProgress } from "./progress";
import { useStore } from "./store";

const store = () => useStore.getState();
const fail = (e: unknown) => store().notify((e as ApiError).message, { kind: "err" });

let timer: ReturnType<typeof setTimeout> | undefined;
let generation = 0;

/** Looks at media/ (media.scan) after Start is on screen and again after moves, assigning, „Popraw ścieżki”, an
 *  import, a write-back and a reload. Never for archives opened together. A failed scan only means no banner. */
export function scanSoon(delay = 300) {
  clearTimeout(timer);
  timer = setTimeout(() => void scanNow(), delay);
}

export async function scanNow(): Promise<MediaScan | null> {
  const { archive, phase } = store();
  if (!archive || archive.combined || phase === "chooseMode" || archive.workMode == null) return null;
  const mine = ++generation;
  try {
    const scan = await call<MediaScan>("media.scan");
    // A later scan, or another archive, has taken over meanwhile.
    if (mine === generation && store().archive?.archiveId === archive.archiveId) useStore.setState({ scan });
    return scan;
  } catch {
    return null;
  }
}

const author = () => store().editor ?? "?";

/** Why the archive may not be moved into silently: another program's file is named in the dialog. */
function foreignFact(): Partial<FinalFacts> {
  const a = store().archive;
  return a?.foreign && !a.foreignConfirmed ? { foreign: { origin: a.origin, version: a.gedcomVersion, dataFile: a.dataFile } } : {};
}

function adopt(result: { status: RestructureResult["status"]; report: MediaScan }) {
  useStore.setState({ scan: result.report });
  store().changed(result.status);
}

export interface MoveArgs {
  to: MediaLayout;
  folders?: string[];
  files?: string[];
  only?: boolean;
  assignTo?: string;
  /** Default: a switch (not `only`) stores the layout. */
  setLayout?: boolean;
}

/** Moves files in the archive: unsaved changes first, then „Kto edytuje?”, then the final-change dialog (kind
 *  „restructure”, with the backup); „Przenieś pliki” there starts the move. `count` is what the dialog names. */
export function moveFiles(args: MoveArgs, filesCount: number, then?: () => void) {
  store().whenSaved(
    () =>
      store().requireEdit(() =>
        store().confirmFinal("restructure", ({ allowForeign }) => void runMove(args, allowForeign, then), { files: filesCount, ...foreignFact() }),
      ),
    "Przed przeniesieniem plików",
  );
}

async function runMove(args: MoveArgs, allowForeign: boolean, then?: () => void) {
  try {
    const result = await withProgress(MOVING_LABEL, call<RestructureResult>("media.restructure", { ...args, author: author(), allowForeign }));
    adopt(result);
    then?.();
    const toast = movedToast(result);
    store().notify(toast.text, { kind: toast.kind, detail: toast.detail, action: result.journal ? { label: "Cofnij porządkowanie", run: () => askUndo(result.journal!) } : undefined });
  } catch (e) {
    fail(e);
    scanSoon();
  }
}

/** „Dokończ” on an interrupted run: the files left are moved; the dialog names how many. */
export function finishMove(left: number) {
  store().whenSaved(
    () =>
      store().requireEdit(() =>
        store().confirmFinal(
          "restructure",
          async ({ allowForeign }) => {
            try {
              const result = await withProgress(MOVING_LABEL, call<RestructureResult>("media.finishRestructure", { author: author(), allowForeign }));
              adopt(result);
              const toast = movedToast(result);
              store().notify(toast.text, { kind: toast.kind, detail: toast.detail, action: result.journal ? { label: "Cofnij porządkowanie", run: () => askUndo(result.journal!) } : undefined });
            } catch (e) {
              fail(e);
              scanSoon();
              void refreshStatus();
            }
          },
          { files: left, ...foreignFact() },
        ),
      ),
    "Przed dokończeniem przenoszenia",
  );
}

/** „Cofnij porządkowanie” / „Cofnij przeniesione”: asks first, because files are moved. */
export function askUndo(journal: string) {
  store().setAsk({
    title: UNDO_ASK.title,
    text: UNDO_ASK.text,
    icon: "warn",
    buttons: [
      { label: "Anuluj", kind: "ghost" },
      { label: "Cofnij porządkowanie", kind: "primary", run: () => undoMove(journal) },
    ],
  });
}

function undoMove(journal: string) {
  store().whenSaved(
    () =>
      store().requireEdit(async () => {
        try {
          const result = await withProgress(MOVING_BACK_LABEL, call<UndoRestructureResult>("media.undoRestructure", { journal, author: author(), allowForeign: foreignFact().foreign != null }));
          adopt(result);
          const toast = undoneToast(result);
          store().notify(toast.text, { detail: toast.detail });
        } catch (e) {
          fail(e);
          scanSoon();
          void refreshStatus();
        }
      }),
    "Przed cofnięciem porządkowania",
  );
}

async function refreshStatus() {
  try {
    store().changed(await call<import("../api/types").ArchiveStatus>("archive.status"));
  } catch {
    /* the screen keeps what it had */
  }
}

/** The plan a switch or a move would carry out (a read, nothing changes). */
export function previewMove(args: Pick<MoveArgs, "to" | "folders" | "files" | "only">): Promise<RestructurePlan> {
  return call<RestructurePlan>("media.restructurePreview", args);
}

/** „Tylko nowe pliki”: stores the layout only; what lies elsewhere stays and shows as „Do uporządkowania”. */
export async function setLayoutOnly(layout: MediaLayout): Promise<boolean> {
  try {
    await call("media.setLayout", { layout });
    store().changed();
    scanSoon(0);
    return true;
  } catch (e) {
    fail(e);
    return false;
  }
}

/** „Nie teraz” on the Start banner: asked again only when the findings change. */
export async function tidyNotNow(scan: MediaScan) {
  try {
    await call("media.tidySeen", { fingerprint: scan.fingerprint });
    useStore.setState({ scan: { ...scan, seen: true } });
  } catch (e) {
    fail(e);
  }
}
