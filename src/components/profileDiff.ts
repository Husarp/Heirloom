/** What a „Zdjęcie profilowe dla:” list changes: `profileOf` are those who have the photo as their profile photo now,
 *  `ticked` those left ticked. Newly ticked people get it (`set`), unticked ones lose it (`unset`). */
export function profileChange(profileOf: readonly string[], ticked: readonly string[]): { set: string[]; unset: string[] } {
  const had = new Set(profileOf);
  const want = new Set(ticked);
  return { set: ticked.filter((id) => !had.has(id)), unset: profileOf.filter((id) => !want.has(id)) };
}
