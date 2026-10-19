// The data `tree.graph` returns (crates/heirloom-api/src/tree.rs) and small helpers over it.

import type { PersonSummary } from "../../api/types";

export type GraphPerson = PersonSummary & {
  parents: string[];
  children: string[];
  partners: string[];
  descendants: number;
  /** Of them, the ones within two generations (children and grandchildren): Rodzina asks before unfolding more. */
  descendantsNear: number;
  ancestors: number;
  hasParents: boolean;
};

export interface GraphUnion {
  id: string;
  partners: string[];
  children: { id: string; pedi: string | null }[];
  married: boolean;
  year: string | null;
  uncertain: boolean;
  allChildren: number;
}

export interface Graph {
  focus: string;
  /** The depth it was fetched with, so a graph meant for another view isn't laid out. */
  up: number;
  down: number;
  /** The people unfolded below (Potomkowie's „+N potomków”) it was fetched with. */
  expand?: string[];
  /** The children (and people below them) unfolded in Rodzina it was fetched with: how many generations below each
   *  (null: all). */
  unfold?: { id: string; depth: number | null }[];
  people: Record<string, GraphPerson>;
  unions: GraphUnion[];
}

/** The union (family) two people share as partners. */
export function unionOf(graph: Graph, a: string, b: string): GraphUnion | undefined {
  return graph.unions.find((u) => u.partners.includes(a) && u.partners.includes(b));
}

/** The union in which `child` is a child. */
export function parentUnion(graph: Graph, child: string): GraphUnion | undefined {
  return graph.unions.find((u) => u.children.some((c) => c.id === child));
}

/** Children of a union, in birth order. */
export function childrenOf(graph: Graph, union: GraphUnion): string[] {
  return union.children
    .map((c) => c.id)
    .filter((id) => graph.people[id])
    .sort((a, b) => (graph.people[a].birth?.sort ?? Number.MAX_SAFE_INTEGER) - (graph.people[b].birth?.sort ?? Number.MAX_SAFE_INTEGER));
}

export function pediOf(graph: Graph, child: string): string | null {
  for (const u of graph.unions) {
    const c = u.children.find((x) => x.id === child);
    if (c) return c.pedi;
  }
  return null;
}

/** Father first, then mother (the pedigree's order). */
export function parentsOrdered(graph: Graph, id: string): (string | null)[] {
  const p = graph.people[id];
  if (!p) return [null, null];
  const parents = p.parents.filter((x) => graph.people[x]);
  const father = parents.find((x) => graph.people[x].sex === "M") ?? null;
  const mother = parents.find((x) => graph.people[x].sex === "F") ?? null;
  const rest = parents.filter((x) => x !== father && x !== mother);
  return [father ?? rest.shift() ?? null, mother ?? rest.shift() ?? null];
}

/** Ustawienia › Drzewo › „Rozwijanie potomków (widok Rodzina)”: what the „+N” under a child does when the line goes
 *  on for more than two generations: ask first, unfold all of them, or the next two generations. */
export type FamilyUnfold = "ask" | "all" | "two";

/** How many generations „Kolejne 2 pokolenia” unfolds. */
export const UNFOLD_NEAR = 2;

/** Rodzina, the „+N” of `p` clicked: `depth` (generations below them, null for all) to unfold at once, or `ask` for
 *  the question „wszystkich czy 2 pokolenia”, which is needed only when the line goes beyond two generations. */
export function unfoldAnswer(p: GraphPerson, choice: FamilyUnfold): { ask: true } | { ask: false; depth: number | null } {
  if (p.descendants <= p.descendantsNear || choice === "all") return { ask: false, depth: null };
  return choice === "two" ? { ask: false, depth: UNFOLD_NEAR } : { ask: true };
}

/** The unfolded branches without `id` and without anyone below them (folding a branch folds what was unfolded in it). */
export function foldBranch(unfolded: ReadonlyMap<string, number | null>, graph: Graph, id: string): Map<string, number | null> {
  const below = new Set<string>([id]);
  const stack = [id];
  while (stack.length) for (const c of graph.people[stack.pop()!]?.children ?? []) if (!below.has(c)) (below.add(c), stack.push(c));
  return new Map([...unfolded].filter(([k]) => !below.has(k)));
}
