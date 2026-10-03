// The reading panel of Historie (spec §4.28, right side): the whole text, who appears in it, photos and where it
// comes from. In edit mode it also offers a simple form and deleting.

import { BookOpen, Calendar, FileX, Lightbulb, MapPin, MessageSquareQuote, Network, Pencil, Quote, Trash2, X, type LucideIcon } from "lucide-react";
import { Fragment, useState } from "react";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, EmptyState, Segmented, Spinner, Thumb } from "../../components/bits";
import { Markdown } from "../../components/Markdown";

export const KINDS: Record<string, { label: string; icon: LucideIcon; question: string }> = {
  story: { label: "Historia", icon: BookOpen, question: "Usunąć tę historię?" },
  saying: { label: "Powiedzonko", icon: Quote, question: "Usunąć to powiedzonko?" },
  trivia: { label: "Ciekawostka", icon: Lightbulb, question: "Usunąć tę ciekawostkę?" },
};

const CERTAINTY: Record<string, string> = { high: "pewne", medium: "prawdopodobne", low: "niepewne" };

/** A story date; approximate ones („ok. 1850”, „przed 1910”) get the dotted marker of spec §5.7. The API sends only
 *  the words, so the qualifier is read from them. */
export function StoryDate({ text }: { text: string | null }) {
  if (!text) return <span>bez daty</span>;
  return <span className={/^(ok\.|około|przed|po|między|wyliczone:?)\s/.test(text) ? "uncertain" : undefined}>{text}</span>;
}

interface StoryDetail {
  id: string;
  kind: string;
  title: string | null;
  body: string;
  date: string | null;
  place: string | null;
  certainty: "high" | "medium" | "low" | null;
  inferred: boolean;
  people: { id: string; name: string; initials: string; branch: number }[];
  owner: string | null;
  sources: { id: string; title: string | null }[];
  photos: { id: string; path: string | null; caption: string | null }[];
}

export function StoryPanel({ id }: { id: string }) {
  const { data: story, error } = useApi<StoryDetail>("story.get", { id });
  const go = useStore((s) => s.go);
  const editing = useStore((s) => s.mode === "edit");
  const setAsk = useStore((s) => s.setAsk);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [form, setForm] = useState(false);
  const kind = KINDS[story?.kind ?? "story"] ?? { label: "Tekst", icon: BookOpen, question: "Usunąć ten tekst?" };
  const Icon = kind.icon;

  const remove = () =>
    setAsk({
      title: kind.question,
      text: "Zniknie z Historii i z profili osób. Do zapisu możesz to cofnąć (Ctrl Z), a po zapisie — w Historii zmian.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń",
          kind: "danger",
          run: () =>
            requireEdit(async () => {
              try {
                await call("text.delete", { id });
                afterChange();
                go({ name: "stories" });
              } catch (e) {
                notify((e as Error).message, { kind: "err" });
              }
            }),
        },
      ],
    });

  return (
    <aside className="stories-panel">
      <div className="row" style={{ gap: 8 }}>
        <span className="row label-caps stories-kind">
          <Icon size={14} />
          {kind.label}
        </span>
        <span className="grow" />
        {editing && story && !form && (
          <>
            <button className="btn ghost xs" onClick={() => setForm(true)}>
              <Pencil size={13} />
              Edytuj
            </button>
            <button className="btn ghost xs" onClick={remove}>
              <Trash2 size={13} />
              Usuń
            </button>
          </>
        )}
        <button className="icon-btn" title="Zamknij" aria-label="Zamknij" onClick={() => go({ name: "stories" })}>
          <X size={17} />
        </button>
      </div>
      {error ? (
        <EmptyState title="Nie ma takiej historii" text={error.code === "not_found" ? "Mogła zostać usunięta albo cofnięta." : error.message} />
      ) : !story ? (
        <div className="row" style={{ justifyContent: "center", padding: 40, color: "var(--text3)" }}>
          <Spinner size={20} />
        </div>
      ) : form && editing ? (
        <StoryForm story={story} onDone={() => setForm(false)} />
      ) : (
        <StoryText story={story} />
      )}
    </aside>
  );
}

function StoryText({ story }: { story: StoryDetail }) {
  const go = useStore((s) => s.go);
  const saying = story.kind === "saying";
  // A saying without a title is just the quote: it reads as a pull quote (spec §5.9).
  const quoteOnly = saying && !story.title;
  const owner = story.owner;
  return (
    <>
      {(story.title || story.date || story.place) && (
        <div className="col" style={{ gap: 8 }}>
          {story.title && <h2 className={`stories-title${saying ? " saying" : ""}`}>{story.title}</h2>}
          {(story.date || story.place) && (
            <div className="row stories-meta">
              {story.date && (
                <span className="row">
                  <Calendar size={14} />
                  <StoryDate text={story.date} />
                </span>
              )}
              {story.place && (
                <span className="row">
                  <MapPin size={14} />
                  {/* Places takes the place as written in the records ("Wólka, Łęczna"). */}
                  <button className="stories-place" onClick={() => go({ name: "places", place: story.place ?? undefined })}>
                    {story.place}
                  </button>
                </span>
              )}
            </div>
          )}
        </div>
      )}

      {story.people.length > 0 && (
        <div className="col" style={{ gap: 6 }}>
          <span className="label-caps">Występują</span>
          <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
            {story.people.map((p) => (
              <button key={p.id} className="stories-person" onClick={() => go({ name: "person", id: p.id })}>
                <Avatar initials={p.initials} branch={p.branch} size={24} />
                {p.name}
                {p.id === owner && <span className="stories-main">główna</span>}
              </button>
            ))}
          </div>
        </div>
      )}

      {story.body.trim() && <Markdown text={story.body} from={owner ?? undefined} className={quoteOnly ? "stories-quote" : "reading stories-body"} />}

      {story.photos.length > 0 && (
        <div className="stories-photos">
          {story.photos.map((p) => (
            <button key={p.id} className="stories-photo" title={p.caption ?? undefined} onClick={() => go({ name: "media", id: p.id })}>
              <Thumb path={p.path} icon={<FileX size={16} />} label="Brak pliku" style={{ width: "100%", height: "100%" }} />
            </button>
          ))}
        </div>
      )}

      {(story.sources.length > 0 || story.certainty || story.inferred) && (
        <div className="stories-provenance">
          <MessageSquareQuote size={16} color="var(--text2)" style={{ flex: "none" }} />
          <span className="grow">
            {story.sources.length === 0
              ? "Bez podanego źródła"
              : story.sources.map((s, i) => (
                  <Fragment key={s.id}>
                    {i > 0 && "; "}
                    <button className="stories-source" onClick={() => go({ name: "sources", id: s.id })}>
                      {s.title || "Źródło bez tytułu"}
                      <sup className="source-mark">[{i + 1}]</sup>
                    </button>
                  </Fragment>
                ))}
          </span>
          {story.certainty && <span className={`stories-pill${story.certainty === "high" ? " sure" : ""}`}>{CERTAINTY[story.certainty]}</span>}
          {story.inferred && <span className="stories-pill">wywnioskowane</span>}
        </div>
      )}

      {owner && (
        <div className="row" style={{ gap: 8 }}>
          <button className="btn primary" style={{ padding: "0 16px" }} onClick={() => go({ name: "person", id: owner })}>
            Przejdź do profilu
          </button>
          <button className="btn secondary" onClick={() => go({ name: "tree", view: "family", person: owner })}>
            <Network size={14} />
            Pokaż w drzewie
          </button>
        </div>
      )}
    </>
  );
}

function StoryForm({ story, onDone }: { story: StoryDetail; onDone: () => void }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [kind, setKind] = useState(story.kind);
  const [title, setTitle] = useState(story.title ?? "");
  const [date, setDate] = useState(story.date ?? "");
  const [place, setPlace] = useState(story.place ?? "");
  const [certainty, setCertainty] = useState<string>(story.certainty ?? "none");
  const [body, setBody] = useState(story.body);
  const [busy, setBusy] = useState(false);

  const save = () =>
    requireEdit(async () => {
      if (!body.trim()) {
        notify("Tekst jest pusty.", { kind: "err" });
        return;
      }
      setBusy(true);
      try {
        await call("text.save", {
          id: story.id,
          // The API needs the person only to link a new text; an existing one keeps its people.
          person: story.owner ?? "",
          kind,
          title,
          place,
          body,
          certainty: certainty === "none" ? null : certainty,
          // The date comes back in words ("12 marca 1878"); send it only when changed, so an untouched date keeps
          // its exact stored form.
          ...(date.trim() !== (story.date ?? "") ? { date } : {}),
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
    <div className="col stories-form">
      {story.kind in KINDS && (
        <Segmented
          size={28}
          value={kind}
          onChange={setKind}
          options={Object.entries(KINDS).map(([value, k]) => ({ value, label: k.label, icon: <k.icon size={13} /> }))}
        />
      )}
      <label className="field">
        <span className="field-label">Tytuł</span>
        <input className="input" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="np. Zima 1915: ucieczka przed frontem" />
      </label>
      <div className="row" style={{ gap: 8 }}>
        <label className="field grow">
          <span className="field-label">Kiedy</span>
          <input className="input sm" value={date} onChange={(e) => setDate(e.target.value)} placeholder="np. zima 1915, ok. 1920" />
        </label>
        <label className="field grow">
          <span className="field-label">Gdzie</span>
          <input className="input sm" value={place} onChange={(e) => setPlace(e.target.value)} placeholder="np. Lublin" />
        </label>
      </div>
      <div className="field">
        <span className="field-label">Pewność</span>
        <Segmented
          variant="neutral"
          size={28}
          value={certainty}
          onChange={setCertainty}
          options={[
            { value: "high", label: "pewne" },
            { value: "medium", label: "prawdopodobne" },
            { value: "low", label: "niepewne" },
            { value: "none", label: "nie określono" },
          ]}
        />
      </div>
      <label className="field">
        <span className="field-label">Treść</span>
        <textarea className="input" rows={12} value={body} onChange={(e) => setBody(e.target.value)} />
      </label>
      <span className="stories-hint">Wzmianki o osobach, np. [Józef](person:@I12@), zostaw w tej postaci — wtedy pozostaną linkami.</span>
      <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
        <button className="btn ghost" onClick={onDone}>
          Anuluj
        </button>
        <button className="btn primary" disabled={busy} onClick={save}>
          Zapisz
        </button>
      </div>
    </div>
  );
}
