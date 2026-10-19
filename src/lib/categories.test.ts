import { describe, expect, it } from "vitest";
import { choiceOf, choiceValue, cleanName, parseSaved, validateName, withAdded, withCurrent, withoutCategory, withRenamed } from "./categories";

describe("category names", () => {
  it("cleans spaces", () => {
    expect(cleanName("  Listy   rodzinne ")).toBe("Listy rodzinne");
  });

  it("refuses an empty name, a long one and the built-in kinds, whatever the letters", () => {
    expect(validateName("   ", [])).toBe("Wpisz nazwę kategorii.");
    expect(validateName("x".repeat(61), [])).toMatch(/najwyżej 60/);
    for (const name of ["Zdjęcie", "dokument", " INNE ", "Zdjęcia"]) expect(validateName(name, [])).toMatch(/wbudowanego/);
    expect(validateName("Pocztówki", [])).toBeNull();
  });

  it("refuses a name that exists, ignoring case and Polish letters' case", () => {
    expect(validateName("pocztówki", ["Pocztówki", "Listy"])).toBe("Taka kategoria już jest.");
    expect(validateName("ŁĄKI", ["łąki"])).toBe("Taka kategoria już jest.");
    expect(validateName("Mapy", ["Pocztówki"])).toBeNull();
  });

  it("lets a category keep its own name or change only its letters when renamed", () => {
    expect(validateName("POCZTÓWKI", ["Pocztówki", "Listy"], "Pocztówki")).toBeNull();
    expect(validateName("listy", ["Pocztówki", "Listy"], "Pocztówki")).toBe("Taka kategoria już jest.");
  });
});

describe("the saved list", () => {
  it("is read carefully", () => {
    const raw = [{ name: " Mapy ", base: "document" }, { name: "Mapy", base: "photo" }, { name: "", base: "photo" }, { name: "Zły", base: "video" }, null, { name: "Kartki", base: "photo" }];
    expect(parseSaved(raw)).toEqual([{ name: "Mapy", base: "document" }, { name: "Kartki", base: "photo" }]);
    expect(parseSaved(undefined)).toEqual([]);
    expect(parseSaved({ name: "Mapy" })).toEqual([]);
  });

  it("adds, renames and removes without touching the others", () => {
    const saved = [{ name: "Mapy", base: "document" as const }, { name: "Kartki", base: "photo" as const }];
    expect(withAdded(saved, "Listy", "document").map((c) => c.name)).toEqual(["Mapy", "Kartki", "Listy"]);
    expect(withAdded(saved, "Mapy", "photo")).toEqual([{ name: "Kartki", base: "photo" }, { name: "Mapy", base: "photo" }]);
    expect(withRenamed(saved, "Kartki", "Pocztówki")).toEqual([{ name: "Mapy", base: "document" }, { name: "Pocztówki", base: "photo" }]);
    expect(withRenamed(saved, "Nikt", "X")).toEqual(saved);
    expect(withoutCategory(saved, "Mapy")).toEqual([{ name: "Kartki", base: "photo" }]);
  });
});

describe("the kind choices of a file", () => {
  const categories = [{ name: "Pocztówki", base: "photo" as const }, { name: "Mapy", base: "document" as const }];

  it("turns a file's kind and category into one menu value and back", () => {
    expect(choiceValue({ kind: "photo", category: null })).toBe("photo");
    expect(choiceValue({ kind: "photo", category: "Pocztówki" })).toBe("cat:Pocztówki");
    expect(choiceOf("cat:Mapy", categories)).toEqual({ kind: "document", category: "Mapy" });
    expect(choiceOf("other", categories)).toEqual({ kind: "other", category: null });
    expect(choiceOf("cat:Nie ma", categories)).toBeNull();
    expect(choiceOf("__add", categories)).toBeNull();
  });

  it("keeps a category that was just added in the choices before the list has it", () => {
    expect(withCurrent(categories, { kind: "other", category: "Pamiątki" }).map((c) => [c.name, c.base])).toEqual([["Pocztówki", "photo"], ["Mapy", "document"], ["Pamiątki", "other"]]);
    expect(withCurrent(categories, { kind: "photo", category: "Pocztówki" })).toHaveLength(2);
    expect(withCurrent(categories, { kind: "photo", category: null })).toHaveLength(2);
  });
});
