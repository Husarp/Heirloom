// Layouts of the focus views (spec §3.9): Rodzina (the person with parents, partners, children and siblings),
// Przodkowie (a horizontal pedigree) and Potomkowie (a top-down descendant tree). World coordinates; cards are
// 204 × 72. Pure functions, so they are easy to test and cheap to recompute.

import { childrenOf, parentsOrdered, pediOf, type Graph, type GraphPerson } from "./graph";

export const CARD_W = 204;
export const CARD_H = 72;
const PARTNER_GAP = 48;
const SIBLING_GAP = 24;
// Rodzina with 9 or more siblings: their cards are narrower (no photo), so the row still fits.
export const NARROW_W = 150;
const NARROW_GAP = 16;
const FAMILY_PITCH = 230;
const PEDIGREE_COLUMN = 280;
const PEDIGREE_ROW = 88;
const DESCENDANT_PITCH = 180;

export type LinkKind = "partner" | "birth" | "adopted" | "uncertain" | "placeholder";

export interface SceneCard {
  id: string;
  x: number;
  y: number;
  sub: string | null;
  /** An unknown ancestor: a dashed box with this text instead of a person. */
  stub?: string;
  focus?: boolean;
  /** A narrower card (NARROW_W) than CARD_W. */
  w?: number;
  /** What this person is to the card they hang from (edit mode's tools under the card change it). */
  rel?: { of: string; kind: "parent" | "partner" | "child" | "sibling" };
}

export interface SceneLink {
  key: string;
  d: string;
  kind: LinkKind;
  /** The people a link joins; it is highlighted when all of them are on the highlighted line. */
  people: string[];
}

export interface SceneUnion {
  key: string;
  x: number;
  y: number;
  people: string[];
}

export interface ScenePill {
  key: string;
  x: number;
  y: number;
  label: string;
  /** The person to go to (refocus) when clicked. */
  target: string;
  direction: "down" | "up" | "right";
}

export interface SceneBox {
  key: string;
  x: number;
  y: number;
  w: number;
  h: number;
  label: string;
  action: "add-parents";
  of?: string;
}

export interface SceneLabel {
  key: string;
  x: number;
  y: number;
  text: string;
}

export interface Scene {
  cards: SceneCard[];
  links: SceneLink[];
  unions: SceneUnion[];
  pills: ScenePill[];
  boxes: SceneBox[];
  labels: SceneLabel[];
  bounds: { minX: number; minY: number; maxX: number; maxY: number };
}

function bounds(scene: Omit<Scene, "bounds">): Scene["bounds"] {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const grow = (x: number, y: number, w: number, h: number) => {
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x + w);
    maxY = Math.max(maxY, y + h);
  };
  for (const c of scene.cards) grow(c.x, c.y, c.w ?? CARD_W, CARD_H);
  for (const b of scene.boxes) grow(b.x, b.y, b.w, b.h);
  for (const l of scene.labels) grow(l.x, l.y - 16, 120, 20);
  if (minX === Infinity) return { minX: 0, minY: 0, maxX: CARD_W, maxY: CARD_H };
  return { minX, minY, maxX, maxY };
}

function linkKind(pedi: string | null, uncertain: boolean): LinkKind {
  if (uncertain) return "uncertain";
  if (pedi === "ADOPTED" || pedi === "FOSTER" || pedi === "OTHER") return "adopted";
  return "birth";
}

/** The sub-line of a card: nickname or maiden name (Rodzina, Przodkowie). */
export function subLine(p: GraphPerson | undefined): string | null {
  if (!p) return null;
  if (p.nickname) return `„${p.nickname}”`;
  if (p.maiden) return `z d. ${p.maiden}`;
  return null;
}

/** "∞ Marianna z d. Nowak" (Potomkowie). */
function spouseLine(graph: Graph, p: GraphPerson): string | null {
  const spouse = p.partners.map((id) => graph.people[id]).find(Boolean);
  if (!spouse) return null;
  return `∞ ${spouse.given || spouse.name}${spouse.maiden ? ` z d. ${spouse.maiden}` : spouse.surname && spouse.surname !== p.surname ? ` ${spouse.surname}` : ""}`;
}

// Ustawienia › Drzewo › Linie: „Zaokrąglone” draws each child's line as one elbow with rounded bends.
const BEND = 10;

/** Down, across and down again, with rounded bends: parents to one child. */
function elbow(x0: number, y0: number, barY: number, x1: number, y1: number): string {
  const w = Math.abs(x1 - x0);
  if (w < 1) return `M${x0} ${y0} V${y1}`;
  const r = Math.min(BEND, w / 2, Math.abs(barY - y0), Math.abs(y1 - barY));
  const dx = Math.sign(x1 - x0);
  return `M${x0} ${y0} V${barY - r} Q${x0} ${barY} ${x0 + dx * r} ${barY} H${x1 - dx * r} Q${x1} ${barY} ${x1} ${barY + r} V${y1}`;
}

/** Vertical, then horizontal, with a rounded bend (Przodkowie). */
function bend(x0: number, y0: number, y1: number, x1: number): string {
  const r = Math.min(BEND, Math.abs(y1 - y0), Math.abs(x1 - x0));
  if (r < 1) return `M${x0} ${y0} V${y1} H${x1}`;
  return `M${x0} ${y0} V${y1 - Math.sign(y1 - y0) * r} Q${x0} ${y1} ${x0 + Math.sign(x1 - x0) * r} ${y1} H${x1}`;
}

/** Parents above, a descent line to a sibling bar and down into each child (spec §3.9 routing). */
function descent(links: SceneLink[], key: string, fromX: number, fromY: number, children: { id: string; x: number; y: number; w?: number; kind: LinkKind }[], barOffset: number, parents: string[], rounded = false) {
  if (!children.length) return;
  const barY = children[0].y - barOffset;
  if (rounded) {
    for (const c of children) links.push({ key: `${key}-${c.id}`, d: elbow(fromX, fromY, barY, c.x + (c.w ?? CARD_W) / 2, c.y), kind: c.kind, people: [...parents, c.id] });
    return;
  }
  const xs = children.map((c) => c.x + (c.w ?? CARD_W) / 2);
  const left = Math.min(fromX, ...xs);
  const right = Math.max(fromX, ...xs);
  links.push({ key: `${key}-down`, d: `M${fromX} ${fromY} V${barY}`, kind: "birth", people: parents });
  if (right > left) links.push({ key: `${key}-bar`, d: `M${left} ${barY} H${right}`, kind: "birth", people: parents });
  for (const c of children) {
    links.push({ key: `${key}-${c.id}`, d: `M${c.x + (c.w ?? CARD_W) / 2} ${barY} V${c.y}`, kind: c.kind, people: [...parents, c.id] });
  }
}

export function layoutFamily(graph: Graph, focusId: string, options: { editing: boolean; rounded?: boolean }): Scene {
  const cards: SceneCard[] = [];
  const links: SceneLink[] = [];
  const unions: SceneUnion[] = [];
  const pills: ScenePill[] = [];
  const boxes: SceneBox[] = [];
  const labels: SceneLabel[] = [];
  const focus = graph.people[focusId];
  if (!focus) return { cards, links, unions, pills, boxes, labels, bounds: { minX: 0, minY: 0, maxX: 0, maxY: 0 } };
  const placed = new Map<string, SceneCard>();
  const place = (id: string, x: number, y: number, extra: Partial<SceneCard> = {}) => {
    const card = { id, x, y, sub: subLine(graph.people[id]), ...extra };
    cards.push(card);
    placed.set(id, card);
    return card;
  };

  // Row 0: the person in the middle, partners beside (first to the right, second to the left…).
  place(focusId, 0, 0, { focus: true });
  const partners = focus.partners.filter((p) => graph.people[p]);
  partners.forEach((p, i) => {
    const step = Math.floor(i / 2) + 1;
    const x = i % 2 === 0 ? step * (CARD_W + PARTNER_GAP) : -step * (CARD_W + PARTNER_GAP);
    place(p, x, 0, { rel: { of: focusId, kind: "partner" } });
  });
  for (const p of partners) {
    const a = placed.get(focusId)!;
    const b = placed.get(p)!;
    const [l, r] = a.x < b.x ? [a, b] : [b, a];
    const midX = (l.x + CARD_W + r.x) / 2;
    links.push({ key: `partner-${p}`, d: `M${l.x + CARD_W} ${CARD_H / 2} H${r.x}`, kind: "partner", people: [focusId, p] });
    unions.push({ key: `union-${p}`, x: midX, y: CARD_H / 2, people: [focusId, p] });
  }
  const rowLeft = Math.min(...cards.map((c) => c.x));
  const rowRight = Math.max(...cards.map((c) => c.x + CARD_W));

  // Row -1: the parents of the person and of each partner, as couples. A partner's parents sit straight above the
  // partner and the person's own parents move aside for them; where that doesn't fit (several partners), the person's
  // parents stay above the person and the partners' parents move outwards.
  const y = -FAMILY_PITCH;
  type Block = { left: number; right: number };
  const clash = (a: Block, list: Block[]) => list.some((b) => a.left < b.right + SIBLING_GAP && a.right > b.left - SIBLING_GAP);
  const couple = (child: string) => {
    const present = parentsOrdered(graph, child).filter(Boolean) as string[];
    return { present, width: present.length === 2 ? CARD_W * 2 + PARTNER_GAP : present.length ? CARD_W : 0 };
  };
  const addBox = (x: number): Block => ({ left: x + (CARD_W - 150) / 2, right: x + (CARD_W + 150) / 2 });
  const own = couple(focusId);
  const others = partners.map((p) => ({ p, ...couple(p), cx: placed.get(p)!.x + CARD_W / 2 }));
  // „Dodaj rodziców” stays right above its person.
  const fixed: Block[] = [];
  if (!own.width && options.editing) fixed.push(addBox(0));
  for (const o of others) if (!o.width && options.editing) fixed.push(addBox(placed.get(o.p)!.x));
  let ownLeft = CARD_W / 2 - own.width / 2;
  const otherLeft = new Map<string, number>();
  const straight: Block[] = [...fixed];
  let fits = true;
  for (const o of others.filter((o) => o.width)) {
    const block = { left: o.cx - o.width / 2, right: o.cx + o.width / 2 };
    if (clash(block, straight)) fits = false;
    straight.push(block);
    otherLeft.set(o.p, block.left);
  }
  if (fits && own.width) {
    const candidates = [ownLeft, ...straight.map((b) => b.left - SIBLING_GAP - own.width), ...straight.map((b) => b.right + SIBLING_GAP)]
      .filter((l) => !clash({ left: l, right: l + own.width }, straight))
      .sort((a, b) => Math.abs(a - ownLeft) - Math.abs(b - ownLeft));
    if (candidates.length && Math.abs(candidates[0] - ownLeft) <= CARD_W + PARTNER_GAP) ownLeft = candidates[0];
    else fits = false;
  }
  // A partner's „Dodaj rodziców” (edit mode) that has to move aside too, with its line bent to the partner.
  const boxLeft = new Map<string, number>();
  if (!fits) {
    const occupied: Block[] = !own.width && options.editing ? [addBox(0)] : [];
    if (own.width) occupied.push({ left: ownLeft, right: ownLeft + own.width });
    for (const o of others) {
      const width = o.width || (options.editing ? 150 : 0);
      if (!width) continue;
      let left = o.cx - width / 2;
      for (let guard = 0; guard < 40 && clash({ left, right: left + width }, occupied); guard++) left += o.cx > CARD_W / 2 ? CARD_W / 2 : -CARD_W / 2;
      occupied.push({ left, right: left + width });
      (o.width ? otherLeft : boxLeft).set(o.p, left);
    }
  }
  const placeCouple = (child: string, present: string[], left: number) => {
    if (present.length === 2) {
      const rel = { of: child, kind: "parent" as const };
      const pair = [place(present[0], left, y, { rel }), place(present[1], left + CARD_W + PARTNER_GAP, y, { rel })];
      const midX = left + CARD_W + PARTNER_GAP / 2;
      links.push({ key: `pp-${child}`, d: `M${left + CARD_W} ${y + CARD_H / 2} H${left + CARD_W + PARTNER_GAP}`, kind: "partner", people: present });
      unions.push({ key: `pu-${child}`, x: midX, y: y + CARD_H / 2, people: present });
      return { fromX: midX, fromY: y + CARD_H / 2, cards: pair };
    }
    return { fromX: left + CARD_W / 2, fromY: y + CARD_H, cards: [place(present[0], left, y, { rel: { of: child, kind: "parent" } })] };
  };
  const ownCouple = own.width ? placeCouple(focusId, own.present, ownLeft) : null;
  if (ownCouple) {
    for (const c of ownCouple.cards) {
      const p = graph.people[c.id];
      if (p && p.ancestors > 0) pills.push({ key: `up-${c.id}`, x: c.x + CARD_W - 60, y: c.y - 11, label: `+${p.ancestors}`, target: c.id, direction: "up" });
    }
  } else if (options.editing) {
    boxes.push({ key: `add-${focusId}`, x: (CARD_W - 150) / 2, y: -FAMILY_PITCH + (CARD_H - 52) / 2, w: 150, h: 52, label: "Dodaj rodziców", action: "add-parents", of: focusId });
    links.push({ key: `add-link-${focusId}`, d: `M${CARD_W / 2} ${-FAMILY_PITCH + (CARD_H + 52) / 2} V0`, kind: "placeholder", people: [] });
  }
  // The partners' parents come down to a lower bar than the siblings' (25 above the row, not 50), so the two never
  // share a stretch of line; a third partner's and further out lower still, so they don't share one with the first two.
  const landing: number[] = [];
  for (const [i, o] of others.entries()) {
    const card = placed.get(o.p)!;
    const drop = 25 - 10 * Math.min(2, Math.floor(i / 2));
    if (o.width) {
      const c = placeCouple(o.p, o.present, otherLeft.get(o.p)!);
      landing.push(c.fromX);
      descent(links, `pp-${o.p}`, c.fromX, c.fromY, [{ id: o.p, x: card.x, y: 0, kind: "birth" }], drop, o.present, options.rounded);
    } else if (options.editing) {
      const left = boxLeft.get(o.p) ?? card.x + (CARD_W - 150) / 2;
      const fromX = left + 75;
      const fromY = -FAMILY_PITCH + (CARD_H + 52) / 2;
      const toX = card.x + CARD_W / 2;
      landing.push(fromX);
      boxes.push({ key: `add-${o.p}`, x: left, y: -FAMILY_PITCH + (CARD_H - 52) / 2, w: 150, h: 52, label: "Dodaj rodziców", action: "add-parents", of: o.p });
      const d = fromX === toX ? `M${toX} ${fromY} V0` : options.rounded ? elbow(fromX, fromY, -drop, toX, 0) : `M${fromX} ${fromY} V${-drop} H${toX} V0`;
      links.push({ key: `add-link-${o.p}`, d, kind: "placeholder", people: [] });
    }
  }

  // Siblings, always shown, in birth order: the older ones left of the person, the younger ones right of the
  // partners, and half-siblings at the outer end on the side of the parent they share. From 9 on, narrower cards.
  const birth = (id: string) => graph.people[id]?.birth?.sort ?? Number.MAX_SAFE_INTEGER;
  const brood = [...new Set(graph.unions.filter((u) => u.children.some((c) => c.id === focusId)).flatMap((u) => u.children.map((c) => c.id)))]
    .filter((id) => graph.people[id] && (id === focusId || !placed.has(id)))
    .sort((a, b) => birth(a) - birth(b));
  const at = brood.indexOf(focusId);
  const older = brood.slice(0, at);
  const younger = brood.slice(at + 1);
  const [father, mother] = parentsOrdered(graph, focusId);
  const halfOf = (parent: string | null, skip: string[]) =>
    parent ? graph.people[parent].children.filter((id) => id !== focusId && graph.people[id] && !placed.has(id) && !brood.includes(id) && !skip.includes(id)).sort((a, b) => birth(a) - birth(b)) : [];
  const halfLeft = halfOf(father, []);
  const halfRight = halfOf(mother, halfLeft);
  const narrow = older.length + younger.length + halfLeft.length + halfRight.length >= 9;
  const w = narrow ? NARROW_W : CARD_W;
  const gap = narrow ? NARROW_GAP : SIBLING_GAP;
  // Clear of the partners' parents' lines coming down beside them.
  let left = Math.min(rowLeft, ...landing.filter((x) => x < CARD_W / 2).map((x) => x - SIBLING_GAP)) - SIBLING_GAP;
  let right = Math.max(rowRight, ...landing.filter((x) => x > CARD_W / 2).map((x) => x + SIBLING_GAP)) + SIBLING_GAP;
  const half = (p: GraphPerson) => (p.sex === "M" ? "brat przyrodni" : p.sex === "F" ? "siostra przyrodnia" : "rodzeństwo przyrodnie");
  const sibling = (id: string, x: number, isHalf: boolean) => {
    const p = graph.people[id];
    const sub = isHalf ? [half(p), subLine(p)].filter(Boolean).join(" · ") : subLine(p);
    return place(id, x, 0, { sub, rel: { of: focusId, kind: "sibling" }, ...(narrow ? { w } : {}) });
  };
  const leftSide = [...halfLeft, ...older].reverse().map((id) => {
    left -= w;
    const card = sibling(id, left, halfLeft.includes(id));
    left -= gap;
    return card;
  });
  const rightSide = [...younger, ...halfRight].map((id) => {
    const card = sibling(id, right, halfRight.includes(id));
    right += w + gap;
    return card;
  });
  const kid = (c: SceneCard) => ({ id: c.id, x: c.x, y: c.y, w: c.w, kind: linkKind(pediOf(graph, c.id), false) });
  if (ownCouple) {
    const full = [...leftSide, placed.get(focusId)!, ...rightSide].filter((c) => !halfLeft.includes(c.id) && !halfRight.includes(c.id)).sort((a, b) => a.x - b.x);
    descent(links, "focus-parents", ownCouple.fromX, ownCouple.fromY, full.map(kid), 50, own.present, options.rounded);
    // Half-siblings: from the shared parent's card, to a bar above the full siblings' one.
    for (const [parent, ids] of [[father, halfLeft], [mother, halfRight]] as const) {
      const from = parent ? placed.get(parent) : undefined;
      if (!from || !ids.length) continue;
      descent(links, `half-${parent}`, from.x + CARD_W / 2, from.y + CARD_H, ids.map((id) => kid(placed.get(id)!)).sort((a, b) => a.x - b.x), 75, [parent!], options.rounded);
    }
  }

  // Row +1: the children of each union, centred under it, pushed apart where they would overlap.
  const groups: { key: string; fromX: number; fromY: number; parents: string[]; kids: string[] }[] = [];
  // A child listed in two of the person's unions (archives opened together, before both mothers are linked) is
  // drawn once, under the first.
  const claimed = new Set<string>();
  for (const u of graph.unions.filter((u) => u.partners.includes(focusId))) {
    const kids = childrenOf(graph, u).filter((id) => !placed.has(id) && !claimed.has(id));
    for (const id of kids) claimed.add(id);
    if (!kids.length) continue;
    const other = u.partners.find((x) => x !== focusId && placed.has(x));
    const marker = other ? unions.find((m) => m.people.includes(focusId) && m.people.includes(other)) : undefined;
    groups.push({ key: u.id, fromX: marker?.x ?? CARD_W / 2, fromY: marker ? marker.y : CARD_H, parents: other ? [focusId, other] : [focusId], kids });
  }
  groups.sort((a, b) => a.fromX - b.fromX);
  let cursor = -Infinity;
  groups.forEach((g, k) => {
    const width = g.kids.length * CARD_W + (g.kids.length - 1) * SIBLING_GAP;
    // One union: its children centred under it. Several (remarriage): the first union's children end under its
    // marker and the others' start under theirs, so two families never share a stretch of line (spec §3.9).
    let left = groups.length === 1 ? g.fromX - width / 2 : k === 0 ? g.fromX + CARD_W / 2 - width : g.fromX - CARD_W / 2;
    if (left < cursor) left = cursor;
    const kids = g.kids.map((id, i) => {
      const card = place(id, left + i * (CARD_W + SIBLING_GAP), FAMILY_PITCH, { rel: { of: focusId, kind: "child" } });
      const p = graph.people[id];
      if (p && p.descendants > 0) pills.push({ key: `down-${id}`, x: card.x + CARD_W - 64, y: card.y + CARD_H - 11, label: `+${p.descendants}`, target: id, direction: "down" });
      return { id, x: card.x, y: card.y, kind: linkKind(pediOf(graph, id), false) };
    });
    descent(links, `kids-${g.key}`, g.fromX, g.fromY, kids, 50, g.parents, options.rounded);
    cursor = left + width + SIBLING_GAP * 2;
  });

  const scene = { cards, links, unions, pills, boxes, labels };
  return { ...scene, bounds: bounds(scene) };
}

/** Przodkowie: the person on the left, parents, grandparents and great-grandparents to the right (spec §4.2). */
export function layoutAncestors(graph: Graph, focusId: string, generations = 3, rounded = false): Scene {
  const cards: SceneCard[] = [];
  const links: SceneLink[] = [];
  const pills: ScenePill[] = [];
  const labels: SceneLabel[] = [];
  const slots: (string | null)[][] = [[focusId]];
  for (let g = 0; g < generations; g++) {
    const next: (string | null)[] = [];
    for (const id of slots[g]) {
      if (!id) next.push(null, null);
      else next.push(...parentsOrdered(graph, id));
    }
    slots.push(next);
  }
  const G = generations;
  const y = (g: number, i: number): number => {
    if (g === G) return (i - (2 ** G - 1) / 2) * PEDIGREE_ROW;
    return (y(g + 1, 2 * i) + y(g + 1, 2 * i + 1)) / 2;
  };
  const x = (g: number) => g * PEDIGREE_COLUMN;
  const titles = ["Osoba", "Rodzice", "Dziadkowie", "Pradziadkowie", "Prapradziadkowie"];
  const top = y(G, 0);
  for (let g = 0; g <= G; g++) labels.push({ key: `gen-${g}`, x: x(g), y: top - 24, text: titles[g] ?? `Pokolenie ${g}` });
  for (let g = 0; g <= G; g++) {
    slots[g].forEach((id, i) => {
      const cy = y(g, i);
      if (id) {
        const child = g > 0 ? slots[g - 1][Math.floor(i / 2)] : null;
        cards.push({ id, x: x(g), y: cy, sub: subLine(graph.people[id]), focus: g === 0, ...(child ? { rel: { of: child, kind: "parent" as const } } : {}) });
        const p = graph.people[id];
        if (g === G && p && p.ancestors > 0) pills.push({ key: `more-${id}`, x: x(g) + CARD_W + 12, y: cy + CARD_H / 2 - 12, label: `+${p.ancestors}`, target: id, direction: "right" });
      } else if (g > 0 && slots[g - 1][Math.floor(i / 2)]) {
        // An unknown parent of a known person.
        cards.push({ id: `stub-${g}-${i}`, x: x(g), y: cy, sub: null, stub: i % 2 === 0 ? "ojciec nieznany" : "matka nieznana" });
      }
    });
  }
  // Connectors: from the child's right edge, right 38, a vertical between the parents, right 38 into each.
  for (let g = 0; g < G; g++) {
    slots[g].forEach((id, i) => {
      if (!id) return;
      const childY = y(g, i) + CARD_H / 2;
      const fy = y(g + 1, 2 * i) + CARD_H / 2;
      const my = y(g + 1, 2 * i + 1) + CARD_H / 2;
      const x0 = x(g) + CARD_W;
      const xm = x0 + (PEDIGREE_COLUMN - CARD_W) / 2;
      const x1 = x(g + 1);
      const [f, m] = [slots[g + 1][2 * i], slots[g + 1][2 * i + 1]];
      links.push({ key: `a-${id}-trunk`, d: `M${x0} ${childY} H${xm}`, kind: "birth", people: [id] });
      links.push({ key: `a-${id}-f`, d: rounded ? bend(xm, childY, fy, x1) : `M${xm} ${childY} V${fy} H${x1}`, kind: f ? "birth" : "placeholder", people: f ? [id, f] : [] });
      links.push({ key: `a-${id}-m`, d: rounded ? bend(xm, childY, my, x1) : `M${xm} ${childY} V${my} H${x1}`, kind: m ? "birth" : "placeholder", people: m ? [id, m] : [] });
    });
  }
  const scene = { cards, links, unions: [], pills, boxes: [], labels };
  return { ...scene, bounds: bounds(scene) };
}

/** Potomkowie: a tidy top-down tree; spouses are in the card's sub-line (spec §4.3). */
export function layoutDescendants(graph: Graph, focusId: string, depth = 2, rounded = false): Scene {
  const cards: SceneCard[] = [];
  const links: SceneLink[] = [];
  const pills: ScenePill[] = [];
  const labels: SceneLabel[] = [];
  const kidsOf = (id: string): string[] => {
    const p = graph.people[id];
    if (!p) return [];
    const unions = graph.unions.filter((u) => u.partners.includes(id));
    const list = unions.flatMap((u) => childrenOf(graph, u));
    return [...new Set(list)];
  };
  const widths = new Map<string, number>();
  const width = (id: string, level: number): number => {
    const kids = level < depth ? kidsOf(id) : [];
    const w = kids.length ? Math.max(CARD_W, kids.reduce((sum, k) => sum + width(k, level + 1), 0) + SIBLING_GAP * (kids.length - 1)) : CARD_W;
    widths.set(`${level}:${id}`, w);
    return w;
  };
  width(focusId, 0);
  const place = (id: string, level: number, left: number, parent?: string) => {
    const w = widths.get(`${level}:${id}`) ?? CARD_W;
    const cx = left + (w - CARD_W) / 2;
    const cy = level * DESCENDANT_PITCH;
    const p = graph.people[id];
    cards.push({ id, x: cx, y: cy, sub: p ? spouseLine(graph, p) : null, focus: level === 0, ...(parent ? { rel: { of: parent, kind: "child" as const } } : {}) });
    const kids = level < depth ? kidsOf(id) : [];
    if (level === depth && p && p.descendants > 0) {
      const n = p.descendants;
      pills.push({ key: `more-${id}`, x: cx + 70, y: cy + CARD_H + 12, label: `+${n} ${n === 1 ? "potomek" : "potomków"}`, target: id, direction: "down" });
    }
    let cursor = left;
    const placedKids: { id: string; x: number; y: number; kind: LinkKind }[] = [];
    for (const k of kids) {
      const kw = widths.get(`${level + 1}:${k}`) ?? CARD_W;
      place(k, level + 1, cursor, id);
      placedKids.push({ id: k, x: cursor + (kw - CARD_W) / 2, y: (level + 1) * DESCENDANT_PITCH, kind: linkKind(pediOf(graph, k), false) });
      cursor += kw + SIBLING_GAP;
    }
    if (placedKids.length) descent(links, `d-${id}`, cx + CARD_W / 2, cy + CARD_H, placedKids, 26, [id], rounded);
  };
  place(focusId, 0, 0);
  const titles = ["Osoba", "Dzieci", "Wnuki", "Prawnuki", "Praprawnuki"];
  const minX = Math.min(...cards.map((c) => c.x));
  for (let level = 0; level <= depth; level++) {
    if (cards.some((c) => c.y === level * DESCENDANT_PITCH)) labels.push({ key: `row-${level}`, x: minX, y: level * DESCENDANT_PITCH - 20, text: titles[level] ?? `Pokolenie +${level}` });
  }
  const scene = { cards, links, unions: [], pills, boxes: [], labels };
  return { ...scene, bounds: bounds(scene) };
}

/** Everyone on the line of `id`: its ancestors and descendants within the graph (for highlights and dimming). */
export function directLine(graph: Graph, id: string): Set<string> {
  const out = new Set<string>([id]);
  const up = [id];
  while (up.length) {
    const p = graph.people[up.pop()!];
    for (const parent of p?.parents ?? []) if (graph.people[parent] && !out.has(parent)) (out.add(parent), up.push(parent));
  }
  const down = [id];
  while (down.length) {
    const p = graph.people[down.pop()!];
    for (const child of p?.children ?? []) if (graph.people[child] && !out.has(child)) (out.add(child), down.push(child));
  }
  return out;
}

/** The path from `from` up or down to `to` (for the hover highlight in Przodkowie / Potomkowie). */
export function pathBetween(graph: Graph, from: string, to: string): Set<string> {
  const previous = new Map<string, string | null>([[from, null]]);
  const queue = [from];
  while (queue.length) {
    const id = queue.shift()!;
    if (id === to) break;
    const p = graph.people[id];
    for (const next of [...(p?.parents ?? []), ...(p?.children ?? [])]) {
      if (graph.people[next] && !previous.has(next)) {
        previous.set(next, id);
        queue.push(next);
      }
    }
  }
  const out = new Set<string>();
  let at: string | null | undefined = previous.has(to) ? to : undefined;
  while (at) {
    out.add(at);
    at = previous.get(at) ?? null;
  }
  return out;
}

