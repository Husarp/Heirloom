import { describe, expect, it } from "vitest";
import { computerOnlyNames, nameChoices } from "./editorNames";

describe("nameChoices", () => {
  it("lists archive editors, then computer names, without duplicates or the typed name", () => {
    expect(nameChoices(["Ewa", "Adam"], ["adam", "Jan", "Ola"], "ola", 6)).toEqual(["Ewa", "Adam", "Jan"]);
  });
  it("stops at the limit", () => {
    expect(nameChoices(["A", "B", "C"], ["D"], "", 2)).toEqual(["A", "B"]);
  });
});

describe("computerOnlyNames", () => {
  it("drops names the archive already has", () => {
    expect(computerOnlyNames(["Ewa"], ["ewa", "Jan", "Jan"])).toEqual(["Jan"]);
  });
});
