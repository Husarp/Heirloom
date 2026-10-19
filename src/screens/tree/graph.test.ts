import { describe, expect, it } from "vitest";
import { foldBranch, unfoldAnswer, type Graph, type GraphPerson } from "./graph";

const person = (id: string, children: string[], descendants: number, descendantsNear: number) => ({ id, children, descendants, descendantsNear }) as unknown as GraphPerson;

describe("Rodzina's „+N”: what to do with it", () => {
  const short = person("a", ["b"], 3, 3);
  const long = person("a", ["b"], 9, 4);

  it("unfolds at once when the line goes two generations down at most", () => {
    for (const choice of ["ask", "all", "two"] as const) expect(unfoldAnswer(short, choice)).toEqual({ ask: false, depth: null });
  });

  it("asks when it goes further, unless the choice was remembered", () => {
    expect(unfoldAnswer(long, "ask")).toEqual({ ask: true });
    expect(unfoldAnswer(long, "all")).toEqual({ ask: false, depth: null });
    expect(unfoldAnswer(long, "two")).toEqual({ ask: false, depth: 2 });
  });
});

describe("Rodzina: folding a branch", () => {
  const graph = { people: { k: person("k", ["g"], 2, 2), g: person("g", ["gg"], 1, 1), gg: person("gg", [], 0, 0), other: person("other", [], 0, 0) } } as unknown as Graph;

  it("takes out the branch and what was unfolded below it, and nothing else", () => {
    const unfolded = new Map<string, number | null>([["k", 2], ["g", null], ["other", 2]]);
    expect([...foldBranch(unfolded, graph, "k")]).toEqual([["other", 2]]);
    expect([...foldBranch(unfolded, graph, "g")]).toEqual([["k", 2], ["other", 2]]);
    // The map in hand is left as it was.
    expect(unfolded.size).toBe(3);
  });
});
