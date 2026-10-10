// „Przenieś zmiany do archiwum” (DESIGN A.8), the merge of what changed in the archive meanwhile (A.9) and the undo
// (A.12). The backend does the work; these are the questions in front of it and what follows.

import { call, type ApiError } from "../api/transport";
import type { ArchiveStatus, RebaseResult, WriteBackPreview, WriteBackResult } from "../api/types";
import { displayPath } from "../lib/format";
import { askRebase } from "../screens/archive/RebaseDialog";
import {
  FAST_FORWARD_DONE,
  NOTHING_TO_MOVE,
  REBASE_DONE,
  TOO_DIFFERENT_TITLE,
  UNDO_DONE,
  UNDO_FAILED_TITLE,
  WRITE_BACK_FAILED_TITLE,
  WRITE_BACK_REFUSED_TITLE,
  choicesToSend,
  undoConfirm,
  writeBackDone,
  writeBackRefusal,
} from "../screens/archive/writeBack";
import type { FinalFacts } from "../screens/archive/workModes";
import { discardMine, keepAsNewArchive, withBusy } from "./workMode";
import { useStore } from "./store";

const store = () => useStore.getState();
const fail = (e: unknown) => store().notify((e as ApiError).message, { kind: "err" });

/** A message that must be read, not a toast that goes away. */
function tell(title: string, text: string) {
  store().setAsk({ title, text, icon: "warn", buttons: [{ label: "Rozumiem", kind: "primary" }] });
}

/** The status as it is now (banners and Settings read it). */
async function refreshStatus() {
  try {
    store().changed(await call<ArchiveStatus>("archive.status"));
  } catch {
    /* the screen keeps what it had */
  }
}

/** Who the write-back is written by in the archive's history: whoever edits, or edited last. */
function writer(): string {
  const s = store();
  return s.editor ?? s.app?.lastEditor ?? s.archive?.editors?.[0]?.name ?? "Heirloom";
}

/** Runs `work` behind the busy window; while it runs the number of files copied is shown (`job.progress`). */
async function withCounts<T>(text: string, work: Promise<T>): Promise<T> {
  useStore.setState({ busy: { text } });
  let over = false;
  const poll = async () => {
    try {
      const p = await call<{ running: boolean; done: number; total: number }>("job.progress");
      if (!over && p.running && p.total > 0) useStore.setState({ busy: { text, note: `${p.done} z ${p.total} plików` } });
    } catch {
      /* the numbers are a courtesy */
    }
    if (!over) timer = setTimeout(poll, 300);
  };
  let timer = setTimeout(poll, 300);
  try {
    return await work;
  } finally {
    over = true;
    clearTimeout(timer);
    useStore.setState({ busy: null });
  }
}

/** „Przenieś do archiwum…”. `then` runs once the changes are in the archive („Przenieś do archiwum i przełącz”). */
export function startWriteBack(then?: () => void) {
  store().whenSaved(() => void writeBackFlow(then, true), "Zanim przeniesiesz zmiany");
}

async function writeBackFlow(then: (() => void) | undefined, mayRebase: boolean): Promise<void> {
  let preview: WriteBackPreview;
  try {
    preview = await call<WriteBackPreview>("archive.writeBackPreview");
  } catch (e) {
    return fail(e);
  }
  const refusal = writeBackRefusal(preview);
  if (refusal) return tell(WRITE_BACK_REFUSED_TITLE, refusal);
  // Someone changed the archive: their changes are taken in first, then the preview is asked again.
  if (preview.originalChanged) {
    if (!mayRebase || (await rebaseFlow()) !== "merged") return;
    return writeBackFlow(then, false);
  }
  if (preview.changes === 0 && preview.files === 0) {
    store().notify(NOTHING_TO_MOVE);
    then?.();
    return;
  }
  const facts = {
    name: preview.name,
    root: displayPath(preview.root),
    people: preview.people,
    files: preview.files,
    changes: preview.changes,
    bytes: preview.bytes,
    foreign: preview.foreign,
  } as Partial<FinalFacts>;
  store().confirmFinal("writeBack", (options) => write(then, options?.allowForeign ?? false), facts);
}

async function write(then: (() => void) | undefined, allowForeign: boolean): Promise<void> {
  try {
    const done = await withCounts("Przenoszę zmiany do archiwum…", call<WriteBackResult>("archive.writeBack", { author: writer(), allowForeign }));
    store().changed(done.status);
    if (then) return then();
    const toast = writeBackDone(done.result);
    store().notify(toast.text, { detail: toast.detail, action: { label: "Cofnij", run: undoWriteBack } });
  } catch (e) {
    const error = e as ApiError;
    await refreshStatus();
    // Changed again while the dialog was open: the merge first, then once more.
    if (error.code === "original_changed") {
      if ((await rebaseFlow()) === "merged") await writeBackFlow(then, false);
      return;
    }
    tell(error.code === "interrupted" ? "Przenoszenie nie zostało dokończone" : WRITE_BACK_FAILED_TITLE, error.message);
  }
}

// ---- the archive changed meanwhile ----

export type RebaseOutcome = "merged" | "later" | "refused" | "failed";

/** „Wczytaj zmiany”. */
export function startRebase() {
  store().whenSaved(() => void rebaseFlow(), "Zanim wczytasz zmiany");
}

/** The changes made in the archive, merged into the user's: nothing of the user's own: loaded at once; otherwise the
 *  dialog says what will be merged, and asks which version to keep where both changed the same. */
export async function rebaseFlow(): Promise<RebaseOutcome> {
  let preview: RebaseResult["result"];
  try {
    preview = (await call<RebaseResult>("archive.rebase", { preview: true })).result;
  } catch (e) {
    return rebaseFailed(e);
  }
  if (preview.merged) return "merged";
  let choices: Record<string, "mine" | "theirs"> | undefined;
  if (preview.myChanges > 0 || preview.conflicts.length > 0) {
    const asked = await askRebase(preview);
    if (!asked) return "later";
    choices = preview.conflicts.length > 0 ? choicesToSend(preview.conflicts, asked) : undefined;
  }
  try {
    const done = await withBusy("Łączę zmiany z archiwum…", () => call<RebaseResult>("archive.rebase", choices ? { choices } : {}));
    store().changed(done.status);
    if (done.result.merged && done.result.fastForward) store().notify(FAST_FORWARD_DONE);
    else store().notify(REBASE_DONE.text, { detail: REBASE_DONE.detail });
    return "merged";
  } catch (e) {
    return rebaseFailed(e);
  }
}

function rebaseFailed(e: unknown): RebaseOutcome {
  const error = e as ApiError;
  if (error.code !== "too_different") {
    fail(e);
    return "failed";
  }
  store().setAsk({
    title: TOO_DIFFERENT_TITLE,
    text: error.message,
    icon: "warn",
    wide: true,
    buttons: [
      { label: "Zachowaj moje zmiany jako osobne archiwum…", kind: "secondary", run: keepAsNewArchive },
      { label: "Pracuj dalej na swojej wersji", kind: "ghost" },
      { label: "Zacznij od nowa z obecnym archiwum…", kind: "secondary", run: discardMine },
    ],
  });
  return "refused";
}

// ---- undo ----

/** [Cofnij] after a write-back and in Settings › Ostatnie przeniesienie. */
export function undoWriteBack() {
  store().whenSaved(() => {
    const ask = undoConfirm(store().archive?.overlay?.lastWriteBack ?? null);
    store().setAsk({
      title: ask.title,
      text: ask.text,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        { label: "Cofnij przeniesienie", kind: "primary", run: undo },
      ],
    });
  }, "Zanim cofniesz przeniesienie");
}

async function undo() {
  try {
    const status = await withBusy("Cofam przeniesienie…", () => call<ArchiveStatus>("archive.undoWriteBack"));
    store().changed(status);
    store().notify(UNDO_DONE.text, { detail: UNDO_DONE.detail });
  } catch (e) {
    await refreshStatus();
    tell(UNDO_FAILED_TITLE, (e as ApiError).message);
  }
}
