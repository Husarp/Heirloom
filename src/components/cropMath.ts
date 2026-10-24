// Square-frame maths for cropping a photo. A crop is {x, y, w, h}: fractions 0–1 of the (upright) image. The frame is
// always square in pixels, so for an image of width/height `aspect` its w and h differ: h = w · aspect.

export interface Crop {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Corner = "nw" | "ne" | "sw" | "se";

/** The smallest frame, as a share of the image's shorter side. */
export const MIN_SIDE = 0.1;

/** The shorter side's length in image-height units (the image is `aspect` wide and 1 high). */
const shorter = (aspect: number) => Math.min(aspect, 1);

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** A square frame of side `s` (image-height units) with its top-left corner at (x, y), pulled inside the image. */
function frame(x: number, y: number, s: number, aspect: number): Crop {
  const w = s / aspect;
  return { x: clamp(x, 0, 1 - w), y: clamp(y, 0, 1 - s), w, h: s };
}

/** The frame's side in image-height units (a stored crop may be a little off square: the shorter side counts, as on the server). */
const sideOf = (c: Crop, aspect: number) => Math.min(c.w * aspect, c.h);

/** Any crop as a square frame inside the image, keeping its top-left corner (moved back inside when needed). */
export function squareFrame(c: Crop, aspect: number): Crop {
  const s = clamp(sideOf(c, aspect), MIN_SIDE * shorter(aspect), shorter(aspect));
  return frame(c.x, c.y, s, aspect);
}

/** The frame first shown: the largest square, a bit smaller, in the middle. */
export function defaultCrop(aspect: number): Crop {
  const s = shorter(aspect) * 0.9;
  return frame((1 - s / aspect) / 2, (1 - s) / 2, s, aspect);
}

/** The frame moved by (dx, dy) fractions of the image; it stops at the edges. */
export function moveCrop(c: Crop, dx: number, dy: number, aspect: number): Crop {
  return frame(c.x + dx, c.y + dy, sideOf(c, aspect), aspect);
}

/** The frame resized by dragging one corner to the pointer (px, py) (fractions of the image); the opposite corner
 *  stays, the frame stays square, at least 10 % of the shorter side and inside the image. */
export function resizeCrop(c: Crop, corner: Corner, px: number, py: number, aspect: number): Crop {
  const east = corner === "ne" || corner === "se";
  const south = corner === "sw" || corner === "se";
  const s0 = sideOf(c, aspect);
  const ax = east ? c.x : c.x + s0 / aspect;
  const ay = south ? c.y : c.y + s0;
  const wanted = Math.max(Math.abs(px - ax) * aspect, Math.abs(py - ay));
  const room = Math.min((east ? 1 - ax : ax) * aspect, south ? 1 - ay : ay);
  const s = clamp(wanted, MIN_SIDE * shorter(aspect), Math.max(room, MIN_SIDE * shorter(aspect)));
  return frame(east ? ax : ax - s / aspect, south ? ay : ay - s, s, aspect);
}

/** The CSS of an `<img>` inside a `size`×`size` box (overflow hidden, position relative) that shows exactly the crop. */
export function cropStyle(c: Crop, aspect: number, size: number): { width: number; height: number; left: number; top: number } {
  const height = size / sideOf(c, aspect);
  const width = height * aspect;
  return { width, height, left: -c.x * width, top: -c.y * height };
}
