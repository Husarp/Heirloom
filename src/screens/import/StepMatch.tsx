// Import · 3 Dopasuj osoby (spec §4.12): for each person in the batch, New / Merge / Skip; the candidates with their
// match percentage; field by field what the archive has and what the batch brings; where they land in the tree.

import { ArrowRight, Check, Library, Merge, MessageCircleQuestion, Search, SkipForward, UserPlus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { call } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { useApi } from "../../app/useApi";
import { Avatar, Segmented, useDismiss } from "../../components/bits";
import { count } from "../../lib/format";
import type { Graph } from "../tree/graph";
import { Footer } from "./ImportWizard";
import { MiniLegend, MiniTree, familyOf, mergeTarget } from "./MiniTree";
import { QuestionCard } from "./StepCheck";
import { useWizard, type Act, type CompareRow, type ImportPerson, type ImportState, type Step } from "./types";

type Tab = "all" | "undecided" | "new" | "merged";

const CHOICE_LABEL: Record<string, string> = { keep: "Zachowaj", replace: "Zastąp", variant: "Wariant", add: "Dodaj", skip: "Pomiń" };

export function StatusBadge({ p }: { p: ImportPerson }) {
  const percent = p.candidates[0]?.percent;
  switch (p.status) {
    case "new":
      return <span className="badge new">Nowa</span>;
    case "merged":
      return <span className="badge accent">Połączona</span>;
    case "skipped":
      return <span className="badge outline">Pominięta</span>;
    case "match":
      return <span className="badge accent num">Połącz? {percent}%</span>;
    default:
      return <span className="badge num">{percent != null ? `Połącz? ${percent}%` : "Do decyzji"}</span>;
  }
}

export function StepMatch({ state, act, goStep }: { state: ImportState; act: Act; goStep: (s: Step) => void }) {
  const chosen = useWizard((w) => w.person);
  const setWizard = useWizard((w) => w.set);
  const [tab, setTab] = useState<Tab>("all");
  const persons = state.persons;
  const undecided = (p: ImportPerson) => p.decision === "undecided";
  const tabs: [Tab, string, number][] = [
    ["all", "Wszystkie", persons.length],
    ["undecided", "Do decyzji", persons.filter(undecided).length],
    ["new", "Nowe", persons.filter((p) => p.decision === "new").length],
    ["merged", "Połączone", persons.filter((p) => p.decision === "merge").length],
  ];
  const shown = persons.filter((p) => (tab === "all" ? true : tab === "undecided" ? undecided(p) : tab === "new" ? p.decision === "new" : p.decision === "merge"));
  const selected = persons.find((p) => p.id === chosen) ?? persons.find(undecided) ?? persons[0];
  useEffect(() => {
    if (selected && selected.id !== chosen) setWizard({ person: selected.id });
  }, [selected, chosen, setWizard]);

  const waiting = persons.filter(undecided);
  const nextUndecided = () => {
    const at = persons.findIndex((p) => p.id === selected?.id);
    const next = waiting.find((p) => persons.indexOf(p) > at && p.id !== selected?.id) ?? waiting.find((p) => p.id !== selected?.id);
    if (next) setWizard({ person: next.id });
  };

  // The archive around the person they would be joined with, for the mini tree.
  const target = selected ? mergeTarget(selected) : null;
  const { data: graph } = useApi<Graph>(target ? "tree.graph" : null, { id: target, up: 1, down: 1 });
  const family = useMemo(() => (selected ? familyOf(state, selected.id, target && graph?.focus === target ? graph : null) : null), [state, selected, target, graph]);

  return (
    <>
      <div className="imp-match">
        <aside className="imp-people">
          <div className="imp-people-head">
            <span style={{ fontSize: 14, fontWeight: 600 }}>
              Osoby w paczce <span style={{ fontWeight: 400, color: "var(--text3)" }}>{persons.length}</span>
            </span>
            <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
              {tabs.map(([key, label, n]) => (
                <button key={key} className={`imp-tab${tab === key ? " on" : ""}`} onClick={() => setTab(key)}>
                  {label} {n}
                </button>
              ))}
            </div>
          </div>
          <div className="scroll grow">
            {shown.map((p) => (
              <button key={p.id} className={`imp-person-row${p.id === selected?.id ? " on" : ""}`} onClick={() => setWizard({ person: p.id })}>
                <Avatar initials={p.initials} size={30} dashed={p.decision === "new"} />
                <span className="col grow" style={{ minWidth: 0, alignItems: "flex-start" }}>
                  <span className="ellipsis" style={{ fontSize: 14, fontWeight: 500, maxWidth: "100%" }}>
                    {p.name}
                  </span>
                  <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)", maxWidth: "100%" }}>
                    {[p.maiden && `z d. ${p.maiden}`, p.years].filter(Boolean).join(" · ") || p.id}
                  </span>
                </span>
                <StatusBadge p={p} />
              </button>
            ))}
            {shown.length === 0 && <div style={{ padding: "14px 16px", fontSize: 13, color: "var(--text3)" }}>Nikogo w tej grupie.</div>}
          </div>
        </aside>
        {selected ? <Decision key={selected.id} p={selected} state={state} act={act} /> : <div />}
        <aside className="imp-where">
          <span style={{ fontSize: 14, fontWeight: 600 }}>Gdzie trafią do drzewa</span>
          {family && <MiniTree family={family} width={198} height={330} cardW={88} cardH={36} />}
          <MiniLegend />
        </aside>
      </div>
      <Footer
        status={
          <>
            Krok 3 z 5 · {waiting.length > 0 ? <b>{count(waiting.length, "osoba czeka", "osoby czekają", "osób czeka")} na decyzję</b> : "każda osoba ma decyzję"} · {count(state.summary.new, "nowa", "nowe", "nowych")} ·{" "}
            {count(state.summary.merged, "połączona", "połączone", "połączonych")}
          </>
        }
      >
        <button className="btn secondary" onClick={() => goStep(2)}>
          Wstecz
        </button>
        {waiting.length > 0 && !(waiting.length === 1 && waiting[0].id === selected?.id) ? (
          <button className="btn primary" onClick={nextUndecided}>
            Następna do decyzji
            <ArrowRight size={15} />
          </button>
        ) : (
          <button className="btn primary" disabled={waiting.length > 0} onClick={() => goStep(4)}>
            Dalej
            <ArrowRight size={15} />
          </button>
        )}
      </Footer>
    </>
  );
}

function Decision({ p, state, act }: { p: ImportPerson; state: ImportState; act: Act }) {
  const [searching, setSearching] = useState(false);
  const questions = state.questions.filter((q) => q.about.some((a) => a.id === p.id));
  const target = p.target;
  const main = p.candidates.find((c) => c.id === target) ?? p.candidates[0];
  const others = p.candidates.filter((c) => c.id !== main?.id).slice(0, 3);
  // Someone picked by hand (not among the suggestions).
  const manual = target && !p.candidates.some((c) => c.id === target) ? target : null;
  const { data: manualPanel } = useApi<{ person: PersonSummary }>(manual ? "person.panel" : null, { id: manual });
  const merging = p.decision === "merge";

  const decide = (kind: "new" | "merge" | "skip", to?: string) => {
    if (kind === "merge" && !to && !target && p.candidates.length === 0) {
      setSearching(true);
      return;
    }
    act("import.decide", { person: p.id, kind, target: kind === "merge" ? (to ?? target ?? main?.id) : undefined });
  };

  return (
    <section className="imp-decision scroll">
      <div className="row" style={{ gap: 12, alignItems: "flex-end" }}>
        <span className="col grow" style={{ gap: 2 }}>
          <span style={{ fontSize: 12, color: "var(--text3)" }}>
            Z paczki · {p.id} · {count(p.facts, "fakt", "fakty", "faktów")}
            {p.files.length > 0 && `, ${count(p.files.length, "plik", "pliki", "plików")}`}
          </span>
          <span className="imp-person-name">{p.name}</span>
        </span>
        <Segmented
          value={p.decision === "merge" ? "merge" : p.decision === "skip" ? "skip" : p.decision === "new" ? "new" : ("" as "new")}
          onChange={(v) => decide(v)}
          size={30}
          options={[
            { value: "new", label: "Nowa", icon: <UserPlus size={14} /> },
            { value: "merge", label: "Połącz", icon: <Merge size={14} /> },
            { value: "skip", label: "Pomiń", icon: <SkipForward size={14} /> },
          ]}
        />
      </div>

      {(main || manual) && p.decision !== "skip" && (
        <div className="col" style={{ gap: 8 }}>
          {manual ? (
            <CandidateCard person={manualPanel?.person ?? null} fallback={manual} note="wybrano ręcznie" on={merging} onPick={() => decide("merge", manual)} />
          ) : (
            main && <CandidateCard person={main} context={[main.years, main.context].filter(Boolean).join(" · ")} percent={main.percent} on={merging && target === main.id} onPick={() => decide("merge", main.id)} />
          )}
          {(manual ? p.candidates.slice(0, 3) : others).map((c) => (
            <span key={c.id} style={{ fontSize: 12, color: "var(--text2)" }}>
              Inny kandydat:{" "}
              <button className="link" onClick={() => decide("merge", c.id)}>
                {c.name}, {[c.years, c.context].filter(Boolean).join(", ")} ({c.percent}%)
              </button>
            </span>
          ))}
        </div>
      )}
      {p.decision !== "skip" && (searching ? <PersonSearch onPick={(id) => (setSearching(false), decide("merge", id))} onClose={() => setSearching(false)} /> : (
        <button className="link" style={{ alignSelf: "flex-start", fontSize: 12 }} onClick={() => setSearching(true)}>
          <Search size={12} style={{ verticalAlign: -2, marginRight: 4 }} />
          {p.candidates.length > 0 ? "Połącz z inną osobą z archiwum…" : "Połącz z osobą z archiwum…"}
        </button>
      ))}

      {merging && p.sameTarget.length > 0 && (
        <div className="imp-note-box">
          Ta sama osoba z archiwum co {p.sameTarget.map((o) => `${o.name} (${o.id})`).join(", ")} — ich dane zostaną połączone w jedną osobę.
        </div>
      )}
      {p.compare.length > 0 && p.decision !== "skip" && p.decision !== "new" && <Compare p={p} act={act} />}
      {p.decision === "new" && (
        <div className="imp-note-box">
          {p.name} trafi do archiwum jako nowa osoba{p.facts > 0 ? ` z ${count(p.facts, "faktem", "faktami", "faktami")}` : ""}.
          {p.candidates.length > 0 && " Jeśli to ktoś z archiwum, wybierz „Połącz”."}
        </div>
      )}
      {p.decision === "skip" && <div className="imp-note-box">Ta osoba i jej fakty nie zostaną zapisane. Relacje z nią też zostaną pominięte.</div>}

      {questions.length > 0 && (
        <div className="imp-small-card">
          <span className="label-caps row" style={{ gap: 6, color: "var(--accent-text)" }}>
            <MessageCircleQuestion size={13} />
            Pytanie z paczki
          </span>
          {questions.map((q) => (
            <QuestionCard key={q.id} question={q} act={act} compact />
          ))}
        </div>
      )}
    </section>
  );
}

function CandidateCard({
  person,
  fallback,
  context,
  percent,
  note,
  on,
  onPick,
}: {
  person: PersonSummary | null;
  fallback?: string;
  context?: string;
  percent?: number;
  note?: string;
  on: boolean;
  onPick: () => void;
}) {
  return (
    <button className={`imp-candidate${on ? " on" : ""}`} onClick={onPick}>
      <span className={`imp-radio${on ? " on" : ""}`} />
      <Avatar initials={person?.initials ?? "?"} branch={person?.branch} photo={person?.photo} size={34} />
      <span className="col grow" style={{ minWidth: 0, alignItems: "flex-start" }}>
        <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, maxWidth: "100%" }}>
          {person?.name ?? fallback}
        </span>
        {context && (
          <span className="ellipsis" style={{ fontSize: 12, color: "var(--text2)", maxWidth: "100%" }}>
            {context}
          </span>
        )}
      </span>
      {percent != null ? (
        <span className="col" style={{ alignItems: "flex-end" }}>
          <span className="num" style={{ fontSize: 15, fontWeight: 700, color: "var(--accent-text)" }}>
            {percent}%
          </span>
          <span style={{ fontSize: 11, color: "var(--text3)" }}>dopasowanie</span>
        </span>
      ) : (
        <span style={{ fontSize: 11, color: "var(--text3)" }}>{note}</span>
      )}
    </button>
  );
}

function Compare({ p, act }: { p: ImportPerson; act: Act }) {
  return (
    <div className="card imp-compare">
      <div className="imp-compare-row head">
        <span>Pole</span>
        <span>W archiwum</span>
        <span>Z importu</span>
        <span>Decyzja</span>
      </div>
      {p.compare.map((row) => (
        <CompareLine key={row.field} row={row} onChoose={(choice) => act("import.field", { person: p.id, field: row.field, choice })} />
      ))}
    </div>
  );
}

function CompareLine({ row, onChoose }: { row: CompareRow; onChoose: (choice: string) => void }) {
  const basis = row.evidence?.basis;
  return (
    <div className="imp-compare-row">
      <span style={{ color: "var(--text2)" }}>{row.label}</span>
      <span style={{ color: row.archive ? undefined : "var(--text3)" }}>{row.archive ?? "—"}</span>
      <span className="col" style={{ gap: 3, minWidth: 0 }}>
        <span style={{ fontWeight: row.same ? 400 : 600, color: row.import ? undefined : "var(--text3)" }}>{row.import ?? "—"}</span>
        {row.evidence && (row.evidence.source || basis) && (
          <span className="imp-evidence">
            <Library size={11} style={{ flex: "none" }} />
            {row.evidence.source && <span className="ellipsis">{row.evidence.source}</span>}
            {basis && <span className={`imp-basis ${basis === "stated" ? "stated" : ""}`}>{basis === "stated" ? "podane wprost" : "wywnioskowane"}</span>}
          </span>
        )}
      </span>
      {row.import == null ? (
        <span style={{ color: "var(--text3)" }}>bez zmian</span>
      ) : row.same || row.options.length === 0 ? (
        <span className="row" style={{ gap: 5, color: "var(--text3)" }}>
          <Check size={14} />
          zgodne
        </span>
      ) : (
        <Segmented full size={26} value={row.choice} onChange={onChoose} options={row.options.map((o) => ({ value: o, label: CHOICE_LABEL[o] ?? o }))} />
      )}
    </div>
  );
}

type Hit = PersonSummary & { context: string };

function PersonSearch({ onPick, onClose }: { onPick: (id: string) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const ref = useDismiss<HTMLDivElement>(true, onClose);
  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    // Only the answer for the latest query counts (an earlier, slower one mustn't replace it).
    let latest = true;
    call<Hit[]>("people.search", { q: query, limit: 8 })
      .then((h) => latest && setHits(h))
      .catch(() => {});
    return () => {
      latest = false;
    };
  }, [query]);
  return (
    <div ref={ref} className="col" style={{ gap: 4, position: "relative" }}>
      <div className="search-box">
        <Search size={14} />
        <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={(e) => (e.key === "Escape" ? onClose() : e.key === "Enter" && hits[0] && onPick(hits[0].id))} placeholder="Szukaj osoby w archiwum…" />
      </div>
      {hits.length > 0 && (
        <div className="card" style={{ padding: "4px 0" }}>
          {hits.map((h) => (
            <button key={h.id} className="menu-item" style={{ minHeight: 44, gap: 10 }} onClick={() => onPick(h.id)}>
              <Avatar initials={h.initials} branch={h.branch} photo={h.photo} size={26} />
              <span className="col grow" style={{ minWidth: 0 }}>
                <span className="ellipsis" style={{ fontWeight: 500 }}>
                  {h.name}
                </span>
                <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                  {h.context}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
