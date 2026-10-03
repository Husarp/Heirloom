import { ChevronLeft, ChevronRight, ExternalLink, FileText, FileX, FolderOpen, Image as ImageIcon, ImageOff, Maximize, Minimize, Minus, Plus, X } from "lucide-react";
import { Fragment, useEffect, useRef, useState, type MouseEvent, type PointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { mediaUrl } from "../../api/transport";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Spinner } from "../../components/bits";
import { num } from "../../lib/format";
import { fileName, isImage, isPdf, isWebImage, MetaRows, openInProgram, showInFolder, TextBlock, useElementSize, type MediaItem } from "./shared";
import "./media.css";

export interface LightboxItem {
  id: string;
  path: string | null;
  title?: string | null;
  date?: string | null;
  kind?: string;
  /** The full path on disk, for "Otwórz w programie" and "Pokaż w folderze". Fetched with `media.get` when left out. */
  absolute?: string | null;
  /** The file is described in the archive but isn't on disk. */
  missing?: boolean;
}

type Info = Partial<Omit<MediaItem, "kind">> & LightboxItem;

const ZOOM_STEPS = [0.05, 0.1, 0.15, 0.2, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8];
const MAX_ZOOM = 8;
/** Room around a fitted image: the prev/next buttons at the sides, the "Pokaż oryginał" link below. */
const PAD_X = 88;
const PAD_Y = 32;
/** A filmstrip thumbnail (72) plus the gap (8). */
const STRIP_STEP = 80;

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** Podgląd (spec §4.31): the image with zoom and panning, ← → between items, a filmstrip and an info panel. PDFs
 *  show in the WebView's own viewer. Esc closes. Rendered over the whole window, in a fixed dark palette. */
export function Lightbox({ items, index, onClose, title }: { items: LightboxItem[]; index: number; onClose: () => void; title?: string }) {
  const go = useStore((s) => s.go);
  const [pos, setPos] = useState(() => clamp(index, 0, Math.max(0, items.length - 1)));
  const current = Math.min(pos, items.length - 1);
  const item = items[current] as LightboxItem | undefined;

  // Description, people, place and the rest, also for items that came without them (e.g. from a profile gallery).
  const { data: details, fresh } = useApi<MediaItem>(item ? "media.get" : null, item ? { id: item.id } : undefined);
  const info = (item ? { ...item, ...(fresh && details ? details : null) } : undefined) as Info | undefined;

  const [zoom, setZoom] = useState<number | "fit">("fit");
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [natural, setNatural] = useState<{ w: number; h: number } | null>(null);
  const [failed, setFailed] = useState(false);
  const [original, setOriginal] = useState(false);
  const [dragging, setDragging] = useState(false);
  // Every item opens fitted to the window.
  const [shownId, setShownId] = useState(item?.id);
  if (item && shownId !== item.id) {
    setShownId(item.id);
    setZoom("fit");
    setOffset({ x: 0, y: 0 });
    setNatural(null);
    setFailed(false);
    setOriginal(false);
  }

  const [stageRef, stage, stageEl] = useElementSize<HTMLDivElement>();
  const [stripRef, stripSize] = useElementSize<HTMLDivElement>();
  const rootRef = useRef<HTMLDivElement>(null);
  const dragStart = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);

  const path = info?.path ?? null;
  const absolute = info?.absolute ?? null;
  const missing = !path || !!info?.missing;
  const showsImage = !missing && isImage(path) && !failed;
  const showsPdf = !missing && isPdf(path);

  const fit =
    natural && stage.width > 0
      ? Math.min(Math.max(stage.width - 2 * PAD_X, 40) / natural.w, Math.max(stage.height - 2 * PAD_Y, 40) / natural.h, 1)
      : 1;
  const scale = zoom === "fit" ? fit : zoom;
  const zoomable = showsImage && natural != null;
  const shownW = natural ? natural.w * scale : 0;
  const shownH = natural ? natural.h * scale : 0;
  const maxX = Math.max(0, (shownW - stage.width) / 2);
  const maxY = Math.max(0, (shownH - stage.height) / 2);
  const off = { x: clamp(offset.x, -maxX, maxX), y: clamp(offset.y, -maxY, maxY) };
  const pannable = zoomable && (maxX > 0 || maxY > 0);

  /** Zooms keeping the point `around` (relative to the stage centre) where it is. */
  const zoomTo = (next: number, around = { x: 0, y: 0 }) => {
    if (!zoomable) return;
    const target = Math.min(next, MAX_ZOOM);
    if (target <= fit * 1.001) {
      setZoom("fit");
      setOffset({ x: 0, y: 0 });
      return;
    }
    const k = target / scale;
    setOffset({ x: around.x - (around.x - off.x) * k, y: around.y - (around.y - off.y) * k });
    setZoom(target);
  };
  const zoomIn = () => zoomTo(ZOOM_STEPS.find((s) => s > scale * 1.01) ?? MAX_ZOOM);
  const zoomOut = () => zoomTo([...ZOOM_STEPS].reverse().find((s) => s < scale * 0.99) ?? fit);
  const fitAgain = () => {
    setZoom("fit");
    setOffset({ x: 0, y: 0 });
  };
  const showOriginal = () => {
    // Only formats the WebView can draw are swapped for the file itself; for TIFF the converted view is the best.
    if (isWebImage(path)) setOriginal(true);
    setZoom(1);
    setOffset({ x: 0, y: 0 });
  };
  const step = (delta: number) => setPos(clamp(current + delta, 0, items.length - 1));
  const openPerson = (id: string) => {
    onClose();
    go({ name: "person", id });
  };

  // Keys go to the lightbox first (capture), so Esc and the arrows don't also reach the screen underneath. Keys
  // meant for something opened above it (Ctrl K, a dialog) are left alone; `body` means a focused button went away.
  const onKey = useRef<(e: KeyboardEvent) => void>(() => {});
  onKey.current = (e) => {
    const target = e.target as Node;
    if (e.ctrlKey || e.metaKey || (target !== document.body && !rootRef.current?.contains(target))) return;
    const actions: Record<string, () => void> = {
      Escape: onClose,
      ArrowLeft: () => step(-1),
      ArrowRight: () => step(1),
      Home: () => setPos(0),
      End: () => setPos(items.length - 1),
      "+": zoomIn,
      "=": zoomIn,
      "-": zoomOut,
      "0": fitAgain,
    };
    const action = actions[e.key];
    if (!action) return;
    e.preventDefault();
    e.stopPropagation();
    action();
  };
  useEffect(() => {
    const handler = (e: KeyboardEvent) => onKey.current(e);
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, []);

  // The wheel zooms at the cursor. A native listener, because React's wheel handler can't prevent the default.
  const onWheel = useRef<(e: WheelEvent) => void>(() => {});
  onWheel.current = (e) => {
    if (!stageEl) return;
    const rect = stageEl.getBoundingClientRect();
    zoomTo(scale * Math.exp(-e.deltaY * 0.0015), { x: e.clientX - rect.left - rect.width / 2, y: e.clientY - rect.top - rect.height / 2 });
  };
  useEffect(() => {
    if (!stageEl) return;
    const handler = (e: WheelEvent) => {
      e.preventDefault();
      onWheel.current(e);
    };
    stageEl.addEventListener("wheel", handler, { passive: false });
    return () => stageEl.removeEventListener("wheel", handler);
  }, [stageEl]);

  // Focus moves into the lightbox and back to where it was when it closes.
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    rootRef.current?.focus();
    return () => previous?.focus?.();
  }, []);

  const [fullscreen, setFullscreen] = useState(() => document.fullscreenElement != null);
  const enteredFullscreen = useRef(false);
  useEffect(() => {
    const update = () => setFullscreen(document.fullscreenElement != null);
    document.addEventListener("fullscreenchange", update);
    return () => {
      document.removeEventListener("fullscreenchange", update);
      // Closing the lightbox also ends the full screen it asked for.
      if (enteredFullscreen.current && document.fullscreenElement) document.exitFullscreen().catch(() => {});
    };
  }, []);
  const toggleFullscreen = () => {
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
    else
      document.documentElement
        .requestFullscreen()
        .then(() => {
          enteredFullscreen.current = true;
        })
        .catch(() => {});
  };

  const noItems = !item;
  useEffect(() => {
    if (noItems) onClose();
  }, [noItems, onClose]);
  if (!item || !info) return null;

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!pannable || e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    dragStart.current = { x: e.clientX, y: e.clientY, ox: off.x, oy: off.y };
    setDragging(true);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = dragStart.current;
    if (d) setOffset({ x: clamp(d.ox + e.clientX - d.x, -maxX, maxX), y: clamp(d.oy + e.clientY - d.y, -maxY, maxY) });
  };
  const endDrag = () => {
    dragStart.current = null;
    setDragging(false);
  };
  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    if (!zoomable || (e.target as HTMLElement).closest("button")) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const around = { x: e.clientX - rect.left - rect.width / 2, y: e.clientY - rect.top - rect.height / 2 };
    if (zoom !== "fit") fitAgain();
    else zoomTo(fit < 1 ? 1 : 2, around);
  };

  const visible = Math.max(1, Math.floor((stripSize.width + 8) / STRIP_STEP));
  const stripStart = clamp(current - Math.floor(visible / 2), 0, Math.max(0, items.length - visible));
  const people = info.people ?? [];
  const src = path ? mediaUrl(path, original ? "file" : "view") : "";

  return createPortal(
    <div ref={rootRef} className="lightbox" role="dialog" aria-modal="true" aria-label={info.title || "Podgląd"} tabIndex={-1}>
      <div className="lb-top">
        <span className="lb-count">
          <b>
            {num(current + 1)} z {num(items.length)}
          </b>
          {title && ` · ${title}`}
        </span>
        <div className="lb-zoom">
          <button onClick={zoomOut} disabled={!zoomable || zoom === "fit"} title="Pomniejsz (−)">
            <Minus size={15} />
          </button>
          <span>{zoomable ? `${Math.round(scale * 100)}%` : "—"}</span>
          <button onClick={zoomIn} disabled={!zoomable || scale >= MAX_ZOOM} title="Powiększ (+)">
            <Plus size={15} />
          </button>
        </div>
        <span style={{ flex: 1 }} />
        <button className="lb-btn" onClick={toggleFullscreen}>
          {fullscreen ? <Minimize size={15} /> : <Maximize size={15} />}
          {fullscreen ? "Zakończ pełny ekran" : "Pełny ekran"}
        </button>
        <button className="lb-btn" onClick={() => openInProgram(absolute)} disabled={!absolute || missing}>
          <ExternalLink size={15} />
          Otwórz w programie
        </button>
        <button className="lb-btn" onClick={() => showInFolder(absolute)} disabled={!absolute || missing}>
          <FolderOpen size={15} />
          Pokaż w folderze
        </button>
        <button className="lb-btn lb-close" onClick={onClose} title="Zamknij podgląd">
          <X size={15} />
          Esc
        </button>
      </div>

      <div className="lb-body">
        <div
          ref={stageRef}
          className="lb-stage"
          style={{ cursor: pannable ? (dragging ? "grabbing" : "grab") : undefined }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={endDrag}
          onPointerCancel={endDrag}
          onDoubleClick={onDoubleClick}
        >
          {showsImage && path && (
            <>
              {!natural && (
                <div className="lb-placeholder">
                  <img src={mediaUrl(path, "thumb", 256)} alt="" draggable={false} onError={(e) => (e.currentTarget.style.display = "none")} />
                  <Spinner size={22} />
                </div>
              )}
              <img
                key={path}
                className="lb-image"
                src={src}
                alt={info.title ?? ""}
                draggable={false}
                onLoad={(e) => setNatural({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
                onError={() => setFailed(true)}
                style={
                  natural
                    ? { width: shownW, height: shownH, transform: `translate(-50%, -50%) translate(${off.x}px, ${off.y}px)` }
                    : { visibility: "hidden" }
                }
              />
            </>
          )}
          {showsPdf && path && <iframe className="lb-frame" src={mediaUrl(path, "file")} title={info.title || fileName(path)} />}
          {missing && (
            <Message icon={<FileX size={32} />} text="Brak pliku" sub={fileName(path)}>
              <button
                className="lb-btn"
                onClick={() => {
                  onClose();
                  go({ name: "missingFiles" });
                }}
              >
                Brakujące pliki…
              </button>
            </Message>
          )}
          {!missing && !showsPdf && !showsImage && (
            <Message
              icon={failed ? <ImageOff size={32} /> : <FileText size={32} />}
              text={failed ? "Nie można pokazać tego obrazu." : "Tego pliku nie da się tu wyświetlić."}
              sub={fileName(path)}
            >
              {absolute && (
                <button className="lb-btn" onClick={() => openInProgram(absolute)}>
                  <ExternalLink size={15} />
                  Otwórz w programie
                </button>
              )}
            </Message>
          )}
          {zoomable && fit < 1 && (
            <button className="lb-original" onClick={zoom === "fit" ? showOriginal : fitAgain}>
              {zoom === "fit" ? `Pokaż oryginał (${natural.w} × ${natural.h})` : "Dopasuj do okna"}
            </button>
          )}
          {current > 0 && (
            <button className="lb-nav prev" onClick={() => step(-1)} title="Poprzednie (←)">
              <ChevronLeft size={22} />
            </button>
          )}
          {current < items.length - 1 && (
            <button className="lb-nav next" onClick={() => step(1)} title="Następne (→)">
              <ChevronRight size={22} />
            </button>
          )}
        </div>

        <aside className="lb-info">
          <div className="serif" style={{ fontSize: 22, lineHeight: 1.25, fontWeight: 600, overflowWrap: "anywhere" }}>
            {info.title || fileName(path) || "Bez podpisu"}
          </div>
          {info.note && <p style={{ fontSize: 14, lineHeight: 1.55, color: "var(--text2)", whiteSpace: "pre-wrap" }}>{info.note}</p>}
          <MetaRows item={info} gap={7} before={onClose} />
          {people.length > 0 && (
            <div className="media-section">
              <span className="label-caps">Osoby</span>
              <span className="serif" style={{ fontSize: 15, color: "var(--accent-text)" }}>
                {people.map((p, i) => (
                  <Fragment key={p.id}>
                    {i > 0 && " · "}
                    <button className="media-link" onClick={() => openPerson(p.id)}>
                      {p.name}
                    </button>
                  </Fragment>
                ))}
              </span>
            </div>
          )}
          {info.transcription && <TextBlock label="Transkrypcja" text={info.transcription} />}
          {info.translation && <TextBlock label="Tłumaczenie" text={info.translation} />}
        </aside>
      </div>

      <div ref={stripRef} className="lb-strip">
        {items.slice(stripStart, stripStart + visible).map((it, i) => (
          <button
            key={it.id}
            className={`lb-thumb${stripStart + i === current ? " on" : ""}`}
            onClick={() => setPos(stripStart + i)}
            title={it.title || fileName(it.path)}
            aria-current={stripStart + i === current}
          >
            <StripThumb item={it} />
          </button>
        ))}
      </div>
    </div>,
    document.body,
  );
}

function Message({ icon, text, sub, children }: { icon: ReactNode; text: string; sub?: string; children?: ReactNode }) {
  return (
    <div className="lb-message">
      {icon}
      <span style={{ color: "#bdb5a8" }}>{text}</span>
      {sub && (
        <span className="mono" style={{ fontSize: 12 }}>
          {sub}
        </span>
      )}
      {children}
    </div>
  );
}

function StripThumb({ item }: { item: LightboxItem }) {
  const [failed, setFailed] = useState(false);
  if (!item.path || item.missing) return <FileX size={15} />;
  if (!isImage(item.path)) return <FileText size={15} />;
  return (
    <>
      <ImageIcon size={15} />
      {!failed && <img src={mediaUrl(item.path, "thumb", 128)} alt="" loading="lazy" draggable={false} onError={() => setFailed(true)} />}
    </>
  );
}
