// The four looks of a person zoomed far out (mini and dot levels of the focus views, and the low zoom of Całe drzewo;
// design/pomniejszone-drzewo-makiety.html). Pure geometry and colours in screen pixels, so the DOM cards of the focus
// views and the PixiJS canvas of Całe drzewo draw the same thing: colours are plain strings (`var(--b3)` for the DOM,
// "#rrggbb" for a canvas), nothing here touches CSS or the document.

export type ZoomLook = "shapes" | "outline" | "tiles" | "cards";

export const ZOOM_LOOKS: readonly ZoomLook[] = ["shapes", "outline", "tiles", "cards"];
export const DEFAULT_ZOOM_LOOK: ZoomLook = "outline";

/** The look stored in the archive's display settings (`display.treeZoomLook`); missing or unknown means the default. */
export function zoomLookOf(value: unknown): ZoomLook {
  return ZOOM_LOOKS.includes(value as ZoomLook) ? (value as ZoomLook) : DEFAULT_ZOOM_LOOK;
}

/** The theme's colours the looks use. `neutral` stands in for a branch colour when the tree isn't coloured. */
export interface LookInk {
  canvas: string;
  surface: string;
  line: string;
  muted: string;
  accent: string;
  neutral: string;
}

/** The same, as CSS variables of the app (they follow the theme and the palette by themselves). */
export const CSS_INK: LookInk = { canvas: "var(--canvas)", surface: "var(--card)", line: "var(--line)", muted: "var(--text2)", accent: "var(--accent)", neutral: "var(--line)" };

export interface Paint {
  color: string;
  alpha: number;
}

export type MarkShape = "circle" | "square" | "diamond" | "card";

export interface Mark {
  shape: MarkShape;
  /** Size in screen px, centred on the person's box. A diamond is a square of this size turned by 45° about its centre. */
  w: number;
  h: number;
  /** Corner radius (a circle's is w / 2). */
  radius: number;
  /** Border width, inside the box. */
  strokeWidth: number;
  stroke: Paint | null;
  fill: Paint | null;
  /** Opaque colour under a see-through fill, so a line running under the mark never shows through. */
  base: string | null;
  /** The branch-colour stripe along the top of a card. */
  stripe: { height: number; color: string } | null;
  /** The grey bar standing for the text of a card. */
  bar: { x: number; y: number; w: number; h: number; paint: Paint } | null;
  /** The accent ring of the selected person: a box around the mark, border inside it. */
  ring: { w: number; h: number; radius: number; width: number; color: string };
}

/** The mark's base size by zoom: the height of a person's box, but never above 16 px or below 7 px. */
export function markUnit(zoom: number): number {
  return Math.min(16, Math.max(7, 72 * zoom * 0.8));
}

/** The base size the canvas textures of the marks are drawn at; a sprite is scaled to the size of the zoom. */
export const MARK_REF_UNIT = 16;

/** The scale of a mark texture (drawn at `MARK_REF_UNIT`) for a world drawn at `zoom`: the mark's screen size, undone by 1 / zoom. */
export function markScale(zoom: number): number {
  return markUnit(zoom) / MARK_REF_UNIT / zoom;
}

/** How many textures of a look differ by the person's branch: A and B tell branches apart by shape, C and D don't. */
export function markVariants(look: ZoomLook): number {
  return look === "shapes" || look === "outline" ? 3 : 1;
}

/** Which of the look's `markVariants` textures belongs to a branch. */
export function markVariant(look: ZoomLook, branch: number): number {
  return markVariants(look) === 1 ? 0 : ((branch % 3) + 3) % 3;
}

const SHAPES: MarkShape[] = ["circle", "square", "diamond"];

/** The shape that tells branches apart without colour (A and B): by branch colour index, circle if there is none. */
export function branchShape(branch: number | null): MarkShape {
  return SHAPES[(branch ?? 3) % 3];
}

/**
 * The mark of one person. `color` is the branch colour (null: the tree isn't coloured), `branch` its index (for the
 * shape), `u` the base size from `markUnit`. The proportions are the mockup's at u = 16.
 */
export function markFor(look: ZoomLook, color: string | null, branch: number | null, u: number, ink: LookInk): Mark {
  const col = color ?? ink.neutral;
  const accent = (w: number, h: number, radius: number, pad: number) => ({ w: w + 2 * pad, h: h + 2 * pad, radius, width: 2.4, color: ink.accent });
  const pad = Math.max(2.5, u * 0.31);
  if (look === "shapes" || look === "outline") {
    const shape = branchShape(branch);
    const filled = look === "shapes";
    const strokeWidth = filled ? 0 : Math.max(1.5, u * 0.2);
    // A diamond reaches further than a square of the same size, so a line meeting it still touches: its outer corners
    // lie 1.5 px (at u = 16) beyond the circle that touches the other shapes.
    const side = shape === "diamond" ? (u + u * 0.1875 * 2) / Math.SQRT2 : u;
    const radius = shape === "circle" ? side / 2 : shape === "diamond" ? Math.max(0.5, u * 0.06) : Math.max(1, u * (filled ? 0.125 : 0.28));
    return {
      shape,
      w: side,
      h: side,
      radius,
      strokeWidth,
      stroke: filled ? null : { color: col, alpha: 1 },
      fill: filled ? { color: col, alpha: 1 } : { color: ink.canvas, alpha: 1 },
      base: null,
      stripe: null,
      bar: null,
      ring: accent(u, u, (u + 2 * pad) / 2, pad),
    };
  }
  if (look === "tiles") {
    return {
      shape: "square",
      w: u,
      h: u,
      radius: Math.max(1.5, u * 0.31),
      strokeWidth: Math.max(1, u * 0.125),
      stroke: { color: col, alpha: 1 },
      fill: { color: col, alpha: 0.22 },
      base: ink.canvas,
      stripe: null,
      bar: null,
      ring: accent(u, u, Math.max(3, u * 0.5), Math.max(2.5, u * 0.25)),
    };
  }
  const w = u * 1.875;
  const h = u * 0.875;
  const stripe = Math.max(2, u * 0.28);
  return {
    shape: "card",
    w,
    h,
    radius: Math.max(1.5, u * 0.22),
    strokeWidth: Math.max(1, u * 0.1),
    stroke: { color: ink.line, alpha: 0.55 },
    fill: { color: ink.surface, alpha: 1 },
    base: null,
    stripe: { height: stripe, color: col },
    bar: u >= 9 ? { x: u * 0.31, y: h / 2 + 0.5, w: w - u * 0.75, h: Math.max(1, u * 0.14), paint: { color: ink.muted, alpha: 0.45 } } : null,
    ring: accent(w, h, Math.max(3, u * 0.375), Math.max(2.5, u * 0.22)),
  };
}

/** A paint as a CSS colour: `color-mix` with transparent for a see-through one. */
export function paintCss(p: Paint): string {
  return p.alpha >= 1 ? p.color : `color-mix(in srgb, ${p.color} ${Math.round(p.alpha * 100)}%, transparent)`;
}

/** The fill as one opaque CSS colour when the mark has a base under it (a tile's soft fill over the canvas). */
export function fillCss(m: Mark): string | undefined {
  if (!m.fill) return undefined;
  return m.base && m.fill.alpha < 1 ? `color-mix(in srgb, ${m.fill.color} ${Math.round(m.fill.alpha * 100)}%, ${m.base})` : paintCss(m.fill);
}
