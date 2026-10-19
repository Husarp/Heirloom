// Own file categories („Pocztówki”): a name and the kind it behaves like. The list is kept in the archive's display
// settings (`mediaCategories`); the backend (`media.categories`) adds the names found on files, so this file only holds
// the pure parts: checking a name, changing the saved list, and the kind choices of a file.

/** The kind a category behaves like: in Galeria (photo), in Dokumenty (document) or neither (other). */
export type Base = "photo" | "document" | "other";

/** One row of `media.categories`: `saved` is false for a name found only on files. */
export interface Category {
  name: string;
  base: Base;
  count: number;
  saved: boolean;
}

export interface SavedCategory {
  name: string;
  base: Base;
}

export const BASES: Base[] = ["photo", "document", "other"];
export const BASE_LABEL: Record<Base, string> = { photo: "Zdjęcie", document: "Dokument", other: "Inne" };
/** „jak zdjęcie” next to a category's name. */
export const BASE_LIKE: Record<Base, string> = { photo: "jak zdjęcie", document: "jak dokument", other: "inne" };

export const MAX_NAME = 60;
/** The built-in kinds' names (in the media filters and the lists they also appear in the plural). */
const BUILT_IN = ["zdjęcie", "zdjęcia", "dokument", "dokumenty", "inne", "inny"];

const lower = (text: string) => text.toLocaleLowerCase("pl");
export const sameName = (a: string, b: string) => lower(a) === lower(b);
/** Trimmed, with single spaces (the backend stores it the same way). */
export const cleanName = (name: string) => name.trim().replace(/\s+/g, " ");

/** Why a name can't be used, or null. `existing`: every name in use; `renaming`: the one being renamed, which may
 *  keep its own name (or change only its letters). */
export function validateName(raw: string, existing: readonly string[], renaming?: string): string | null {
  const name = cleanName(raw);
  if (!name) return "Wpisz nazwę kategorii.";
  if (name.length > MAX_NAME) return `Nazwa może mieć najwyżej ${MAX_NAME} znaków.`;
  if (BUILT_IN.includes(lower(name))) return "To nazwa wbudowanego rodzaju — wybierz inną.";
  if (existing.some((e) => sameName(e, name) && !(renaming != null && sameName(e, renaming)))) return "Taka kategoria już jest.";
  return null;
}

/** `display.mediaCategories` as stored; anything that isn't a name with a known kind is left out. */
export function parseSaved(raw: unknown): SavedCategory[] {
  if (!Array.isArray(raw)) return [];
  const out: SavedCategory[] = [];
  for (const entry of raw) {
    const name = typeof entry?.name === "string" ? cleanName(entry.name) : "";
    const base = entry?.base;
    if (name && BASES.includes(base) && !out.some((o) => o.name === name)) out.push({ name, base });
  }
  return out;
}

export const withAdded = (saved: readonly SavedCategory[], name: string, base: Base): SavedCategory[] => [...saved.filter((c) => c.name !== name), { name, base }];
export const withRenamed = (saved: readonly SavedCategory[], from: string, to: string): SavedCategory[] => saved.map((c) => (c.name === from ? { ...c, name: to } : c));
export const withoutCategory = (saved: readonly SavedCategory[], name: string): SavedCategory[] => saved.filter((c) => c.name !== name);

/** What a file's kind is, as one value of a menu: `photo`, `document`, `other`, or `cat:<name>`. */
export const choiceValue = (item: { kind: string; category?: string | null }): string => (item.category ? `cat:${item.category}` : item.kind);

/** The kind and category a menu value stands for (null for a category that isn't in the list). */
export function choiceOf(value: string, categories: readonly { name: string; base: Base }[]): { kind: Base; category: string | null } | null {
  if (value.startsWith("cat:")) {
    const found = categories.find((c) => c.name === value.slice(4));
    return found ? { kind: found.base, category: found.name } : null;
  }
  return BASES.includes(value as Base) ? { kind: value as Base, category: null } : null;
}

/** The categories, with `current` among them even when the list doesn't have it yet (just added). */
export function withCurrent<T extends { name: string; base: Base }>(categories: readonly T[], current: { kind: string; category?: string | null }): { name: string; base: Base }[] {
  const list: { name: string; base: Base }[] = categories.map((c) => ({ name: c.name, base: c.base }));
  if (current.category && !list.some((c) => c.name === current.category)) list.push({ name: current.category, base: BASES.includes(current.kind as Base) ? (current.kind as Base) : "other" });
  return list;
}
