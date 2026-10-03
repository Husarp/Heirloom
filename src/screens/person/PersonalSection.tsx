// „Dane osobowe” (design 17a): the profile's first section, edited in place with the same layout as reading; the
// same editor makes a new person. The draft stays here until „Gotowe”; only then does it become an unsaved change.

import { ChevronDown, ImagePlus, Image as ImageIcon, MapPin, Plus, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { useSaveHandler } from "../../app/useShortcuts";
import { Checkbox, Segmented, Thumb, useDismiss } from "../../components/bits";
import { count } from "../../lib/format";
import { pickFiles } from "../../lib/native";
import { runEdit } from "../media/shared";
import { SectionFoot } from "./sectionEdit";
import type { Profile } from "./types";

export type RelationKind = "parent" | "partner" | "child" | "sibling";
type EventKey = "birth" | "baptism" | "death" | "burial";
type Certainty = "" | "high" | "medium" | "low";

interface EventForm {
  date: string;
  place: string;
  certainty: Certainty;
}

interface Form {
  given: string;
  surname: string;
  birthSurname: string;
  nickname: string;
  sex: "M" | "F" | "U";
  status: "living" | "deceased" | "unknown";
  otherNames: { given: string; surname: string; note: string }[];
  events: Record<EventKey, EventForm>;
  occupation: string;
  religion: string;
  noData: string[];
  tags: string[];
}

interface EditData {
  given: string;
  surname: string;
  birthSurname: string;
  nickname: string;
  sex: "M" | "F" | "U";
  status: "living" | "deceased" | "unknown";
  otherNames: { given: string; surname: string; note: string }[];
  events: Record<EventKey, { date: string; place: string; certainty: Certainty | null } | null>;
  occupation: string;
  religion: string;
  noData: string[];
}

const EVENTS: { key: EventKey; label: string }[] = [
  { key: "birth", label: "Urodzenie" },
  { key: "baptism", label: "Chrzest" },
  { key: "death", label: "Zgon" },
  { key: "burial", label: "Pochówek" },
];

const NO_EVENT: EventForm = { date: "", place: "", certainty: "" };

const EMPTY: Form = {
  given: "",
  surname: "",
  birthSurname: "",
  nickname: "",
  sex: "U",
  status: "unknown",
  otherNames: [],
  events: { birth: NO_EVENT, baptism: NO_EVENT, death: NO_EVENT, burial: NO_EVENT },
  occupation: "",
  religion: "",
  noData: [],
  tags: [],
};

const RELATION_WORD: Record<RelationKind, string> = { parent: "rodzic", partner: "partner", child: "dziecko", sibling: "rodzeństwo" };

function formFrom(d: EditData, tags: string[]): Form {
  const event = (k: EventKey): EventForm => {
    const e = d.events[k];
    return e ? { date: e.date, place: e.place, certainty: e.certainty ?? "" } : NO_EVENT;
  };
  return {
    given: d.given,
    surname: d.surname,
    birthSurname: d.birthSurname,
    nickname: d.nickname,
    sex: d.sex,
    status: d.status,
    otherNames: d.otherNames,
    events: { birth: event("birth"), baptism: event("baptism"), death: event("death"), burial: event("burial") },
    occupation: d.occupation,
    religion: d.religion,
    noData: d.noData,
    tags,
  };
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** What `person.update` gets: only what changed. The API rewrites each field it is sent (the name from given and
 *  surname, an event left without a date and place goes), so what isn't shown here stays untouched. */
function changesOf(form: Form, initial: Form): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  const trim = (s: string) => s.trim();
  if (!same([form.given, form.surname, form.birthSurname, form.nickname, form.otherNames], [initial.given, initial.surname, initial.birthSurname, initial.nickname, initial.otherNames])) {
    // The API reads the nickname and the other names only together with the name.
    Object.assign(out, { given: trim(form.given), surname: trim(form.surname), birthSurname: trim(form.birthSurname), nickname: trim(form.nickname) });
    if (!same(form.otherNames, initial.otherNames)) out.otherNames = form.otherNames;
  }
  for (const key of ["sex", "status", "occupation", "religion", "noData", "tags"] as const) {
    if (!same(form[key], initial[key])) out[key] = form[key];
  }
  const events: Record<string, Partial<EventForm>> = {};
  for (const { key } of EVENTS) {
    const now = form.events[key];
    const was = initial.events[key];
    const change: Partial<EventForm> = {};
    if (now.date !== was.date) change.date = now.date;
    if (now.place !== was.place) change.place = now.place;
    if (now.certainty !== was.certainty) change.certainty = now.certainty;
    if (Object.keys(change).length) events[key] = change;
  }
  if (Object.keys(events).length) out.events = events;
  return out;
}

function payloadOf(form: Form) {
  return {
    given: form.given.trim(),
    surname: form.surname.trim(),
    birthSurname: form.birthSurname.trim(),
    nickname: form.nickname.trim(),
    sex: form.sex,
    status: form.status,
    otherNames: form.otherNames,
    occupation: form.occupation,
    religion: form.religion,
    noData: form.noData,
    tags: form.tags,
    events: form.events,
  };
}

/** The personal data in edit mode. `id` null: a new person (optionally a relative of `relation.of`). */
export function PersonalEditor({
  id,
  profile,
  relation,
  relatedName,
  defaults,
  onDone,
  onCancel,
}: {
  id: string | null;
  profile: Profile | null;
  relation?: { kind: RelationKind; of: string };
  relatedName?: string;
  /** A new person's starting values (a child or sibling takes the family's surname). */
  defaults?: { surname?: string };
  onDone: (createdId?: string) => void;
  onCancel: () => void;
}) {
  const { data: editData } = useApi<EditData>(id ? "person.editData" : null, { id });
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const setLeaveGuard = useStore((s) => s.setLeaveGuard);
  const requireEdit = useStore((s) => s.requireEdit);
  const go = useStore((s) => s.go);
  const undo = useStore((s) => s.undo);
  // A new person starts from a whole form (an absent default is an empty field, never undefined), and that start is
  // what „changed” compares with, so an untouched form leaves without asking.
  const start: Form = { ...EMPTY, surname: defaults?.surname ?? "" };
  const [form, setForm] = useState<Form | null>(id ? null : start);
  const [initial, setInitial] = useState<Form>(start);
  const [forms, setForms] = useState(false);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);

  useEffect(() => {
    if (id && editData && !form) {
      const f = formFrom(editData, profile?.tags ?? []);
      setForm(f);
      setInitial(f);
      setForms(!!f.birthSurname || f.otherNames.length > 0);
    }
  }, [id, editData, form, profile]);

  const changed = useMemo(() => (form ? !same(form, initial) : false), [form, initial]);
  // Leaving the profile (sidebar, Back, Ctrl K) asks first while the draft has changes.
  useEffect(() => setLeaveGuard(changed ? "Odrzucić zmiany w danych osobowych?" : null), [changed, setLeaveGuard]);
  useEffect(() => () => setLeaveGuard(null), [setLeaveGuard]);

  const set = <K extends keyof Form>(key: K, value: Form[K]) => setForm((f) => (f ? { ...f, [key]: value } : f));
  const setEvent = (key: EventKey, part: Partial<EventForm>) => setForm((f) => (f ? { ...f, events: { ...f.events, [key]: { ...f.events[key], ...part } } } : f));

  const done = async (thenSave = false) => {
    if (!form || busyRef.current) return;
    // Reached in browse mode (Back, with „Kto edytuje?” cancelled): editing first.
    if (useStore.getState().mode !== "edit") {
      requireEdit(() => void done(thenSave));
      return;
    }
    if (!form.given.trim() && !form.surname.trim()) {
      notify("Podaj imię albo nazwisko.", { kind: "err" });
      return;
    }
    busyRef.current = true;
    setBusy(true);
    try {
      let created: string | undefined;
      if (id) {
        const changes = changesOf(form, initial);
        if (Object.keys(changes).length) await call("person.update", { id, ...changes });
      } else {
        created = (await call<{ id: string }>("person.create", { ...payloadOf(form), relation })).id;
      }
      setLeaveGuard(null);
      afterChange();
      onDone(created);
      if (thenSave) await useStore.getState().save();
    } catch (e) {
      notify((e as Error).message, { kind: "err" });
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const cancel = () => {
    if (!changed) return onCancel();
    setAsk({
      title: "Odrzucić zmiany w danych osobowych?",
      text: "To, co wpisano w tej sekcji, nie zostanie zapamiętane.",
      icon: "warn",
      buttons: [
        { label: "Wróć do edycji", kind: "ghost" },
        {
          label: "Odrzuć zmiany",
          kind: "danger",
          run: () => {
            setLeaveGuard(null);
            onCancel();
          },
        },
      ],
    });
  };

  // Ctrl S: the section is finished and the file saved in one go.
  useSaveHandler(() => void done(true));

  const changePhoto = () =>
    runEdit(async () => {
      if (!id) return;
      const paths = await pickFiles("Zdjęcie profilowe", [{ name: "Zdjęcia", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp"] }], false);
      if (!paths.length) return;
      await call("media.add", { paths, person: id, kind: "photo", profile: true });
      afterChange();
    });

  // Deleting takes the person out of their families (and texts only about them); photos and sources stay. It is an
  // unsaved change like any other, so „Cofnij” in the bar brings the person back.
  const remove = () =>
    setAsk({
      title: `Usunąć osobę ${profile?.person.name ?? ""}?`,
      text: "Osoba zniknie z drzewa i list, razem z powiązaniami w rodzinach i tekstami, które opisują tylko ją. Zdjęcia i źródła zostaną w archiwum. Zmiana czeka na zapis — do tego czasu można ją cofnąć.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń osobę",
          kind: "danger",
          run: () =>
            requireEdit(async () => {
              try {
                await call("person.delete", { id });
                setLeaveGuard(null);
                useStore.getState().finishSection();
                afterChange();
                go({ name: "people" });
                notify(`Usunięto: ${profile?.person.name ?? ""}.`, { action: { label: "Cofnij", run: () => void undo() } });
              } catch (e) {
                notify((e as Error).message, { kind: "err" });
              }
            }),
        },
      ],
    });

  if (!form) return <section className="hero editing" style={{ minHeight: 320 }} aria-busy="true" />;
  const living = form.status === "living";
  const photo = profile?.portrait?.path;

  return (
    <section className="hero editing" aria-label="Dane osobowe, edycja">
      <div className="portrait">
        {photo ? (
          <Thumb path={photo} size={512} style={{ width: 200, height: 256, borderRadius: "var(--r-card)" }} />
        ) : (
          <div className="portrait-empty">
            <ImageIcon size={24} />
            <span>{id ? "Brak zdjęcia" : "Zdjęcie dodasz po utworzeniu osoby"}</span>
          </div>
        )}
        {id && (
          <button className="btn secondary" style={{ height: 34 }} onClick={changePhoto}>
            <ImagePlus size={14} />
            {photo ? "Zmień zdjęcie" : "Dodaj zdjęcie"}
          </button>
        )}
      </div>
      <div className="personal-form">
        <div className="row" style={{ gap: 10 }}>
          <span className="section-kicker">{id ? "Dane osobowe" : relation && relatedName ? `Nowa osoba · ${RELATION_WORD[relation.kind]}: ${relatedName}` : "Nowa osoba"}</span>
          <span className="editing-tag">{id ? "edytujesz tę sekcję" : "nowa osoba"}</span>
        </div>
        <div className="pf-grid2">
          <Field label="Imiona">
            <input className="input name-input" autoFocus value={form.given} onChange={(e) => set("given", e.target.value)} />
          </Field>
          <Field label="Nazwisko" note={form.sex === "F" ? "po ślubie, jeśli je zmieniła" : undefined}>
            <input className="input name-input" value={form.surname} onChange={(e) => set("surname", e.target.value)} />
            {!forms && (
              <button className="link small-link" onClick={() => setForms(true)}>
                + inne formy nazwiska
              </button>
            )}
          </Field>
        </div>
        {forms && (
          <div className="pf-forms">
            <div className="pf-grid2">
              <Field label="Nazwisko rodowe" note={form.sex === "M" ? "zwykle to samo co nazwisko" : "nazwisko panieńskie"}>
                {form.noData.includes("birthSurname") ? (
                  <div className="input no-data">brak danych w źródłach</div>
                ) : (
                  <input className="input" value={form.birthSurname} onChange={(e) => set("birthSurname", e.target.value)} />
                )}
              </Field>
              <NoDataBox field="birthSurname" form={form} set={set} />
            </div>
            {form.otherNames.map((n, i) => (
              <div key={i} className="row" style={{ gap: 8 }}>
                <input className="input sm" placeholder="Imiona" value={n.given} onChange={(e) => set("otherNames", form.otherNames.map((x, j) => (j === i ? { ...x, given: e.target.value } : x)))} />
                <input className="input sm" placeholder="Nazwisko" value={n.surname} onChange={(e) => set("otherNames", form.otherNames.map((x, j) => (j === i ? { ...x, surname: e.target.value } : x)))} />
                <input className="input sm" placeholder="Opis, np. zapis w akcie" value={n.note} onChange={(e) => set("otherNames", form.otherNames.map((x, j) => (j === i ? { ...x, note: e.target.value } : x)))} />
                <button className="icon-btn" title="Usuń tę formę" aria-label="Usuń tę formę" onClick={() => set("otherNames", form.otherNames.filter((_, j) => j !== i))}>
                  <X size={15} />
                </button>
              </div>
            ))}
            <button className="btn dashed sm" style={{ alignSelf: "flex-start" }} onClick={() => set("otherNames", [...form.otherNames, { given: form.given, surname: "", note: "" }])}>
              <Plus size={13} />
              Dodaj formę (np. zapis w akcie, pseudonim)
            </button>
          </div>
        )}
        <div className="pf-grid2">
          <Field label="Przydomek">
            <input className="input nick-input" value={form.nickname} placeholder="np. Dziadek Józek" onChange={(e) => set("nickname", e.target.value)} />
          </Field>
          <Field label="Płeć">
            <Segmented
              full
              size={34}
              value={form.sex === "M" || form.sex === "F" ? form.sex : "U"}
              onChange={(v) => set("sex", v)}
              options={[
                { value: "M", label: "Mężczyzna" },
                { value: "F", label: "Kobieta" },
                { value: "U", label: "Nieznana" },
              ]}
            />
          </Field>
        </div>
        <div className="pf-grid2">
          <Field label="Zawód">
            <input className="input" value={form.occupation} placeholder="np. kolejarz" onChange={(e) => set("occupation", e.target.value)} />
          </Field>
          <Field label="Czy żyje">
            <Segmented
              full
              size={34}
              variant="neutral"
              value={form.status}
              onChange={(v) => set("status", v)}
              options={[
                { value: "living", label: "Żyje" },
                { value: "deceased", label: "Nie żyje" },
                { value: "unknown", label: "Nie wiadomo" },
              ]}
            />
          </Field>
        </div>
        <div className="pf-events">
          <span />
          <span className="pf-col-label">Data</span>
          <span className="pf-col-label">Miejsce</span>
          <span className="pf-col-label">Pewność</span>
          {EVENTS.filter((e) => !living || e.key === "birth" || e.key === "baptism").map((e) => (
            <EventRow key={e.key} label={e.label} value={form.events[e.key]} onChange={(part) => setEvent(e.key, part)} />
          ))}
          <span className="pf-label">Wyznanie</span>
          {form.noData.includes("religion") ? (
            <span className="input no-data">—</span>
          ) : (
            <input className="input" value={form.religion} placeholder="np. rzymskokatolickie" onChange={(e) => set("religion", e.target.value)} />
          )}
          <NoDataBox field="religion" form={form} set={set} />
          <span />
        </div>
        <Tags tags={form.tags} onChange={(tags) => set("tags", tags)} />
        <SectionFoot
          onCancel={cancel}
          onDone={() => void done()}
          busy={busy}
          doneLabel={id ? "Gotowe" : "Utwórz osobę"}
          left={
            id ? (
              <button className="btn ghost" style={{ color: "var(--err)" }} onClick={remove}>
                <Trash2 size={14} />
                Usuń osobę…
              </button>
            ) : undefined
          }
        />
      </div>
    </section>
  );
}

function Field({ label, note, children }: { label: string; note?: string; children: ReactNode }) {
  return (
    <label className="field">
      <span className="field-label">
        {label}
        {note && <span style={{ color: "var(--text3)", fontSize: 12 }}>· {note}</span>}
      </span>
      {children}
    </label>
  );
}

/** „Brak danych w źródłach”: the information isn't there and won't be (spec §4.8); the field counts as filled. */
function NoDataBox({ field, form, set }: { field: string; form: Form; set: <K extends keyof Form>(key: K, value: Form[K]) => void }) {
  const on = form.noData.includes(field);
  return (
    <span className="row pf-nodata" onClick={() => set("noData", on ? form.noData.filter((f) => f !== field) : [...form.noData, field])}>
      <Checkbox on={on} onChange={(v) => set("noData", v ? [...form.noData, field] : form.noData.filter((f) => f !== field))} />
      Brak danych w źródłach
    </span>
  );
}

interface DateFeedback {
  qualifier: "empty" | "exact" | "month" | "year" | "about" | "before" | "after" | "range" | "text";
  text?: string;
}

const CERTAINTY_LABEL: Record<Certainty, string> = { "": "—", high: "pewne", medium: "prawdopodobne", low: "niepewne" };

/** One event: the smart date (how it was read shows under it, spec §5.11), the place and how certain it is. */
function EventRow({ label, value, onChange }: { label: string; value: EventForm; onChange: (part: Partial<EventForm>) => void }) {
  const [feedback, setFeedback] = useState<DateFeedback | null>(null);
  useEffect(() => {
    const handle = window.setTimeout(() => {
      call<DateFeedback>("date.parse", { text: value.date })
        .then(setFeedback)
        .catch(() => setFeedback(null));
    }, 180);
    return () => window.clearTimeout(handle);
  }, [value.date]);
  const empty = !value.date.trim() && !value.place.trim();
  return (
    <>
      <span className="pf-label">{label}</span>
      <input className="input" aria-label={`${label}: data`} value={value.date} placeholder="np. 12.03.1878" onChange={(e) => onChange({ date: e.target.value })} />
      <PlaceField label={`${label}: miejsce`} value={value.place} onChange={(place) => onChange({ place })} />
      <span className="select-wrap">
        <select className="input" aria-label={`${label}: pewność`} value={value.certainty} disabled={empty} onChange={(e) => onChange({ certainty: e.target.value as Certainty })}>
          {(Object.keys(CERTAINTY_LABEL) as Certainty[]).map((c) => (
            <option key={c} value={c}>
              {CERTAINTY_LABEL[c]}
            </option>
          ))}
        </select>
        <ChevronDown size={14} className="select-chevron" />
      </span>
      <span />
      <span className={`pf-feedback${feedback?.qualifier === "text" ? " text" : ""}`}>
        {feedback && feedback.qualifier !== "empty" ? (feedback.qualifier === "text" ? "zapiszę dokładnie tak, jak wpisano" : feedback.text) : ""}
      </span>
      <span />
      <span />
    </>
  );
}

interface PlaceRow {
  path: string[];
  name: string;
  level: number;
  count: number;
}

let placeCache: { version: number; rows: PlaceRow[] } | null = null;

/** Place picker with the places already in the archive (spec §4.8), or a new one as typed. */
function PlaceField({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  const dataVersion = useStore((s) => s.dataVersion);
  const [open, setOpen] = useState(false);
  const [rows, setRows] = useState<PlaceRow[]>([]);
  const [active, setActive] = useState(0);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  useEffect(() => {
    if (placeCache?.version === dataVersion) {
      setRows(placeCache.rows);
      return;
    }
    call<{ places: PlaceRow[] }>("places.list")
      .then((d) => {
        placeCache = { version: dataVersion, rows: d.places };
        setRows(d.places);
      })
      .catch(() => {});
  }, [dataVersion]);
  const fold = (t: string) => t.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/ł/g, "l");
  const q = fold(value.trim());
  const matches = q ? rows.filter((r) => fold(r.name).startsWith(q) || fold([...r.path].reverse().join(", ")).includes(q)).sort((a, b) => b.count - a.count).slice(0, 6) : [];
  const pick = (r: PlaceRow) => {
    onChange([...r.path].reverse().join(", "));
    setOpen(false);
  };
  return (
    <div ref={ref} style={{ position: "relative", minWidth: 0 }}>
      <input
        className="input"
        aria-label={label}
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
          setActive(0);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(e) => {
          // Ctrl Enter is „Gotowe” for the whole section, not a pick from the list.
          if (!open || !matches.length || e.ctrlKey || e.metaKey) return;
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setActive((a) => Math.min(a + 1, matches.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((a) => Math.max(a - 1, 0));
          } else if (e.key === "Enter") {
            e.preventDefault();
            pick(matches[active]);
          } else if (e.key === "Escape") {
            // Closes the list only, not the section.
            e.preventDefault();
            setOpen(false);
          }
        }}
        placeholder="np. Wólka, par. Łęczna"
      />
      {open && matches.length > 0 && (
        <div className="popover" style={{ top: 44, left: 0, width: "max(100%, 320px)", fontSize: 14 }}>
          {matches.map((r, i) => (
            <button key={r.path.join("|")} className={`place-option${i === active ? " active" : ""}`} onMouseDown={(e) => e.preventDefault()} onClick={() => pick(r)}>
              <MapPin size={15} color="var(--text3)" style={{ flex: "none" }} />
              <span className="col grow" style={{ minWidth: 0, textAlign: "left" }}>
                <span style={{ fontWeight: 500 }}>{r.name}</span>
                <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                  {[...r.path].reverse().join(" › ")}
                </span>
              </span>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>{count(r.count, "osoba", "osoby", "osób")}</span>
            </button>
          ))}
          <div className="place-option" style={{ height: 40, borderTop: "1px solid var(--border)", color: "var(--accent-text)", fontWeight: 500 }}>
            <Plus size={14} />
            Nowe miejsce „{value.trim()}” zapisze się tak, jak wpisano
          </div>
        </div>
      )}
    </div>
  );
}

function Tags({ tags, onChange }: { tags: string[]; onChange: (tags: string[]) => void }) {
  const [input, setInput] = useState<string | null>(null);
  return (
    <div className="row pf-tags">
      <span className="pf-label" style={{ width: 96, flex: "none" }}>
        Tagi
      </span>
      {tags.map((t) => (
        <span key={t} className="tag-chip">
          {t}
          <button title="Usuń tag" aria-label={`Usuń tag ${t}`} onClick={() => onChange(tags.filter((x) => x !== t))}>
            <X size={13} />
          </button>
        </span>
      ))}
      {input == null ? (
        <button className="chip dashed" style={{ height: 32 }} onClick={() => setInput("")}>
          <Plus size={13} /> Dodaj
        </button>
      ) : (
        <input
          className="input sm"
          autoFocus
          style={{ width: 170, height: 32 }}
          value={input}
          placeholder="np. kolejarz"
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              if (input.trim()) onChange([...new Set([...tags, input.trim()])]);
              setInput(null);
            }
            if (e.key === "Escape") {
              e.preventDefault();
              setInput(null);
            }
          }}
          onBlur={() => {
            if (input.trim()) onChange([...new Set([...tags, input.trim()])]);
            setInput(null);
          }}
        />
      )}
    </div>
  );
}

