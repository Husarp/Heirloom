// The place list as a tree: built from places.list (paths largest part first) and flattened into the rows each view
// shows. Kept apart from the screen so it can be tested.

import { collator, fold } from "../surnames/split";

export interface PlaceRow {
  path: string[];
  name: string;
  level: number;
  count: number;
  leaf: boolean;
}

export interface Node extends PlaceRow {
  key: string;
  children: Node[];
}

/** One row of the list: the place, its indentation, and a note (the place above it, in the A–Z view). */
export interface Line {
  node: Node;
  depth: number;
  open: boolean;
  note?: string;
}

export interface Tree {
  roots: Node[];
  all: Node[];
}

export type View = "admin" | "parish" | "az";

/** A place as the archive writes it, smallest part first. It is also the route's `place`, so other screens can link
 *  to a place with a fact's place as written. */
export function written(path: string[]): string {
  return [...path].reverse().join(", ");
}

/** The route's `place` (as written, or a JSON array) as a path, largest part first like the API. */
export function parsePlace(text: string | undefined): string[] | null {
  if (!text) return null;
  if (text.startsWith("[")) {
    try {
      const path: unknown = JSON.parse(text);
      if (Array.isArray(path) && path.length) return path.map(String);
    } catch {
      // Not JSON: a place as written.
    }
  }
  const path = text
    .split(",")
    .map((p) => p.trim())
    .filter(Boolean)
    .reverse();
  return path.length ? path : null;
}

const byName = (a: Node, b: Node) => collator.compare(a.name, b.name) || collator.compare(a.key, b.key);

/** There is no separate parish hierarchy in the data yet, so parishes are recognised by name. */
const isParish = (node: Node) => /^(parafia|par\.)\s/i.test(node.name);

export function buildTree(rows: PlaceRow[]): Tree {
  const all: Node[] = rows.map((r) => ({ ...r, key: written(r.path), children: [] }));
  const byKey = new Map(all.map((n) => [n.key, n]));
  const roots: Node[] = [];
  for (const node of all) {
    const parent = node.path.length > 1 ? byKey.get(written(node.path.slice(0, -1))) : undefined;
    (parent ? parent.children : roots).push(node);
  }
  // The API sorts by bytes (capitals first, Polish letters last); Polish order reads better.
  for (const node of all) node.children.sort(byName);
  roots.sort(byName);
  return { roots, all };
}

/** The rows of a view. While searching: the matching places and the places above them, all open. */
export function linesFor(tree: Tree, view: View, expanded: Set<string>, q: string): Line[] {
  const matches = (n: Node) => fold(n.name).includes(q);
  if (view === "az") {
    return tree.all
      .filter((n) => !q || matches(n))
      .sort(byName)
      .map((node) => ({ node, depth: 0, open: false, note: node.path.length > 1 ? node.path[node.path.length - 2] : undefined }));
  }
  let visible: Set<string> | null = null;
  if (q) {
    visible = new Set();
    for (const n of tree.all) {
      if (matches(n)) for (let i = 1; i <= n.path.length; i++) visible.add(written(n.path.slice(0, i)));
    }
  }
  const out: Line[] = [];
  const walk = (list: Node[], depth: number) => {
    for (const node of list) {
      if (visible && !visible.has(node.key)) continue;
      const open = !node.leaf && (visible != null || expanded.has(node.key));
      out.push({ node, depth, open });
      if (open) walk(node.children, depth + 1);
    }
  };
  walk(view === "parish" ? tree.all.filter(isParish).sort(byName) : tree.roots, 0);
  return out;
}
