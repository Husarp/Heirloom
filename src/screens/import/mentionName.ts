// Import · 2 Sprawdź: the name typed for a mention of someone the batch doesn't have („Antoniego Maja” → imię Antoniego,
// nazwisko Maja). It is only a start: mentions are often inflected, so the user corrects it before adding.

/** The visible text of a mention split at its first word: the first word is the imię, the rest the nazwisko. */
export function splitMentionName(visible: string | null | undefined): { given: string; surname: string } {
  const words = (visible ?? "").trim().split(/\s+/).filter(Boolean);
  return { given: words[0] ?? "", surname: words.slice(1).join(" ") };
}
