import { describe, expect, it } from "vitest";
import { CSS_INK, DEFAULT_ZOOM_LOOK, fillCss, MARK_REF_UNIT, markFor, markScale, markUnit, markVariant, markVariants, paintCss, ZOOM_LOOKS, zoomLookOf, type LookInk, type ZoomLook } from "./zoomLook";

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
    // Its tips lie 1.5 px beyond the circle of the other shapes (mockup: r + 1.5).
    expect((d.w * Math.SQRT2) / 2).toBeCloseTo(16 / 2 + 1.5);
  });
});

describe("the accent ring", () => {
  it("runs to the middle of its stroke at the mockup's size, so the border sits half a stroke further out", () => {
    // Mockup at u = 16: circle r + 5, tile and card 4 px around the box, to the middle of the 2.4 px stroke.
    const ring = (look: ZoomLook) => markFor(look, "x", 0, 16, LIGHT).ring;
    expect(ring("shapes").w).toBeCloseTo(16 + 2 * 5 + 2.4);
    expect(ring("outline").w).toBeCloseTo(16 + 2 * 5 + 2.4);
    expect(ring("tiles").w).toBeCloseTo(16 + 2 * 4 + 2.4);
    expect(ring("tiles").radius).toBeCloseTo(8 + 1.2);
    expect(ring("shapes").w / 2).toBeGreaterThan(markFor("shapes", "x", 2, 16, LIGHT).w * Math.SQRT2 / 2);
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

describe("markScale and markVariant (canvas textures)", () => {
  it("scales a texture drawn at the reference size to the mark's screen size, undone by the zoom", () => {
    for (const zoom of [0.01, 0.1, 0.2, 0.39]) expect(markScale(zoom) * zoom * MARK_REF_UNIT).toBeCloseTo(markUnit(zoom));
  });
  it("tells branches apart by shape only in Kształty and Obwódka", () => {
    expect([0, 1, 2, 3, 4].map((b) => markVariant("shapes", b))).toEqual([0, 1, 2, 0, 1]);
    expect(markVariants("outline")).toBe(3);
    for (const look of ["tiles", "cards"] as const) {
      expect(markVariants(look)).toBe(1);
      expect(markVariant(look, 7)).toBe(0);
    }
  });
  it("picks the variant whose texture has the shape the branch gets in the focus views", () => {
    for (const b of [1, 2, 3, 8, 12]) expect(["circle", "square", "diamond"][markVariant("outline", b)]).toBe(markFor("outline", null, b, 16, LIGHT).shape);
  });
});
