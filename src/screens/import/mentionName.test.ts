import { describe, expect, it } from "vitest";
import { splitMentionName } from "./mentionName";

describe("the name prefilled for an unknown mention", () => {
  it("takes the first word as imię and the rest as nazwisko", () => {
    expect(splitMentionName("Antoniego Maja")).toEqual({ given: "Antoniego", surname: "Maja" });
    expect(splitMentionName("  Jan   Kowalski-Nowak  ")).toEqual({ given: "Jan", surname: "Kowalski-Nowak" });
    expect(splitMentionName("Maria Anna z Kowalskich")).toEqual({ given: "Maria", surname: "Anna z Kowalskich" });
  });

  it("leaves the nazwisko empty for one word and both empty for nothing", () => {
    expect(splitMentionName("Zosię")).toEqual({ given: "Zosię", surname: "" });
    expect(splitMentionName("   ")).toEqual({ given: "", surname: "" });
    expect(splitMentionName(null)).toEqual({ given: "", surname: "" });
    expect(splitMentionName(undefined)).toEqual({ given: "", surname: "" });
  });
});
