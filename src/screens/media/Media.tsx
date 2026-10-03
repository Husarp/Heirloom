import { useVirtualizer } from "@tanstack/react-virtual";
import { FileText, FileX, ImagePlus, Images, LayoutGrid, List, Star } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Dropdown, EmptyState, Spinner } from "../../components/bits";
import { count, num } from "../../lib/format";
import { onFileDrop, pickFiles } from "../../lib/native";
import { Lightbox } from "./Lightbox";
import { MediaDetail } from "./MediaDetail";
import { extension, fileName, isImage, isPdf, kindText, runEdit, ThumbImage, useElementSize, type MediaItem } from "./shared";
import "./media.css";

type Sort = "date" | "name" | "added";
type View = "grid" | "list";
type Todo = "noTitle" | "noPeople" | "transcription";

interface Filters {
  /** "all", "photo", "document", "other", "pdf" or "type:<document type>". */
  kind: string;
  decade: number | "none" | null;
  todo: Todo | null;
}

type Row =
  | { type: "group"; key: string; label: string; count: number }
  | { type: "tiles"; key: string; items: MediaItem[] }
  | { type: "item"; key: string; item: MediaItem };

const ALL: Filters = { kind: "all", decade: null, todo: null };
const MIN_TILE = 128;
const GAP = 12;
/** The caption (17) and date (16) lines under a thumbnail, with their 4 px gaps. */
const TILE_TEXT = 41;
const GROUP_TITLE = 36;
const LIST_ROW = 40;
const LIST_COLUMNS = "40px minmax(0, 1fr) 96px 130px 56px";
const LIST_COLUMNS_NARROW = "40px minmax(0, 1fr) 96px";

const FILE_FILTERS = [
  { name: "Zdjęcia i dokumenty", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif", "avif", "pdf"] },
  { name: "Wszystkie pliki", extensions: ["*"] },
];

// Filters, sort and view survive leaving Media (e.g. a profile and Back) for the rest of the session.
let remembered: { filters: Filters; sort: Sort; view: View } = { filters: ALL, sort: "date", view: "grid" };

const collator = new Intl.Collator("pl", { numeric: true, sensitivity: "base" });
const typeKey = (type: string | null) => type?.trim().toLowerCase() ?? "";
const capitalize = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);
const nameOf = (m: MediaItem) => m.title?.trim() || fileName(m.path);

const KIND_LABELS: Record<string, string> = { all: "Wszystko", photo: "Zdjęcia", document: "Dokumenty", other: "Inne", pdf: "PDF" };
const TODO_LABELS: Record<Todo, string> = { noTitle: "Bez podpisu", noPeople: "Bez osób", transcription: "Z transkrypcją" };
const decadeLabel = (decade: number | "none") => (decade === "none" ? "Bez daty" : `${decade}–${decade + 9}`);

function matchKind(m: MediaItem, kind: string): boolean {
  if (kind === "all") return true;
  if (kind === "pdf") return isPdf(m.path);
  if (kind.startsWith("type:")) return typeKey(m.documentType) === kind.slice(5);
  return m.kind === kind;
}
const matchDecade = (m: MediaItem, decade: Filters["decade"]) => decade == null || (decade === "none" ? m.decade == null : m.decade === decade);
function matchTodo(m: MediaItem, todo: Todo | null): boolean {
  if (todo === "noTitle") return !m.title?.trim();
  if (todo === "noPeople") return m.people.length === 0;
  if (todo === "transcription") return !!m.transcription?.trim();
  return true;
}

interface FacetRow<T> {
  value: T;
  label: string;
  count: number;
}

/** The filter rows with their counts. Each group counts what the other groups' filters leave, so a number is what
 *  a click shows. Kinds, document types and decades the archive doesn't have are left out; Wszystko, Zdjęcia,
 *  Dokumenty and the three „do uzupełnienia” rows always show. */
function facets(items: MediaItem[], f: Filters) {
  const forKind = items.filter((m) => matchDecade(m, f.decade) && matchTodo(m, f.todo));
  const forDecade = items.filter((m) => matchKind(m, f.kind) && matchTodo(m, f.todo));
  const forTodo = items.filter((m) => matchKind(m, f.kind) && matchDecade(m, f.decade));

  const types = new Map<string, { label: string; raw: string; total: number }>();
  for (const m of items) {
    const key = typeKey(m.documentType);
    if (!key) continue;
    const known = types.get(key);
    if (known) known.total++;
    else types.set(key, { label: capitalize(m.documentType!.trim()), raw: m.documentType!.trim(), total: 1 });
  }
  const kinds = ["all", "photo", "document"];
  if (items.some((m) => m.kind === "other")) kinds.push("other");
  for (const [key] of [...types].sort((a, b) => b[1].total - a[1].total || collator.compare(a[1].label, b[1].label))) kinds.push(`type:${key}`);
  if (items.some((m) => isPdf(m.path))) kinds.push("pdf");
  if (!kinds.includes(f.kind)) kinds.push(f.kind);
  const kindLabel = (k: string) => KIND_LABELS[k] ?? types.get(k.slice(5))?.label ?? capitalize(k.slice(5));

  const decades = new Set<number>();
  let undated = false;
  for (const m of items) {
    if (m.decade == null) undated = true;
    else decades.add(m.decade);
  }
  const decadeValues: (number | "none")[] = [...decades].sort((a, b) => a - b);
  if (undated) decadeValues.push("none");
  if (f.decade != null && !decadeValues.includes(f.decade)) decadeValues.push(f.decade);

  return {
    kind: kinds.map((value): FacetRow<string> => ({ value, label: kindLabel(value), count: forKind.filter((m) => matchKind(m, value)).length })),
    decade: decadeValues.map((value): FacetRow<number | "none"> => ({ value, label: decadeLabel(value), count: forDecade.filter((m) => matchDecade(m, value)).length })),
    todo: (Object.keys(TODO_LABELS) as Todo[]).map((value): FacetRow<Todo> => ({ value, label: TODO_LABELS[value], count: forTodo.filter((m) => matchTodo(m, value)).length })),
    kindLabel,
    documentTypes: [...types.values()].map((t) => t.raw),
  };
}

/** „2 os.”, „akt”, „bez osób” in the tile's corner (spec §4.30). Documents show their type's first word. */
function tileTag(m: MediaItem): string | null {
  const type = m.documentType?.trim();
  if (type) return type.split(/\s+/)[0].toLowerCase();
  if (m.kind === "document") return isPdf(m.path) ? "PDF" : "dokument";
  return m.people.length === 0 ? "bez osób" : `${m.people.length} os.`;
}

/** Selecting keeps the id in the route (so Back from a profile returns to the same photo) but replaces the route
 *  instead of adding a history step for every click. */
function setSelected(id: string | undefined) {
  useStore.setState((s) => (s.route.name === "media" && s.route.id !== id ? { route: { name: "media", id } } : {}));
}

/** Media (spec §4.30): filters, a grid or list of every photo and document, and the detail panel. */
export function Media({ selected }: { selected?: string }) {
  const editing = useStore((s) => s.mode === "edit");
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const { data, error, loading } = useApi<{ items: MediaItem[] }>("media.list");
  const items = data?.items;

  const [filters, setFilters] = useState(remembered.filters);
  const [sort, setSort] = useState(remembered.sort);
  const [view, setView] = useState(remembered.view);
  useEffect(() => {
    remembered = { filters, sort, view };
  }, [filters, sort, view]);

  const [lightbox, setLightbox] = useState<{ items: MediaItem[]; index: number } | null>(null);
  const [adding, setAdding] = useState(0);
  const [dropHover, setDropHover] = useState(false);

  const facet = useMemo(() => facets(items ?? [], filters), [items, filters]);
  const shown = useMemo(() => {
    if (!items) return [];
    const list = items.filter((m) => matchKind(m, filters.kind) && matchDecade(m, filters.decade) && matchTodo(m, filters.todo));
    if (sort === "date") list.sort((a, b) => (a.sort ?? Infinity) - (b.sort ?? Infinity) || collator.compare(nameOf(a), nameOf(b)));
    else if (sort === "name") list.sort((a, b) => collator.compare(nameOf(a), nameOf(b)));
    else {
      // The archive keeps records in the order they were added, so the newest are last.
      const order = new Map(items.map((m, i) => [m.id, i]));
      list.sort((a, b) => order.get(b.id)! - order.get(a.id)!);
    }
    return list;
  }, [items, filters, sort]);

  const [scrollRef, scrollSize, scrollEl] = useElementSize<HTMLDivElement>();
  // 24 px on the left; on the right 14 px plus the 10 px scrollbar gutter, which is always kept (no reflow).
  const inner = Math.max(0, scrollSize.width - 38);
  const cols = Math.max(1, Math.floor((inner + GAP) / (MIN_TILE + GAP)));
  const tileW = (inner - GAP * (cols - 1)) / cols;
  const tileH = Math.round(tileW * 0.75) + TILE_TEXT;
  const thumbSize = tileW * (window.devicePixelRatio || 1) > 300 ? 512 : 256;
  const narrowList = inner < 520;

  const rows = useMemo(() => {
    // Grouped by decade when sorted by date (spec §4.30); the other sorts are one run.
    const groups = new Map<string, { label: string; items: MediaItem[] }>();
    for (const m of shown) {
      const key = sort === "date" ? String(m.decade ?? "none") : "all";
      const group = groups.get(key);
      if (group) group.items.push(m);
      else groups.set(key, { label: decadeLabel(m.decade ?? "none"), items: [m] });
    }
    const out: Row[] = [];
    for (const [key, group] of groups) {
      if (sort === "date") out.push({ type: "group", key: `g:${key}`, label: group.label, count: group.items.length });
      if (view === "list") for (const m of group.items) out.push({ type: "item", key: m.id, item: m });
      else for (let i = 0; i < group.items.length; i += cols) out.push({ type: "tiles", key: `t:${key}:${i}`, items: group.items.slice(i, i + cols) });
    }
    return out;
  }, [shown, sort, view, cols]);

  const sizeOf = (row: Row) => (row.type === "group" ? (view === "list" ? LIST_ROW : GROUP_TITLE) : row.type === "item" ? LIST_ROW : tileH + GAP);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollEl,
    estimateSize: (i) => sizeOf(rows[i]),
    getItemKey: (i) => rows[i].key,
    overscan: 4,
    paddingStart: view === "list" ? 48 : 14,
    paddingEnd: 20,
  });
  // Row heights follow the window width; nothing is measured from the DOM, so they're recalculated here.
  useLayoutEffect(() => {
    virtualizer.measure();
  }, [virtualizer, rows, tileH, view]);

  // Bring the selected item into view when it comes from outside (a profile's gallery, Back, a new file) — not
  // after a click in the grid, where it's already visible.
  const revealed = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!items || !selected || revealed.current === selected) return;
    if (!shown.some((m) => m.id === selected)) {
      // Hidden by a filter (remembered from earlier): show everything. Not there yet (just added): wait for it.
      if (filters !== ALL && items.some((m) => m.id === selected)) setFilters(ALL);
      return;
    }
    if (scrollSize.width === 0) return;
    const index = rows.findIndex((r) => (r.type === "tiles" ? r.items.some((m) => m.id === selected) : r.type === "item" && r.item.id === selected));
    if (index < 0) return;
    revealed.current = selected;
    virtualizer.scrollToIndex(index, { align: "center" });
  }, [items, shown, rows, selected, scrollSize.width, filters, virtualizer]);

  const pick = (id: string) => {
    revealed.current = id;
    setSelected(id);
  };
  const openLightbox = (item: MediaItem) => {
    const index = shown.findIndex((m) => m.id === item.id);
    setLightbox(index >= 0 ? { items: shown, index } : { items: [item], index: 0 });
  };

  const addPaths = (paths: string[]) => {
    if (!paths.length) return;
    runEdit(async () => {
      setAdding(paths.length);
      try {
        const added = await call<{ id: string; duplicate: boolean }[]>("media.add", { paths });
        afterChange();
        const fresh = added.filter((a) => !a.duplicate);
        const known = added.length - fresh.length;
        notify(
          fresh.length
            ? `Dodano ${count(fresh.length, "plik", "pliki", "plików")} do Mediów.${known ? ` ${count(known, "plik był", "pliki były", "plików było")} już w archiwum.` : ""}`
            : added.length === 1
              ? "Ten plik jest już w archiwum."
              : "Te pliki są już w archiwum.",
        );
        const first = fresh[0] ?? added[0];
        if (first) setSelected(first.id);
      } finally {
        setAdding(0);
      }
    });
  };
  const addFiles = () =>
    runEdit(async () => {
      addPaths(await pickFiles("Dodaj pliki do archiwum", FILE_FILTERS));
    });

  // Files dropped from Explorer. The listener is window-wide, so it lives only while Media is open.
  const dropped = useRef(addPaths);
  dropped.current = addPaths;
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let gone = false;
    onFileDrop((paths) => dropped.current(paths), setDropHover)
      .then((stop) => {
        if (gone) stop();
        else unlisten = stop;
      })
      .catch(() => {});
    return () => {
      gone = true;
      unlisten?.();
    };
  }, []);

  // Memoised: the virtualiser re-renders this screen on every scroll frame.
  const selectedItem = useMemo(() => items?.find((m) => m.id === selected) ?? null, [items, selected]);
  const missingCount = useMemo(() => items?.filter((m) => m.missing).length ?? 0, [items]);
  const activeLabels = [
    filters.kind !== "all" ? facet.kindLabel(filters.kind) : null,
    filters.decade != null ? decadeLabel(filters.decade) : null,
    filters.todo ? TODO_LABELS[filters.todo] : null,
  ].filter(Boolean);
  const label = activeLabels.length ? activeLabels.join(" · ") : "Wszystko";

  let middle: ReactNode = null;
  if (!data && loading) {
    middle = (
      <div className="col grow" style={{ alignItems: "center", justifyContent: "center", gap: 10, color: "var(--text3)", fontSize: 13 }}>
        <Spinner size={20} />
        Wczytuję media…
      </div>
    );
  } else if (!items) {
    middle = <EmptyState title="Nie udało się wczytać Mediów" text={error?.message} />;
  } else if (items.length === 0) {
    middle = editing ? (
      <div className="media-empty-drop">
        <ImagePlus size={26} color="var(--text3)" />
        <div className="serif" style={{ fontSize: 21, fontWeight: 600, color: "var(--text)" }}>
          Przeciągnij tu zdjęcia
        </div>
        <span>Zdjęcia, skany i pliki PDF zostaną skopiowane do folderu media/ w archiwum.</span>
        <button className="btn primary" onClick={addFiles}>
          Wybierz pliki…
        </button>
      </div>
    ) : (
      <EmptyState icon={<Images size={28} />} title="Nie ma jeszcze zdjęć ani dokumentów" text="Aby je dodać, włącz tryb edycji." />
    );
  }

  return (
    <div className="media-screen">
      <nav className="media-filters" aria-label="Filtry">
        <h1 className="serif" style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.15 }}>
          Media
        </h1>
        {items && items.length > 0 && (
          <>
            <FilterGroup label="Rodzaj" rows={facet.kind} value={filters.kind} onPick={(kind) => setFilters({ ...filters, kind: kind === filters.kind ? "all" : kind })} />
            <FilterGroup label="Dekada" rows={facet.decade} value={filters.decade} onPick={(decade) => setFilters({ ...filters, decade: decade === filters.decade ? null : decade })} />
            <FilterGroup label="Do uzupełnienia" rows={facet.todo} value={filters.todo} onPick={(todo) => setFilters({ ...filters, todo: todo === filters.todo ? null : todo })} />
          </>
        )}
      </nav>

      <section className="media-main">
        {items && items.length > 0 && (
          <div className="media-toolbar">
            <span className="grow ellipsis" style={{ color: "var(--text2)" }}>
              {label} · <b style={{ color: "var(--text)", fontWeight: 600 }}>{num(shown.length)}</b>
            </span>
            {adding > 0 && (
              <span className="row" style={{ gap: 6, color: "var(--text3)", whiteSpace: "nowrap" }}>
                <Spinner size={14} />
                Dodaję {count(adding, "plik", "pliki", "plików")}…
              </span>
            )}
            {editing && (
              <button className="btn secondary sm" style={{ fontSize: 13 }} onClick={addFiles} disabled={adding > 0}>
                <ImagePlus size={14} />
                Dodaj pliki
              </button>
            )}
            <Dropdown
              label="Sortuj:"
              value={sort}
              onChange={setSort}
              align="right"
              width={190}
              options={[
                { value: "date", label: "data zdjęcia" },
                { value: "name", label: "nazwa" },
                { value: "added", label: "ostatnio dodane" },
              ]}
            />
            <div className="seg accent media-view-switch" role="radiogroup" aria-label="Widok">
              <button className={view === "grid" ? "on" : ""} role="radio" aria-checked={view === "grid"} title="Siatka" onClick={() => setView("grid")}>
                <LayoutGrid size={14} />
              </button>
              <button className={view === "list" ? "on" : ""} role="radio" aria-checked={view === "list"} title="Lista" onClick={() => setView("list")}>
                <List size={14} />
              </button>
            </div>
          </div>
        )}
        {missingCount > 0 && (
          <div style={{ padding: "12px 24px 0", flex: "none" }}>
            <div className="banner warn" style={{ padding: "10px 10px 10px 14px" }}>
              <FileX size={16} color="var(--warn)" style={{ flex: "none" }} />
              <span className="grow">Brakuje {count(missingCount, "pliku", "plików", "plików")} w folderze archiwum.</span>
              <button className="btn secondary sm" onClick={() => go({ name: "missingFiles" })}>
                Brakujące pliki…
              </button>
            </div>
          </div>
        )}
        {middle}
        {items && items.length > 0 && shown.length === 0 && (
          <EmptyState
            title="Nic tu nie pasuje"
            text="Żaden plik nie spełnia wybranych filtrów."
            action={
              <button className="link" onClick={() => setFilters(ALL)}>
                Pokaż wszystko
              </button>
            }
          />
        )}
        {items && shown.length > 0 && (
          <div ref={scrollRef} className="media-scroll">
            <div className="media-sizer" style={{ height: virtualizer.getTotalSize() }}>
              {view === "list" && (
                <>
                  <div className="media-list-frame" style={{ top: 14, bottom: 20 }} />
                  <div className="media-list-head" style={{ gridTemplateColumns: narrowList ? LIST_COLUMNS_NARROW : LIST_COLUMNS }}>
                    <span />
                    <span>Podpis</span>
                    <span>Data</span>
                    {!narrowList && <span>Rodzaj</span>}
                    {!narrowList && <span>Osoby</span>}
                  </div>
                </>
              )}
              {virtualizer.getVirtualItems().map((v) => {
                const row = rows[v.index];
                const list = view === "list";
                const last = list && v.index === rows.length - 1;
                return (
                  <div
                    key={v.key}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: list ? 1 : 0,
                      right: list ? 1 : 0,
                      height: v.size,
                      transform: `translateY(${v.start}px)`,
                      ...(last ? { borderRadius: "0 0 7px 7px", overflow: "hidden" } : null),
                    }}
                  >
                    {row.type === "group" ? (
                      view === "list" ? (
                        <div className="media-list-group">
                          {row.label} <span className="count">{num(row.count)}</span>
                        </div>
                      ) : (
                        <div className="media-group-title">
                          {row.label} <span className="count">{num(row.count)}</span>
                        </div>
                      )
                    ) : row.type === "tiles" ? (
                      <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`, gap: GAP }}>
                        {row.items.map((m) => (
                          <Tile key={m.id} item={m} selected={m.id === selected} thumbSize={thumbSize} onSelect={() => pick(m.id)} onOpen={() => openLightbox(m)} />
                        ))}
                      </div>
                    ) : (
                      <ListRow
                        item={row.item}
                        selected={row.item.id === selected}
                        columns={narrowList ? LIST_COLUMNS_NARROW : LIST_COLUMNS}
                        narrow={narrowList}
                        onSelect={() => pick(row.item.id)}
                        onOpen={() => openLightbox(row.item)}
                      />
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}
        {dropHover && (
          <div className="media-drop">
            <ImagePlus size={28} />
            Upuść pliki, aby dodać je do archiwum
          </div>
        )}
      </section>

      <MediaDetail
        key={selectedItem?.id ?? "none"}
        item={selectedItem}
        documentTypes={facet.documentTypes}
        onPreview={() => selectedItem && openLightbox(selectedItem)}
        onDeleted={() => setSelected(undefined)}
      />

      {lightbox && (
        <Lightbox
          items={lightbox.items}
          index={lightbox.index}
          onClose={() => setLightbox(null)}
          title={activeLabels.length && lightbox.items.length > 1 ? `Media: ${label}` : "Media"}
        />
      )}
    </div>
  );
}

function FilterGroup<T extends string | number>({ label, rows, value, onPick }: { label: string; rows: FacetRow<T>[]; value: T | null; onPick: (value: T) => void }) {
  return (
    <div className="media-filter-group" role="group" aria-label={label}>
      <span className="label-caps">{label}</span>
      {rows.map((r) => (
        <button key={String(r.value)} className={`media-filter${r.value === value ? " on" : ""}`} aria-pressed={r.value === value} onClick={() => onPick(r.value)}>
          <span className="grow ellipsis">{r.label}</span>
          <span className="count">{num(r.count)}</span>
        </button>
      ))}
    </div>
  );
}

/** What fills a thumbnail: the image, or an icon for documents and missing files. */
function Preview({ item, size, small }: { item: MediaItem; size: number; small?: boolean }) {
  const icon = small ? 14 : 20;
  if (item.missing || !item.path) {
    return (
      <>
        <FileX size={icon} />
        {!small && <span>Brak pliku</span>}
      </>
    );
  }
  if (!isImage(item.path)) {
    return (
      <>
        <FileText size={icon} />
        {!small && <span>{extension(item.path)}</span>}
      </>
    );
  }
  return <ThumbImage path={item.path} size={size} iconSize={icon} labels={!small} />;
}

function Tile({ item, selected, thumbSize, onSelect, onOpen }: { item: MediaItem; selected: boolean; thumbSize: number; onSelect: () => void; onOpen: () => void }) {
  const tag = tileTag(item);
  const caption = nameOf(item) || "Bez podpisu";
  const profileOf = item.people.filter((p) => item.profileOf.includes(p.id)).map((p) => p.name);
  return (
    <button className={`media-tile${selected ? " selected" : ""}`} aria-pressed={selected} title={caption} onClick={onSelect} onDoubleClick={onOpen}>
      <span className={`media-thumb${item.missing || !item.path ? " missing" : ""}`}>
        <Preview item={item} size={thumbSize} />
        {tag && <span className="media-tag">{tag}</span>}
        {item.profileOf.length > 0 && (
          <span className="media-star" title={`Zdjęcie profilowe: ${profileOf.join(", ")}`}>
            <Star size={11} fill="currentColor" />
          </span>
        )}
      </span>
      <span className="media-caption ellipsis" style={item.title?.trim() ? undefined : { color: "var(--text3)" }}>
        {caption}
      </span>
      <span className="media-date">{item.date ? <span className={item.uncertain ? "uncertain" : undefined}>{item.date}</span> : " "}</span>
    </button>
  );
}

function ListRow({
  item,
  selected,
  columns,
  narrow,
  onSelect,
  onOpen,
}: {
  item: MediaItem;
  selected: boolean;
  columns: string;
  narrow: boolean;
  onSelect: () => void;
  onOpen: () => void;
}) {
  return (
    <button className={`media-list-row${selected ? " selected" : ""}`} style={{ gridTemplateColumns: columns }} aria-pressed={selected} onClick={onSelect} onDoubleClick={onOpen}>
      <span className={`media-thumb${item.missing || !item.path ? " missing" : ""}`}>
        <Preview item={item} size={128} small />
      </span>
      <span className="row" style={{ gap: 6, minWidth: 0 }}>
        {item.profileOf.length > 0 && <Star size={12} fill="currentColor" color="var(--accent-text)" style={{ flex: "none" }} aria-label="zdjęcie profilowe" />}
        <span className="ellipsis" style={item.title?.trim() ? undefined : { color: "var(--text3)" }}>
          {nameOf(item) || "Bez podpisu"}
        </span>
      </span>
      <span className="num ellipsis" style={{ color: item.date ? "var(--text2)" : "var(--text3)" }}>
        {item.date ? <span className={item.uncertain ? "uncertain" : undefined}>{item.date}</span> : "—"}
      </span>
      {!narrow && (
        <span className="ellipsis" style={{ color: "var(--text2)" }}>
          {kindText(item)}
        </span>
      )}
      {!narrow && (
        <span className="num" style={{ color: item.people.length ? "var(--text2)" : "var(--text3)" }}>
          {item.people.length ? `${item.people.length} os.` : "—"}
        </span>
      )}
    </button>
  );
}
