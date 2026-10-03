// Pieces shared by the media library, the lightbox and the missing-files tool (spec §4.30, §4.31, §4.33).

import { ImageOff, Loader } from "lucide-react";
import { Fragment, useEffect, useState, type ReactNode } from "react";
import { mediaUrl, type ApiError } from "../../api/transport";
import { useStore, type Route } from "../../app/store";
import { bytes } from "../../lib/format";
import { openPath, revealPath } from "../../lib/native";

export interface MediaPerson {
  id: string;
  name: string;
  initials: string;
  branch: number;
  photo: string | null;
}

/** One photo or document as `media.list` and `media.get` return it (crates/heirloom-api/src/lists.rs, `media_json`). */
export interface MediaItem {
  id: string;
  /** Relative to the archive folder ("media/x.jpg"); null when the record has no file. */
  path: string | null;
  kind: "photo" | "document" | "other";
  /** "akt urodzenia", "list"… */
  documentType: string | null;
  title: string | null;
  /** Short: "14.02.1904", "ok. 1906". */
  date: string | null;
  /** Long: "14 lutego 1904". */
  dateLong: string | null;
  uncertain: boolean;
  sort: number | null;
  /** 1900 for 1900–1909; null when undated. */
  decade: number | null;
  place: string | null;
  transcription: string | null;
  translation: string | null;
  note: string | null;
  /** The file is described in the archive but isn't on disk. */
  missing: boolean;
  size: number | null;
  /** The full path on disk (for "Otwórz w programie" and "Pokaż w folderze"). */
  absolute: string | null;
  people: MediaPerson[];
  /** People whose profile photo this is. */
  profileOf: string[];
  format: string | null;
  sources: { id: string; title: string | null }[];
}

export const fileName = (path: string | null | undefined) => path?.split(/[\\/]/).pop() ?? "";

export const extension = (path: string | null | undefined) => {
  const name = fileName(path);
  return name.includes(".") ? (name.split(".").pop() ?? "").toUpperCase() : "";
};

/** Images the file server can show and make thumbnails of (TIFF is converted for the WebView). */
export const isImage = (path: string | null | undefined) => !!path && /\.(jpe?g|png|gif|webp|bmp|tiff?|avif)$/i.test(path);

/** Formats the WebView shows as they are, so the original can be opened in the lightbox. */
export const isWebImage = (path: string | null | undefined) => !!path && /\.(jpe?g|png|gif|webp|bmp|avif)$/i.test(path);

export const isPdf = (path: string | null | undefined) => !!path && /\.pdf$/i.test(path);

const KIND_LABEL: Record<string, string> = { photo: "zdjęcie", document: "dokument", other: "inny plik" };

/** "akt urodzenia" when the document type is known, else "zdjęcie" / "dokument" / "inny plik". */
export function kindText(item: { kind?: string; documentType?: string | null }): string {
  return item.documentType?.trim() || KIND_LABEL[item.kind ?? ""] || "plik";
}

/** File-dialog filters that offer files like `name` first ("Wskaż plik M0142.jpg"). */
export function filtersLike(name: string) {
  const ext = name.includes(".") ? name.split(".").pop() : null;
  const all = { name: "Wszystkie pliki", extensions: ["*"] };
  return ext ? [{ name: `Pliki ${ext.toUpperCase()}`, extensions: [ext.toLowerCase(), ext.toUpperCase()] }, all] : [all];
}

/** Runs a change in edit mode ("Kto edytuje?" first when needed) and shows a failure as a toast. */
export function runEdit(action: () => Promise<void>) {
  const { requireEdit, notify } = useStore.getState();
  requireEdit(() => {
    action().catch((e: ApiError) => notify(e.message, { kind: "err" }));
  });
}

export function openInProgram(absolute: string | null | undefined) {
  if (!absolute) return;
  openPath(absolute).catch(() => useStore.getState().notify("Nie można otworzyć tego pliku.", { kind: "err" }));
}

export function showInFolder(absolute: string | null | undefined) {
  if (!absolute) return;
  revealPath(absolute).catch(() => useStore.getState().notify("Nie można pokazać pliku w folderze.", { kind: "err" }));
}

/** The size of an element, kept up to date. Returns a callback ref, so it works for elements rendered later. */
export function useElementSize<T extends HTMLElement>() {
  const [element, setElement] = useState<T | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  useEffect(() => {
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize((s) => (s.width === width && s.height === height ? s : { width, height }));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [element]);
  return [setElement, size, element] as const;
}

/** A thumbnail with the tile states from the design: „Przygotowuję podgląd…” while the file server makes it (shown
 *  only when that takes a moment, so cached thumbnails don't flash it) and „Brak podglądu” when the file can't be
 *  read. The container positions the image (cover in the grid, contain in the detail panel). */
export function ThumbImage({ path, size, iconSize = 20, labels = true }: { path: string; size: number; iconSize?: number; labels?: boolean }) {
  const [state, setState] = useState<"loading" | "ok" | "error">("loading");
  const [shownPath, setShownPath] = useState(path);
  if (shownPath !== path) {
    setShownPath(path);
    setState("loading");
  }
  return (
    <>
      {state === "loading" && (
        <span className="media-pending">
          <Loader size={iconSize} className="spin" />
          {labels && <span>Przygotowuję podgląd…</span>}
        </span>
      )}
      {state === "error" ? (
        <>
          <ImageOff size={iconSize} />
          {labels && <span>Brak podglądu</span>}
        </>
      ) : (
        <img
          src={mediaUrl(path, "thumb", size)}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setState("ok")}
          onError={() => setState("error")}
          style={{ opacity: state === "ok" ? 1 : 0 }}
        />
      )}
    </>
  );
}

/** The key–value rows under a media title: Rodzaj, Data, Miejsce, Źródło, Plik (spec §4.30). Empty rows are left
 *  out, as everywhere in browse mode. `before` runs ahead of a link's navigation (the lightbox closes itself). */
export function MetaRows({
  item,
  gap = 6,
  hide = [],
  before,
}: {
  item: Partial<Omit<MediaItem, "kind">> & { kind?: string };
  gap?: number;
  hide?: ("kind" | "date" | "place")[];
  before?: () => void;
}) {
  const go = useStore((s) => s.go);
  const open = (route: Route) => {
    before?.();
    go(route);
  };
  const rows: [string, ReactNode][] = [];
  if (!hide.includes("kind")) rows.push(["Rodzaj", kindText(item)]);
  const date = item.dateLong ?? item.date;
  if (date && !hide.includes("date")) rows.push(["Data", <span className={item.uncertain ? "uncertain" : undefined}>{date}</span>]);
  const place = item.place;
  if (place && !hide.includes("place"))
    rows.push([
      "Miejsce",
      <button className="media-link" onClick={() => open({ name: "places", place })}>
        {place}
      </button>,
    ]);
  if (item.sources?.length)
    rows.push([
      item.sources.length > 1 ? "Źródła" : "Źródło",
      item.sources.map((s, i) => (
        <Fragment key={s.id}>
          {i > 0 && ", "}
          <button className="media-link" onClick={() => open({ name: "sources", id: s.id })}>
            {s.title || "Źródło bez tytułu"}
          </button>
        </Fragment>
      )),
    ]);
  if (item.path) {
    const parts = [fileName(item.path), item.size != null ? bytes(item.size) : null].filter(Boolean).join(" · ");
    rows.push([
      "Plik",
      <span title={item.absolute ?? item.path}>
        {parts}
        {item.missing && <span style={{ color: "var(--warn)" }}> · brak na dysku</span>}
      </span>,
    ]);
  }
  return (
    <div className="col" style={{ gap, fontSize: 13 }}>
      {rows.map(([key, value]) => (
        <div key={key} className="row" style={{ gap: 10, alignItems: "baseline" }}>
          <span style={{ width: 80, flex: "none", color: "var(--text3)" }}>{key}</span>
          <span style={{ minWidth: 0, overflowWrap: "anywhere" }}>{value}</span>
        </div>
      ))}
    </div>
  );
}

/** A transcription or translation: serif reading text, selectable (spec §1.5: 15 / 1.7). */
export function TextBlock({ label, text }: { label: string; text: string }) {
  return (
    <div className="col media-section" style={{ gap: 6 }}>
      <span className="label-caps">{label}</span>
      <div className="serif selectable" style={{ fontSize: 15, lineHeight: 1.7, color: "var(--text2)", whiteSpace: "pre-wrap" }}>
        {text}
      </div>
    </div>
  );
}
