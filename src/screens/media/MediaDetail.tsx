import { ExternalLink, FileSearch, FileText, FileX, FolderOpen, Image as ImageIcon, Maximize2, Plus, Search, Star, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useState, type KeyboardEvent } from "react";
import { call } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { afterChange, useStore } from "../../app/store";
import { Avatar, Lifespan, Segmented, useDismiss } from "../../components/bits";
import { Dialog } from "../../components/Dialog";
import { count } from "../../lib/format";
import { pickFiles } from "../../lib/native";
import {
  extension,
  fileName,
  filtersLike,
  isImage,
  isPdf,
  MetaRows,
  openInProgram,
  runEdit,
  showInFolder,
  TextBlock,
  ThumbImage,
  type MediaItem,
  type MediaPerson,
} from "./shared";

/** Offered while typing a document type (DESIGNER_ANSWERS §4 „rodzaj”), together with the types already in use. */
const DOCUMENT_TYPES = ["akt urodzenia", "akt małżeństwa", "akt zgonu", "dokument osobisty", "list", "zrzut ekranu indeksu"];

/** The 320 px panel beside the grid (spec §4.30). In edit mode the caption, date, place and kind become fields, and
 *  people can be added, removed or given this photo as their profile photo. */
export function MediaDetail({
  item,
  documentTypes,
  onPreview,
  onDeleted,
}: {
  item: MediaItem | null;
  documentTypes: string[];
  onPreview: () => void;
  onDeleted: () => void;
}) {
  const editing = useStore((s) => s.mode === "edit");
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const [confirmDelete, setConfirmDelete] = useState(false);

  if (!item) {
    return (
      <aside className="media-panel">
        <div className="media-panel-empty">
          <ImageIcon size={24} />
          <span>Wybierz zdjęcie lub dokument, aby zobaczyć szczegóły.</span>
          {editing && <span>Nowe pliki możesz też przeciągnąć tu z Eksploratora.</span>}
        </div>
      </aside>
    );
  }

  const missing = item.missing || !item.path;
  const image = !missing && isImage(item.path);
  const pdf = !missing && isPdf(item.path);
  const canPreview = image || pdf;
  const relink = async () => {
    const [path] = await pickFiles(`Wskaż plik ${fileName(item.path)}`, filtersLike(fileName(item.path)), false);
    if (!path) return;
    runEdit(async () => {
      await call("media.relink", { id: item.id, path });
      afterChange();
      notify("Plik jest z powrotem w archiwum.");
    });
  };

  return (
    <aside className="media-panel">
      <button className={`media-preview${missing ? " missing" : ""}`} onClick={onPreview} disabled={!canPreview} title={canPreview ? "Otwórz podgląd" : undefined}>
        {missing ? (
          <>
            <FileX size={24} />
            <span>Brak pliku</span>
          </>
        ) : image && item.path ? (
          <ThumbImage path={item.path} size={512} iconSize={24} />
        ) : (
          <>
            <FileText size={24} />
            <span>{extension(item.path) || "Plik"}</span>
          </>
        )}
      </button>

      {editing ? (
        <EditFields item={item} documentTypes={documentTypes} />
      ) : (
        <div className="media-title">{item.title?.trim() || fileName(item.path) || "Bez podpisu"}</div>
      )}
      {item.note && <p style={{ fontSize: 14, lineHeight: 1.55, color: "var(--text2)", whiteSpace: "pre-wrap" }}>{item.note}</p>}
      <MetaRows item={item} hide={editing ? ["kind", "date", "place"] : []} />

      {missing && item.path && (
        <div className="banner warn" style={{ fontSize: 13, alignItems: "flex-start" }}>
          <FileX size={16} color="var(--warn)" style={{ flex: "none", marginTop: 1 }} />
          <span>Tego pliku nie ma w folderze archiwum. Ktoś mógł go przenieść poza programem.</span>
        </div>
      )}

      {(item.people.length > 0 || editing) && (
        <div className="media-section">
          <span className="label-caps">{item.kind === "photo" ? "Na zdjęciu" : "Osoby"}</span>
          {item.people.length > 0 && (
            <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
              {item.people.map((p) => (
                <PersonChip key={p.id} item={item} person={p} editing={editing} canBeProfile={image} />
              ))}
            </div>
          )}
          {editing && <PersonPicker item={item} />}
        </div>
      )}

      <div className="col" style={{ gap: 6 }}>
        {canPreview && (
          <button className="btn primary" onClick={onPreview}>
            <Maximize2 size={14} />
            Otwórz podgląd
          </button>
        )}
        {!missing && !image && (
          <button className={`btn ${pdf ? "secondary" : "primary"}`} onClick={() => openInProgram(item.absolute)} disabled={!item.absolute}>
            <ExternalLink size={14} />
            Otwórz w programie
          </button>
        )}
        {!missing && (
          <button className="btn secondary" onClick={() => showInFolder(item.absolute)} disabled={!item.absolute}>
            <FolderOpen size={14} />
            Pokaż w folderze
          </button>
        )}
        {missing && editing && (
          <button className="btn secondary" onClick={relink}>
            <FileSearch size={14} />
            Wskaż plik…
          </button>
        )}
        {missing && (
          <button className="btn ghost" onClick={() => go({ name: "missingFiles" })}>
            Wszystkie brakujące pliki
          </button>
        )}
      </div>

      {item.transcription && <TextBlock label="Transkrypcja" text={item.transcription} />}
      {item.translation && <TextBlock label="Tłumaczenie" text={item.translation} />}

      {editing && (
        <button className="btn ghost media-danger" style={{ alignSelf: "flex-start" }} onClick={() => setConfirmDelete(true)}>
          <Trash2 size={14} />
          Usuń z archiwum
        </button>
      )}
      {confirmDelete && <DeleteDialog item={item} onClose={() => setConfirmDelete(false)} onDeleted={onDeleted} />}
    </aside>
  );
}

function EditFields({ item, documentTypes }: { item: MediaItem; documentTypes: string[] }) {
  const saved = { title: item.title ?? "", date: item.date ?? "", place: item.place ?? "", kind: item.kind, documentType: item.documentType ?? "" };
  const savedKey = JSON.stringify(saved);
  const [form, setForm] = useState(saved);
  const [base, setBase] = useState(savedKey);
  // After a save, an undo or any other change, the fields show what the archive now says.
  if (base !== savedKey) {
    setBase(savedKey);
    setForm(saved);
  }
  const [busy, setBusy] = useState(false);
  const dirty = JSON.stringify(form) !== savedKey;
  const set = (patch: Partial<typeof saved>) => setForm((f) => ({ ...f, ...patch }));

  const save = () => {
    if (!dirty || busy) return;
    runEdit(async () => {
      const args: Record<string, string> = { id: item.id };
      if (form.title !== saved.title) args.title = form.title;
      if (form.date !== saved.date) args.date = form.date;
      if (form.place !== saved.place) args.place = form.place;
      // The API rewrites the kind and the document type together, so both are always sent.
      if (form.kind !== saved.kind || form.documentType !== saved.documentType) {
        args.kind = form.kind;
        args.documentType = form.kind === "document" ? form.documentType : "";
      }
      setBusy(true);
      try {
        await call("media.update", args);
      } finally {
        setBusy(false);
      }
      afterChange();
    });
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") save();
    else if (e.key === "Escape" && dirty) {
      e.stopPropagation();
      setForm(saved);
    }
  };
  const suggestions = [...new Set([...DOCUMENT_TYPES, ...documentTypes])];

  return (
    <div className="col" style={{ gap: 10 }}>
      <label className="field">
        <span className="field-label">Podpis</span>
        <input className="input sm" value={form.title} onChange={(e) => set({ title: e.target.value })} onKeyDown={keys} placeholder="Np. Józef i Marianna w dniu ślubu" />
      </label>
      <label className="field">
        <span className="field-label">Data</span>
        <input className="input sm" value={form.date} onChange={(e) => set({ date: e.target.value })} onKeyDown={keys} placeholder="Np. 14.02.1904 albo ok. 1925" />
      </label>
      <label className="field">
        <span className="field-label">Miejsce</span>
        <input className="input sm" value={form.place} onChange={(e) => set({ place: e.target.value })} onKeyDown={keys} placeholder="Np. Łęczna" />
      </label>
      <div className="field">
        <span className="field-label">Rodzaj</span>
        <Segmented
          variant="neutral"
          full
          value={form.kind}
          onChange={(kind) => set({ kind })}
          options={[
            { value: "photo", label: "Zdjęcie" },
            { value: "document", label: "Dokument" },
            { value: "other", label: "Inny" },
          ]}
        />
      </div>
      {form.kind === "document" && (
        <label className="field">
          <span className="field-label">Typ dokumentu</span>
          <input
            className="input sm"
            list="media-document-types"
            value={form.documentType}
            onChange={(e) => set({ documentType: e.target.value })}
            onKeyDown={keys}
            placeholder="Np. akt urodzenia"
          />
          <datalist id="media-document-types">
            {suggestions.map((t) => (
              <option key={t} value={t} />
            ))}
          </datalist>
        </label>
      )}
      {dirty && (
        <div className="row" style={{ gap: 8, justifyContent: "flex-end" }}>
          <button className="btn ghost sm" onClick={() => setForm(saved)}>
            Anuluj
          </button>
          <button className="btn primary sm" onClick={save} disabled={busy}>
            Zapisz
          </button>
        </div>
      )}
    </div>
  );
}

function PersonChip({ item, person, editing, canBeProfile }: { item: MediaItem; person: MediaPerson; editing: boolean; canBeProfile: boolean }) {
  const go = useStore((s) => s.go);
  const setAsk = useStore((s) => s.setAsk);
  const profile = item.profileOf.includes(person.id);
  const setProfile = () =>
    runEdit(async () => {
      await call("media.setProfile", { id: item.id, person: person.id });
      afterChange();
    });
  const unlink = () =>
    setAsk({
      title: `Odłączyć osobę: ${person.name}?`,
      text: "Plik zniknie z jej profilu, ale zostanie w Mediach. Do zapisu możesz to cofnąć (Ctrl Z).",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Odłącz",
          kind: "danger",
          run: () =>
            runEdit(async () => {
              await call("media.unlink", { id: item.id, person: person.id });
              afterChange();
            }),
        },
      ],
    });
  return (
    <span className={`media-person${editing ? " editing" : ""}`}>
      {editing && canBeProfile ? (
        <button
          className="tool"
          onClick={setProfile}
          disabled={profile}
          title={profile ? `Zdjęcie profilowe: ${person.name}` : `Ustaw jako zdjęcie profilowe: ${person.name}`}
        >
          <Star size={13} fill={profile ? "currentColor" : "none"} />
        </button>
      ) : (
        profile && (
          <span title="Zdjęcie profilowe" style={{ display: "inline-flex", marginRight: 2 }}>
            <Star size={12} fill="currentColor" />
          </span>
        )
      )}
      <button className="name" onClick={() => go({ name: "person", id: person.id })} title="Otwórz profil">
        {person.name}
      </button>
      {editing && (
        <button className="tool" onClick={unlink} title={`Odłącz: ${person.name}`}>
          <X size={13} />
        </button>
      )}
    </span>
  );
}

type Found = PersonSummary & { context: string };

/** „Dodaj do osoby”: a person search (`people.search`); ↑ ↓ Enter pick, Esc closes. It stays open for the next
 *  person, since group photos usually get several. */
function PersonPicker({ item }: { item: MediaItem }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Found[]>([]);
  const [active, setActive] = useState(0);
  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
  }, []);
  const ref = useDismiss<HTMLDivElement>(open, close);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults([]);
      return;
    }
    let stale = false;
    const timer = setTimeout(() => {
      call<Found[]>("people.search", { q, limit: 8 })
        .then((found) => {
          if (stale) return;
          setResults(found);
          setActive(0);
        })
        .catch(() => {});
    }, 120);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [query]);

  const linked = new Set(item.people.map((p) => p.id));
  const add = (person: Found) => {
    if (linked.has(person.id)) return;
    runEdit(async () => {
      await call("media.link", { id: item.id, person: person.id });
      afterChange();
      setQuery("");
    });
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(a + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(a - 1, 0));
    } else if (e.key === "Enter" && results[active]) {
      e.preventDefault();
      add(results[active]);
    }
  };

  if (!open) {
    return (
      <button className="chip dashed" style={{ height: 28, alignSelf: "flex-start" }} onClick={() => setOpen(true)}>
        <Plus size={13} />
        Dodaj do osoby
      </button>
    );
  }
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <label className="search-box">
        <Search size={15} />
        <input autoFocus value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={keys} placeholder="Szukaj osoby…" aria-label="Szukaj osoby" />
      </label>
      {query.trim() && (
        <div className="popover media-results">
          {results.length === 0 ? (
            <div style={{ padding: "10px 12px", fontSize: 13, color: "var(--text3)" }}>Nikogo nie znaleziono.</div>
          ) : (
            results.map((p, i) => {
              const dated = p.birth != null || p.death != null;
              return (
                <button
                  key={p.id}
                  className={`menu-item media-result${i === active ? " active" : ""}`}
                  disabled={linked.has(p.id)}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => add(p)}
                >
                  <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={28} />
                  <span className="col grow" style={{ minWidth: 0 }}>
                    <span className="ellipsis" style={{ fontSize: 14, fontWeight: 500 }}>
                      {p.name}
                    </span>
                    <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                      {linked.has(p.id) ? (
                        "już na liście"
                      ) : (
                        <>
                          {dated && <Lifespan birth={p.birth} death={p.death} living={p.living} />}
                          {dated && p.context ? " · " : ""}
                          {p.context}
                        </>
                      )}
                    </span>
                  </span>
                </button>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}

function DeleteDialog({ item, onClose, onDeleted }: { item: MediaItem; onClose: () => void; onDeleted: () => void }) {
  const notify = useStore((s) => s.notify);
  const undo = useStore((s) => s.undo);
  const [busy, setBusy] = useState(false);
  const name = item.title?.trim() || fileName(item.path) || "Ten plik";
  const people = item.people.length;
  const remove = () =>
    runEdit(async () => {
      setBusy(true);
      try {
        await call("media.delete", { id: item.id });
      } finally {
        setBusy(false);
      }
      onClose();
      onDeleted();
      afterChange();
      notify(`Usunięto „${name}” z archiwum.`, { action: { label: "Cofnij", run: () => void undo().catch(() => {}) } });
    });
  return (
    <Dialog width={460} onClose={onClose}>
      <div className="dialog-body" style={{ flexDirection: "row", gap: 14 }}>
        <div className="dialog-icon" style={{ flex: "none" }}>
          <Trash2 size={19} />
        </div>
        <div className="col" style={{ gap: 6, minWidth: 0 }}>
          <div className="dialog-title">Usunąć z archiwum?</div>
          <div className="dialog-text" style={{ overflowWrap: "anywhere" }}>
            „{name}” zniknie z Mediów
            {people > 0 && ` i z ${people === 1 ? "profilu" : "profili"} ${count(people, "osoby", "osób", "osób")}`}. Plik na dysku zostanie
            na swoim miejscu.
          </div>
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn danger" onClick={remove} disabled={busy}>
          <Trash2 size={15} />
          Usuń z archiwum
        </button>
      </div>
    </Dialog>
  );
}
