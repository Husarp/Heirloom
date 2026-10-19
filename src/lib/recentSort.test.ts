import { describe, expect, it } from "vitest";
import type { RecentArchive } from "../api/types";
import { sortRecent } from "./recentSort";

const r = (name: string, openedAt: string, editedAt?: string): RecentArchive => ({ path: name, name, openedAt, people: 1, editedAt });
const list = [r("Żak", "2026-10-03"), r("Ćma", "2026-10-02", "2026-09-01"), r("Adam", "2026-10-01", "2026-10-05"), r("Ewa", "2026-09-30")];

describe("sortRecent", () => {
  it("keeps the stored order for opened", () => {
    expect(sortRecent(list, "opened").map((x) => x.name)).toEqual(["Żak", "Ćma", "Adam", "Ewa"]);
  });
  it("puts the newest edit first, then the never edited by opened", () => {
    expect(sortRecent(list, "edited").map((x) => x.name)).toEqual(["Adam", "Ćma", "Żak", "Ewa"]);
  });
  it("sorts by name with Polish collation", () => {
    expect(sortRecent(list, "name").map((x) => x.name)).toEqual(["Adam", "Ćma", "Ewa", "Żak"]);
  });
  it("does not change the input", () => {
    sortRecent(list, "name");
    expect(list[0]?.name).toBe("Żak");
  });
});
