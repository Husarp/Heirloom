import { describe, expect, it } from "vitest";
import { fitName } from "./nameFit";

// Every letter 0.6 of the font size wide.
const width = (text: string, size: number) => text.length * size * 0.6;

describe("a name on a tree card", () => {
  it("keeps the full size when it fits", () => {
    expect(fitName("Jan KOWALSKI", 125, 15, width)).toEqual({ size: 15, lines: 1, cut: false });
  });

  it("gets a little smaller before it takes two lines", () => {
    // 14 letters: 126 px at 15 px, 117.6 px at 14 px.
    expect(fitName("Józef KOWALSKI", 120, 15, width)).toEqual({ size: 14, lines: 1, cut: false });
  });

  it("goes onto two lines rather than below 13 px", () => {
    expect(fitName("Stanisław KAMIŃSKI", 125, 15, width)).toEqual({ size: 13, lines: 2, cut: false });
  });

  it("is cut, with the whole name in the tooltip, only when two lines at the smallest size are not enough", () => {
    expect(fitName("Maria Magdalena WIŚNIEWSKA-KONOPNICKA", 125, 15, width)).toEqual({ size: 11.5, lines: 2, cut: true });
  });

  it("a single long word stays on one line and shrinks further", () => {
    expect(fitName("WIERZBICKA-KOWALCZYK", 150, 15, width)).toEqual({ size: 12.5, lines: 1, cut: false });
  });
});
