import {
  ArrowRight,
  BookOpen,
  Briefcase,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Droplets,
  FileText,
  Gem,
  Globe,
  GraduationCap,
  HeartHandshake,
  House,
  Image as ImageIcon,
  Import,
  Landmark,
  Lightbulb,
  Link as LinkIcon,
  Lock,
  Pencil,
  PencilLine,
  Plus,
  Quote,
  Search,
  Star,
  Trash2,
  TriangleAlert,
  Undo2,
  User,
  UserPlus,
  X,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { call } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, rowButton, Segmented, Thumb, useDismiss } from "../../components/bits";
import { Markdown, plainText } from "../../components/Markdown";
import { RichEditor } from "../../components/RichEditor";
import { cardName, cardYears, clock, count, relativeTime } from "../../lib/format";
import { openUrl, pickFiles } from "../../lib/native";
import { Lightbox } from "../media/Lightbox";
import { runEdit } from "../media/shared";
import { RelativeTools } from "./RelativeTools";
import { SectionEditButton, SectionFoot, useSection, type SectionEdit } from "./sectionEdit";
import type { Profile, TextItem } from "./types";

type Props = { data: Profile };

function SectionHead({ title, count: n, right, edit }: { title: string; count?: number; right?: ReactNode; edit?: SectionEdit }) {
  return (
    <div className="row" style={{ gap: 10, alignItems: "baseline" }}>
      <h2 className="section-title">{title}</h2>
      {n != null && n > 0 && <span style={{ fontSize: 17, color: "var(--text3)" }}>{n}</span>}
      <span className="grow" />
      {right}
      {edit && <SectionEditButton edit={edit} />}
    </div>
  );
}

/** The class of a section, framed while it is being edited. */
const sectionClass = (edit: SectionEdit) => `profile-section${edit.open ? " editing" : ""}`;

/** The foot of an open section. */
function Foot({ edit }: { edit: SectionEdit }) {
  return edit.open ? <SectionFoot onCancel={edit.cancel} onDone={edit.done} /> : null;
}

function Sup({ sources }: { sources: number[] }) {
  if (!sources.length) return null;
  return <sup className="source-mark">[{sources.join(", ")}]</sup>;
}

function useEditing() {
  return useStore((s) => s.mode) === "edit";
}

const DRAFT_QUESTION = "Odrzucić wpisany tekst?";

/** A text being typed and not yet added: closing its section, leaving the profile or switching the archive asks
 *  first (the leave guard). */
function useDraftGuard(dirty: boolean) {
  useEffect(() => {
    const { leaveGuard, setLeaveGuard } = useStore.getState();
    if (dirty) setLeaveGuard(DRAFT_QUESTION);
    else if (leaveGuard === DRAFT_QUESTION) setLeaveGuard(null);
  }, [dirty]);
  useEffect(
    () => () => {
      if (useStore.getState().leaveGuard === DRAFT_QUESTION) useStore.getState().setLeaveGuard(null);
    },
    [],
  );
}

/** Adds or edits one text (a biography section, story, saying, trivia, note or the summary). */
function TextForm({
  person,
  kind,
  item,
  onDone,
  withTitle,
  withDate,
  placeholder,
}: {
  person: string;
  kind: TextItem["kind"];
  item?: TextItem;
  onDone: () => void;
  withTitle?: boolean;
  withDate?: boolean;
  placeholder?: string;
}) {
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [title, setTitle] = useState(item?.title ?? "");
  const [body, setBody] = useState(item?.body ?? "");
  const [date, setDate] = useState(item?.date ?? "");
  const [place, setPlace] = useState(item?.place ?? "");
  const [busy, setBusy] = useState(false);
  useDraftGuard(body !== (item?.body ?? "") || title !== (item?.title ?? "") || date !== (item?.date ?? "") || place !== (item?.place ?? ""));
  const save = () =>
    requireEdit(async () => {
      if (!body.trim()) {
        notify("Tekst jest pusty.", { kind: "err" });
        return;
      }
      setBusy(true);
      try {
        await call("text.save", {
          id: item?.id ?? undefined,
          person,
          kind,
          body,
          ...(withTitle ? { title } : {}),
          ...(withDate ? { date, place } : {}),
        });
        afterChange();
        onDone();
      } catch (e) {
        notify((e as Error).message, { kind: "err" });
      } finally {
        setBusy(false);
      }
    });
  return (
    <div className="text-form">
      {withTitle && <input className="input" placeholder={kind === "bio" ? "Tytuł sekcji, np. Dzieciństwo w Wólce" : "Tytuł historii"} value={title} onChange={(e) => setTitle(e.target.value)} />}
      {withDate && (
        <div className="row" style={{ gap: 8 }}>
          <input className="input sm" placeholder="Kiedy (np. zima 1915, ok. 1920)" value={date} onChange={(e) => setDate(e.target.value)} />
          <input className="input sm" placeholder="Gdzie (np. Lublin)" value={place} onChange={(e) => setPlace(e.target.value)} />
        </div>
      )}
      <RichEditor value={body} onChange={setBody} placeholder={placeholder} autofocus minHeight={kind === "bio" || kind === "story" ? 180 : 90} />
      <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
        <button className="btn ghost" onClick={onDone}>
          Anuluj
        </button>
        <button className="btn primary" disabled={busy} onClick={save}>
          {item ? "Zmień tekst" : "Dodaj tekst"}
        </button>
      </div>
    </div>
  );
}

function ItemTools({ person, item, onEdit, movable }: { person: string; item: TextItem; onEdit: () => void; movable?: boolean }) {
  const setAsk = useStore((s) => s.setAsk);
  if (!item.id) return null;
  const move = (delta: number) =>
    runEdit(async () => {
      await call("text.move", { person, id: item.id, delta });
      afterChange();
    });
  const remove = () =>
    setAsk({
      title: "Usunąć ten tekst?",
      text: "Zniknie z profilu. Do zapisu możesz to cofnąć (Ctrl Z), a po zapisie — w Historii zmian.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              await call("text.delete", { id: item.id, person });
              afterChange();
            }),
        },
      ],
    });
  return (
    <span className="item-tools">
      {movable && (
        <>
          <button className="icon-btn" title="Wyżej" onClick={() => move(-1)}>
            <ChevronUp size={15} />
          </button>
          <button className="icon-btn" title="Niżej" onClick={() => move(1)}>
            <ChevronDown size={15} />
          </button>
        </>
      )}
      <button className="icon-btn" title="Edytuj" onClick={onEdit}>
        <Pencil size={14} />
      </button>
      <button className="icon-btn" title="Usuń" onClick={remove}>
        <Trash2 size={14} />
      </button>
    </span>
  );
}

function AddButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button className="btn dashed add-inline" onClick={onClick}>
      <Plus size={14} />
      {label}
    </button>
  );
}

// ---------- W skrócie ----------

export function SummarySection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:summary`, "W skrócie");
  const [edit, setEdit] = useState(false);
  useEffect(() => {
    if (!sec.open) setEdit(false);
  }, [sec.open]);
  const uncertain = data.facts.some((f) => f.uncertain);
  if (!editing && !data.summary && data.facts.length === 0) return null;
  return (
    <section id="sec-summary" className={sectionClass(sec)} style={{ gap: 18 }}>
      <SectionHead title="W skrócie" edit={sec} right={sec.open && data.summary && !edit && <ItemTools person={data.person.id} item={data.summary} onEdit={() => setEdit(true)} />} />
      {edit ? (
        <TextForm person={data.person.id} kind="summary" item={data.summary ?? undefined} onDone={() => setEdit(false)} placeholder="2–4 zdania: kim był, gdzie żył, co najważniejsze." />
      ) : data.summary ? (
        <Markdown text={data.summary.body} from={data.person.id} className="lead" />
      ) : (
        editing && <AddButton label="Dodaj krótkie podsumowanie" onClick={() => sec.start(() => setEdit(true))} />
      )}
      {data.facts.length > 0 && (
        <div className="facts-grid">
          {data.facts.map((f, i) => (
            <div key={`${f.key}-${i}`} className="fact-cell">
              <span className="fact-key">{f.key}</span>
              <span className="fact-value">
                <span className={f.uncertain ? "uncertain-strong" : undefined}>{f.value}</span>
                <Sup sources={f.sources} />
              </span>
            </div>
          ))}
        </div>
      )}
      {uncertain && (
        <div className="row" style={{ gap: 8, fontSize: 12, color: "var(--text3)" }}>
          <span className="uncertain-strong">ok. 1878</span>
          <span>= data przybliżona, niepotwierdzona lub sprzeczna</span>
        </div>
      )}
      {sec.open && data.facts.length > 0 && <span style={{ fontSize: 12, color: "var(--text3)" }}>Daty i miejsca zmienisz w danych osobowych na górze profilu.</span>}
      <Foot edit={sec} />
    </section>
  );
}

// ---------- Rodzina ----------

type RelationKind = "parent" | "partner" | "child" | "sibling";

/** A direct relation as `person.relations` returns it: what the family section needs to change it. */
interface Relation extends PersonSummary {
  role: RelationKind | "associate";
  family: string | null;
  label: string;
  detail: string;
  years: string;
  pedi?: string;
  married?: boolean;
  assoRole?: string;
}

const RELATION_WORD: Record<RelationKind, string> = { parent: "rodzica", partner: "partnera", child: "dziecko", sibling: "rodzeństwo" };

export function FamilySection({ data }: Props) {
  const go = useStore((s) => s.go);
  const setAsk = useStore((s) => s.setAsk);
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:family`, "Rodzina");
  // In edit mode every relative has its tools (the same as under the tree's cards); using one opens the section.
  const { data: relations } = useApi<Relation[]>(editing ? "person.relations" : null, { id: data.person.id });
  const [adding, setAdding] = useState(false);
  useEffect(() => {
    if (!sec.open) setAdding(false);
  }, [sec.open]);
  if (!editing && data.family.length === 0) return null;

  const run = (fn: () => Promise<unknown>) =>
    runEdit(async () => {
      await fn();
      afterChange();
    });
  const remove = (r: Relation) =>
    setAsk({
      title: `Usunąć relację z: ${r.name}?`,
      text: "Obie osoby zostają w archiwum; znika tylko połączenie między nimi. Do zapisu możesz to cofnąć.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń relację",
          kind: "danger",
          run: () => run(() => call("relation.dissociate", { person: data.person.id, other: r.id })),
        },
      ],
    });
  const associates = relations?.filter((r) => r.role === "associate") ?? [];

  return (
    <section id="sec-family" className={sectionClass(sec)} style={{ gap: 16 }}>
      <SectionHead title="Rodzina" edit={sec} />
      {data.family.map((g, i) => (
        <div key={`${g.title}-${i}`} className="col" style={{ gap: 8 }}>
          <span className="label-caps" style={{ fontSize: 12 }}>
            {g.title}
          </span>
          <div className={`family-grid${editing ? " editing" : ""}`}>
            {g.people.map((r) => {
              const rel = editing ? relations?.find((x) => x.id === r.id && x.role !== "associate") : undefined;
              return (
                <div key={r.id} className="family-tile" style={{ boxShadow: `inset 0 3px 0 -1px var(--b${r.branch})` }}>
                  <button className="row family-tile-main" onClick={() => go({ name: "person", id: r.id })}>
                    <Avatar initials={r.initials} branch={r.branch} photo={r.photo} size={36} from={r.from} />
                    <span className="col" style={{ minWidth: 0, textAlign: "left" }}>
                      <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
                        {cardName(r)}
                      </span>
                      <span className="ellipsis" style={{ fontSize: 12, color: "var(--text2)" }}>
                        {r.line}
                      </span>
                    </span>
                  </button>
                  {rel && rel.role !== "associate" && (
                    <RelativeTools
                      variant="row"
                      of={data.person}
                      relative={rel}
                      kind={rel.role}
                      family={rel.family}
                      pedi={rel.pedi}
                      // „ojciec (adopcja)” already says the kind; otherwise it is added („córka · przybrana”).
                      chip={rel.pedi && rel.pedi !== "birth" && !rel.label.includes("(") ? `${rel.label} · ${rel.detail}` : rel.label}
                      guard={(then) => sec.start(then)}
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}
      {sec.open && associates.length > 0 && (
        <div className="col" style={{ gap: 8 }}>
          <span className="label-caps" style={{ fontSize: 12 }}>
            Spoza rodziny
          </span>
          <div className="family-grid editing">
            {associates.map((a) => (
              <div key={`${a.id}-${a.label}`} className="family-tile">
                <button className="row family-tile-main" onClick={() => go({ name: "person", id: a.id })}>
                  <Avatar initials={a.initials} branch={a.branch} photo={a.photo} size={36} />
                  <span className="col" style={{ minWidth: 0, textAlign: "left" }}>
                    <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
                      {cardName(a)}
                    </span>
                    <span className="ellipsis" style={{ fontSize: 12, color: "var(--text2)" }}>
                      {a.years}
                    </span>
                  </span>
                </button>
                <span className="row family-tile-tools">
                  <RelationChip relation={a} personId={data.person.id} run={run} />
                  <span className="grow" />
                  <button className="icon-btn" title="Usuń relację" aria-label={`Usuń relację z: ${a.name}`} onClick={() => remove(a)}>
                    <Trash2 size={14} />
                  </button>
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
      {editing && !adding && (
        <button className="family-add" onClick={() => sec.start(() => setAdding(true))}>
          <Plus size={14} />
          Dodaj krewnego
        </button>
      )}
      {sec.open && adding && <AddRelation id={data.person.id} onDone={() => setAdding(false)} />}
      <Foot edit={sec} />
    </section>
  );
}

const ASSO_TYPES: { role: string; label: string; icon: ReactNode; phrase?: string }[] = [
  { role: "FRIEND", label: "przyjaciel", icon: <HeartHandshake size={14} /> },
  { role: "OTHER", label: "kolega z pracy", icon: <Briefcase size={14} />, phrase: "kolega z pracy" },
  { role: "NGHBR", label: "sąsiad", icon: <House size={14} /> },
  { role: "GODP", label: "chrzestny / chrzestna", icon: <Droplets size={14} /> },
  { role: "OTHER", label: "opiekun / wychowawca", icon: <GraduationCap size={14} />, phrase: "opiekun" },
  { role: "OTHER", label: "narzeczony / narzeczona", icon: <Gem size={14} />, phrase: "narzeczony" },
];

/** The kind of a relation outside the family, changeable (the family's own are changed with `RelativeTools`). */
function RelationChip({ relation: r, personId, run }: { relation: Relation; personId: string; run: (fn: () => Promise<unknown>) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  if (!r.label) return null;
  return (
    <div ref={ref} style={{ position: "relative", minWidth: 0 }}>
      <button className="relation-chip" style={open ? { borderColor: "var(--accent)" } : undefined} onClick={() => setOpen((o) => !o)} aria-haspopup="menu">
        <span className="ellipsis">{r.label}</span>
        <ChevronDown size={12} />
      </button>
      {open && (
        <div className="popover" style={{ left: 0, top: 32, width: 260, padding: "4px 0", fontSize: 13 }}>
          <div className="menu-label">Spoza rodziny</div>
          {ASSO_TYPES.map((t) => (
            <button
              key={t.label}
              className={`menu-item${r.label === t.label || r.label === t.phrase ? " on" : ""}`}
              onClick={() => {
                run(() => call("relation.associate", { person: personId, other: r.id, role: t.role, phrase: t.phrase }));
                setOpen(false);
              }}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

type Hit = PersonSummary & { context: string };

/** „Dodaj krewnego”: links someone already in the archive (search), or makes a new person as this relative. */
function AddRelation({ id, onDone }: { id: string; onDone: () => void }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const go = useStore((s) => s.go);
  const [kind, setKind] = useState<RelationKind | "associate">("parent");
  const [assoc, setAssoc] = useState(0);
  const [custom, setCustom] = useState("");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    // An answer for an earlier query can arrive after the latest one.
    let stale = false;
    call<Hit[]>("people.search", { q: query, limit: 6 })
      .then((list) => !stale && setHits(list.filter((h) => h.id !== id)))
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [query, id]);
  const link = (other: string) =>
    runEdit(async () => {
      if (kind === "associate") {
        const t = ASSO_TYPES[assoc];
        await call("relation.associate", { person: id, other, role: custom.trim() ? "OTHER" : t.role, phrase: custom.trim() || t.phrase });
      } else {
        await call("relation.add", { kind, person: id, other });
      }
      afterChange();
      onDone();
    });
  return (
    <div className="add-relative">
      <div className="row" style={{ gap: 8 }}>
        <span style={{ fontSize: 13, fontWeight: 600 }}>Dodaj krewnego</span>
        <span className="grow" />
        <button className="icon-btn" title="Zamknij" aria-label="Zamknij" onClick={onDone}>
          <X size={15} />
        </button>
      </div>
      <Segmented
        full
        value={kind}
        onChange={setKind}
        size={30}
        options={[
          { value: "parent", label: "Rodzic" },
          { value: "partner", label: "Partner" },
          { value: "child", label: "Dziecko" },
          { value: "sibling", label: "Rodzeństwo" },
          { value: "associate", label: "Spoza rodziny" },
        ]}
      />
      {kind === "associate" && (
        <div className="col" style={{ gap: 6 }}>
          <div className="row" style={{ flexWrap: "wrap", gap: 4 }}>
            {ASSO_TYPES.map((t, i) => (
              <button key={t.label} className={`chip${i === assoc && !custom ? " on" : ""}`} style={{ height: 30 }} onClick={() => setAssoc(i)}>
                {t.icon}
                {t.label}
              </button>
            ))}
          </div>
          <div className="row" style={{ gap: 6 }}>
            <PencilLine size={14} color="var(--text3)" />
            <input className="input sm" placeholder="inna… (własny opis)" value={custom} onChange={(e) => setCustom(e.target.value)} />
          </div>
        </div>
      )}
      <div className="search-box" style={{ height: 38 }}>
        <Search size={14} />
        <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Szukaj osoby, która już jest w archiwum…" />
      </div>
      {hits.map((h) => (
        <button key={h.id} className="menu-item" style={{ minHeight: 48, gap: 10, borderRadius: "var(--r-ctl)" }} onClick={() => link(h.id)}>
          <Avatar initials={h.initials} branch={h.branch} photo={h.photo} size={30} />
          <span className="col grow" style={{ minWidth: 0 }}>
            <span style={{ fontSize: 14, fontWeight: 500 }}>{h.name}</span>
            <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
              {h.context}
            </span>
          </span>
          <span className="num" style={{ fontSize: 12, color: "var(--text2)" }}>
            {cardYears(h.birth?.year, h.death?.year, h.living)}
          </span>
        </button>
      ))}
      {kind !== "associate" && (
        <button className="btn dashed" onClick={() => requireEdit(() => go({ name: "edit", id: null, relation: { kind, of: id } }))}>
          <UserPlus size={14} />
          Utwórz nową osobę jako {RELATION_WORD[kind]}
        </button>
      )}
    </div>
  );
}

// ---------- Życiorys ----------

export function BioSection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:bio`, "Życiorys");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useClosed(sec, setEditingId, setAdding);
  if (!editing && data.bio.length === 0) return null;
  const cited = [...new Set(data.bio.flatMap((b) => b.sources))].sort((a, b) => a - b);
  return (
    <section id="sec-bio" className={sectionClass(sec)} style={{ gap: 20 }}>
      <SectionHead title="Życiorys" edit={sec} />
      {data.bio.map((b) =>
        editingId === b.id ? (
          <TextForm key={b.id} person={data.person.id} kind="bio" item={b} withTitle onDone={() => setEditingId(null)} />
        ) : (
          <div key={b.id ?? b.title} id={b.id ? `sec-bio-${b.id}` : undefined} className="bio-part">
            {(b.title || sec.open) && (
              <div className="row" style={{ gap: 8 }}>
                {b.title && <h3 className="bio-title">{b.title}</h3>}
                <span className="grow" />
                {sec.open && <ItemTools person={data.person.id} item={b} onEdit={() => setEditingId(b.id)} movable />}
              </div>
            )}
            <div className={b.certainty === "low" ? "low-certainty" : undefined}>
              <Markdown text={b.body} from={data.person.id} className="reading" />
              <Sup sources={b.sources} />
            </div>
          </div>
        ),
      )}
      {editing && (adding ? <TextForm person={data.person.id} kind="bio" withTitle onDone={() => setAdding(false)} /> : (sec.open || data.bio.length === 0) && <AddButton label="Dodaj część życiorysu" onClick={() => sec.start(() => setAdding(true))} />)}
      {cited.length > 0 && (
        <div className="footnotes">
          <span className="label-caps" style={{ letterSpacing: "0.08em" }}>
            Źródła
          </span>
          {cited.map((n) => {
            const s = data.sources.find((x) => x.n === n);
            return (
              <span key={n}>
                <b style={{ color: "var(--accent-text)", fontWeight: 600 }}>[{n}]</b> {s?.title}
                {s?.page ? `, ${s.page}` : ""}
              </span>
            );
          })}
        </div>
      )}
      <Foot edit={sec} />
    </section>
  );
}

/** A closed section forgets the text being added or edited in it. */
function useClosed(sec: SectionEdit, setEditingId: (id: string | null) => void, setAdding: (on: boolean) => void) {
  useEffect(() => {
    if (!sec.open) {
      setEditingId(null);
      setAdding(false);
    }
  }, [sec.open, setEditingId, setAdding]);
}

// ---------- Historie, Powiedzonka, Ciekawostki ----------

export function StoriesSection({ data }: Props) {
  const editing = useEditing();
  const go = useStore((s) => s.go);
  const sec = useSection(`${data.person.id}:stories`, "Historie");
  const [open, setOpen] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useClosed(sec, setEditingId, setAdding);
  if (!editing && data.stories.length === 0) return null;
  return (
    <section id="sec-stories" className={sectionClass(sec)} style={{ gap: 14 }}>
      <SectionHead title="Historie" count={data.stories.length} edit={sec} />
      {data.stories.map((s) =>
        editingId === s.id ? (
          <TextForm key={s.id} person={data.person.id} kind="story" item={s} withTitle withDate onDone={() => setEditingId(null)} />
        ) : (
          <div key={s.id ?? s.title} className="story-card">
            <div className="row" style={{ gap: 6 }}>
              <BookOpen size={14} color="var(--accent-text)" />
              <span className="label-caps" style={{ color: "var(--accent-text)", letterSpacing: "0.08em" }}>
                Historia rodzinna
              </span>
              <span className="grow" />
              {sec.open && <ItemTools person={data.person.id} item={s} onEdit={() => setEditingId(s.id)} />}
            </div>
            {s.title && <span className="story-title">{s.title}</span>}
            {(s.date || s.place) && <span style={{ fontSize: 13, color: "var(--text2)" }}>{[s.date, s.place].filter(Boolean).join(" · ")}</span>}
            {open === s.id ? (
              <Markdown text={s.body} from={data.person.id} className="reading story-full" />
            ) : (
              <span className="story-excerpt">{excerpt(plainText(s.body), 260)}</span>
            )}
            <div className="row" style={{ gap: 14 }}>
              {plainText(s.body).length > 260 && (
                <button className="link" style={{ fontSize: 13 }} onClick={() => setOpen(open === s.id ? null : s.id)}>
                  {open === s.id ? "Zwiń" : "Czytaj całą historię →"}
                </button>
              )}
              {s.people.length > 1 && (
                <span style={{ fontSize: 12, color: "var(--text3)" }}>
                  Występują:{" "}
                  {s.people.map((p, i) => (
                    <span key={p.id}>
                      {i > 0 && ", "}
                      <button className="link" onClick={() => go({ name: "person", id: p.id })}>
                        {p.name}
                      </button>
                    </span>
                  ))}
                </span>
              )}
            </div>
          </div>
        ),
      )}
      {editing && (adding ? <TextForm person={data.person.id} kind="story" withTitle withDate onDone={() => setAdding(false)} /> : (sec.open || data.stories.length === 0) && <AddButton label="Dodaj historię" onClick={() => sec.start(() => setAdding(true))} />)}
      <Foot edit={sec} />
    </section>
  );
}

function excerpt(text: string, n: number): string {
  if (text.length <= n) return text;
  return `${text.slice(0, n).replace(/\s+\S*$/, "")}…`;
}

export function SayingsSection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:sayings`, "Powiedzonka");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useClosed(sec, setEditingId, setAdding);
  if (!editing && data.sayings.length === 0) return null;
  return (
    <section id="sec-sayings" className={sectionClass(sec)} style={{ gap: 12 }}>
      <SectionHead title="Powiedzonka" count={data.sayings.length} edit={sec} />
      {data.sayings.map((s) =>
        editingId === s.id ? (
          <TextForm key={s.id} person={data.person.id} kind="saying" item={s} onDone={() => setEditingId(null)} />
        ) : (
          <div key={s.id ?? s.body} className="pull-quote">
            <div className="row">
              <Quote size={22} color="var(--accent-text)" />
              <span className="grow" />
              {sec.open && <ItemTools person={data.person.id} item={s} onEdit={() => setEditingId(s.id)} />}
            </div>
            <Markdown text={s.body} from={data.person.id} className="quote-text" />
            <span className="attribution">
              Powiedzonko{s.sources.length ? ` · źródło [${s.sources.join(", ")}]` : ""}
              {s.date ? ` · ${s.date}` : ""}
            </span>
          </div>
        ),
      )}
      {editing && (adding ? <TextForm person={data.person.id} kind="saying" onDone={() => setAdding(false)} placeholder="„Pociąg nie czeka, a robota nie ucieknie.”" /> : (sec.open || data.sayings.length === 0) && <AddButton label="Dodaj powiedzonko" onClick={() => sec.start(() => setAdding(true))} />)}
      <Foot edit={sec} />
    </section>
  );
}

export function TriviaSection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:trivia`, "Ciekawostki");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useClosed(sec, setEditingId, setAdding);
  if (!editing && data.trivia.length === 0) return null;
  return (
    <section id="sec-trivia" className={sectionClass(sec)} style={{ gap: 12 }}>
      <SectionHead title="Ciekawostki" count={data.trivia.length} edit={sec} />
      {data.trivia.map((t) =>
        editingId === t.id ? (
          <TextForm key={t.id} person={data.person.id} kind="trivia" item={t} onDone={() => setEditingId(null)} />
        ) : (
          <div key={t.id ?? t.body} className="callout">
            <Lightbulb size={18} color="var(--accent-text)" style={{ marginTop: 2, flex: "none" }} />
            <div className="col grow">
              <span className="callout-label">Ciekawostka</span>
              <Markdown text={t.body} from={data.person.id} />
            </div>
            {sec.open && <ItemTools person={data.person.id} item={t} onEdit={() => setEditingId(t.id)} />}
          </div>
        ),
      )}
      {editing && (adding ? <TextForm person={data.person.id} kind="trivia" onDone={() => setAdding(false)} /> : (sec.open || data.trivia.length === 0) && <AddButton label="Dodaj ciekawostkę" onClick={() => sec.start(() => setAdding(true))} />)}
      <Foot edit={sec} />
    </section>
  );
}

// ---------- Powiązania prowadzące donikąd ----------

/** Links to records that are no longer in the archive (an undone import, a file from another program): shown, not
 *  left out without a word; in edit mode each can be taken out. */
export function BrokenLinksNotice({ data }: Props) {
  const editing = useEditing();
  const links = data.brokenLinks ?? [];
  if (links.length === 0) return null;
  const remove = (record: string, xref: string) =>
    useStore.getState().outsideSection(() =>
      runEdit(async () => {
        await call("person.removeBrokenLink", { record, xref });
        afterChange();
      }),
    );
  return (
    <div className="broken-links">
      <TriangleAlert size={18} style={{ flex: "none", color: "var(--warn)" }} />
      <div className="col grow" style={{ gap: 6, minWidth: 0 }}>
        <b>{links.length === 1 ? "Jedno powiązanie prowadzi donikąd" : `Powiązania prowadzące donikąd: ${links.length}`}</b>
        <span style={{ color: "var(--text2)" }}>Wskazują wpisy, których nie ma już w archiwum — na przykład po cofnięciu importu.</span>
        {links.map((l) => (
          <span key={`${l.record}-${l.xref}`} className="row" style={{ gap: 10 }}>
            <span className="grow">
              {l.what.charAt(0).toUpperCase() + l.what.slice(1)} <span className="mono">{l.xref}</span>
              {l.record !== data.person.id && <span style={{ color: "var(--text3)" }}> · w rodzinie {l.record}</span>} — ten link prowadzi donikąd
            </span>
            {editing && (
              <button className="btn ghost xs" onClick={() => remove(l.record, l.xref)}>
                <X size={12} /> Usuń link
              </button>
            )}
          </span>
        ))}
      </div>
    </div>
  );
}

// ---------- Galeria, Dokumenty ----------

export function GallerySection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:gallery`, "Galeria");
  const setAsk = useStore((s) => s.setAsk);
  const [all, setAll] = useState(false);
  const [viewing, setViewing] = useState<number | null>(null);
  if (!editing && data.gallery.length === 0) return null;
  const shown = all ? data.gallery : data.gallery.slice(0, 8);
  const add = () =>
    runEdit(async () => {
      const paths = await pickFiles("Dodaj zdjęcia", [{ name: "Zdjęcia", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif"] }]);
      if (!paths.length) return;
      await call("media.add", { paths, person: data.person.id, kind: "photo", profile: data.gallery.length === 0 });
      afterChange();
    });
  const setProfile = (id: string) =>
    runEdit(async () => {
      await call("media.setProfile", { id, person: data.person.id });
      afterChange();
    });
  const unlink = (id: string) =>
    setAsk({
      title: "Odłączyć zdjęcie od tej osoby?",
      text: "Zniknie z jej galerii, ale zostanie w Mediach. Do zapisu możesz to cofnąć (Ctrl Z).",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Odłącz",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              await call("media.unlink", { id, person: data.person.id });
              afterChange();
            }),
        },
      ],
    });
  return (
    <section id="sec-gallery" className={sectionClass(sec)} style={{ gap: 14 }}>
      <SectionHead
        title="Galeria"
        count={data.gallery.length}
        edit={sec}
        right={
          data.gallery.length > 8 && (
            <button className="link" style={{ fontSize: 13 }} onClick={() => setAll((a) => !a)}>
              {all ? "Pokaż mniej" : `Pokaż wszystkie (${data.gallery.length})`}
            </button>
          )
        }
      />
      <div className="gallery-grid">
        {shown.map((g) => (
          <div key={g.id} className="gallery-item">
            <button style={{ position: "relative", display: "block", width: "100%" }} onClick={() => setViewing(data.gallery.indexOf(g))} title="Otwórz podgląd">
              <Thumb path={g.path} size={512} icon={<ImageIcon size={18} />} style={{ aspectRatio: "4 / 3", width: "100%" }} />
              {g.profile && (
                <span className="star-badge" title="Zdjęcie profilowe">
                  <Star size={11} fill="currentColor" />
                </span>
              )}
            </button>
            <span className="ellipsis" style={{ fontSize: 13 }}>
              {g.caption || "bez podpisu"}
            </span>
            {g.date && <span className={g.uncertain ? "uncertain" : undefined} style={{ fontSize: 12, color: "var(--text3)", alignSelf: "flex-start" }}>{g.date}</span>}
            {sec.open && (
              <span className="row" style={{ gap: 4 }}>
                {!g.profile && (
                  <button className="btn ghost xs" onClick={() => setProfile(g.id)}>
                    <Star size={12} /> profilowe
                  </button>
                )}
                <button className="btn ghost xs" onClick={() => unlink(g.id)}>
                  <X size={12} /> odłącz
                </button>
              </span>
            )}
          </div>
        ))}
        {editing && (sec.open || data.gallery.length === 0) && (
          <button className="gallery-add" onClick={() => sec.start(add)}>
            <Plus size={18} />
            Dodaj zdjęcia
          </button>
        )}
      </div>
      <Foot edit={sec} />
      {viewing != null && (
        <Lightbox items={data.gallery.map((g) => ({ id: g.id, path: g.path, title: g.caption, date: g.date }))} index={viewing} onClose={() => setViewing(null)} title={data.person.name} />
      )}
    </section>
  );
}

export function DocumentsSection({ data }: Props) {
  const [viewing, setViewing] = useState<number | null>(null);
  if (data.documents.length === 0) return null;
  return (
    <section id="sec-documents" className="profile-section" style={{ gap: 14 }}>
      <SectionHead title="Dokumenty" count={data.documents.length} />
      <div className="card">
        {data.documents.map((d) => (
          <div key={d.id} className="list-row clickable" style={{ height: 72, gap: 14 }} {...rowButton(() => setViewing(data.documents.indexOf(d)))}>
            <span className="doc-thumb">
              <FileText size={16} />
            </span>
            <span className="col grow" style={{ minWidth: 0 }}>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>{d.kind}</span>
              <span className="serif ellipsis" style={{ fontSize: 16, fontWeight: 600 }}>
                {d.title}
              </span>
              {d.meta && <span style={{ fontSize: 12, color: "var(--text2)" }}>{d.meta}</span>}
            </span>
            {d.tags.map((t) => (
              <span key={t} className="badge" style={{ fontWeight: 400 }}>
                {t}
              </span>
            ))}
            <span className="num" style={{ width: 44, textAlign: "right", fontSize: 13, color: "var(--text2)" }}>
              {d.year}
            </span>
          </div>
        ))}
      </div>
      {viewing != null && (
        <Lightbox items={data.documents.map((d) => ({ id: d.id, path: d.path, title: d.title, date: d.year ? String(d.year) : null }))} index={viewing} onClose={() => setViewing(null)} title={data.person.name} />
      )}
    </section>
  );
}

// ---------- Źródła, Linki ----------

export function SourcesSection({ data }: Props) {
  const go = useStore((s) => s.go);
  if (data.sources.length === 0) return null;
  return (
    <section id="sec-sources" className="profile-section" style={{ gap: 12 }}>
      <SectionHead title="Źródła" count={data.sources.length} />
      <div className="card">
        {data.sources.map((s) => (
          <div key={s.id} className="list-row clickable" style={{ minHeight: 48, gap: 12, padding: "8px 16px" }} {...rowButton(() => go({ name: "sources", id: s.id }))}>
            <b style={{ color: "var(--accent-text)", fontWeight: 600, width: 30 }}>[{s.n}]</b>
            <span className="col grow" style={{ minWidth: 0 }}>
              <span style={{ fontSize: 14 }}>{s.title}</span>
              {(s.page || s.repository) && <span style={{ fontSize: 12, color: "var(--text3)" }}>{[s.page, s.repository].filter(Boolean).join(" · ")}</span>}
            </span>
            <ArrowRight size={14} color="var(--text3)" />
          </div>
        ))}
      </div>
    </section>
  );
}

function linkIcon(kind: string | null) {
  if (kind === "geneteka" || kind === "familysearch") return <Search size={15} />;
  if (kind === "grave") return <Landmark size={15} />;
  if (kind === "wikipedia") return <Globe size={15} />;
  return <LinkIcon size={15} />;
}

export function LinksSection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:links`, "Linki");
  const [adding, setAdding] = useState(false);
  useEffect(() => {
    if (!sec.open) setAdding(false);
  }, [sec.open]);
  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  useDraftGuard(adding && (!!url.trim() || !!title.trim()));
  if (!editing && data.links.length === 0) return null;
  const save = (links: { url: string; title: string; kind: string | null }[]) =>
    runEdit(async () => {
      await call("person.update", { id: data.person.id, links });
      afterChange();
    });
  // A link without a title comes with its address as the title; sent back as it is, that would become a real title.
  const current = data.links.map((l) => ({ url: l.url, title: l.title === l.url ? "" : l.title, kind: l.kind }));
  const kindFor = (u: string) => (u.includes("wikipedia.org") ? "wikipedia" : u.includes("geneteka") ? "geneteka" : u.includes("familysearch") ? "familysearch" : u.includes("grobonet") ? "grave" : "other");
  return (
    <section id="sec-links" className={sectionClass(sec)} style={{ gap: 12 }}>
      <SectionHead title="Linki" count={data.links.length} edit={sec} />
      {data.links.length > 0 && (
        <div className="card">
          {data.links.map((l) => (
            <div key={l.url} className="list-row clickable" style={{ height: 52, gap: 12 }} {...rowButton(() => openUrl(l.url))}>
              <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
                {linkIcon(l.kind)}
              </span>
              <span className="col grow" style={{ minWidth: 0 }}>
                <span className="ellipsis" style={{ fontSize: 14, fontWeight: 500 }}>
                  {l.title}
                </span>
                <span style={{ fontSize: 12, color: "var(--text3)" }}>{l.domain}</span>
              </span>
              {sec.open ? (
                <button
                  className="icon-btn"
                  title="Usuń link"
                  onClick={(e) => {
                    e.stopPropagation();
                    save(current.filter((c) => c.url !== l.url));
                  }}
                >
                  <Trash2 size={14} />
                </button>
              ) : (
                <span style={{ fontSize: 15, color: "var(--text3)" }}>↗</span>
              )}
            </div>
          ))}
        </div>
      )}
      {editing &&
        (adding ? (
          <div className="text-form">
            <input className="input" placeholder="https://pl.wikipedia.org/wiki/…" value={url} onChange={(e) => setUrl(e.target.value)} autoFocus />
            <input className="input" placeholder="Tytuł, np. Wikipedia: Wólka" value={title} onChange={(e) => setTitle(e.target.value)} />
            <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
              <button className="btn ghost" onClick={() => setAdding(false)}>
                Anuluj
              </button>
              <button
                className="btn primary"
                disabled={!/^https?:\/\//.test(url.trim())}
                onClick={() => {
                  save([...current, { url: url.trim(), title: title.trim(), kind: kindFor(url) }]);
                  setAdding(false);
                  setUrl("");
                  setTitle("");
                }}
              >
                Dodaj link
              </button>
            </div>
          </div>
        ) : (
          (sec.open || data.links.length === 0) && <AddButton label="Dodaj link (Wikipedia, Geneteka, grób…)" onClick={() => sec.start(() => setAdding(true))} />
        ))}
      <Foot edit={sec} />
    </section>
  );
}

// ---------- Oś życia, Wspomniany w ----------

export function TimelineSection({ data }: Props) {
  if (data.timeline.length === 0) return null;
  return (
    <section id="sec-timeline" className="profile-section" style={{ gap: 12 }}>
      <SectionHead
        title="Oś życia"
        right={
          <span className="row" style={{ gap: 6, fontSize: 12, color: "var(--text3)" }}>
            <span style={{ width: 10, height: 10, borderRadius: "50%", border: "2px solid var(--line)" }} />
            zdarzenia bliskich
          </span>
        }
      />
      <div className="timeline">
        {data.timeline.map((t, i) => (
          <div key={i} className={`timeline-row${t.family ? " family" : ""}`}>
            <span className={`num tl-date${t.uncertain ? " uncertain" : ""}`}>{t.date}</span>
            <span className="tl-age">{t.age}</span>
            <span className="tl-rail">
              <span className="tl-dot" />
            </span>
            <span className="tl-content">
              <span style={{ fontSize: 15, fontWeight: t.family ? 400 : 600 }}>
                {t.type}
                {t.place && <span style={{ color: "var(--accent-text)", fontWeight: 400 }}> · {t.place}</span>}
                <Sup sources={t.sources} />
              </span>
              {t.description && <span style={{ fontSize: 13, color: "var(--text2)" }}>{t.description}</span>}
            </span>
          </div>
        ))}
      </div>
      {data.undated.length > 0 && (
        <div style={{ borderTop: "1px solid var(--border)", paddingTop: 14, display: "grid", gridTemplateColumns: "110px 1fr", columnGap: 10, rowGap: 4 }}>
          <span className="label-caps" style={{ textAlign: "right" }}>
            Bez daty
          </span>
          <span className="col" style={{ gap: 4 }}>
            {data.undated.map((u, i) => (
              <span key={i}>
                <b style={{ fontWeight: 600 }}>{u.type}</b> · {u.value}
              </span>
            ))}
          </span>
        </div>
      )}
    </section>
  );
}

export function MentionedSection({ data }: Props) {
  const go = useStore((s) => s.go);
  if (data.mentionedIn.length === 0) return null;
  return (
    <section id="sec-mentioned" className="profile-section" style={{ gap: 12 }}>
      <SectionHead title="Wspomniany w" count={data.mentionedIn.length} />
      <div className="card">
        {data.mentionedIn.map((m, i) => (
          <div key={m.id ?? i} className="list-row clickable" style={{ minHeight: 56, padding: "8px 16px", gap: 12 }} {...rowButton(() => m.owner && go({ name: "person", id: m.owner }))}>
            {m.kind === "story" ? <BookOpen size={16} color="var(--text2)" /> : m.kind === "bio" ? <User size={16} color="var(--text2)" /> : <FileText size={16} color="var(--text2)" />}
            <span className="col grow" style={{ minWidth: 0 }}>
              <span className="serif" style={{ fontSize: 16, fontWeight: 600 }}>
                {m.title}
              </span>
              <span className="ellipsis" style={{ fontSize: 13, color: "var(--text2)" }}>
                „{m.quote}”
              </span>
            </span>
          </div>
        ))}
      </div>
    </section>
  );
}

// ---------- Uwagi badawcze, Historia zmian, Inne dane z pliku ----------

export function NotesSection({ data }: Props) {
  const editing = useEditing();
  const sec = useSection(`${data.person.id}:notes`, "Uwagi badawcze");
  const [open, setOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  useClosed(sec, setEditingId, setAdding);
  if (!editing && data.notes.length === 0) return null;
  const shown = open || sec.open;
  return (
    <section id="sec-notes" className={sectionClass(sec)} style={{ gap: 10 }}>
      {editing && (
        <div className="row" style={{ justifyContent: "flex-end", marginBottom: -4 }}>
          <SectionEditButton edit={sec} />
        </div>
      )}
      <button className="card row collapse-head" onClick={() => setOpen((o) => !o)} aria-expanded={shown}>
        {shown ? <ChevronDown size={16} color="var(--text3)" /> : <ChevronRight size={16} color="var(--text3)" />}
        <span className="serif" style={{ fontSize: 20, fontWeight: 600 }}>
          Uwagi badawcze
        </span>
        <span style={{ fontSize: 14, color: "var(--text3)" }}>{data.notes.length}</span>
        {!shown && data.notes[0] && (
          <span className="ellipsis grow" style={{ fontSize: 13, color: "var(--text2)", textAlign: "left" }}>
            {excerpt(plainText(data.notes[0].body), 110)}
          </span>
        )}
      </button>
      {shown && (
        <div className="col" style={{ gap: 10 }}>
          {data.notes.map((n) =>
            editingId === n.id ? (
              <TextForm key={n.id} person={data.person.id} kind="note" item={n} onDone={() => setEditingId(null)} />
            ) : (
              <div key={n.id ?? n.body} className="info-box row" style={{ alignItems: "flex-start", gap: 10 }}>
                <Markdown text={n.body} from={data.person.id} className="grow" />
                {sec.open && <ItemTools person={data.person.id} item={n} onEdit={() => setEditingId(n.id)} />}
              </div>
            ),
          )}
          {editing && (adding ? <TextForm person={data.person.id} kind="note" onDone={() => setAdding(false)} placeholder="Np. „Czy Józef miał brata Wojciecha? Sprawdzić księgi z Łęcznej 1880–1885.”" /> : (sec.open || data.notes.length === 0) && <AddButton label="Dodaj uwagę" onClick={() => sec.start(() => setAdding(true))} />)}
        </div>
      )}
      <Foot edit={sec} />
    </section>
  );
}

const MONTHS_GENITIVE = ["stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października", "listopada", "grudnia"];

function dayLabel(iso: string, to?: string): string {
  const d = new Date(iso);
  const today = new Date();
  const same = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  const yesterday = new Date(today.getTime() - 86_400_000);
  const range = to && clock(to) !== clock(iso) ? `${clock(iso)}–${clock(to)}` : clock(iso);
  if (same(d, today)) return `Dziś, ${range}`;
  if (same(d, yesterday)) return `Wczoraj, ${range}`;
  return `${String(d.getDate()).padStart(2, "0")}.${String(d.getMonth() + 1).padStart(2, "0")}.${d.getFullYear()}`;
}

export function HistorySection({ data }: Props) {
  const editing = useEditing();
  const notify = useStore((s) => s.notify);
  const [filter, setFilter] = useState<"all" | "import" | "manual">("all");
  const [open, setOpen] = useState<number | null>(0);
  const { origin, groups } = data.history;
  if (!origin && groups.length === 0) return null;
  const shown = groups.filter((g) => filter === "all" || g.kind === filter);
  // Outside any section: an open one is finished first, so its „Anuluj” can't take this back.
  const undo = (index: number) =>
    useStore.getState().outsideSection(() =>
      runEdit(async () => {
        const status = await call<{ undoSkipped?: number }>("history.undo", { index });
        afterChange(status as never);
        notify(status.undoSkipped ? "Nie cofnięto: ten wpis zmienił się później." : "Cofnięto — zmiana czeka na zapis.", { kind: status.undoSkipped ? "err" : "info" });
      }),
    );
  const originDate = origin ? new Date(origin.ts) : null;
  return (
    <section id="sec-history" className="profile-section" style={{ gap: 12 }}>
      <SectionHead title="Historia zmian" />
      <div className="row" style={{ gap: 10 }}>
        {origin && originDate && (
          <span className="row origin-chip">
            <Import size={14} />
            Dodano {originDate.getDate()} {MONTHS_GENITIVE[originDate.getMonth()]} {originDate.getFullYear()} — {origin.batch ? `import „${origin.batch}”` : `ręcznie (${origin.who})`}
          </span>
        )}
        <span className="grow" />
        <Segmented
          variant="neutral"
          value={filter}
          onChange={setFilter}
          options={[
            { value: "all", label: "Wszystko" },
            { value: "import", label: "Importy" },
            { value: "manual", label: "Zmiany ręczne" },
          ]}
        />
      </div>
      {shown.length > 0 && (
        <div className="card">
          {shown.map((g, i) => (
            <div key={`${g.from}-${i}`} style={{ borderTop: i ? "1px solid var(--border)" : undefined }}>
              <button className="row history-group" onClick={() => setOpen(open === i ? null : i)}>
                {open === i ? <ChevronDown size={15} color="var(--text3)" /> : <ChevronRight size={15} color="var(--text3)" />}
                <span style={{ fontWeight: 600, whiteSpace: "nowrap" }}>{dayLabel(g.from, g.to)}</span>
                <span className="ellipsis grow" style={{ color: "var(--text2)", textAlign: "left" }}>
                  {g.batch ? `import „${g.batch}” · import` : `${g.who} · sesja edycji`}
                </span>
                <span style={{ color: "var(--text3)", whiteSpace: "nowrap" }}>{count(g.count, "zmiana", "zmiany", "zmian")}</span>
              </button>
              {open === i && (
                <div style={{ padding: "0 16px 10px 40px" }}>
                  {g.lines.map((l, j) => (
                    <div key={j} className="history-line">
                      <span style={{ color: "var(--text2)" }}>{l.field}</span>
                      <span className="ellipsis" style={{ color: "var(--text3)" }}>
                        {l.old ?? "—"}
                      </span>
                      <ArrowRight size={13} color="var(--text3)" />
                      <span className="ellipsis" style={{ fontWeight: 500 }}>
                        {l.new ?? "—"}
                      </span>
                      {editing ? (
                        <button className="btn secondary xs" onClick={() => undo(l.index)}>
                          <Undo2 size={12} />
                          Cofnij
                        </button>
                      ) : (
                        <span />
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {data.changed && <span style={{ fontSize: 12, color: "var(--text3)" }}>Ostatnia zmiana w pliku: {relativeTime(data.changed)}</span>}
    </section>
  );
}

export function OtherDataSection({ data }: Props) {
  const [open, setOpen] = useState(false);
  if (data.other.length === 0) return null;
  return (
    <section id="sec-other" className="profile-section">
      <div className="card">
        <button className="row collapse-head" style={{ border: "none", width: "100%" }} onClick={() => setOpen((o) => !o)}>
          {open ? <ChevronDown size={16} color="var(--text3)" /> : <ChevronRight size={16} color="var(--text3)" />}
          <span className="serif" style={{ fontSize: 20, fontWeight: 600 }}>
            Inne dane z pliku
          </span>
          <span style={{ fontSize: 14, color: "var(--text3)" }}>{data.other.length}</span>
          <span className="grow" />
          <span className="row" style={{ gap: 5, fontSize: 12, color: "var(--text3)" }}>
            <Lock size={13} />
            tylko do odczytu
          </span>
        </button>
        {open && (
          <>
            {data.other.map((o, i) => (
              <div key={i} className="other-row">
                <span className="mono" style={{ color: "var(--text2)" }}>
                  {o.tag}
                </span>
                <span className="selectable" style={{ whiteSpace: "pre-wrap" }}>
                  {o.value}
                  {o.more && <span style={{ color: "var(--text3)" }}>{`\n${o.more}`}</span>}
                </span>
              </div>
            ))}
            <div style={{ padding: "10px 18px", fontSize: 12, color: "var(--text3)", borderTop: "1px solid var(--border)" }}>
              Pola z innych programów, których Heirloom nie zna. Zachowujemy je w pliku bez zmian.
            </div>
          </>
        )}
      </div>
    </section>
  );
}

