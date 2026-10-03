import { describe, expect, it } from "vitest";
import { SIDE, sideColors } from "./colors";

// Grandparents gf+gm → father; mgf+mgm → mother; father+mother → me, sister; me+wife → son.
// The father also has a son from another marriage (half-brother); the wife's parents are wf+wm.
const parents: Record<string, [string | null, string | null]> = {
  father: ["gf", "gm"],
  mother: ["mgf", "mgm"],
  me: ["father", "mother"],
  sister: ["father", "mother"],
  half: ["father", null],
  son: ["me", "wife"],
  wife: ["wf", "wm"],
};
const childrenOf = (k: string) => Object.keys(parents).filter((c) => parents[c].includes(k));
const of = (k: string) => parents[k] ?? [null, null];

describe("„Koloruj wg: strona ojca–matki”", () => {
  it("colours both sides relative to the person in the centre", () => {
    const map = sideColors("me", of, childrenOf);
    expect(["father", "gf", "gm"].map((k) => map.get(k))).toEqual([SIDE.father, SIDE.father, SIDE.father]);
    expect(["mother", "mgf", "mgm"].map((k) => map.get(k))).toEqual([SIDE.mother, SIDE.mother, SIDE.mother]);
    expect(["me", "sister", "half"].map((k) => map.get(k))).toEqual([SIDE.centre, SIDE.centre, SIDE.centre]);
    // Partners, children and in-laws stay without a colour.
    expect(["wife", "son", "wf", "wm"].map((k) => map.has(k))).toEqual([false, false, false, false]);
  });

  it("moves with the centre", () => {
    const map = sideColors("son", of, childrenOf);
    expect(map.get("me")).toBe(SIDE.father);
    expect(map.get("gm")).toBe(SIDE.father);
    expect(map.get("wf")).toBe(SIDE.mother);
    expect(map.get("son")).toBe(SIDE.centre);
  });

  it("keeps the father's side for an ancestor on both sides, and works without parents", () => {
    const cousins: Record<string, [string | null, string | null]> = { ...parents, mother: ["gf", "x"] };
    const map = sideColors("me", (k: string) => cousins[k] ?? [null, null], childrenOf);
    expect(map.get("gf")).toBe(SIDE.father);
    expect(map.get("x")).toBe(SIDE.mother);
    expect([...sideColors("gf", of, childrenOf)]).toEqual([["gf", SIDE.centre]]);
  });
});
