import { describe, expect, it } from "vitest";
import type { Graph, GraphPerson } from "./graph";
import { CARD_W, layoutFamily, NARROW_W } from "./layout";

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
