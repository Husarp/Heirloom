// A tree view's camera kept apart from the screen size: what Back and „Ostatnie miejsce” store (store.ts
// `TreeCamera`) is the zoom and the world point in the middle of the screen, measured from `centre` (the centre
// person's card in the focus views, the world's origin in Całe drzewo). Kept apart from the canvases so it can be
// tested.

import type { TreeCamera } from "../../app/store";

/** A canvas camera: the world's offset on screen and the zoom. */
export interface Camera2D {
  x: number;
  y: number;
  zoom: number;
}

type Size = { w: number; h: number };
type Point = { x: number; y: number };

/** What a camera shows, for storing. */
export function viewOf(camera: Camera2D, size: Size, centre: Point): TreeCamera {
  return { zoom: camera.zoom, x: (size.w / 2 - camera.x) / camera.zoom - centre.x, y: (size.h / 2 - camera.y) / camera.zoom - centre.y };
}

/** The camera that shows a stored view again, on a screen of `size`, its zoom kept within `minZoom`–`maxZoom`. */
export function cameraOf(view: TreeCamera, size: Size, centre: Point, minZoom: number, maxZoom: number): Camera2D {
  const zoom = Math.min(maxZoom, Math.max(minZoom, view.zoom));
  return { zoom, x: size.w / 2 - (centre.x + view.x) * zoom, y: size.h / 2 - (centre.y + view.y) * zoom };
}

type Box = { x: number; y: number; r: number; b: number };

/** Potomkowie, a branch just unfolded: the camera that shows `box` (the person and their row of children, in the
 *  world) within `view` (on screen) by the least move from `camera`. Zoomed out only as far as the row needs and not
 *  below 75 % (full cards); a row still wider than the screen is shown with the person (`anchor`, the top middle of
 *  their card) in the middle, the children spreading to both sides below them. */
export function unfoldView(camera: Camera2D, box: Box, anchor: Point, view: { left: number; top: number; right: number; bottom: number }): Camera2D {
  const fit = (view.right - view.left) / (box.r - box.x);
  const zoom = fit >= camera.zoom ? camera.zoom : Math.max(fit, Math.min(camera.zoom, 0.75));
  // Zooming keeps the person where they were on screen.
  let x = camera.x + anchor.x * (camera.zoom - zoom);
  let y = camera.y + anchor.y * (camera.zoom - zoom);
  const least = (a: number, b: number, lo: number, hi: number) => (b - a > hi - lo ? null : a < lo ? lo - a : b > hi ? hi - b : 0);
  x += least(x + box.x * zoom, x + box.r * zoom, view.left, view.right) ?? (view.left + view.right) / 2 - (x + anchor.x * zoom);
  y += least(y + box.y * zoom, y + box.b * zoom, view.top, view.bottom) ?? view.top - (y + box.y * zoom);
  return zoom === camera.zoom && x === camera.x && y === camera.y ? camera : { zoom, x, y };
}
