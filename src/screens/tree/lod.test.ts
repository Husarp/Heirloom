import { describe, expect, it } from "vitest";
import { compactTextScale, nextLod, type Lod } from "./lod";

describe("levels of detail", () => {
  it("keeps full cards down to 75 %, compact ones down to 30 %, blocks down to 15 %, dots below", () => {
    expect(nextLod(1, 0.76)).toBe(1);
    expect(nextLod(1, 0.74)).toBe(2);
    expect(nextLod(2, 0.31)).toBe(2);
    expect(nextLod(2, 0.29)).toBe(3);
    expect(nextLod(3, 0.16)).toBe(3);
    expect(nextLod(3, 0.14)).toBe(4);
    expect(nextLod(1, 0.08)).toBe(4);
  });

  it("goes back up a few points higher (80 / 33 / 18 %)", () => {
    expect(nextLod(2, 0.79)).toBe(2);
    expect(nextLod(2, 0.8)).toBe(1);
    expect(nextLod(3, 0.32)).toBe(3);
    expect(nextLod(3, 0.33)).toBe(2);
    expect(nextLod(4, 0.17)).toBe(4);
    expect(nextLod(4, 0.18)).toBe(3);
    expect(nextLod(4, 1)).toBe(1);
  });

  it("never flickers: a zoom held inside a band keeps the level it came with", () => {
    for (const lod of [2, 3] as Lod[]) expect(nextLod(lod, 0.3)).toBe(lod);
  });

  it("keeps the compact text at its screen size down to 50 %, then shrinks it to 9 px at 30 %", () => {
    expect(compactTextScale(1)).toBe(1);
    expect(compactTextScale(0.5)).toBe(1);
    expect(compactTextScale(0.4) * 13).toBeCloseTo(11);
    expect(compactTextScale(0.3) * 13).toBeCloseTo(9);
    expect(compactTextScale(0.2) * 13).toBeCloseTo(9);
  });
});
