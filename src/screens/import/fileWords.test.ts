import { describe, expect, it } from "vitest";
import { matchPeople, missingLabel, needAttention as attention, notAttachedLabel, undescribedLabel } from "./fileWords";

describe("words by the kind of a file", () => {
  it("names the photo, the document and anything else", () => {
    expect([missingLabel("photo"), missingLabel("document"), missingLabel("other"), missingLabel("note")]).toEqual(["Brak zdjęcia", "Brak dokumentu", "Brak pliku", "Brak pliku"]);
    expect([undescribedLabel("photo"), undescribedLabel("document"), undescribedLabel(undefined)]).toEqual(["Zdjęcie nieopisane", "Dokument nieopisany", "Plik nieopisany"]);
    expect([notAttachedLabel("photo"), notAttachedLabel("document"), notAttachedLabel("other")]).toEqual(["Nie dołączono zdjęcia", "Nie dołączono dokumentu", "Nie dołączono pliku"]);
  });
  it("says what needs attention", () => {
    const needAttention = (kinds: string[]) => attention(kinds).replace(/\u00a0/g, " ");
    expect(needAttention(["photo"])).toBe("1 zdjęcie wymaga");
    expect(needAttention(["photo", "photo", "photo"])).toBe("3 zdjęcia wymagają");
    expect(needAttention(Array(5).fill("document"))).toBe("5 dokumentów wymaga");
    expect(needAttention(["photo", "document"])).toBe("2 pliki wymagają");
    expect(needAttention(["other"])).toBe("1 plik wymaga");
  });
});

describe("matchPeople", () => {
  const list = [{ name: "Żaneta Łukasiewicz" }, { name: "Jan Kowalski" }, { name: "Janina Kowalska-Nowak" }];
  it("ignores case and Polish diacritics", () => {
    expect(matchPeople(list, "zan luk").map((p) => p.name)).toEqual(["Żaneta Łukasiewicz"]);
    expect(matchPeople(list, "ŁUKA").map((p) => p.name)).toEqual(["Żaneta Łukasiewicz"]);
    expect(matchPeople(list, "jan").map((p) => p.name)).toEqual(["Jan Kowalski", "Janina Kowalska-Nowak"]);
  });
  it("needs every word, any order, and keeps all for an empty query", () => {
    expect(matchPeople(list, "kowalski jan").map((p) => p.name)).toEqual(["Jan Kowalski"]);
    expect(matchPeople(list, "  ")).toHaveLength(3);
    expect(matchPeople(list, "xyz")).toEqual([]);
  });
});
