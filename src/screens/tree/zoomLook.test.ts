import { describe, expect, it } from "vitest";
import { CSS_INK, DEFAULT_ZOOM_LOOK, fillCss, markFor, markUnit, paintCss, ZOOM_LOOKS, zoomLookOf, type LookInk } from "./zoomLook";

const LIGHT: LookInk = { canvas: "#f2eee7", surface: "#ffffff", line: "#857c6f", muted: "#554e45", accent: "#2f5d50", neutral: "#857c6f" };
const DARK: LookInk = { canvas: "#1f1e1a", surface: "#24221e", line: "#7a7266", muted: "#bdb5a8", accent: "#7fb8a4", neutral: "#7a7266" };

describe("zoomLookOf", () => {
  it("defaults to the outline when missing or unknown", () => {
    expect(DEFAULT_ZOOM_LOOK).toBe("outline");
    expect(zoomLookOf(undefined)).toBe("outline");
    expect(zoomLookOf(null)).toBe("outline");
    expect(zoomLookOf("sparkles")).toBe("outline");
  });
  it("keeps the four known looks", () => {
    for (const look of ZOOM_LOOKS) expect(zoomLookOf(look)).toBe(look);
  });
});

describe("markFor", () => {
  it("gives every look a fill or a stroke in the branch colour, in light and dark", () => {
    for (const ink of [LIGHT, DARK]) {
      for (const look of ZOOM_LOOKS) {
        const m = markFor(look, "#b5562f", 1, 16, ink);
        const paints = [m.fill?.color, m.stroke?.color, m.stripe?.color];
        expect(paints).toContain("#b5562f");
        expect(m.fill || m.stroke).toBeTruthy();
        expect(m.ring.color).toBe(ink.accent);
      }
    }
  });
  it("draws shapes filled and the outline empty with a border", () => {
    const filled = markFor("shapes", "#b5562f", 0, 16, LIGHT);
    expect(filled.fill).toEqual({ color: "#b5562f", alpha: 1 });
    expect(filled.stroke).toBeNull();
    const outline = markFor("outline", "#b5562f", 0, 16, LIGHT);
    expect(outline.stroke?.color).toBe("#b5562f");
    expect(outline.fill?.color).toBe(LIGHT.canvas);
    expect(outline.strokeWidth).toBeGreaterThan(2);
  });
  it("tells branches apart by shape in shapes and outline, not in tiles and cards", () => {
    expect(markFor("shapes", "x", 3, 16, LIGHT).shape).toBe("circle");
    expect(markFor("outline", "x", 4, 16, LIGHT).shape).toBe("square");
    expect(markFor("outline", "x", 5, 16, LIGHT).shape).toBe("diamond");
    expect(markFor("outline", null, null, 16, LIGHT).shape).toBe("circle");
    expect(markFor("tiles", "x", 5, 16, LIGHT).shape).toBe("square");
    expect(markFor("cards", "x", 5, 16, LIGHT).shape).toBe("card");
  });
  it("keeps a soft tile over an opaque base, so a line under it never shows", () => {
    const tile = markFor("tiles", "var(--b3)", 3, 16, CSS_INK);
    expect(tile.fill?.alpha).toBeLessThan(1);
    expect(tile.base).toBe("var(--canvas)");
    expect(fillCss(tile)).toBe("color-mix(in srgb, var(--b3) 22%, var(--canvas))");
  });
  it("uses the neutral colour without a branch colour", () => {
    for (const look of ZOOM_LOOKS) {
      const m = markFor(look, null, null, 16, DARK);
      const used = [m.fill?.color, m.stroke?.color, m.stripe?.color];
      expect(used).toContain(DARK.neutral);
    }
  });
  it("makes the card a wider rectangle with a stripe on top", () => {
    const c = markFor("cards", "#8c3b3b", 4, 16, LIGHT);
    expect(c.w).toBeGreaterThan(c.h);
    expect(c.stripe).toEqual({ height: expect.any(Number), color: "#8c3b3b" });
    expect(c.fill?.color).toBe(LIGHT.surface);
  });
  it("reaches a diamond further than a square so lines still touch it", () => {
    const d = markFor("shapes", "x", 5, 16, LIGHT);
    expect((d.w * Math.SQRT2) / 2).toBeGreaterThan(8);
  });
});

describe("markUnit and paintCss", () => {
  it("grows with the zoom between 7 and 16 px", () => {
    expect(markUnit(0.06)).toBe(7);
    expect(markUnit(0.3)).toBe(16);
    expect(markUnit(0.18)).toBeGreaterThan(markUnit(0.15));
    expect(markUnit(5)).toBe(16);
  });
  it("mixes a see-through paint with transparent", () => {
    expect(paintCss({ color: "#fff", alpha: 1 })).toBe("#fff");
    expect(paintCss({ color: "#fff", alpha: 0.55 })).toBe("color-mix(in srgb, #fff 55%, transparent)");
  });
});
