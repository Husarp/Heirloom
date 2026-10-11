import { describe, expect, it } from "vitest";
import type { BackupInfo } from "../../api/types";
import {
  backupNotes,
  backupSizeText,
  backupStatusLine,
  backupWhen,
  CANCELLED_TEXT,
  doneToast,
  lastBackupNote,
  noRoom,
  readyText,
  runningText,
} from "./backupText";
import { foreignLine } from "./workModes";

const plain = (text: string) => text.replace(/\u00a0/g, " ");
const now = new Date(2026, 9, 10, 16, 0);
const at = (days: number, h = 14, m = 5) => new Date(2026, 9, 10 - days, h, m).toISOString();

const info = (patch: Partial<BackupInfo> = {}): BackupInfo => ({
  folder: "C:\\Users\\Ewa\\Documents\\Heirloom\\Kopie zapasowe\\Rodzina",
  folderLabel: "Dokumenty › Heirloom › Kopie zapasowe",
  files: 1230,
  bytes: 1_200_000_000,
  needed: 1_310_000_000,
  free: 35_000_000_000,
  lastZip: null,
  current: false,
  inOneDrive: false,
  sameDisk: false,
  cloudOnly: { files: 0, bytes: 0 },
  ...patch,
});

describe("backup status line", () => {
  it("says when there is no backup yet (or its file is gone)", () => {
    expect(backupStatusLine(info())).toBe("Nie ma jeszcze kopii zapasowej tego archiwum.");
    expect(backupStatusLine(info({ lastZip: { path: "x.zip", at: at(1), bytes: 5, exists: false } }), now)).toBe("Nie ma jeszcze kopii zapasowej tego archiwum.");
  });
  it("names the last backup", () => {
    expect(backupStatusLine(info({ lastZip: { path: "x.zip", at: at(3), bytes: 5, exists: true } }), now)).toBe("Ostatnia kopia zapasowa: 3 dni temu.");
    expect(backupStatusLine(info({ lastZip: { path: "x.zip", at: at(0), bytes: 5, exists: true } }), now)).toBe("Ostatnia kopia zapasowa: dziś 14:05.");
  });
  it("says a backup is current when nothing changed since", () => {
    const last = { path: "x.zip", at: at(0), bytes: 5, exists: true };
    expect(backupStatusLine(info({ lastZip: last, current: true }), now)).toBe("✓ Kopia z dziś 14:05 jest aktualna — od niej nic się nie zmieniło.");
    expect(backupStatusLine(info({ lastZip: { ...last, at: at(1) }, current: true }), now)).toContain("Kopia z wczoraj 14:05 jest aktualna");
    expect(backupStatusLine(info({ lastZip: { ...last, at: at(40) }, current: true }), now)).toContain("Kopia z 31.08.2026 14:05 jest aktualna");
  });
  it("writes a date for older backups in a sentence", () => {
    expect(backupWhen(at(5), now)).toBe("05.10.2026 14:05");
  });
});

describe("backup size and room", () => {
  it("shows the size and the free room", () => {
    expect(plain(backupSizeText(info()))).toBe("ok. 1,2 GB (wolne: 35 GB)");
    expect(plain(backupSizeText(info({ free: null })))).toBe("ok. 1,2 GB");
  });
  it("knows when there is no room", () => {
    expect(noRoom(info())).toBe(false);
    expect(noRoom(info({ free: null }))).toBe(false);
    expect(noRoom(info({ free: 2_300_000_000, needed: 6_100_000_000 }))).toBe(true);
  });
});

describe("backup notes", () => {
  it("has none for an ordinary backup", () => {
    expect(backupNotes(info())).toEqual([]);
  });
  it("says there is not enough room, with the disk, and offers another place", () => {
    const [note] = backupNotes(info({ free: 2_300_000_000, needed: 6_100_000_000, bytes: 5_800_000_000 }));
    expect(note.kind).toBe("noRoom");
    expect(note.choose).toBe(true);
    expect(plain(note.text)).toBe("Za mało miejsca na dysku C: — kopia zajmie ok. 6,1 GB, a wolne jest 2,3 GB. Wybierz inne miejsce, np. pendrive.");
  });
  it("warns about OneDrive only when the zip is over 1 GB", () => {
    expect(backupNotes(info({ inOneDrive: true, bytes: 900_000_000 }))).toEqual([]);
    expect(plain(backupNotes(info({ inOneDrive: true }))[0].text)).toBe("Folder kopii jest w OneDrive — kopia (1,2 GB) zajmie tam miejsce i będzie wysyłana do chmury.");
  });
  it("warns about the same disk", () => {
    expect(backupNotes(info({ sameDisk: true }))[0].text).toBe("Kopia będzie na tym samym dysku co archiwum. Dla pewności skopiuj ją czasem na pendrive.");
  });
  it("names cloud-only files with the right Polish forms", () => {
    const text = (files: number) => plain(backupNotes(info({ cloudOnly: { files, bytes: 2_300_000_000 } }))[0].text);
    expect(text(215)).toBe("215 plików archiwum jest tylko w chmurze OneDrive — kopia je pobierze (2,3 GB).");
    expect(text(1)).toBe("1 plik archiwum jest tylko w chmurze OneDrive — kopia go pobierze (2,3 GB).");
    expect(text(3)).toBe("3 pliki archiwum są tylko w chmurze OneDrive — kopia je pobierze (2,3 GB).");
  });
  it("lists the most pressing note first", () => {
    const kinds = backupNotes(info({ free: 1, sameDisk: true, inOneDrive: true, cloudOnly: { files: 2, bytes: 5 } })).map((n) => n.kind);
    expect(kinds).toEqual(["noRoom", "oneDrive", "sameDisk", "cloudOnly"]);
  });
});

describe("backup progress and results", () => {
  it("counts the files, with the plural of the total", () => {
    expect(plain(runningText({ done: 412, total: 1230 }))).toBe("Tworzę kopię zapasową… 412 z 1 230 plików");
    expect(plain(runningText({ done: 0, total: 1 }))).toBe("Tworzę kopię zapasową… 0 z 1 pliku");
    expect(plain(runningText({ done: 1, total: 3 }))).toBe("Tworzę kopię zapasową… 1 z 3 plików");
    expect(runningText(null)).toBe("Tworzę kopię zapasową…");
    expect(runningText({ done: 0, total: 0 })).toBe("Tworzę kopię zapasową…");
  });
  it("says the backup is ready, with the file's name from either kind of path", () => {
    expect(plain(readyText({ path: "C:\\Kopie\\Rodzina\\Heirloom-Rodzina-2026-10-10-1405.zip", bytes: 1_200_000_000, files: 3 }))).toBe(
      "✓ Kopia gotowa: Heirloom-Rodzina-2026-10-10-1405.zip (1,2 GB)",
    );
    expect(readyText({ path: "/tmp/k/a.zip", bytes: 5, files: 1 })).toContain("a.zip");
  });
  it("makes the toast and the cancelled text", () => {
    expect(plain(doneToast({ path: "x", bytes: 1_200_000_000, files: 1230 }))).toBe("Utworzono kopię zapasową: 1 230 plików, 1,2 GB.");
    expect(plain(doneToast({ path: "x", bytes: 2000, files: 1 }))).toBe("Utworzono kopię zapasową: 1 plik, 2,0 KB.");
    expect(CANCELLED_TEXT).toBe("Przerwano tworzenie kopii — nic nie zostało zapisane.");
  });
  it("makes the Settings note", () => {
    expect(plain(lastBackupNote(info(), now))).toBe("Jeszcze nie było kopii.");
    expect(plain(lastBackupNote(info({ lastZip: { path: "x.zip", at: at(3), bytes: 1_200_000_000, exists: true } }), now))).toBe("Ostatnia: 3 dni temu (1,2 GB)");
  });
});

describe("the foreign-file line", () => {
  it("names the program and the format", () => {
    expect(foreignLine({ origin: "MyHeritage", version: "5.5.1", dataFile: "drzewo.ged" })).toBe(
      "Plik drzewo.ged utworzył program MyHeritage (GEDCOM 5.5.1). Po zapisie będzie w formacie GEDCOM 7 — MyHeritage może go nie otworzyć. Dane, których Heirloom nie zna, zostaną w pliku.",
    );
  });
  it("copes with an unknown program", () => {
    expect(foreignLine({ origin: null, version: null, dataFile: "a.ged" })).toBe(
      "Plik a.ged utworzył inny program. Po zapisie będzie w formacie GEDCOM 7 — ten program może go nie otworzyć. Dane, których Heirloom nie zna, zostaną w pliku.",
    );
  });
});
