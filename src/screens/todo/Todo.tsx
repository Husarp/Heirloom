import { ChevronDown, ChevronRight, CircleCheck, FileText, ImageOff } from "lucide-react";
import { useState, type ReactNode } from "react";
import { call } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, EmptyState, Segmented, Spinner, Thumb } from "../../components/bits";
import { fileName, kindText, runEdit, type MediaItem } from "../media/shared";
import { PAGE, personChips, sectionLabel, TOAST_IGNORED, TOAST_RESTORED, toastDone, visible } from "./todoText";
import { UndescribedChip } from "./UndescribedChip";
import "./todo.css";

type TodoPerson = PersonSummary & { years: string; linked: boolean; batch: string | null; hint: string | null };

interface TodoData {
  people: TodoPerson[];
  photos: MediaItem[];
  documents: MediaItem[];
  ignored: { people: TodoPerson[]; photos: MediaItem[]; documents: MediaItem[] };
  counts: { people: number; photos: number; documents: number; ignored: number };
}

type Filter = "all" | "people" | "photos" | "documents";
type Limits = { people: number; photos: number; documents: number };

const HOW_IT_FILLS = "Tu trafiają osoby wspomniane w imporcie bez własnych danych, osoby oznaczone „Do dokończenia” oraz zdjęcia i dokumenty bez podpisu.";

/** „Do uzupełnienia”: unfinished people, and photos and documents without a caption, in one place. „Gotowe” and „Pomiń”
 *  are ordinary record edits; a skipped item waits folded under „Pominięte” and comes back with „Przywróć”. */
export function Todo() {
  const { data, error } = useApi<TodoData>("todo.list", {});
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const [filter, setFilter] = useState<Filter>("all");
  const [limit, setLimit] = useState<Limits>({ people: PAGE, photos: PAGE, documents: PAGE });
  const [foldedOpen, setFoldedOpen] = useState(false);

  /** One change in edit mode, then the toast. */
  const act = (method: string, args: object, message: string) =>
    runEdit(async () => {
      await call(method, args);
      afterChange();
      notify(message);
    });
  const personDone = (p: TodoPerson) => act("person.setTodo", { id: p.id, todo: false }, toastDone(p.name));
  const personSkip = (p: TodoPerson) => act("person.setIgnored", { id: p.id, ignored: true }, TOAST_IGNORED);
  const personRestore = (p: TodoPerson) => act("person.setIgnored", { id: p.id, ignored: false }, TOAST_RESTORED);
  const fileSkip = (m: MediaItem) => act("media.setIgnored", { id: m.id, ignored: true }, TOAST_IGNORED);
  const fileRestore = (m: MediaItem) => act("media.setIgnored", { id: m.id, ignored: false }, TOAST_RESTORED);
  const describe = (m: MediaItem) => go({ name: "media", id: m.id });
  const more = (key: keyof Limits) => setLimit((l) => ({ ...l, [key]: l[key] + PAGE }));

  const head = (
    <div className="col" style={{ gap: 4 }}>
      <h1 className="page-title">Do uzupełnienia</h1>
      <span style={{ fontSize: 14, color: "var(--text2)" }}>Niedokończone osoby oraz zdjęcia i dokumenty bez opisu — w jednym miejscu.</span>
    </div>
  );

  if (error)
    return (
      <div className="page">
        <div className="todo-page">
          {head}
          <div className="banner err">{error.message}</div>
        </div>
      </div>
    );
  if (!data)
    return (
      <div className="page">
        <div className="todo-page">
          {head}
          <div className="row" style={{ gap: 10, color: "var(--text3)", fontSize: 13 }}>
            <Spinner size={16} />
            Sprawdzam, co czeka na uzupełnienie…
          </div>
        </div>
      </div>
    );

  const ignored = data.ignored;
  const ignoredCount = ignored.people.length + ignored.photos.length + ignored.documents.length;
  const nothing = data.people.length + data.photos.length + data.documents.length === 0;
  const options = [
    { value: "all" as const, label: "Wszystko" },
    { value: "people" as const, label: sectionLabel("Osoby", data.people.length) },
    { value: "photos" as const, label: sectionLabel("Zdjęcia", data.photos.length) },
    { value: "documents" as const, label: sectionLabel("Dokumenty", data.documents.length) },
  ];

  const moreButton = (n: number, k: keyof Limits) =>
    n > 0 ? (
      <button className="btn secondary sm todo-more" onClick={() => more(k)}>
        Pokaż więcej ({n})
      </button>
    ) : null;

  /** A section: shown under „Wszystko” when it has something, always under its own filter. */
  const section = (key: Exclude<Filter, "all">, title: string, n: number, body: ReactNode) =>
    (filter === "all" && n > 0) || filter === key ? (
      <section key={key} className="col" style={{ gap: 10 }}>
        {filter === "all" && (
          <h2 className="todo-section">
            {title} <span>{n}</span>
          </h2>
        )}
        {n > 0 ? body : <span style={{ fontSize: 14, color: "var(--text3)" }}>Nic tu nie czeka.</span>}
      </section>
    ) : null;

  const people = visible(data.people, limit.people);
  const photos = visible(data.photos, limit.photos);
  const documents = visible(data.documents, limit.documents);

  return (
    <div className="page">
      <div className="todo-page">
        {head}

        {nothing ? (
          ignoredCount === 0 ? (
            <div className="card">
              <EmptyState icon={<CircleCheck size={32} color="var(--accent-text)" />} title="Nic nie czeka na uzupełnienie." text={HOW_IT_FILLS} />
            </div>
          ) : (
            <span style={{ fontSize: 14, color: "var(--text2)" }}>Nic nie czeka na uzupełnienie. {HOW_IT_FILLS}</span>
          )
        ) : (
          <>
            <Segmented options={options} value={filter} onChange={setFilter} variant="neutral" style={{ alignSelf: "flex-start" }} />

            {section(
              "people",
              "Osoby",
              data.people.length,
              <div className="col" style={{ gap: 8 }}>
                {people.items.map((p) => (
                  <div key={p.id} className="todo-row">
                    <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={44} />
                    <div className="col todo-main">
                      <span className="row" style={{ gap: 8, alignItems: "baseline", minWidth: 0 }}>
                        <span className="serif ellipsis todo-name">{p.name}</span>
                        {p.years && <span className="todo-years">{p.years}</span>}
                      </span>
                      {p.hint && <span className="todo-hint">{p.hint}</span>}
                      <span className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                        {personChips(p).map((c) => (
                          <span key={c.text} className={`todo-chip${c.warn ? " warn" : ""}`} title={c.title}>
                            {c.text}
                          </span>
                        ))}
                      </span>
                    </div>
                    <div className="row todo-actions">
                      <button className="btn secondary sm" onClick={() => go({ name: "person", id: p.id })}>
                        Otwórz profil
                      </button>
                      <button className="btn ghost sm" title="Usuwa oznaczenie „do uzupełnienia”" onClick={() => personDone(p)}>
                        Gotowe
                      </button>
                      <button className="btn ghost sm" title="Chowa osobę w „Pominięte”" onClick={() => personSkip(p)}>
                        Pomiń
                      </button>
                    </div>
                  </div>
                ))}
                {moreButton(people.more, "people")}
              </div>,
            )}

            {section(
              "photos",
              "Zdjęcia",
              data.photos.length,
              <>
                <div className="todo-grid">
                  {photos.items.map((m) => (
                    <div key={m.id} className="todo-tile">
                      <div className="todo-thumb">
                        <Thumb
                          path={m.missing ? null : m.path}
                          size={256}
                          icon={<ImageOff size={20} />}
                          label={m.missing ? "Brak pliku" : undefined}
                          style={{ width: "100%", height: "100%", ...(m.missing ? { background: "transparent", border: "1px dashed var(--line)" } : null) }}
                        />
                        <UndescribedChip />
                      </div>
                      <span className="ellipsis todo-file" title={fileName(m.path)}>
                        {fileName(m.path) || "bez pliku"}
                      </span>
                      <PeopleLine item={m} />
                      <span className="row" style={{ gap: 4 }}>
                        <button className="btn secondary xs" onClick={() => describe(m)}>
                          Opisz
                        </button>
                        <button className="btn ghost xs" onClick={() => fileSkip(m)}>
                          Pomiń
                        </button>
                      </span>
                    </div>
                  ))}
                </div>
                {moreButton(photos.more, "photos")}
              </>,
            )}

            {section(
              "documents",
              "Dokumenty",
              data.documents.length,
              <div className="col" style={{ gap: 8 }}>
                {documents.items.map((m) => (
                  <div key={m.id} className="todo-row">
                    <FileText size={22} color="var(--text3)" style={{ flex: "none" }} />
                    <div className="col todo-main">
                      <span className="ellipsis todo-file" title={fileName(m.path)}>
                        {fileName(m.path) || "bez pliku"}
                      </span>
                      <span className="todo-hint">{kindText(m)}</span>
                      <PeopleLine item={m} />
                    </div>
                    <div className="row todo-actions">
                      <button className="btn secondary sm" onClick={() => describe(m)}>
                        Opisz
                      </button>
                      <button className="btn ghost sm" onClick={() => fileSkip(m)}>
                        Pomiń
                      </button>
                    </div>
                  </div>
                ))}
                {moreButton(documents.more, "documents")}
              </div>,
            )}
          </>
        )}

        {ignoredCount > 0 && (
          <section className="col" style={{ gap: 8 }}>
            <button className="todo-fold" aria-expanded={foldedOpen} onClick={() => setFoldedOpen((o) => !o)}>
              {foldedOpen ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
              Pominięte ({ignoredCount})
            </button>
            {foldedOpen && (
              <div className="col" style={{ gap: 2 }}>
                {ignored.people.map((p) => (
                  <div key={p.id} className="todo-skipped">
                    <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={28} />
                    <span className="grow ellipsis">{p.name}</span>
                    <button className="btn ghost xs" onClick={() => personRestore(p)}>
                      Przywróć
                    </button>
                  </div>
                ))}
                {[...ignored.photos, ...ignored.documents].map((m) => (
                  <div key={m.id} className="todo-skipped">
                    <Thumb path={m.missing || m.kind !== "photo" ? null : m.path} size={128} icon={<FileText size={14} />} style={{ width: 28, height: 28, flex: "none", borderRadius: 4 }} />
                    <span className="grow ellipsis" title={fileName(m.path)}>
                      {fileName(m.path) || "bez pliku"}
                    </span>
                    <button className="btn ghost xs" onClick={() => fileRestore(m)}>
                      Przywróć
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}

/** The people on a photo or document, short ("Jan Nowak, Anna Kowalska +2"). */
function PeopleLine({ item }: { item: MediaItem }) {
  if (item.people.length === 0) return null;
  const names = item.people.map((p) => p.name);
  return (
    <span className="ellipsis todo-hint" title={names.join(", ")}>
      {names.slice(0, 2).join(", ")}
      {names.length > 2 ? ` +${names.length - 2}` : ""}
    </span>
  );
}
