// Pure helpers of „Do uzupełnienia”: the chip texts, the „Pokaż więcej” slicing and the toasts.

/** How many items of a section are shown at first, and added by one „Pokaż więcej”. */
export const PAGE = 60;

export interface Chip {
  text: string;
  /** Drawn in the warning colour. */
  warn?: boolean;
  title?: string;
}

/** The chips under an unfinished person: not in the family yet, and where the record came from. */
export function personChips(p: { linked: boolean; batch: string | null }): Chip[] {
  const chips: Chip[] = [];
  if (!p.linked) chips.push({ text: "bez powiązań — nie ma jej w drzewie", warn: true, title: "Dodaj relację na profilu, w sekcji Rodzina." });
  chips.push({ text: p.batch ? `z importu „${p.batch}”` : "dodana ręcznie" });
  return chips;
}

/** The first `limit` items, and how many are left behind „Pokaż więcej”. */
export function visible<T>(list: T[], limit: number): { items: T[]; more: number } {
  return { items: list.slice(0, limit), more: Math.max(0, list.length - limit) };
}

/** „Osoby (3)”; the count is left out when there is nothing. */
export function sectionLabel(label: string, n: number): string {
  return n > 0 ? `${label} (${n})` : label;
}

export const toastDone = (name: string) => `Gotowe: ${name}.`;
export const TOAST_IGNORED = "Pominięto — znajdziesz to w „Pominięte”.";
export const TOAST_RESTORED = "Przywrócono.";

/** The profile note: where the person comes from, and that they are not in the family yet. */
export function noteText(todo: { linked: boolean; batch: string | null }): string {
  const origin = todo.batch ? `Ta osoba pochodzi z importu „${todo.batch}” i czeka na uzupełnienie.` : "Oznaczono ją do dokończenia.";
  return todo.linked ? origin : `${origin} Nie jest jeszcze połączona z rodziną, więc nie widać jej w drzewie.`;
}
