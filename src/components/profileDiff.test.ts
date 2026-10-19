import { describe, expect, it } from "vitest";
import { profileChange } from "./profileDiff";

describe("profileChange", () => {
  it("newly ticked get the photo, unticked lose it, the rest is untouched", () => {
    expect(profileChange(["a", "b"], ["b", "c"])).toEqual({ set: ["c"], unset: ["a"] });
  });
  it("nothing changed gives nothing to do", () => {
    expect(profileChange(["a"], ["a"])).toEqual({ set: [], unset: [] });
    expect(profileChange([], [])).toEqual({ set: [], unset: [] });
  });
  it("unticking everyone only unsets, ticking from empty only sets", () => {
    expect(profileChange(["a", "b"], [])).toEqual({ set: [], unset: ["a", "b"] });
    expect(profileChange([], ["a", "b"])).toEqual({ set: ["a", "b"], unset: [] });
  });
});
