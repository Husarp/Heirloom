import { BookOpen, FileSearch, History, House, Images, Import, Library, MapPin, Network, Search, SearchX, Settings, Signature, UserPlus, Users } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { call } from "../api/transport";
import type { PersonSummary } from "../api/types";
import { useStore, type Route } from "../app/store";
import { Avatar } from "../components/bits";
import { cardName, cardYears, count, num, people as peopleCount } from "../lib/format";

type Hit = PersonSummary & { context: string };
type PlaceRow = { name: string; path: string[]; count: number };
type SurnameGroup = { key: string; name: string; plural: string; branch: number; born: number; married: number };

interface Item {
  key: string;
  group: "Osoby" | "Miejsca" | "Polecenia" | "Podobne nazwiska";
  /** The row itself. */
  node: ReactNode;
  run: () => void;
  /** Ctrl Enter: the person in the tree. */
  alt?: () => void;
}

/** Folds one character at a time, so positions in the folded text are positions in the original. */
function foldChars(text: string): string {
  return [...text].map((c) => (c === "ł" || c === "Ł" ? "l" : c.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").charAt(0) || c)).join("");
}
const fold = (text: string) => foldChars(text).trim();

/** The matched part of `text` for the query, found without Polish letters (design 17e: „WIŚNIEWSK” in bold). */
function Marked({ text, query }: { text: string; query: string }) {
  const typed = fold(query).split(/\s+/).filter(Boolean).sort((a, b) => b.length - a.length);
  // A surname found through its other gender's form: the stem they share is marked (WIŚNIEWSK-a / -i).
  const words = [...typed, ...typed.filter((w) => GENDERED.test(w)).map((w) => w.slice(0, -1))];
  const folded = foldChars(text);
  for (const w of words) {
    // A word start: the beginning, or after a space or a hyphen.
    let at = -1;
    for (let i = folded.indexOf(w); i >= 0; i = folded.indexOf(w, i + 1)) {
      if (i === 0 || /[\s-]/.test(folded[i - 1])) {
        at = i;
        break;
      }
    }
    if (at >= 0) {
      const chars = [...text];
      return (
        <>
          {chars.slice(0, at).join("")}
          <mark>{chars.slice(at, at + w.length).join("")}</mark>
          {chars.slice(at + w.length).join("")}
        </>
      );
    }
  }
  return <>{text}</>;
}

/** A whole surname word with a gendered ending (Wiśniewsk-a, Zawadzk-i). */
const GENDERED = /(sk|ck|dzk)[ai]$/;

/** Why a person matches when their name as shown doesn't say it: the maiden name, a nickname, the other gender's
 *  form of the surname (of either surname), another form from a record. */
function whyOf(h: Hit, query: string): string | null {
  const words = fold(query).split(/\s+/).filter(Boolean);
  // Words as the search compares them: whole (a hyphenated surname is one word) and the parts of a hyphenated one.
  const starts = (text: string, w: string) => {
    const f = fold(text);
    return f.split(/\s+/).some((t) => t.startsWith(w)) || f.split(/[\s-]+/).some((t) => t.startsWith(w));
  };
  const shown = `${h.name} ${cardName(h)}`;
  const unexplained = words.filter((w) => !starts(shown, w));
  if (unexplained.length === 0) return null;
  if (h.maiden && unexplained.every((w) => starts(h.maiden!, w))) return "nazwisko panieńskie";
  if (h.nickname && unexplained.every((w) => starts(h.nickname!, w))) return "przydomek";
  const otherForm = (name: string | null) => !!name && unexplained.some((w) => GENDERED.test(w) && fold(name).slice(0, -1) === w.slice(0, -1));
  if (otherForm(h.surname)) return fold(h.surname).endsWith("i") ? "forma męska" : "forma żeńska";
  if (otherForm(h.maiden)) return "nazwisko panieńskie";
  return "inna forma zapisu";
}

/** Levenshtein distance, for surnames that sound alike when nothing matched („wiszniewska” → Wiśniewska). */
function distance(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    let prev = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const temp = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = temp;
    }
  }
  return row[b.length];
}

/** Ctrl K (design 17e, PLAN §4.1): people, places and commands; without Polish letters, maiden names and the other
 *  gender's form included; the matched part marked and a label saying why a result matches. „Skocz do osoby…” opens
 *  the same window for people only. */
export function CommandPalette() {
  const setPalette = useStore((s) => s.setPalette);
  const options = useStore((s) => s.palette);
  const go = useStore((s) => s.go);
  const requireEdit = useStore((s) => s.requireEdit);
  const peopleOnly = options?.scope === "people";
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [ms, setMs] = useState<number | null>(null);
  const [places, setPlaces] = useState<PlaceRow[] | null>(null);
  const [surnames, setSurnames] = useState<SurnameGroup[] | null>(null);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const close = () => setPalette(false);
  const goTo = (route: Route) => {
    close();
    go(route);
  };
  const q = query.trim();

  useEffect(() => {
    let cancelled = false;
    if (!q) {
      setHits([]);
      setMs(null);
      return;
    }
    const started = performance.now();
    call<Hit[]>("people.search", { q, limit: 12 })
      .then((list) => {
        if (cancelled) return;
        setHits(list);
        setMs(Math.max(1, Math.round(performance.now() - started)));
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [q]);

  // Places and surnames are loaded once, when typing starts.
  useEffect(() => {
    if (!q || peopleOnly) return;
    if (!places) call<{ places: PlaceRow[] }>("places.list").then((r) => setPlaces(r.places)).catch(() => {});
    if (!surnames) call<{ groups: SurnameGroup[] }>("surnames.list").then((r) => setSurnames(r.groups)).catch(() => {});
  }, [q, places, surnames, peopleOnly]);

  const commands = useMemo(
    () => [
      { label: "Start", icon: <House size={15} />, route: { name: "start" } as Route, words: "start strona glowna" },
      { label: "Drzewo", icon: <Network size={15} />, route: { name: "tree", view: "family" } as Route, words: "drzewo rodzina" },
      { label: "Całe drzewo", icon: <Network size={15} />, route: { name: "tree", view: "overview" } as Route, words: "cale drzewo mapa" },
      { label: "Osoby", icon: <Users size={15} />, route: { name: "people" } as Route, words: "osoby lista tabela" },
      { label: "Nazwiska", icon: <Signature size={15} />, route: { name: "surnames" } as Route, words: "nazwiska rody" },
      { label: "Miejsca", icon: <MapPin size={15} />, route: { name: "places" } as Route, words: "miejsca wsie parafie" },
      { label: "Historie", icon: <BookOpen size={15} />, route: { name: "stories" } as Route, words: "historie opowiesci powiedzonka ciekawostki" },
      { label: "Media", icon: <Images size={15} />, route: { name: "media" } as Route, words: "media zdjecia dokumenty" },
      { label: "Źródła", icon: <Library size={15} />, route: { name: "sources" } as Route, words: "zrodla akty" },
      { label: "Import", icon: <Import size={15} />, route: { name: "import" } as Route, words: "import paczka ai pliki" },
      { label: "Historia zmian", icon: <History size={15} />, route: { name: "activity" } as Route, words: "historia zmian cofnij" },
      { label: "Ustawienia", icon: <Settings size={15} />, route: { name: "settings" } as Route, words: "ustawienia wyglad motyw" },
    ],
    [],
  );

  const items: Item[] = [];
  const pick = (id: string) => {
    if (options?.pick) {
      close();
      options.pick(id);
    } else goTo({ name: "person", id });
  };
  for (const h of hits) {
    const why = whyOf(h, q);
    const line = [h.maiden ? `z d. ${h.maiden}` : null, cardYears(h.birth?.year, h.death?.year, h.living) || null, h.birthPlace?.split(",")[0] ?? null].filter(Boolean).join(" · ");
    items.push({
      key: h.id,
      group: "Osoby",
      run: () => pick(h.id),
      alt: peopleOnly ? undefined : () => goTo({ name: "tree", view: "family", person: h.id }),
      node: (
        <>
          <Avatar initials={h.initials} branch={h.branch} photo={h.photo} size={32} />
          <span className="col grow" style={{ minWidth: 0, lineHeight: 1.3 }}>
            <span className="ellipsis" style={{ fontSize: 14 }}>
              <Marked text={cardName(h)} query={q} />
            </span>
            <span className="ellipsis" style={{ fontSize: 12, color: "var(--text2)" }}>
              {why ? <Marked text={line || h.context} query={q} /> : line || h.context}
            </span>
          </span>
          {why && <span className="palette-why">{why}</span>}
        </>
      ),
    });
  }
  if (!peopleOnly && q && places) {
    const f = fold(q);
    const found = places.filter((p) => fold(p.name).startsWith(f) || fold(p.name).includes(` ${f}`)).sort((a, b) => b.count - a.count).slice(0, 4);
    for (const p of found) {
      items.push({
        key: `place-${p.path.join("/")}`,
        group: "Miejsca",
        run: () => goTo({ name: "places", place: JSON.stringify(p.path) }),
        node: (
          <>
            <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
              <MapPin size={15} />
            </span>
            <span className="col grow" style={{ minWidth: 0, lineHeight: 1.3 }}>
              <span className="ellipsis" style={{ fontSize: 14 }}>
                <Marked text={[p.name, ...p.path.slice(0, -1).reverse()].join(", ")} query={q} />
              </span>
              <span style={{ fontSize: 12, color: "var(--text2)" }}>{peopleCount(p.count)}</span>
            </span>
          </>
        ),
      });
    }
  }
  if (!peopleOnly) {
    const f = fold(q);
    if (q && surnames) {
      for (const g of surnames.filter((g) => fold(g.name).startsWith(f) || fold(g.plural).startsWith(f)).slice(0, 2)) {
        items.push({
          key: `surname-${g.key}`,
          group: "Polecenia",
          run: () => goTo({ name: "surnames", key: g.key }),
          node: (
            <>
              <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
                <Signature size={15} />
              </span>
              <span className="col grow" style={{ minWidth: 0, lineHeight: 1.3 }}>
                <span className="ellipsis" style={{ fontSize: 14 }}>
                  Pokaż nazwisko <Marked text={g.name} query={q} /> w Nazwiskach
                </span>
                <span style={{ fontSize: 12, color: "var(--text2)" }}>{peopleCount(g.born + g.married)}</span>
              </span>
            </>
          ),
        });
      }
    }
    const shownCommands = q ? commands.filter((c) => fold(`${c.words} ${c.label}`).includes(f)) : commands.slice(0, 6);
    for (const c of shownCommands) {
      items.push({
        key: c.label,
        group: "Polecenia",
        run: () => goTo(c.route),
        node: (
          <>
            <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
              {c.icon}
            </span>
            <span className="grow" style={{ fontSize: 14 }}>
              {c.label}
            </span>
          </>
        ),
      });
    }
    if (!q || "dodaj nowa osoba".includes(f)) {
      items.push({
        key: "add-person",
        group: "Polecenia",
        run: () => {
          close();
          requireEdit(() => go({ name: "edit", id: null }));
        },
        node: (
          <>
            <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
              <UserPlus size={15} />
            </span>
            <span className="grow" style={{ fontSize: 14 }}>
              Dodaj osobę
            </span>
          </>
        ),
      });
    }
  }
  const nothing = !!q && ms != null && items.length === 0;

  // No results: surnames that sound alike, and the full text search in stories (design 17e „Brak wyników”).
  const similar: Item[] = [];
  if (nothing && surnames) {
    const f = fold(q);
    const alike = surnames
      .map((g) => ({ g, d: Math.min(distance(f, fold(g.name)), distance(f, fold(g.plural))) }))
      .filter((x) => x.d <= Math.max(2, Math.floor(f.length / 4)))
      .sort((a, b) => a.d - b.d || b.g.born - a.g.born)
      .slice(0, 3);
    for (const { g } of alike) {
      similar.push({
        key: `similar-${g.key}`,
        group: "Podobne nazwiska",
        run: () => goTo({ name: "surnames", key: g.key }),
        node: (
          <>
            <Signature size={15} color="var(--text2)" />
            <span className="grow" style={{ fontSize: 14 }}>
              {g.name} <span style={{ color: "var(--text2)", fontSize: 12 }}>· {peopleCount(g.born + g.married)} · brzmi podobnie</span>
            </span>
          </>
        ),
      });
    }
  }
  if (nothing && !peopleOnly) {
    similar.push({
      key: "fulltext",
      group: "Podobne nazwiska",
      run: () => goTo({ name: "stories", q }),
      node: (
        <>
          <FileSearch size={15} color="var(--text2)" />
          <span className="grow" style={{ fontSize: 14 }}>
            Szukaj „{q}” w historiach i notatkach
          </span>
        </>
      ),
    });
  }
  const list = nothing ? similar : items;

  useEffect(() => setActive(0), [q, hits.length]);
  useEffect(() => {
    listRef.current?.querySelector(".palette-item.active")?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const peopleResults = hits.length;
  const meta = q && ms != null ? `${count(peopleResults, "wynik", "wyniki", "wyników")} · ${num(ms)} ms` : "";

  return (
    <div className="backdrop palette-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="dialog palette" role="dialog" aria-label={peopleOnly ? "Skocz do osoby" : "Szukaj"}>
        <div className="palette-input">
          <Search size={18} color="var(--text2)" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={peopleOnly ? "Skocz do osoby…" : "Szukaj osób, miejsc i poleceń…"}
            aria-controls="palette-list"
            aria-activedescendant={list[active] ? `palette-${list[active].key}` : undefined}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                close();
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(a + 1, list.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                const item = list[active];
                if (!item) return;
                if ((e.ctrlKey || e.metaKey) && item.alt) item.alt();
                else item.run();
              }
            }}
          />
          {meta && <span className="palette-meta">{meta}</span>}
          <span className="kbd">Esc</span>
        </div>
        {nothing && (
          <div className="palette-empty">
            <SearchX size={24} color="var(--text2)" />
            <b className="serif">Brak wyników dla „{q}”</b>
            <span>Szukamy w imionach, nazwiskach (także panieńskich i ich odmianach) oraz miejscach, bez względu na polskie znaki.</span>
          </div>
        )}
        {list.length > 0 && (
          <div className="palette-list scroll" id="palette-list" role="listbox" ref={listRef}>
            {list.map((item, i) => (
              <Fragment key={item.key}>
                {item.group !== list[i - 1]?.group && <div className="menu-label palette-group">{item.group}</div>}
                <div
                  id={`palette-${item.key}`}
                  role="option"
                  aria-selected={i === active}
                  className={`palette-item${i === active ? " active" : ""}`}
                  onMouseEnter={() => setActive(i)}
                  onClick={item.run}
                >
                  {item.node}
                  {i === active && <span className="palette-enter">Enter</span>}
                </div>
              </Fragment>
            ))}
          </div>
        )}
        <div className="palette-foot">
          <span>↑ ↓ wybierz</span>
          <span>{peopleOnly ? "Enter pokaż w drzewie" : "Enter otwórz"}</span>
          {!peopleOnly && <span>Ctrl Enter pokaż w drzewie</span>}
          <span className="grow" />
          <span>Esc zamknij</span>
        </div>
      </div>
    </div>
  );
}
