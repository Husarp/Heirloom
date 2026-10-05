// Facts edited where they are read: a cell of „W skrócie” (born, died, occupation…) and a row of „Oś życia” open a
// small form in place. Every change is one command (`fact.update`, `fact.add`, `fact.delete`), so Ctrl Z takes it
// back and „Zapisz” in the bar saves it like any other edit.

import { Pencil, Plus, Trash2 } from "lucide-react";
import { useState, type KeyboardEvent } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { runEdit } from "../media/shared";
import { PlaceField, useDateFeedback } from "./PersonalSection";
import { useDraftGuard } from "./sectionEdit";
import type { Fact, FactRef } from "./types";

/** The kinds a new fact can be, as the form offers them. */
const KINDS: { tag: string; label: string }[] = [
  { tag: "RESI", label: "Zamieszkanie" },
  { tag: "OCCU", label: "Praca" },
  { tag: "EDUC", label: "Nauka" },
  { tag: "_MILT", label: "Służba wojskowa" },
  { tag: "EMIG", label: "Emigracja" },
  { tag: "EVEN", label: "Inne wydarzenie" },
];

/** What the description field is called for each kind. */
function textLabel(tag: string, field: FactRef["textField"]): { label: string; placeholder: string } {
  if (tag === "OCCU") return { label: "Zawód", placeholder: "np. kolejarz" };
  if (tag === "EDUC") return { label: "Szkoła", placeholder: "np. gimnazjum w Lublinie" };
  if (tag === "RESI") return { label: "Opis", placeholder: "np. ul. Kolejowa 5 (u teściów)" };
  if (tag === "RELI") return { label: "Wyznanie", placeholder: "np. rzymskokatolickie" };
  if (field === "value") return { label: "Opis", placeholder: "np. 8 pułk piechoty" };
  return { label: "Opis", placeholder: "np. (według relacji babci)" };
}

/** „Dodaj kolejny zawód” — the button under a cell that can take one more fact of its kind. */
export function addLabel(tag: string): string {
  if (tag === "OCCU") return "Dodaj kolejny zawód";
  if (tag === "RESI") return "Dodaj kolejne miejsce zamieszkania";
  if (tag === "EDUC") return "Dodaj kolejną szkołę";
  return "Dodaj kolejny wpis";
}

/** One fact: its date, place and description, in place. `fact` changes one; `add` makes a new one (of `add.tag`, or
 *  of a kind chosen here). */
export function FactForm({
  fact,
  add,
  onDone,
}: {
  fact?: FactRef;
  add?: { record: string; tag?: string; kind?: string | null };
  onDone: () => void;
}) {
  const setAsk = useStore((s) => s.setAsk);
  const [tag, setTag] = useState(fact?.tag ?? add?.tag ?? "RESI");
  const [kind, setKind] = useState(fact?.kind ?? add?.kind ?? "");
  const [date, setDate] = useState(fact?.date ?? "");
  const [place, setPlace] = useState(fact?.place ?? "");
  const [text, setText] = useState(fact?.text ?? "");
  const [busy, setBusy] = useState(false);
  const feedback = useDateFeedback(date);
  useDraftGuard(date !== (fact?.date ?? "") || place !== (fact?.place ?? "") || text !== (fact?.text ?? ""));
  const field = fact?.textField ?? (["OCCU", "RESI", "EDUC", "RELI", "_MILT"].includes(tag) ? "value" : "note");
  const words = textLabel(tag, field);
  const choosing = !fact && !add?.tag;

  const save = () =>
    runEdit(async () => {
      setBusy(true);
      try {
        if (fact) {
          // Only what changed: an untouched date written in an older form stays exactly as it was.
          const changes: Record<string, string> = {};
          if (date !== fact.date) changes.date = date;
          if (place !== fact.place) changes.place = place;
          if (text !== fact.text) changes.text = text;
          if (Object.keys(changes).length) await call("fact.update", { record: fact.record, index: fact.index, tag: fact.tag, ...changes });
        } else {
          await call("fact.add", { record: add?.record, tag, kind: tag === "EVEN" ? kind : add?.kind, date, place, text });
        }
        afterChange();
        onDone();
      } finally {
        setBusy(false);
      }
    });

  const remove = () => {
    if (!fact) return;
    const what = [fact.label, fact.text].filter(Boolean).join(": ");
    setAsk({
      title: "Usunąć ten wpis?",
      text: `„${what}” zniknie z profilu: z osi życia i z „W skrócie”.${fact.shared ? ` To wpis wspólny — zniknie też z profilu: ${fact.shared}.` : ""} Do zapisu możesz to cofnąć (Ctrl Z).`,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń wpis",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              await call("fact.delete", { record: fact.record, index: fact.index, tag: fact.tag });
              afterChange();
              onDone();
            }),
        },
      ],
    });
  };

  // Enter saves and Esc closes the form (not the whole section); the place list takes them first while it is open.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.defaultPrevented || e.ctrlKey || e.metaKey) return;
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT") {
      e.preventDefault();
      save();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onDone();
    }
  };

  return (
    <div className="text-form fact-form" onKeyDown={onKey} onClick={(e) => e.stopPropagation()}>
      {fact ? (
        <span className="fact-form-title">{fact.label}</span>
      ) : (
        choosing && (
          <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
            {KINDS.map((k) => (
              <button key={k.tag} className={`chip${tag === k.tag ? " on" : ""}`} onClick={() => setTag(k.tag)}>
                {k.label}
              </button>
            ))}
          </div>
        )
      )}
      {choosing && tag === "EVEN" && (
        <label className="field">
          <span className="field-label">Co to za wydarzenie</span>
          <input className="input" value={kind} placeholder="np. Pierwsza komunia" autoFocus onChange={(e) => setKind(e.target.value)} />
        </label>
      )}
      <div className="fact-form-grid">
        <label className="field">
          <span className="field-label">Kiedy</span>
          <input className="input" value={date} placeholder="np. 1905, ok. 1920, od 1905 do 1930" autoFocus={!choosing} onChange={(e) => setDate(e.target.value)} />
          <span className={`pf-feedback${feedback?.qualifier === "text" ? " text" : ""}`}>
            {feedback && feedback.qualifier !== "empty" ? (feedback.qualifier === "text" ? "zapiszę dokładnie tak, jak wpisano" : feedback.text) : date ? "" : "puste: bez daty"}
          </span>
        </label>
        <label className="field">
          <span className="field-label">Gdzie</span>
          <PlaceField label="Miejsce" value={place} onChange={setPlace} />
        </label>
      </div>
      <label className="field">
        <span className="field-label">{words.label}</span>
        <input className="input" value={text} placeholder={fact?.derived && !text ? `puste: „${fact.derived}”` : words.placeholder} onChange={(e) => setText(e.target.value)} />
      </label>
      {fact?.derived && <span className="fact-form-note">„{fact.derived}” wynika z rodziców — zmienisz ich w sekcji Rodzina. Opis wpisany tutaj pokaże się zamiast tego.</span>}
      {fact?.shared && <span className="fact-form-note">To wpis wspólny z: {fact.shared} — zmiana pokaże się też na tamtym profilu.</span>}
      <div className="row" style={{ gap: 8 }}>
        {fact && (
          <button className="btn ghost sm" style={{ color: "var(--err)" }} onClick={remove}>
            <Trash2 size={13} />
            Usuń wpis…
          </button>
        )}
        <span className="grow" />
        <button className="btn ghost" onClick={onDone}>
          Anuluj
        </button>
        <button className="btn primary" disabled={busy || (choosing && tag === "EVEN" && !kind.trim())} onClick={save}>
          {fact ? "Zmień" : "Dodaj"}
        </button>
      </div>
    </div>
  );
}

/** A cell of „W skrócie” opened for editing: one fact goes straight to its form; a cell of several (two occupations)
 *  lists them, each with its pencil, and can take one more of its kind. */
export function FactCellEditor({ cell, onClose }: { cell: Fact; onClose: () => void }) {
  const { facts, add } = cell.edit;
  const [open, setOpen] = useState<number | "new" | null>(null);
  if (!add && facts.length === 1) return <FactForm fact={facts[0]} onDone={onClose} />;
  return (
    <div className="col fact-cell-editor" style={{ gap: 8 }}>
      <span className="fact-key">{cell.key}</span>
      {facts.map((f, i) =>
        open === i ? (
          <FactForm key={`${f.record}-${f.index}`} fact={f} onDone={() => setOpen(null)} />
        ) : (
          <button key={`${f.record}-${f.index}`} className="row fact-line" disabled={open != null} onClick={() => setOpen(i)}>
            <span className="grow" style={{ minWidth: 0 }}>
              {f.text || f.place || f.date || "—"}
              {(f.date || (f.text && f.place)) && <span style={{ color: "var(--text3)" }}> · {[f.text ? f.place : "", f.date].filter(Boolean).join(" · ")}</span>}
            </span>
            <Pencil size={13} color="var(--text3)" />
          </button>
        ),
      )}
      {add && (open === "new" ? <FactForm add={add} onDone={() => setOpen(null)} /> : (
        <button className="btn dashed sm" style={{ alignSelf: "flex-start" }} disabled={open != null} onClick={() => setOpen("new")}>
          <Plus size={13} />
          {addLabel(add.tag)}
        </button>
      ))}
      <div className="row" style={{ justifyContent: "flex-end" }}>
        <button className="btn ghost sm" onClick={onClose}>
          Zamknij
        </button>
      </div>
    </div>
  );
}
