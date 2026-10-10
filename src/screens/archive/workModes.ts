// „Pracuj na archiwum” and „Nie ruszaj oryginału”: the texts and choices the question, the final-change dialog and
// Settings share (pure, so they are tested on their own).

import type { ModeInfo } from "../../api/types";
import { bytes, count, num, plural } from "../../lib/format";

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
