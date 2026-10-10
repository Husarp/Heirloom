// „Pracuj na archiwum” and „Nie ruszaj oryginału”: the texts and choices the question, the final-change dialog and
// Settings share (pure, so they are tested on their own).

import type { ArchiveStatus, Editor, ModeInfo, OverlayInfo, OverlayStatus } from "../../api/types";
import { bytes, clock, count, dayMonth, displayPath, num, plural, relativeTime } from "../../lib/format";

export type WorkMode = "inPlace" | "overlay";

/** The card preselected in „Jak chcesz pracować z tym archiwum?”: a folder that can't be written can only be left
 *  alone; another program's file the user hasn't worked on before is safer left alone; otherwise in place. */
export function defaultMode(info: ModeInfo): WorkMode {
  if (!info.writable) return "overlay";
  if (info.usedBefore) return "inPlace";
  if (info.foreign) return "overlay";
  return info.suggested;
}

/** At most one note under the cards. */
export function modeNote(info: ModeInfo): string | null {
  if (!info.writable)
    return "W tym folderze nie da się nic zapisać (np. płyta, zablokowany pendrive albo folder tylko do odczytu), więc zostaje „Nie ruszaj oryginału”.";
  if (info.usedBefore) return "Do tej pory Heirloom zapisywał zmiany w tym archiwum.";
  if (info.foreign) return `Ten plik utworzył program ${info.foreign} — polecamy „Nie ruszaj oryginału”.`;
  return null;
}

export type FinalKind = "import" | "writeBack" | "restructure";

export interface FinalFacts {
  name: string;
  root?: string;
  people?: number;
  files?: number;
  changes?: number;
  bytes?: number;
  dataFile?: string;
  /** The file was made by another program and the user hasn't confirmed saving into it: the dialog says so. */
  foreign?: { origin: string | null; version: string | null; dataFile: string } | null;
}

export const FINAL_TITLES: Record<FinalKind, string> = {
  import: "Zapisać import w archiwum?",
  writeBack: "Przenieść zmiany do archiwum?",
  restructure: "Przenieść pliki w archiwum?",
};

export const FINAL_ACTIONS: Record<FinalKind, string> = {
  import: "Zapisz import",
  writeBack: "Przenieś do archiwum",
  restructure: "Przenieś pliki",
};

/** The first line of the final-change dialog: what will change, and how far it can be undone. */
export function finalChangeText(kind: FinalKind, f: FinalFacts): string {
  const people = f.people ?? 0;
  const files = f.files ?? 0;
  if (kind === "import") {
    const what = [count(people, "osobę", "osoby", "osób"), files > 0 ? count(files, "plik", "pliki", "plików") : null].filter(Boolean).join(" i ");
    return `Import doda ${what} do archiwum „${f.name}”. Można go później cofnąć („Cofnij import”), ale skopiowane zdjęcia zostaną w folderze media, a osoby zmienione potem przez kogoś innego nie wrócą do stanu sprzed importu.`;
  }
  if (kind === "writeBack") {
    const changes = f.changes ?? 0;
    let what = count(changes, "zmiana", "zmiany", "zmian");
    if (people > 0) what += ` (${count(people, "osoba", "osoby", "osób")})`;
    if (files > 0) what += ` i ${count(files, "plik", "pliki", "plików")}${f.bytes ? ` (${bytes(f.bytes)})` : ""}`;
    return `Do archiwum „${f.name}”${f.root ? ` (${f.root})` : ""} ${plural(changes + files, "trafi", "trafią", "trafi")}: ${what}. Zobaczy je każdy, kto otworzy to archiwum. Przeniesienie można cofnąć, dopóki nikt inny nie zmieni archiwum.`;
  }
  return `Heirloom przeniesie ${num(files)} ${plural(files, "plik", "pliki", "plików")} w folderze media i zmieni ich ścieżki w pliku ${f.dataFile ?? ""}. Porządkowanie można cofnąć („Cofnij porządkowanie”).`;
}

/** „Masz 14 zmian, których nie ma jeszcze w archiwum.” */
export function pendingText(n: number): string {
  return `Masz ${count(n, "zmianę", "zmiany", "zmian")}, ${n === 1 ? "której" : "których"} nie ma jeszcze w archiwum.`;
}

// ---- Where „Nie ruszaj oryginału” shows (DESIGN A.5, A.6): texts and decisions, pure so they are tested ----

/** „Na tym archiwum pracują też: Ewa (ostatnio 3 dni temu).” — people other than the last editor here. */
export function editorsNote(editors: Editor[], now = new Date()): string | null {
  if (editors.length === 0) return null;
  const each = editors.map((e) => (e.lastEdited ? `${e.name} (ostatnio ${relativeTime(e.lastEdited, now)})` : e.name));
  return `Na tym archiwum pracują też: ${each.join(", ")}.`;
}

/** The folder of „Twoje zmiany” as the question shows it: the suggested one, or inside the parent the user chose.
 *  `chosenParent`: the folder picked with [Zmień…]; the archive's own folder name is added to it. */
export function changesFolderShown(info: ModeInfo, chosenParent: string | null): string {
  if (!chosenParent) return info.folder.label;
  const leaf = info.folder.path.split(/[\\/]/).filter(Boolean).pop() ?? "";
  const parent = displayPath(chosenParent).replace(/[\\/]+$/, "");
  return leaf ? `${parent}${parent.includes("\\") || /^[A-Za-z]:$/.test(parent) ? "\\" : "/"}${leaf}` : parent;
}

/** What goes into the edit bar and the places that name where changes are saved. */
export function savesWhere(archive: Pick<ArchiveStatus, "workMode" | "dataFile">): string {
  return archive.workMode === "overlay" ? "u Ciebie, w folderze Twoich zmian" : `w pliku ${archive.dataFile}`;
}

/** „Archiwum: E:\…\rodzina.ged · Twoje zmiany: C:\…” (A1) or just the data file's path. */
export function sidebarTip(archive: Pick<ArchiveStatus, "dataPath" | "overlay">): string {
  const original = displayPath(archive.dataPath);
  return archive.overlay ? `Archiwum: ${original} · Twoje zmiany: ${displayPath(archive.overlay.folder)}` : original;
}

/** The tooltip of „Gdzie?” in the edit bar. */
export function whereTip(overlay: Pick<OverlayStatus, "folder">): string {
  return `Twoje zmiany: ${displayPath(overlay.folder)}`;
}

/** The last segment of a folder path. */
export function leafOf(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/** „2026-10-10” in local time (the date in the names of renamed folders). */
export function isoDay(d = new Date()): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export type BannerKind = "interrupted" | "originalMissing" | "originalChanged" | "pending";

/** Which banners Start shows on top, in order. The user's ✕ hides one until the next start. While the archive can't
 *  be reached nothing can be moved into it, so only that is said. */
export function startBanners(overlay: OverlayStatus | null | undefined, hidden: readonly string[]): BannerKind[] {
  if (!overlay) return [];
  const all: BannerKind[] = [];
  if (overlay.interrupted) all.push("interrupted");
  if (overlay.originalMissing) all.push("originalMissing");
  else {
    if (overlay.originalChanged) all.push("originalChanged");
    if (overlay.pending > 0) all.push("pending");
  }
  return all.filter((kind) => !hidden.includes(kind));
}

export const BANNER_TEXT = {
  pending: (n: number) => `${pendingText(n)} Są zapisane tylko na tym komputerze.`,
  originalChanged: "Ktoś zmienił archiwum od ostatniego razu.",
  originalMissing: (root: string) =>
    `Nie można otworzyć archiwum (${displayPath(root)}) — może pendrive jest odłączony. Możesz dalej oglądać i zmieniać swoją wersję; zdjęcia z archiwum pojawią się, gdy będzie dostępne.`,
};

/** Closing with changes the archive doesn't have, when the archive may travel without them (another drive, a share):
 *  once for each archive in a session. `shown` holds the archives already told. */
export function closeReminder(archive: ArchiveStatus | null, shown: ReadonlySet<string>): { key: string; title: string; text: string } | null {
  const o = archive?.overlay;
  if (!archive || archive.workMode !== "overlay" || !o) return null;
  if (o.pending <= 0 || o.sameDrive || o.originalMissing || shown.has(archive.dataPath)) return null;
  const n = o.pending;
  const verb = plural(n, "jest", "są", "jest");
  return {
    key: archive.dataPath,
    title: "Twoich zmian nie ma jeszcze w archiwum",
    text: `${count(n, "zmiana", "zmiany", "zmian")} ${verb} zapisanych tylko na tym komputerze. Archiwum (${displayPath(archive.root)}) ${n === 1 ? "jej" : "ich"} nie ma.`,
  };
}

/** Ustawienia › Twoje zmiany › „Czeka na przeniesienie”. */
export function waitingText(o: Pick<OverlayInfo, "pending" | "people" | "newFiles" | "newBytes">): string {
  const parts: string[] = [];
  if (o.pending > 0) parts.push(`${count(o.pending, "zmiana", "zmiany", "zmian")}${o.people > 0 ? ` (${count(o.people, "osoba", "osoby", "osób")})` : ""}`);
  if (o.newFiles > 0) parts.push(`${count(o.newFiles, "nowy plik", "nowe pliki", "nowych plików")}${o.newBytes > 0 ? ` (${bytes(o.newBytes)})` : ""}`);
  return parts.length ? parts.join(", ") : "Nic — archiwum ma wszystkie Twoje zmiany";
}

/** „nigdy” or „3 października, 14:05”. */
export function lastWriteBackText(iso: string | null): string {
  if (!iso) return "nigdy";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "nigdy" : `${dayMonth(d)}, ${clock(iso)}`;
}

export const SWITCH_TO_OVERLAY = {
  title: "Pracować bez zmieniania archiwum?",
  text: (folderLabel: string) => `Od teraz Twoje zmiany będą trafiać do folderu ${folderLabel}. Archiwum zostanie takie, jakie jest teraz.`,
};

/** „Masz 14 zmian, których nie ma w archiwum” (A1 → A2 with changes waiting). */
export function pendingTitle(n: number): string {
  return `Masz ${count(n, "zmianę", "zmiany", "zmian")}, ${n === 1 ? "której" : "których"} nie ma w archiwum`;
}
export const PENDING_SWITCH_TEXT = "Żeby pracować na archiwum, Twoje zmiany muszą najpierw do niego trafić.";

/** „Porzucić 14 zmian?” and the folder that is kept. */
export function discardTitle(n: number): string {
  return n > 0 ? `Porzucić ${count(n, "zmianę", "zmiany", "zmian")}?` : "Zacząć od nowa?";
}
export function discardText(folderName: string, day: string): string {
  return `Folder Twoich zmian dostanie nazwę „${folderName} (porzucone ${day})” — możesz go jeszcze otworzyć przez „Otwórz folder…”. Zaczniesz od tego, co jest teraz w archiwum.`;
}
