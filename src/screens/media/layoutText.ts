// Part B texts and decisions (DESIGN B.5–B.7): the Settings section, the switch dialogs, the Start banner and the
// tidy dialog. Pure, so they are tested on their own. Everything is Polish and calm: nothing here says "scan" or "layout".

import type { LayoutInterrupted, MediaLayout, MediaScan, RestructurePlan, RestructureResult, ScanFolder, ScanUnknown, UndoRestructureResult } from "../../api/types";
import { count, dayMonth, num, plural } from "../../lib/format";

export const filesWord = (n: number) => count(n, "plik", "pliki", "plików");
const foldersWord = (n: number) => count(n, "folder", "foldery", "folderów");

// ---- Settings › Archiwum › Zdjęcia i pliki (B.7, B.8) ----

export const LAYOUT_OPTIONS: { value: MediaLayout; label: string }[] = [
  { value: "flat", label: "Wszystkie razem" },
  { value: "perPerson", label: "W folderach osób" },
];

/** „Teraz: 312 plików w folderach osób, 12 luzem”. */
export function layoutNow(scan: Pick<MediaScan, "layout">): string {
  const { folderOwned, rootOwned } = scan.layout;
  if (folderOwned === 0 && rootOwned === 0) return "Teraz: w folderze media nie ma jeszcze zdjęć osób";
  return `Teraz: ${filesWord(folderOwned)} w folderach osób, ${num(rootOwned)} luzem`;
}

/** A1: the layout is the original's, read-only. */
export function layoutAsInArchive(layout: MediaLayout): string {
  return `Jak w archiwum: ${layout === "perPerson" ? "w folderach osób" : "wszystkie razem"}. Pliki w archiwum przeniesiesz w trybie „Pracuj na archiwum”.`;
}

/** Unknown files that are not in a folder being asked about (those belong to the folder's question, not counted twice). */
export function looseUnknown(scan: Pick<MediaScan, "unknown" | "folders" | "counts">, overlay = false): number {
  const asked = new Set(scan.folders.filter((f) => f.question && !(overlay && f.question === "unpack")).map((f) => f.path));
  return scan.unknown.length === scan.counts.unknown ? scan.unknown.filter((u) => !asked.has(u.folder)).length : scan.counts.unknown;
}

/** Files (not folders) waiting to be tidied. In „Nie ruszaj oryginału” nothing is moved, so loose files of people are not counted. */
export function tidyFiles(scan: Pick<MediaScan, "unknown" | "folders" | "counts">, overlay = false): number {
  const c = scan.counts;
  return c.moved + (overlay ? 0 : c.looseLinked) + looseUnknown(scan, overlay) + c.outside;
}

/** Folders asked about; in A1 only „czyj to folder?” is asked (unpacking a folder moves files). */
export function tidyFolders(scan: Pick<MediaScan, "folders" | "counts">, overlay = false): number {
  return overlay ? scan.folders.filter((f) => f.question === "whose" && !f.leftAlone).length : scan.counts.folders;
}

/** „Do uporządkowania: 14 plików” (+ „, 1 folder”), or „nic”. */
export function tidyNote(scan: Pick<MediaScan, "unknown" | "folders" | "counts">, overlay = false): string {
  const files = tidyFiles(scan, overlay);
  const folders = tidyFolders(scan, overlay);
  const parts = [files > 0 ? filesWord(files) : null, folders > 0 ? foldersWord(folders) : null].filter(Boolean);
  return `Do uporządkowania: ${parts.length ? parts.join(", ") : "nic"}`;
}

/** „3 października (1 198 plików)”; the Settings row says „Ostatnie porządkowanie: …”. */
export function lastRunText(journal: NonNullable<MediaScan["journal"]>): string {
  const d = new Date(journal.ts);
  const when = Number.isNaN(d.getTime()) ? "" : `${dayMonth(d)} `;
  return `${when}(${filesWord(journal.moved)})`;
}

// ---- The switch dialogs (B.7) ----

export function switchTitle(to: MediaLayout): string {
  return to === "perPerson" ? "Ułożyć pliki w folderach osób?" : "Wyjąć pliki z folderów osób?";
}

/** „1 204 pliki, które już są” / „1 plik, który już jest”. */
function filesAlreadyThere(n: number): string {
  return `${plural(n, `${num(n)} plik, który już jest`, `${num(n)} pliki, które już są`, `${num(n)} plików, które już są`)}`;
}

export function switchText(to: MediaLayout, plan: Pick<RestructurePlan, "count" | "newFolders">): string {
  const n = plan.count;
  if (to === "perPerson") {
    const example = plan.newFolders[0] ? plan.newFolders[0].replace(/^media[\\/]/, "") : "Kowalski Józef";
    const lead = `Nowe zdjęcia i dokumenty będą trafiać do folderów osób, np. media\\${example}.`;
    return n > 0 ? `${lead} Heirloom może też od razu przenieść tam ${filesAlreadyThere(n)} w archiwum.` : lead;
  }
  if (n === 0) return "Nowe zdjęcia i dokumenty będą trafiać prosto do folderu media. Żadne pliki nie leżą teraz w folderach osób, więc nic nie trzeba przenosić.";
  return `Heirloom przeniesie ${filesWord(n)} z folderów osób prosto do folderu media, a puste foldery usunie. Gdy dwa pliki mają tę samą nazwę, drugi dostanie numer, np. „ślub (2).jpg”. Foldery z plikami, których Heirloom nie zna, zostaną.`;
}

/** The bullets under the text; only to B2 has any. */
export function switchBullets(to: MediaLayout, plan: Pick<RestructurePlan, "groupFiles" | "unassigned" | "longPaths">, dataFile: string): string[] {
  if (to !== "perPerson") return [];
  const out: string[] = [];
  if (plan.groupFiles > 0) out.push("Zdjęcie kilku osób trafi do folderu jednej z nich — w programie widać je u wszystkich.");
  if (plan.unassigned > 0) out.push(`${filesWord(plan.unassigned)} bez przypisanej osoby ${plural(plan.unassigned, "zostanie", "zostaną", "zostanie")} w folderze media.`);
  if (plan.longPaths > 0) out.push("Część folderów dostanie krótsze nazwy, bo pełna ścieżka byłaby za długa dla Windows.");
  out.push(`Ścieżki w pliku ${dataFile} zmienią się razem z plikami. Nic nie zostanie usunięte, a przeniesienie można cofnąć.`);
  out.push("Ten układ zobaczą wszyscy, którzy korzystają z tego archiwum.");
  return out;
}

// ---- Results: toasts ----

export const MOVING_LABEL = "Przenoszę pliki…";
export const MOVING_BACK_LABEL = "Przenoszę pliki z powrotem…";

/** „6 plików było otwartych w innym programie — zostały na miejscu. Spróbuj ponownie później.” */
export function heldText(held: number): string {
  const were = plural(held, "był otwarty", "były otwarte", "było otwartych");
  const stayed = plural(held, "został", "zostały", "zostały");
  return `${filesWord(held)} ${were} w innym programie — ${stayed} na miejscu. Spróbuj ponownie później.`;
}

export function movedToast(r: Pick<RestructureResult, "moved" | "held"> & { assigned?: number }): { text: string; detail?: string; kind?: "warn" } {
  const text = r.moved > 0 ? `Przeniesiono ${filesWord(r.moved)}.` : "Żaden plik nie został przeniesiony.";
  // Assigning the unpacked files is an ordinary change: it waits for Zapisz like any other.
  const detail = [r.held > 0 ? heldText(r.held) : null, (r.assigned ?? 0) > 0 ? `Przypisanie ${count(r.assigned!, "pliku", "plików", "plików")} czeka na zapis.` : null].filter(Boolean).join(" ");
  // Nothing moved is not a success: no green check.
  return { text, detail: detail || undefined, ...(r.moved > 0 ? {} : { kind: "warn" as const }) };
}

export function undoneToast(r: Pick<UndoRestructureResult, "movedBack" | "notBack">): { text: string; detail?: string } {
  const text = r.movedBack > 0 ? `Cofnięto porządkowanie: ${filesWord(r.movedBack)} ${plural(r.movedBack, "wrócił", "wróciły", "wróciło")} na dawne miejsce.` : "Cofnięto porządkowanie.";
  const detail = r.notBack > 0 ? `${filesWord(r.notBack)} ${plural(r.notBack, "został", "zostały", "zostało")} na nowym miejscu, bo na dawnym leży już inny plik.` : undefined;
  return { text, detail };
}

export const UNDO_ASK = {
  title: "Cofnąć porządkowanie?",
  text: "Heirloom przeniesie pliki z powrotem na dawne miejsca. Zdjęcia będą widoczne przez cały czas.",
};

// ---- Interrupted run (B.6) ----

export function interruptedText(i: Pick<LayoutInterrupted, "done" | "total">): string {
  return `Przenoszenie plików zostało przerwane (przeniesiono ${num(i.done)} z ${num(i.total)}). Wszystkie zdjęcia są widoczne.`;
}

// ---- Start banner (B.5) ----

/** „5 plików bez przypisania, 1 folder”. */
export function bannerParts(scan: Pick<MediaScan, "unknown" | "folders" | "counts">, overlay = false): string {
  const c = scan.counts;
  const parts: string[] = [];
  const unknown = looseUnknown(scan, overlay);
  if (unknown > 0) parts.push(`${filesWord(unknown)} bez przypisania`);
  if (!overlay && c.looseLinked > 0) parts.push(`${filesWord(c.looseLinked)} luzem`);
  if (c.moved > 0) parts.push(`${filesWord(c.moved)} w innym miejscu`);
  if (c.outside > 0) parts.push(`${filesWord(c.outside)} poza archiwum`);
  const folders = tidyFolders(scan, overlay);
  if (folders > 0) parts.push(foldersWord(folders));
  return parts.join(", ");
}

export function bannerText(scan: Pick<MediaScan, "unknown" | "folders" | "counts">, overlay = false): string {
  return `W folderze media jest coś do uporządkowania: ${bannerParts(scan, overlay)}.`;
}

/** Calm: only when something is found, and "Nie teraz" wasn't said to exactly these findings; never for sets. In A1 only
 *  what the dialog can still do something about counts, so the banner never points to an empty dialog. */
export function bannerShows(scan: Pick<MediaScan, "total" | "seen" | "unknown" | "folders" | "counts"> | null, combined: boolean, overlay = false): boolean {
  if (!scan || combined || scan.seen) return false;
  return overlay ? tidyFiles(scan, true) + tidyFolders(scan, true) > 0 : scan.total > 0;
}

// ---- Brakujące pliki ----

export const foundElsewhereText = (n: number) => `Znaleziono w innym folderze archiwum (${num(n)})`;

// ---- The tidy dialog (B.5) ----

export const TIDY_TITLE = "Porządek w zdjęciach";
export const A1_NOTE = "W trybie „Nie ruszaj oryginału” pliki w archiwum zostają na miejscu — możesz je tylko przypisać do osób.";
/** The button under the unknown files: only rows set to a person are „przypisane”; rows left in the folder are just kept. */
export function unknownApplyLabel(assign: number, leave: number): string {
  if (assign + leave === 0) return "Przypisz zaznaczone";
  if (leave === 0) return `Przypisz zaznaczone (${num(assign)})`;
  if (assign === 0) return `Zostaw zaznaczone (${num(leave)})`;
  return `Zastosuj wybór (${num(assign + leave)})`;
}
export const UNKNOWN_FOOTER = "Pliki zostawione w folderze nie pojawią się w programie. O te same pliki nie zapytamy ponownie.";

export const movedSection = (n: number) =>
  `${filesWord(n)} ${plural(n, "jest", "są", "jest")} w innym miejscu, niż zapisano (np. ktoś ${plural(n, "go", "je", "je")} przeniósł w Eksploratorze). Zdjęcia są widoczne, ale inne programy ${plural(n, "go", "ich", "ich")} nie znajdą.`;
export const looseSection = (n: number) => `${count(n, "zdjęcie", "zdjęcia", "zdjęć")} ${plural(n, "osoby leży", "osób leżą", "osób leży")} luzem w folderze media.`;
export const unknownSection = (n: number) =>
  `${filesWord(n)} w folderze media nie ${plural(n, "jest przypisany", "są przypisane", "jest przypisanych")} do nikogo — Heirloom ${plural(n, "go", "ich", "ich")} nie pokazuje.`;
export const outsideSection = (n: number) =>
  `${filesWord(n)} ${plural(n, "leży", "leżą", "leży")} poza folderem archiwum (np. C:\\Users\\…). Heirloom ${plural(n, "go", "ich", "ich")} nie pokazuje.`;

/** „W folderze media jest folder „Kowalski Józef” (23 pliki, w tym 5 nieznanych programowi). Może ktoś odłożył go tam na chwilę.” */
export function unpackQuestion(f: Pick<ScanFolder, "name" | "files" | "unknown">): string {
  const unknown = f.unknown > 0 ? `, w tym ${f.unknown} ${plural(f.unknown, "nieznany", "nieznane", "nieznanych")} programowi` : "";
  return `W folderze media jest folder „${f.name}” (${filesWord(f.files)}${unknown}). Może ktoś odłożył go tam na chwilę.`;
}

/** How many folders are asked about one by one; the rest are asked together (a flat archive that still has a folder for each person). */
export const FOLDERS_ONE_BY_ONE = 3;

/** „Jest tu jeszcze 38 folderów (41 plików). Może ktoś odłożył je tam na chwilę.” */
export function moreFoldersQuestion(folders: Pick<ScanFolder, "files">[]): string {
  const n = folders.length;
  const files = folders.reduce((sum, f) => sum + f.files, 0);
  return `W folderze media ${plural(n, "jest jeszcze", "są jeszcze", "jest jeszcze")} ${foldersWord(n)} (${filesWord(files)}). Może ktoś ${plural(n, "odłożył go", "odłożył je", "odłożył je")} tam na chwilę.`;
}

/** „Folder „Dziadek Józek” (23 pliki) — czyj to folder?” */
export function whoseQuestion(f: Pick<ScanFolder, "name" | "files">): string {
  return `Folder „${f.name}” (${filesWord(f.files)}) — czyj to folder?`;
}

export function personLabel(p: { name: string; years: string | null }): string {
  return p.years ? `${p.name} (${p.years})` : p.name;
}

/** A person's years as the scan prints them: „1878–1944”, „1878–”, „–1944”. */
export function yearsOf(birth: string | null | undefined, death: string | null | undefined, living = false): string | null {
  const b = birth?.match(/\d{4}/)?.[0] ?? "";
  const d = death?.match(/\d{4}/)?.[0] ?? "";
  if (!b && !d) return null;
  return living && !d ? `${b}–` : `${b}–${d}`;
}

/** Which sections the tidy dialog shows. Unknown files in a folder that is asked about belong to that question. */
export interface TidySections {
  moved: boolean;
  loose: boolean;
  unknown: ScanUnknown[];
  unpack: ScanFolder[];
  whose: ScanFolder[];
  outside: boolean;
}

export function tidySections(scan: MediaScan, overlay: boolean, dismissed: ReadonlySet<string> = new Set()): TidySections {
  const asked = scan.folders.filter((f) => f.question && !f.leftAlone && !dismissed.has(f.path));
  // In A1 an unpack question is not asked (nothing moves), so its unknown files stay in the assign list.
  const askedPaths = new Set(scan.folders.filter((f) => f.question && !(overlay && f.question === "unpack")).map((f) => f.path));
  return {
    moved: scan.moved.length > 0,
    loose: !overlay && scan.looseLinked.length > 0 && !dismissed.has("loose"),
    unknown: scan.unknown.filter((u) => !askedPaths.has(u.folder)),
    unpack: overlay ? [] : asked.filter((f) => f.question === "unpack"),
    whose: asked.filter((f) => f.question === "whose"),
    outside: scan.outside.length > 0,
  };
}

/** Nothing left to show: the dialog says so instead of staying empty. */
export function tidyEmpty(s: TidySections): boolean {
  return !s.moved && !s.loose && s.unknown.length === 0 && s.unpack.length === 0 && s.whose.length === 0 && !s.outside;
}

// ---- Historia zmian ----

/** The journal of a „Uporządkowano pliki” row (batch `porzadkowanie-<ts>`), which media.undoRestructure takes; null for the
 *  row of an undo itself (`…-cofniecie`) and for batches that aren't a run. */
export function layoutJournal(batch: string | null | undefined): string | null {
  const m = batch?.match(/^porzadkowanie-(.+)$/);
  return m && !m[1].endsWith("-cofniecie") ? m[1] : null;
}
