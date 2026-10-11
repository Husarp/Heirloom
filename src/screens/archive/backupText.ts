// Backups of the whole archive (DESIGN A.7): the texts of the final-change dialog's backup block and of Settings, as
// pure functions so they are tested on their own.

import type { BackupInfo, BackupResult, JobProgress } from "../../api/types";
import { bytes, clock, count, num, plural, relativeTime } from "../../lib/format";
import { leafOf } from "./workModes";

const NBSP = "\u00a0";

/** „dziś 14:05”, „wczoraj 14:05”, else the date: how a backup is named in a sentence. */
export function backupWhen(iso: string, now = new Date()): string {
  const t = relativeTime(iso, now);
  if (/^\d\d:\d\d$/.test(t)) return `dziś ${t}`;
  if (t === "wczoraj") return `wczoraj ${clock(iso)}`;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(date.getDate())}.${p(date.getMonth() + 1)}.${date.getFullYear()} ${clock(iso)}`;
}

/** The last backup, when its file is still there. */
function lastZip(info: BackupInfo) {
  return info.lastZip?.exists ? info.lastZip : null;
}

/** The line on top of the backup block. */
export function backupStatusLine(info: BackupInfo, now = new Date()): string {
  const last = lastZip(info);
  if (!last) return "Nie ma jeszcze kopii zapasowej tego archiwum.";
  if (info.current) return `✓ Kopia z ${backupWhen(last.at, now)} jest aktualna — od niej nic się nie zmieniło.`;
  const t = relativeTime(last.at, now);
  return `Ostatnia kopia zapasowa: ${/^\d\d:\d\d$/.test(t) ? `dziś ${t}` : t}.`;
}

/** „ok. 1,2 GB (wolne: 35 GB)”. */
export function backupSizeText(info: BackupInfo): string {
  return `ok.${NBSP}${bytes(info.bytes)}${info.free == null ? "" : ` (wolne: ${bytes(info.free)})`}`;
}

/** The disk the folder is on, „C:”, when the path says it. */
function driveOf(path: string): string | null {
  return /^[A-Za-z]:/.test(path) ? path.slice(0, 2).toUpperCase() : null;
}

/** There is not enough room for the zip in the backup folder. */
export function noRoom(info: BackupInfo): boolean {
  return info.free != null && info.needed > info.free;
}

export interface BackupNote {
  kind: "noRoom" | "oneDrive" | "sameDisk" | "cloudOnly";
  text: string;
  /** Offers [Wybierz miejsce…]. */
  choose?: boolean;
}

/** The notes under the block, most pressing first. */
export function backupNotes(info: BackupInfo): BackupNote[] {
  const notes: BackupNote[] = [];
  if (noRoom(info)) {
    const drive = driveOf(info.folder);
    notes.push({
      kind: "noRoom",
      text: `Za mało miejsca ${drive ? `na dysku ${drive}` : "w tym miejscu"} — kopia zajmie ok. ${bytes(info.needed)}, a wolne jest ${bytes(info.free ?? 0)}. Wybierz inne miejsce, np. pendrive.`,
      choose: true,
    });
  }
  if (info.inOneDrive && info.bytes > 1_000_000_000)
    notes.push({ kind: "oneDrive", text: `Folder kopii jest w OneDrive — kopia (${bytes(info.bytes)}) zajmie tam miejsce i będzie wysyłana do chmury.` });
  if (info.sameDisk) notes.push({ kind: "sameDisk", text: "Kopia będzie na tym samym dysku co archiwum. Dla pewności skopiuj ją czasem na pendrive." });
  if (info.cloudOnly.files > 0)
    notes.push({
      kind: "cloudOnly",
      text: `${count(info.cloudOnly.files, "plik archiwum jest", "pliki archiwum są", "plików archiwum jest")} tylko w chmurze OneDrive — kopia ${plural(info.cloudOnly.files, "go pobierze", "je pobierze", "je pobierze")} (${bytes(info.cloudOnly.bytes)}).`,
    });
  return notes;
}

/** „Tworzę kopię zapasową… 412 z 1 230 plików” (no numbers until the first file). */
export function runningText(p: Pick<JobProgress, "done" | "total"> | null): string {
  if (!p || p.total <= 0) return "Tworzę kopię zapasową…";
  return `Tworzę kopię zapasową… ${num(p.done)} z ${num(p.total)} ${plural(p.total, "pliku", "plików", "plików")}`;
}

export const CANCELLED_TEXT = "Przerwano tworzenie kopii — nic nie zostało zapisane.";

/** „✓ Kopia gotowa: Heirloom-Rodzina-2026-10-10-1405.zip (1,2 GB)”. */
export function readyText(result: BackupResult): string {
  return `✓ Kopia gotowa: ${leafOf(result.path)} (${bytes(result.bytes)})`;
}

/** The toast after „Utwórz kopię”. */
export function doneToast(result: BackupResult): string {
  return `Utworzono kopię zapasową: ${count(result.files, "plik", "pliki", "plików")}, ${bytes(result.bytes)}.`;
}

/** Settings: „Ostatnia: 3 dni temu (1,2 GB)”. */
export function lastBackupNote(info: BackupInfo, now = new Date()): string {
  const last = lastZip(info);
  if (!last) return "Jeszcze nie było kopii.";
  const t = relativeTime(last.at, now);
  return `Ostatnia: ${/^\d\d:\d\d$/.test(t) ? `dziś ${t}` : t} (${bytes(last.bytes)})`;
}

export const ROTATION_NOTE = "Heirloom trzyma 5 ostatnich takich kopii tego archiwum, starsze usuwa.";
