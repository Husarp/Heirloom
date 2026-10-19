// Step 4's words by the kind of a file (a photo, a document, anything else) and the search in „Przypisz”.

import { count, fold } from "../../lib/format";

/** The kind a file behaves like (a file with an own category passes its base kind). */
export type Noun = "photo" | "document" | "other";

export function nounOf(kind: string | null | undefined): Noun {
  return kind === "photo" || kind === "document" ? kind : "other";
}

/** Status chip: nothing was delivered for the file. */
export const missingLabel = (kind: string | null | undefined) => ({ photo: "Brak zdjęcia", document: "Brak dokumentu", other: "Brak pliku" })[nounOf(kind)];

/** Status chip: the package doesn't describe the file. */
export const undescribedLabel = (kind: string | null | undefined) => ({ photo: "Zdjęcie nieopisane", document: "Dokument nieopisany", other: "Plik nieopisany" })[nounOf(kind)];

/** The empty thumb of a file that wasn't delivered. */
export const notAttachedLabel = (kind: string | null | undefined) => ({ photo: "Nie dołączono zdjęcia", document: "Nie dołączono dokumentu", other: "Nie dołączono pliku" })[nounOf(kind)];

/** The footer: „3 pliki wymagają uwagi”; „2 zdjęcia wymagają uwagi” when they are all photos (documents likewise). */
export function needAttention(kinds: (string | null | undefined)[]): string {
  const nouns = new Set(kinds.map(nounOf));
  const only = nouns.size === 1 ? [...nouns][0] : "other";
  const n = kinds.length;
  const [one, few, many] = { photo: ["zdjęcie wymaga", "zdjęcia wymagają", "zdjęć wymaga"], document: ["dokument wymaga", "dokumenty wymagają", "dokumentów wymaga"], other: ["plik wymaga", "pliki wymagają", "plików wymaga"] }[only];
  return count(n, one, few, many);
}

/** The people matching what was typed (diacritics and case ignored), in their order. Every word has to be in the name. */
export function matchPeople<T extends { name: string }>(list: readonly T[], query: string): T[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...list];
  return list.filter((p) => {
    const name = fold(p.name);
    return words.every((w) => name.includes(w));
  });
}
