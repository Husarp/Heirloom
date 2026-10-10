import { describe, expect, it } from "vitest";
import type { ArchiveStatus, ModeInfo, OverlayStatus } from "../../api/types";
import {
  changesFolderShown,
  closeReminder as rawReminder,
  defaultMode,
  discardText,
  discardTitle,
  editorsNote,
  finalChangeText as rawFinal,
  lastWriteBackText,
  modeNote,
  pendingText as rawPending,
  pendingTitle,
  savesWhere,
  sidebarTip,
  startBanners,
  waitingText as rawWaiting,
  whereTip,
} from "./workModes";

/** The texts with ordinary spaces (numbers and their words are joined by no-break spaces). */
const plain = (text: string) => text.replace(/\u00a0/g, " ");
const finalChangeText = (...args: Parameters<typeof rawFinal>) => plain(rawFinal(...args));
const pendingText = (n: number) => plain(rawPending(n));

const info = (patch: Partial<ModeInfo>): ModeInfo => ({
  writable: true,
  foreign: null,
  usedBefore: false,
  editors: [],
  folder: { path: "C:\\Users\\Ewa\\Documents\\Heirloom\\Moje zmiany\\Rodzina", label: "Dokumenty › Heirloom › Moje zmiany › Rodzina" },
  suggested: "inPlace",
  ...patch,
});

describe("defaultMode", () => {
  it("leaves a folder that can't be written alone", () => {
    expect(defaultMode(info({ writable: false, usedBefore: true }))).toBe("overlay");
    expect(modeNote(info({ writable: false }))).toContain("nie da się nic zapisać");
  });
  it("keeps working in place on an archive used before", () => {
    expect(defaultMode(info({ usedBefore: true, foreign: "MyHeritage" }))).toBe("inPlace");
    expect(modeNote(info({ usedBefore: true }))).toBe("Do tej pory Heirloom zapisywał zmiany w tym archiwum.");
  });
  it("recommends leaving another program's file alone", () => {
    expect(defaultMode(info({ foreign: "MYHERITAGE" }))).toBe("overlay");
    expect(modeNote(info({ foreign: "MYHERITAGE" }))).toContain("MYHERITAGE");
  });
  it("is in place otherwise", () => {
    expect(defaultMode(info({}))).toBe("inPlace");
    expect(modeNote(info({}))).toBeNull();
  });
});

describe("finalChangeText", () => {
  it("counts in Polish", () => {
    expect(finalChangeText("import", { name: "Rodzina Kowalskich", people: 12, files: 30 })).toMatch(/^Import doda 12 osób i 30 plików do archiwum „Rodzina Kowalskich”\./);
    expect(finalChangeText("import", { name: "R", people: 1, files: 0 })).toMatch(/^Import doda 1 osobę do archiwum/);
    expect(finalChangeText("import", { name: "R", people: 3, files: 2 })).toMatch(/^Import doda 3 osoby i 2 pliki/);
    const back = finalChangeText("writeBack", { name: "Rodzina Kowalskich", root: "E:\\Rodzina Kowalskich", changes: 37, people: 12, files: 12, bytes: 48_000_000 });
    expect(back).toBe(
      "Do archiwum „Rodzina Kowalskich” (E:\\Rodzina Kowalskich) trafi: 37 zmian (12 osób) i 12 plików (48 MB). Zobaczy je każdy, kto otworzy to archiwum. Przeniesienie można cofnąć, dopóki nikt inny nie zmieni archiwum.",
    );
    expect(finalChangeText("writeBack", { name: "R", changes: 2, people: 1 })).toContain("trafią: 2 zmiany (1 osoba).");
    expect(finalChangeText("restructure", { name: "R", files: 1204, dataFile: "rodzina.ged" })).toContain("przeniesie 1 204 pliki w folderze media i zmieni ich ścieżki w pliku rodzina.ged");
  });
  it("says how many changes wait", () => {
    expect(pendingText(14)).toBe("Masz 14 zmian, których nie ma jeszcze w archiwum.");
    expect(pendingText(1)).toBe("Masz 1 zmianę, której nie ma jeszcze w archiwum.");
  });
});

const overlay = (patch: Partial<OverlayStatus> = {}): OverlayStatus => ({
  folder: "C:\\Users\\Ewa\\Documents\\Heirloom\\Moje zmiany\\Rodzina Kowalskich",
  folderLabel: "Dokumenty › Heirloom › Moje zmiany › Rodzina Kowalskich",
  dataPath: "C:\\x\\rodzina.ged",
  pending: 0,
  originalChanged: false,
  originalMissing: false,
  lastWriteBack: null,
  canUndoWriteBack: false,
  interrupted: null,
  sameDrive: false,
  ...patch,
});

const status = (patch: Partial<ArchiveStatus> = {}) =>
  ({ workMode: "overlay", dataFile: "rodzina.ged", dataPath: "E:\\Rodzina Kowalskich\\rodzina.ged", root: "E:\\Rodzina Kowalskich", overlay: overlay({ pending: 14 }), ...patch }) as ArchiveStatus;

describe("the question's notes and folder", () => {
  it("names the others who work on the archive", () => {
    expect(editorsNote([])).toBeNull();
    const now = new Date("2026-10-10T12:00:00");
    expect(editorsNote([{ name: "Ewa", lastEdited: "2026-10-07T09:00:00" }], now)).toBe("Na tym archiwum pracują też: Ewa (ostatnio 3 dni temu).");
    expect(editorsNote([{ name: "Ewa", lastEdited: null }, { name: "Jan", lastEdited: null }])).toBe("Na tym archiwum pracują też: Ewa, Jan.");
  });
  it("shows the suggested folder, or the chosen one with the archive's folder name", () => {
    expect(changesFolderShown(info({}), null)).toContain("Dokumenty › Heirloom");
    expect(changesFolderShown(info({}), "D:\\Pendrive\\")).toBe("D:\\Pendrive\\Rodzina");
    expect(changesFolderShown(info({}), "/home/ewa/zmiany")).toBe("/home/ewa/zmiany/Rodzina");
  });
});

describe("where changes are saved", () => {
  it("says the file in place and the folder of changes otherwise", () => {
    expect(savesWhere({ workMode: "inPlace", dataFile: "rodzina.ged" })).toBe("w pliku rodzina.ged");
    expect(savesWhere({ workMode: "overlay", dataFile: "rodzina.ged" })).toBe("u Ciebie, w folderze Twoich zmian");
  });
  it("tells both places in the sidebar and the edit bar", () => {
    expect(sidebarTip({ dataPath: "E:\\R\\rodzina.ged", overlay: null })).toBe("E:\\R\\rodzina.ged");
    expect(sidebarTip({ dataPath: "E:\\R\\rodzina.ged", overlay: overlay() })).toBe("Archiwum: E:\\R\\rodzina.ged · Twoje zmiany: C:\\Users\\Ewa\\Documents\\Heirloom\\Moje zmiany\\Rodzina Kowalskich");
    expect(whereTip(overlay())).toMatch(/^Twoje zmiany: C:\\Users\\Ewa/);
  });
});

describe("startBanners", () => {
  it("shows nothing without a folder of changes or without anything waiting", () => {
    expect(startBanners(null, [])).toEqual([]);
    expect(startBanners(overlay(), [])).toEqual([]);
  });
  it("lists what applies, interrupted first", () => {
    expect(startBanners(overlay({ pending: 3, originalChanged: true }), [])).toEqual(["originalChanged", "pending"]);
    expect(startBanners(overlay({ pending: 3, interrupted: "Przenoszenie przerwano." }), [])).toEqual(["interrupted", "pending"]);
  });
  it("says only that the archive is missing while it can't be reached", () => {
    expect(startBanners(overlay({ pending: 3, originalChanged: true, originalMissing: true }), [])).toEqual(["originalMissing"]);
  });
  it("leaves out what the user closed", () => {
    expect(startBanners(overlay({ pending: 3, originalChanged: true }), ["pending"])).toEqual(["originalChanged"]);
  });
});

describe("closeReminder", () => {
  const none = new Set<string>();
  it("reminds when the archive is on another drive and changes wait", () => {
    const r = rawReminder(status(), none)!;
    expect(r.title).toBe("Twoich zmian nie ma jeszcze w archiwum");
    expect(plain(r.text)).toBe("14 zmian jest zapisanych tylko na tym komputerze. Archiwum (E:\\Rodzina Kowalskich) ich nie ma.");
  });
  it("counts in Polish", () => {
    expect(plain(rawReminder(status({ overlay: overlay({ pending: 1 }) }), none)!.text)).toBe("1 zmiana jest zapisanych tylko na tym komputerze. Archiwum (E:\\Rodzina Kowalskich) jej nie ma.");
    expect(plain(rawReminder(status({ overlay: overlay({ pending: 3 }) }), none)!.text)).toContain("3 zmiany są zapisanych");
  });
  it("is silent for the same drive, nothing waiting, the archive missing, A2, or a second time", () => {
    expect(rawReminder(status({ overlay: overlay({ pending: 14, sameDrive: true }) }), none)).toBeNull();
    expect(rawReminder(status({ overlay: overlay({ pending: 0 }) }), none)).toBeNull();
    expect(rawReminder(status({ overlay: overlay({ pending: 5, originalMissing: true }) }), none)).toBeNull();
    expect(rawReminder(status({ workMode: "inPlace", overlay: null }), none)).toBeNull();
    expect(rawReminder(null, none)).toBeNull();
    expect(rawReminder(status(), new Set(["E:\\Rodzina Kowalskich\\rodzina.ged"]))).toBeNull();
  });
});

describe("Ustawienia › Twoje zmiany", () => {
  const waiting = (n: number, people: number, files: number, size: number) => plain(rawWaiting({ pending: n, people, newFiles: files, newBytes: size }));
  it("says what waits", () => {
    expect(waiting(37, 12, 12, 48_000_000)).toBe("37 zmian (12 osób), 12 nowych plików (48 MB)");
    expect(waiting(1, 1, 0, 0)).toBe("1 zmiana (1 osoba)");
    expect(waiting(0, 0, 2, 1_000_000)).toContain("2 nowe pliki");
    expect(waiting(0, 0, 0, 0)).toBe("Nic — archiwum ma wszystkie Twoje zmiany");
  });
  it("tells the last write-back", () => {
    expect(lastWriteBackText(null)).toBe("nigdy");
    expect(plain(lastWriteBackText("2026-10-03T14:05:00"))).toBe("3 października, 14:05");
  });
});

describe("switching and discarding", () => {
  it("asks about changes the archive lacks, in Polish", () => {
    expect(plain(pendingTitle(14))).toBe("Masz 14 zmian, których nie ma w archiwum");
    expect(plain(pendingTitle(1))).toBe("Masz 1 zmianę, której nie ma w archiwum");
    expect(plain(pendingTitle(3))).toBe("Masz 3 zmiany, których nie ma w archiwum");
  });
  it("asks before discarding, naming the kept folder", () => {
    expect(plain(discardTitle(14))).toBe("Porzucić 14 zmian?");
    expect(plain(discardTitle(0))).toBe("Zacząć od nowa?");
    expect(discardText("Rodzina Kowalskich", "2026-10-10")).toBe(
      "Folder Twoich zmian dostanie nazwę „Rodzina Kowalskich (porzucone 2026-10-10)” — możesz go jeszcze otworzyć przez „Otwórz folder…”. Zaczniesz od tego, co jest teraz w archiwum.",
    );
  });
});
