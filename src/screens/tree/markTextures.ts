// The textures of the zoomed-out looks (zoomLook.ts) for the PixiJS canvas of „Całe drzewo”. A person is two sprites:
// `under` (the same for everyone: the canvas under an outline, a mini-card's body) and `tint` (white, tinted with the
// branch colour). Made once per look and theme at MARK_REF_UNIT; sprites are scaled to the zoom (markScale).

import { Graphics, Rectangle, type Renderer, type Texture } from "pixi.js";
import { MARK_REF_UNIT, markFor, markVariants, type LookInk, type Mark, type ZoomLook } from "./zoomLook";

export interface MarkTextures {
  /** Per `markVariant`; null where the look has nothing under the tint. */
  under: (Texture | null)[];
  tint: Texture[];
}

const WHITE = "#ffffff";

/** The mark's outline path, its edge moved `inset` px inwards. W × H is the texture (a diamond's is its diagonal). */
function outline(g: Graphics, m: Mark, W: number, H: number, inset: number): Graphics {
  if (m.shape === "circle") return g.circle(W / 2, H / 2, Math.max(0, m.w / 2 - inset));
  if (m.shape === "diamond") {
    const d = Math.max(0, W / 2 - inset * Math.SQRT2);
    return g.poly([W / 2, H / 2 - d, W / 2 + d, H / 2, W / 2, H / 2 + d, W / 2 - d, H / 2]);
  }
  return g.roundRect(inset, inset, m.w - 2 * inset, m.h - 2 * inset, Math.max(0, m.radius - inset));
}

/** The branch-colour stripe along the top of a mini-card, inside its border and rounded with it. */
function stripe(g: Graphics, m: Mark): Graphics {
  const x0 = m.strokeWidth;
  const x1 = m.w - m.strokeWidth;
  const y0 = m.strokeWidth;
  const y1 = y0 + m.stripe!.height;
  const r = Math.min(Math.max(0, m.radius - m.strokeWidth), y1 - y0, (x1 - x0) / 2);
  if (r < 0.1) return g.rect(x0, y0, x1 - x0, y1 - y0);
  return g.moveTo(x0, y1).lineTo(x0, y0 + r).arcTo(x0, y0, x0 + r, y0, r).lineTo(x1 - r, y0).arcTo(x1, y0, x1, y0 + r, r).lineTo(x1, y1).closePath();
}

export function buildMarkTextures(renderer: Renderer, look: ZoomLook, ink: LookInk): MarkTextures {
  const result: MarkTextures = { under: [], tint: [] };
  for (let v = 0; v < markVariants(look); v++) {
    // Branch v picks the shape; the colour is white, for the sprite's tint to colour.
    const m = markFor(look, WHITE, v, MARK_REF_UNIT, ink);
    const W = m.shape === "diamond" ? m.w * Math.SQRT2 : m.w;
    const H = m.shape === "diamond" ? m.w * Math.SQRT2 : m.h;
    const half = m.strokeWidth / 2;
    const make = (g: Graphics) => {
      const t = renderer.generateTexture({ target: g, frame: new Rectangle(0, 0, W, H), resolution: 4, antialias: true });
      g.destroy();
      return t;
    };

    let under: Graphics | null = null;
    let tint: Graphics;
    if (look === "shapes") {
      tint = outline(new Graphics(), m, W, H, 0).fill(WHITE);
    } else if (look === "outline") {
      // The ring in the branch colour over a disc of the canvas, so a line under the mark doesn't show in the middle.
      under = outline(new Graphics(), m, W, H, half).fill(m.fill!.color);
      tint = outline(new Graphics(), m, W, H, half).stroke({ width: m.strokeWidth, color: WHITE });
    } else if (look === "tiles") {
      under = outline(new Graphics(), m, W, H, 0).fill(m.base!);
      tint = outline(new Graphics(), m, W, H, 0).fill({ color: WHITE, alpha: m.fill!.alpha });
      outline(tint, m, W, H, half).stroke({ width: m.strokeWidth, color: WHITE });
    } else {
      under = outline(new Graphics(), m, W, H, 0).fill(m.fill!);
      outline(under, m, W, H, half).stroke({ width: m.strokeWidth, color: m.stroke!.color, alpha: m.stroke!.alpha });
      if (m.bar) under.roundRect(m.bar.x, m.bar.y, m.bar.w, m.bar.h, m.bar.h / 2).fill(m.bar.paint);
      tint = stripe(new Graphics(), m).fill(WHITE);
    }
    result.under.push(under ? make(under) : null);
    result.tint.push(make(tint));
  }
  return result;
}
