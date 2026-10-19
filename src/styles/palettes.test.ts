import { describe, expect, it } from "vitest";
import tokens from "./tokens.css?raw";

// The branch colours (--b1 … --b12) in tokens.css come in three palettes (muted = no data-palette, light, bright), each
// for the light and the dark theme. A branch colour is a stripe, a dot, a card behind a letter: it has to show on the
// card and the canvas, and its ink (--bN-ink) has to be readable on it.

type Theme = "light" | "dark";
type Palette = "muted" | "light" | "bright";

interface Rule {
  attrs: Record<string, string>;
  vars: Record<string, string>;
  order: number;
}

/** The rules whose selector is `:root` with attribute tests (`:root[data-theme="dark"][data-palette="light"]`); the
 *  `.pal-*` selectors beside them (a palette shown on its own) share the same declarations. */
function parse(css: string): Rule[] {
  const rules: Rule[] = [];
  const clean = css.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const [, selectors, body] of clean.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    const vars: Record<string, string> = {};
    for (const [, name, value] of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) vars[name] = value.trim();
    for (const selector of selectors.split(",")) {
      const sel = selector.trim();
      if (!/^:root(\[[^\]]+\])*$/.test(sel)) continue;
      const attrs: Record<string, string> = {};
      for (const [, name, value] of sel.matchAll(/\[([\w-]+)="([^"]*)"\]/g)) attrs[name] = value;
      rules.push({ attrs, vars, order: rules.length });
    }
  }
  return rules;
}

/** What the browser would compute on <html data-theme=… data-palette=…>: the more specific rule wins, then the later. */
function computed(rules: Rule[], theme: Theme, palette: Palette): Record<string, string> {
  const state: Record<string, string> = { "data-theme": theme };
  if (palette !== "muted") state["data-palette"] = palette;
  const matching = rules
    .filter((r) => Object.entries(r.attrs).every(([name, value]) => state[name] === value))
    .sort((a, b) => Object.keys(a.attrs).length - Object.keys(b.attrs).length || a.order - b.order);
  const out: Record<string, string> = {};
  for (const r of matching) Object.assign(out, r.vars);
  return out;
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** `amount` of `a` over `b` (sRGB; the app mixes in oklab, which comes out about the same for this check). */
function mix(a: string, b: string, amount: number): string {
  const channel = (i: number) => Math.round(parseInt(a.slice(i, i + 2), 16) * amount + parseInt(b.slice(i, i + 2), 16) * (1 - amount));
  return `#${[1, 3, 5].map((i) => channel(i).toString(16).padStart(2, "0")).join("")}`;
}

function contrast(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

const rules = parse(tokens);
const themes: Theme[] = ["light", "dark"];
const palettes: Palette[] = ["muted", "light", "bright"];
const numbers = Array.from({ length: 12 }, (_, i) => i + 1);

describe("branch colour palettes", () => {
  it("reads the three palettes of both themes", () => {
    const sample = computed(rules, "dark", "bright");
    expect(sample["--b1"]).toMatch(/^#[0-9a-f]{6}$/);
    expect(sample["--card"]).toBe("#24221e");
    // Each palette is its own set of colours.
    const sets = new Set(themes.flatMap((t) => palettes.map((p) => numbers.map((n) => computed(rules, t, p)[`--b${n}`]).join())));
    expect(sets.size).toBe(6);
  });

  for (const theme of themes) {
    for (const palette of palettes) {
      describe(`${palette} on the ${theme} theme`, () => {
        const vars = computed(rules, theme, palette);
        const surfaces = [vars["--card"], vars["--canvas"]];

        it("defines 12 colours, each with an ink", () => {
          for (const n of numbers) {
            expect(vars[`--b${n}`], `--b${n}`).toMatch(/^#[0-9a-f]{6}$/);
            expect(vars[`--b${n}-ink`], `--b${n}-ink`).toMatch(/^#[0-9a-f]{6}$/);
          }
        });

        it("keeps every ink readable on its colour (4.5 : 1)", () => {
          for (const n of numbers) {
            expect(contrast(vars[`--b${n}`], vars[`--b${n}-ink`]), `--b${n} ${vars[`--b${n}`]} with ${vars[`--b${n}-ink`]}`).toBeGreaterThanOrEqual(4.5);
          }
        });

        it("keeps the initials on an avatar readable (the branch colour at 30 % on --surface, text 4.5 : 1)", () => {
          for (const n of numbers) {
            expect(contrast(mix(vars[`--b${n}`], vars["--surface"], 0.3), vars["--text"]), `--b${n} ${vars[`--b${n}`]}`).toBeGreaterThanOrEqual(4.5);
          }
        });

        it("keeps every colour visible on the card and the canvas (3 : 1)", () => {
          for (const n of numbers) {
            for (const surface of surfaces) {
              expect(contrast(vars[`--b${n}`], surface), `--b${n} ${vars[`--b${n}`]} on ${surface}`).toBeGreaterThanOrEqual(3);
            }
          }
        });
      });
    }
  }
});
