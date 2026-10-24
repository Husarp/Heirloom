import { describe, expect, it } from "vitest";
import {
  angleText,
  clampCrop,
  fitBox,
  isEditableImage,
  isWhole,
  MIN_SIDE,
  moveCrop,
  previewImage,
  resizeCrop,
  roundAngle,
  squareCrop,
  straightenedSize,
  straightenScale,
  turnedSize,
  WHOLE,
  type Crop,
} from "./photoMath";

const near = (a: number, b: number) => expect(a).toBeCloseTo(b, 9);
/** The frame's sides in picture-height units: equal for a square. */
const sides = (c: Crop, aspect: number) => [c.w * aspect, c.h];

describe("straightening", () => {
  it("keeps the largest rectangle without empty corners (same values as the Rust tests)", () => {
    near(straightenScale(400, 300, 10), 0.8221394548303579);
    expect(straightenedSize(400, 300, 10)).toEqual({ w: 329, h: 247 });
    expect(straightenedSize(400, 300, -10)).toEqual({ w: 329, h: 247 });
    expect(straightenedSize(400, 300, 15)).toEqual({ w: 305, h: 229 });
    expect(straightenedSize(400, 300, 0.005)).toEqual({ w: 400, h: 300 });
  });
  it("does not depend on which side is which", () => {
    near(straightenScale(300, 400, 7.3), straightenScale(400, 300, 7.3));
    near(straightenScale(500, 500, 0), 1);
  });
  it("turned sides swap after an odd number of quarter turns", () => {
    expect(turnedSize(400, 300, 0)).toEqual({ w: 400, h: 300 });
    expect(turnedSize(400, 300, 1)).toEqual({ w: 300, h: 400 });
    expect(turnedSize(400, 300, 2)).toEqual({ w: 400, h: 300 });
    expect(turnedSize(400, 300, 3)).toEqual({ w: 300, h: 400 });
  });
});

describe("the preview", () => {
  it("fits a box of the picture's aspect into the room", () => {
    expect(fitBox(2, 400, 300)).toEqual({ w: 400, h: 200 });
    expect(fitBox(0.5, 400, 300)).toEqual({ w: 150, h: 300 });
    expect(fitBox(0, 400, 300)).toEqual({ w: 0, h: 0 });
  });
  it("draws the picture 1/k larger than the box and turned about its centre", () => {
    const box = { w: 300, h: 225 };
    const p = previewImage({ w: 400, h: 300 }, 0, 10, box);
    const k = straightenScale(400, 300, 10);
    near(p.width, 300 / k);
    near(p.height, 225 / k);
    expect(p.transform).toBe("translate(-50%, -50%) rotate(10deg)");
    // Not turned: the box is the picture.
    expect(previewImage({ w: 400, h: 300 }, 0, 0, box)).toEqual({ width: 300, height: 225, transform: "translate(-50%, -50%) rotate(0deg)" });
  });
  it("after a quarter turn the picture's own sides are the box's, swapped", () => {
    // 400×300 turned once is 300×400: the box is portrait.
    const portrait = { w: 150, h: 200 };
    const p = previewImage({ w: 400, h: 300 }, 1, 0, portrait);
    expect([p.width, p.height]).toEqual([200, 150]);
    expect(p.transform).toBe("translate(-50%, -50%) rotate(90deg)");
    expect(previewImage({ w: 400, h: 300 }, 3, -2.5, portrait).transform).toBe("translate(-50%, -50%) rotate(267.5deg)");
  });
});

describe("the crop frame", () => {
  it("is whole when it covers the picture", () => {
    expect(isWhole(WHOLE)).toBe(true);
    expect(isWhole({ x: 0, y: 0, w: 0.9, h: 1 })).toBe(false);
  });
  it("stays inside the picture and at least the smallest side", () => {
    expect(clampCrop({ x: -0.2, y: 0.9, w: 0.5, h: 0.5 })).toEqual({ x: 0, y: 0.5, w: 0.5, h: 0.5 });
    const tiny = clampCrop({ x: 0.5, y: 0.5, w: 0, h: 0.01 });
    expect([tiny.w, tiny.h]).toEqual([MIN_SIDE, MIN_SIDE]);
    expect(clampCrop({ x: 0.2, y: 0.2, w: 3, h: 3 })).toEqual(WHOLE);
  });
  it("moves and stops at the edges", () => {
    const c = { x: 0.2, y: 0.3, w: 0.4, h: 0.4 };
    expect(moveCrop(c, 0.1, -0.1)).toEqual({ x: expect.closeTo(0.3), y: expect.closeTo(0.2), w: 0.4, h: 0.4 });
    expect(moveCrop(c, 5, 5)).toEqual({ x: 0.6, y: 0.6, w: 0.4, h: 0.4 });
    expect(moveCrop(c, -5, -5)).toEqual({ x: 0, y: 0, w: 0.4, h: 0.4 });
  });
  it("a free frame follows the dragged corner and keeps the opposite one", () => {
    const c = { x: 0.2, y: 0.2, w: 0.4, h: 0.4 };
    const se = resizeCrop(c, "se", 0.9, 0.7, false, 1.5);
    near(se.x, 0.2);
    near(se.y, 0.2);
    near(se.w, 0.7);
    near(se.h, 0.5);
    const nw = resizeCrop(c, "nw", 0.1, 0.05, false, 1.5);
    near(nw.x, 0.1);
    near(nw.y, 0.05);
    near(nw.x + nw.w, 0.6);
    near(nw.y + nw.h, 0.6);
    // Dragged out of the picture, past the opposite corner: stops at the edge / the smallest side.
    const out = resizeCrop(c, "se", 3, -2, false, 1.5);
    near(out.x + out.w, 1);
    near(out.h, MIN_SIDE);
  });
  it("a square frame has equal sides in pixels, whichever corner is dragged", () => {
    const aspect = 1.5;
    const c = { x: 0.2, y: 0.2, w: 0.2, h: 0.3 };
    for (const corner of ["nw", "ne", "sw", "se"] as const) {
      const r = resizeCrop(c, corner, 0.5, 0.5, true, aspect);
      near(r.w * aspect, r.h);
      expect(r.x).toBeGreaterThanOrEqual(-1e-9);
      expect(r.x + r.w).toBeLessThanOrEqual(1 + 1e-9);
      expect(r.y + r.h).toBeLessThanOrEqual(1 + 1e-9);
    }
    // The anchor stays and the dragged corner is where the larger reach says.
    const se = resizeCrop({ x: 0.1, y: 0.1, w: 0.3, h: 0.3 }, "se", 0.7, 0.5, true, aspect);
    near(se.x, 0.1);
    near(se.y, 0.1);
    expect(sides(se, aspect)[0]).toBeCloseTo(0.9, 9);
  });
  it("a square frame fits the room of the picture", () => {
    // The room is 0.5 wide (a full 1.0 in height units) but 0.5 high: the height decides.
    const r = resizeCrop({ x: 0.5, y: 0.5, w: 0.2, h: 0.3 }, "se", 5, 5, true, 2);
    near(r.y + r.h, 1);
    near(r.x + r.w, 0.75);
    near(r.w * 2, r.h);
  });
  it("squaring keeps the shorter side around the same centre", () => {
    const aspect = 2;
    const s = squareCrop({ x: 0.1, y: 0.1, w: 0.6, h: 0.4 }, aspect);
    near(s.w * aspect, s.h);
    near(s.h, 0.4);
    near(s.x + s.w / 2, 0.4);
    near(s.y + s.h / 2, 0.3);
    // Tall picture: the width is the shorter side.
    const tall = squareCrop({ x: 0, y: 0, w: 0.5, h: 1 }, 0.5);
    near(tall.w * 0.5, tall.h);
    expect(tall.h).toBeCloseTo(0.25, 9);
  });
});

describe("the angle", () => {
  it("is written to a tenth of a degree with a Polish comma and a real minus", () => {
    expect(angleText(1.5)).toBe("+1,5°");
    expect(angleText(-0.3)).toBe("−0,3°");
    expect(angleText(0)).toBe("0°");
    expect(angleText(0.04)).toBe("0°");
    expect(angleText(-0.04)).toBe("0°");
    expect(angleText(15)).toBe("+15°");
    expect(angleText(-12.25)).toBe("−12,3°");
  });
  it("is rounded to a tenth and never −0", () => {
    expect(roundAngle(1.2345)).toBe(1.2);
    expect(Object.is(roundAngle(-0.04), 0)).toBe(true);
  });
});

describe("which pictures can be edited", () => {
  it("only the formats the server reads", () => {
    for (const p of ["a.jpg", "b.JPEG", "c.png", "d.gif", "e.webp", "f.bmp", "g.tif", "h.TIFF"]) expect(isEditableImage(p)).toBe(true);
    for (const p of ["a.avif", "b.heic", "c.pdf", "d", null, undefined]) expect(isEditableImage(p)).toBe(false);
  });
});
