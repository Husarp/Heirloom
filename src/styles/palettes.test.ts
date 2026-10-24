import { describe, expect, it } from "vitest";
import { ACCENTS, SCHEMES } from "../app/colorLooks";
import schemes from "./schemes.css?raw";
import tokens from "./tokens.css?raw";

// The branch colours (--b1 … --b12) in tokens.css come in three palettes (muted = no data-palette, light, bright), each
// for the light and the dark theme. A branch colour is a stripe, a dot, a card behind a letter: it has to show on the
// card and the canvas, and its ink (--bN-ink) has to be readable on it. The same on every colour scheme (schemes.css:
// the canvas and the card change with it), and the text and accent tokens have to be readable on every scheme, with
// every accent, in both themes.

type Theme = "light" | "dark";
type Palette = "muted" | "light" | "bright";
interface Look {
  theme: Theme;
  palette?: Palette;
  scheme?: string;
  accent?: string;
}

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

/** What the browser would compute on <html data-theme=… data-palette=… data-scheme=… data-accent=…> (the default palette,
 *  scheme and accent are no attribute): the more specific rule wins, then the later. */
function computed(rules: Rule[], { theme, palette = "muted", scheme = "museum", accent = "default" }: Look): Record<string, string> {
  const state: Record<string, string> = { "data-theme": theme };
  if (palette !== "muted") state["data-palette"] = palette;
  if (scheme !== "museum") state["data-scheme"] = scheme;
  if (accent !== "default") state["data-accent"] = accent;
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

const rules = parse(tokens + "\n" + schemes);
const themes: Theme[] = ["light", "dark"];
const palettes: Palette[] = ["muted", "light", "bright"];
const schemeIds = SCHEMES.map((s) => s.value);
const accentIds = ACCENTS.map((a) => a.value);
const numbers = Array.from({ length: 12 }, (_, i) => i + 1);

/** Every colour token of a scheme (the rest of tokens.css — fonts, radii, shadows — is the same for all). */
const SCHEME_TOKENS = [
  "bg", "surface", "surface2", "border", "text", "text2", "text3", "accent", "accent-ink", "accent-soft", "accent-text", "accent-hover",
  "sidebar", "sidebar-text", "sidebar-muted", "sidebar-border", "nav-active-bg", "nav-active-text", "canvas", "grid", "line", "card", "card-border",
  "pop-border", "err", "err-soft", "warn", "warn-soft", "toast-bg", "toast-text", "toast-icon", "toast-action", "toast-err",
].map((name) => `--${name}`);
/** What an accent replaces. */
const ACCENT_TOKENS = ["accent", "accent-ink", "accent-soft", "accent-text", "accent-hover", "nav-active-bg", "nav-active-text", "toast-icon", "toast-action"].map((name) => `--${name}`);

/** The rule of one data-scheme / data-accent value (and theme), as written in the CSS. */
function declared(attr: "data-scheme" | "data-accent", value: string, theme: Theme): Record<string, string> | undefined {
  const dark = theme === "dark";
  return rules.find((r) => r.attrs[attr] === value && (r.attrs["data-theme"] === "dark") === dark && Object.keys(r.attrs).length === (dark ? 2 : 1))?.vars;
}

describe("branch colour palettes", () => {
  it("reads the three palettes of both themes", () => {
    const sample = computed(rules, { theme: "dark", palette: "bright" });
    expect(sample["--b1"]).toMatch(/^#[0-9a-f]{6}$/);
    expect(sample["--card"]).toBe("#24221e");
    // Each palette is its own set of colours.
    const sets = new Set(themes.flatMap((t) => palettes.map((p) => numbers.map((n) => computed(rules, { theme: t, palette: p })[`--b${n}`]).join())));
    expect(sets.size).toBe(6);
  });

  for (const scheme of schemeIds) {
    for (const theme of themes) {
      for (const palette of palettes) {
        describe(`${palette} on the ${theme} theme, scheme ${scheme}`, () => {
          const vars = computed(rules, { theme, palette, scheme });
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
  }
});

describe("colour schemes and accent colours", () => {
  it("has a rule for every scheme and accent the settings offer, and none the settings don't", () => {
    for (const theme of themes) {
      for (const scheme of schemeIds.filter((s) => s !== "museum")) expect(declared("data-scheme", scheme, theme), `${scheme} ${theme}`).toBeDefined();
      for (const accent of accentIds.filter((a) => a !== "default")) expect(declared("data-accent", accent, theme), `${accent} ${theme}`).toBeDefined();
    }
    const inCss = (attr: string) => new Set(rules.map((r) => r.attrs[attr]).filter(Boolean));
    expect([...inCss("data-scheme")].sort()).toEqual(schemeIds.filter((s) => s !== "museum").sort());
    expect([...inCss("data-accent")].sort()).toEqual(accentIds.filter((a) => a !== "default").sort());
  });

  it("gives every scheme and every accent the whole set of tokens, in both themes", () => {
    for (const theme of themes) {
      for (const scheme of schemeIds.filter((s) => s !== "museum")) {
        expect(Object.keys(declared("data-scheme", scheme, theme)!).sort(), `${scheme} ${theme}`).toEqual([...SCHEME_TOKENS].sort());
      }
      for (const accent of accentIds.filter((a) => a !== "default")) {
        expect(Object.keys(declared("data-accent", accent, theme)!).sort(), `${accent} ${theme}`).toEqual([...ACCENT_TOKENS].sort());
      }
    }
  });

  it("keeps „Muzeum” and the default accent as they were", () => {
    const light = computed(rules, { theme: "light" });
    const dark = computed(rules, { theme: "dark" });
    expect([light["--bg"], light["--surface"], light["--sidebar"], light["--canvas"], light["--accent"], light["--accent-hover"], light["--toast-bg"]]).toEqual([
      "#f7f4ee", "#ffffff", "#efeae1", "#f2eee7", "#2f5d50", "#1f3f36", "#24211d",
    ]);
    expect([dark["--bg"], dark["--surface"], dark["--sidebar"], dark["--canvas"], dark["--accent"], dark["--accent-hover"], dark["--toast-bg"]]).toEqual([
      "#1b1a17", "#24221e", "#171613", "#1f1e1a", "#7fb8a4", "#b3dccd", "#ede7dd",
    ]);
    // Zieleń is the same green on any scheme.
    expect(computed(rules, { theme: "light", accent: "green" })["--accent"]).toBe("#2f5d50");
    expect(computed(rules, { theme: "dark", accent: "green" })["--accent"]).toBe("#7fb8a4");
  });

  it("applies a scheme in both themes and lets an accent replace the scheme's own", () => {
    for (const theme of themes) {
      const own = computed(rules, { theme, scheme: "poranek" });
      const navy = computed(rules, { theme, scheme: "poranek", accent: "navy" });
      expect(own["--accent"]).not.toBe(computed(rules, { theme })["--accent"]);
      expect(navy["--accent"]).toBe(declared("data-accent", "navy", theme)!["--accent"]);
      expect(navy["--accent-soft"]).not.toBe(own["--accent-soft"]);
      // The rest of the scheme stays.
      expect([navy["--bg"], navy["--sidebar"], navy["--canvas"]]).toEqual([own["--bg"], own["--sidebar"], own["--canvas"]]);
    }
  });

  it("makes the schemes look different (page, sidebar, canvas and accent) in each theme", () => {
    for (const theme of themes) {
      for (const token of ["--bg", "--sidebar", "--canvas", "--accent"]) {
        const values = schemeIds.map((scheme) => computed(rules, { theme, scheme })[token]);
        expect(new Set(values).size, `${token} ${theme}`).toBe(schemeIds.length);
      }
    }
  });

  for (const scheme of schemeIds) {
    for (const accent of accentIds) {
      for (const theme of themes) {
        it(`keeps the text readable (WCAG AA): scheme ${scheme}, accent ${accent}, ${theme} theme`, () => {
          const v = computed(rules, { theme, scheme, accent });
          for (const token of SCHEME_TOKENS) expect(v[token], token).toMatch(/^#[0-9a-f]{6}$/);
          const check = (fg: string, bg: string, min: number) =>
            expect(contrast(v[`--${fg}`], v[`--${bg}`]), `${fg} ${v[`--${fg}`]} on ${bg} ${v[`--${bg}`]}`).toBeGreaterThanOrEqual(min);

          // Text on the page, the cards and lists, the secondary surface and the canvas.
          for (const text of ["text", "text2", "text3"]) for (const on of ["bg", "surface", "surface2", "canvas"]) check(text, on, 4.5);
          // The sidebar, and the item of the open screen in it.
          for (const text of ["sidebar-text", "sidebar-muted"]) check(text, "sidebar", 4.5);
          check("nav-active-text", "nav-active-bg", 4.5);
          // The accent: as text (links, chips, the chosen option), as a fill with its ink, as a control edge.
          for (const on of ["bg", "surface", "accent-soft"]) check("accent-text", on, 4.5);
          for (const on of ["bg", "surface"]) check("accent-hover", on, 4.5);
          check("accent-ink", "accent", 4.5);
          for (const on of ["bg", "surface"]) check("accent", on, 3);
          // Messages and the toast.
          for (const on of ["bg", "surface", "err-soft"]) check("err", on, 4.5);
          for (const on of ["bg", "surface", "warn-soft"]) check("warn", on, 4.5);
          for (const token of ["toast-text", "toast-icon", "toast-action", "toast-err"]) check(token, "toast-bg", 4.5);
          // The line (no branch colour, control edges).
          for (const on of ["bg", "surface", "canvas"]) check("line", on, 3);
        });
      }
    }
  }
});
