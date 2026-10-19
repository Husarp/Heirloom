// Dragging the photos of a profile gallery into an order of your own. The pure part is tested; the drag itself uses
// pointer events (not HTML5 drag and drop: the window takes over dragging for dropped files).

import { useEffect, useRef, useState, type PointerEvent } from "react";

/** `ids` with `id` put just before `target` (or just after it). Unknown ids leave the list as it was. */
export function moveNextTo(ids: readonly string[], id: string, target: string, after: boolean): string[] {
  if (id === target || !ids.includes(id) || !ids.includes(target)) return [...ids];
  const rest = ids.filter((x) => x !== id);
  rest.splice(rest.indexOf(target) + (after ? 1 : 0), 0, id);
  return rest;
}

/** `id` one place earlier (-1) or later (+1); at the ends nothing changes. */
export function shiftBy(ids: readonly string[], id: string, delta: -1 | 1): string[] {
  const at = ids.indexOf(id);
  const to = at + delta;
  if (at < 0 || to < 0 || to >= ids.length) return [...ids];
  const next = [...ids];
  [next[at], next[to]] = [next[to], next[at]];
  return next;
}

export const sameOrder = (a: readonly string[], b: readonly string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

export interface TileDrag {
  /** The photo being dragged and where the pointer is. */
  id: string;
  x: number;
  y: number;
  /** The photo it would go next to, and on which side. */
  target: string | null;
  after: boolean;
}

/** Tiles carry `data-gallery-id`. Pointer down on a tile (not on a button inside it) and a move of a few pixels starts
 *  a drag; letting go over another tile calls `onDrop`; Esc cancels. `consumeClick()` is true once right after a drag,
 *  so the click that ends it does not open the photo. */
export function useTileDrag(enabled: boolean, onDrop: (id: string, target: string, after: boolean) => void) {
  const [drag, setDrag] = useState<TileDrag | null>(null);
  const clicked = useRef(false);
  const drop = useRef(onDrop);
  drop.current = onDrop;
  const cancel = useRef<(() => void) | null>(null);
  useEffect(() => () => cancel.current?.(), []);

  const onPointerDown = (e: PointerEvent<HTMLElement>, id: string) => {
    if (!enabled || e.button !== 0 || (e.target as HTMLElement).closest("button.profile-star, .profile-star-pop, .gallery-actions")) return;
    const [x0, y0] = [e.clientX, e.clientY];
    let moved = false;
    let over: { target: string | null; after: boolean } = { target: null, after: false };
    const move = (ev: globalThis.PointerEvent) => {
      if (!moved && Math.hypot(ev.clientX - x0, ev.clientY - y0) < 6) return;
      moved = true;
      const tile = document.elementFromPoint(ev.clientX, ev.clientY)?.closest<HTMLElement>("[data-gallery-id]");
      const target = tile?.dataset.galleryId;
      if (tile && target && target !== id) {
        const box = tile.getBoundingClientRect();
        over = { target, after: ev.clientX > box.left + box.width / 2 };
      } else {
        over = { target: null, after: false };
      }
      setDrag({ id, x: ev.clientX, y: ev.clientY, ...over });
    };
    const finish = (dropIt: boolean) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", abort);
      window.removeEventListener("keydown", key, true);
      cancel.current = null;
      if (moved) {
        clicked.current = true;
        setTimeout(() => (clicked.current = false), 0);
        if (dropIt && over.target) drop.current(id, over.target, over.after);
      }
      setDrag(null);
    };
    const up = () => finish(true);
    const abort = () => finish(false);
    const key = (ev: KeyboardEvent) => {
      if (ev.key !== "Escape" || !moved) return;
      // Esc ends only the drag, not the open section behind it.
      ev.preventDefault();
      ev.stopPropagation();
      finish(false);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", abort);
    window.addEventListener("keydown", key, true);
    cancel.current = abort;
  };

  return { drag, onPointerDown, consumeClick: () => clicked.current };
}
