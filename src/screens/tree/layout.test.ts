import { describe, expect, it } from "vitest";
import type { Graph, GraphPerson } from "./graph";
import { CARD_W, CARD_H, layoutAncestors, layoutDescendants, layoutFamily, NARROW_W } from "./layout";

// father+mother → older, me, younger (+ extra children); father+other → half; me+wife, me+second (2nd partner, left).
function family(extra = 0): Graph {
  const people: Record<string, GraphPerson> = {};
  const add = (id: string, year: number, sex: string, parents: string[] = [], partners: string[] = []) => {
    people[id] = { id, name: id, given: id, surname: "", sex, birth: { sort: year * 10000, year: String(year) }, parents, children: [], partners, descendants: 0, ancestors: 0, hasParents: parents.length > 0 } as unknown as GraphPerson;
  };
  add("father", 1900, "M", [], ["mother", "other"]);
  add("mother", 1902, "F", [], ["father"]);
  add("other", 1905, "F", [], ["father"]);
  add("older", 1925, "F", ["father", "mother"]);
  add("me", 1927, "M", ["father", "mother"], ["wife", "second"]);
  add("younger", 1930, "M", ["father", "mother"]);
  add("half", 1940, "M", ["father", "other"]);
  add("wife", 1928, "F", ["wf", "wm"], ["me"]);
  add("wf", 1900, "M", [], ["wm"]);
  add("wm", 1901, "F", [], ["wf"]);
  add("second", 1929, "F", ["sf", "sm"], ["me"]);
  add("sf", 1899, "M", [], ["sm"]);
  add("sm", 1903, "F", [], ["sf"]);
  const kids = ["older", "me", "younger"];
  for (let i = 0; i < extra; i++) {
    add(`x${i}`, 1931 + i, "F", ["father", "mother"]);
    kids.push(`x${i}`);
  }
  const union = (id: string, partners: string[], children: string[]) => ({ id, partners, children: children.map((c) => ({ id: c, pedi: null })), married: true, year: null, uncertain: false, allChildren: children.length });
  for (const [p, c] of [["father", [...kids, "half"]], ["mother", kids], ["other", ["half"]], ["wf", ["wife"]], ["wm", ["wife"]], ["sf", ["second"]], ["sm", ["second"]]] as const) people[p].children = [...c];
  return {
    focus: "me",
    up: 1,
    down: 1,
    people,
    unions: [union("u1", ["father", "mother"], kids), union("u2", ["father", "other"], ["half"]), union("u3", ["wf", "wm"], ["wife"]), union("u4", ["sf", "sm"], ["second"]), union("u5", ["me", "wife"], []), union("u6", ["me", "second"], [])],
  };
}

const x = (scene: ReturnType<typeof layoutFamily>, id: string) => scene.cards.find((c) => c.id === id)!.x;

describe("Rodzina: siblings", () => {
  it("shows them all as cards: older left of the person and the 2nd partner, younger right of the 1st partner, the half-brother outermost on the father's side", () => {
    const scene = layoutFamily(family(), "me", { editing: false });
    expect(x(scene, "second")).toBeLessThan(0);
    expect(x(scene, "older") + CARD_W).toBeLessThan(x(scene, "second"));
    expect(x(scene, "half")).toBeLessThan(x(scene, "older"));
    expect(x(scene, "younger")).toBeGreaterThan(x(scene, "wife") + CARD_W);
    expect(scene.cards.find((c) => c.id === "half")!.sub).toBe("brat przyrodni");
    expect(scene.boxes).toEqual([]);
    // No two cards on a row overlap.
    const row = scene.cards.filter((c) => c.y === 0).sort((a, b) => a.x - b.x);
    for (let i = 1; i < row.length; i++) expect(row[i].x).toBeGreaterThanOrEqual(row[i - 1].x + (row[i - 1].w ?? CARD_W));
  });

  it("keeps the siblings clear of the partners' parents' lines", () => {
    const scene = layoutFamily(family(), "me", { editing: false });
    const landing = (p: string) => scene.links.find((l) => l.key === `pp-${p}-down`)!.d.match(/^M(-?[\d.]+)/)![1];
    expect(x(scene, "older") + CARD_W).toBeLessThan(+landing("second"));
    expect(x(scene, "younger")).toBeGreaterThan(+landing("wife"));
  });

  it("uses narrower cards from 9 siblings on", () => {
    expect(layoutFamily(family(5), "me", { editing: false }).cards.find((c) => c.id === "older")!.w).toBeUndefined();
    expect(layoutFamily(family(6), "me", { editing: false }).cards.find((c) => c.id === "older")!.w).toBe(NARROW_W);
  });
});

describe("Rodzina: partners' parents", () => {
  it("never run two partners' parents' lines along the same stretch (three partners)", () => {
    const graph = family();
    const add = (id: string, parents: string[] = [], partners: string[] = []) => {
      graph.people[id] = { ...graph.people.wife, id, name: id, given: id, parents, partners, children: [] };
    };
    add("third", ["tf", "tm"], ["me"]);
    add("tf", [], ["tm"]);
    add("tm", [], ["tf"]);
    graph.people.tf.sex = "M";
    graph.people.tf.children = graph.people.tm.children = ["third"];
    graph.people.me.partners = ["wife", "second", "third"];
    const union = (id: string, partners: string[], children: string[]) => ({ id, partners, children: children.map((c) => ({ id: c, pedi: null })), married: true, year: null, uncertain: false, allChildren: children.length });
    graph.unions.push(union("u7", ["tf", "tm"], ["third"]), union("u8", ["me", "third"], []));
    const scene = layoutFamily(graph, "me", { editing: false });
    const bars = scene.links
      .filter((l) => /^pp-.*-bar$/.test(l.key))
      .map((l) => l.d.match(/^M(-?[\d.]+) (-?[\d.]+) H(-?[\d.]+)$/)!.slice(1).map(Number));
    expect(bars.length).toBeGreaterThanOrEqual(2);
    for (let i = 0; i < bars.length; i++)
      for (let j = i + 1; j < bars.length; j++) {
        const [a, b] = [bars[i], bars[j]];
        if (a[1] === b[1]) expect(Math.min(a[2], b[2]) <= Math.max(a[0], b[0])).toBe(true);
      }
  });
});

// A graph from [id, sex, father, mother, birth year] rows; the unions come from the parents.
function graphOf(rows: [string, string, string | null, string | null, number][], focus: string, extraUnions: [string, string][] = []): Graph {
  const people: Record<string, GraphPerson> = {};
  for (const [id, sex, f, m, year] of rows) {
    const parents = [f, m].filter(Boolean) as string[];
    people[id] = { id, name: id, given: id, surname: "", sex, birth: { sort: year * 10000, year: String(year) }, parents, children: [], partners: [], descendants: 0, ancestors: 0, hasParents: parents.length > 0 } as unknown as GraphPerson;
  }
  const unions = new Map<string, { id: string; partners: string[]; children: { id: string; pedi: null }[]; married: boolean; year: null; uncertain: boolean; allChildren: number }>();
  const unionOf = (a: string, b: string | null) => {
    const key = [a, b].filter(Boolean).sort().join("+");
    if (!unions.has(key)) unions.set(key, { id: key, partners: [a, b].filter(Boolean) as string[], children: [], married: true, year: null, uncertain: false, allChildren: 0 });
    if (b && !people[a].partners.includes(b)) (people[a].partners.push(b), people[b].partners.push(a));
    return unions.get(key)!;
  };
  for (const [id, , f, m] of rows) {
    if (!f && !m) continue;
    unionOf((f ?? m)!, f && m ? m : null).children.push({ id, pedi: null });
    for (const p of [f, m]) if (p) people[p].children.push(id);
  }
  for (const [a, b] of extraUnions) unionOf(a, b);
  // Everyone's counts, as tree.rs gives them.
  const count = (id: string, next: (p: GraphPerson) => string[]) => {
    const seen = new Set<string>();
    const stack = [id];
    while (stack.length) for (const n of next(people[stack.pop()!])) if (!seen.has(n)) (seen.add(n), stack.push(n));
    return seen.size;
  };
  for (const p of Object.values(people)) {
    p.descendants = count(p.id, (q) => q.children);
    p.ancestors = count(p.id, (q) => q.parents);
  }
  return { focus, up: 64, down: 0, people, unions: [...unions.values()].map((u) => ({ ...u, allChildren: u.children.length })) };
}

const card = (scene: ReturnType<typeof layoutFamily>, id: string) => scene.cards.find((c) => c.id === id)!;
function noOverlaps(scene: ReturnType<typeof layoutFamily>) {
  for (const a of scene.cards)
    for (const b of scene.cards) {
      if (a === b) continue;
      const apart = a.x + (a.w ?? CARD_W) <= b.x || b.x + (b.w ?? CARD_W) <= a.x || a.y + CARD_H <= b.y || b.y + CARD_H <= a.y;
      expect(apart, `${a.id} and ${b.id} overlap`).toBe(true);
    }
}

describe("Przodkowie", () => {
  // me ← f, m; f ← ff, fm; ff ← fff, ffm; fff ← ffff (father only); ffff ← f5, m5; m ← (no parents);
  // fm ← fmf (father only). Seven generations on the father's line.
  const rows: [string, string, string | null, string | null, number][] = [
    ["f5", "M", null, null, 1700],
    ["m5", "F", null, null, 1702],
    ["ffff", "M", "f5", "m5", 1730],
    ["fff", "M", "ffff", null, 1760],
    ["ffm", "F", null, null, 1762],
    ["ff", "M", "fff", "ffm", 1790],
    ["fmf", "M", null, null, 1765],
    ["fm", "F", "fmf", null, 1792],
    ["f", "M", "ff", "fm", 1820],
    ["m", "F", null, null, 1822],
    ["me", "M", "f", "m", 1850],
  ];

  it("shows the whole line, the person on the right and each generation a column further left", () => {
    const scene = layoutAncestors(graphOf(rows, "me"), "me");
    for (const [id] of rows) expect(card(scene, id), id).toBeDefined();
    expect(card(scene, "me").focus).toBe(true);
    expect(Math.max(...scene.cards.map((c) => c.x))).toBe(card(scene, "me").x);
    expect(card(scene, "f").x).toBeLessThan(card(scene, "me").x);
    expect(card(scene, "f5").x).toBe(card(scene, "me").x - 5 * (card(scene, "me").x - card(scene, "f").x));
    expect(scene.pills).toEqual([]);
    noOverlaps(scene);
  });

  it("puts each child halfway between its parents", () => {
    const scene = layoutAncestors(graphOf(rows, "me"), "me");
    const mid = (a: string, b: string) => (card(scene, a).y + card(scene, b).y) / 2;
    expect(card(scene, "me").y).toBe(mid("f", "m"));
    expect(card(scene, "ff").y).toBe(mid("fff", "ffm"));
  });

  it("ends a line where neither parent is known, and boxes an unknown one beside a known one", () => {
    const scene = layoutAncestors(graphOf(rows, "me"), "me");
    const stubs = scene.cards.filter((c) => c.stub);
    // fff's and fm's mothers are unknown; m, ffm, fmf, f5 and m5 end their lines.
    expect(stubs.map((c) => c.stub).sort()).toEqual(["matka nieznana", "matka nieznana"]);
    expect(scene.links.filter((l) => l.kind === "placeholder")).toHaveLength(2);
  });

  it("keeps a tidy tree: a line that ends early leaves its rows to the others", () => {
    const scene = layoutAncestors(graphOf(rows, "me"), "me");
    // Seven ends of lines; one row each would be 6 rows' pitch from the top card to the bottom one.
    const ys = scene.cards.map((c) => c.y);
    expect(Math.max(...ys) - Math.min(...ys)).toBeLessThan(6 * 88);
  });

  it("names the first four columns only, and keeps those names at the top of the screen", () => {
    const scene = layoutAncestors(graphOf(rows, "me"), "me");
    expect(scene.labels.map((l) => l.text)).toEqual(["Osoba", "Rodzice", "Dziadkowie", "Pradziadkowie"]);
    expect(scene.labels.every((l) => l.stick === "top")).toBe(true);
    expect(scene.labels[1].x).toBe(card(scene, "f").x);
  });

  it("draws someone met twice (cousins who married) once, the second time as a note", () => {
    // me's parents are cousins: both descend from g.
    const graph = graphOf(
      [
        ["g", "M", null, null, 1750],
        ["a", "M", "g", null, 1780],
        ["b", "F", "g", null, 1782],
        ["f", "M", "a", null, 1810],
        ["m", "F", "b", null, 1812],
        ["me", "M", "f", "m", 1840],
      ],
      "me",
    );
    const scene = layoutAncestors(graph, "me");
    expect(scene.cards.filter((c) => c.id === "g")).toHaveLength(1);
    expect(scene.cards.some((c) => c.stub?.startsWith("↻ g"))).toBe(true);
    expect(new Set(scene.cards.map((c) => c.id)).size).toBe(scene.cards.length);
    noOverlaps(scene);
  });
});

describe("Potomkowie", () => {
  // root ∞ wife → c1, c2; c1 ∞ w1 → g1, g2; g1 → gg1 → ggg1 → gggg1 → ggggg1; c2 (no partner) → g3.
  const rows: [string, string, string | null, string | null, number][] = [
    ["root", "M", null, null, 1800],
    ["wife", "F", null, null, 1802],
    ["c1", "M", "root", "wife", 1825],
    ["c2", "F", "root", "wife", 1827],
    ["w1", "F", null, null, 1826],
    ["g1", "M", "c1", "w1", 1850],
    ["g2", "F", "c1", "w1", 1852],
    ["g3", "M", null, "c2", 1855],
    ["gg1", "M", "g1", null, 1880],
    ["ggg1", "M", "gg1", null, 1910],
    ["gggg1", "M", "ggg1", null, 1940],
    ["ggggg1", "M", "gggg1", null, 1970],
  ];
  const graph = graphOf(rows, "root");

  it("stops two generations down, with a pill that unfolds the branch in place", () => {
    const scene = layoutDescendants(graph, "root", { depth: 2 });
    expect(scene.cards.map((c) => c.id)).not.toContain("gg1");
    const pill = scene.pills.find((p) => p.target === "g1")!;
    expect(pill.unfold).toBe(true);
    expect(pill.label).toBe("+4 potomków");
  });

  it("unfolds three more generations below an unfolded person, the centre staying where it was", () => {
    const folded = layoutDescendants(graph, "root", { depth: 2 });
    const scene = layoutDescendants(graph, "root", { depth: 2, unfolded: ["g1"] });
    expect(card(scene, "root").focus).toBe(true);
    expect(card(scene, "root").y).toBe(card(folded, "root").y);
    for (const id of ["gg1", "ggg1", "gggg1"]) expect(card(scene, id), id).toBeDefined();
    expect(scene.cards.map((c) => c.id)).not.toContain("ggggg1");
    expect(scene.pills.map((p) => p.target)).toEqual(["gggg1"]);
    // Clicking that one goes on from there.
    expect(card(layoutDescendants(graph, "root", { depth: 2, unfolded: ["g1", "gggg1"] }), "ggggg1")).toBeDefined();
    noOverlaps(scene);
  });

  it("names every generation in the column at the left edge", () => {
    const scene = layoutDescendants(graph, "root", { depth: 2, unfolded: ["g1"] });
    expect(scene.labels.map((l) => l.text)).toEqual(["Osoba", "Dzieci", "Wnuki", "Prawnuki", "Praprawnuki", "Pokolenie +5"]);
    expect(scene.labels.every((l) => l.stick === "left")).toBe(true);
    // Not part of the tree's extent: they are drawn on screen.
    expect(scene.bounds.minX).toBe(Math.min(...scene.cards.map((c) => c.x)));
  });

  it("shows partners as greyed cards beside each person, the children hanging from their marriage", () => {
    const scene = layoutDescendants(graph, "root", { depth: 2, partners: true });
    expect(card(scene, "wife").partner).toBe(true);
    expect(card(scene, "wife").rel).toEqual({ of: "root", kind: "partner" });
    expect(card(scene, "wife").y).toBe(card(scene, "root").y);
    expect(card(scene, "wife").x).toBeGreaterThan(card(scene, "root").x);
    expect(card(scene, "w1").partner).toBe(true);
    expect(scene.cards.filter((c) => c.partner).map((c) => c.id).sort()).toEqual(["w1", "wife"]);
    // Not followed further: no pill on a partner.
    expect(scene.pills.some((p) => p.target === "wife" || p.target === "w1")).toBe(false);
    // root and wife's children come down from the marker between them; c2's son from c2's own card.
    const marker = scene.unions.find((u) => u.people.includes("root") && u.people.includes("wife"))!;
    expect(marker.x).toBeGreaterThan(card(scene, "root").x + CARD_W);
    expect(marker.x).toBeLessThan(card(scene, "wife").x);
    expect(scene.links.find((l) => l.key === "d-root-down")!.d.startsWith(`M${marker.x} `)).toBe(true);
    expect(scene.links.find((l) => l.key === "d-c2-down")!.d.startsWith(`M${card(scene, "c2").x + CARD_W / 2} `)).toBe(true);
    expect(card(scene, "root").sub).toBeNull();
    noOverlaps(scene);
  });

  it("draws rounded lines too (Ustawienia › Drzewo › Linie)", () => {
    const scenes = [layoutDescendants(graph, "root", { depth: 2, partners: true, rounded: true, unfolded: ["g1"] }), layoutAncestors(graph, "ggggg1", true)];
    for (const scene of scenes) for (const l of scene.links) expect(l.d, l.key).not.toMatch(/NaN|Infinity/);
  });

  it("without partners, names the spouse in the sub-line instead", () => {
    const scene = layoutDescendants(graph, "root", { depth: 2 });
    expect(scene.cards.some((c) => c.partner)).toBe(false);
    expect(card(scene, "root").sub).toBe("∞ wife");
    expect(scene.unions).toEqual([]);
  });
});
