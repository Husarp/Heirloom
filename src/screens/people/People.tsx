import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDown,
  ArrowUp,
  Bookmark,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Columns3,
  Copy,
  Group,
  HeartPulse,
  Image as ImageIcon,
  LayoutGrid,
  Plus,
  Search,
  Signature,
  Table2,
  Trash2,
  UserPlus,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { AzRail, Avatar, BranchShape, DateBit, rowButton, Segmented, useDismiss } from "../../components/bits";
import { cardYears, count, num, relativeTime, roman } from "../../lib/format";
import "./people.css";

interface Group {
  key: string;
  name: string;
  plural: string;
  branch: number;
  born: number;
  married: number;
}

interface ListData {
  people: PersonSummary[];
  groups: Group[];
  memberships: { birth: string | null; married: string[] }[];
  generations: number;
}

type ColumnKey = "birth" | "birthPlace" | "death" | "deathPlace" | "generation" | "branch" | "photos" | "changed" | "created" | "age" | "sex";

const COLUMNS: { key: ColumnKey; label: string; width: string; align?: "center" | "right" }[] = [
  { key: "birth", label: "Urodzenie", width: "96px" },
  { key: "birthPlace", label: "Miejsce urodzenia", width: "150px" },
  { key: "death", label: "Zgon", width: "96px" },
  { key: "deathPlace", label: "Miejsce zgonu", width: "140px" },
  { key: "generation", label: "Pok.", width: "44px", align: "center" },
  { key: "branch", label: "Gałąź", width: "120px" },
  { key: "photos", label: "Zdjęcia", width: "60px", align: "right" },
  { key: "changed", label: "Zmieniono", width: "110px" },
  { key: "created", label: "Dodano", width: "110px" },
  { key: "age", label: "Wiek", width: "60px", align: "right" },
  { key: "sex", label: "Płeć", width: "70px" },
];
const DEFAULT_COLUMNS: ColumnKey[] = ["birth", "birthPlace", "death", "generation", "branch", "photos", "changed"];

type SortKey = "name" | ColumnKey;
type GroupBy = "none" | "surname" | "generation" | "branch" | "birthPlace" | "decade";
type Living = "living" | "dead" | "all";
type MarriedWomen = "both" | "birth" | "current";

interface Filters {
  query: string;
  living: Living;
  hasPhoto: boolean;
  noBirth: boolean;
  place: string | null;
  surnames: string[];
}

interface Row {
  kind: "group" | "person";
  key: string;
  group?: { label: string; count: number; branch: number; collapsed: boolean; id: string };
  person?: PersonSummary;
  alsoIn?: string[];
}

function fold(text: string): string {
  return text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/ł/g, "l");
}

function loadLocal<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function saveLocal(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Only a convenience (column choice, saved filters).
  }
}

const EMPTY_FILTERS: Filters = { query: "", living: "all", hasPhoto: false, noBirth: false, place: null, surnames: [] };

/** Osoby (spec §4.9): a virtualised table with fixed 40 px rows, filters, grouping, columns, A–Z and a photo grid. */
export function People() {
  const { data } = useApi<ListData>("people.list");
  const archive = useStore((s) => s.archive);
  const go = useStore((s) => s.go);
  const mode = useStore((s) => s.mode);
  const requireEdit = useStore((s) => s.requireEdit);
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS);
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "name", dir: 1 });
  const [groupBy, setGroupBy] = useState<GroupBy>("surname");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [columns, setColumns] = useState<ColumnKey[]>(() => loadLocal("people.columns", DEFAULT_COLUMNS));
  const [view, setView] = useState<"table" | "grid">("table");
  const display = archive?.display ?? {};
  const marriedWomen = (display.marriedWomen as MarriedWomen) ?? "both";
  const maidenStyle = (display.maidenStyle as string) ?? "zd";
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => saveLocal("people.columns", columns), [columns]);

  const groupsByKey = useMemo(() => new Map((data?.groups ?? []).map((g) => [g.key, g])), [data]);

  const filtered = useMemo(() => {
    if (!data) return [] as { p: PersonSummary; index: number }[];
    const q = fold(filters.query.trim());
    return data.people
      .map((p, index) => ({ p, index }))
      .filter(({ p, index }) => {
        if (q && !fold(`${p.name} ${p.maiden ?? ""} ${p.nickname ?? ""} ${p.given} ${p.surname}`).includes(q)) return false;
        if (filters.living === "living" && !p.living) return false;
        if (filters.living === "dead" && p.living) return false;
        if (filters.hasPhoto && p.photoCount === 0) return false;
        if (filters.noBirth && p.birth) return false;
        if (filters.place && (p.birthPlace ?? "").split(",")[0].trim() !== filters.place) return false;
        if (filters.surnames.length) {
          const m = data.memberships[index];
          const keys = [m.birth, ...m.married].filter(Boolean) as string[];
          if (!keys.some((k) => filters.surnames.includes(k))) return false;
        }
        return true;
      });
  }, [data, filters]);

  const sorted = useMemo(() => {
    const value = (p: PersonSummary): string | number => {
      switch (sort.key) {
        case "name":
          return fold(`${p.surname} ${p.given}`);
        case "birth":
          return p.birth?.sort ?? Number.MAX_SAFE_INTEGER;
        case "death":
          return p.death?.sort ?? Number.MAX_SAFE_INTEGER;
        case "birthPlace":
          return fold(p.birthPlace ?? "~");
        case "deathPlace":
          return fold(p.deathPlace ?? "~");
        case "generation":
          return p.generation ?? 999;
        case "branch":
          return fold(p.branchName || "~");
        case "photos":
          return -p.photoCount;
        case "changed":
          return p.changed ? -Date.parse(p.changed) : 0;
        case "created":
          return p.created ? -Date.parse(p.created) : 0;
        case "age":
          return ageOf(p) ?? 999;
        case "sex":
          return p.sex;
      }
    };
    // One key per person: folding the names inside the comparison took about a second for 10 000 people.
    const keyed = filtered.map((x) => ({ x, v: value(x.p) }));
    keyed.sort((a, b) => (a.v < b.v ? -1 : a.v > b.v ? 1 : 0) * sort.dir);
    return keyed.map((k) => k.x);
  }, [filtered, sort]);

  const rows: Row[] = useMemo(() => {
    if (!data) return [];
    if (groupBy === "none") return sorted.map(({ p }) => ({ kind: "person", key: p.id, person: p }));
    const buckets = new Map<string, { label: string; branch: number; people: { p: PersonSummary; alsoIn?: string[] }[]; order: string }>();
    const add = (id: string, label: string, branch: number, order: string, p: PersonSummary, alsoIn?: string[]) => {
      let b = buckets.get(id);
      if (!b) {
        b = { label, branch, people: [], order };
        buckets.set(id, b);
      }
      b.people.push({ p, alsoIn });
    };
    for (const { p, index } of sorted) {
      switch (groupBy) {
        case "surname":
        case "branch": {
          const m = data.memberships[index];
          const birthGroup = m.birth ? groupsByKey.get(m.birth) : undefined;
          const marriedGroups = m.married.map((k) => groupsByKey.get(k)).filter(Boolean) as Group[];
          const label = (g?: Group) => g?.plural ?? "(bez nazwiska)";
          const targets: Group[] =
            groupBy === "branch" || marriedWomen === "birth"
              ? birthGroup
                ? [birthGroup]
                : []
              : marriedWomen === "current"
                ? marriedGroups.length
                  ? [marriedGroups[marriedGroups.length - 1]]
                  : birthGroup
                    ? [birthGroup]
                    : []
                : [...(birthGroup ? [birthGroup] : []), ...marriedGroups];
          if (targets.length === 0) add("~none", "(bez nazwiska)", 12, "~", p);
          for (const g of targets) {
            const others = targets.filter((t) => t !== g).map((t) => t.plural);
            add(g.key, label(g), g.branch, fold(g.plural), p, others.length ? others : undefined);
          }
          break;
        }
        case "generation":
          add(`g${p.generation ?? 0}`, p.generation ? `Pokolenie ${roman(p.generation)}` : "Bez pokolenia", 12, String(p.generation ?? 999).padStart(4, "0"), p);
          break;
        case "birthPlace": {
          const place = (p.birthPlace ?? "").split(",")[0].trim();
          add(place || "~", place || "Bez miejsca urodzenia", 12, place ? fold(place) : "~", p);
          break;
        }
        case "decade": {
          const year = p.birth?.sort ? Math.floor(p.birth.sort / 100000) : null;
          const decade = year != null ? Math.floor(year / 10) * 10 : null;
          add(decade != null ? `d${decade}` : "~", decade != null ? `${decade}–${decade + 9}` : "Bez daty urodzenia", 12, decade != null ? String(decade).padStart(5, "0") : "~", p);
          break;
        }
      }
    }
    // Surnames A–Z in Polish order, to match the A–Z rail („(bez nazwiska)” last); branches by size, like their colours.
    const ordered = [...buckets.entries()].sort((a, b) =>
      groupBy === "surname"
        ? Number(a[0] === "~none") - Number(b[0] === "~none") || a[1].label.localeCompare(b[1].label, "pl")
        : groupBy === "branch"
          ? b[1].people.length - a[1].people.length || a[1].order.localeCompare(b[1].order)
          : a[1].order.localeCompare(b[1].order),
    );
    const out: Row[] = [];
    for (const [id, b] of ordered) {
      const isCollapsed = collapsed.has(id);
      out.push({ kind: "group", key: `group:${id}`, group: { label: b.label, count: b.people.length, branch: b.branch, collapsed: isCollapsed, id } });
      if (!isCollapsed) for (const { p, alsoIn } of b.people) out.push({ kind: "person", key: `${id}:${p.id}`, person: p, alsoIn });
    }
    return out;
  }, [data, sorted, groupBy, collapsed, groupsByKey, marriedWomen]);

  const rowHeight = document.documentElement.classList.contains("compact") ? 32 : 40;
  const virtualizer = useVirtualizer({ count: rows.length, getScrollElement: () => scrollRef.current, estimateSize: () => rowHeight, overscan: 12 });

  const letters = useMemo(() => {
    const set = new Set<string>();
    for (const r of rows) {
      const text = r.kind === "group" ? r.group!.label : r.person!.surname;
      const first = text.trim().charAt(0).toUpperCase();
      if (first) set.add(first);
    }
    return set;
  }, [rows]);

  const jump = (letter: string) => {
    const index = rows.findIndex((r) => (r.kind === "group" ? r.group!.label : groupBy === "none" ? r.person!.surname : "").toUpperCase().startsWith(letter));
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "start" });
  };

  const places = useMemo(() => {
    const map = new Map<string, number>();
    for (const p of data?.people ?? []) {
      const place = (p.birthPlace ?? "").split(",")[0].trim();
      if (place) map.set(place, (map.get(place) ?? 0) + 1);
    }
    return [...map.entries()].sort((a, b) => b[1] - a[1]);
  }, [data]);

  const shown = columns.map((k) => COLUMNS.find((c) => c.key === k)!).filter(Boolean);
  const template = `28px minmax(0,1fr) ${shown.map((c) => c.width).join(" ")}`;
  const total = data?.people.length ?? 0;
  const matching = filtered.length;

  const setSortKey = (key: SortKey) => setSort((s) => (s.key === key ? { key, dir: (s.dir * -1) as 1 | -1 } : { key, dir: 1 }));

  return (
    <div className="people">
      <div className="row people-head">
        <div className="col" style={{ gap: 4 }}>
          <h1 className="page-title">Osoby</h1>
          <span style={{ fontSize: 13, color: "var(--text2)" }}>
            {matching === total ? count(total, "osoba", "osoby", "osób") : `${num(matching)} z ${count(total, "osoby", "osób", "osób")} pasuje do filtrów`}
          </span>
        </div>
        <span className="grow" />
        {mode === "edit" && (
          <button className="btn secondary" style={{ height: 34 }} onClick={() => requireEdit(() => go({ name: "edit", id: null }))}>
            <UserPlus size={15} />
            Dodaj osobę
          </button>
        )}
        <div className="search-box" style={{ width: 260 }}>
          <Search size={15} />
          <input value={filters.query} onChange={(e) => setFilters({ ...filters, query: e.target.value })} placeholder="Szukaj na liście…" />
          {filters.query && (
            <button onClick={() => setFilters({ ...filters, query: "" })} aria-label="Wyczyść">
              <X size={14} />
            </button>
          )}
        </div>
        <ColumnPicker columns={columns} setColumns={setColumns} />
        <Segmented
          value={view}
          onChange={setView}
          options={[
            { value: "table", label: "", icon: <Table2 size={15} />, title: "Tabela" },
            { value: "grid", label: "", icon: <LayoutGrid size={15} />, title: "Siatka zdjęć" },
          ]}
        />
      </div>
      <div className="row people-filters">
        {filters.surnames.length > 0 && (
          <span className="chip on filter-chip">
            <span style={{ color: "var(--text2)" }}>Nazwisko:</span>
            <b>{filters.surnames.map((k) => groupsByKey.get(k)?.plural ?? k).join(", ")}</b>
            <button onClick={() => setFilters({ ...filters, surnames: [] })} aria-label="Usuń filtr">
              <X size={13} />
            </button>
          </span>
        )}
        {filters.place && (
          <span className="chip on filter-chip">
            <span style={{ color: "var(--text2)" }}>Miejsce:</span>
            <b>{filters.place}</b>
            <button onClick={() => setFilters({ ...filters, place: null })} aria-label="Usuń filtr">
              <X size={13} />
            </button>
          </span>
        )}
        {filters.hasPhoto && (
          <span className="chip on filter-chip">
            <b>Ma zdjęcie</b>
            <button onClick={() => setFilters({ ...filters, hasPhoto: false })} aria-label="Usuń filtr">
              <X size={13} />
            </button>
          </span>
        )}
        {filters.noBirth && (
          <span className="chip on filter-chip">
            <b>Brak daty urodzenia</b>
            <button onClick={() => setFilters({ ...filters, noBirth: false })} aria-label="Usuń filtr">
              <X size={13} />
            </button>
          </span>
        )}
        <Segmented
          variant="neutral"
          value={filters.living}
          onChange={(living) => setFilters({ ...filters, living })}
          options={[
            { value: "living", label: "Żyjący", icon: <HeartPulse size={13} /> },
            { value: "dead", label: "Zmarli" },
            { value: "all", label: "Wszyscy" },
          ]}
        />
        {!filters.hasPhoto && (
          <button className="chip dashed" onClick={() => setFilters({ ...filters, hasPhoto: true })}>
            <Plus size={13} />
            Ma zdjęcie
          </button>
        )}
        {!filters.noBirth && (
          <button className="chip dashed" onClick={() => setFilters({ ...filters, noBirth: true })}>
            <Plus size={13} />
            Brak daty urodzenia
          </button>
        )}
        <PickChip label="Miejsce" items={places.map(([p, n]) => ({ value: p, label: p, note: String(n) }))} onPick={(place) => setFilters({ ...filters, place })} />
        <PickChip
          label="Nazwisko"
          items={(data?.groups ?? []).map((g) => ({ value: g.key, label: g.plural, note: String(g.born + g.married) }))}
          onPick={(key) => setFilters({ ...filters, surnames: [...new Set([...filters.surnames, key])] })}
        />
        <span className="grow" />
        <SurnameOptions marriedWomen={marriedWomen} maidenStyle={maidenStyle} />
        <GroupPicker value={groupBy} onChange={setGroupBy} />
        <SavedFilters filters={filters} onApply={setFilters} />
      </div>
      {view === "grid" ? (
        <PhotoGrid people={sorted.map((x) => x.p)} />
      ) : (
        <div className="row people-body">
          <div className="people-table">
            <div className="people-row header" style={{ gridTemplateColumns: template }}>
              <span />
              <SortHeader label="Imię i nazwisko" active={sort.key === "name"} dir={sort.dir} onClick={() => setSortKey("name")} />
              {shown.map((c) => (
                <SortHeader key={c.key} label={c.label} align={c.align} active={sort.key === c.key} dir={sort.dir} onClick={() => setSortKey(c.key)} />
              ))}
            </div>
            <div ref={scrollRef} className="scroll" style={{ flex: 1 }}>
              {rows.length === 0 && data && (
                <div style={{ padding: 32, color: "var(--text3)", fontSize: 14 }}>Nikt nie pasuje do tych filtrów.</div>
              )}
              <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
                {virtualizer.getVirtualItems().map((item) => {
                  const row = rows[item.index];
                  const style = { position: "absolute" as const, top: 0, left: 0, right: 0, height: item.size, transform: `translateY(${item.start}px)` };
                  if (row.kind === "group") {
                    const g = row.group!;
                    return (
                      <button
                        key={row.key}
                        className="people-row group"
                        style={style}
                        onClick={() =>
                          setCollapsed((c) => {
                            const next = new Set(c);
                            if (next.has(g.id)) next.delete(g.id);
                            else next.add(g.id);
                            return next;
                          })
                        }
                      >
                        {g.collapsed ? <ChevronRight size={14} color="var(--text3)" /> : <ChevronDown size={14} color="var(--text3)" />}
                        {(groupBy === "surname" || groupBy === "branch") && <BranchShape branch={g.branch} size={10} />}
                        <span className="serif" style={{ fontSize: 16, fontWeight: 600 }}>
                          {g.label}
                        </span>
                        <span style={{ color: "var(--text3)" }}>{count(g.count, "osoba", "osoby", "osób")}</span>
                      </button>
                    );
                  }
                  const p = row.person!;
                  return (
                    <div key={row.key} className="people-row person" style={{ ...style, gridTemplateColumns: template }} {...rowButton(() => go({ name: "person", id: p.id }))}>
                      <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={28} />
                      <span className="row" style={{ gap: 8, minWidth: 0 }}>
                        {/* The name keeps its room; the maiden name and the badge shrink first. */}
                        <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, flex: "0 1 auto", minWidth: 90 }}>
                          {displayName(p, maidenStyle)}
                        </span>
                        {(p.maiden || p.nickname) && maidenStyle === "zd" && (
                          <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)", flex: "0 8 auto", minWidth: 24 }}>
                            {p.maiden ? `z d. ${p.maiden}` : `„${p.nickname}”`}
                          </span>
                        )}
                        {row.alsoIn && (
                          <span className="also-badge" style={{ flex: "0 16 auto", minWidth: 24, overflow: "hidden" }}>
                            <Copy size={11} style={{ flex: "none" }} />
                            <span className="ellipsis">też w: {row.alsoIn.join(", ")}</span>
                          </span>
                        )}
                      </span>
                      {shown.map((c) => (
                        <Cell key={c.key} column={c.key} person={p} align={c.align} />
                      ))}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
          <AzRail has={letters} onPick={jump} />
        </div>
      )}
    </div>
  );
}

/** "Helena Wiśniewska", "Helena z Kowalskich Wiśniewska", "Helena Wiśniewska (Kowalska)" (Ustawienia › Osoby i daty). */
function displayName(p: PersonSummary, style: string): string {
  if (!p.maiden) return p.name;
  if (style === "zKowalskich") {
    const genitive = p.maiden.endsWith("ska") ? `${p.maiden.slice(0, -3)}skich` : p.maiden.endsWith("cka") ? `${p.maiden.slice(0, -3)}ckich` : `${p.maiden}ów`;
    return `${p.given} z ${genitive} ${p.surname}`;
  }
  if (style === "paren") return `${p.name} (${p.maiden})`;
  return p.name;
}

function ageOf(p: PersonSummary): number | null {
  const from = p.birth?.sort;
  if (!from) return null;
  const end = p.death?.sort ?? (p.living ? (new Date().getFullYear() * 10000 + (new Date().getMonth() + 1) * 100 + new Date().getDate()) * 10 : null);
  if (!end) return null;
  return Math.floor((end - from) / 100000);
}

function Cell({ column, person: p, align }: { column: ColumnKey; person: PersonSummary; align?: "center" | "right" }) {
  const style = { textAlign: align, fontVariantNumeric: "tabular-nums" as const };
  switch (column) {
    case "birth":
      return (
        <span className="ellipsis" style={style}>
          {p.birth ? <DateBit date={p.birth} field="short" /> : ""}
        </span>
      );
    case "death":
      return (
        <span className="ellipsis" style={style}>
          {p.death ? <DateBit date={p.death} field="short" /> : p.living ? "" : "—"}
        </span>
      );
    case "birthPlace":
      return (
        <span className="ellipsis" style={{ color: "var(--text2)" }}>
          {p.birthPlace}
        </span>
      );
    case "deathPlace":
      return (
        <span className="ellipsis" style={{ color: "var(--text2)" }}>
          {p.deathPlace}
        </span>
      );
    case "generation":
      return <span style={{ ...style, color: "var(--text2)" }}>{p.generation ? roman(p.generation) : ""}</span>;
    case "branch":
      return (
        <span className="row ellipsis" style={{ gap: 6, color: "var(--text2)" }}>
          {p.branchName && <BranchShape branch={p.branch} size={8} />}
          <span className="ellipsis">{p.branchName}</span>
        </span>
      );
    case "photos":
      return (
        <span className="row" style={{ justifyContent: "flex-end", gap: 4 }}>
          {p.photoCount > 0 ? (
            <>
              {p.photoCount}
              <ImageIcon size={12} color="var(--text3)" />
            </>
          ) : (
            <span style={{ color: "var(--text3)" }}>0</span>
          )}
        </span>
      );
    case "changed":
      return <span style={{ fontSize: 12, color: "var(--text3)" }}>{p.changed ? relativeTime(p.changed) : ""}</span>;
    case "created":
      return <span style={{ fontSize: 12, color: "var(--text3)" }}>{p.created ? relativeTime(p.created) : ""}</span>;
    case "age": {
      const age = ageOf(p);
      return <span style={style}>{age ?? ""}</span>;
    }
    case "sex":
      return <span style={{ color: "var(--text2)" }}>{p.sex === "M" ? "mężczyzna" : p.sex === "F" ? "kobieta" : "—"}</span>;
  }
}

function SortHeader({ label, active, dir, onClick, align }: { label: string; active: boolean; dir: 1 | -1; onClick: () => void; align?: "center" | "right" }) {
  return (
    <button className="row sort-header" onClick={onClick} style={{ justifyContent: align === "right" ? "flex-end" : align === "center" ? "center" : "flex-start", color: active ? "var(--text)" : undefined }}>
      {label}
      {active && (dir === 1 ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
    </button>
  );
}

function ColumnPicker({ columns, setColumns }: { columns: ColumnKey[]; setColumns: (c: ColumnKey[]) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="chip" style={{ height: 34, color: "var(--text)" }} onClick={() => setOpen((o) => !o)}>
        <Columns3 size={15} />
        Kolumny <span style={{ color: "var(--text3)" }}>{columns.length + 1} z {COLUMNS.length + 1}</span>
      </button>
      {open && (
        <div className="popover" style={{ top: 40, right: 0, width: 240, padding: "4px 0" }}>
          <div className="menu-label">Widoczne kolumny</div>
          {COLUMNS.map((c) => {
            const on = columns.includes(c.key);
            return (
              <button key={c.key} className="menu-item" onClick={() => setColumns(on ? columns.filter((k) => k !== c.key) : COLUMNS.map((x) => x.key).filter((k) => k === c.key || columns.includes(k)))}>
                <span className={`check${on ? " on" : ""}`} style={{ width: 16, height: 16 }}>
                  {on && "✓"}
                </span>
                {c.label}
              </button>
            );
          })}
          <button className="menu-item" style={{ color: "var(--accent-text)" }} onClick={() => setColumns(DEFAULT_COLUMNS)}>
            Przywróć domyślne
          </button>
        </div>
      )}
    </div>
  );
}

function PickChip({ label, items, onPick }: { label: string; items: { value: string; label: string; note?: string }[]; onPick: (v: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const list = items.filter((i) => fold(i.label).includes(fold(q))).slice(0, 60);
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="chip dashed" onClick={() => setOpen((o) => !o)}>
        <Plus size={13} />
        {label}
      </button>
      {open && (
        <div className="popover" style={{ top: 36, left: 0, width: 260 }}>
          <div style={{ padding: 8 }}>
            <div className="search-box">
              <Search size={14} />
              <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder={`Szukaj: ${label.toLowerCase()}…`} />
            </div>
          </div>
          <div className="scroll" style={{ maxHeight: 280, paddingBottom: 4 }}>
            {list.map((i) => (
              <button
                key={i.value}
                className="menu-item"
                onClick={() => {
                  onPick(i.value);
                  setOpen(false);
                }}
              >
                <span className="grow ellipsis">{i.label}</span>
                {i.note && <span style={{ fontSize: 12, color: "var(--text3)" }}>{i.note}</span>}
              </button>
            ))}
            {list.length === 0 && <div style={{ padding: "8px 12px", fontSize: 13, color: "var(--text3)" }}>Brak wyników.</div>}
          </div>
        </div>
      )}
    </div>
  );
}

function GroupPicker({ value, onChange }: { value: GroupBy; onChange: (v: GroupBy) => void }) {
  const labels: Record<GroupBy, string> = { none: "bez grupowania", surname: "nazwisko", branch: "gałąź", generation: "pokolenie", birthPlace: "miejsce urodzenia", decade: "dekada urodzenia" };
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="chip" style={{ color: "var(--text)" }} onClick={() => setOpen((o) => !o)}>
        <Group size={14} color="var(--text2)" />
        <span>
          Grupuj: <b>{labels[value]}</b>
        </span>
        <ChevronDown size={13} color="var(--text3)" />
      </button>
      {open && (
        <div className="popover" style={{ top: 36, right: 0, width: 220, padding: "4px 0" }}>
          {(Object.keys(labels) as GroupBy[]).map((k) => (
            <button
              key={k}
              className={`menu-item${k === value ? " on" : ""}`}
              onClick={() => {
                onChange(k);
                setOpen(false);
              }}
            >
              {labels[k]}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** „Nazwiska: w obu grupach” (spec §4.9 popover): where married women appear, and how their names are written. */
function SurnameOptions({ marriedWomen, maidenStyle }: { marriedWomen: MarriedWomen; maidenStyle: string }) {
  const setArchive = useStore((s) => s.setArchive);
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const set = async (display: Record<string, string>) => {
    setArchive(await call("archive.setSettings", { display }));
  };
  const words: Record<MarriedWomen, string> = { both: "w obu grupach", birth: "rodowe", current: "obecne" };
  const option = (value: MarriedWomen, title: string, text: string) => (
    <button className={`surname-option${marriedWomen === value ? " on" : ""}`} onClick={() => set({ marriedWomen: value })}>
      <span className={`radio${marriedWomen === value ? " on" : ""}`} />
      <span className="col" style={{ textAlign: "left" }}>
        <span style={{ fontWeight: 600 }}>{title}</span>
        <span style={{ color: "var(--text2)" }}>{text}</span>
      </span>
    </button>
  );
  const style = (value: string, text: string) => (
    <button className={`surname-option${maidenStyle === value ? " on" : ""}`} style={{ minHeight: 36 }} onClick={() => set({ maidenStyle: value })}>
      <span className={`radio${maidenStyle === value ? " on" : ""}`} />
      <span className="serif" style={{ fontSize: 15 }}>
        {text}
      </span>
    </button>
  );
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="chip" style={open ? { border: "1.5px solid var(--accent)", boxShadow: "var(--ring)", color: "var(--text)" } : { color: "var(--text)" }} onClick={() => setOpen((o) => !o)}>
        <Signature size={14} color="var(--text2)" />
        <span>
          Nazwiska: <b>{words[marriedWomen]}</b>
        </span>
        {open ? <ChevronUp size={13} color="var(--text3)" /> : <ChevronDown size={13} color="var(--text3)" />}
      </button>
      {open && (
        <div className="popover" style={{ top: 38, right: 0, width: 360, fontSize: 13, zIndex: 5 }}>
          <div className="menu-label" style={{ padding: "10px 14px 4px" }}>
            Kobiety po ślubie i nazwiska podwójne
          </div>
          {option("both", "W obu grupach", "Każda kobieta po ślubie jest i przy nazwisku rodowym, i przy nazwisku po mężu.")}
          {option("birth", "Tylko nazwisko rodowe", "Każdy raz, w rodzinie, z której pochodzi.")}
          {option("current", "Tylko obecne nazwisko", "Każdy raz, pod nazwiskiem, którego używał(a).")}
          <div className="menu-label" style={{ padding: "10px 14px 4px", borderTop: "1px solid var(--border)" }}>
            Zapis nazwiska
          </div>
          {style("zd", "Helena Wiśniewska z d. Kowalska")}
          {style("zKowalskich", "Helena z Kowalskich Wiśniewska")}
          {style("paren", "Helena Wiśniewska (Kowalska)")}
          <div style={{ padding: "8px 14px 10px", fontSize: 12, color: "var(--text3)" }}>Zapisane w ustawieniach tego archiwum.</div>
        </div>
      )}
    </div>
  );
}

function SavedFilters({ filters, onApply }: { filters: Filters; onApply: (f: Filters) => void }) {
  const [saved, setSaved] = useState<{ name: string; filters: Filters }[]>(() => loadLocal("people.savedFilters", []));
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const store = (list: { name: string; filters: Filters }[]) => {
    setSaved(list);
    saveLocal("people.savedFilters", list);
  };
  const current = JSON.stringify({ ...filters, query: "" }) !== JSON.stringify({ ...EMPTY_FILTERS });
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="chip" style={{ color: "var(--text)" }} onClick={() => setOpen((o) => !o)}>
        <Bookmark size={14} color="var(--text2)" />
        Zapisane filtry
        <ChevronDown size={13} color="var(--text3)" />
      </button>
      {open && (
        <div className="popover" style={{ top: 36, right: 0, width: 260, padding: "4px 0" }}>
          {saved.length === 0 && <div style={{ padding: "8px 12px", fontSize: 13, color: "var(--text3)" }}>Nie ma jeszcze zapisanych filtrów.</div>}
          {saved.map((s, i) => (
            <div key={s.name} className="row" style={{ paddingRight: 6 }}>
              <button
                className="menu-item grow"
                onClick={() => {
                  onApply(s.filters);
                  setOpen(false);
                }}
              >
                {s.name}
              </button>
              <button className="icon-btn" style={{ width: 26, height: 26 }} title="Usuń" onClick={() => store(saved.filter((_, j) => j !== i))}>
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          {current && (
            <button
              className="menu-item"
              style={{ color: "var(--accent-text)", borderTop: "1px solid var(--border)" }}
              onClick={() => {
                const name = window.prompt("Nazwa filtra", "Mój filtr");
                if (name) store([...saved, { name, filters }]);
              }}
            >
              <Plus size={14} color="var(--accent-text)" />
              Zapisz obecne filtry
            </button>
          )}
        </div>
      )}
    </div>
  );
}

/** The photo-grid view (not drawn in the design; FEEDBACK). */
function PhotoGrid({ people }: { people: PersonSummary[] }) {
  const go = useStore((s) => s.go);
  const [limit, setLimit] = useState(240);
  const list = people.slice(0, limit);
  return (
    <div className="scroll" style={{ flex: 1, padding: "0 24px 24px" }}>
      <div className="people-grid">
        {list.map((p) => (
          <button key={p.id} className="people-card" onClick={() => go({ name: "person", id: p.id })}>
            <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={96} tint={22} />
            <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, maxWidth: "100%" }}>
              {p.name}
            </span>
            <span style={{ fontSize: 12, color: "var(--text2)" }}>
              {cardYears(p.birth?.year, p.death?.year, p.living) || "—"}
            </span>
          </button>
        ))}
      </div>
      {people.length > limit && (
        <div style={{ textAlign: "center", padding: 16 }}>
          <button className="btn secondary" onClick={() => setLimit((l) => l + 240)}>
            Pokaż więcej ({num(people.length - limit)})
          </button>
        </div>
      )}
    </div>
  );
}

