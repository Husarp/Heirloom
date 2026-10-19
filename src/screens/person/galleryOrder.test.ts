import { describe, expect, it } from "vitest";
import { moveNextTo, sameOrder, shiftBy } from "./galleryOrder";

const ids = ["a", "b", "c", "d"];

describe("moveNextTo", () => {
  it("puts a photo before or after another one", () => {
    expect(moveNextTo(ids, "d", "b", false)).toEqual(["a", "d", "b", "c"]);
    expect(moveNextTo(ids, "a", "c", true)).toEqual(["b", "c", "a", "d"]);
    expect(moveNextTo(ids, "a", "d", true)).toEqual(["b", "c", "d", "a"]);
    expect(moveNextTo(ids, "c", "a", false)).toEqual(["c", "a", "b", "d"]);
  });

  it("leaves the order alone for the same or unknown photos", () => {
    expect(moveNextTo(ids, "b", "b", true)).toEqual(ids);
    expect(moveNextTo(ids, "x", "b", true)).toEqual(ids);
    expect(moveNextTo(ids, "b", "x", true)).toEqual(ids);
  });

  it("a move to where the photo already is gives the same order", () => {
    expect(sameOrder(moveNextTo(ids, "b", "c", false), ids)).toBe(true);
    expect(sameOrder(moveNextTo(ids, "b", "a", true), ids)).toBe(true);
  });
});

describe("shiftBy", () => {
  it("moves one place and stops at the ends", () => {
    expect(shiftBy(ids, "b", -1)).toEqual(["b", "a", "c", "d"]);
    expect(shiftBy(ids, "b", 1)).toEqual(["a", "c", "b", "d"]);
    expect(shiftBy(ids, "a", -1)).toEqual(ids);
    expect(shiftBy(ids, "d", 1)).toEqual(ids);
    expect(shiftBy(ids, "x", 1)).toEqual(ids);
  });
});
