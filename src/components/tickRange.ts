/** A list of checkboxes where a click or Ctrl+click toggles one row and Shift+click ticks the whole range from the last
 *  clicked row (like a file list). `ids` are the rows in order, `ticked` the ticked ones, `last` the index of the row
 *  clicked before (null at first). Returns the ticked ids, in the order of the rows. */
export function clickRow(ids: readonly string[], ticked: readonly string[], index: number, last: number | null, shift: boolean): string[] {
  const on = new Set(ticked);
  if (shift && last != null && last >= 0 && last < ids.length) {
    const [from, to] = last < index ? [last, index] : [index, last];
    for (let i = from; i <= to; i++) on.add(ids[i]);
  } else if (!on.delete(ids[index])) {
    on.add(ids[index]);
  }
  return ids.filter((id) => on.has(id));
}
