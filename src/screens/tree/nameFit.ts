// A name on a tree card (0.4.2): a long one gets a smaller font, step by step down to a size still easy to read; one
// still too long goes onto two lines, and only then is it cut, with the whole name in the tooltip. The cards keep
// their size, so the layout never jumps.

export interface NameFit {
  size: number;
  lines: 1 | 2;
  /** Doesn't fit even so: cut with „…”, the whole name in the tooltip. */
  cut: boolean;
}

const STEP = 0.5;
/** Smaller than this, a name reads better on two lines (given names, then the surname). */
const MIN_ONE = 13;
const MAX_TWO = 13;
const MIN = 11.5;

/** `width(text, size)`: how wide the text is in px at that font size; `room`: the card's width for the name. */
export function fitName(text: string, room: number, max: number, width: (text: string, size: number) => number): NameFit {
  // Text gets about as much narrower as its font, so the first try is close (and is checked like the others).
  const guess = Math.floor(((max * room) / Math.max(width(text, max), 1)) / STEP) * STEP;
  for (let size = Math.min(max, Math.max(guess, MIN_ONE)); size >= MIN_ONE; size -= STEP) {
    if (width(text, size) <= room) return { size, lines: 1, cut: false };
  }
  const words = text.split(" ").filter(Boolean);
  if (words.length < 2) {
    for (let size = MIN_ONE - STEP; size >= MIN; size -= STEP) if (width(text, size) <= room) return { size, lines: 1, cut: false };
    return { size: MIN, lines: 1, cut: true };
  }
  for (let size = Math.min(max, MAX_TWO); size >= MIN; size -= STEP) {
    if (lineCount(words, room, size, width) <= 2) return { size, lines: 2, cut: false };
  }
  return { size: MIN, lines: 2, cut: true };
}

/** How many lines the words take when each line holds as many as fit (as the browser wraps them). */
function lineCount(words: string[], room: number, size: number, width: (text: string, size: number) => number): number {
  let lines = 1;
  let line = "";
  for (const word of words) {
    if (width(word, size) > room) return Infinity;
    const longer = line ? `${line} ${word}` : word;
    if (line && width(longer, size) > room) {
      lines++;
      line = word;
    } else line = longer;
  }
  return lines;
}

let probe: HTMLSpanElement | null = null;
const widths = new Map<string, number>();

/** The width of a card name in the cards' own font (`.card-name`), measured once per text and size. */
export function nameWidth(text: string, size: number): number {
  const key = `${size}|${text}`;
  const known = widths.get(key);
  if (known != null) return known;
  if (!probe) {
    probe = document.createElement("span");
    probe.setAttribute("aria-hidden", "true");
    probe.style.cssText = "position:absolute;left:-10000px;top:0;visibility:hidden;white-space:pre;font-family:var(--f-serif);font-weight:var(--name-w);line-height:1.2";
    document.body.appendChild(probe);
  }
  probe.style.fontSize = `${size}px`;
  probe.textContent = text;
  const w = probe.getBoundingClientRect().width / (Number(document.documentElement.style.zoom) || 1);
  widths.set(key, w);
  return w;
}

/** Once the font has loaded, the widths measured with the fallback font are wrong. */
export function forgetNameWidths(): void {
  widths.clear();
}
