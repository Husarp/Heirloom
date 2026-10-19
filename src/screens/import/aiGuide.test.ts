import { describe, expect, it } from "vitest";
import instructions from "../../../docs/AI_INSTRUCTIONS.md?raw";
import { GUIDE_STEPS, guideOpenByDefault } from "./aiGuide";
import wizard from "./ImportWizard.tsx?raw";
import load from "./StepLoad.tsx?raw";

const guide = GUIDE_STEPS.map((s) => `${s.title} ${s.text}`).join("\n");

describe("the AI guide in Import · Wczytaj", () => {
  it("names the buttons and fields the screen really has", () => {
    for (const label of ["Kopiuj instrukcję dla AI", "Wklej odpowiedź AI", "Upuść wszystko naraz"]) {
      expect(load).toContain(label);
    }
    expect(guide).toContain("„Wklej odpowiedź AI”");
    expect(guide).toContain("„Upuść wszystko naraz”");
    expect(GUIDE_STEPS.filter((s) => s.copy)).toHaveLength(1);
  });

  it("names the wizard's steps as the wizard does", () => {
    for (const step of ["Sprawdź", "Dopasuj osoby", "Zdjęcia i pliki", "Podsumowanie"]) {
      expect(wizard).toContain(`"${step}"`);
      expect(guide).toContain(step);
    }
  });

  it("says what the AI instructions say", () => {
    for (const phrase of ["Pliki do Tadeusza", "„gotowe”", "„tak”", "ponumerowane pytania", "czesc-1.json", "M001", "dalej", "Część B"]) {
      expect(instructions).toContain(phrase);
      expect(guide).toContain(phrase);
    }
    // „ZRÓB PLIK” is no longer the trigger, only kept working for people used to it.
    expect(guide).not.toContain("ZRÓB PLIK");
    expect(instructions).toContain("ZRÓB PLIK");
  });

  it("is open at first only while there were no imports", () => {
    expect(guideOpenByDefault(undefined)).toBe(false);
    expect(guideOpenByDefault(null)).toBe(false);
    expect(guideOpenByDefault([])).toBe(true);
    expect(guideOpenByDefault([{}])).toBe(false);
  });
});
