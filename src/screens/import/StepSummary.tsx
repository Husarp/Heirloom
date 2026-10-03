// Import · 5 Podsumowanie (spec §4.14): every change to be saved, each person and field can be unticked; who imports and
// a note for the change history. Only what is ticked is saved, and the whole import can be undone later.

import { ArrowRight, Check, ChevronDown, ChevronRight, ShieldCheck, TriangleAlert } from "lucide-react";
import { useMemo, useState } from "react";
import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Checkbox, Spinner } from "../../components/bits";
import { count, people as peopleCount, plural } from "../../lib/format";
import type { Graph } from "../tree/graph";
import { Footer } from "./ImportWizard";
import { MiniLegend, MiniTree, familyOf, mergeTarget } from "./MiniTree";
import { askUndoImport } from "./undoImport";
import { useWizard, type Act, type CommitResult, type CompareRow, type ImportPerson, type ImportState, type Step } from "./types";

/** A merged person's lines that would change something in the archive. */
function changeLines(p: ImportPerson): CompareRow[] {
  return p.compare.filter((r) => !r.same && r.options.length > 0);
}

function lineOn(p: ImportPerson, r: CompareRow): boolean {
  return !p.excludedFields.includes(r.field) && r.choice !== "keep" && r.choice !== "skip";
}

export function StepSummary({ state, act, goStep }: { state: ImportState; act: Act; goStep: (s: Step) => void }) {
  const app = useStore((s) => s.app);
  const archive = useStore((s) => s.archive);
  const editor = useStore((s) => s.editor);
  const whenSaved = useStore((s) => s.whenSaved);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const requireEdit = useStore((s) => s.requireEdit);
  const setWizard = useWizard((w) => w.set);
  const reset = useWizard((w) => w.reset);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [seen, setSeen] = useState<Set<string>>(new Set());
  const [author, setAuthor] = useState(editor ?? app?.lastEditor ?? archive?.editors[0]?.name ?? "");
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);

  // Rows worth a second look: people the check warned about, and suggestions that were only probable.
  const flagged = useMemo(() => {
    const ids = new Set(state.issues.filter((i) => i.level === "warning" && i.target).map((i) => i.target!));
    for (const p of state.persons) if (p.status === "review" || (p.decision === "merge" && (p.candidates.find((c) => c.id === p.target)?.percent ?? 100) < 80)) ids.add(p.id);
    return ids;
  }, [state]);
  const rows = [...state.persons].sort((a, b) => Number(a.decision === "skip") - Number(b.decision === "skip"));
  const toReview = rows.filter((p) => flagged.has(p.id) && p.decision !== "skip");
  const included = state.persons.filter((p) => !p.excluded && (p.decision === "new" || p.decision === "merge"));
  const expandable = rows.filter((p) => p.decision === "merge" && changeLines(p).length > 0);

  const toggle = (id: string) => {
    setOpen((o) => {
      const next = new Set(o);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setSeen((s) => new Set(s).add(id));
  };

  // The mini tree: someone joined with the archive who brings new relatives, else the first person saved.
  const anchor = useMemo(() => {
    const merged = included.filter((p) => p.decision === "merge");
    const bringsNew = merged.find((p) =>
      state.relationships.some((r) => {
        const other = r.child === p.id ? r.parent : r.parent === p.id ? r.child : r.a === p.id ? r.b : r.b === p.id ? r.a : null;
        return other && state.persons.find((x) => x.id === other)?.decision === "new";
      }),
    );
    return bringsNew ?? merged[0] ?? included[0] ?? null;
  }, [included, state]);
  const target = anchor ? mergeTarget(anchor) : null;
  const { data: graph } = useApi<Graph>(target ? "tree.graph" : null, { id: target, up: 1, down: 1 });
  const family = useMemo(() => (anchor ? familyOf(state, anchor.id, target && graph?.focus === target ? graph : null) : null), [anchor, state, target, graph]);

  // Saving the import changes the family data like any edit, so it goes through „Kto edytuje?” (and a read-only
  // archive stops it there). The name given there stands in for an empty „Kto importuje”.
  const commit = () =>
    requireEdit(() => {
      const who = author.trim() || useStore.getState().editor?.trim() || "";
      if (!who) {
        notify("Wpisz, kto importuje — to imię trafi do historii zmian.", { kind: "err" });
        return;
      }
      commitAs(who);
    });

  const commitAs = (who: string, allowForeign = false) =>
    whenSaved(async () => {
      setSaving(true);
      try {
        const result = await call<CommitResult>("import.commit", { author: who, note: note.trim() || null, allowForeign });
        // The import is its own save: „Cofnij zapis” in the bar must not reach back past it.
        useStore.setState({ lastSave: null });
        changed();
        setWizard({ done: result });
        const saved = [
          result.files > 0 && count(result.files, "plik", "pliki", "plików"),
          result.merged > 0 && count(result.merged, "połączona osoba", "połączone osoby", "połączonych osób"),
        ].filter(Boolean);
        // Design 17f: the batch and what came with it under the message; undo and the tree as actions.
        notify(result.message, {
          detail: [result.batch, ...saved].join(" · "),
          actions: [{ label: "Pokaż w drzewie", run: () => useStore.getState().go({ name: "tree", view: "family" }) }],
          action: {
            label: "Cofnij import",
            run: () =>
              askUndoImport(result.batch, () =>
                requireEdit(async () => {
                  try {
                    const status = await call<ArchiveStatus>("history.undo", { batch: result.batch });
                    changed(status);
                    notify("Import cofnięty — zmiana czeka na zapis.");
                  } catch (e) {
                    notify((e as ApiError).message, { kind: "err" });
                  }
                }),
              ),
          },
        });
      } catch (e) {
        const error = e as ApiError;
        // The first save into another program's file asks first, like „Zapisz” does (PLAN §11.2 rule 4).
        if (error.code === "foreign")
          setAsk({
            title: "Zapisać import w pliku z innego programu?",
            text: `Plik ${archive?.dataFile ?? ""} utworzył program ${archive?.origin ?? "nieznany"}. Heirloom zapisze go w formacie GEDCOM 7 i zachowa dane, których nie zna. Przed zapisem zrobi kopię zapasową oryginału w folderze .heirloom/kopie.`,
            icon: "warn",
            buttons: [
              { label: "Anuluj", kind: "ghost" },
              { label: "Zapisz w tym pliku", kind: "primary", run: () => commitAs(who, true) },
            ],
          });
        else notify(error.message, { kind: "err" });
      } finally {
        setSaving(false);
      }
    }, "Przed zapisaniem importu");

  const cancel = () =>
    setAsk({
      title: "Anulować ten import?",
      text: "Nic z paczki nie zostało zapisane. Decyzje podjęte w krokach 2–5 przepadną.",
      icon: "warn",
      buttons: [
        { label: "Wróć do importu", kind: "ghost" },
        {
          label: "Anuluj import",
          kind: "danger",
          run: async () => {
            await call("import.cancel").catch(() => {});
            reset();
          },
        },
      ],
    });

  const s = state.summary;
  return (
    <>
      <div className="imp-body imp-summary">
        <div className="col scroll" style={{ gap: 14, minHeight: 0 }}>
          <div className="imp-stats">
            <div className="imp-stat-card">
              <span className="label-caps">Osoby</span>
              <div className="row" style={{ gap: 18, flexWrap: "wrap" }}>
                <Stat n={s.new} label={plural(s.new, "nowa", "nowe", "nowych")} />
                <Stat n={s.merged} label={plural(s.merged, "połączona", "połączone", "połączonych")} />
                <Stat n={s.skipped} label={plural(s.skipped, "pominięta", "pominięte", "pominiętych")} />
              </div>
            </div>
            <div className="imp-stat-card">
              <span className="label-caps">Dane</span>
              <div className="row" style={{ gap: 18, flexWrap: "wrap" }}>
                <Stat n={s.events} label={plural(s.events, "zdarzenie", "zdarzenia", "zdarzeń")} />
                <Stat n={s.relationships} label={plural(s.relationships, "relacja", "relacje", "relacji")} />
                <Stat n={s.texts} label={plural(s.texts, "tekst", "teksty", "tekstów")} />
                <Stat n={s.files} label={`${plural(s.files, "plik", "pliki", "plików")}${s.duplicates > 0 ? ` (${count(s.duplicates, "duplikat", "duplikaty", "duplikatów")})` : ""}`} />
                <Stat n={s.sources} label={plural(s.sources, "źródło", "źródła", "źródeł")} />
              </div>
            </div>
          </div>

          <div className="card">
            <div className="imp-changes-head">
              <span style={{ fontSize: 14, fontWeight: 600 }}>Zmiany do zatwierdzenia</span>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>zaznaczone zostaną zapisane</span>
              <span className="grow" />
              {toReview.length > 0 && (
                <span style={{ fontSize: 12, color: "var(--text2)" }}>
                  Przejrzane{" "}
                  <b>
                    {toReview.filter((p) => seen.has(p.id)).length} z {toReview.length}
                  </b>
                </span>
              )}
              {expandable.length > 0 && (
                <button className="link" onClick={() => (open.size > 0 ? setOpen(new Set()) : (setOpen(new Set(expandable.map((p) => p.id))), setSeen(new Set([...seen, ...expandable.map((p) => p.id)]))))}>
                  {open.size > 0 ? "Zwiń wszystkie" : "Rozwiń wszystkie"}
                </button>
              )}
            </div>
            {rows.map((p) => {
              const lines = p.decision === "merge" ? changeLines(p) : [];
              const skipped = p.decision === "skip";
              const on = !skipped && !p.excluded;
              const review = flagged.has(p.id) && !skipped;
              const expanded = open.has(p.id);
              const detail = skipped
                ? "decyzja w kroku 3"
                : p.decision === "merge"
                  ? lines.length > 0
                    ? `${lines.filter((r) => lineOn(p, r)).length} z ${count(lines.length, "zmiany", "zmian", "zmian")}`
                    : "bez zmian w danych"
                  : [p.maiden && `z d. ${p.maiden}`, count(p.facts, "fakt", "fakty", "faktów"), p.files.length > 0 && count(p.files.length, "plik", "pliki", "plików")].filter(Boolean).join(" · ");
              return (
                <div key={p.id}>
                  <div className={`imp-change-row${review ? " review" : ""}`}>
                    <Checkbox on={on} disabled={skipped} onChange={(v) => act("import.include", { person: p.id, include: v })} />
                    <button className="imp-chevron" disabled={lines.length === 0} onClick={() => toggle(p.id)} aria-label={expanded ? "Zwiń" : "Rozwiń"}>
                      {lines.length > 0 && (expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />)}
                    </button>
                    <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, color: on ? undefined : "var(--text3)", minWidth: 0 }}>
                      {p.name}
                    </span>
                    {review && <span style={{ fontSize: 12, fontWeight: 600, color: "var(--warn)", whiteSpace: "nowrap" }}>do przejrzenia</span>}
                    <span className="grow" />
                    <span className={`badge ${skipped ? "outline" : p.decision === "new" ? "new" : "accent"}`}>{skipped ? "pominięta" : p.decision === "new" ? "nowa" : "połączona"}</span>
                    <span className="imp-change-detail ellipsis">{detail}</span>
                  </div>
                  {expanded &&
                    lines.map((r) => (
                      <div key={r.field} className="imp-change-line">
                        <Checkbox on={lineOn(p, r) && on} disabled={!on} onChange={(v) => (v && !lineOn(p, r) && (r.choice === "keep" || r.choice === "skip") ? act("import.field", { person: p.id, field: r.field, choice: r.options.includes("variant") ? "variant" : "add" }).then(() => act("import.include", { person: p.id, field: r.field, include: true })) : act("import.include", { person: p.id, field: r.field, include: v }))} />
                        <span style={{ color: "var(--text2)" }}>{r.label}</span>
                        <span className="ellipsis" style={{ color: "var(--text3)" }}>
                          {r.archive ?? "—"}
                        </span>
                        <ArrowRight size={13} color="var(--text3)" />
                        <span className="ellipsis" style={{ fontWeight: 600 }}>
                          {r.import}
                          {r.choice === "variant" && <span style={{ fontWeight: 400, color: "var(--text3)" }}> (jako wariant)</span>}
                        </span>
                      </div>
                    ))}
                </div>
              );
            })}
          </div>
        </div>

        <div className="col scroll" style={{ gap: 14, minHeight: 0 }}>
          {family && (
            <div className="card" style={{ padding: "12px 14px", display: "flex", flexDirection: "column", gap: 10 }}>
              <span style={{ fontSize: 14, fontWeight: 600 }}>Gdzie dołączą do drzewa</span>
              <MiniTree family={family} width={310} height={210} cardW={96} cardH={34} />
              <MiniLegend />
            </div>
          )}
          {(state.unresolved.mentions > 0 || state.unresolved.questions > 0) && (
            <div className="imp-unresolved">
              <span className="row" style={{ gap: 8, fontWeight: 600 }}>
                <TriangleAlert size={15} color="var(--warn)" />
                Nierozwiązane
              </span>
              {state.unresolved.mentions > 0 && <span>{count(state.unresolved.mentions, "wzmianka zostanie", "wzmianki zostaną", "wzmianek zostanie")} zwykłym tekstem</span>}
              {state.unresolved.questions > 0 && <span>{count(state.unresolved.questions, "pytanie", "pytania", "pytań")} bez odpowiedzi</span>}
              <button className="btn secondary sm" style={{ alignSelf: "flex-start" }} onClick={() => goStep(2)}>
                Wróć do kroku 2
              </button>
            </div>
          )}
          <div className="card" style={{ padding: "14px 16px", display: "flex", flexDirection: "column", gap: 12 }}>
            <label className="field">
              <span className="field-label">Kto importuje</span>
              <input className="input sm" style={{ height: 36 }} list="imp-editors" value={author} onChange={(e) => setAuthor(e.target.value)} placeholder="Imię" />
              <datalist id="imp-editors">
                {archive?.editors.map((e) => (
                  <option key={e.name} value={e.name} />
                ))}
              </datalist>
            </label>
            <label className="field">
              <span className="field-label">Notatka do historii zmian</span>
              <textarea className="input" style={{ height: 60, fontSize: 13 }} value={note} onChange={(e) => setNote(e.target.value)} placeholder="np. akty z Łęcznej, od cioci Heleny" />
            </label>
          </div>
        </div>
      </div>
      <Footer
        status={
          state.undecided > 0 ? (
            <button className="row link" style={{ gap: 7, color: "var(--warn)" }} onClick={() => goStep(3)}>
              <TriangleAlert size={15} />
              {count(state.undecided, "osoba czeka", "osoby czekają", "osób czeka")} na decyzję w kroku 3
            </button>
          ) : (
            <span className="row" style={{ gap: 7 }}>
              <ShieldCheck size={15} color="var(--accent-text)" />
              Nic nie zapisze się bez Twojego zatwierdzenia · import można cofnąć
            </span>
          )
        }
      >
        <button className="btn ghost" onClick={cancel}>
          Anuluj
        </button>
        <button className="btn secondary" onClick={() => goStep(4)}>
          Wróć
        </button>
        <button className="btn primary" disabled={saving || included.length + state.files.filter((f) => f.status === "und" && f.people.length > 0).length === 0 || state.undecided > 0} onClick={commit}>
          {saving ? <Spinner size={15} /> : <Check size={15} />}
          Zatwierdź i zapisz wybrane ({peopleCount(included.length)})
        </button>
      </Footer>
    </>
  );
}

function Stat({ n, label }: { n: number; label: string }) {
  return (
    <span className="col">
      <span className="serif num" style={{ fontSize: 26, lineHeight: 1.2, fontWeight: 600 }}>
        {n}
      </span>
      <span style={{ fontSize: 12, color: "var(--text2)" }}>{label}</span>
    </span>
  );
}
