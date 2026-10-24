import { describe, expect, it } from "vitest";
import { cropStyle, defaultCrop, MIN_SIDE, moveCrop, resizeCrop, squareFrame, type Crop } from "./cropMath";

const near = (a: number, b: number) => expect(a).toBeCloseTo(b, 9);
/** The frame's width and height in image-height units: equal for a square. */
const sides = (c: Crop, aspect: number) => [c.w * aspect, c.h];

describe("cropMath", () => {
  it("the default frame is the largest square, a bit smaller, centred", () => {
    const wide = defaultCrop(2);
    near(wide.h, 0.9);
    near(wide.w, 0.45);
    near(wide.x, 0.275);
    near(wide.y, 0.05);
    const tall = defaultCrop(0.5);
    near(tall.w, 0.9);
    near(tall.h, 0.45);
    near(tall.x, 0.05);
    near(tall.y, 0.275);
    for (const c of [wide, tall]) expect(sides(c, c === wide ? 2 : 0.5)[0]).toBeCloseTo(sides(c, c === wide ? 2 : 0.5)[1], 9);
  });

  it("moves and stops at the edges", () => {
    const c = defaultCrop(2);
    const moved = moveCrop(c, 0.1, -0.02, 2);
    near(moved.x, c.x + 0.1);
    near(moved.y, c.y - 0.02);
    expect(moved.w).toBe(c.w);
    const corner = moveCrop(c, 5, 5, 2);
    near(corner.x + corner.w, 1);
    near(corner.y + corner.h, 1);
    const back = moveCrop(c, -5, -5, 2);
    expect([back.x, back.y]).toEqual([0, 0]);
  });

  it("resizes from a corner keeping the opposite one and the square", () => {
    const aspect = 1.5;
    const c = squareFrame({ x: 0.2, y: 0.2, w: 0.2, h: 0.3 }, aspect);
    const grown = resizeCrop(c, "se", 0.7, 0.6, aspect);
    near(grown.x, c.x);
    near(grown.y, c.y);
    near(sides(grown, aspect)[0], sides(grown, aspect)[1]);
    // The pointer's farther axis decides: 0.5 across · 1.5 = 0.75 high units, 0.4 down.
    near(grown.h, 0.75);
    const shrunk = resizeCrop(c, "nw", 0.35, 0.35, aspect);
    near(shrunk.x + shrunk.w, c.x + c.w);
    near(shrunk.y + shrunk.h, c.y + c.h);
    const flipped = resizeCrop(c, "ne", c.x + c.w + 0.1, c.y + c.h - 0.2, aspect);
    near(flipped.x, c.x);
    near(flipped.y + flipped.h, c.y + c.h);
  });

  it("does not get smaller than 10 % of the shorter side or bigger than the room", () => {
    const aspect = 2;
    const c = defaultCrop(aspect);
    const tiny = resizeCrop(c, "se", c.x, c.y, aspect);
    near(tiny.h, MIN_SIDE);
    const huge = resizeCrop(c, "se", 5, 5, aspect);
    expect(huge.y + huge.h).toBeLessThanOrEqual(1 + 1e-9);
    expect(huge.x + huge.w).toBeLessThanOrEqual(1 + 1e-9);
    near(huge.h, 1 - c.y);
    const portrait = resizeCrop(defaultCrop(0.5), "nw", -5, -5, 0.5);
    expect(portrait.x).toBeGreaterThanOrEqual(0);
    expect(portrait.y).toBeGreaterThanOrEqual(0);
  });

  it("squares an off-square stored crop with the shorter side, inside the image", () => {
    const c = squareFrame({ x: 0.9, y: 0.9, w: 0.5, h: 0.25 }, 2);
    near(c.h, 0.25);
    near(c.w, 0.125);
    near(c.x + c.w, 1);
    near(c.y + c.h, 1);
  });

  it("the style shows exactly the crop in the box", () => {
    // 400×200 image, crop {0.25, 0, 0.5, 1} = pixels 100,0 200×200 shown in 72 px.
    const s = cropStyle({ x: 0.25, y: 0, w: 0.5, h: 1 }, 2, 72);
    expect(s).toEqual({ width: 144, height: 72, left: -36, top: -0 });
    const half = cropStyle({ x: 0.5, y: 0.5, w: 0.25, h: 0.5 }, 2, 100);
    expect(half.height).toBe(200);
    expect(half.width).toBe(400);
    expect([half.left, half.top]).toEqual([-200, -100]);
  });
});
