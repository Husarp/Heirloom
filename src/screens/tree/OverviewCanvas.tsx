// „Całe drzewo” (spec §3.11, §4.4): everyone in generation bands, grouped into surname clusters, drawn with PixiJS so
// thousands of people pan and zoom smoothly (PLAN §5.6). Only what is on screen is drawn in detail; the canvas
// buffer is screen-sized and only grows, so resizing the window never rebuilds it (Phase 0 finding).

import { Application, BitmapFont, BitmapText, Container, Graphics, Sprite, type Renderer } from "pixi.js";
import { Minus, Plus, LocateFixed, Maximize } from "lucide-react";
import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from "react";
import type { TreeCamera } from "../../app/store";
import { cardYears, count, roman } from "../../lib/format";
import { cameraOf, viewOf } from "./camera";
import { placeLabels } from "./labels";

export interface OverviewData {
  /** [id, x, y, branch, given, surname, birth year, death year, birth uncertain, death uncertain, living, generation,
   *  surname branch, [father, mother] as indices, and in archives opened together the archives' keys] (tree.rs
   *  `overview`). */
  people: [string, number, number, number, string, string, string | null, string | null, boolean, boolean, boolean, number | null, number, [number | null, number | null], string[]?][];
  clusters: { label: string; branch: number; count: number; fromGen: number | null; toGen: number | null; x: number; width: number }[];
  bands: number;
  bandHeight: number;
  bandYears: (number | null)[];
  width: number;
}

export interface OverviewHandle {
  fit: () => void;
  centerOn: (id: string) => void;
  zoomBy: (f: number) => void;
  /** Fits one surname cluster on screen (its columns across its generation bands). */
  showCluster: (c: { x: number; width: number; fromGen: number | null; toGen: number | null }) => void;
  /** Where the camera is, in world coordinates (for Back and „Ostatnie miejsce”). */
  getView: () => TreeCamera | null;
}

const MIN_ZOOM = 0.005;
const MAX_ZOOM = 1.6;
const ORIGIN = { x: 0, y: 0 };

const CARD_W = 204;
const CARD_H = 72;
/** The grid the cards sit on (tree.rs `overview`): a click in the gap still finds the nearest card. */
const CELL_W = 224;
const CELL_H = 92;
const LEFT_COLUMN = 96;

type Cluster = OverviewData["clusters"][number];

/** A surname label's width on screen: the dot, the name (serif 14 px), the count (12 px), gaps, padding and border
 *  of `.cluster-label`. Measured once the fonts are in. */
const labelWidths = new Map<string, number>();
let measureContext: CanvasRenderingContext2D | null = null;
function labelWidth(c: Cluster): number {
  const key = `${c.label}|${c.count}`;
  const known = labelWidths.get(key);
  if (known != null) return known;
  measureContext ??= document.createElement("canvas").getContext("2d");
  const css = getComputedStyle(document.documentElement);
  let width = c.label.length * 8 + `${c.count} os.`.length * 6.6;
  if (measureContext) {
    measureContext.font = `600 14px ${css.getPropertyValue("--f-serif")}`;
    width = measureContext.measureText(c.label).width;
    measureContext.font = `12px ${css.getPropertyValue("--f-sans")}`;
    width += measureContext.measureText(`${c.count} os.`).width;
  }
  width += 8 + 6 + 6 + 18 + 2;
  if (document.fonts.status === "loaded") labelWidths.set(key, width);
  return width;
}

function cssColor(name: string): number {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const hex = value.startsWith("#") ? value.slice(1) : "9c9385";
  return parseInt(hex.length === 3 ? hex.split("").map((c) => c + c).join("") : hex, 16);
}

/** `--line` (no colour) and `--b1` … `--b12`, read from the theme in use. */
function palette(): number[] {
  return [cssColor("--line"), ...Array.from({ length: 12 }, (_, k) => cssColor(`--b${k + 1}`))];
}

interface Card {
  root: Container;
  bg: Sprite;
  stripe: Sprite;
  name: BitmapText;
  years: BitmapText;
}

// † and · for the years on cards (design v2, A9).
const CHARS = [["a", "z"], ["A", "Z"], ["0", "9"], " ąćęłńóśźżĄĆĘŁŃÓŚŹŻéüöäÉÜÖÄ–-.,()?„”'/&†·…"];

export const OverviewCanvas = forwardRef<OverviewHandle, {
  data: OverviewData;
  dark: boolean;
  hidden: boolean;
  focus: string | null;
  /** The person in the side panel, ringed like a selected card in the other views. */
  selected: string | null;
  /** The colour of a person: 0 none (`--line`), 1–12 `--b1` …; a new function re-tints everyone at once. */
  colorFor: (index: number) => number;
  onSelect: (id: string) => void;
  onOpen: (id: string) => void;
  onZoom: (zoom: number) => void;
  /** A camera to come back to (Back, „Ostatnie miejsce”); `onRestored` says it was used. */
  restore?: TreeCamera | null;
  onRestored?: () => void;
  /** The camera moved (to remember the place). */
  onCamera?: () => void;
}>(function OverviewCanvas({ data, dark, hidden, focus, selected, colorFor, onSelect, onOpen, onZoom, restore, onRestored, onCamera }, ref) {
  const host = useRef<HTMLDivElement>(null);
  const api = useRef<(Omit<OverviewHandle, "getView"> & { recolor: () => void; drawSelection: () => void; drawLine: () => void; setView: (v: TreeCamera) => void }) | null>(null);
  const [overlay, setOverlay] = useState<{ x: number; y: number; zoom: number }>({ x: 0, y: 0, zoom: 0.08 });
  const callbacks = useRef({ onSelect, onOpen, onZoom, onCamera, colorFor });
  callbacks.current = { onSelect, onOpen, onZoom, onCamera, colorFor };
  const running = useRef(true);
  running.current = !hidden;
  const labelSpots = useRef(new Map<string, number>());
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const focusRef = useRef(focus);
  focusRef.current = focus;
  // The camera outside the Pixi app, so a rebuild (the data edited, the theme changed) keeps the place.
  const kept = useRef<TreeCamera | null>(null);
  const restoreRef = useRef(restore);
  restoreRef.current = restore;
  const restored = useRef(onRestored);
  restored.current = onRestored;

  useEffect(() => api.current?.recolor(), [colorFor]);
  useEffect(() => api.current?.drawSelection(), [selected]);
  useEffect(() => api.current?.drawLine(), [focus]);
  // While the app is starting, it picks up `restore` itself.
  useEffect(() => {
    if (!restore || !api.current) return;
    api.current.setView(restore);
    restored.current?.();
  }, [restore]);

  useImperativeHandle(ref, () => ({
    fit: () => api.current?.fit(),
    centerOn: (id) => api.current?.centerOn(id),
    zoomBy: (f) => api.current?.zoomBy(f),
    showCluster: (c) => api.current?.showCluster(c),
    getView: () => kept.current,
  }));

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let destroyed = false;
    let cleanup = () => {};
    const app = new Application();
    const view = { w: el.clientWidth, h: el.clientHeight };

    (async () => {
      await document.fonts.ready;
      await app.init({
        width: Math.max(screen.availWidth, view.w),
        height: Math.max(screen.availHeight, view.h),
        background: cssColor("--canvas"),
        antialias: true,
        autoDensity: true,
        resolution: window.devicePixelRatio || 1,
        preference: "webgl",
      });
      if (destroyed) {
        app.destroy(true);
        return;
      }
      el.appendChild(app.canvas);
      app.stage.eventMode = "none";

      const text = cssColor("--text");
      const text2 = cssColor("--text2");
      const accent = cssColor("--accent");
      const card = cssColor("--card");
      const border = cssColor("--border");
      // Cards stand out from the canvas in both themes (D2).
      const cardBorder = cssColor("--card-border");
      const nameFont = `Heirloom-name-${dark ? "d" : "l"}`;
      const smallFont = `Heirloom-small-${dark ? "d" : "l"}`;
      BitmapFont.install({ name: nameFont, style: { fontFamily: "Newsreader Variable, Georgia, serif", fontSize: 40, fontWeight: "600", fill: text }, chars: CHARS });
      BitmapFont.install({ name: smallFont, style: { fontFamily: "IBM Plex Sans, sans-serif", fontSize: 32, fill: text2 }, chars: CHARS });

      const renderer = app.renderer as Renderer;
      const texture = (g: Graphics) => {
        const t = renderer.generateTexture({ target: g, resolution: 2 });
        g.destroy();
        return t;
      };
      const circle = texture(new Graphics().circle(8, 8, 8).fill(0xffffff));
      const square = texture(new Graphics().roundRect(0, 0, 16, 16, 3).fill(0xffffff));
      const diamond = texture(new Graphics().poly([8, 0, 16, 8, 8, 16, 0, 8]).fill(0xffffff));
      const block = texture(new Graphics().roundRect(0, 0, CARD_W, CARD_H, 6).fill(0xffffff));
      const cardBg = texture(new Graphics().roundRect(0.75, 0.75, CARD_W - 1.5, CARD_H - 1.5, 8).fill(card).stroke({ width: 1.5, color: cardBorder }));
      const stripeTex = texture(new Graphics().roundRect(0, 0, 8, CARD_H - 1.5, 4).fill(0xffffff));
      const shapeFor = (branch: number) => [circle, square, diamond][branch % 3];

      const world = new Container({ isRenderGroup: true });
      const bands = new Graphics();
      const directLine = new Graphics();
      const dots = new Container();
      const blocks = new Container();
      const cards = new Container();
      const selection = new Graphics();
      world.addChild(bands, blocks, directLine, dots, cards, selection);
      app.stage.addChild(world);

      const n = data.people.length;
      const index = new Map<string, number>();
      data.people.forEach((p, i) => index.set(p[0], i));
      const bandTop = (g: number) => (g - 1) * data.bandHeight;
      const worldH = data.bands * data.bandHeight;

      // Bands: odd ones slightly darker, each with a top hairline.
      for (let g = 1; g <= data.bands; g++) {
        if (g % 2 === 1) bands.rect(-2000, bandTop(g), data.width + 4000, data.bandHeight).fill({ color: text, alpha: 0.03 });
        bands.rect(-2000, bandTop(g), data.width + 4000, 1).fill({ color: border, alpha: 0.6 });
      }

      let colors = palette();
      const colorOf = (i: number) => colors[callbacks.current.colorFor(i)] ?? colors[0];
      const dotSprites: Sprite[] = [];
      const blockSprites: Sprite[] = [];
      for (let i = 0; i < n; i++) {
        const [, x, y, branch] = data.people[i];
        const color = colorOf(i);
        const d = new Sprite(shapeFor(branch));
        d.anchor.set(0.5);
        d.position.set(x + CARD_W / 2, y + CARD_H / 2);
        d.tint = color;
        dots.addChild(d);
        dotSprites.push(d);
        const b = new Sprite(block);
        b.position.set(x, y);
        b.tint = color;
        blocks.addChild(b);
        blockSprites.push(b);
      }

      const pool: Card[] = [];
      const shown = new Map<number, Card>();
      const acquire = (): Card => {
        const reused = pool.pop();
        if (reused) {
          reused.root.visible = true;
          return reused;
        }
        const root = new Container();
        const bg = new Sprite(cardBg);
        const stripe = new Sprite(stripeTex);
        stripe.position.set(0.75, 0.75);
        stripe.width = 4;
        const name = new BitmapText({ text: "", style: { fontFamily: nameFont, fontSize: 15 } });
        const years = new BitmapText({ text: "", style: { fontFamily: smallFont, fontSize: 12 } });
        name.position.set(14, 18);
        years.position.set(14, 40);
        root.addChild(bg, stripe, name, years);
        cards.addChild(root);
        return { root, bg, stripe, name, years };
      };
      const release = (c: Card) => {
        c.root.visible = false;
        pool.push(c);
      };

      let dirty = true;
      const camera = { x: 0, y: 0, zoom: 0.08 };
      const setCamera = (x: number, y: number, zoom: number) => {
        camera.x = x;
        camera.y = y;
        camera.zoom = zoom;
        world.position.set(x, y);
        world.scale.set(zoom);
        dirty = true;
      };
      const fit = () => {
        const zoom = Math.min((view.w - LEFT_COLUMN - 40) / Math.max(1, data.width), (view.h - 120) / Math.max(1, worldH));
        const z = Math.max(0.01, Math.min(1, zoom));
        setCamera(LEFT_COLUMN + 20 + (view.w - LEFT_COLUMN - 40 - data.width * z) / 2, 90 + (view.h - 120 - worldH * z) / 2, z);
      };
      const zoomAt = (z: number, mx: number, my: number) => {
        const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
        setCamera(mx - ((mx - camera.x) / camera.zoom) * zoom, my - ((my - camera.y) / camera.zoom) * zoom, zoom);
      };
      const centerOn = (id: string) => {
        const i = index.get(id);
        if (i == null) return;
        const [, x, y] = data.people[i];
        const zoom = Math.max(camera.zoom, 0.8);
        setCamera(view.w / 2 - (x + CARD_W / 2) * zoom, view.h / 2 - (y + CARD_H / 2) * zoom, zoom);
      };
      const showCluster = (c: { x: number; width: number; fromGen: number | null; toGen: number | null }) => {
        const top = ((c.fromGen ?? 1) - 1) * data.bandHeight;
        const height = ((c.toGen ?? c.fromGen ?? 1) - (c.fromGen ?? 1) + 1) * data.bandHeight;
        const zoom = Math.min(0.9, Math.max(0.02, Math.min((view.w - LEFT_COLUMN - 80) / Math.max(c.width, 400), (view.h - 160) / Math.max(height, 200))));
        setCamera(LEFT_COLUMN + (view.w - LEFT_COLUMN) / 2 - (c.x + c.width / 2) * zoom, 110 + (view.h - 130) / 2 - (top + height / 2) * zoom, zoom);
      };
      // „Koloruj wg” changed: every dot and block and the cards on screen take the new colours (no rebuild).
      const recolor = () => {
        colors = palette();
        for (let i = 0; i < n; i++) {
          const color = colorOf(i);
          dotSprites[i].tint = color;
          blockSprites[i].tint = color;
        }
        for (const [i, c] of shown) c.stripe.tint = colorOf(i);
        dirty = true;
      };
      // The selected person: an accent border with a soft halo around the card or block, a ring around the dot.
      const drawSelection = () => {
        selection.clear();
        const i = selectedRef.current == null ? undefined : index.get(selectedRef.current);
        if (i == null) return;
        const [, x, y] = data.people[i];
        const u = 1 / camera.zoom;
        if (camera.zoom < 0.15) {
          selection.circle(x + CARD_W / 2, y + CARD_H / 2, 9 * u).stroke({ width: 4 * u, color: accent, alpha: 0.25 });
          selection.circle(x + CARD_W / 2, y + CARD_H / 2, 7 * u).stroke({ width: 2 * u, color: accent });
        } else {
          selection.roundRect(x - 3 * u, y - 3 * u, CARD_W + 6 * u, CARD_H + 6 * u, 10).stroke({ width: 4 * u, color: accent, alpha: 0.2 });
          selection.roundRect(x, y, CARD_W, CARD_H, 8).stroke({ width: 2 * u, color: accent });
        }
      };
      const setView = (v: TreeCamera) => {
        // Right after the tree is shown again, before the resize observer has told the size.
        if (el.clientWidth > 0 && el.clientHeight > 0) {
          view.w = el.clientWidth;
          view.h = el.clientHeight;
        }
        const c = cameraOf(v, view, ORIGIN, MIN_ZOOM, MAX_ZOOM);
        setCamera(c.x, c.y, c.zoom);
      };

      // The direct line of the person the tree was opened at: father, else mother, each generation up.
      const drawLine = () => {
        directLine.clear();
        const focus = focusRef.current;
        const start = focus == null ? undefined : index.get(focus);
        if (start == null) return;
        const line: number[] = [];
        for (let i: number | null = start; i != null && !line.includes(i) && line.length <= 200; ) {
          line.push(i);
          const [father, mother]: [number | null, number | null] = data.people[i][13];
          i = father ?? mother;
        }
        const pts = line.map((i) => [data.people[i][1] + CARD_W / 2, data.people[i][2] + CARD_H / 2]);
        if (pts.length >= 2) {
          directLine.moveTo(pts[0][0], pts[0][1]);
          for (const [x, y] of pts.slice(1)) directLine.lineTo(x, y);
          directLine.stroke({ width: 2.5 / camera.zoom, color: accent, alpha: 0.85, join: "round" });
        }
        const [, x, y] = data.people[start];
        directLine.roundRect(x - 6 / camera.zoom, y - 6 / camera.zoom, CARD_W + 12 / camera.zoom, CARD_H + 12 / camera.zoom, 10 / camera.zoom).stroke({ width: 2.5 / camera.zoom, color: accent });
      };
      api.current = { fit, centerOn, zoomBy: (f) => zoomAt(camera.zoom * f, view.w / 2, view.h / 2), showCluster, recolor, drawSelection, drawLine, setView };

      let lastZoom = -1;
      const update = () => {
        const z = camera.zoom;
        // Levels of detail by card width on screen: dots, blocks, cards (spec §3.8 thresholds: 15 % / 40 %).
        const lod = z >= 0.4 ? 2 : z >= 0.15 ? 3 : 4;
        dots.visible = lod === 4;
        blocks.visible = lod === 3;
        const left = -camera.x / z;
        const top = -camera.y / z;
        const right = left + view.w / z;
        const bottom = top + view.h / z;
        if (lod === 4 && z !== lastZoom) {
          const s = 7 / 16 / z;
          for (const d of dotSprites) d.scale.set(s);
        }
        if (z !== lastZoom) {
          drawLine();
          drawSelection();
        }
        lastZoom = z;
        if (lod !== 2) {
          for (const c of shown.values()) release(c);
          shown.clear();
        } else {
          const wanted = new Set<number>();
          for (let i = 0; i < n; i++) {
            const [, x, y] = data.people[i];
            if (x + CARD_W >= left && x <= right && y + CARD_H >= top && y <= bottom) wanted.add(i);
          }
          for (const [i, c] of shown) {
            if (!wanted.has(i)) {
              release(c);
              shown.delete(i);
            }
          }
          for (const i of wanted) {
            if (shown.has(i)) continue;
            const c = acquire();
            const p = data.people[i];
            c.root.position.set(p[1], p[2]);
            c.stripe.tint = colorOf(i);
            // „Józef KOWALSKI”, „1878 † 1951” (design v2, A9).
            c.name.text = `${p[4]} ${p[5].toLocaleUpperCase("pl-PL")}`.trim();
            c.years.text = cardYears(p[6], p[7], !!p[10]);
            // A long name gets smaller down to 12 px and is cut with „…” only below that (as in nameFit.ts).
            const maxName = CARD_W - 24;
            c.name.scale.set(1);
            const wide = c.name.width;
            if (wide > maxName) c.name.scale.set(Math.max(maxName / wide, 0.8));
            // Cut by the share that fits rather than measuring shorter and shorter tries.
            if (wide * 0.8 > maxName) c.name.text = `${c.name.text.slice(0, Math.floor((c.name.text.length * maxName) / (wide * 0.8)) - 2).trimEnd()}…`;
            shown.set(i, c);
          }
        }
        kept.current = viewOf(camera, view, ORIGIN);
        callbacks.current.onZoom(z);
        callbacks.current.onCamera?.();
        setOverlay({ x: camera.x, y: camera.y, zoom: z });
      };

      app.ticker.add(() => {
        if (!running.current) return;
        if (dirty) {
          dirty = false;
          update();
        }
      });

      // Input: drag to pan, wheel to zoom at the cursor, click to select, double click to open the family view.
      const canvas = app.canvas;
      let drag: { x: number; y: number; moved: boolean } | null = null;
      // The nearest person: within their grid cell (card and the gaps around it) at the block and card levels,
      // within 14 px on screen at the dot level.
      const hit = (mx: number, my: number): number | null => {
        const wx = (mx - camera.x) / camera.zoom;
        const wy = (my - camera.y) / camera.zoom;
        const dotLevel = camera.zoom < 0.15;
        const reach = 14 / camera.zoom;
        let best: number | null = null;
        let bestDistance = Infinity;
        for (let i = 0; i < n; i++) {
          const [, x, y] = data.people[i];
          const dx = wx - (x + CARD_W / 2);
          const dy = wy - (y + CARD_H / 2);
          const distance = dx * dx + dy * dy;
          if (dotLevel ? distance > reach * reach : Math.abs(dx) > CELL_W / 2 || Math.abs(dy) > CELL_H / 2) continue;
          if (distance < bestDistance) {
            best = i;
            bestDistance = distance;
          }
        }
        return best;
      };
      const pos = (e: PointerEvent | WheelEvent | MouseEvent) => {
        const r = canvas.getBoundingClientRect();
        return [e.clientX - r.left, e.clientY - r.top] as const;
      };
      // Only the left button pans and selects: the mouse's back button must not pick the person under it.
      const onDown = (e: PointerEvent) => {
        // Esc (close the panel) belongs to the tree after a click in it.
        el.focus({ preventScroll: true });
        if (e.button !== 0) return;
        drag = { x: e.clientX, y: e.clientY, moved: false };
        canvas.setPointerCapture(e.pointerId);
      };
      const onMove = (e: PointerEvent) => {
        if (!drag) return;
        const dx = e.clientX - drag.x;
        const dy = e.clientY - drag.y;
        if (Math.abs(dx) + Math.abs(dy) > 2) drag.moved = true;
        drag.x = e.clientX;
        drag.y = e.clientY;
        setCamera(camera.x + dx, camera.y + dy, camera.zoom);
      };
      const onUp = (e: PointerEvent) => {
        if (e.button !== 0) return;
        if (drag && !drag.moved) {
          const [mx, my] = pos(e);
          const i = hit(mx, my);
          if (i != null) callbacks.current.onSelect(data.people[i][0]);
        }
        drag = null;
      };
      const onDouble = (e: MouseEvent) => {
        const [mx, my] = pos(e);
        const i = hit(mx, my);
        if (i != null) callbacks.current.onOpen(data.people[i][0]);
        else zoomAt(camera.zoom * 2, mx, my);
      };
      const onWheel = (e: WheelEvent) => {
        e.preventDefault();
        const [mx, my] = pos(e);
        zoomAt(camera.zoom * Math.exp(-e.deltaY * 0.0015), mx, my);
      };
      canvas.addEventListener("pointerdown", onDown);
      canvas.addEventListener("pointermove", onMove);
      canvas.addEventListener("pointerup", onUp);
      canvas.addEventListener("dblclick", onDouble);
      canvas.addEventListener("wheel", onWheel, { passive: false });
      const observer = new ResizeObserver(() => {
        // Hidden (another screen in front): the size to come back to stays.
        if (el.clientWidth === 0 || el.clientHeight === 0) return;
        view.w = el.clientWidth;
        view.h = el.clientHeight;
        if (view.w > app.screen.width || view.h > app.screen.height) app.renderer.resize(Math.max(view.w, app.screen.width), Math.max(view.h, app.screen.height));
        dirty = true;
      });
      observer.observe(el);
      // A camera to come back to, else the one before a rebuild, else the person the tree was opened at.
      const focus = focusRef.current;
      const back = restoreRef.current ?? kept.current;
      if (back) {
        setView(back);
        if (restoreRef.current) restored.current?.();
      } else if (focus && index.has(focus)) {
        fit();
        const i = index.get(focus)!;
        const [, x, y] = data.people[i];
        const z = Math.max(camera.zoom, 0.08);
        setCamera(view.w / 2 - (x + CARD_W / 2) * z, view.h / 2 - (y + CARD_H / 2) * z, z);
      } else fit();

      cleanup = () => {
        observer.disconnect();
        canvas.removeEventListener("wheel", onWheel);
        api.current = null;
        app.destroy(true, { children: true, texture: true, textureSource: true });
      };
    })();

    return () => {
      destroyed = true;
      cleanup();
    };
    // Rebuilt when the data or the theme changes (a new centre person only redraws the line, above).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, dark]);

  const z = overlay.zoom;
  const bandTopScreen = (g: number) => overlay.y + (g - 1) * data.bandHeight * z;
  // Surname labels over the families on screen; the spots of the last frame keep shown labels in place.
  const clusterKey = (c: Cluster) => `${c.label}-${c.x}`;
  const { placed, spots } = placeLabels(
    data.clusters
      .filter((c) => c.fromGen)
      .map((c) => ({
        key: clusterKey(c),
        count: c.count,
        width: labelWidth(c),
        left: overlay.x + c.x * z,
        right: overlay.x + (c.x + c.width) * z,
        top: bandTopScreen(c.fromGen!),
        bottom: bandTopScreen(c.toGen ?? c.fromGen!) + data.bandHeight * z,
      })),
    // Under both toolbar rows.
    { left: LEFT_COLUMN, top: 92, right: host.current?.clientWidth ?? 2000, bottom: host.current?.clientHeight ?? 1200 },
    labelSpots.current,
    z >= 0.4,
  );
  labelSpots.current = spots;
  const byKey = new Map(data.clusters.map((c) => [clusterKey(c), c]));
  return (
    <div className="overview-host" ref={host} tabIndex={-1} style={{ display: hidden ? "none" : undefined }}>
      <div className="band-labels">
        {Array.from({ length: data.bands }, (_, k) => k + 1).map((g) => {
          const top = bandTopScreen(g);
          const height = data.bandHeight * z;
          const year = data.bandYears[g - 1];
          return (
            <div key={g} className="band-label" style={{ top, height }}>
              {height > 26 && (
                <>
                  <span className="serif" style={{ fontSize: 17, lineHeight: 1.1, fontWeight: 500 }}>
                    {roman(g)}
                  </span>
                  {height > 44 && year && <span style={{ fontSize: 12, color: "var(--text3)" }}>ok. {Math.round(year / 10) * 10}</span>}
                </>
              )}
            </div>
          );
        })}
      </div>
      {placed.map(({ key, x, y }) => {
        const c = byKey.get(key)!;
        return (
          <button
            key={key}
            className="cluster-label"
            style={{ left: x, top: y }}
            onClick={() => api.current?.showCluster(c)}
            title="Przybliż grupę"
          >
            <span style={{ width: 8, height: 8, background: `var(--b${c.branch})`, borderRadius: c.branch % 3 === 0 ? "50%" : 2, transform: c.branch % 3 === 2 ? "rotate(45deg) scale(.85)" : undefined }} />
            <span className="serif" style={{ fontSize: 14, fontWeight: 600 }}>
              {c.label}
            </span>
            <span style={{ fontSize: 12, color: "var(--text3)" }}>{c.count} os.</span>
          </button>
        );
      })}
      <div className="overview-info">
        <span className="chip-info">
          <span>{count(data.people.length, "osoba", "osoby", "osób")}</span>
          <span>{count(data.bands, "pokolenie", "pokolenia", "pokoleń")}</span>
          <span style={{ color: "var(--text3)" }}>Kliknij osobę: szczegóły · podwójnie: jej rodzina</span>
        </span>
        <span className="zoom-h">
          <button onClick={() => api.current?.zoomBy(1 / 1.3)} title="Oddal">
            <Minus size={15} />
          </button>
          <span>{Math.round(z * 100)}%</span>
          <button onClick={() => api.current?.zoomBy(1.3)} title="Przybliż">
            <Plus size={15} />
          </button>
          <button onClick={() => api.current?.fit()} title="Dopasuj do ekranu">
            <Maximize size={15} />
          </button>
          <button onClick={() => focus && api.current?.centerOn(focus)} title="Wyśrodkuj na wybranej osobie" style={{ color: "var(--accent-text)" }}>
            <LocateFixed size={15} />
          </button>
        </span>
      </div>
    </div>
  );
});
