// Draws a focus view (Rodzina, Przodkowie, Potomkowie) as DOM cards over an SVG of connectors, in world coordinates
// with a CSS transform for the camera. Four levels of detail by zoom (spec §3.4–§3.8): full card ≥ 75 %, compact
// 40–75 %, mini 15–40 %, dots below; text in the compact and mini levels keeps its screen size.

import { Camera, ChevronLeft, ChevronsDown, ChevronsUp, Plus } from "lucide-react";
import { forwardRef, useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import type { TreeCamera } from "../../app/store";
import { ArchiveDot, Avatar, CardYears } from "../../components/bits";
import { cardName, cardYears } from "../../lib/format";
import { RelativeTools } from "../person/RelativeTools";
import { cameraOf, unfoldView, viewOf, type Camera2D } from "./camera";
import type { Graph, GraphPerson } from "./graph";
import { CARD_H, CARD_W, type Scene, type SceneCard } from "./layout";
import { fitName, forgetNameWidths, nameWidth } from "./nameFit";

export type Lod = 1 | 2 | 3 | 4;

/** Hysteresis: switch down at 75 / 40 / 15 %, back up a few points higher (spec §3.8). */
export function nextLod(current: Lod, zoom: number): Lod {
  const down = [0.75, 0.4, 0.15];
  const up = [0.8, 0.45, 0.2];
  let lod = current;
  while (lod < 4 && zoom < down[lod - 1]) lod = (lod + 1) as Lod;
  while (lod > 1 && zoom >= up[lod - 2]) lod = (lod - 1) as Lod;
  return lod;
}

export interface FocusCanvasHandle {
  fit: () => void;
  centerOn: (id: string, animate?: boolean) => void;
  zoomBy: (factor: number) => void;
  /** Moves the camera only when the card is (partly) off screen: keyboard steps keep the chosen card in view. */
  reveal: (id: string) => void;
  /** Where the camera is, relative to the centre person's card (for Back and „Ostatnie miejsce”). */
  getView: () => TreeCamera | null;
}

const MIN_ZOOM = 0.06;
const MAX_ZOOM = 2;
/** The column of the generations' names at the left edge (Potomkowie; `.tree-gutter`). */
const GUTTER = 112;
/** Przodkowie's column names stop here, under the two toolbar rows, when the tree is scrolled down. */
const STICKY_TOP = 104;

export interface CardStyle {
  color: (p: GraphPerson) => number | null;
  dimmed: (id: string) => boolean;
  highlighted: Set<string>;
}

function shape(branch: number): CSSProperties {
  const s = branch % 3;
  return { borderRadius: s === 0 ? "50%" : s === 1 ? 1.5 : 1, transform: s === 2 ? "rotate(45deg) scale(0.85)" : undefined };
}

export const FocusCanvas = forwardRef<FocusCanvasHandle, {
  scene: Scene;
  graph: Graph;
  selected: string | null;
  style: CardStyle;
  hidden: boolean;
  animate: boolean;
  onSelect: (id: string | null) => void;
  onOpen: (id: string) => void;
  onHover: (id: string | null) => void;
  onPill: (target: string, unfold: boolean) => void;
  onBox: (action: string, of?: string) => void;
  onZoom: (zoom: number) => void;
  focusId: string;
  photos: boolean;
  /** Where the person starts on screen: the middle (Rodzina), the right (Przodkowie) or the top (Potomkowie). */
  anchor: "center" | "right" | "top";
  /** In a new layout of the same centre, the card that keeps its place on screen (the one just unfolded); else the
   *  centre person's. */
  pin?: string;
  /** A camera to come back to (Back, „Ostatnie miejsce”), used instead of the usual placement once the centre
   *  person's card is laid out; `onRestored` then says it was used. */
  restore?: TreeCamera | null;
  onRestored?: () => void;
  /** The camera moved (to remember the place). */
  onCamera?: () => void;
  /** Edit mode: the tools under the relatives' cards, under all of them (`all`, Rodzina) or the chosen one's. */
  tools?: "all" | "selected" | null;
  /** A relative was unlinked or deleted with those tools. */
  onGone?: (id: string) => void;
}>(function FocusCanvas(props, ref) {
  const { scene, graph, selected, style, hidden, animate, onSelect, onOpen, onHover, onPill, onBox, onZoom, focusId, photos, anchor, pin, restore, onRestored, onCamera, tools, onGone } = props;
  const host = useRef<HTMLDivElement>(null);
  const [camera, setCamera] = useState<Camera2D>({ x: 0, y: 0, zoom: 1 });
  const [lod, setLod] = useState<Lod>(1);
  const [smooth, setSmooth] = useState(false);
  const drag = useRef<{ x: number; y: number; moved: boolean } | null>(null);
  const cameraRef = useRef(camera);
  cameraRef.current = camera;
  // The card names are fitted to the measured width of their font, so once it has loaded they are fitted again.
  const [, setFontReady] = useState(false);
  useEffect(() => {
    document.fonts?.load('600 15px "Newsreader Variable"').then(() => {
      forgetNameWidths();
      setFontReady(true);
    });
  }, []);

  const apply = useCallback(
    (next: Camera2D, withAnimation = false) => {
      setSmooth(withAnimation && animate);
      setCamera(next);
      setLod((l) => nextLod(l, next.zoom));
      onZoom(next.zoom);
    },
    [animate, onZoom],
  );

  const size = () => ({ w: host.current?.clientWidth ?? 800, h: host.current?.clientHeight ?? 600 });
  // The part of the screen left of the column of names isn't room for the tree.
  const gutter = scene.labels.some((l) => l.stick === "left") ? GUTTER : 0;

  const fit = useCallback(() => {
    const { w, h } = size();
    const b = scene.bounds;
    const bw = b.maxX - b.minX + 160;
    const bh = b.maxY - b.minY + 200;
    const zoom = Math.min(1.1, Math.max(0.1, Math.min((w - gutter) / bw, h / bh)));
    apply({ zoom, x: gutter + (w - gutter) / 2 - ((b.minX + b.maxX) / 2) * zoom, y: h / 2 - ((b.minY + b.maxY) / 2) * zoom + 10 }, true);
  }, [scene, apply, gutter]);

  const centerOn = useCallback(
    (id: string, withAnimation = true) => {
      const card = scene.cards.find((c) => c.id === id);
      if (!card) return;
      const { w, h } = size();
      const zoom = Math.max(cameraRef.current.zoom, 0.8);
      apply({ zoom, x: gutter + (w - gutter) / 2 - (card.x + (card.w ?? CARD_W) / 2) * zoom, y: h / 2 - (card.y + CARD_H / 2) * zoom }, withAnimation);
    },
    [scene, apply, gutter],
  );

  const zoomAt = useCallback(
    (factor: number, mx?: number, my?: number) => {
      const c = cameraRef.current;
      const { w, h } = size();
      const px = mx ?? w / 2;
      const py = my ?? h / 2;
      const zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, c.zoom * factor));
      apply({ zoom, x: px - ((px - c.x) / c.zoom) * zoom, y: py - ((py - c.y) / c.zoom) * zoom }, mx == null);
    },
    [apply],
  );

  const reveal = useCallback(
    (id: string) => {
      const card = scene.cards.find((c) => c.id === id);
      if (!card) return;
      const { w, h } = size();
      const c = cameraRef.current;
      const x = c.x + card.x * c.zoom;
      const y = c.y + card.y * c.zoom;
      // The toolbars cover the top 110 px.
      const margin = 24;
      if (x < gutter + margin || y < 110 || x + (card.w ?? CARD_W) * c.zoom > w - margin || y + CARD_H * c.zoom > h - margin) centerOn(id);
    },
    [scene, centerOn, gutter],
  );

  const focusCentre = useCallback(() => {
    const card = scene.cards.find((c) => c.id === focusId);
    return card ? { x: card.x + CARD_W / 2, y: card.y + CARD_H / 2 } : null;
  }, [scene, focusId]);

  const getView = useCallback(() => {
    const centre = focusCentre();
    return centre ? viewOf(cameraRef.current, size(), centre) : null;
  }, [focusCentre]);

  useImperativeHandle(ref, () => ({ fit, centerOn, zoomBy: (f) => zoomAt(f), reveal, getView }), [fit, centerOn, zoomAt, reveal, getView]);

  const cameraMoved = useRef(onCamera);
  cameraMoved.current = onCamera;
  useEffect(() => cameraMoved.current?.(), [camera]);

  // A new scene: another person in the middle moves the camera to them with animation; the same person in a new
  // layout (fresh data, edit mode, a branch unfolded) moves the camera with their card, or with the pinned one, so it
  // stays put on screen.
  const lastFocus = useRef<string | null>(null);
  const lastScene = useRef<Scene | null>(null);
  // The first placement needs the canvas size; a window that isn't shown yet (minimised) reports 0 × 0.
  const [hasSize, setHasSize] = useState(false);
  // Przodkowie: the tree hangs from the right edge, so when the canvas narrows (the side panel opens) the chosen card,
  // or else the person at the right, stays in view: the tree moves left only as far as that card needs, so a card
  // clicked further left stays under the pointer. Widening again (the panel closed) moves it back as far.
  const rightAnchored = useRef(anchor === "right");
  rightAnchored.current = anchor === "right";
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const panelShift = useRef(0);
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let width = el.clientWidth;
    const observer = new ResizeObserver(() => {
      setHasSize(el.clientWidth > 0 && el.clientHeight > 0);
      const change = el.clientWidth - width;
      if (change && width > 0 && el.clientWidth > 0 && rightAnchored.current && lastFocus.current) {
        const c = cameraRef.current;
        let shift = 0;
        if (change > 0) shift = Math.min(change, panelShift.current);
        else {
          const id = selectedRef.current ?? lastFocus.current;
          const card = lastScene.current?.cards.find((k) => k.id === id);
          if (card) shift = -Math.min(-change, Math.max(0, c.x + (card.x + (card.w ?? CARD_W)) * c.zoom - (el.clientWidth - 24)));
        }
        panelShift.current -= shift;
        if (shift) setCamera({ ...c, x: c.x + shift });
      }
      width = el.clientWidth;
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const card = scene.cards.find((c) => c.id === focusId);
    if (!card) return;
    const { w, h } = size();
    if (lastFocus.current === null && (w === 0 || h === 0)) return;
    // Przodkowie: the person at the right, their ancestors to scroll to on the left; Potomkowie: at the top, right of
    // the column of names.
    const placeX = (zoom: number) => (anchor === "right" ? w - 80 - (card.x + CARD_W) * zoom : gutter + (w - gutter) / 2 - (card.x + CARD_W / 2) * zoom);
    if (restore && w > 0 && h > 0) {
      panelShift.current = 0;
      apply(cameraOf(restore, { w, h }, { x: card.x + CARD_W / 2, y: card.y + CARD_H / 2 }, MIN_ZOOM, MAX_ZOOM));
      onRestored?.();
    } else if (lastFocus.current === null) {
      panelShift.current = 0;
      const y = anchor === "top" ? 150 - card.y : h / 2 - (card.y + CARD_H / 2) + (anchor === "center" && scene.bounds.minY < -100 ? 40 : 0);
      apply({ zoom: 1, x: placeX(1), y });
    } else if (lastFocus.current !== focusId) {
      panelShift.current = 0;
      const zoom = Math.max(cameraRef.current.zoom, 0.8);
      const y = anchor === "top" ? 150 - card.y * zoom : h / 2 - (card.y + CARD_H / 2) * zoom;
      apply({ zoom, x: placeX(zoom), y }, true);
    } else {
      const old = lastScene.current?.cards;
      const id = pin && old?.some((c) => c.id === pin) && scene.cards.some((c) => c.id === pin) ? pin : focusId;
      const before = old?.find((c) => c.id === id);
      const now = scene.cards.find((c) => c.id === id);
      const c = cameraRef.current;
      let next = before && now && (before.x !== now.x || before.y !== now.y) ? { zoom: c.zoom, x: c.x - (now.x - before.x) * c.zoom, y: c.y - (now.y - before.y) * c.zoom } : c;
      // A branch just unfolded: its card and the row of children below come into view, by the least move (and a
      // little less zoom where the row is wider than the screen, as far as full cards).
      const fresh = id === pin && now ? scene.cards.filter((k) => k.rel?.of === id && !old?.some((o) => o.id === k.id)) : [];
      if (now && fresh.length) {
        const row = [now, ...fresh];
        const box = { x: Math.min(...row.map((k) => k.x)), y: Math.min(...row.map((k) => k.y)), r: Math.max(...row.map((k) => k.x + (k.w ?? CARD_W))), b: Math.max(...row.map((k) => k.y + CARD_H)) };
        // The toolbars cover the top 110 px.
        next = unfoldView(next, box, { x: now.x + (now.w ?? CARD_W) / 2, y: now.y }, { left: gutter + 24, top: 110, right: w - 24, bottom: h - 24 });
      }
      if (next !== c) apply(next, true);
    }
    lastFocus.current = focusId;
    lastScene.current = scene;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusId, scene, hasSize, restore]);

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const r = el.getBoundingClientRect();
      zoomAt(Math.exp(-e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [zoomAt]);

  const onPointerDown = (e: React.PointerEvent) => {
    // The keys (arrows, Enter…) go to the tree after a click in it.
    host.current?.focus({ preventScroll: true });
    // Only the left button pans and deselects (the mouse's back button goes back, nothing more).
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, .tree-card, .tree-strip, .relative-menu")) return;
    drag.current = { x: e.clientX, y: e.clientY, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    if (Math.abs(dx) + Math.abs(dy) > 2) d.moved = true;
    drag.current = { x: e.clientX, y: e.clientY, moved: d.moved };
    setSmooth(false);
    setCamera((c) => ({ ...c, x: c.x + dx, y: c.y + dy }));
  };
  const onPointerUp = () => {
    if (drag.current && !drag.current.moved) onSelect(null);
    drag.current = null;
  };

  const inv = 1 / camera.zoom;
  const worldStyle: CSSProperties = {
    transform: `translate(${camera.x}px, ${camera.y}px) scale(${camera.zoom})`,
    transition: smooth ? "transform 280ms ease" : undefined,
    ["--inv" as string]: inv,
  };
  const b = scene.bounds;
  const pad = 400;

  // The dot grid pans and zooms with the tree (spec §3.1).
  const grid = Math.max(8, 24 * camera.zoom);

  return (
    <div
      ref={host}
      className={`tree-canvas lod-${lod}`}
      tabIndex={0}
      aria-label="Drzewo. Strzałki przechodzą między osobami, Enter pokazuje panel, Shift Enter otwiera profil."
      style={{ display: hidden ? "none" : undefined, backgroundSize: `${grid}px ${grid}px`, backgroundPosition: `${camera.x}px ${camera.y}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      // A control focused at the edge (a pill half off screen) would scroll the canvas itself, shifting the tree
      // under the column of names; the camera does all the moving, so the canvas stays unscrolled.
      onScroll={(e) => {
        e.currentTarget.scrollLeft = 0;
        e.currentTarget.scrollTop = 0;
      }}
    >
      <div className="tree-world" style={worldStyle}>
        <svg className="tree-lines" style={{ left: b.minX - pad, top: b.minY - pad, width: b.maxX - b.minX + pad * 2, height: b.maxY - b.minY + pad * 2 }} viewBox={`${b.minX - pad} ${b.minY - pad} ${b.maxX - b.minX + pad * 2} ${b.maxY - b.minY + pad * 2}`}>
          {scene.links.map((l) => {
            const on = l.people.length > 0 && l.people.every((p) => style.highlighted.has(p));
            return <path key={l.key} d={l.d} className={`link ${l.kind}${on ? " on" : ""}`} />;
          })}
          {lod === 1 &&
            scene.unions.map((u) => {
              const own = selected != null && u.people.includes(selected);
              const path = u.people.every((p) => style.highlighted.has(p));
              return (
                <g key={u.key}>
                  <circle cx={u.x} cy={u.y} r={own ? 5.5 : path ? 4.5 : 3.5} className={`union${own || path ? " on" : ""}`} style={{ strokeWidth: own ? 2 : 1.5 }} />
                  {own && <circle cx={u.x} cy={u.y} r={2} className="union-dot" />}
                </g>
              );
            })}
        </svg>
        {lod <= 2 &&
          scene.labels
            .filter((l) => !l.stick)
            .map((l) => (
              <span key={l.key} className="tree-label" style={{ left: l.x, top: l.y }}>
                {l.text}
              </span>
            ))}
        {lod <= 2 &&
          scene.boxes.map((box) => (
            <button key={box.key} className={`tree-box ${box.action}`} style={{ left: box.x, top: box.y, width: box.w, height: box.h }} onClick={() => onBox(box.action, box.of)}>
              {box.action === "add-parents" && <Plus size={14} />}
              {box.label}
            </button>
          ))}
        {scene.cards.map((c) => (
          <Card
            key={c.id}
            card={c}
            person={graph.people[c.id]}
            lod={lod}
            selected={selected === c.id}
            color={graph.people[c.id] ? style.color(graph.people[c.id]) : null}
            dimmed={style.dimmed(c.id)}
            animate={animate}
            photos={photos}
            onSelect={onSelect}
            onOpen={onOpen}
            onHover={onHover}
          />
        ))}
        {lod <= 2 &&
          scene.pills.map((p) => (
            <button
              key={p.key}
              className={`tree-pill ${p.direction}`}
              style={{ left: p.x, top: p.y }}
              onClick={() => onPill(p.target, !!p.unfold)}
              title={p.unfold ? "Rozwiń kolejne 3 pokolenia tej gałęzi" : p.direction === "down" ? "Potomkowie — pokaż od tej osoby" : "Dalsi przodkowie — pokaż od tej osoby"}
            >
              {p.direction === "up" ? <ChevronsUp size={12} /> : p.direction === "left" ? <ChevronLeft size={12} /> : <ChevronsDown size={12} />}
              {p.label}
            </button>
          ))}
        {lod === 1 &&
          tools &&
          scene.cards.map((c) => {
            const p = graph.people[c.id];
            const of = c.rel && graph.people[c.rel.of];
            if (!c.rel || !p || !of || c.stub || (tools === "selected" && selected !== c.id)) return null;
            const [parent, child] = c.rel.kind === "parent" ? [c.id, c.rel.of] : [c.rel.of, c.id];
            const union = c.rel.kind === "parent" || c.rel.kind === "child" ? graph.unions.find((u) => u.partners.includes(parent) && u.children.some((k) => k.id === child)) : undefined;
            // Under the card, or beside it at the top where the next card is too close below (Przodkowie's last column).
            const w = c.w ?? CARD_W;
            const side = scene.cards.some((o) => o !== c && o.y > c.y && o.y < c.y + CARD_H + 30 && o.x < c.x + w && o.x + (o.w ?? CARD_W) > c.x);
            return (
              <div
                key={`tools-${c.id}`}
                className={`tree-strip${side ? " side" : ""}`}
                style={{ transform: side ? `translate(${c.x + w - 1}px, ${c.y - 3}px)` : `translate(${c.x}px, ${c.y + CARD_H - 1}px)`, transition: animate ? "transform 280ms ease, opacity 200ms" : undefined, opacity: style.dimmed(c.id) ? 0.35 : 1 }}
              >
                <RelativeTools variant="strip" of={of} relative={p} kind={c.rel.kind} family={union?.id} pedi={union?.children.find((k) => k.id === child)?.pedi} onGone={onGone} />
              </div>
            );
          })}
      </div>
      <StickyLabels labels={scene.labels} camera={camera} lod={lod} smooth={smooth && animate} />
    </div>
  );
});

/** The labels kept in view (`SceneLabel.stick`), on screen rather than in the world: Przodkowie's column names
 *  follow their columns across and stop under the toolbars going down; Potomkowie's row names sit in a column at the
 *  left edge, over whatever is scrolled under it, beside their rows. */
function StickyLabels({ labels, camera, lod, smooth }: { labels: Scene["labels"]; camera: Camera2D; lod: Lod; smooth: boolean }) {
  const top = labels.filter((l) => l.stick === "top");
  const left = labels.filter((l) => l.stick === "left");
  const transition = smooth ? "left 280ms ease, top 280ms ease" : undefined;
  // Row names while the rows are far enough apart for them.
  const pitch = left.length > 1 ? (left[1].y - left[0].y) * camera.zoom : Infinity;
  return (
    <>
      {lod <= 2 &&
        top.map((l) => (
          <span key={l.key} className="tree-label sticky top" style={{ left: camera.x + l.x * camera.zoom, top: Math.max(STICKY_TOP, camera.y + l.y * camera.zoom), transition }}>
            {l.text}
          </span>
        ))}
      {left.length > 0 && (
        <div className="tree-gutter">
          {pitch >= 24 &&
            left.map((l) => {
              // A row gone up under the toolbars takes its name with it (none of it peeks out beside them).
              const top = camera.y + (l.y + CARD_H / 2) * camera.zoom;
              return (
                top >= STICKY_TOP + 8 && (
                  <span key={l.key} className="tree-label sticky left" style={{ top, transition }}>
                    {l.text}
                  </span>
                )
              );
            })}
        </div>
      )}
    </>
  );
}

function Card({
  card,
  person: p,
  lod,
  selected,
  color,
  dimmed,
  animate,
  photos,
  onSelect,
  onOpen,
  onHover,
}: {
  card: SceneCard;
  person: GraphPerson | undefined;
  lod: Lod;
  selected: boolean;
  color: number | null;
  dimmed: boolean;
  animate: boolean;
  photos: boolean;
  onSelect: (id: string) => void;
  onOpen: (id: string) => void;
  onHover: (id: string | null) => void;
}) {
  const base: CSSProperties = {
    transform: `translate(${card.x}px, ${card.y}px)`,
    width: card.w,
    transition: animate ? "transform 280ms ease, opacity 200ms" : undefined,
    opacity: dimmed ? 0.35 : undefined,
  };
  if (card.stub) {
    return (
      <div className="tree-card stub" style={base}>
        {lod <= 2 && card.stub}
      </div>
    );
  }
  if (!p) return null;
  const branchColor = color ? `var(--b${color})` : "var(--line)";
  const events = {
    onClick: (e: React.MouseEvent) => {
      e.stopPropagation();
      onSelect(card.id);
    },
    onDoubleClick: () => onOpen(card.id),
    onMouseEnter: () => onHover(card.id),
    onMouseLeave: () => onHover(null),
  };
  if (lod === 4) {
    return (
      <div className="tree-card dot" style={base} {...events}>
        <span className="dot-shape" style={{ background: branchColor, ...shape(color ?? 3) }} />
      </div>
    );
  }
  if (lod === 3) {
    // Dark ink on the light branch colours 4, 6, 9 and 10 (D3); per theme through the tokens.
    const ink = color && [4, 6, 9, 10].includes(color) ? "var(--lod-ink-dk)" : "var(--lod-ink)";
    return (
      <div className={`tree-card mini${selected ? " selected" : ""}${card.partner ? " partner" : ""}`} style={{ ...base, background: branchColor }} {...events}>
        <span className="mini-letter" style={{ color: color ? ink : "var(--lod-ink)" }}>
          {(p.given || p.name).charAt(0)}
        </span>
      </div>
    );
  }
  const dates = (
    <span className="card-dates">
      <CardYears birth={p.birth} death={p.death} living={p.living} />
    </span>
  );
  if (lod === 2) {
    return (
      <div className={`tree-card compact${selected ? " selected" : ""}${card.partner ? " partner" : ""}`} style={base} {...events}>
        <span className="stripe" style={{ background: branchColor }} />
        <ArchiveDot from={p.from} size={9} style={CARD_DOT} />
        <span className="compact-text">
          <span className="compact-name">{p.given || p.name}</span>
          <span className="compact-years num">{cardYears(p.birth?.year, p.death?.year, p.living)}</span>
        </span>
      </div>
    );
  }
  const name = cardName(p);
  // The room left of the padding, border and photo (`.tree-card.full`, selected), less 2 px for rounding, and of the
  // archive's dot in archives opened together.
  const fit = fitName(name, (card.w ? card.w - 23 : CARD_W - 79) - (p.from ? 8 : 0), card.w ? 14 : 15, nameWidth);
  return (
    <div className={`tree-card full${selected ? " selected" : ""}${card.focus ? " focus" : ""}${card.w ? " narrow" : ""}${card.partner ? " partner" : ""}`} style={base} {...events}>
      <span className="stripe" style={{ background: branchColor }} />
      <ArchiveDot from={p.from} size={10} style={CARD_DOT} />
      {!card.w && <Avatar initials={p.initials} branch={color ?? undefined} photo={photos ? p.photo : null} size={40} tint={22} />}
      <span className="card-text">
        <span className={`card-name${fit.lines === 2 ? " two" : ""}`} style={{ fontSize: fit.size }} title={fit.cut ? name : undefined}>
          {name}
        </span>
        {card.sub && <span className="card-sub">{card.sub}</span>}
        <span className="card-meta">
          {dates}
          <span className="grow" />
          {p.photoCount > 0 && (
            <span className="card-photos">
              <Camera size={12} />
              {p.photoCount}
            </span>
          )}
        </span>
      </span>
    </div>
  );
}

/** Archives opened together: the archive's dot in the card's top right corner. */
const CARD_DOT: CSSProperties = { position: "absolute", top: 6, right: 6, boxShadow: "0 0 0 2px var(--card)" };

export function useStableSet(values: Iterable<string>): Set<string> {
  const list = [...values].sort().join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps
  return useMemo(() => new Set(list ? list.split("|") : []), [list]);
}
