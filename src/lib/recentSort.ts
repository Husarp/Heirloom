import type { RecentArchive } from "../api/types";

export type RecentSort = "opened" | "edited" | "name";

const collator = new Intl.Collator("pl");

/** The recent archives in the chosen order (a new array). „opened” is the stored order; „edited”: newest edit first, the
 *  never-edited ones after, by when they were opened. */
export function sortRecent(list: RecentArchive[], sort: RecentSort): RecentArchive[] {
  const byOpened = (a: RecentArchive, b: RecentArchive) => b.openedAt.localeCompare(a.openedAt);
  const out = [...list];
  if (sort === "name") return out.sort((a, b) => collator.compare(a.name, b.name));
  if (sort === "edited") {
    return out.sort((a, b) => {
      if (a.editedAt && b.editedAt) return b.editedAt.localeCompare(a.editedAt);
      if (a.editedAt || b.editedAt) return a.editedAt ? -1 : 1;
      return byOpened(a, b);
    });
  }
  return out;
}
