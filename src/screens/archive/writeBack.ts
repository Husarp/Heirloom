// „Przenieś zmiany do archiwum”, „Archiwum zmieniło się” and the undo (DESIGN A.8, A.9, A.11, A.12): the texts and
// decisions, pure so they are tested on their own.

import type { RebaseConflict } from "../../api/types";
import { clock, count, dayMonth, plural } from "../../lib/format";

/** Why the changes can't go into the archive now; the changes stay with the user. null: nothing stands in the way. */
export const WRITE_BACK_REFUSALS = {
  originalMissing: "Archiwum jest teraz niedostępne — przenieść zmiany można, gdy będzie dostępne.",
  lockedOriginal: "Właściciel archiwum zablokował zmiany (Tylko do odczytu). Twoje zmiany zostają u Ciebie.",
  notWritable: "W archiwum nie da się nic zapisać (np. płyta lub zablokowany pendrive). Twoje zmiany zostają u Ciebie.",
};

export function writeBackRefusal(p: { originalMissing: boolean; lockedOriginal: boolean; writable: boolean }): string | null {
  if (p.originalMissing) return WRITE_BACK_REFUSALS.originalMissing;
  if (p.lockedOriginal) return WRITE_BACK_REFUSALS.lockedOriginal;
  if (!p.writable) return WRITE_BACK_REFUSALS.notWritable;
  return null;
}

export const WRITE_BACK_REFUSED_TITLE = "Nie można teraz przenieść zmian";

/** The toast after a write-back: „Przeniesiono zmiany do archiwum.” and „37 zmian, 12 plików.” */
export function writeBackDone(r: { changes: number; files: number }): { text: string; detail: string } {
  const parts: string[] = [];
  if (r.changes > 0 || r.files === 0) parts.push(count(r.changes, "zmiana", "zmiany", "zmian"));
  if (r.files > 0) parts.push(count(r.files, "plik", "pliki", "plików"));
  return { text: "Przeniesiono zmiany do archiwum.", detail: `${parts.join(", ")}.` };
}

export const WRITE_BACK_FAILED_TITLE = "Zmiany nie trafiły do archiwum";
export const NOTHING_TO_MOVE = "Archiwum ma już wszystkie Twoje zmiany.";

/** „Archiwum wróci do stanu sprzed 14:05. Twoje zmiany zostaną u Ciebie.” */
export function undoConfirm(lastWriteBack: string | null): { title: string; text: string } {
  const at = clock(lastWriteBack);
  return {
    title: "Cofnąć przeniesienie zmian do archiwum?",
    text: `Archiwum wróci do stanu sprzed ${at || "tego przeniesienia"}. Twoje zmiany zostaną u Ciebie.`,
  };
}
export const UNDO_DONE = { text: "Cofnięto przeniesienie.", detail: "Archiwum jest takie jak przed nim. Twoje zmiany znów czekają na przeniesienie." };
export const UNDO_FAILED_TITLE = "Nie da się cofnąć";

// ---- A.9: the archive changed ----

export const REBASE_TITLE = "Archiwum zmieniło się";
export const FAST_FORWARD_DONE = "Wczytano nowe zmiany z archiwum.";
export const REBASE_DONE = { text: "Zmiany z archiwum połączono z Twoimi.", detail: "Nic nie zginęło." };
export const TOO_DIFFERENT_TITLE = "Archiwum wygląda teraz zupełnie inaczej";

/** „Od 3 października ktoś zmienił archiwum (14 zmian). Ty masz 37 zmian. Heirloom połączy jedno z drugim — nic nie zginie.” */
export function rebaseText(r: { since?: string | null; theirChanges: number; myChanges: number }): string {
  const date = r.since && !Number.isNaN(new Date(r.since).getTime()) ? new Date(r.since) : null;
  const theirs = count(r.theirChanges, "zmiana", "zmiany", "zmian");
  const mine = count(r.myChanges, "zmianę", "zmiany", "zmian");
  return `${date ? `Od ${dayMonth(date)} ktoś` : "Ktoś"} zmienił archiwum (${theirs}). Ty masz ${mine}. Heirloom połączy jedno z drugim — nic nie zginie.`;
}

/** „U 3 osób zmieniono to samo i u Ciebie, i w archiwum. Wybierz, którą wersję zostawić:” */
export function conflictsText(conflicts: Pick<RebaseConflict, "tag">[]): string {
  const n = conflicts.length;
  const people = conflicts.every((c) => c.tag === "INDI");
  const who = people ? `${n}${" "}${plural(n, "osoby", "osób", "osób")}` : `${n}${" "}${plural(n, "wpisu", "wpisów", "wpisów")}`;
  return `U ${who} zmieniono to samo i u Ciebie, i w archiwum. Wybierz, którą wersję zostawić:`;
}

export type Choice = "mine" | "theirs";

/** The version kept for each record unless the user chooses: the backend's default (the changed one when the other side
 *  deleted it, else the user's). */
export function defaultChoices(conflicts: RebaseConflict[]): Record<string, Choice> {
  return Object.fromEntries(conflicts.map((c) => [c.key, c.default]));
}

/** The `choices` sent to `archive.rebase`: every conflict has one, so nothing is left to a default by accident. */
export function choicesToSend(conflicts: RebaseConflict[], chosen: Record<string, Choice>): Record<string, Choice> {
  return Object.fromEntries(conflicts.map((c) => [c.key, chosen[c.key] ?? c.default]));
}

/** A word under the choice when a side deleted the record. */
export function sideNote(c: RebaseConflict, side: Choice): string | null {
  if (side === "mine" && c.mine == null) return "usunięte u Ciebie";
  if (side === "theirs" && c.theirs == null) return "usunięte w archiwum";
  return null;
}

/** „(z Twoimi zmianami: 14)” beside an archive of an opened-together set that is shown in the user's version. */
export function withChangesLabel(a: { withChanges?: boolean; pending?: number }): string | null {
  if (!a.withChanges) return null;
  return a.pending && a.pending > 0 ? `(z Twoimi zmianami: ${a.pending})` : "(z Twoimi zmianami)";
}
