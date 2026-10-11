import { describe, expect, it } from "vitest";
import type { RebaseConflict } from "../../api/types";
import {
  WRITE_BACK_REFUSALS,
  choicesToSend,
  conflictsText as rawConflicts,
  defaultChoices,
  fieldsLine,
  keepRestNote,
  rebaseText as rawRebase,
  sideNote,
  undoConfirm,
  withChangesLabel,
  writeBackDone as rawDone,
  writeBackRefusal,
} from "./writeBack";

const plain = (text: string) => text.replace(/ /g, " ");
const rebaseText = (...a: Parameters<typeof rawRebase>) => plain(rawRebase(...a));
const conflictsText = (...a: Parameters<typeof rawConflicts>) => plain(rawConflicts(...a));
const writeBackDone = (r: { changes: number; files: number }) => {
  const d = rawDone(r);
  return { text: d.text, detail: plain(d.detail) };
};

const conflict = (patch: Partial<RebaseConflict>): RebaseConflict => ({
  key: "@I1@",
  tag: "INDI",
  name: "Jan Kowalski",
  default: "mine",
  mine: "0 @I1@ INDI",
  theirs: "0 @I1@ INDI",
  base: "0 @I1@ INDI",
  fields: ["Imię"],
  mineChanges: [],
  theirChanges: [],
  ...patch,
});

describe("the refusals of the write-back", () => {
  const ok = { originalMissing: false, lockedOriginal: false, writable: true };
  it("says nothing when the way is clear", () => expect(writeBackRefusal(ok)).toBeNull());
  it("names the lock and the unwritable archive", () => {
    expect(writeBackRefusal({ ...ok, lockedOriginal: true })).toBe(WRITE_BACK_REFUSALS.lockedOriginal);
    expect(writeBackRefusal({ ...ok, writable: false })).toBe(WRITE_BACK_REFUSALS.notWritable);
    expect(WRITE_BACK_REFUSALS.lockedOriginal).toBe("Właściciel archiwum zablokował zmiany (Tylko do odczytu). Twoje zmiany zostają u Ciebie.");
    expect(WRITE_BACK_REFUSALS.notWritable).toBe("W archiwum nie da się nic zapisać (np. płyta lub zablokowany pendrive). Twoje zmiany zostają u Ciebie.");
  });
  it("puts a missing archive before the rest", () => {
    expect(writeBackRefusal({ originalMissing: true, lockedOriginal: true, writable: false })).toBe(WRITE_BACK_REFUSALS.originalMissing);
  });
});

describe("the toast after a write-back", () => {
  it("counts changes and files", () => {
    expect(writeBackDone({ changes: 37, files: 12 })).toEqual({ text: "Przeniesiono zmiany do archiwum.", detail: "37 zmian, 12 plików." });
    expect(writeBackDone({ changes: 1, files: 1 }).detail).toBe("1 zmiana, 1 plik.");
    expect(writeBackDone({ changes: 2, files: 0 }).detail).toBe("2 zmiany.");
    expect(writeBackDone({ changes: 0, files: 3 }).detail).toBe("3 pliki.");
  });
});

describe("the undo question", () => {
  it("names the time of the write-back", () => {
    const at = new Date(2026, 9, 3, 14, 5).toISOString();
    const q = undoConfirm(at);
    expect(q.title).toBe("Cofnąć przeniesienie zmian do archiwum?");
    expect(q.text).toBe("Archiwum wróci do stanu sprzed 14:05. Twoje zmiany zostaną u Ciebie.");
    expect(undoConfirm(null).text).toContain("sprzed tego przeniesienia");
  });
});

describe("the rebase dialog", () => {
  it("says since when and how much, with Polish plurals", () => {
    const since = new Date(2026, 9, 3, 12).toISOString();
    expect(rebaseText({ since, theirChanges: 14, myChanges: 37 })).toBe("Od 3 października ktoś zmienił archiwum (14 zmian). Ty masz 37 zmian. Heirloom połączy jedno z drugim — nic nie zginie.");
    expect(rebaseText({ since, theirChanges: 1, myChanges: 1 })).toContain("(1 zmiana). Ty masz 1 zmianę.");
    expect(rebaseText({ since, theirChanges: 2, myChanges: 22 })).toContain("(2 zmiany). Ty masz 22 zmiany.");
    expect(rebaseText({ since: "", theirChanges: 3, myChanges: 5 })).toMatch(/^Ktoś zmienił archiwum/);
  });
  it("counts the records both sides changed", () => {
    const p = (n: number) => Array.from({ length: n }, () => ({ tag: "INDI" }));
    expect(conflictsText(p(3))).toBe("U 3 osób zmieniono to samo i u Ciebie, i w archiwum. Wybierz, którą wersję zostawić:");
    expect(conflictsText(p(1))).toMatch(/^U 1 osoby /);
    expect(conflictsText(p(2))).toMatch(/^U 2 osób /);
    expect(conflictsText([{ tag: "INDI" }, { tag: "SOUR" }])).toMatch(/^U 2 wpisów /);
    expect(conflictsText([{ tag: "NOTE" }])).toMatch(/^U 1 wpisu /);
  });
  it("names the clashing fields and promises to keep the rest", () => {
    expect(fieldsLine(conflict({ fields: ["Urodzenie · data"] }))).toBe("Zmieniono po obu stronach: Urodzenie · data");
    expect(fieldsLine(conflict({ fields: ["Imię", "Nazwisko"] }))).toBe("Zmieniono po obu stronach: Imię, Nazwisko");
    expect(fieldsLine(conflict({ fields: [], mine: null }))).toBe("Usunięto u Ciebie, a w archiwum zmieniono.");
    expect(fieldsLine(conflict({ fields: [], theirs: null }))).toBe("Usunięto w archiwum, a u Ciebie zmieniono.");
    expect(keepRestNote(conflict({}))).toBe("Pozostałe zmiany z obu stron zostaną zachowane.");
    expect(keepRestNote(conflict({ fields: [], mine: null }))).toBeNull();
  });
  it("preselects the backend's default and sends a choice for every conflict", () => {
    const list = [conflict({ key: "@I1@" }), conflict({ key: "@I2@", default: "theirs", mine: null })];
    expect(defaultChoices(list)).toEqual({ "@I1@": "mine", "@I2@": "theirs" });
    expect(choicesToSend(list, { "@I1@": "theirs" })).toEqual({ "@I1@": "theirs", "@I2@": "theirs" });
    expect(choicesToSend(list, {})).toEqual({ "@I1@": "mine", "@I2@": "theirs" });
  });
  it("says when a side deleted the record", () => {
    expect(sideNote(conflict({ mine: null }), "mine")).toBe("usunięte u Ciebie");
    expect(sideNote(conflict({ theirs: null }), "theirs")).toBe("usunięte w archiwum");
    expect(sideNote(conflict({}), "mine")).toBeNull();
  });
});

describe("archives opened together", () => {
  it("names the count of the user's changes", () => {
    expect(withChangesLabel({ withChanges: true, pending: 14 })).toBe("(z Twoimi zmianami: 14)");
    expect(withChangesLabel({ withChanges: true, pending: 0 })).toBe("(z Twoimi zmianami)");
    expect(withChangesLabel({ withChanges: false, pending: 3 })).toBeNull();
    expect(withChangesLabel({})).toBeNull();
  });
});
