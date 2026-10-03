// The source viewer (spec §4.32): the scan beside its transcription and translation. Images can be zoomed and
// turned; PDFs use the WebView's own viewer.

import { ChevronLeft, ChevronRight, FileImage, RotateCw, ZoomIn, ZoomOut } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { mediaUrl } from "../../api/transport";

export interface Scan {
  id: string;
  path: string | null;
  title: string | null;
  transcription: string | null;
  translation: string | null;
  kind: string;
}

export type ViewMode = "scan" | "both" | "text";

/** Zoom steps, relative to the scan fitted in the viewer. */
const ZOOMS = [1, 1.25, 1.5, 2, 3, 4];

export function SourceViewer({ scans, text, mode }: { scans: Scan[]; text: string | null; mode: ViewMode }) {
  const [index, setIndex] = useState(0);
  const scan = scans[Math.min(index, scans.length - 1)] as Scan | undefined;
  // The scan's own transcription first; else the source's text (GEDCOM TEXT, the words of the record).
  const columns: { title: string; body: string; quiet?: boolean }[] = [];
  if (scan?.transcription) columns.push({ title: "Transkrypcja", body: scan.transcription, quiet: true });
  else if (text) columns.push({ title: "Tekst źródła", body: text, quiet: true });
  if (scan?.translation) columns.push({ title: "Tłumaczenie", body: scan.translation });

  // „Tekst” on a scan without text still shows the scan, so the pager to the other scans stays at hand.
  const showScan = scan != null && (mode !== "text" || columns.length === 0);
  const showText = mode !== "scan" && columns.length > 0;
  if (!showScan && !showText) return null;
  const template = [...(showScan ? ["minmax(0, 1.1fr)"] : []), ...(showText ? columns.map(() => "minmax(0, 1fr)") : [])].join(" ");

  return (
    <div className={`sources-viewer${!showScan ? " text-only" : !showText ? " tall" : ""}`} style={{ gridTemplateColumns: template }}>
      {showScan && scan && <ScanColumn key={scan.id} scan={scan} index={index} total={scans.length} onPage={setIndex} />}
      {showText &&
        columns.map((c) => (
          <div key={c.title} className="sources-col">
            <div className="sources-col-head">
              <b>{c.title}</b>
            </div>
            <div className={`sources-col-text${c.quiet ? " quiet" : ""}`}>{c.body}</div>
          </div>
        ))}
    </div>
  );
}

function ScanColumn({ scan, index, total, onPage }: { scan: Scan; index: number; total: number; onPage: (i: number) => void }) {
  const [zoom, setZoom] = useState(1);
  const [turn, setTurn] = useState(0);
  const [failed, setFailed] = useState(false);
  const path = scan.path ?? "";
  const pdf = /\.pdf$/i.test(path);
  const image = /\.(jpe?g|png|gif|webp|bmp|tiff?|heic|avif)$/i.test(path);
  const tools = image && !failed;
  const step = (dir: 1 | -1) => setZoom((z) => (dir > 0 ? ZOOMS.find((v) => v > z) ?? z : [...ZOOMS].reverse().find((v) => v < z) ?? z));

  return (
    <div className="sources-col">
      <div className="sources-col-head">
        <b>Skan</b>
        {total > 1 && (
          <span className="row" style={{ gap: 2 }}>
            <button className="sources-tool" title="Poprzedni skan" disabled={index === 0} onClick={() => onPage(index - 1)}>
              <ChevronLeft size={14} />
            </button>
            <span className="num">
              {index + 1} z {total}
            </span>
            <button className="sources-tool" title="Następny skan" disabled={index >= total - 1} onClick={() => onPage(index + 1)}>
              <ChevronRight size={14} />
            </button>
          </span>
        )}
        <span className="grow" />
        {tools && (
          <>
            <button className="sources-tool" title="Pomniejsz" disabled={zoom <= ZOOMS[0]} onClick={() => step(-1)}>
              <ZoomOut size={14} />
            </button>
            <span className="num">{Math.round(zoom * 100)}%</span>
            <button className="sources-tool" title="Powiększ" disabled={zoom >= ZOOMS[ZOOMS.length - 1]} onClick={() => step(1)}>
              <ZoomIn size={14} />
            </button>
            <button className="sources-tool" title="Obróć" style={{ marginLeft: 6 }} onClick={() => setTurn((t) => (t + 1) % 4)}>
              <RotateCw size={14} />
            </button>
          </>
        )}
      </div>
      <div className="sources-scan" title={scan.title ?? undefined}>
        {!scan.path || failed || (!pdf && !image) ? (
          <div className="sources-scan-empty">
            <FileImage size={24} />
            {!scan.path || failed ? "Brak pliku skanu" : "Tego pliku nie da się tu pokazać"}
          </div>
        ) : pdf ? (
          <iframe className="sources-pdf" src={mediaUrl(path, "file")} title={scan.title ?? "Skan"} />
        ) : (
          <ScanImage src={mediaUrl(path, "view")} zoom={zoom} turn={turn} onError={() => setFailed(true)} />
        )}
      </div>
    </div>
  );
}

/** The image fitted to the viewer, then zoomed and turned in quarter turns; the box scrolls when it's bigger. */
function ScanImage({ src, zoom, turn, onError }: { src: string; zoom: number; turn: number; onError: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [view, setView] = useState({ w: 0, h: 0 });
  const [natural, setNatural] = useState<{ w: number; h: number } | null>(null);

  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setView({ w: el.clientWidth, h: el.clientHeight }));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  let layout: { width: number; height: number; outerW: number; outerH: number } | null = null;
  if (natural && view.w > 0 && view.h > 0) {
    const sideways = turn % 2 === 1;
    const w = sideways ? natural.h : natural.w;
    const h = sideways ? natural.w : natural.h;
    const scale = Math.min(view.w / w, view.h / h) * zoom;
    layout = { width: natural.w * scale, height: natural.h * scale, outerW: Math.max(view.w, w * scale), outerH: Math.max(view.h, h * scale) };
  }

  return (
    <div ref={box} className="sources-scan-box">
      <div style={{ position: "relative", width: layout?.outerW ?? "100%", height: layout?.outerH ?? "100%" }}>
        <img
          src={src}
          alt=""
          draggable={false}
          onLoad={(e) => setNatural({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
          onError={onError}
          style={
            layout
              ? {
                  position: "absolute",
                  left: "50%",
                  top: "50%",
                  width: layout.width,
                  height: layout.height,
                  maxWidth: "none",
                  transform: `translate(-50%, -50%) rotate(${turn * 90}deg)`,
                }
              : { position: "absolute", width: 1, height: 1, opacity: 0 }
          }
        />
      </div>
    </div>
  );
}
