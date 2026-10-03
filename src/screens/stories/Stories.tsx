// Historie (spec §4.28): the family's stories, sayings and trivia by decade, with filters (kind, person or branch,
// decade, text), sorting and a reading panel.

import { BookOpen, ChevronDown, Image as ImageIcon, Import, Layers, Lightbulb, Quote, Search, Users, X } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, BranchShape, EmptyState, Segmented, Spinner, Thumb, useDismiss } from "../../components/bits";
import { num } from "../../lib/format";
import { KINDS, StoryDate, StoryPanel } from "./StoryPanel";
import "./stories.css";

interface PersonRef {
  id: string;
  name: string;
  initials: string;
  branch: number;
  photo: string | null;
}

interface StoryItem {
  id: string;
  kind: string;
  title: string;
  excerpt: string;
  date: string | null;
  sort: number | null;
  decade: number | null;
  person: PersonRef | null;
  branch: number | null;
  photo: string | null;
  /** Title and text, lower-cased and without Polish letters. */
  search: string;
}

interface Row extends StoryItem {
  decadeKey: number | null;
  order: number;
}

type KindFilter = "all" | "story" | "saying" | "trivia";
type Sort = "date" | "title";

/** Lower case without diacritics, like the API's `search` field ("Łukasz" → "lukasz"). */
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/ł/g, "l");
}

/** Dates kept only as text ("zima 1915") have no sort key; their year still places them in the right decade. */
function toRow(item: StoryItem): Row {
  const textYear = item.sort == null ? item.date?.match(/\b(\d{3,4})\b/)?.[1] : undefined;
  const year = item.sort != null ? Math.floor(item.sort / 100_000) : textYear ? Number(textYear) : null;
  return {
    ...item,
    decadeKey: year == null ? null : Math.floor(year / 10) * 10,
    // The API's key is yyyymmdd × 10; a year from text goes after the exact dates of that year.
    order: item.sort ?? (year == null ? Number.MAX_SAFE_INTEGER : (year * 10_000 + 1300) * 10),
  };
}

const decadeName = (key: string) => (key === "none" ? "bez daty" : `${key}–${Number(key) + 9}`);

export function Stories({ selected, initialQuery }: { selected?: string; initialQuery?: string }) {
  const { data, error } = useApi<{ items: StoryItem[] }>("stories.list");
  const go = useStore((s) => s.go);
  const [kind, setKind] = useState<KindFilter>("all");
  const [who, setWho] = useState<string | null>(null); // "p:<person id>" or "b:<branch>"
  const [decade, setDecade] = useState<string | null>(null); // "1910" or "none"
  const [query, setQuery] = useState(initialQuery ?? "");
  const [sort, setSort] = useState<Sort>("date");
  const listRef = useRef<HTMLDivElement>(null);

  const rows = useMemo(() => (data?.items ?? []).map(toRow), [data]);
  // Branch colours are named after the surname groups that carry them (stories only know the colour).
  const { data: surnames } = useApi<{ groups: { plural: string; branch: number; born: number }[] }>(rows.length ? "surnames.list" : null);

  const branchName = (b: number) => {
    const names = (surnames?.groups ?? []).filter((g) => g.branch === b).sort((x, y) => y.born - x.born).map((g) => g.plural);
    return names.length ? names.slice(0, 2).join(", ") + (names.length > 2 ? "…" : "") : `gałąź ${b}`;
  };

  const owners = useMemo(() => {
    const map = new Map<string, { person: PersonRef; n: number }>();
    for (const r of rows) {
      if (!r.person) continue;
      const entry = map.get(r.person.id);
      if (entry) entry.n++;
      else map.set(r.person.id, { person: r.person, n: 1 });
    }
    return [...map.values()].sort((a, b) => b.n - a.n || a.person.name.localeCompare(b.person.name, "pl"));
  }, [rows]);

  const branches = useMemo(() => {
    const map = new Map<number, number>();
    for (const r of rows) if (r.branch) map.set(r.branch, (map.get(r.branch) ?? 0) + 1);
    return [...map.entries()].sort((a, b) => b[1] - a[1] || a[0] - b[0]);
  }, [rows]);

  const decades = useMemo(() => {
    const map = new Map<string, number>();
    for (const r of rows) {
      const key = r.decadeKey == null ? "none" : String(r.decadeKey);
      map.set(key, (map.get(key) ?? 0) + 1);
    }
    return [...map.entries()].sort(([a], [b]) => (a === "none" ? 1 : b === "none" ? -1 : Number(a) - Number(b)));
  }, [rows]);

  // Everything but the kind, so the kind switch can show how many of each match.
  const base = useMemo(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    return rows.filter(
      (r) =>
        (!who || (who.startsWith("p:") ? r.person?.id === who.slice(2) : String(r.branch) === who.slice(2))) &&
        (!decade || (decade === "none" ? r.decadeKey == null : String(r.decadeKey) === decade)) &&
        words.every((w) => r.search.includes(w)),
    );
  }, [rows, who, decade, query]);

  const counts: Record<KindFilter, number> = { all: base.length, story: 0, saying: 0, trivia: 0 };
  for (const r of base) if (r.kind in counts) counts[r.kind as KindFilter]++;

  const sections = useMemo(() => {
    const shown = base.filter((r) => kind === "all" || r.kind === kind);
    if (sort === "title") {
      // Sayings start with a quotation mark; compare the words.
      const key = (r: Row) => r.title.replace(/^[„"“”«»'‚\s]+/, "");
      return [{ key: "all", label: null as string | null, items: [...shown].sort((a, b) => key(a).localeCompare(key(b), "pl")) }];
    }
    const groups = new Map<string, { key: string; label: string | null; items: Row[] }>();
    for (const r of [...shown].sort((a, b) => a.order - b.order)) {
      const key = r.decadeKey == null ? "none" : String(r.decadeKey);
      let group = groups.get(key);
      if (!group) groups.set(key, (group = { key, label: key === "none" ? "Bez daty" : decadeName(key), items: [] }));
      group.items.push(r);
    }
    return [...groups.values()];
  }, [base, kind, sort]);

  const shownCount = sections.reduce((n, s) => n + s.items.length, 0);

  // Arriving with a story chosen elsewhere (a profile's „Czytaj całą historię”): bring it into view.
  useEffect(() => {
    listRef.current?.querySelector(".stories-row.selected")?.scrollIntoView({ block: "nearest" });
  }, [selected, rows.length]);

  const clearFilters = () => {
    setKind("all");
    setWho(null);
    setDecade(null);
    setQuery("");
  };

  const whoLabel = !who ? "Osoba lub gałąź" : who.startsWith("p:") ? (
    <>
      Osoba: <b>{owners.find((o) => o.person.id === who.slice(2))?.person.name ?? "?"}</b>
    </>
  ) : (
    <>
      Gałąź: <b>{branchName(Number(who.slice(2)))}</b>
    </>
  );

  const empty = data != null && rows.length === 0;

  return (
    <div className={`stories${selected ? " with-panel" : ""}`}>
      <div className="stories-main">
        <div className="stories-head">
          <h1 className="page-title grow">
            Historie {data && <span className="stories-count">{num(rows.length)}</span>}
          </h1>
          {rows.length > 0 && (
            <div className="search-box" style={{ width: 260 }}>
              <Search size={15} />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Szukaj w treści…" />
              {query && (
                <button onClick={() => setQuery("")} aria-label="Wyczyść">
                  <X size={14} />
                </button>
              )}
            </div>
          )}
        </div>

        {rows.length > 0 && (
          <div className="stories-filters">
            <Segmented
              size={28}
              value={kind}
              onChange={setKind}
              options={[
                { value: "all", label: `Wszystkie ${num(counts.all)}`, icon: <Layers size={13} /> },
                { value: "story", label: `Historie ${num(counts.story)}`, icon: <BookOpen size={13} /> },
                { value: "saying", label: `Powiedzonka ${num(counts.saying)}`, icon: <Quote size={13} /> },
                { value: "trivia", label: `Ciekawostki ${num(counts.trivia)}`, icon: <Lightbulb size={13} /> },
              ]}
            />
            <MenuButton label={whoLabel}>
              <MenuItem on={!who} onClick={() => setWho(null)}>
                <span className="grow">Wszystkie osoby i gałęzie</span>
              </MenuItem>
              {branches.length > 0 && <div className="menu-label">Gałęzie</div>}
              {branches.map(([b, n]) => (
                <MenuItem key={b} on={who === `b:${b}`} onClick={() => setWho(`b:${b}`)} note={n}>
                  <BranchShape branch={b} />
                  <span className="grow ellipsis">{branchName(b)}</span>
                </MenuItem>
              ))}
              {owners.length > 0 && <div className="menu-label">Osoby</div>}
              {owners.map(({ person, n }) => (
                <MenuItem key={person.id} on={who === `p:${person.id}`} onClick={() => setWho(`p:${person.id}`)} note={n}>
                  <span className="grow ellipsis">{person.name}</span>
                </MenuItem>
              ))}
            </MenuButton>
            <MenuButton
              width={200}
              label={
                decade ? (
                  <>
                    Dekada: <b>{decadeName(decade)}</b>
                  </>
                ) : (
                  "Dekada"
                )
              }
            >
              <MenuItem on={!decade} onClick={() => setDecade(null)}>
                <span className="grow">Wszystkie dekady</span>
              </MenuItem>
              {decades.map(([key, n]) => (
                <MenuItem key={key} on={decade === key} onClick={() => setDecade(key)} note={n}>
                  <span className="grow">{decadeName(key)}</span>
                </MenuItem>
              ))}
            </MenuButton>
            <span className="grow" />
            <MenuButton
              plain
              align="right"
              width={200}
              label={
                <>
                  Sortuj: <b>{sort === "date" ? "chronologicznie" : "alfabetycznie"}</b>
                </>
              }
            >
              <MenuItem on={sort === "date"} onClick={() => setSort("date")}>
                <span className="grow">chronologicznie</span>
              </MenuItem>
              <MenuItem on={sort === "title"} onClick={() => setSort("title")}>
                <span className="grow">alfabetycznie</span>
              </MenuItem>
            </MenuButton>
          </div>
        )}

        {empty ? (
          <div className="stories-empty">
            <EmptyState
              icon={<BookOpen size={28} />}
              title="Nie ma jeszcze historii"
              text="Dodaj je w profilu osoby albo zaimportuj paczkę."
              action={
                <div className="row" style={{ gap: 8, marginTop: 6 }}>
                  <button className="btn secondary" onClick={() => go({ name: "people" })}>
                    <Users size={15} />
                    Osoby
                  </button>
                  <button className="btn primary" onClick={() => go({ name: "import" })}>
                    <Import size={15} />
                    Importuj paczkę
                  </button>
                </div>
              }
            />
          </div>
        ) : (
          <div ref={listRef} className="stories-list">
            {error && <div className="stories-none">{error.message}</div>}
            {!data && !error && (
              <div className="stories-none">
                <Spinner size={18} />
              </div>
            )}
            {sections.map((s) => (
              <Fragment key={s.key}>
                {s.label && (
                  <div className="stories-decade">
                    {s.label}
                    <span>{num(s.items.length)}</span>
                  </div>
                )}
                {s.items.map((r) => (
                  <StoryRow key={r.id} item={r} selected={r.id === selected} onOpen={() => go({ name: "stories", id: r.id })} />
                ))}
              </Fragment>
            ))}
            {data && rows.length > 0 && shownCount === 0 && (
              <div className="stories-none">
                Nic nie pasuje do tych filtrów.{" "}
                <button className="link" onClick={clearFilters}>
                  Wyczyść filtry
                </button>
              </div>
            )}
          </div>
        )}
      </div>
      {selected && <StoryPanel key={selected} id={selected} />}
    </div>
  );
}

function StoryRow({ item, selected, onOpen }: { item: Row; selected: boolean; onOpen: () => void }) {
  const Icon = (KINDS[item.kind] ?? KINDS.story).icon;
  return (
    <button className={`stories-row${selected ? " selected" : ""}`} onClick={onOpen}>
      {/* Initials, as drawn; the person's photo is the thumbnail on the right. */}
      <Avatar initials={item.person?.initials ?? "?"} branch={item.person?.branch} size={36} />
      <span className="col grow" style={{ gap: 2 }}>
        <span className="row" style={{ gap: 8, minWidth: 0 }}>
          <Icon size={14} className="stories-row-icon" />
          <span className={`stories-row-title ellipsis${item.kind === "saying" ? " saying" : ""}`}>{item.title}</span>
        </span>
        <span className="stories-row-meta">
          {item.person && `${item.person.name} · `}
          <StoryDate text={item.date} />
        </span>
        {item.excerpt && <span className="stories-row-excerpt">{item.excerpt}</span>}
      </span>
      {item.photo && <Thumb path={item.photo} size={128} icon={<ImageIcon size={16} />} style={{ width: 72, height: 54, flex: "none", borderRadius: 4, border: "none" }} />}
    </button>
  );
}

/** A dropdown chip-button (spec §5.4), or plain text for „Sortuj”, with a menu under it (§5.14). Picking an
 *  item closes the menu. */
function MenuButton({
  label,
  plain,
  width = 260,
  align = "left",
  children,
}: {
  label: ReactNode;
  plain?: boolean;
  width?: number;
  align?: "left" | "right";
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const close = () => setOpen(false);
  const ref = useDismiss<HTMLDivElement>(open, close);
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className={plain ? "stories-sort" : `chip stories-chip${open ? " open" : ""}`} onClick={() => setOpen(!open)} aria-expanded={open}>
        <span className="ellipsis">{label}</span>
        {!plain && <ChevronDown size={13} className="stories-chevron" />}
      </button>
      {open && (
        <div className="popover stories-menu" style={{ [align]: 0, width }} onClick={(e) => (e.target as HTMLElement).closest("button") && close()}>
          {children}
        </div>
      )}
    </div>
  );
}

function MenuItem({ on, note, onClick, children }: { on: boolean; note?: number; onClick: () => void; children: ReactNode }) {
  return (
    <button className={`menu-item${on ? " on" : ""}`} onClick={onClick}>
      {children}
      {note != null && <span className="stories-menu-count">{num(note)}</span>}
    </button>
  );
}
