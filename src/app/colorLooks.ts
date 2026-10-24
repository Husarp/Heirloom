import type { Appearance } from "../api/types";

/** The colour schemes and accent colours of Ustawienia › Wygląd, in the order shown. `value` is the `data-scheme` /
 *  `data-accent` on <html> (src/styles/schemes.css; „Muzeum” and „Domyślny” have no rule: they are tokens.css). */
export const SCHEMES: { value: Appearance["scheme"]; label: string; note: string }[] = [
  { value: "museum", label: "Muzeum", note: "Ciepły papier i zieleń, stonowane" },
  { value: "poranek", label: "Poranek", note: "Jasny i chłodny, lazur; w ciemnym granatowa noc" },
  { value: "terakota", label: "Terakota", note: "Ciepły i słoneczny; w ciemnym kawa z mlekiem" },
  { value: "wrzos", label: "Wrzos", note: "Łagodny, liliowy i fioletowy; w ciemnym oberżyna" },
];

export const ACCENTS: { value: Appearance["accent"]; label: string }[] = [
  { value: "default", label: "Domyślny dla kolorystyki" },
  { value: "green", label: "Zieleń" },
  { value: "navy", label: "Granat" },
  { value: "burgundy", label: "Bordo" },
  { value: "plum", label: "Śliwka" },
  { value: "copper", label: "Miedź" },
  { value: "sea", label: "Morski" },
];

/** The scheme / accent of the settings, or the default for a value this version doesn't know (a file from a newer one). */
export function knownScheme(value: string | undefined): Appearance["scheme"] {
  return SCHEMES.find((s) => s.value === value)?.value ?? "museum";
}
export function knownAccent(value: string | undefined): Appearance["accent"] {
  return ACCENTS.find((a) => a.value === value)?.value ?? "default";
}
