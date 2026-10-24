import { RotateCcw, RotateCw, Undo2 } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { createPortal } from "react-dom";
import { call, mediaUrl } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { Segmented, Spinner } from "../../components/bits";
import { fileName, runEdit, useElementSize } from "./shared";
import {
  angleText,
  fitBox,
  isWhole,
  MAX_ANGLE,
  moveCrop,
  previewImage,
  resizeCrop,
  roundAngle,
  squareCrop,
  turnedSize,
  WHOLE,
  type Corner,
  type Crop,
} from "./photoMath";
import "./media.css";

type Tool = "rotate" | "straighten" | "crop";

/** What `media.editPhoto` and `media.restoreOriginal` answer. */
interface EditResult {
  path: string;
  droppedCrops: number;
}

/** Room kept around the picture in the stage (the crop handles stick out of the frame). */
const PAD = 28;
const CORNERS: Corner[] = ["nw", "ne", "sw", "se"];
const LOST_CROP = "Kadr zdjęcia profilowego trzeba ustawić na nowo.";

type Drag = { kind: "move" | Corner; x: number; y: number; from: Crop };

/** „Edytuj zdjęcie”: turn, straighten and crop a photo. Saving makes a new version of the file (the original stays in
 *  the archive and can be restored); the preview is the file server's `view` image, drawn the way the server will make
 *  the new one. Rendered over the whole window in the lightbox's dark palette. Esc = Anuluj. */
export function PhotoEditor({ id, path, onClose }: { id: string; path: string; onClose: () => void }) {
  const notify = useStore((s) => s.notify);
  const [natural, setNatural] = useState<{ w: number; h: number } | null>(null);
  const [failed, setFailed] = useState(false);
  const [tool, setTool] = useState<Tool>("rotate");
  const [quarter, setQuarter] = useState(0);
  const [angle, setAngle] = useState(0);
  const [crop, setCrop] = useState<Crop>(WHOLE);
  const [square, setSquare] = useState(false);
  const [saving, setSaving] = useState(false);
  const [stageRef, stage] = useElementSize<HTMLDivElement>();
  const rootRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);

  // Focus moves into the editor and back to where it was when it closes.
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    rootRef.current?.focus();
    return () => previous?.focus?.();
  }, []);

  const turned = natural ? turnedSize(natural.w, natural.h, quarter) : null;
  const aspect = turned ? turned.w / turned.h : 1;
  const box = turned ? fitBox(aspect, stage.width - 2 * PAD, stage.height - 2 * PAD) : { w: 0, h: 0 };
  const changed = quarter !== 0 || angle !== 0 || !isWhole(crop);

  const turn = (by: number) => {
    setQuarter((q) => (q + by + 4) % 4);
    // The frame was drawn on the picture as it was.
    resetCrop();
  };
  const resetCrop = () => {
    setCrop(WHOLE);
    setSquare(false);
  };
  const chooseSquare = (on: boolean) => {
    setSquare(on);
    if (on) setCrop((c) => squareCrop(c, aspect));
  };

  const begin = (e: PointerEvent, kind: Drag["kind"]) => {
    if (tool !== "crop") return;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag.current = { kind, x: e.clientX, y: e.clientY, from: crop };
  };
  const dragTo = (e: PointerEvent) => {
    const d = drag.current;
    const rect = boxRef.current?.getBoundingClientRect();
    if (!d || !rect || rect.width === 0 || rect.height === 0) return;
    if (d.kind === "move") setCrop(moveCrop(d.from, (e.clientX - d.x) / rect.width, (e.clientY - d.y) / rect.height));
    else setCrop(resizeCrop(d.from, d.kind, (e.clientX - rect.left) / rect.width, (e.clientY - rect.top) / rect.height, square, aspect));
  };
  const end = () => {
    drag.current = null;
  };
  const onFrameKey = (e: KeyboardEvent) => {
    const step = e.shiftKey ? 0.05 : 0.01;
    const by: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const move = by[e.key];
    if (!move) return;
    e.preventDefault();
    e.stopPropagation();
    setCrop((c) => moveCrop(c, move[0], move[1]));
  };
  // Keys stay in the editor: Esc is „Anuluj” and does not reach the lightbox or the screen under it.
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || e.ctrlKey || e.metaKey) return;
    e.preventDefault();
    e.stopPropagation();
    if (!saving) onClose();
  };

  const save = () => {
    if (!changed || saving) return;
    runEdit(async () => {
      setSaving(true);
      try {
        const result = await call<EditResult>("media.editPhoto", {
          id,
          rotate: quarter * 90,
          angle,
          crop: isWhole(crop) ? null : crop,
        });
        afterChange();
        notify("Zapisano nową wersję zdjęcia.", {
          detail: result.droppedCrops > 0 ? `Oryginał został w archiwum. ${LOST_CROP}` : "Oryginał został w archiwum — możesz go przywrócić.",
        });
        onClose();
      } finally {
        setSaving(false);
      }
    });
  };

  const showFrame = !isWhole(crop) || tool === "crop";
  const image = natural && box.w > 0 ? previewImage(natural, quarter, angle, box) : null;

  return createPortal(
    <div ref={rootRef} className="photo-editor" role="dialog" aria-modal="true" aria-label="Edytuj zdjęcie" tabIndex={-1} onKeyDown={onKey}>
      <div className="pe-top">
        <b>Edytuj zdjęcie</b>
        <span className="pe-file">{fileName(path)}</span>
        <span style={{ flex: 1 }} />
        <button className="lb-btn" onClick={onClose} disabled={saving} title="Anuluj (Esc)">
          Anuluj
        </button>
        <button className="pe-save" onClick={save} disabled={!changed || saving}>
          {saving && <Spinner size={14} />}
          Zapisz jako nową wersję
        </button>
      </div>

      <div ref={stageRef} className="pe-stage">
        {failed ? (
          <span className="pe-failed">Nie można wczytać tego zdjęcia.</span>
        ) : (
          <>
            {!natural && <Spinner size={22} />}
            <div ref={boxRef} className="pe-box" style={natural ? { width: box.w, height: box.h } : { display: "none" }}>
              <div className="pe-picture">
                <img
                  src={mediaUrl(path, "view")}
                  alt=""
                  draggable={false}
                  onLoad={(e) => {
                    const { naturalWidth: w, naturalHeight: h } = e.currentTarget;
                    if (w && h) setNatural({ w, h });
                    else setFailed(true);
                  }}
                  onError={() => setFailed(true)}
                  style={image ? image : { visibility: "hidden" }}
                />
                {tool === "straighten" && <div className="pe-grid" />}
              </div>
              {showFrame && (
                <div
                  ref={frameRef}
                  className={`pe-frame${tool === "crop" ? " active" : ""}`}
                  tabIndex={tool === "crop" ? 0 : -1}
                  role="group"
                  aria-label="Kadr. Strzałki przesuwają ramkę."
                  style={{ left: `${crop.x * 100}%`, top: `${crop.y * 100}%`, width: `${crop.w * 100}%`, height: `${crop.h * 100}%` }}
                  onPointerDown={(e) => begin(e, "move")}
                  onPointerMove={dragTo}
                  onPointerUp={end}
                  onPointerCancel={end}
                  onKeyDown={tool === "crop" ? onFrameKey : undefined}
                >
                  {tool === "crop" &&
                    CORNERS.map((c) => (
                      <span key={c} className={`pe-handle ${c}`} onPointerDown={(e) => begin(e, c)} onPointerMove={dragTo} onPointerUp={end} onPointerCancel={end} />
                    ))}
                </div>
              )}
            </div>
          </>
        )}
      </div>

      <div className="pe-tools">
        <Segmented<Tool>
          value={tool}
          onChange={setTool}
          options={[
            { value: "rotate", label: "Obróć" },
            { value: "straighten", label: "Wyprostuj" },
            { value: "crop", label: "Przytnij" },
          ]}
        />
        <div className="pe-controls">
          {tool === "rotate" && (
            <>
              <button className="lb-btn pe-outline" onClick={() => turn(-1)}>
                <RotateCcw size={15} />
                W lewo
              </button>
              <button className="lb-btn pe-outline" onClick={() => turn(1)}>
                <RotateCw size={15} />
                W prawo
              </button>
            </>
          )}
          {tool === "straighten" && (
            <>
              <input
                className="pe-slider"
                type="range"
                min={-MAX_ANGLE}
                max={MAX_ANGLE}
                step={0.1}
                value={angle}
                aria-label="Kąt prostowania"
                onChange={(e) => setAngle(roundAngle(Number(e.target.value)))}
              />
              <span className="pe-angle">{angleText(angle)}</span>
              <button className="lb-btn pe-outline" onClick={() => setAngle(0)} disabled={angle === 0}>
                Wyzeruj
              </button>
            </>
          )}
          {tool === "crop" && (
            <>
              <Segmented<"free" | "square">
                value={square ? "square" : "free"}
                onChange={(v) => chooseSquare(v === "square")}
                options={[
                  { value: "free", label: "Dowolnie" },
                  { value: "square", label: "Kwadrat" },
                ]}
              />
              <button className="lb-btn pe-outline" onClick={resetCrop} disabled={isWhole(crop) && !square}>
                Wyzeruj
              </button>
            </>
          )}
        </div>
        <span className="pe-note">Oryginał zostanie w archiwum — możesz go przywrócić.</span>
      </div>
    </div>,
    document.body,
  );
}

/** Under an edited photo: it is a new version, and the original can come back. Used in the lightbox and the detail panel. */
export function EditedNote({ id }: { id: string }) {
  const [busy, setBusy] = useState(false);
  const restore = () =>
    runEdit(async () => {
      setBusy(true);
      try {
        const result = await call<EditResult>("media.restoreOriginal", { id });
        afterChange();
        useStore.getState().notify("Przywrócono oryginał.", {
          detail: result.droppedCrops > 0 ? `Edytowana wersja została w archiwum. ${LOST_CROP}` : "Edytowana wersja została w archiwum.",
        });
      } finally {
        setBusy(false);
      }
    });
  return (
    <div className="media-section">
      <span style={{ fontSize: 13, color: "var(--text2)" }}>Edytowane zdjęcie · oryginał zachowany</span>
      <button className="btn secondary sm" style={{ alignSelf: "flex-start" }} onClick={restore} disabled={busy}>
        <Undo2 size={14} />
        Przywróć oryginał
      </button>
    </div>
  );
}
