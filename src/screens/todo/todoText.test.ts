import { describe, expect, it } from "vitest";
import { noteText, PAGE, personChips, sectionLabel, toastDone, visible } from "./todoText";

describe("personChips", () => {
  it("warns about a person outside the tree and names the import", () => {
    const chips = personChips({ linked: false, batch: "Rodzina Nowak" });
    expect(chips.map((c) => c.text)).toEqual(["bez powiązań — nie ma jej w drzewie", "z importu „Rodzina Nowak”"]);
    expect(chips[0].warn).toBe(true);
    expect(chips[0].title).toBe("Dodaj relację na profilu, w sekcji Rodzina.");
    expect(chips[1].warn).toBeUndefined();
  });
  it("says „dodana ręcznie” without a batch, and no warning when linked", () => {
    expect(personChips({ linked: true, batch: null })).toEqual([{ text: "dodana ręcznie" }]);
  });
});

describe("visible", () => {
  const list = Array.from({ length: 130 }, (_, i) => i);
  it("cuts at the limit and counts the rest", () => {
    const v = visible(list, PAGE);
    expect(v.items).toHaveLength(60);
    expect(v.more).toBe(70);
  });
  it("shows all once the limit passes the length", () => {
    expect(visible(list, 2 * PAGE + PAGE)).toEqual({ items: list, more: 0 });
    expect(visible([], PAGE)).toEqual({ items: [], more: 0 });
  });
});

describe("texts", () => {
  it("leaves out an empty count", () => {
    expect(sectionLabel("Osoby", 0)).toBe("Osoby");
    expect(sectionLabel("Osoby", 4)).toBe("Osoby (4)");
  });
  it("toast names the person", () => expect(toastDone("Jan Nowak")).toBe("Gotowe: Jan Nowak."));
});

describe("noteText", () => {
  it("names the import and warns when outside the family", () => {
    expect(noteText({ linked: false, batch: "Nowak" })).toBe(
      "Ta osoba pochodzi z importu „Nowak” — materiały tylko o niej wspominają. Nie jest jeszcze połączona z rodziną, więc nie widać jej w drzewie.",
    );
  });
  it("says it was marked by hand without a batch", () => {
    expect(noteText({ linked: true, batch: null })).toBe("Oznaczono ją do dokończenia.");
    expect(noteText({ linked: false, batch: null })).toBe("Oznaczono ją do dokończenia. Nie jest jeszcze połączona z rodziną, więc nie widać jej w drzewie.");
  });
});
