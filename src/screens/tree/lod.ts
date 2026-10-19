// Levels of detail of the focus views by zoom (spec §3.8): 1 full card ≥ 75 %, 2 compact (name and years) 30–75 %,
// 3 mini (coloured blocks with the initial) 15–30 %, 4 dots below.

export type Lod = 1 | 2 | 3 | 4;

/** Hysteresis: switch down at 75 / 30 / 15 %, back up a few points higher (80 / 33 / 18). */
export function nextLod(current: Lod, zoom: number): Lod {
  const down = [0.75, 0.3, 0.15];
  const up = [0.8, 0.33, 0.18];
  let lod = current;
  while (lod < 4 && zoom < down[lod - 1]) lod = (lod + 1) as Lod;
  while (lod > 1 && zoom >= up[lod - 2]) lod = (lod - 1) as Lod;
  return lod;
}

/** The compact card's text size as a share of its full screen size (13 px name, 11 px years): it keeps its screen size
 *  down to 50 % zoom, then shrinks with the card to about 9 px at 30 %, so name and years still fit two lines. */
export function compactTextScale(zoom: number): number {
  if (zoom >= 0.5) return 1;
  return (9 + (Math.max(zoom, 0.3) - 0.3) * 20) / 13;
}
