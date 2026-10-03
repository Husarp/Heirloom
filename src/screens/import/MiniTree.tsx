// „Gdzie trafią do drzewa” (spec §4.12, §4.14): one person from the batch with their parents, partners and children,
// marked as new (from the import), merged (joined with someone in the archive) or already in the archive.

import type { Graph } from "../tree/graph";
import type { ImportPerson, ImportState } from "./types";
import { cardYears } from "../../lib/format";

export interface MiniNode {
  key: string;
  name: string;
  years: string;
  state: "new" | "merged" | "existing";
  branch?: number | null;
}

export interface MiniFamily {
  parents: MiniNode[];
  couple: MiniNode[];
  children: MiniNode[];
}

/** The archive person a batch person will be joined with, when that's the decision (or the strong suggestion). */
export function mergeTarget(p: ImportPerson): string | null {
  if (p.decision === "merge") return p.target;
  if (p.decision === "undecided" && p.status === "match") return p.target;
  return null;
}

function yearsOf(g: Graph["people"][string]): string {
  return cardYears(g.birth?.year, g.death?.year, g.living);
}

/** The family around batch person `pid`; `archive` is the tree graph of the person they are joined with, if any. */
export function familyOf(state: ImportState, pid: string, archive: Graph | null): MiniFamily | null {
  const me = state.persons.find((p) => p.id === pid);
  if (!me) return null;
  const rel = state.relationships;
  const included = (p: ImportPerson | undefined): p is ImportPerson => !!p && p.decision !== "skip" && !p.excluded;
  const byId = (id: string | null) => state.persons.find((p) => p.id === id);
  const target = mergeTarget(me);
  const graphMe = target && archive?.people[target] ? archive.people[target] : null;

  // Archive relatives first (unchanged), then the batch's: a batch relative joined with one of them marks that card.
  const slots: Record<keyof MiniFamily, MiniNode[]> = { parents: [], couple: [], children: [] };
  const archiveSlot = (ids: string[], slot: keyof MiniFamily) => {
    for (const id of ids) {
      const g = archive?.people[id];
      if (g) slots[slot].push({ key: id, name: g.name, years: yearsOf(g), state: "existing", branch: g.branch });
    }
  };
  if (graphMe) {
    archiveSlot(graphMe.parents, "parents");
    archiveSlot(graphMe.partners, "couple");
    archiveSlot(graphMe.children, "children");
  }
  const batchSlot = (ids: (string | null)[], slot: keyof MiniFamily) => {
    for (const id of ids) {
      const p = byId(id);
      if (!included(p)) continue;
      const joined = mergeTarget(p);
      const existing = joined ? slots[slot].find((n) => n.key === joined) : undefined;
      if (existing) existing.state = "merged";
      else slots[slot].push({ key: p.id, name: p.name, years: p.years, state: joined ? "merged" : "new" });
    }
  };
  batchSlot(rel.filter((r) => r.type === "parent" && r.child === pid).map((r) => r.parent), "parents");
  batchSlot(rel.filter((r) => r.type === "partners" && (r.a === pid || r.b === pid)).map((r) => (r.a === pid ? r.b : r.a)), "couple");
  batchSlot(rel.filter((r) => r.type === "parent" && r.parent === pid).map((r) => r.child), "children");

  const self: MiniNode = { key: me.id, name: me.name, years: me.years || (graphMe ? yearsOf(graphMe) : ""), state: target ? "merged" : "new", branch: graphMe?.branch };
  // What the import changes first, so it stays visible when a row is too long.
  const order = { new: 0, merged: 1, existing: 2 };
  const sort = (list: MiniNode[]) => [...list].sort((a, b) => order[a.state] - order[b.state]);
  return { parents: sort(slots.parents), couple: [self, ...sort(slots.couple)], children: sort(slots.children) };
}

export function MiniTree({ family, width, height, cardW, cardH }: { family: MiniFamily; width: number; height: number; cardW: number; cardH: number }) {
  const gap = 10;
  const rowGap = 36;
  const fit = Math.max(1, Math.floor((width - 12 + gap) / (cardW + gap)));
  const rows = (["parents", "couple", "children"] as const).filter((r) => family[r].length > 0);
  const total = rows.length * cardH + (rows.length - 1) * rowGap;
  const top = Math.max(8, (height - total) / 2);

  type Placed = MiniNode & { x: number; y: number };
  const placed: Record<string, Placed[]> = {};
  // Rows too long for the box show their first cards and a „+N” badge on the last one.
  const hidden: { x: number; y: number; n: number }[] = [];
  rows.forEach((row, k) => {
    const list = family[row];
    const shown = list.slice(0, fit);
    const rowW = shown.length * cardW + (shown.length - 1) * gap;
    const x0 = (width - rowW) / 2;
    const y = top + k * (cardH + rowGap);
    placed[row] = shown.map((n, i) => ({ ...n, x: x0 + i * (cardW + gap), y }));
    if (list.length > shown.length) hidden.push({ x: x0 + rowW, y, n: list.length - shown.length });
  });

  const focus = placed.couple?.[0];
  const lines: { d: string; fresh: boolean }[] = [];
  const bus = (real: Placed[], to: Placed[], down: Placed) => {
    if (real.length === 0) return;
    const x = (real[0].x + real[real.length - 1].x + cardW) / 2;
    const y1 = real[0].y + cardH;
    const mid = y1 + rowGap / 2;
    for (const t of to) {
      const tx = t.x + cardW / 2;
      lines.push({ d: `M${x},${y1} V${mid} H${tx} V${t.y}`, fresh: t.state === "new" || real.some((n) => n.state === "new") || down.state === "new" });
    }
  };
  if (focus && placed.parents) bus(placed.parents, [focus], focus);
  if (focus && placed.children) bus(placed.couple.slice(0, 2), placed.children, focus);
  // The couple: a line between the partners.
  const partner = placed.couple?.[1];
  if (focus && partner) lines.push({ d: `M${focus.x + cardW},${focus.y + cardH / 2} H${partner.x}`, fresh: partner.state === "new" || focus.state === "new" });

  return (
    <div className="imp-mini" style={{ height }}>
      <svg width={width} height={height} style={{ position: "absolute", inset: 0 }}>
        {lines.map((l, k) => (
          <path key={k} d={l.d} fill="none" style={l.fresh ? { stroke: "var(--accent)", strokeWidth: 1.5, strokeDasharray: "4 3" } : { stroke: "var(--line)", strokeWidth: 1.5 }} />
        ))}
      </svg>
      {Object.values(placed)
        .flat()
        .map((n) => (
          <div key={n.key} className={`imp-mini-card ${n.state}`} style={{ left: n.x, top: n.y, width: cardW, height: cardH }} title={`${n.name} ${n.years}`.trim()}>
            <span className="stripe" style={{ background: n.branch ? `var(--b${n.branch})` : "var(--line)" }} />
            <span className="col" style={{ minWidth: 0 }}>
              <span className="imp-mini-name">{n.name}</span>
              {n.years && <span className="imp-mini-years">{n.years}</span>}
            </span>
          </div>
        ))}
      {hidden.map((h) => (
        <span key={`${h.x}-${h.y}`} className="imp-mini-more" style={{ left: h.x - 24, top: h.y - 8 }}>
          +{h.n}
        </span>
      ))}
    </div>
  );
}

export function MiniLegend() {
  return (
    <div className="col" style={{ gap: 6, fontSize: 12, color: "var(--text2)" }}>
      <span className="row" style={{ gap: 8 }}>
        <span className="imp-swatch new" />
        nowa osoba z importu
      </span>
      <span className="row" style={{ gap: 8 }}>
        <span className="imp-swatch merged" />
        połączona z istniejącą
      </span>
      <span className="row" style={{ gap: 8 }}>
        <span className="imp-swatch existing" />
        już w archiwum, bez zmian
      </span>
    </div>
  );
}
