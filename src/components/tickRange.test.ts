import { describe, expect, it } from "vitest";
import { clickRow } from "./tickRange";

const ids = ["a", "b", "c", "d", "e"];

describe("clickRow", () => {
  it("a click toggles one row", () => {
    expect(clickRow(ids, ["a"], 2, 0, false)).toEqual(["a", "c"]);
    expect(clickRow(ids, ["a", "c"], 2, 2, false)).toEqual(["a"]);
  });

  it("Shift+click ticks the range from the last clicked row, in either direction", () => {
    expect(clickRow(ids, ["b"], 3, 1, true)).toEqual(["b", "c", "d"]);
    expect(clickRow(ids, ["d"], 1, 3, true)).toEqual(["b", "c", "d"]);
    expect(clickRow(ids, ["a", "e"], 3, 1, true)).toEqual(["a", "b", "c", "d", "e"]);
  });

  it("Shift+click only ticks, it never unticks", () => {
    expect(clickRow(ids, ["a", "b", "c"], 2, 0, true)).toEqual(["a", "b", "c"]);
  });

  it("Shift+click with no row clicked before toggles the row", () => {
    expect(clickRow(ids, [], 2, null, true)).toEqual(["c"]);
    expect(clickRow(ids, ["c"], 2, null, true)).toEqual([]);
  });

  it("keeps the order of the rows", () => {
    expect(clickRow(ids, ["d", "a"], 1, 3, false)).toEqual(["a", "b", "d"]);
  });
});
