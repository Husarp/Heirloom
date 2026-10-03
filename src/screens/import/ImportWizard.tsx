// Import (spec §4.10–§4.14): the built-in converter from the researcher's AI answers and files. Five steps; the batch
// lives in the backend and nothing is written before „Zatwierdź” in the last step.

import { Check, CircleCheck, Network, RotateCcw, Users } from "lucide-react";
import { Fragment, useCallback, useEffect, useLayoutEffect, useState, type ReactNode } from "react";
import { call, type ApiError } from "../../api/transport";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Spinner } from "../../components/bits";
import { count } from "../../lib/format";
import { StepCheck } from "./StepCheck";
import { StepFiles } from "./StepFiles";
import { StepLoad } from "./StepLoad";
import { StepMatch } from "./StepMatch";
import { StepSummary } from "./StepSummary";
import { useWizard, type CommitResult, type ImportState, type PastImport, type Step } from "./types";
import { askUndoImport, undoImport } from "./undoImport";
import "./import.css";

const STEPS = ["Wczytaj", "Sprawdź", "Dopasuj osoby", "Zdjęcia i pliki", "Podsumowanie"];

export function ImportWizard() {
  const archiveId = useStore((s) => s.archive?.archiveId ?? null);
  const wizardArchive = useWizard((w) => w.archiveId);
  const step = useWizard((w) => w.step);
  const done = useWizard((w) => w.done);
  const setWizard = useWizard((w) => w.set);
  const notify = useStore((s) => s.notify);
  const [state, setState] = useState<ImportState | null>(null);
  const [ready, setReady] = useState(false);

  // Another archive was opened since: its import starts from scratch (the backend dropped the old batch too).
  useLayoutEffect(() => {
    if (wizardArchive === archiveId) return;
    useWizard.getState().reset();
    setWizard({ archiveId });
  }, [archiveId, wizardArchive, setWizard]);

  // Coming back to the Import: the batch may still be there (or gone, after opening another archive).
  useEffect(() => {
    call<ImportState | null>("import.state")
      .then((s) => {
        setState(s);
        if (!s && useWizard.getState().step > 1) setWizard({ step: 1 });
      })
      .catch(() => {})
      .finally(() => setReady(true));
  }, [setWizard]);

  const act = useCallback(
    async (method: string, args?: object) => {
      try {
        const next = await call<ImportState | null>(method, args);
        setState(next);
        return next;
      } catch (e) {
        notify((e as ApiError).message, { kind: "err" });
        return null;
      }
    },
    [notify],
  );

  const goStep = (s: Step) => setWizard({ step: s, furthest: Math.max(useWizard.getState().furthest, s) as Step });
  const title = step === 1 || !state ? "Nowy import" : `Import · ${state.batch.name}`;

  if (wizardArchive !== archiveId) return null;
  if (done) return <Done result={done} />;

  return (
    <div className="imp">
      <header className="imp-head">
        <span className="imp-title" title={title}>
          {title}
        </span>
        {STEPS.map((label, k) => {
          const n = (k + 1) as Step;
          const kind = n < step ? "done" : n === step ? "current" : "future";
          return (
            <Fragment key={label}>
              <button className={`imp-step ${kind}`} disabled={kind !== "done"} onClick={() => goStep(n)}>
                <span className="imp-step-dot">{kind === "done" ? <Check size={13} strokeWidth={3} /> : n}</span>
                <span className="imp-step-label">{label}</span>
              </button>
              {k < STEPS.length - 1 && <span className="imp-step-sep" />}
            </Fragment>
          );
        })}
      </header>
      {!ready ? (
        <div className="imp-body center">
          <Spinner size={20} />
        </div>
      ) : step === 1 || !state ? (
        <StepLoad state={state} setState={setState} act={act} next={() => goStep(2)} />
      ) : step === 2 ? (
        <StepCheck state={state} act={act} goStep={goStep} />
      ) : step === 3 ? (
        <StepMatch state={state} act={act} goStep={goStep} />
      ) : step === 4 ? (
        <StepFiles state={state} act={act} goStep={goStep} />
      ) : (
        <StepSummary state={state} act={act} goStep={goStep} />
      )}
    </div>
  );
}

/** The bar under each step (2–5): status on the left, buttons on the right. */
export function Footer({ status, children }: { status: ReactNode; children: ReactNode }) {
  return (
    <footer className="imp-foot">
      <span className="imp-foot-status">{status}</span>
      <span className="grow" />
      {children}
    </footer>
  );
}

/** After „Zatwierdź”: what was saved, and the way back (spec §4.14, DESIGNER_ANSWERS). */
function Done({ result }: { result: CommitResult }) {
  const go = useStore((s) => s.go);
  const requireEdit = useStore((s) => s.requireEdit);
  const reset = useWizard((w) => w.reset);
  // Undone here, from the import's toast or anywhere else: the history knows (fetched again after every change).
  const { data: past } = useApi<PastImport[]>("import.history");
  const entry = past?.find((h) => h.name === result.batch);
  const undo = () => askUndoImport(result.batch, () => requireEdit(() => undoImport(result.batch)));
  const parts = [
    result.people > 0 && count(result.people, "nowa osoba", "nowe osoby", "nowych osób"),
    result.merged > 0 && count(result.merged, "połączona", "połączone", "połączonych"),
    result.facts > 0 && count(result.facts, "fakt", "fakty", "faktów"),
    result.texts > 0 && count(result.texts, "tekst", "teksty", "tekstów"),
    result.files > 0 && count(result.files, "plik", "pliki", "plików"),
  ].filter(Boolean);
  return (
    <div className="imp">
      <div className="imp-done">
        <span className="imp-done-icon">
          <CircleCheck size={30} />
        </span>
        <span className="imp-done-title">{result.message}</span>
        <span style={{ color: "var(--text2)", fontSize: 14 }}>
          Paczka „{result.batch}” jest zapisana w archiwum{parts.length > 0 ? `: ${parts.join(" · ")}` : ""}.
        </span>
        <div className="row" style={{ gap: 10, marginTop: 8 }}>
          <button className="btn primary" onClick={() => go({ name: "tree", view: "family" })}>
            <Network size={15} />
            Pokaż w drzewie
          </button>
          <button className="btn secondary" onClick={() => go({ name: "people" })}>
            <Users size={15} />
            Osoby
          </button>
          <button className="btn secondary" onClick={reset}>
            Nowy import
          </button>
        </div>
        <button className="btn ghost sm" style={{ marginTop: 4 }} disabled={entry?.active === false} onClick={undo}>
          <RotateCcw size={14} />
          {entry?.undone ? "Cofnięto — zapisz, aby usunąć z pliku" : entry?.active === false ? "Nie ma czego cofnąć — wszystko zmieniono później" : "Cofnij import"}
        </button>
      </div>
    </div>
  );
}
