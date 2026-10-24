// The maths behind the photo editor (PhotoEditor.tsx). The server makes the picture (crates/heirloom-api/src/photo_edit.rs):
// quarter turns, then the straightening, then the crop; the preview here uses the same formulas.

/** A part of the picture as fractions 0–1 of its width and height (after the turns and the straightening). */
export interface Crop {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Corner = "nw" | "ne" | "sw" | "se";

/** The straighten slider's range, in degrees. */
export const MAX_ANGLE = 15;

/** The smallest frame side, as a share of the picture. */
export const MIN_SIDE = 0.05;

export const WHOLE: Crop = { x: 0, y: 0, w: 1, h: 1 };

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** Formats the server can read and write (the `image` crate: no AVIF, no HEIC). */
export const isEditableImage = (path: string | null | undefined) => !!path && /\.(jpe?g|png|gif|webp|bmp|tiff?)$/i.test(path);

/** The scale of the largest centred rectangle of the picture's own aspect that has no empty corners once the picture
 *  is turned by `degrees` (the same `k` as the server). */
export function straightenScale(width: number, height: number, degrees: number): number {
  const rad = (degrees * Math.PI) / 180;
  const c = Math.abs(Math.cos(rad));
  const s = Math.abs(Math.sin(rad));
  return Math.min(width / (width * c + height * s), height / (width * s + height * c));
}

/** The size in pixels the server gives a straightened picture. */
export function straightenedSize(width: number, height: number, degrees: number): { w: number; h: number } {
  if (Math.abs(degrees) < 0.01) return { w: width, h: height };
  const k = straightenScale(width, height, degrees);
  return { w: Math.max(1, Math.round(k * width)), h: Math.max(1, Math.round(k * height)) };
}

/** The picture's size after `quarter` turns of 90° (an odd number swaps the sides). */
export function turnedSize(width: number, height: number, quarter: number): { w: number; h: number } {
  return quarter % 2 === 0 ? { w: width, h: height } : { w: height, h: width };
}

/** The largest box of the given aspect (width / height) that fits in `maxW` × `maxH`. */
export function fitBox(aspect: number, maxW: number, maxH: number): { w: number; h: number } {
  if (!(aspect > 0) || maxW <= 0 || maxH <= 0) return { w: 0, h: 0 };
  const h = Math.min(maxH, maxW / aspect);
  return { w: h * aspect, h };
}

/** The CSS of the `<img>` inside the preview box (centred, `overflow: hidden` on the box): the picture of
 *  `natural` pixels turned by the quarter turns and the angle, and `1 / k` larger than the box so that it shows exactly the
 *  rectangle the server keeps. `box` is the size of the box, whose aspect is the one after the quarter turns. */
export function previewImage(
  natural: { w: number; h: number },
  quarter: number,
  angle: number,
  box: { w: number; h: number },
): { width: number; height: number; transform: string } {
  const k = Math.abs(angle) < 0.01 ? 1 : straightenScale(natural.w, natural.h, angle);
  const odd = quarter % 2 !== 0;
  // Before the turn the picture's sides are the box's, swapped after an odd number of turns.
  const width = (odd ? box.h : box.w) / k;
  const height = (odd ? box.w : box.h) / k;
  return { width, height, transform: `translate(-50%, -50%) rotate(${quarter * 90 + angle}deg)` };
}

export const isWhole = (c: Crop) => c.x <= 1e-6 && c.y <= 1e-6 && c.w >= 1 - 1e-6 && c.h >= 1 - 1e-6;

/** A frame inside the picture, at least [MIN_SIDE] on each side. */
export function clampCrop(c: Crop): Crop {
  const w = clamp(c.w, MIN_SIDE, 1);
  const h = clamp(c.h, MIN_SIDE, 1);
  return { x: clamp(c.x, 0, 1 - w), y: clamp(c.y, 0, 1 - h), w, h };
}

/** The frame moved by (dx, dy) fractions; it stops at the edges. */
export function moveCrop(c: Crop, dx: number, dy: number): Crop {
  return { ...c, x: clamp(c.x + dx, 0, 1 - c.w), y: clamp(c.y + dy, 0, 1 - c.h) };
}

/** The frame squared (equal sides in pixels): the shorter side stays, around the same centre. `aspect` is the
 *  picture's width / height. */
export function squareCrop(c: Crop, aspect: number): Crop {
  const side = Math.min(c.w * aspect, c.h);
  const w = side / aspect;
  return clampCrop({ x: c.x + (c.w - w) / 2, y: c.y + (c.h - side) / 2, w, h: side });
}

/** The frame resized by dragging one corner to the pointer (`px`, `py`: fractions of the picture); the opposite
 *  corner stays. A square frame keeps equal sides in pixels. Never smaller than [MIN_SIDE], never outside. */
export function resizeCrop(c: Crop, corner: Corner, px: number, py: number, square: boolean, aspect: number): Crop {
  const east = corner === "ne" || corner === "se";
  const south = corner === "sw" || corner === "se";
  const ax = east ? c.x : c.x + c.w;
  const ay = south ? c.y : c.y + c.h;
  const roomW = east ? 1 - ax : ax;
  const roomH = south ? 1 - ay : ay;
  const reachW = east ? px - ax : ax - px;
  const reachH = south ? py - ay : ay - py;
  let w: number;
  let h: number;
  if (square) {
    // The side in picture-height units.
    const min = MIN_SIDE * Math.min(aspect, 1);
    const side = clamp(Math.max(reachW * aspect, reachH), min, Math.max(Math.min(roomW * aspect, roomH), min));
    w = side / aspect;
    h = side;
  } else {
    w = clamp(reachW, MIN_SIDE, Math.max(roomW, MIN_SIDE));
    h = clamp(reachH, MIN_SIDE, Math.max(roomH, MIN_SIDE));
  }
  return { x: east ? ax : ax - w, y: south ? ay : ay - h, w, h };
}

/** „+1,5°”, „−0,3°”, „0°” (the angle is shown to a tenth of a degree). */
export function angleText(degrees: number): string {
  const tenths = Math.round(Math.abs(degrees) * 10);
  if (tenths === 0) return "0°";
  return `${degrees > 0 ? "+" : "−"}${(tenths / 10).toString().replace(".", ",")}°`;
}

/** The angle as the slider gives it, to a tenth of a degree (no −0). */
export const roundAngle = (degrees: number) => (Math.sign(degrees) * Math.round(Math.abs(degrees) * 10)) / 10 || 0;
