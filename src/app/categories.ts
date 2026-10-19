// Own file categories („Pocztówki”) in the running app: the list (saved choices plus names found on files) and writing
// the saved list. Names, kinds and the rules are in lib/categories.ts.

import { call, type ApiError } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { parseSaved, type Category, type SavedCategory } from "../lib/categories";
import { useStore } from "./store";
import { useApi } from "./useApi";

/** The archive's categories, each with the number of files that use it; empty until read. */
export function useCategories(): Category[] {
  return useApi<Category[]>("media.categories").data ?? [];
}

/** The saved list as the archive has it now. */
export const savedCategories = (): SavedCategory[] => parseSaved(useStore.getState().archive?.display.mediaCategories);

/** Writes the saved list into the archive's settings. These are choices of the archive, not family data, so it works
 *  without edit mode (like the other settings). A failure is shown and gives false. */
export async function saveCategories(list: SavedCategory[]): Promise<boolean> {
  if (JSON.stringify(list) === JSON.stringify(savedCategories())) return true;
  try {
    useStore.getState().changed(await call<ArchiveStatus>("archive.setSettings", { display: { mediaCategories: list } }));
    return true;
  } catch (e) {
    useStore.getState().notify((e as ApiError).message, { kind: "err" });
    return false;
  }
}
