// „Kadr zdjęcia profilowego”: choose the square part of a photo that shows in the round avatar and on the profile.
// The photo itself does not change. Used on the profile and in import step 4.

import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { Dialog } from "./Dialog";
import { cropStyle, defaultCrop, moveCrop, resizeCrop, squareFrame, type Corner, type Crop } from "./cropMath";
import "./CropDialog.css";

/** The picture is fitted into this box. */
const STAGE_W = 460;
const STAGE_H = 360;
const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];

type Drag = { kind: "move" | Corner; x: number; y: number; from: Crop };

/** `src` is the photo (the upright picture); `initial` the crop it has now (null: none yet). `onSave(null)` is „Bez
 *  kadrowania” and is offered only when there was a crop. The caller closes the dialog after it has saved. */
export function CropDialog({
  src,
  initial,
  title,
  onSave,
  onClose,
}: {
  src: string;
  initial: Crop | null;
  title: string;
  onSave: (crop: Crop | null) => void;
  onClose: () => void;
}) {
  const [aspect, setAspect] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);
  const [crop, setCrop] = useState<Crop | null>(null);
  const drag = useRef<Drag | null>(null);
  const stage = useRef<HTMLDivElement>(null);
  const frameEl = useRef<HTMLDivElement>(null);
  // The arrow keys work from the start.
  const ready = !!crop;
  useEffect(() => {
    if (ready) frameEl.current?.focus();
  }, [ready]);

  const fit = aspect ? Math.min(STAGE_W / aspect, STAGE_H) : STAGE_H;
  const box = { width: aspect ? fit * aspect : STAGE_W, height: fit };

  const begin = (e: PointerEvent, kind: Drag["kind"]) => {
    if (!crop) return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag.current = { kind, x: e.clientX, y: e.clientY, from: crop };
  };
  const dragTo = (e: PointerEvent) => {
    const d = drag.current;
    const rect = stage.current?.getBoundingClientRect();
    if (!d || !rect || !aspect) return;
    if (d.kind === "move") setCrop(moveCrop(d.from, (e.clientX - d.x) / rect.width, (e.clientY - d.y) / rect.height, aspect));
    else setCrop(resizeCrop(d.from, d.kind, (e.clientX - rect.left) / rect.width, (e.clientY - rect.top) / rect.height, aspect));
  };
  const end = () => {
    drag.current = null;
  };
  const onKey = (e: KeyboardEvent) => {
    const step = e.shiftKey ? 0.05 : 0.01;
    const move: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const by = move[e.key];
    if (!by || !crop || !aspect) return;
    e.preventDefault();
    e.stopPropagation();
    setCrop(moveCrop(crop, by[0], by[1], aspect));
  };

  const preview = (size: number) =>
    crop && aspect ? (
      <span className="crop-preview" style={{ width: size, height: size }}>
        <img src={src} alt="" draggable={false} style={{ position: "absolute", maxWidth: "none", ...cropStyle(crop, aspect, size) }} />
      </span>
    ) : null;

  return (
    <Dialog width={780} onClose={onClose} labelledBy="crop-title">
      <div className="dialog-body">
        <div className="dialog-title" id="crop-title">
          {title}
        </div>
        <div className="dialog-text">Wybierz, co ma być widać w kółku i na profilu. Samo zdjęcie się nie zmienia.</div>
        <div className="crop-layout">
          <div ref={stage} className="crop-stage" style={box}>
            <img
              src={src}
              alt=""
              draggable={false}
              onLoad={(e) => {
                const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
                if (!w || !h) return setFailed(true);
                setAspect(w / h);
                setCrop(initial ? squareFrame(initial, w / h) : defaultCrop(w / h));
              }}
              onError={() => setFailed(true)}
            />
            {failed && <span className="crop-failed">Nie można wczytać zdjęcia.</span>}
            {crop && aspect && (
              <div
                ref={frameEl}
                className="crop-frame"
                tabIndex={0}
                role="group"
                aria-label="Kadr. Strzałki przesuwają ramkę."
                style={{ left: `${crop.x * 100}%`, top: `${crop.y * 100}%`, width: `${crop.w * 100}%`, height: `${crop.h * 100}%` }}
                onPointerDown={(e) => begin(e, "move")}
                onPointerMove={dragTo}
                onPointerUp={end}
                onPointerCancel={end}
                onKeyDown={onKey}
              >
                <span className="crop-circle" />
                {CORNERS.map((c) => (
                  <span key={c} className={`crop-handle ${c}`} onPointerDown={(e) => begin(e, c)} onPointerMove={dragTo} onPointerUp={end} onPointerCancel={end} />
                ))}
              </div>
            )}
          </div>
          <div className="crop-side">
            <span className="field-label">Podgląd</span>
            <div className="row" style={{ gap: 14, alignItems: "flex-end" }}>
              {preview(72)}
              {preview(32)}
            </div>
          </div>
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onClose}>
          Anuluj
        </button>
        {initial && (
          <button className="btn secondary" onClick={() => onSave(null)}>
            Bez kadrowania
          </button>
        )}
        <button className="btn primary" disabled={!crop} onClick={() => crop && onSave(crop)}>
          Zapisz kadr
        </button>
      </div>
    </Dialog>
  );
}
