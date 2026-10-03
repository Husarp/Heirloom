// The surname labels over „Całe drzewo” (spec §3.11): where each one goes on screen. Kept apart from the canvas so it
// can be tested. A label already on screen keeps its spot while it fits; a newcomer moves aside or waits, so labels
// don't come and go while zooming.

/** One family's region on screen (px) and its label's measured width. */
export interface LabelRegion {
  key: string;
  count: number;
  width: number;
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** The part of the screen the canvas shows: right of the generation column, under the toolbars (a label's top may
 *  reach `top`; the family itself counts from one label height lower). */
export interface LabelArea {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** x is the label's centre, y its bottom edge (`.cluster-label` is moved by -50 %, -100 %). */
export interface PlacedLabel {
  key: string;
  x: number;
  y: number;
}

const HEIGHT = 28;
/** A second row of labels sits this much lower. */
export const ROW = 32;
/** How much of a family must be on screen for its label (across, up–down), and the room a label keeps from the
 *  others. A label already shown needs less of both, so it doesn't blink at the edge of fitting while zooming. */
const MIN_ACROSS = { shown: 12, newcomer: 24 };
const MIN_DOWN = { shown: 10, newcomer: 20 };
const GAP = { shown: 4, newcomer: 16 };

/** The spots a label may take, best first: centred over the visible part of its family at the top of its first
 *  generation, then left- and right-aligned there; below the card level (where it would cover cards) the same three
 *  one row lower, and further rows while still over the family. */
function candidates(r: LabelRegion, area: LabelArea, cardLevel: boolean): { x: number; y: number }[] {
  const from = Math.max(r.left, area.left);
  const to = Math.min(r.right, area.right);
  const half = r.width / 2;
  const clampX = (x: number) => Math.min(Math.max(x, area.left + half + 4), area.right - half - 4);
  const y = Math.max(area.top + HEIGHT, r.top);
  const xs = [clampX((from + to) / 2), clampX(from + half + 4), clampX(to - half - 4)];
  const list = xs.map((x) => ({ x, y }));
  if (!cardLevel) {
    const bottom = Math.min(r.bottom, area.bottom);
    for (let row = y + ROW; row <= bottom; row += ROW) list.push(...xs.map((x) => ({ x, y: row })));
  }
  return list;
}

/** Places the labels of the families on screen. `previous` holds, for each label shown last time, the spot it took
 *  (an index into its candidates); the result's `spots` is the same for this frame. */
export function placeLabels(regions: LabelRegion[], area: LabelArea, previous: Map<string, number>, cardLevel: boolean) {
  const visible = regions.filter((r) => {
    const kind = previous.has(r.key) ? "shown" : "newcomer";
    return Math.min(r.right, area.right) - Math.max(r.left, area.left) >= MIN_ACROSS[kind] && Math.min(r.bottom, area.bottom) - Math.max(r.top, area.top + HEIGHT) >= MIN_DOWN[kind];
  });
  // Labels already shown first (they keep their place), then bigger families.
  visible.sort((a, b) => Number(previous.has(b.key)) - Number(previous.has(a.key)) || b.count - a.count || a.key.localeCompare(b.key));
  const boxes: { left: number; right: number; top: number; bottom: number }[] = [];
  const placed: PlacedLabel[] = [];
  const spots = new Map<string, number>();
  for (const r of visible) {
    const options = candidates(r, area, cardLevel);
    const before = previous.get(r.key);
    const gap = GAP[before == null ? "newcomer" : "shown"];
    const order = options.map((_, k) => k);
    if (before != null && before < options.length) order.unshift(...order.splice(before, 1));
    for (const k of order) {
      const { x, y } = options[k];
      const box = { left: x - r.width / 2, right: x + r.width / 2, top: y - HEIGHT, bottom: y };
      if (boxes.some((b) => box.left - gap < b.right && box.right + gap > b.left && box.top < b.bottom && box.bottom > b.top)) continue;
      boxes.push(box);
      placed.push({ key: r.key, x, y });
      spots.set(r.key, k);
      break;
    }
  }
  return { placed, spots };
}
