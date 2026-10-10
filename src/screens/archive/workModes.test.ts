import { describe, expect, it } from "vitest";
import type { ModeInfo } from "../../api/types";
import { defaultMode, finalChangeText as rawFinal, modeNote, pendingText as rawPending } from "./workModes";

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
