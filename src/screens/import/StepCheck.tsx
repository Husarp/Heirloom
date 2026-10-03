// Import · 2 Sprawdź (spec §4.11): the package at a glance, problems grouped with their fixes, the AI's questions.
// „Dalej” stays off while there are errors.

import { ArrowRight, CircleCheck, CircleX, Info, MessageCircleQuestion, TriangleAlert } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Dialog } from "../../components/Dialog";
import { Segmented } from "../../components/bits";
import { count } from "../../lib/format";
import { Footer } from "./ImportWizard";
import { useWizard, type Act, type Answer, type ImportIssue, type ImportState, type Step } from "./types";

const GROUPS = [
  { level: "error", label: "Błędy", icon: CircleX, color: "var(--err)" },
  { level: "warning", label: "Ostrzeżenia", icon: TriangleAlert, color: "var(--warn)" },
  { level: "info", label: "Informacje", icon: Info, color: "var(--text3)" },
] as const;

/** "2026-09-28" → "28.09.2026"; anything else as written. */
export function dayText(iso: string | null): string | null {
  const m = iso?.match(/^(\d{4})-(\d{2})-(\d{2})/);
  return m ? `${m[3]}.${m[2]}.${m[1]}` : iso;
}

export function PackageBar({ state }: { state: ImportState }) {
  const b = state.batch;
  const c = b.counts;
  const counters: [number, string, string, string][] = [
    [c.persons, "osoba", "osoby", "osób"],
    [c.events, "zdarzenie", "zdarzenia", "zdarzeń"],
    [c.relationships, "relacja", "relacje", "relacji"],
    [c.texts, "tekst", "teksty", "tekstów"],
    [c.files, "plik", "pliki", "plików"],
    [c.sources, "źródło", "źródła", "źródeł"],
  ];
  return (
    <div className="imp-package">
      <span style={{ color: "var(--text2)" }}>
        {b.author && (
          <>
            Przygotował(a) <b>{b.author}</b> ·{" "}
          </>
        )}
        {b.created && <>{dayText(b.created)} · </>}
        {b.totalParts && b.totalParts > 1 ? (
          <>
            części <b>{b.parts.length} z {b.totalParts}</b>
          </>
        ) : (
          "jedna część"
        )}
      </span>
      <span className="grow" />
      {counters.map(([n, one, few, many]) => (
        <span key={one} className="imp-counter">
          <span className="serif num">{n}</span>
          <span>{count(n, one, few, many).replace(/^[\d\s  ]+/, "")}</span>
        </span>
      ))}
    </div>
  );
}

export function StepCheck({ state, act, goStep }: { state: ImportState; act: Act; goStep: (s: Step) => void }) {
  const setWizard = useWizard((w) => w.set);
  const text = useWizard((w) => w.text);
  const [pasting, setPasting] = useState(false);
  const infos = state.issues.filter((i) => i.level === "info").length;
  const unanswered = state.questions.filter((q) => !q.answer).length;

  const banner =
    state.errors > 0
      ? { kind: "err", icon: <CircleX size={22} color="var(--err)" />, title: `${count(state.errors, "błąd", "błędy", "błędów")} — import zablokowany` }
      : state.warnings > 0
        ? { kind: "warn", icon: <TriangleAlert size={22} color="var(--warn)" />, title: `${count(state.warnings, "ostrzeżenie", "ostrzeżenia", "ostrzeżeń")} — możesz przejść dalej` }
        : { kind: "ok", icon: <CircleCheck size={22} color="var(--accent-text)" />, title: "Wszystko w porządku" };
  const details = [
    state.errors > 0 && state.warnings > 0 && count(state.warnings, "ostrzeżenie", "ostrzeżenia", "ostrzeżeń"),
    state.questions.length > 0 && count(state.questions.length, "pytanie od AI", "pytania od AI", "pytań od AI"),
    infos > 0 && count(infos, "informacja", "informacje", "informacji"),
  ].filter(Boolean);

  const pasteMissing = async (more: string) => {
    // Kept in step 1's text too, so going back there doesn't lose it.
    setWizard({ text: `${text.trimEnd()}\n\n${more}` });
    const next = await act("import.load", { texts: [more], add: true });
    if (next) setPasting(false);
  };

  return (
    <>
      <PackageBar state={state} />
      <div className="imp-body imp-check">
        <div className="col scroll" style={{ gap: 14, minHeight: 0 }}>
          <div className={`imp-banner ${banner.kind}`}>
            {banner.icon}
            <span className="col" style={{ gap: 2 }}>
              <span style={{ fontSize: 15, fontWeight: 600 }}>{banner.title}</span>
              {details.length > 0 && <span style={{ fontSize: 13, color: "var(--text2)" }}>{details.join(" · ")}</span>}
            </span>
          </div>
          {GROUPS.map((g) => {
            const list = state.issues.filter((i) => i.level === g.level);
            if (list.length === 0) return null;
            return (
              <div key={g.level} className="col" style={{ gap: 6 }}>
                <span className="label-caps">
                  {g.label} <span style={{ color: "var(--text2)" }}>{list.length}</span>
                </span>
                <div className="card">
                  {list.map((issue, k) => (
                    <IssueRow key={`${issue.message}-${k}`} issue={issue} icon={<g.icon size={17} color={g.color} />} state={state} act={act} goStep={goStep} onPaste={() => setPasting(true)} />
                  ))}
                </div>
              </div>
            );
          })}
        </div>
        <div className="col scroll" style={{ gap: 12, minHeight: 0 }}>
          <span className="label-caps row" style={{ gap: 6, color: "var(--accent-text)" }}>
            <MessageCircleQuestion size={14} />
            Pytania od AI · {state.questions.length}
          </span>
          {state.questions.length === 0 ? (
            <span style={{ fontSize: 13, color: "var(--text3)" }}>AI nie zadało pytań w tej paczce.</span>
          ) : (
            <>
              {state.questions.map((q) => (
                <QuestionCard key={q.id} question={q} act={act} />
              ))}
              <span style={{ fontSize: 12, lineHeight: 1.5, color: "var(--text3)" }}>Odpowiedź zapisze się jako uwaga badawcza przy osobie.</span>
            </>
          )}
        </div>
      </div>
      <Footer
        status={
          state.errors > 0 ? (
            <span className="row" style={{ gap: 6, color: "var(--err)", fontWeight: 500 }}>
              <CircleX size={15} />
              Napraw błąd, aby przejść dalej
            </span>
          ) : unanswered > 0 ? (
            `${count(unanswered, "pytanie czeka", "pytania czekają", "pytań czeka")} na odpowiedź — możesz odpowiedzieć później`
          ) : (
            "Paczka sprawdzona"
          )
        }
      >
        <button className="btn secondary" onClick={() => goStep(1)}>
          Wstecz
        </button>
        <button className="btn primary" disabled={state.errors > 0} onClick={() => goStep(3)}>
          Dalej
          <ArrowRight size={15} />
        </button>
      </Footer>
      {pasting && <PasteDialog onClose={() => setPasting(false)} onPaste={pasteMissing} />}
    </>
  );
}

function IssueRow({ issue, icon, state, act, goStep, onPaste }: { issue: ImportIssue; icon: ReactNode; state: ImportState; act: Act; goStep: (s: Step) => void; onPaste: () => void }) {
  const setWizard = useWizard((w) => w.set);
  const [fixing, setFixing] = useState(false);
  const [date, setDate] = useState("");
  const target = issue.target;
  const file = target && /^M\d+$/.test(target) ? state.files.find((f) => f.file === target) : undefined;
  const person = target ? state.persons.find((p) => p.id === target) : undefined;
  const skipped = file?.status === "skipped";

  const fix = async () => {
    if (await act("import.fixDate", { event: target, date })) setFixing(false);
  };

  let actions: ReactNode = null;
  if (issue.action === "paste") {
    actions = (
      <button className="btn primary xs imp-act" onClick={onPaste}>
        Wklej brakującą część
      </button>
    );
  } else if (issue.action === "fix" && target) {
    actions = fixing ? (
      <span className="row" style={{ gap: 6 }}>
        <input className="input sm" style={{ width: 150, height: 30 }} autoFocus value={date} onChange={(e) => setDate(e.target.value)} onKeyDown={(e) => e.key === "Enter" && fix()} placeholder="np. ok. 1915" />
        <button className="btn primary xs imp-act" disabled={!date.trim()} onClick={fix}>
          Zapisz
        </button>
      </span>
    ) : (
      <button className="btn secondary xs imp-act" onClick={() => setFixing(true)}>
        Popraw
      </button>
    );
  } else if (file) {
    actions = (
      <>
        <button className="btn secondary xs imp-act" onClick={() => (setWizard({ file: file.file }), goStep(4))}>
          Pokaż
        </button>
        <button className="btn secondary xs imp-act" onClick={() => act("import.file", { file: file.file, skip: !skipped })}>
          {skipped ? "Przywróć" : "Pomiń element"}
        </button>
      </>
    );
  } else if (person) {
    actions = (
      <button className="btn secondary xs imp-act" onClick={() => (setWizard({ person: person.id }), goStep(3))}>
        Pokaż
      </button>
    );
  }

  return (
    <div className="imp-issue" style={skipped ? { opacity: 0.55 } : undefined}>
      <span style={{ display: "flex", flex: "none" }}>{icon}</span>
      <span className="col grow" style={{ gap: 2 }}>
        <span style={{ fontSize: 14 }}>{issue.message}</span>
        <span style={{ fontSize: 12, color: "var(--text3)" }}>
          {issue.reference}
          {skipped && " · pominięty"}
        </span>
      </span>
      {actions && (
        <span className="row" style={{ gap: 6, flex: "none" }}>
          {actions}
        </span>
      )}
    </div>
  );
}

export function QuestionCard({ question: q, act, compact }: { question: ImportState["questions"][number]; act: Act; compact?: boolean }) {
  const [note, setNote] = useState(q.note);
  const answer = (value: Answer | "") => act("import.answer", { question: q.id, answer: value, note });
  return (
    <div className={`imp-question${compact ? " compact" : ""}`}>
      <span style={{ fontSize: compact ? 14 : 15, lineHeight: 1.45, fontWeight: 500 }}>{q.text}</span>
      {q.about.length > 0 && <span style={{ fontSize: 12, color: "var(--text3)" }}>{q.about.map((a) => `${a.name ?? a.id} (${a.id})`).join(", ")}</span>}
      <Segmented<Answer | "">
        full
        size={30}
        value={q.answer ?? ""}
        onChange={answer}
        options={[
          { value: "yes", label: "Tak" },
          { value: "no", label: "Nie" },
          { value: "unknown", label: "Nie wiem" },
        ]}
      />
      <input className="input sm" value={note} onChange={(e) => setNote(e.target.value)} onBlur={() => note !== q.note && answer(q.answer ?? "")} placeholder="Notatka (opcjonalnie)" />
    </div>
  );
}

function PasteDialog({ onClose, onPaste }: { onClose: () => void; onPaste: (text: string) => void }) {
  const [text, setText] = useState("");
  return (
    <Dialog width={620} onClose={onClose}>
      <div className="dialog-body">
        <div className="dialog-title">Wklej brakującą część</div>
        <div className="dialog-text">Napisz AI „dalej” albo poproś o brakującą część, a potem wklej tutaj całą odpowiedź. Decyzje podjęte do tej pory zostaną.</div>
        <textarea className="input imp-paste-area" style={{ height: 220 }} autoFocus value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} />
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" disabled={!text.trim()} onClick={() => onPaste(text)}>
          Dodaj do paczki
        </button>
      </div>
    </Dialog>
  );
}
