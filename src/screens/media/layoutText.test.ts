import { describe, expect, it } from "vitest";
import type { MediaScan, ScanFolder, ScanUnknown } from "../../api/types";
import {
  bannerShows,
  bannerText,
  filesWord,
  heldText,
  interruptedText,
  lastRunText,
  layoutAsInArchive,
  layoutJournal,
  layoutNow,
  movedToast,
  switchBullets,
  switchText,
  switchTitle,
  tidyEmpty,
  tidyNote,
  tidySections,
  undoneToast,
  unknownSection,
  unpackQuestion,
  whoseQuestion,
  yearsOf,
} from "./layoutText";

const NB = " ";
const counts = (c: Partial<MediaScan["counts"]> = {}): MediaScan["counts"] => ({ moved: 0, looseLinked: 0, unknown: 0, folders: 0, outside: 0, missing: 0, files: 0, ...c });
const folder = (f: Partial<ScanFolder>): ScanFolder => ({ path: "media/x", name: "x", files: 3, linked: 0, unknown: 3, person: null, leftAlone: false, changed: false, question: null, ...f });
const file = (path: string, folderPath: string): ScanUnknown => ({ path, name: path.split("/").pop()!, size: 1, folder: folderPath, suggestion: null, candidates: [] });
function scan(p: Partial<MediaScan> = {}): MediaScan {
  return {
    layout: { setting: "flat", guess: "flat", effective: "flat", rootOwned: 0, folderOwned: 0, personFolders: 0 },
    moved: [], looseLinked: [], unknown: [], folders: [], outside: [], counts: counts(), total: 0, fingerprint: "f", seen: false, truncated: false, journal: null, interrupted: null,
    ...p,
  };
}

describe("Polish plurals", () => {
  it("files", () => {
    expect(filesWord(1)).toBe(`1${NB}plik`);
    expect(filesWord(2)).toBe(`2${NB}pliki`);
    expect(filesWord(5)).toBe(`5${NB}plików`);
    expect(filesWord(12)).toBe(`12${NB}plików`);
    expect(filesWord(22)).toBe(`22${NB}pliki`);
  });
  it("held files", () => {
    expect(heldText(1)).toContain("plik był otwarty");
    expect(heldText(1)).toContain("został na miejscu");
    expect(heldText(3)).toContain("pliki były otwarte");
    expect(heldText(6)).toBe(`6${NB}plików było otwartych w innym programie — zostały na miejscu. Spróbuj ponownie później.`);
  });
});

describe("Settings", () => {
  it("says where the files lie now", () => {
    expect(layoutNow(scan({ layout: { ...scan().layout, folderOwned: 312, rootOwned: 12 } }))).toBe(`Teraz: 312${NB}plików w folderach osób, 12 luzem`);
  });
  it("A1 shows the original's layout read-only", () => {
    expect(layoutAsInArchive("perPerson")).toBe("Jak w archiwum: w folderach osób. Pliki w archiwum przeniesiesz w trybie „Pracuj na archiwum”.");
  });
  it("counts what is to tidy", () => {
    expect(tidyNote(scan({ counts: counts({ unknown: 10, moved: 4 }) }))).toBe(`Do uporządkowania: 14${NB}plików`);
    expect(tidyNote(scan({ counts: counts({ unknown: 1, folders: 2 }) }))).toBe(`Do uporządkowania: 1${NB}plik, 2${NB}foldery`);
    expect(tidyNote(scan())).toBe("Do uporządkowania: nic");
  });
  it("names the last run", () => {
    expect(lastRunText({ name: "n", ts: "2026-10-03T10:00:00Z", state: "saved", moved: 1198, total: 1198 })).toBe(`3 października (1${NB}198${NB}plików)`);
  });
});

describe("the switch dialogs", () => {
  const plan = { count: 1204, newFolders: ["media/Kowalski Józef"], groupFiles: 3, unassigned: 12, longPaths: 0 };
  it("to per-person", () => {
    expect(switchTitle("perPerson")).toBe("Ułożyć pliki w folderach osób?");
    expect(switchText("perPerson", plan)).toBe(
      `Nowe zdjęcia i dokumenty będą trafiać do folderów osób, np. media\\Kowalski Józef. Heirloom może też od razu przenieść tam 1${NB}204 pliki, które już są w archiwum.`,
    );
    expect(switchBullets("perPerson", plan, "rodzina.ged")).toEqual([
      "Zdjęcie kilku osób trafi do folderu jednej z nich — w programie widać je u wszystkich.",
      `12${NB}plików bez przypisanej osoby zostanie w folderze media.`,
      "Ścieżki w pliku rodzina.ged zmienią się razem z plikami. Nic nie zostanie usunięte, a przeniesienie można cofnąć.",
      "Ten układ zobaczą wszyscy, którzy korzystają z tego archiwum.",
    ]);
  });
  it("leaves out bullets that do not apply", () => {
    const b = switchBullets("perPerson", { groupFiles: 0, unassigned: 0, longPaths: 0 }, "a.ged");
    expect(b).toHaveLength(2);
  });
  it("to flat", () => {
    expect(switchTitle("flat")).toBe("Wyjąć pliki z folderów osób?");
    expect(switchText("flat", { count: 1204, newFolders: [] })).toBe(
      `Heirloom przeniesie 1${NB}204${NB}pliki z folderów osób prosto do folderu media, a puste foldery usunie. Gdy dwa pliki mają tę samą nazwę, drugi dostanie numer, np. „ślub (2).jpg”. Foldery z plikami, których Heirloom nie zna, zostaną.`,
    );
    expect(switchBullets("flat", plan, "a.ged")).toEqual([]);
  });
  it("nothing to move", () => {
    expect(switchText("flat", { count: 0, newFolders: [] })).toContain("nic nie trzeba przenosić");
    expect(switchText("perPerson", { count: 0, newFolders: [] })).not.toContain("od razu przenieść");
  });
});

describe("results", () => {
  it("moved", () => {
    expect(movedToast({ moved: 1198, held: 0 })).toEqual({ text: `Przeniesiono 1${NB}198${NB}plików.`, detail: undefined });
    expect(movedToast({ moved: 0, held: 2 }).text).toBe("Żaden plik nie został przeniesiony.");
  });
  it("undone", () => {
    expect(undoneToast({ movedBack: 5, notBack: 0 }).text).toBe(`Cofnięto porządkowanie: 5${NB}plików wróciło na dawne miejsce.`);
    expect(undoneToast({ movedBack: 5, notBack: 1 }).detail).toContain("na nowym miejscu");
  });
  it("interrupted", () => {
    expect(interruptedText({ done: 120, total: 340 })).toBe("Przenoszenie plików zostało przerwane (przeniesiono 120 z 340). Wszystkie zdjęcia są widoczne.");
  });
});

describe("the Start banner", () => {
  it("lists what was found", () => {
    expect(bannerText(scan({ counts: counts({ unknown: 5, folders: 1 }) }))).toBe(`W folderze media jest coś do uporządkowania: 5${NB}plików bez przypisania, 1${NB}folder.`);
    expect(bannerText(scan({ counts: counts({ moved: 12, looseLinked: 8, outside: 4, folders: 2 }) }))).toContain(`8${NB}plików luzem, 12${NB}plików w innym miejscu, 4${NB}pliki poza archiwum, 2${NB}foldery`);
  });
  it("shows only for new findings, never for sets", () => {
    expect(bannerShows(scan({ total: 3 }), false)).toBe(true);
    expect(bannerShows(scan({ total: 3, seen: true }), false)).toBe(false);
    expect(bannerShows(scan({ total: 0 }), false)).toBe(false);
    expect(bannerShows(scan({ total: 3 }), true)).toBe(false);
    expect(bannerShows(null, false)).toBe(false);
  });
});

describe("the tidy dialog", () => {
  it("texts", () => {
    expect(unknownSection(5)).toBe(`5${NB}plików w folderze media nie jest przypisanych do nikogo — Heirloom ich nie pokazuje.`);
    expect(unpackQuestion({ name: "Kowalski Józef", files: 23, unknown: 5 })).toBe(
      `W folderze media jest folder „Kowalski Józef” (23${NB}pliki, w tym 5 nieznanych programowi). Może ktoś odłożył go tam na chwilę.`,
    );
    expect(whoseQuestion({ name: "Dziadek Józek", files: 23 })).toBe(`Folder „Dziadek Józek” (23${NB}pliki) — czyj to folder?`);
  });
  it("years", () => {
    expect(yearsOf("12 marca 1878", "1944")).toBe("1878–1944");
    expect(yearsOf("1878", null, true)).toBe("1878–");
    expect(yearsOf(null, null)).toBeNull();
  });
  it("shows only the sections with findings; files of a questioned folder belong to its question", () => {
    const s = scan({
      unknown: [file("media/a.jpg", "media"), file("media/Dziadek Józek/b.jpg", "media/Dziadek Józek")],
      folders: [folder({ path: "media/Dziadek Józek", name: "Dziadek Józek", question: "whose" }), folder({ path: "media/skany", question: "unpack" })],
      moved: [{ id: "@O1@", file: "media/x.jpg", found: "media/y/x.jpg" }],
    });
    const t = tidySections(s, false);
    expect(t.moved).toBe(true);
    expect(t.unknown.map((u) => u.path)).toEqual(["media/a.jpg"]);
    expect(t.whose).toHaveLength(1);
    expect(t.unpack).toHaveLength(1);
    expect(t.loose).toBe(false);
    expect(tidyEmpty(t)).toBe(false);
  });
  it("A1 hides unpack and keeps its unknown files assignable", () => {
    const s = scan({ unknown: [file("media/skany/b.jpg", "media/skany")], folders: [folder({ path: "media/skany", question: "unpack" })] });
    const t = tidySections(s, true);
    expect(t.unpack).toHaveLength(0);
    expect(t.unknown).toHaveLength(1);
  });
  it("hides a folder left alone or dismissed; is empty then", () => {
    const s = scan({ folders: [folder({ path: "media/skany", question: "unpack", leftAlone: true }), folder({ path: "media/b", question: "whose" })] });
    expect(tidySections(s, false, new Set(["media/b"])).whose).toHaveLength(0);
    expect(tidyEmpty(tidySections(s, false, new Set(["media/b"])))).toBe(true);
  });
  it("loose files only in a per-person archive that can move", () => {
    const s = scan({ looseLinked: [{ id: "@O1@", ids: ["@O1@"], file: "media/a.jpg", owner: { id: "@I1@", name: "A", years: null } }] });
    expect(tidySections(s, false).loose).toBe(true);
    expect(tidySections(s, true).loose).toBe(false);
  });
});

describe("the history feed", () => {
  it("undoes a run through its journal, not history.undo", () => {
    expect(layoutJournal("porzadkowanie-20261003T101500Z")).toBe("20261003T101500Z");
    expect(layoutJournal("porzadkowanie-20261003T101500Z-cofniecie")).toBeNull();
    expect(layoutJournal("import-2026")).toBeNull();
    expect(layoutJournal(null)).toBeNull();
  });
});
