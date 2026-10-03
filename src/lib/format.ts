// Polish number, plural and time formats (design/IMPLEMENTATION_SPEC.md §1.11).

const NBSP = " ";

/** "1 284", "10 243" — Polish groups thousands even in 4-digit numbers in this design. */
export function num(n: number): string {
  return new Intl.NumberFormat("pl-PL", { useGrouping: "always" } as Intl.NumberFormatOptions)
    .format(n)
    .replace(/\s/g, NBSP);
}

/** Polish plural forms: plural(5, "osoba", "osoby", "osób") → "osób". */
export function plural(n: number, one: string, few: string, many: string): string {
  const abs = Math.abs(n);
  if (abs === 1) return one;
  const lastTwo = abs % 100;
  const last = abs % 10;
  if (last >= 2 && last <= 4 && !(lastTwo >= 12 && lastTwo <= 14)) return few;
  return many;
}

/** "1 284 osoby", "1 osoba", "5 osób". */
export function count(n: number, one: string, few: string, many: string): string {
  return `${num(n)}${NBSP}${plural(n, one, few, many)}`;
}

export const people = (n: number) => count(n, "osoba", "osoby", "osób");

/** "72 lata", "1 rok", "95 lat". */
export const years = (n: number) => count(n, "rok", "lata", "lat");

const MONTHS_GENITIVE = [
  "stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca",
  "lipca", "sierpnia", "września", "października", "listopada", "grudnia",
];
const WEEKDAYS = ["Niedziela", "Poniedziałek", "Wtorek", "Środa", "Czwartek", "Piątek", "Sobota"];

/** "Poniedziałek, 28 września 2026". */
export function todayLong(now = new Date()): string {
  return `${WEEKDAYS[now.getDay()]}, ${now.getDate()} ${MONTHS_GENITIVE[now.getMonth()]} ${now.getFullYear()}`;
}

/** "28 września". */
export function dayMonth(date: Date): string {
  return `${date.getDate()} ${MONTHS_GENITIVE[date.getMonth()]}`;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** "10:42" today, "wczoraj", "3 dni temu", "2 tyg. temu", "12.09.2026". Takes an ISO/RFC 3339 string. */
export function relativeTime(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return "—";
  const date = new Date(iso.length === 10 ? `${iso}T12:00:00` : iso);
  if (Number.isNaN(date.getTime())) return "—";
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000);
  if (days <= 0) return iso.length > 10 ? `${pad(date.getHours())}:${pad(date.getMinutes())}` : "dziś";
  if (days === 1) return "wczoraj";
  if (days < 7) return `${days} dni temu`;
  if (days < 28) return `${Math.floor(days / 7)} tyg. temu`;
  return `${pad(date.getDate())}.${pad(date.getMonth() + 1)}.${date.getFullYear()}`;
}

/** "dziś", "wczoraj", "15.09", "2024" — for lists of archives and editors. */
export function shortWhen(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(date)) / 86_400_000);
  if (days <= 0) return "dziś";
  if (days === 1) return "wczoraj";
  if (date.getFullYear() !== now.getFullYear()) return String(date.getFullYear());
  return `${pad(date.getDate())}.${pad(date.getMonth() + 1)}`;
}

/** "11:04". */
export function clock(iso: string | null | undefined): string {
  if (!iso) return "";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "" : `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Roman numerals for generations: 7 → "VII". */
export function roman(n: number): string {
  const table: [number, string][] = [
    [1000, "M"], [900, "CM"], [500, "D"], [400, "CD"], [100, "C"], [90, "XC"],
    [50, "L"], [40, "XL"], [10, "X"], [9, "IX"], [5, "V"], [4, "IV"], [1, "I"],
  ];
  let out = "";
  let rest = Math.max(0, Math.floor(n));
  for (const [value, letters] of table) {
    while (rest >= value) {
      out += letters;
      rest -= value;
    }
  }
  return out;
}

/** The name on a person card (design v2, A9): "Józef KOWALSKI", "Maria Magdalena WIŚNIEWSKA-ZAWADZKA". */
export function cardName(p: { name: string; given: string; surname: string }): string {
  const given = p.given.trim();
  const surname = p.surname.trim();
  if (!given && !surname) return p.name;
  const upper = surname.toLocaleUpperCase("pl-PL");
  // „Nazwisko Imię” chosen in Ustawienia: the display name then starts with the surname.
  const surnameFirst = !!given && !!surname && p.name.startsWith(surname) && !p.name.startsWith(given);
  return (surnameFirst ? [upper, given] : [given, upper]).filter(Boolean).join(" ");
}

/** Years on a card as text: "1878 † 1951", "1931 · żyje", "ok. 1850 † ?". */
export function cardYears(birth: string | null | undefined, death: string | null | undefined, living: boolean): string {
  if (birth && death) return `${birth} † ${death}`;
  if (birth) return living ? `${birth} · żyje` : `${birth} † ?`;
  if (death) return `? † ${death}`;
  return living ? "żyje" : "";
}

/** "JK" from "Józef Kowalski"; "H" from a single word. */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  return words
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? "")
    .join("");
}

/** A Windows path as people write it: without the `\\?\` prefix long paths get (`\\?\UNC\server` → `\\server`). */
export function displayPath(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) return `\\\\${path.slice(8)}`;
  if (path.startsWith("\\\\?\\")) return path.slice(4);
  return path;
}

/** "geneteka.genealodzy.pl" from a URL. */
export function domain(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** "6,7 GB", "212 MB". */
export function bytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = n;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  const digits = value >= 10 || unit === 0 ? 0 : 1;
  return `${value.toFixed(digits).replace(".", ",")}${NBSP}${units[unit]}`;
}
