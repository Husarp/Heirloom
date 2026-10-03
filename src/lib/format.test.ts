import { describe, expect, it } from "vitest";
import { initials, num, people, plural, relativeTime, roman, years, cardName, cardYears, displayPath } from "./format";

const NBSP = " ";

describe("Polish formats", () => {
  it("groups thousands with a non-breaking space, even for 4 digits", () => {
    expect(num(1284)).toBe(`1${NBSP}284`);
    expect(num(10243)).toBe(`10${NBSP}243`);
    expect(num(12)).toBe("12");
  });

  it("uses the right plural form", () => {
    expect([1, 2, 5, 12, 22, 25, 112, 1284].map((n) => plural(n, "osoba", "osoby", "osób"))).toEqual([
      "osoba", "osoby", "osób", "osób", "osoby", "osób", "osób", "osoby",
    ]);
    expect(people(214)).toBe(`214${NBSP}osób`);
    expect(years(72)).toBe(`72${NBSP}lata`);
  });

  it("writes relative times like the design", () => {
    const now = new Date(2026, 8, 28, 12, 0);
    expect(relativeTime("2026-09-28T10:42:00", now)).toBe("10:42");
    expect(relativeTime("2026-09-27T10:42:00", now)).toBe("wczoraj");
    expect(relativeTime("2026-09-25T10:42:00", now)).toBe("3 dni temu");
    expect(relativeTime("2026-09-14T10:42:00", now)).toBe("2 tyg. temu");
    expect(relativeTime("2026-08-12T10:42:00", now)).toBe("12.08.2026");
  });

  it("roman numerals and initials", () => {
    expect(roman(7)).toBe("VII");
    expect(roman(10)).toBe("X");
    expect(initials("Józef Kowalski")).toBe("JK");
    expect(initials("Maria Magdalena Konstancja")).toBe("MM");
  });
});

describe("cards (design v2, A9)", () => {
  it("writes the surname in capitals, in the order the name is shown", () => {
    expect(cardName({ name: "Józef Kowalski", given: "Józef", surname: "Kowalski" })).toBe("Józef KOWALSKI");
    expect(cardName({ name: "Maria Magdalena Wiśniewska-Zawadzka", given: "Maria Magdalena", surname: "Wiśniewska-Zawadzka" })).toBe("Maria Magdalena WIŚNIEWSKA-ZAWADZKA");
    expect(cardName({ name: "Kowalski Józef", given: "Józef", surname: "Kowalski" })).toBe("KOWALSKI Józef");
    expect(cardName({ name: "Łucja", given: "Łucja", surname: "" })).toBe("Łucja");
  });
  it("writes the years with the cross, and the living as such", () => {
    expect(cardYears("1878", "1951", false)).toBe("1878 † 1951");
    expect(cardYears("ok. 1850", "przed 1910", false)).toBe("ok. 1850 † przed 1910");
    expect(cardYears("1931", null, true)).toBe("1931 · żyje");
    expect(cardYears("1931", null, false)).toBe("1931 † ?");
    expect(cardYears(null, "1944", false)).toBe("? † 1944");
    expect(cardYears(null, null, false)).toBe("");
  });
});

describe("paths", () => {
  it("shows Windows paths without the long-path prefix", () => {
    expect(displayPath("\\\\?\\C:\\Heirloom\\Kowalscy")).toBe("C:\\Heirloom\\Kowalscy");
    expect(displayPath("\\\\?\\UNC\\NAS\\Rodzina")).toBe("\\\\NAS\\Rodzina");
    expect(displayPath("D:\\Heirloom")).toBe("D:\\Heirloom");
  });
});
