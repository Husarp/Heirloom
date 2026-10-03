// „Koloruj wg: strona ojca–matki” (spec §3.2), shared by the focus views (people by id) and Całe drzewo (people by
// index). Kept apart from the screen so it can be tested.

/** The father's side (`--b2`), the mother's side (`--b9`) and the person in the centre with their siblings (`--b12`). */
export const SIDE = { father: 2, mother: 9, centre: 12 } as const;

/** Colours around `centre`: the father and his ancestors, the mother and hers, then the centre person and everyone
 *  who shares a parent with them. Nobody else gets a colour (partners, children, the rest of the family). An
 *  ancestor on both sides (cousins who married) keeps the father's side. `parents` gives [father, mother], either
 *  may be missing. */
export function sideColors<K>(centre: K, parents: (k: K) => readonly (K | null | undefined)[], children: (k: K) => Iterable<K>): Map<K, number> {
  const map = new Map<K, number>();
  const [father, mother] = parents(centre);
  const mark = (start: K | null | undefined, color: number) => {
    const stack = start != null ? [start] : [];
    while (stack.length) {
      const k = stack.pop()!;
      if (map.has(k)) continue;
      map.set(k, color);
      for (const p of parents(k)) if (p != null) stack.push(p);
    }
  };
  mark(father, SIDE.father);
  mark(mother, SIDE.mother);
  map.set(centre, SIDE.centre);
  for (const p of [father, mother]) {
    if (p == null) continue;
    for (const c of children(p)) if (!map.has(c)) map.set(c, SIDE.centre);
  }
  return map;
}
