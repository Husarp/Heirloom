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
