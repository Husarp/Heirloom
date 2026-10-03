// Miejsca (spec §4.27): every place written in the archive as a tree built from the written form („Wólka, parafia
// Łęczna, powiat lubelski” → powiat lubelski › parafia Łęczna › Wólka), and a page per place with the events there.
// No map in v1, only „Otwórz w mapach” (DESIGNER_ANSWERS §3).

import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronDown, ChevronRight, CircleHelp, GitFork, GitMerge, Library, MapIcon, MapPin, Search, Users } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, EmptyState, Segmented, Spinner, Tabs } from "../../components/bits";
import { count, num, people } from "../../lib/format";
import { openUrl } from "../../lib/native";
import { SuggestionBar, collator, fold, useCrumb, usePaneRows } from "../surnames/split";
import { buildTree, linesFor, parsePlace, written, type Line, type Node, type PlaceRow, type View } from "./placeTree";
import "./places.css";

interface PlaceList {
  places: PlaceRow[];
  without: number;
  total: number;
}

type Kind = "born" | "baptised" | "married" | "died" | "buried" | "lived";

interface PlaceEvent {
  date: string | null;
  uncertain: boolean;
  person: { id: string; name: string; initials: string; branch: number; photo: string | null };
  relation: string;
  source: string | null;
}

interface PlaceData {
  name: string;
  path: string[];
  groups: Partial<Record<Kind, PlaceEvent[]>>;
  similar: { path: string[]; name: string; count: number }[];
  children: string[];
  people: number;
  events: number;
}

const KINDS: [Kind, string][] = [
  ["born", "Urodzeni"],
  ["baptised", "Ochrzczeni"],
  ["married", "Śluby"],
  ["died", "Zmarli"],
  ["buried", "Pochowani"],
  ["lived", "Mieszkali"],
];

const events = (n: number) => count(n, "zdarzenie", "zdarzenia", "zdarzeń");

export function Places({ selected }: { selected?: string }) {
  const { data, error } = useApi<PlaceList>("places.list");
  const go = useStore((s) => s.go);
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>("admin");
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  // Lifted here so the chosen kind („Zmarli”…) stays when moving between places.
  const [tab, setTab] = useState<Kind>("born");
  const listRef = useRef<HTMLDivElement>(null);
  const paneRef = useRef<HTMLDivElement>(null);
  const q = fold(query.trim());

  const tree = useMemo(() => buildTree(data?.places ?? []), [data]);
  const path = parsePlace(selected) ?? tree.roots[0]?.path ?? null;
  const key = path ? written(path) : null;
  const lines = useMemo(() => linesFor(tree, view, expanded, q), [tree, view, expanded, q]);
  const rows = useVirtualizer({ count: lines.length, getScrollElement: () => listRef.current, estimateSize: () => 40, overscan: 10 });

  useCrumb(path ? path[path.length - 1] : null);

  // Open the places above the selected one (a link from a profile can point deep into the tree), and start the new
  // place's page at the top.
  useEffect(() => {
    if (!path) return;
    const above = path.slice(0, -1).map((_, i) => written(path.slice(0, i + 1)));
    setExpanded((prev) => (above.every((k) => prev.has(k)) ? prev : new Set([...prev, ...above])));
    paneRef.current?.scrollTo({ top: 0 });
    // `path` follows `key`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  // Bring the selected place into view once, when its row exists.
  const scrolledTo = useRef<string | null>(null);
  useEffect(() => {
    if (!key || scrolledTo.current === key) return;
    const index = lines.findIndex((l) => l.node.key === key);
    if (index < 0) return;
    scrolledTo.current = key;
    rows.scrollToIndex(index, { align: "auto" });
  }, [key, lines, rows]);

  const open = (placePath: string[]) => go({ name: "places", place: written(placePath) });
  const select = (node: Node) => {
    open(node.path);
    // Choosing a place also shows what is inside it.
    if (!node.leaf && !expanded.has(node.key)) setExpanded(new Set(expanded).add(node.key));
  };
  const toggle = (node: Node) => {
    const next = new Set(expanded);
    if (next.has(node.key)) next.delete(node.key);
    else next.add(node.key);
    setExpanded(next);
  };

  if (error) {
    return (
      <div className="page">
        <EmptyState icon={<MapPin size={28} />} title="Nie udało się wczytać miejsc" text={error.message} />
      </div>
    );
  }
  if (data && data.places.length === 0) {
    return (
      <div className="page">
        <EmptyState
          icon={<MapPin size={28} />}
          title="Nie ma jeszcze miejsc"
          text="Miejsca pojawią się tu, gdy przy urodzeniach, ślubach, zgonach i innych zdarzeniach będą zapisane miejscowości."
        />
      </div>
    );
  }

  return (
    <div className="split places">
      <div className="split-list">
        <div className="split-list-main">
          <div className="split-head">
            <h1 className="split-title">
              Miejsca <span className="count">{num(data?.total ?? 0)}</span>
            </h1>
            <label className="search-box">
              <Search size={15} />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Szukaj miejsca…" />
            </label>
            <div className="row">
              <Segmented
                variant="neutral"
                size={26}
                value={view}
                onChange={setView}
                options={[
                  { value: "admin", label: "Administracyjnie" },
                  { value: "parish", label: "Parafie" },
                  { value: "az", label: "A–Z" },
                ]}
              />
            </div>
          </div>
          <div ref={listRef} className="split-scroll">
            {!data && (
              <div className="split-loading">
                <Spinner />
              </div>
            )}
            <div className="split-rows" style={{ height: rows.getTotalSize() }}>
              {rows.getVirtualItems().map((item) => {
                const line = lines[item.index];
                return (
                  <PlaceLine
                    key={line.node.key}
                    line={line}
                    tree={view !== "az"}
                    on={line.node.key === key}
                    top={item.start}
                    searching={q !== ""}
                    onSelect={select}
                    onToggle={toggle}
                  />
                );
              })}
            </div>
            {data && lines.length === 0 && (
              <div className="split-note">
                {q ? `Żadne miejsce nie pasuje do „${query.trim()}”.` : "W zapisanych miejscach nie ma parafii (nazw zaczynających się od „parafia” albo „par.”)."}
              </div>
            )}
            {data && data.without > 0 && !q && (
              <div className="pl-row static" style={{ paddingLeft: 20 }} title="Osoby, przy których nie zapisano żadnego miejsca">
                <span className="icon">
                  <CircleHelp size={14} />
                </span>
                <span className="name" style={{ fontWeight: 600 }}>
                  Bez przypisanego miejsca
                </span>
                <b className="n num">{num(data.without)}</b>
              </div>
            )}
          </div>
        </div>
      </div>
      <div ref={paneRef} className="split-detail">
        {path && <PlaceDetail path={path} pane={paneRef} tab={tab} onTab={setTab} onOpen={open} />}
      </div>
    </div>
  );
}

function PlaceLine({
  line,
  tree,
  on,
  top,
  searching,
  onSelect,
  onToggle,
}: {
  line: Line;
  tree: boolean;
  on: boolean;
  top: number;
  searching: boolean;
  onSelect: (node: Node) => void;
  onToggle: (node: Node) => void;
}) {
  const { node, depth, open, note } = line;
  return (
    <button
      className={`pl-row${on ? " on" : ""}`}
      style={{ paddingLeft: 20 + depth * 18, transform: `translateY(${top}px)` }}
      onClick={() => onSelect(node)}
      aria-current={on || undefined}
    >
      {tree && !node.leaf ? (
        <span
          className="icon pl-toggle"
          title={searching ? undefined : open ? "Zwiń" : "Rozwiń"}
          onClick={(e) => {
            // The chevron only folds; the rest of the row selects.
            e.stopPropagation();
            if (!searching) onToggle(node);
          }}
        >
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </span>
      ) : (
        <span className="icon">
          <MapPin size={14} />
        </span>
      )}
      <span className="name" style={{ fontWeight: tree && depth < 2 ? 600 : 400 }}>
        {node.name}
      </span>
      {note && <span className="type">{note}</span>}
      <b className="n num">{num(node.count)}</b>
    </button>
  );
}

function PlaceDetail({
  path,
  pane,
  tab,
  onTab,
  onOpen,
}: {
  path: string[];
  pane: RefObject<HTMLDivElement | null>;
  tab: Kind;
  onTab: (kind: Kind) => void;
  onOpen: (path: string[]) => void;
}) {
  const { data, error, fresh } = useApi<PlaceData>("place.get", { path });
  if (error) {
    return <EmptyState icon={<MapPin size={28} />} title="Nie ma takiego miejsca" text="Mogło zostać scalone z innym albo przemianowane. Wybierz miejsce z listy." />;
  }
  if (!data) {
    return (
      <div className="split-loading">
        <Spinner />
      </div>
    );
  }
  return <PlacePage data={data} stale={!fresh} pane={pane} tab={tab} onTab={onTab} onOpen={onOpen} />;
}

function PlacePage({
  data,
  stale,
  pane,
  tab,
  onTab,
  onOpen,
}: {
  data: PlaceData;
  stale: boolean;
  pane: RefObject<HTMLDivElement | null>;
  tab: Kind;
  onTab: (kind: Kind) => void;
  onOpen: (path: string[]) => void;
}) {
  const go = useStore((s) => s.go);
  const mode = useStore((s) => s.mode);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const undo = useStore((s) => s.undo);
  const [busy, setBusy] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  // Only the kinds that happen here; the chosen one stays if this place has it.
  const kinds = KINDS.filter(([k]) => (data.groups[k]?.length ?? 0) > 0);
  const current = kinds.some(([k]) => k === tab) ? tab : (kinds[0]?.[0] ?? tab);
  const list = data.groups[current] ?? [];
  const virtual = usePaneRows(list.length, 44, pane, listRef);
  const here = written(data.path);
  const parent = written(data.path.slice(0, -1));
  const sourced = KINDS.reduce((n, [k]) => n + (data.groups[k]?.filter((e) => e.source).length ?? 0), 0);
  const children = [...data.children].sort(collator.compare);

  // „Scal”: every place written like the similar one gets this place's name (an ordinary, undoable change).
  const merge = (from: string[], name: string) =>
    requireEdit(async () => {
      setBusy(true);
      try {
        await call("place.rename", { from, to: data.name });
        afterChange();
        notify(`Scalono „${name}” z „${data.name}”.`, {
          action: { label: "Cofnij", run: () => void undo().catch((e: Error) => notify(e.message, { kind: "err" })) },
        });
      } catch (e) {
        notify((e as Error).message, { kind: "err" });
      } finally {
        setBusy(false);
      }
    });

  return (
    <div className="pl-detail" style={stale ? { opacity: 0.6 } : undefined}>
      <div className="pl-path">
        {[...data.path].reverse().map((part, i) => (
          <Fragment key={i}>
            {i > 0 && <ChevronRight size={12} className="sep" />}
            {i === 0 ? <b>{part}</b> : <button onClick={() => onOpen(data.path.slice(0, data.path.length - i))}>{part}</button>}
          </Fragment>
        ))}
      </div>

      <div className="pl-title">
        <h2>{data.name}</h2>
        <button className="btn secondary" onClick={() => void openUrl(`https://www.openstreetmap.org/search?query=${encodeURIComponent(data.name)}`)}>
          <MapIcon size={15} />
          Otwórz w mapach ↗
        </button>
      </div>

      {data.similar.map((s) => {
        // place.rename only renames the last part, so it merges only places under the same parent.
        const sameParent = written(s.path.slice(0, -1)) === parent;
        return (
          <SuggestionBar
            key={written(s.path)}
            icon={<GitMerge size={15} />}
            actions={
              <>
                <button className="btn secondary" onClick={() => onOpen(s.path)}>
                  Porównaj
                </button>
                {mode === "edit" && sameParent && (
                  <button className="btn primary" disabled={busy} onClick={() => merge(s.path, s.name)}>
                    Scal
                  </button>
                )}
              </>
            }
          >
            Podobne miejsce: <b>{s.name}</b>
            {!sameParent && s.path.length > 1 && `, ${written(s.path.slice(0, -1))}`} ({events(s.count)}). Może to to samo miejsce.
          </SuggestionBar>
        );
      })}

      <div className="pl-cards">
        <div className="pl-card">
          <span className="label-caps" style={{ paddingBottom: 6 }}>
            Nazwy w czasie
          </span>
          <div className="pl-period">
            <b>w archiwum</b>
            <span>{here}</span>
          </div>
        </div>
        <div className="pl-card pl-facts">
          <span className="label-caps">Położenie i źródła</span>
          <span className="pl-fact">
            <Users size={14} />
            <span>
              {people(data.people)} · {events(data.events)}
            </span>
          </span>
          {sourced > 0 && (
            <span className="pl-fact">
              <Library size={14} />
              <span>
                Ze źródłem: {num(sourced)} z {num(data.events)} {data.events === 1 ? "zdarzenia" : "zdarzeń"}
              </span>
            </span>
          )}
          {children.length > 0 && (
            <span className="pl-fact">
              <GitFork size={14} />
              <span>
                Podrzędne:{" "}
                {children.map((c, i) => (
                  <Fragment key={c}>
                    {i > 0 && ", "}
                    <button className="link" onClick={() => onOpen([...data.path, c])}>
                      {c}
                    </button>
                  </Fragment>
                ))}
              </span>
            </span>
          )}
        </div>
      </div>

      {kinds.length > 0 && <Tabs tabs={kinds.map(([k, label]) => ({ value: k, label: `${label} ${num(data.groups[k]?.length ?? 0)}` }))} value={current} onChange={onTab} />}
      <div ref={listRef} className="split-rows" style={{ height: virtual.getTotalSize() }}>
        {virtual.getVirtualItems().map((item) => {
          const e = list[item.index];
          // Without parents or a maiden name the API describes a person by their birthplace, which is this place.
          const relation = e.relation === here ? "" : e.relation;
          return (
            <button
              key={item.key}
              className="pl-event"
              style={{ transform: `translateY(${item.start - virtual.options.scrollMargin}px)` }}
              onClick={() => go({ name: "person", id: e.person.id })}
            >
              <span className={`num ellipsis${e.uncertain ? " uncertain" : ""}`} style={{ color: "var(--text2)" }} title={e.date ?? undefined}>
                {e.date ?? "?"}
              </span>
              <Avatar initials={e.person.initials} branch={e.person.branch} photo={e.person.photo} size={28} />
              <span className="ellipsis">
                <span className="serif" style={{ fontSize: 15, fontWeight: 600 }}>
                  {e.person.name}
                </span>
                {relation && <span style={{ fontSize: 13, color: "var(--text3)" }}> {relation}</span>}
              </span>
              <span className="source ellipsis" title={e.source ?? undefined}>
                {e.source}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
