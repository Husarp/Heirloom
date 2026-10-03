import { describe, expect, it } from "vitest";
import { buildTree, linesFor, parsePlace, written, type PlaceRow } from "./placeTree";

// places.list of the generated test family (byte order, as the API sends it).
const paths = [
  ["Galicja"],
  ["Galicja", "Tarnów"],
  ["Kraków"],
  ["Poznań"],
  ["Warszawa"],
  ["gubernia lubelska"],
  ["gubernia lubelska", "Chełm"],
  ["gubernia lubelska", "Lublin"],
  ["gubernia lubelska", "powiat krasnostawski"],
  ["gubernia lubelska", "powiat krasnostawski", "Żółkiewka"],
  ["gubernia lubelska", "powiat lubelski"],
  ["gubernia lubelska", "powiat lubelski", "parafia Łęczna"],
  ["gubernia lubelska", "powiat lubelski", "parafia Łęczna", "Ciechanki"],
  ["gubernia lubelska", "powiat lubelski", "parafia Łęczna", "Wólka"],
  ["gubernia lubelska", "powiat lubelski", "Łęczna"],
  ["gubernia warszawska"],
  ["gubernia warszawska", "powiat błoński"],
  ["gubernia warszawska", "powiat błoński", "Wiskitki"],
  ["gubernia warszawska", "Łowicz"],
];
const rows: PlaceRow[] = paths.map((path) => ({
  path,
  name: path[path.length - 1],
  level: path.length - 1,
  count: 1,
  leaf: !paths.some((p) => p.length === path.length + 1 && path.every((part, i) => p[i] === part)),
}));
const tree = buildTree(rows);
const names = (lines: { node: { name: string }; depth: number }[]) => lines.map((l) => `${"  ".repeat(l.depth)}${l.node.name}`);

describe("place tree", () => {
  it("reads the route both as written and as a JSON path", () => {
    const path = ["gubernia lubelska", "powiat lubelski", "parafia Łęczna", "Wólka"];
    expect(written(path)).toBe("Wólka, parafia Łęczna, powiat lubelski, gubernia lubelska");
    expect(parsePlace(written(path))).toEqual(path);
    expect(parsePlace(JSON.stringify(path))).toEqual(path);
    expect(parsePlace(" Wólka ,, Łęczna ")).toEqual(["Łęczna", "Wólka"]);
    expect(parsePlace("")).toBeNull();
  });

  it("sorts in Polish order and opens only what is expanded", () => {
    expect(names(linesFor(tree, "admin", new Set(), ""))).toEqual(["Galicja", "gubernia lubelska", "gubernia warszawska", "Kraków", "Poznań", "Warszawa"]);
    const open = new Set(["gubernia lubelska", "powiat lubelski, gubernia lubelska"]);
    expect(names(linesFor(tree, "admin", open, "")).slice(1, 9)).toEqual([
      "gubernia lubelska",
      "  Chełm",
      "  Lublin",
      "  powiat krasnostawski",
      "  powiat lubelski",
      "    Łęczna",
      "    parafia Łęczna",
      "gubernia warszawska",
    ]);
  });

  it("shows matches with the places above them while searching, without Polish letters", () => {
    expect(names(linesFor(tree, "admin", new Set(), "wolka"))).toEqual(["gubernia lubelska", "  powiat lubelski", "    parafia Łęczna", "      Wólka"]);
  });

  it("lists parishes by name, and every place in the A–Z view with the place above it", () => {
    expect(names(linesFor(tree, "parish", new Set(), ""))).toEqual(["parafia Łęczna"]);
    const az = linesFor(tree, "az", new Set(), "");
    expect(az).toHaveLength(paths.length);
    expect(az.map((l) => l.node.name).slice(0, 4)).toEqual(["Chełm", "Ciechanki", "Galicja", "gubernia lubelska"]);
    expect(az.find((l) => l.node.name === "Wólka")?.note).toBe("parafia Łęczna");
  });
});
