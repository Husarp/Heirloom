// The data `tree.graph` returns (crates/heirloom-api/src/tree.rs) and small helpers over it.

import type { PersonSummary } from "../../api/types";

export type GraphPerson = PersonSummary & {
  parents: string[];
  children: string[];
  partners: string[];
  descendants: number;
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
