// Źródła (spec §4.32): the records, indexes, accounts and web pages the facts rest on. The right side shows the
// scan with its texts and „Co potwierdza”: every fact that cites the source.

import { Book, FileText, Image as ImageIcon, Import, Landmark, Library, MessageSquareQuote, Pencil, Plus, Search, Trash2, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { EmptyState, Segmented, Spinner } from "../../components/bits";
import { count, num } from "../../lib/format";
import { openUrl } from "../../lib/native";
import { SourceViewer, type Scan, type ViewMode } from "./SourceViewer";
import "./sources.css";

interface SourceItem {
  id: string;
  title: string;
  kind: string | null;
  icon: string;
  group: string;
  meta: string;
  uses: number;
}

interface SourceDetail {
  id: string;
  title: string | null;
  kind: string | null;
  author: string | null;
  publication: string | null;
  parish: string | null;
  year: string | null;
  akt: string | null;
  url: string | null;
  domain: string | null;
  repository: string | null;
  callNumber: string | null;
  text: string | null;
  note: string | null;
  scans: Scan[];
  facts: {
    fact: string;
    person: { id: string; name: string };
    value: string;
    page: string | null;
    certainty: "high" | "medium" | "low" | "inferred" | null;
  }[];
  peopleCount: number;
}

const ICONS: Record<string, LucideIcon> = {
  "file-text": FileText,
  search: Search,
  "message-square-quote": MessageSquareQuote,
  book: Book,
  image: ImageIcon,
  landmark: Landmark,
};

const GROUPS = [
  { value: "all", label: "Wszystkie" },
  { value: "records", label: "Akty" },
  { value: "indexes", label: "Indeksy" },
  { value: "accounts", label: "Relacje" },
  { value: "web", label: "Strony www" },
  { value: "other", label: "Inne" },
];

/** The import format's source kinds (docs/IMPORT_FORMAT.md) in words. */
const KIND_LABELS: Record<string, string> = {
  parish_record: "akt parafialny",
  civil_record: "akt stanu cywilnego",
  index: "indeks",
  photo: "zdjęcie",
  letter: "list",
  oral: "relacja ustna",
  note: "notatka",
  book: "książka",
  website: "strona www",
  other: "inne",
};

// „wywnioskowane” is a basis, not a certainty level, but the design shows it in the same column (§4.32 flag).
const CERTAINTY: Record<string, string> = { high: "pewne", medium: "prawdopodobne", low: "niepewne", inferred: "wywnioskowane" };

/** „akt urodzenia” when a record's title says which act it is, else the kind in words. */
function typeLabel(kind: string | null, title: string | null): string | null {
  if (!kind) return null;
  const act = (kind === "parish_record" || kind === "civil_record") && title?.match(/^akt (urodzenia|chrztu|małżeństwa|ślubu|zgonu|zejścia)/i);
  return act ? act[0].toLowerCase() : (KIND_LABELS[kind] ?? kind);
}

export function Sources({ selected }: { selected?: string }) {
  const { data, error } = useApi<{ items: SourceItem[] }>("sources.list");
  const go = useStore((s) => s.go);
  const editing = useStore((s) => s.mode === "edit");
  const [group, setGroup] = useState("all");
  const [creating, setCreating] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  const items = data?.items ?? [];
  // Only kinds the archive has; with a single kind the chips would all show the same list.
  const groups = GROUPS.filter((g) => g.value === "all" || items.some((i) => i.group === g.value));
  const showChips = groups.length > 2;
  const active = showChips && groups.some((g) => g.value === group) ? group : "all";
  const shown = active === "all" ? items : items.filter((i) => i.group === active);
  const adding = creating && editing;

  // Arriving with a source chosen elsewhere (a story's provenance): bring it into view.
  useEffect(() => {
    listRef.current?.querySelector(".sources-row.selected")?.scrollIntoView({ block: "nearest" });
  }, [selected, items.length]);

  const open = (id: string) => {
    setCreating(false);
    go({ name: "sources", id });
  };

  if (data && items.length === 0 && !adding) {
    return (
      <div className="page sources-empty">
        <EmptyState
          icon={<Library size={28} />}
          title="Nie ma jeszcze źródeł"
          text="Akty, indeksy i relacje rodzinne pojawią się tu po imporcie paczki albo po dodaniu ich w trybie edycji."
          action={
            <div className="row" style={{ gap: 8, marginTop: 6 }}>
              {editing && (
                <button className="btn secondary" onClick={() => setCreating(true)}>
                  <Plus size={15} />
                  Dodaj źródło
                </button>
              )}
              <button className="btn primary" onClick={() => go({ name: "import" })}>
                <Import size={15} />
                Importuj paczkę
              </button>
            </div>
          }
        />
      </div>
    );
  }

  return (
    <div className="sources">
      <div className="sources-side">
        <div className="sources-head">
          <div className="row" style={{ gap: 8 }}>
            <h1 className="sources-title grow">
              Źródła {data && <span>{num(items.length)}</span>}
            </h1>
            {editing && (
              <button className="btn secondary xs" onClick={() => setCreating(true)}>
                <Plus size={13} />
                Dodaj
              </button>
            )}
          </div>
          {showChips && (
            <div className="sources-types">
              {groups.map((g) => (
                <button key={g.value} className={`sources-type${active === g.value ? " on" : ""}`} onClick={() => setGroup(g.value)}>
                  {g.label}
                </button>
              ))}
            </div>
          )}
        </div>
        <div ref={listRef} className="sources-rows">
          {error && <div className="sources-none">{error.message}</div>}
          {!data && !error && (
            <div className="sources-none">
              <Spinner size={18} />
            </div>
          )}
          {shown.map((s) => {
            const Icon = ICONS[s.icon] ?? FileText;
            const meta = s.meta || typeLabel(s.kind, s.title);
            return (
              <button key={s.id} className={`sources-row${s.id === selected && !adding ? " selected" : ""}`} onClick={() => open(s.id)}>
                <Icon size={16} />
                <span className="col grow" style={{ lineHeight: 1.3, minWidth: 0 }}>
                  <span className="sources-row-name">{s.title}</span>
                  {meta && <span className="sources-row-meta">{meta}</span>}
                </span>
                <span className="sources-row-uses">{s.uses ? count(s.uses, "fakt", "fakty", "faktów") : "nieużywane"}</span>
              </button>
            );
          })}
        </div>
      </div>
      <div className="sources-detail">
        {adding ? (
          <SourceForm
            onDone={(id) => {
              setCreating(false);
              if (id) go({ name: "sources", id });
            }}
          />
        ) : selected ? (
          <SourceView key={selected} id={selected} />
        ) : (
          <div className="sources-pick">
            <EmptyState icon={<Library size={28} />} title="Wybierz źródło" text="Zobaczysz skan, jego transkrypcję i tłumaczenie oraz fakty, które źródło potwierdza." />
          </div>
        )}
      </div>
    </div>
  );
}

function SourceView({ id }: { id: string }) {
  const { data: src, error } = useApi<SourceDetail>("source.get", { id });
  const go = useStore((s) => s.go);
  const editing = useStore((s) => s.mode === "edit");
  const setAsk = useStore((s) => s.setAsk);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [mode, setMode] = useState<ViewMode>("both");
  const [form, setForm] = useState(false);

  if (error) return <EmptyState title="Nie ma takiego źródła" text={error.code === "not_found" ? "Mogło zostać usunięte albo cofnięte." : error.message} />;
  if (!src) {
    return (
      <div className="row" style={{ justifyContent: "center", padding: 40, color: "var(--text3)" }}>
        <Spinner size={20} />
      </div>
    );
  }
  if (form && editing) return <SourceForm source={src} onDone={() => setForm(false)} />;

  const badge = typeLabel(src.kind, src.title);
  const url = src.url;
  const archive = [src.repository, src.callNumber && `sygn. ${src.callNumber}`].filter(Boolean).join(", ");
  const meta = [
    src.parish && (/^par/i.test(src.parish) ? src.parish : `parafia ${src.parish}`),
    src.year,
    src.akt && (/^nr/i.test(src.akt) ? src.akt : `nr ${src.akt}`),
    src.author,
    src.publication,
    archive,
  ].filter(Boolean) as string[];
  const hasScan = src.scans.length > 0;
  const hasText = !!src.text || src.scans.some((s) => s.transcription || s.translation);

  const remove = () =>
    setAsk({
      title: "Usunąć to źródło?",
      text: `${src.facts.length ? `Zniknie też z ${count(src.facts.length, "faktu", "faktów", "faktów")}, ${src.facts.length === 1 ? "który" : "które"} potwierdza. ` : ""}Do zapisu możesz to cofnąć (Ctrl Z), a po zapisie — w Historii zmian.`,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń",
          kind: "danger",
          run: () =>
            requireEdit(async () => {
              try {
                await call("source.delete", { id: src.id });
                afterChange();
                go({ name: "sources" });
              } catch (e) {
                notify((e as Error).message, { kind: "err" });
              }
            }),
        },
      ],
    });

  return (
    <>
      <div className="row" style={{ alignItems: "flex-start", gap: 12 }}>
        <div className="col grow" style={{ gap: 4 }}>
          {badge && <span className="sources-badge">{badge}</span>}
          <h2 className="sources-h2">{src.title || "Źródło bez tytułu"}</h2>
          {(meta.length > 0 || url) && (
            <span className="sources-meta">
              {meta.join(" · ")}
              {url && (
                <>
                  {meta.length > 0 && " · "}
                  <button className="link" onClick={() => openUrl(url)}>
                    {src.domain || url} ↗
                  </button>
                </>
              )}
            </span>
          )}
        </div>
        {editing && (
          <div className="row" style={{ gap: 4 }}>
            <button className="btn ghost xs" onClick={() => setForm(true)}>
              <Pencil size={13} />
              Edytuj
            </button>
            <button className="btn ghost xs" onClick={remove}>
              <Trash2 size={13} />
              Usuń
            </button>
          </div>
        )}
        {hasScan && hasText && (
          <Segmented
            variant="neutral"
            size={28}
            value={mode}
            onChange={setMode}
            options={[
              { value: "scan", label: "Skan" },
              { value: "both", label: "Skan + tekst" },
              { value: "text", label: "Tekst" },
            ]}
          />
        )}
      </div>

      {(hasScan || hasText) && <SourceViewer scans={src.scans} text={src.text} mode={hasScan && hasText ? mode : hasScan ? "scan" : "text"} />}

      {src.note && (
        <div className="info-box selectable">
          <b>Notatka.</b> {src.note}
        </div>
      )}

      <div className="card">
        <div className="row sources-facts-head">
          <b>Co potwierdza</b>
          {src.facts.length > 0 && (
            <span>
              {count(src.facts.length, "fakt", "fakty", "faktów")} u {count(src.peopleCount, "osoby", "osób", "osób")}
            </span>
          )}
        </div>
        {src.facts.length === 0 ? (
          <div className="sources-none">Żaden fakt w archiwum nie powołuje się jeszcze na to źródło.</div>
        ) : (
          src.facts.map((f, i) => (
            <div key={i} className="sources-fact">
              <span className="ellipsis" style={{ color: "var(--text2)" }}>
                {f.fact}
              </span>
              <button className="ellipsis sources-fact-person" onClick={() => go({ name: "person", id: f.person.id })}>
                {f.person.name}
              </button>
              <span className="ellipsis" title={f.value}>
                {f.value || "—"}
              </span>
              <span className="ellipsis" style={{ color: "var(--text3)" }} title={f.page ?? undefined}>
                {f.page}
              </span>
              {f.certainty ? <span className={`sources-pill${f.certainty === "high" ? " sure" : ""}`}>{CERTAINTY[f.certainty]}</span> : <span />}
            </div>
          ))
        )}
      </div>
    </>
  );
}

/** A plain form for a new or existing source (edit mode only). */
function SourceForm({ source, onDone }: { source?: SourceDetail; onDone: (id?: string) => void }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [busy, setBusy] = useState(false);
  const [f, setF] = useState({
    title: source?.title ?? "",
    kind: source?.kind ?? "",
    year: source?.year ?? "",
    parish: source?.parish ?? "",
    akt: source?.akt ?? "",
    archive: source?.repository ?? "",
    callNumber: source?.callNumber ?? "",
    url: source?.url ?? "",
    text: source?.text ?? "",
    note: source?.note ?? "",
  });
  const set = (key: keyof typeof f) => (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) => setF({ ...f, [key]: e.target.value });

  const save = () =>
    requireEdit(async () => {
      if (!f.title.trim()) {
        notify("Podaj tytuł źródła.", { kind: "err" });
        return;
      }
      setBusy(true);
      try {
        // Empty fields are sent too: the API removes what was cleared.
        const result = await call<{ id: string }>("source.save", { ...(source ? { id: source.id } : {}), ...f, kind: f.kind || null });
        afterChange();
        onDone(result.id);
      } catch (e) {
        notify((e as Error).message, { kind: "err" });
      } finally {
        setBusy(false);
      }
    });

  return (
    <div className="col sources-form">
      <h2 className="sources-h2">{source ? "Edytuj źródło" : "Nowe źródło"}</h2>
      <div className="sources-form-grid">
        <Field label="Tytuł" wide>
          <input className="input" autoFocus value={f.title} onChange={set("title")} placeholder="np. Akt urodzenia nr 45/1878" />
        </Field>
        <Field label="Rodzaj">
          <select className="input" value={f.kind} onChange={set("kind")}>
            <option value="">nie określono</option>
            {Object.entries(KIND_LABELS).map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Rok">
          <input className="input" value={f.year} onChange={set("year")} placeholder="np. 1878" />
        </Field>
        <Field label="Parafia">
          <input className="input" value={f.parish} onChange={set("parish")} placeholder="np. Łęczna" />
        </Field>
        <Field label="Numer aktu">
          <input className="input" value={f.akt} onChange={set("akt")} placeholder="np. 45" />
        </Field>
        <Field label="Archiwum">
          <input className="input" value={f.archive} onChange={set("archive")} placeholder="np. Archiwum Państwowe w Lublinie" />
        </Field>
        <Field label="Sygnatura">
          {/* The API keeps the call number with the archive, so it needs one. */}
          <input
            className="input"
            value={f.callNumber}
            onChange={set("callNumber")}
            disabled={!f.archive.trim()}
            placeholder={f.archive.trim() ? "np. 35/1712/0/45" : "najpierw podaj archiwum"}
          />
        </Field>
        <Field label="Adres strony" wide>
          <input className="input" value={f.url} onChange={set("url")} placeholder="https://…" />
        </Field>
        <Field label="Tekst źródła" wide>
          <textarea className="input" rows={5} value={f.text} onChange={set("text")} placeholder="Przepisany tekst aktu albo jego fragment" />
        </Field>
        <Field label="Notatka" wide>
          <textarea className="input" rows={3} value={f.note} onChange={set("note")} />
        </Field>
      </div>
      <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
        <button className="btn ghost" onClick={() => onDone()}>
          Anuluj
        </button>
        <button className="btn primary" disabled={busy} onClick={save}>
          {source ? "Zapisz" : "Dodaj źródło"}
        </button>
      </div>
    </div>
  );
}

function Field({ label, wide, children }: { label: string; wide?: boolean; children: ReactNode }) {
  return (
    <label className="field" style={wide ? { gridColumn: "1 / -1" } : undefined}>
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}
