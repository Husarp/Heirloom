/** Which names to offer as quick picks: the archive's editors first, then the names used on this computer. */
export function nameChoices(archiveEditors: string[], computerNames: string[], exclude: string, limit: number): string[] {
  const skip = exclude.trim().toLowerCase();
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of [...archiveEditors, ...computerNames]) {
    const name = raw.trim();
    const key = name.toLowerCase();
    if (!name || key === skip || seen.has(key)) continue;
    seen.add(key);
    out.push(name);
    if (out.length >= limit) break;
  }
  return out;
}

/** Names used on this computer that the archive's own list doesn't have yet. */
export function computerOnlyNames(archiveEditors: string[], computerNames: string[]): string[] {
  const have = new Set(archiveEditors.map((n) => n.trim().toLowerCase()));
  return nameChoices([], computerNames, "", Infinity).filter((n) => !have.has(n.toLowerCase()));
}
