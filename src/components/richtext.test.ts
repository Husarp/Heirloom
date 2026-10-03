import { describe, expect, it } from "vitest";
import { docToMarkdown, markdownToDoc } from "./richtext";

describe("Markdown ↔ editor document", () => {
  const cases = [
    "Józef urodził się w Wólce jako syn [Antoniego](person:@I2@) i **Agnieszki**.",
    "### Na kolei\n\nPracował jako *zwrotniczy*.",
    "- Lublin\n- Łęczna",
    "1. pierwszy\n2. drugi",
    "> Pociąg nie czeka.",
    "Link: [Geneteka](https://geneteka.genealodzy.pl)",
    "Pierwsza linia\ndruga linia",
  ];
  for (const text of cases) {
    it(`round-trips: ${text.slice(0, 30)}`, () => {
      expect(docToMarkdown(markdownToDoc(text))).toBe(text);
    });
  }

  it("keeps mentions as nodes", () => {
    const doc = markdownToDoc("Syn [Antoniego](person:@I2@).");
    expect(doc.content?.[0].content?.[1]).toEqual({ type: "mention", attrs: { id: "@I2@", label: "Antoniego" } });
  });
});
