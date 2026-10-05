// Facts edited where they are read: a cell of „W skrócie” (born, died, occupation…) and a row of „Oś życia” open a
// small form in place. Every change is one command (`fact.update`, `fact.add`, `fact.delete`), so Ctrl Z takes it
// back and „Zapisz” in the bar saves it like any other edit.

import { Pencil, Plus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
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

/** Events that say something even without a date or a place (born, died, married): emptied in the form they stay as
 *  „tak (bez daty)”; any other fact emptied is deleted only through „Usuń wpis…” (edit.rs, `HAPPENED`). */
const HAPPENED = ["BIRT", "CHR", "BAPM", "DEAT", "BURI", "CREM", "MARR"];

/** A fact as the screen showed it: forms and open cells follow this, not a row's position, so an undo that puts
 *  another fact in its place never leaves a form pointing at the wrong one. */
export function factKey(f: FactRef): string {
  return [f.record, f.index, f.tag, f.date, f.place, f.text].join("\u0001");
}

/** What deleting a fact changes besides the row itself. */
function deleteNote(fact: FactRef): string {
  if (fact.tag === "MARR")
    return ` Bez ślubu ta para nie będzie już pokazywana jako małżeństwo: „żona” i „mąż” zmienią się w „partnerka” i „partner”, także na profilu: ${fact.shared ?? "partnera"}. Jeśli nie znasz tylko daty, wyczyść datę i miejsce — zostanie „tak (bez daty)”.`;
  if (fact.tag === "DEAT")
    return " Bez zgonu (i pogrzebu) osoba urodzona w ostatnich 100 latach będzie traktowana jako żyjąca. Jeśli nie znasz tylko daty, wyczyść datę i miejsce — zostanie „tak (bez daty)”.";
  return fact.shared ? ` To wpis wspólny — zniknie też z profilu: ${fact.shared}.` : "";
}

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
  const notify = useStore((s) => s.notify);
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
  // A description of several lines (a note with line breaks) is edited as such, never joined into one line.
  const multiline = field === "note" || text.includes("\n");
  // The fact this form was opened for. If an undo (Ctrl Z, „Cofnij”) changes it or puts another in its place, the
  // form closes rather than write what was typed onto something else; the server checks the same (`was`).
  const opened = useRef(fact && factKey(fact));
  const typed = [date, place, text].join("\u0001");
  const first = useRef(typed);
  const closing = useRef(false);
  useEffect(() => {
    if (!fact || closing.current || factKey(fact) === opened.current) return;
    closing.current = true;
    if (typed !== first.current) notify("Ten wpis zmienił się w międzyczasie (cofnięta zmiana), więc formularz zamknięto. Otwórz go jeszcze raz.");
    onDone();
  });
  const was = fact && { date: fact.date, place: fact.place, text: fact.text };

  const save = () => {
    // Emptying a fact deletes it, which is asked first; a birth, death or wedding emptied stays „tak (bez daty)”.
    if (fact && !HAPPENED.includes(fact.tag) && !date.trim() && !place.trim() && !text.trim()) return remove();
    return runEdit(async () => {
      setBusy(true);
      try {
        if (fact) {
          // Only what changed: an untouched date written in an older form stays exactly as it was.
          const changes: Record<string, string> = {};
          if (date !== fact.date) changes.date = date;
          if (place !== fact.place) changes.place = place;
          if (text !== fact.text) changes.text = text;
          if (Object.keys(changes).length) await call("fact.update", { record: fact.record, index: fact.index, tag: fact.tag, was, ...changes });
        } else {
          await call("fact.add", { record: add?.record, tag, kind: tag === "EVEN" ? kind : add?.kind, date, place, text });
        }
        closing.current = true;
        afterChange();
        onDone();
      } finally {
        setBusy(false);
      }
    });
  };

  const remove = () => {
    if (!fact) return;
    const what = [fact.label, fact.text].filter(Boolean).join(": ");
    setAsk({
      title: "Usunąć ten wpis?",
      text: `„${what}” zniknie z profilu: z osi życia i z „W skrócie”.${deleteNote(fact)} Do zapisu możesz to cofnąć (Ctrl Z).`,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń wpis",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              await call("fact.delete", { record: fact.record, index: fact.index, tag: fact.tag, was });
              closing.current = true;
              afterChange();
              onDone();
            }),
        },
      ],
    });
  };

  // Enter saves (Ctrl Enter in a description of several lines, where Enter starts a new line) and Esc closes the form
  // (not the whole section); the place list takes them first while it is open.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.defaultPrevented) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (e.key === "Enter" && ((e.target as HTMLElement).tagName === "INPUT" ? !ctrl : ctrl)) {
      e.preventDefault();
      save();
    } else if (e.key === "Escape" && !ctrl) {
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
        {multiline ? (
          <textarea className="input fact-form-text" rows={Math.max(1, text.split("\n").length)} value={text} placeholder={words.placeholder} onChange={(e) => setText(e.target.value)} />
        ) : (
          <input className="input" value={text} placeholder={words.placeholder} onChange={(e) => setText(e.target.value)} />
        )}
      </label>
      {fact?.derived && <span className="fact-form-note">„{fact.derived}” wynika z rodziców — zmienisz ich w sekcji Rodzina. Opis wpisany tutaj pokaże się obok, np. „{fact.derived} (w domu dziadków)”.</span>}
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
  // The fact open (its key, so a deleted or undone one closes rather than open its neighbour), or a new one.
  const [chosen, setOpen] = useState<string | "new" | null>(null);
  const open = chosen === "new" || facts.some((f) => factKey(f) === chosen) ? chosen : null;
  if (chosen !== open) setOpen(open);
  if (!add && facts.length === 1) return <FactForm fact={facts[0]} onDone={onClose} />;
  return (
    <div className="col fact-cell-editor" style={{ gap: 8 }}>
      <span className="fact-key">{cell.key}</span>
      {facts.map((f) =>
        open === factKey(f) ? (
          <FactForm key={factKey(f)} fact={f} onDone={() => setOpen(null)} />
        ) : (
          <button key={factKey(f)} className="row fact-line" disabled={open != null} onClick={() => setOpen(factKey(f))}>
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
