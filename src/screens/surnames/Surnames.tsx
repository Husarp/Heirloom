// Nazwiska (spec §4.26): surname groups sorted by size with an A–Z rail, and a page per group with its variants,
// similar surnames and the people born with the surname or given it by marriage.

import { useVirtualizer } from "@tanstack/react-virtual";
import { Network, Search, Signature, Sparkles } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus, PersonSummary } from "../../api/types";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { AzRail, BranchShape, EmptyState, Lifespan, Spinner, Tabs } from "../../components/bits";
import { num, people, plural } from "../../lib/format";
import { SuggestionBar, collator, fold, useCrumb, usePaneRows } from "./split";
import "./surnames.css";

interface Group {
  key: string;
  name: string;
  plural: string;
  branch: number;
  born: number;
  married: number;
  from: number | null;
  to: number | null;
  places: string[];
  letter: string | null;
}

interface SurnameList {
  groups: Group[];
  withoutSurname: number;
}

type Row = PersonSummary & { context: string; years: string; parentsText: string };

interface Similar {
  key: string;
  name: string;
  plural: string;
  count: number;
}

interface SurnameData {
  key: string;
  name: string;
  plural: string;
  genitive: string;
  branch: number;
  count: number;
  bornCount: number;
  marriedCount: number;
  from: number | null;
  to: number | null;
  places: string[];
  oldest: { id: string; name: string; year: string | null } | null;
  forms: { form: string; count: number; note: string | null }[];
  similar: Similar[];
  born: Row[];
  married: Row[];
}

interface PeopleList {
  people: PersonSummary[];
  memberships: { birth: string | null; married: string[] }[];
}

/** The pseudo-group of people without a birth surname, also used as its route key. */
const NO_SURNAME = "(bez nazwiska)";

/** Branch colour names (spec §1.2) for „Gałąź 1 · Terakota”. */
const BRANCH_NAMES = ["Terakota", "Morska", "Fiolet", "Ochra", "Błękit", "Oliwka", "Morwa", "Łupek", "Bursztyn", "Turkus", "Wiśnia", "Szałwia"];

function yearSpan(from: number | null, to: number | null): string {
  if (from == null || to == null) return "";
  return from === to ? String(from) : `${from}–${to}`;
}

/** "Wólka" from "Wólka, parafia Łęczna, powiat lubelski". */
function town(place: string | null): string {
  return place?.split(",")[0].trim() ?? "";
}

/** Every surname form used in each group (Kowalski, Kowalska…), folded for search. surnames.list doesn't send the
 *  variants, so they come from the people list, loaded only while searching. */
function formsByGroup(list: PeopleList | null): Map<string, string[]> {
  const forms = new Map<string, string[]>();
  if (!list) return forms;
  const add = (key: string, form: string) => {
    const folded = fold(form);
    const known = forms.get(key);
    if (!known) forms.set(key, [folded]);
    else if (!known.includes(folded)) known.push(folded);
  };
  list.people.forEach((p, i) => {
    const m = list.memberships[i];
    if (!m) return;
    // The birth group has the maiden name; the husband's group has the name used after the marriage.
    if (m.birth) add(m.birth, p.maiden ?? p.surname);
    for (const key of m.married) add(key, p.surname);
  });
  return forms;
}

export function Surnames({ selected }: { selected?: string }) {
  const { data, error } = useApi<SurnameList>("surnames.list");
  const go = useStore((s) => s.go);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const paneRef = useRef<HTMLDivElement>(null);
  const q = fold(query.trim());
  const variants = useApi<PeopleList>(q ? "people.list" : null);

  // Largest family first, as drawn (and as the branch colours are given out).
  const groups = useMemo(() => [...(data?.groups ?? [])].sort((a, b) => b.born - a.born || collator.compare(a.name, b.name)), [data]);
  const forms = useMemo(() => formsByGroup(variants.data), [variants.data]);
  const shown = useMemo(
    () => (q ? groups.filter((g) => fold(`${g.plural} ${g.name}`).includes(q) || (forms.get(g.key) ?? []).some((f) => f.includes(q))) : groups),
    [groups, forms, q],
  );
  const key = selected ?? groups[0]?.key ?? (data?.withoutSurname ? NO_SURNAME : undefined);
  const current = groups.find((g) => g.key === key);
  const letters = useMemo(() => new Set(shown.flatMap((g) => (g.letter ? [g.letter] : []))), [shown]);
  const rows = useVirtualizer({ count: shown.length, getScrollElement: () => listRef.current, estimateSize: () => 56, overscan: 8 });

  useCrumb(key === NO_SURNAME ? NO_SURNAME : (current?.plural ?? null));

  // A new group starts at the top of its page, and the rail shows its letter again.
  useEffect(() => {
    paneRef.current?.scrollTo({ top: 0 });
    setPicked(null);
  }, [key]);

  // Bring the selected group into view once (a link from elsewhere can point far down the list).
  const scrolledTo = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (!key || scrolledTo.current === key) return;
    const index = shown.findIndex((g) => g.key === key);
    if (index < 0) return;
    scrolledTo.current = key;
    rows.scrollToIndex(index, { align: "auto" });
  }, [key, shown, rows]);

  const select = (groupKey: string) => go({ name: "surnames", key: groupKey });
  const jump = (letter: string) => {
    const index = shown.findIndex((g) => g.letter === letter);
    if (index >= 0) rows.scrollToIndex(index, { align: "start" });
    setPicked(letter);
  };

  if (error) {
    return (
      <div className="page">
        <EmptyState icon={<Signature size={28} />} title="Nie udało się wczytać nazwisk" text={error.message} />
      </div>
    );
  }
  if (data && groups.length === 0 && !data.withoutSurname) {
    return (
      <div className="page">
        <EmptyState icon={<Signature size={28} />} title="Nie ma jeszcze nazwisk" text="Rodziny pojawią się tu, gdy w archiwum będą osoby z nazwiskami." />
      </div>
    );
  }

  return (
    <div className="split surnames">
      <div className="split-list">
        <div className="split-list-main">
          <div className="split-head">
            <h1 className="split-title">
              Nazwiska <span className="count">{num(groups.length)}</span>
            </h1>
            <label className="search-box">
              <Search size={15} />
              <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Szukaj, także w wariantach…" />
            </label>
          </div>
          <div ref={listRef} className="split-scroll">
            {!data && (
              <div className="split-loading">
                <Spinner />
              </div>
            )}
            <div className="split-rows" style={{ height: rows.getTotalSize() }}>
              {rows.getVirtualItems().map((item) => {
                const g = shown[item.index];
                return (
                  <GroupRow
                    key={g.key}
                    title={g.plural}
                    meta={[yearSpan(g.from, g.to), g.places.join(", ")].filter(Boolean).join(" · ")}
                    count={g.born}
                    married={g.married}
                    branch={g.branch}
                    on={g.key === key}
                    top={item.start}
                    onClick={() => select(g.key)}
                  />
                );
              })}
            </div>
            {!q && data && data.withoutSurname > 0 && (
              <GroupRow title={NO_SURNAME} meta="osoby bez nazwiska" count={data.withoutSurname} married={0} branch={12} on={key === NO_SURNAME} onClick={() => select(NO_SURNAME)} />
            )}
            {q && shown.length === 0 &&
              (!variants.data && !variants.error ? (
                <div className="split-loading">
                  <Spinner />
                </div>
              ) : (
                <div className="split-note">Żadne nazwisko nie pasuje do „{query.trim()}”.</div>
              ))}
          </div>
        </div>
        <AzRail width={26} active={picked ?? current?.letter} has={letters} onPick={jump} />
      </div>
      <div ref={paneRef} className="split-detail">
        {key === NO_SURNAME ? <NoSurname pane={paneRef} /> : key ? <SurnameDetail groupKey={key} pane={paneRef} /> : null}
      </div>
    </div>
  );
}

function GroupRow({
  title,
  meta,
  count,
  married,
  branch,
  on,
  top,
  onClick,
}: {
  title: string;
  meta: string;
  count: number;
  married: number;
  branch: number;
  on: boolean;
  top?: number;
  onClick: () => void;
}) {
  return (
    <button className={`sn-row${on ? " on" : ""}`} style={top != null ? { transform: `translateY(${top}px)` } : undefined} onClick={onClick} aria-current={on || undefined}>
      <BranchShape branch={branch} />
      <span className="col grow" style={{ lineHeight: 1.25 }}>
        <span className="name ellipsis">{title}</span>
        <span className="meta ellipsis">{meta}</span>
      </span>
      <span className="col" style={{ alignItems: "flex-end", lineHeight: 1.25 }}>
        <b className="n num">{num(count)}</b>
        <span className="married">{married > 0 ? `+${num(married)} przez małż.` : ""}</span>
      </span>
    </button>
  );
}

function SurnameDetail({ groupKey, pane }: { groupKey: string; pane: RefObject<HTMLDivElement | null> }) {
  const { data, error, fresh } = useApi<SurnameData>("surname.get", { key: groupKey });
  if (error) return <EmptyState icon={<Signature size={28} />} title="Nie ma takiego nazwiska" text="Mogło zostać zmienione. Wybierz nazwisko z listy." />;
  if (!data) {
    return (
      <div className="split-loading">
        <Spinner />
      </div>
    );
  }
  // Keyed, so the tab starts at „Urodzeni jako…” for every group.
  return <SurnamePage key={data.key} data={data} stale={!fresh} pane={pane} />;
}

function SurnamePage({ data, stale, pane }: { data: SurnameData; stale: boolean; pane: RefObject<HTMLDivElement | null> }) {
  const go = useStore((s) => s.go);
  const [tab, setTab] = useState<"born" | "married">(data.bornCount === 0 && data.marriedCount > 0 ? "married" : "born");
  const listRef = useRef<HTMLDivElement>(null);
  const rows = tab === "born" ? data.born : data.married;
  const virtual = usePaneRows(rows.length, 40, pane, listRef);
  const oldest = data.oldest;
  const [first, second, third] = data.places;
  const born = data.bornCount === 1 ? "1 urodzony" : `${num(data.bornCount)} urodzonych`;
  const tabs: { value: "born" | "married"; label: string }[] = [
    { value: "born", label: `Urodzeni jako ${data.plural} ${num(data.bornCount)}` },
    { value: "married", label: `Przez małżeństwo ${num(data.marriedCount)}` },
  ];

  return (
    <div className="sn-detail" style={stale ? { opacity: 0.6 } : undefined}>
      <div className="sn-title">
        <div className="col grow" style={{ gap: 4, minWidth: 0 }}>
          <span className="sn-branch">
            <BranchShape branch={data.branch} />
            Gałąź {data.branch} · {BRANCH_NAMES[data.branch - 1]}
          </span>
          <h2>{data.plural}</h2>
        </div>
        <button className="btn primary" style={{ padding: "0 16px" }} disabled={!oldest} onClick={() => oldest && go({ name: "tree", view: "family", person: oldest.id })}>
          <Network size={15} />
          Pokaż w drzewie
        </button>
      </div>

      <div className="sn-stats">
        <Stat value={num(data.count)} label={plural(data.count, "osoba", "osoby", "osób")} sub={data.marriedCount > 0 ? `${born} · ${num(data.marriedCount)} przez małż.` : born} />
        <Stat value={yearSpan(data.from, data.to) || "—"} label="lata" sub={data.from != null ? "najstarsza – najmłodsza" : undefined} />
        <Stat value={[first, second].filter(Boolean).join(" · ") || "—"} label="główne miejsca" sub={third ? `oraz ${third}` : undefined} />
        <Stat
          value={oldest ? <button onClick={() => go({ name: "person", id: oldest.id })}>{oldest.name}</button> : "—"}
          title={oldest?.name}
          label="najstarszy przodek"
          sub={oldest?.year ?? undefined}
        />
      </div>

      <div className="col" style={{ gap: 8 }}>
        <span className="label-caps">Warianty</span>
        <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
          {data.forms.map((f) => (
            <span key={f.form} className="sn-variant">
              <span className="form">{f.form}</span>
              <b className="n num">{num(f.count)}</b>
              {f.note && <span className="note">{f.note}</span>}
            </span>
          ))}
        </div>
        <SimilarSurnames data={data} />
      </div>

      <div className="card sn-people">
        <div className="sn-tabs">
          <Tabs tabs={tabs} value={tab} onChange={setTab} />
        </div>
        {rows.length === 0 ? (
          <div className="sn-empty">{tab === "born" ? "Nikt w archiwum nie urodził się z tym nazwiskiem." : "Nikt nie przyjął tego nazwiska przez małżeństwo."}</div>
        ) : (
          <>
            <div className="sn-grid head">
              <span>Imię i nazwisko</span>
              <span>Lata</span>
              <span>Urodzenie</span>
              <span>Rodzice</span>
            </div>
            <div ref={listRef} className="split-rows" style={{ height: virtual.getTotalSize() }}>
              {virtual.getVirtualItems().map((item) => {
                const p = rows[item.index];
                return (
                  <PersonRow
                    key={p.id}
                    name={p.name}
                    maiden={p.maiden}
                    years={<span className={p.birth?.uncertain || p.death?.uncertain ? "uncertain" : undefined}>{p.years}</span>}
                    place={p.birthPlace}
                    parents={p.parentsText}
                    top={item.start - virtual.options.scrollMargin}
                    onClick={() => go({ name: "person", id: p.id })}
                  />
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function Stat({ value, label, sub, title }: { value: ReactNode; label: string; sub?: string; title?: string }) {
  return (
    <div>
      <span className="value ellipsis" title={title ?? (typeof value === "string" ? value : undefined)}>
        {value}
      </span>
      <span className="key">{label}</span>
      {sub && <span className="sub ellipsis">{sub}</span>}
    </div>
  );
}

/** „Podobne: Kowalewski (2 osoby)”. Joining and „To inne nazwisko” are edit actions (DESIGNER_ANSWERS §6); both are
 *  kept in the archive's display settings, which the backend applies to the groups. */
function SimilarSurnames({ data }: { data: SurnameData }) {
  const go = useStore((s) => s.go);
  const mode = useStore((s) => s.mode);
  const display = useStore((s) => s.archive?.display);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [busy, setBusy] = useState(false);

  // surnameJoins: { joined key: group key }; surnameDistinct: sorted key pairs that are different surnames.
  const rawJoins = display?.surnameJoins;
  const rawDistinct = display?.surnameDistinct;
  const joins = rawJoins && typeof rawJoins === "object" && !Array.isArray(rawJoins) ? (rawJoins as Record<string, string>) : {};
  const distinct = Array.isArray(rawDistinct) ? (rawDistinct as string[][]) : [];
  const pair = (a: string, b: string) => [a, b].sort();
  const dismissed = new Set(distinct.map((p) => p.join("|")));
  const similar = data.similar.filter((s) => !joins[s.key] && joins[data.key] !== s.key && !dismissed.has(pair(data.key, s.key).join("|")));

  const save = (patch: Record<string, unknown>, done: string, undo: Record<string, unknown>) =>
    requireEdit(async () => {
      setBusy(true);
      try {
        const status = await call<ArchiveStatus>("archive.setSettings", { display: patch });
        afterChange(status);
        notify(done, {
          action: {
            label: "Cofnij",
            run: () => {
              call<ArchiveStatus>("archive.setSettings", { display: undo })
                .then(afterChange)
                .catch((e: Error) => notify(e.message, { kind: "err" }));
            },
          },
        });
      } catch (e) {
        notify((e as Error).message, { kind: "err" });
      } finally {
        setBusy(false);
      }
    });

  return similar.map((s) => (
    <SuggestionBar
      key={s.key}
      icon={<Sparkles size={15} />}
      actions={
        mode === "edit" && (
          <>
            <button
              className="btn secondary"
              disabled={busy}
              onClick={() =>
                save({ surnameDistinct: [...distinct, pair(data.key, s.key)] }, `„${s.name}” to inne nazwisko. Nie będzie już podpowiadane.`, { surnameDistinct: distinct })
              }
            >
              To inne nazwisko
            </button>
            <button
              className="btn primary"
              disabled={busy}
              onClick={() => save({ surnameJoins: { ...joins, [s.key]: data.key } }, `Nazwisko „${s.name}” dołączono do ${data.genitive}.`, { surnameJoins: joins })}
            >
              Dołącz
            </button>
          </>
        )
      }
    >
      Podobne:{" "}
      <button className="suggest-name" onClick={() => go({ name: "surnames", key: s.key })}>
        {s.name}
      </button>{" "}
      ({people(s.count)}).{mode === "edit" && ` Dołączyć do ${data.genitive}?`}
    </SuggestionBar>
  ));
}

/** „(bez nazwiska)”: people without a birth surname. surname.get has no such group, so they come from the people
 *  list; the table has no „Rodzice” column because the list doesn't carry parents. */
function NoSurname({ pane }: { pane: RefObject<HTMLDivElement | null> }) {
  const go = useStore((s) => s.go);
  const { data } = useApi<PeopleList>("people.list");
  const rows = useMemo(() => (data ? data.people.filter((_, i) => !data.memberships[i]?.birth) : []), [data]);
  const listRef = useRef<HTMLDivElement>(null);
  const virtual = usePaneRows(rows.length, 40, pane, listRef);
  if (!data) {
    return (
      <div className="split-loading">
        <Spinner />
      </div>
    );
  }
  return (
    <div className="sn-detail">
      <div className="sn-title">
        <div className="col grow" style={{ gap: 4 }}>
          <span className="sn-branch">{people(rows.length)} bez nazwiska rodowego</span>
          <h2>{NO_SURNAME}</h2>
        </div>
      </div>
      <div className="card sn-people">
        <div className="sn-grid head no-parents">
          <span>Imię i nazwisko</span>
          <span>Lata</span>
          <span>Urodzenie</span>
        </div>
        <div ref={listRef} className="split-rows" style={{ height: virtual.getTotalSize() }}>
          {virtual.getVirtualItems().map((item) => {
            const p = rows[item.index];
            return (
              <PersonRow
                key={p.id}
                name={p.name}
                maiden={p.maiden}
                years={<Lifespan birth={p.birth} death={p.death} living={p.living} />}
                place={p.birthPlace}
                top={item.start - virtual.options.scrollMargin}
                onClick={() => go({ name: "person", id: p.id })}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}

function PersonRow({ name, maiden, years, place, parents, top, onClick }: { name: string; maiden: string | null; years: ReactNode; place: string | null; parents?: string; top: number; onClick: () => void }) {
  return (
    <button className={`sn-grid person${parents == null ? " no-parents" : ""}`} style={{ transform: `translateY(${top}px)` }} onClick={onClick}>
      {/* The name keeps its room; the maiden name shrinks first (as in Osoby). */}
      <span className="row" style={{ gap: 8, minWidth: 0 }}>
        <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, flex: "0 1 auto", minWidth: 90 }}>
          {name}
        </span>
        {maiden && (
          <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)", flex: "0 8 auto", minWidth: 24 }}>
            z d. {maiden}
          </span>
        )}
      </span>
      <span className="num nowrap muted">{years}</span>
      <span className="muted ellipsis" title={place ?? undefined}>
        {town(place)}
      </span>
      {parents != null && <span className="muted ellipsis">{parents}</span>}
    </button>
  );
}
