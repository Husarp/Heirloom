import { describe, expect, it } from "vitest";
import { placeLabels, ROW, type LabelRegion } from "./labels";

const area = { left: 96, top: 80, right: 1200, bottom: 800 };
const region = (key: string, count: number, left: number, right: number, top = 200, bottom = 700): LabelRegion => ({ key, count, width: 140, left, right, top, bottom });

describe("Całe drzewo: surname labels", () => {
  it("hides a label whose family is above or below the screen, or off to the side", () => {
    const { placed } = placeLabels(
      [region("above", 50, 200, 500, -900, 60), region("below", 50, 600, 900, 820, 1500), region("left", 50, -600, 90), region("here", 10, 300, 700)],
      area,
      new Map(),
      false,
    );
    expect(placed.map((p) => p.key)).toEqual(["here"]);
  });

  it("pins the label under the toolbars when the family starts above the screen", () => {
    const { placed } = placeLabels([region("tall", 50, 300, 700, -400, 700)], area, new Map(), false);
    expect(placed[0].y).toBe(108);
  });

  it("moves a smaller neighbour aside instead of dropping it", () => {
    // Two families side by side, too narrow for both labels in the middle.
    const big = region("Ostrowscy", 90, 300, 480);
    const small = region("Jabłońscy", 40, 480, 560);
    const { placed } = placeLabels([big, small], area, new Map(), true);
    expect(placed.map((p) => p.key).sort()).toEqual(["Jabłońscy", "Ostrowscy"]);
    // Not over its own centre (that would cover Ostrowscy) but left-aligned over its family, on the same row.
    const moved = placed.find((p) => p.key === "Jabłońscy")!;
    expect([moved.x, moved.y]).toEqual([480 + 70 + 4, 200]);
  });

  it("keeps a label already shown in its place; a bigger newcomer moves or waits", () => {
    const small = region("Jabłońscy", 40, 300, 450);
    const first = placeLabels([small], area, new Map(), false);
    const big = region("Ostrowscy", 90, 300, 460);
    const next = placeLabels([big, small], area, first.spots, false);
    const keep = next.placed.find((p) => p.key === "Jabłońscy")!;
    expect(keep).toEqual(first.placed[0]);
    // The newcomer took a spot that doesn't cover it: here one row lower.
    const moved = next.placed.find((p) => p.key === "Ostrowscy")!;
    expect(moved.y).toBe(keep.y + ROW);
  });

  it("uses no lower rows at the card level, where they would cover cards", () => {
    const a = region("A", 90, 300, 460);
    const b = region("B", 40, 300, 460);
    const { placed } = placeLabels([a, b], area, new Map(), true);
    expect(placed.map((p) => p.key)).toEqual(["A"]);
  });
});
