// Import · 4 Zdjęcia i pliki (spec §4.13): every file of the batch with its status, kind, caption and people; the
// profile photo per person. Files are copied into media/ only when the import is confirmed.

import { ArrowRight, Check, CircleCheck, CircleHelp, Copy, ExternalLink, FileText, Image as ImageIcon, ImageOff, Plus, Star, TriangleAlert, Upload, UserPlus } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { call, importFileUrl, type ApiError } from "../../api/transport";
import { useStore } from "../../app/store";
import { useCategories } from "../../app/categories";
import { CategoryDialog } from "../../components/CategoryDialog";
import { CropDialog } from "../../components/CropDialog";
import { cropStyle, type Crop } from "../../components/cropMath";
import { Dialog } from "../../components/Dialog";
import { Avatar, Spinner, useDismiss } from "../../components/bits";
import { ProfileStar, ProfileTag, type StarPerson } from "../../components/ProfileStar";
import { choiceOf, choiceValue, type Base, type Category } from "../../lib/categories";
import { count } from "../../lib/format";
import { openPath, pickFiles } from "../../lib/native";
import { Footer } from "./ImportWizard";
import { matchPeople, missingLabel, needAttention, notAttachedLabel, undescribedLabel } from "./fileWords";
import { useWizard, type Act, type ImportFile, type ImportFileDetail, type ImportState, type Step } from "./types";

/** The chip on a file's thumb; the missing and undescribed ones name the file by its current kind. */
const STATUS: Record<ImportFile["status"], { label: (kind: string) => string; icon: typeof Check; className: string }> = {
  ok: { label: () => "dopasowany", icon: Check, className: "accent" },
  dup: { label: () => "już w archiwum", icon: Copy, className: "plain" },
  miss: { label: missingLabel, icon: TriangleAlert, className: "warn" },
  und: { label: undescribedLabel, icon: CircleHelp, className: "warn" },
  skipped: { label: () => "pominięty", icon: Check, className: "outline" },
};

export function StepFiles({ state, act, goStep }: { state: ImportState; act: Act; goStep: (s: Step) => void }) {
  const highlight = useWizard((w) => w.file);
  const setWizard = useWizard((w) => w.set);
  const paths = useWizard((w) => w.paths);
  const categories = useCategories();
  const [open, setOpen] = useState<string | null>(state.persons.find((p) => p.profile)?.id ?? null);
  const [cropping, setCropping] = useState<string | null>(null);
  const files = state.files;
  const n = (s: ImportFile["status"]) => files.filter((f) => f.status === s).length;
  const attention = n("miss") + n("und");
  const cropPerson = state.persons.find((p) => p.id === cropping);
  const cropFile = cropPerson?.profile ? files.find((f) => f.file === cropPerson.profile) : undefined;
  const people = [...state.persons].filter((p) => p.decision !== "skip").sort((a, b) => b.files.length - a.files.length);

  // „Pokaż” in step 2 points at one file.
  const grid = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!highlight) return;
    grid.current?.querySelector(`[data-file="${CSS.escape(highlight)}"]`)?.scrollIntoView({ block: "center" });
    const timer = setTimeout(() => setWizard({ file: null }), 2500);
    return () => clearTimeout(timer);
  }, [highlight, setWizard]);

  const addFiles = async () => {
    const picked = await pickFiles("Dodaj pliki do paczki");
    if (picked.length === 0) return;
    setWizard({ paths: [...paths, ...picked.filter((p) => !paths.includes(p))] });
    await act("import.load", { paths: picked, add: true });
  };

  return (
    <>
      <div className="imp-files">
        <aside className="imp-people">
          <div className="imp-people-head">
            <span style={{ fontSize: 14, fontWeight: 600 }}>Osoby z paczki</span>
          </div>
          <div className="scroll grow">
            {people.map((p) => {
              const expanded = open === p.id;
              const profile = p.profile ? files.find((f) => f.file === p.profile) : undefined;
              const images = files.filter((f) => f.image && f.status !== "skipped" && f.people.some((x) => x.id === p.id));
              return (
                <div key={p.id} className={`imp-file-person${expanded ? " on" : ""}`}>
                  <button className="imp-file-person-row" onClick={() => setOpen(expanded ? null : p.id)}>
                    <Avatar initials={p.initials} size={32} dashed={p.decision === "new"} />
                    <span className="col grow" style={{ minWidth: 0, alignItems: "flex-start" }}>
                      <span className="ellipsis" style={{ fontSize: 14, fontWeight: 500, maxWidth: "100%" }}>
                        {p.name}
                      </span>
                      <span style={{ fontSize: 12, color: "var(--text3)" }}>{p.files.length > 0 ? count(p.files.length, "plik", "pliki", "plików") : "bez plików"}</span>
                    </span>
                    {p.profile && <Star size={14} color="var(--accent-text)" fill="currentColor" />}
                  </button>
                  {expanded && (
                    <div className="row" style={{ gap: 12, padding: "4px 16px 14px" }}>
                      <span className="imp-profile-preview">
                        {profile?.index != null && profile.image ? <ProfilePreview index={profile.index} loadId={state.loadId} crop={p.profileCrop} /> : <ImageIcon size={18} />}
                      </span>
                      <span className="col" style={{ gap: 3, fontSize: 13 }}>
                        <span>
                          Zdjęcie profilowe: <b>{p.profile ?? "brak"}</b>
                        </span>
                        <span className="row" style={{ gap: 12 }}>
                          <ProfilePicker current={p.profile} images={images} onPick={(file) => act("import.profile", { person: p.id, file })} />
                          {profile?.index != null && profile.image && (
                            <button className="link" onClick={() => setCropping(p.id)}>
                              Kadruj
                            </button>
                          )}
                        </span>
                      </span>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </aside>
        <section className="imp-files-main scroll">
          <div className="row" style={{ gap: 14, fontSize: 13, color: "var(--text2)", flexWrap: "wrap" }}>
            <b style={{ color: "var(--text)" }}>{count(files.length, "plik", "pliki", "plików")}</b>
            {n("ok") > 0 && (
              <span className="row" style={{ gap: 5 }}>
                <CircleCheck size={14} color="var(--accent-text)" />
                {count(n("ok"), "dopasowany", "dopasowane", "dopasowanych")}
              </span>
            )}
            {n("dup") > 0 && (
              <span className="row" style={{ gap: 5 }}>
                <Copy size={14} />
                {n("dup")} już w archiwum
              </span>
            )}
            {attention > 0 && (
              <span className="row" style={{ gap: 5 }}>
                <TriangleAlert size={14} color="var(--warn)" />
                {attention} do sprawdzenia
              </span>
            )}
            {n("skipped") > 0 && <span>{count(n("skipped"), "pominięty", "pominięte", "pominiętych")}</span>}
            <span className="grow" />
            <button className="btn dashed" style={{ height: 34, borderWidth: 1.5 }} onClick={addFiles}>
              <Upload size={14} />
              Dodaj brakujące pliki
            </button>
          </div>
          {files.length === 0 ? (
            <div style={{ fontSize: 13, color: "var(--text3)", padding: "24px 0" }}>W tej paczce nie ma plików. Możesz przejść dalej.</div>
          ) : (
            <div className="imp-file-grid" ref={grid}>
              {files.map((f) => (
                <FileCard key={f.file} f={f} state={state} act={act} categories={categories} profileOf={state.persons.filter((p) => p.profile === f.file).map((p) => p.id)} highlighted={highlight === f.file} />
              ))}
            </div>
          )}
        </section>
      </div>
      {cropPerson?.profile && cropFile?.index != null && (
        <CropDialog
          src={importFileUrl(cropFile.index, 0, state.loadId)}
          initial={cropPerson.profileCrop}
          title="Kadr zdjęcia profilowego"
          onSave={(crop) => {
            setCropping(null);
            void act("import.profile", { person: cropPerson.id, file: cropPerson.profile, crop });
          }}
          onClose={() => setCropping(null)}
        />
      )}
      <Footer status={<>Krok 4 z 5 · {attention > 0 ? <><b>{needAttention(files.filter((f) => f.status === "miss" || f.status === "und").map((f) => f.kind))} uwagi</b> · można je pominąć</> : "pliki gotowe"}</>}>
        <button className="btn secondary" onClick={() => goStep(3)}>
          Wstecz
        </button>
        <button className="btn primary" onClick={() => goStep(5)}>
          Dalej
          <ArrowRight size={15} />
        </button>
      </Footer>
    </>
  );
}

/** The 64 px round preview: the whole thumb, or only the crop when one was chosen (the thumb keeps the aspect ratio). */
function ProfilePreview({ index, loadId, crop }: { index: number; loadId: number; crop: Crop | null }) {
  const [aspect, setAspect] = useState<number | null>(null);
  if (!crop) return <img src={importFileUrl(index, 128, loadId)} alt="" />;
  // The box is 64 px with a 2 px border.
  const style = aspect ? cropStyle(crop, aspect, 60) : { visibility: "hidden" as const };
  return <img className="crop" src={importFileUrl(index, 256, loadId)} alt="" style={style} onLoad={(e) => setAspect(e.currentTarget.naturalWidth / e.currentTarget.naturalHeight)} />;
}

function ProfilePicker({ current, images, onPick }: { current: string | null; images: ImportFile[]; onPick: (file: string | null) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLSpanElement>(open, () => setOpen(false));
  if (images.length === 0 && !current) return <span style={{ color: "var(--text3)", fontSize: 12 }}>Brak zdjęć tej osoby w paczce</span>;
  return (
    <span ref={ref} style={{ position: "relative" }}>
      <button className="link" onClick={() => setOpen((o) => !o)}>
        Zmień
      </button>
      {open && (
        <div className="popover" style={{ top: 22, left: 0, width: 220, padding: "4px 0" }}>
          {images.map((f) => (
            <button key={f.file} className={`menu-item${f.file === current ? " on" : ""}`} onClick={() => (onPick(f.file), setOpen(false))}>
              <span className="ellipsis">
                {f.file} {f.caption && <span style={{ color: "var(--text3)" }}>· {f.caption}</span>}
              </span>
            </button>
          ))}
          <button className="menu-item" onClick={() => (onPick(null), setOpen(false))}>
            Bez zdjęcia profilowego
          </button>
        </div>
      )}
    </span>
  );
}

const ADD_CATEGORY = "__add";

function FileCard({
  f,
  state,
  act,
  categories,
  profileOf,
  highlighted,
}: {
  f: ImportFile;
  state: ImportState;
  act: Act;
  categories: Category[];
  /** Ids of the batch people whose profile photo this file is. */
  profileOf: string[];
  highlighted: boolean;
}) {
  const [caption, setCaption] = useState(f.caption ?? "");
  const [reading, setReading] = useState(false);
  const [adding, setAdding] = useState(false);
  useEffect(() => setCaption(f.caption ?? ""), [f.caption]);
  const status = STATUS[f.status];
  const starPeople: StarPerson[] = f.people.map((p) => ({ id: p.id, name: p.name ?? p.id }));
  const skipped = f.status === "skipped";
  const ext = (f.name ?? "").split(".").pop()?.toUpperCase();
  /** The kind (and own category) of the file; the plain kinds clear a category. */
  const setKind = (kind: Base, category: string | null) => act("import.file", { file: f.file, kind, ...(category ? { category } : {}) });
  const kinds: Category[] = f.category && !categories.some((c) => c.name === f.category) ? [...categories, { name: f.category, base: (f.kind as Base) ?? "other", count: 0, saved: false }] : categories;

  // The star list may unset some people and set others in one go: the two calls run one after the other.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const inOrder = (run: () => Promise<unknown>) => {
    queue.current = queue.current.then(run);
  };

  const attach = async () => {
    const [path] = await pickFiles(`Wskaż plik ${f.file}`, undefined, false);
    if (path) await act("import.attach", { file: f.file, path });
  };

  return (
    <div className={`imp-file-card${highlighted ? " highlighted" : ""}${skipped ? " skipped" : ""}`} data-file={f.file}>
      <div className={`imp-file-thumb has-profile-star${f.status === "miss" ? " missing" : ""}`}>
        {f.index != null && f.image ? (
          <img src={importFileUrl(f.index, 256, state.loadId)} alt="" loading="lazy" />
        ) : f.status === "miss" ? (
          <>
            <ImageOff size={22} />
            <span>{notAttachedLabel(f.kind)}</span>
          </>
        ) : (
          <>
            <FileText size={22} />
            <span>{ext}</span>
          </>
        )}
        <span className={`imp-file-status ${status.className}`}>
          <status.icon size={12} />
          {status.label(f.kind)}
        </span>
        {f.image && f.index != null && (starPeople.length === 0 && !skipped ? (
          <button type="button" className="profile-star outline" disabled title="Najpierw przypisz osobę" aria-label="Najpierw przypisz osobę">
            <Star size={11} />
          </button>
        ) : (
          <ProfileStar
            people={starPeople}
            profileOf={profileOf}
            editable={!skipped}
            onSet={(ids) => inOrder(() => act("import.profile", { persons: ids, file: f.file }))}
            onUnset={(ids) => inOrder(() => act("import.profile", { persons: ids, file: null }))}
          />
        ))}
      </div>
      <div className="imp-file-body">
        <div className="row" style={{ gap: 8 }}>
          <span className="imp-file-name ellipsis grow" title={f.name ?? f.file}>
            {f.name ?? f.file}
          </span>
          <select
            className="imp-kind"
            style={{ maxWidth: "45%" }}
            value={choiceValue(f)}
            disabled={skipped}
            onChange={(e) => {
              if (e.target.value === ADD_CATEGORY) {
                setAdding(true);
                return;
              }
              const choice = choiceOf(e.target.value, kinds);
              if (choice) setKind(choice.kind, choice.category);
            }}
          >
            <option value="photo">zdjęcie</option>
            <option value="document">dokument</option>
            <option value="other">inne</option>
            {kinds.map((c) => (
              <option key={c.name} value={`cat:${c.name}`}>
                {c.name}
              </option>
            ))}
            <option value={ADD_CATEGORY}>+ Dodaj kategorię…</option>
          </select>
        </div>
        {f.documentType && <span style={{ fontSize: 12, color: "var(--text3)", marginTop: -3 }}>{f.documentType}</span>}
        <input
          className="input sm"
          style={{ height: 32 }}
          value={caption}
          disabled={skipped}
          onChange={(e) => setCaption(e.target.value)}
          onBlur={() => caption !== (f.caption ?? "") && act("import.file", { file: f.file, caption })}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          placeholder="Dodaj podpis…"
        />
        <div className="row" style={{ gap: 6, flexWrap: "wrap", minHeight: 24 }}>
          {f.people.map((p) =>
            profileOf.includes(p.id) ? (
              <ProfileTag key={p.id} name={p.name ?? p.id} profile />
            ) : (
              <span key={p.id} className="imp-chip">
                {p.name ?? p.id}
              </span>
            ),
          )}
          {f.transcription && (
            <button className="imp-chip link" onClick={() => setReading(true)} title="Pokaż transkrypcję">
              transkrypcja
            </button>
          )}
          <span className="grow" />
          {skipped ? (
            <button className="link" onClick={() => act("import.file", { file: f.file, skip: false })}>
              Przywróć
            </button>
          ) : (
            <>
              {f.status === "miss" && (
                <button className="link" onClick={attach}>
                  <Plus size={12} style={{ verticalAlign: -2 }} /> Dodaj plik
                </button>
              )}
              {f.status !== "miss" && <AssignButton f={f} state={state} act={act} />}
              {(f.status === "miss" || f.status === "und") && (
                <button className="link muted" onClick={() => act("import.file", { file: f.file, skip: true })}>
                  Pomiń
                </button>
              )}
            </>
          )}
        </div>
      </div>
      {reading && <TranscriptionDialog f={f} loadId={state.loadId} onClose={() => setReading(false)} />}
      {adding && <CategoryDialog base={f.kind === "document" ? "document" : f.kind === "photo" ? "photo" : "other"} onClose={() => setAdding(false)} onAdded={(name, base) => setKind(base, name)} />}
    </div>
  );
}

/** The AI's transcription of a file next to the file itself, with the translation and the note (read-only: corrections
 *  are made in Media after the import). */
function TranscriptionDialog({ f, loadId, onClose }: { f: ImportFile; loadId: number; onClose: () => void }) {
  const notify = useStore((s) => s.notify);
  const [detail, setDetail] = useState<ImportFileDetail | null>(null);
  useEffect(() => {
    call<ImportFileDetail>("import.fileDetail", { file: f.file })
      .then(setDetail)
      .catch((e: ApiError) => {
        notify(e.message, { kind: "err" });
        onClose();
      });
    // Read once per file; `onClose` changes with every render of the card.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [f.file]);
  const meta = detail ? [detail.documentType, detail.date, detail.place].filter(Boolean).join(" · ") : "";
  return (
    <Dialog width={920} onClose={onClose} labelledBy="imp-transcription-title">
      <div className="dialog-body">
        <div className="col" style={{ gap: 2 }}>
          <div className="dialog-title" id="imp-transcription-title">
            {f.file} · {detail?.caption || f.name || "transkrypcja"}
          </div>
          {meta && <span style={{ fontSize: 13, color: "var(--text2)" }}>{meta}</span>}
        </div>
        <div className="imp-transcription">
          <div className="imp-transcription-file">
            {f.index != null && f.image ? (
              <img src={importFileUrl(f.index, 1024, loadId)} alt={f.name ?? f.file} />
            ) : (
              <span className="col" style={{ gap: 8, alignItems: "center", color: "var(--text2)", fontSize: 13 }}>
                <FileText size={28} />
                {f.name ?? f.file}
                {f.path && (
                  <button className="btn secondary sm" onClick={() => openPath(f.path!)}>
                    <ExternalLink size={13} />
                    Otwórz
                  </button>
                )}
              </span>
            )}
          </div>
          <div className="imp-transcription-text scroll">
            {!detail ? (
              <Spinner size={18} />
            ) : (
              <>
                {detail.transcription && <TextPart label="Transkrypcja" text={detail.transcription} />}
                {detail.translation && <TextPart label="Tłumaczenie" text={detail.translation} />}
                {detail.note && <TextPart label="Notatka" text={detail.note} />}
              </>
            )}
          </div>
        </div>
      </div>
      <div className="dialog-foot">
        <span style={{ flex: 1, fontSize: 12, color: "var(--text3)" }}>Poprawisz to w Mediach po imporcie.</span>
        <button className="btn primary" onClick={onClose}>
          Zamknij
        </button>
      </div>
    </Dialog>
  );
}

function TextPart({ label, text }: { label: string; text: string }) {
  return (
    <div className="col" style={{ gap: 6 }}>
      <span className="label-caps">{label}</span>
      <div className="serif selectable" style={{ fontSize: 15, lineHeight: 1.7, color: "var(--text2)", whiteSpace: "pre-wrap" }}>
        {text}
      </div>
    </div>
  );
}

function AssignButton({ f, state, act }: { f: ImportFile; state: ImportState; act: Act }) {
  const [open, setOpen] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const on = new Set(f.people.map((p) => p.id));
  const others = state.persons.filter((p) => !on.has(p.id) && p.decision !== "skip");
  if (others.length === 0) return null;
  return (
    <>
      <button ref={button} className="link" aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <UserPlus size={12} style={{ verticalAlign: -2 }} /> Przypisz
      </button>
      {open && (
        <AssignList
          anchor={button.current}
          people={others}
          onPick={(id) => {
            setOpen(false);
            act("import.file", { file: f.file, people: [...on, id] });
          }}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  );
}

const ASSIGN_W = 320;
const ASSIGN_LIST_MAX = 320;

/** The people to assign, in a portal over everything (the card would cut it off): under the link, above it when there
 *  is no room. A search field on top; ↑/↓ move, Enter picks the highlighted one (the first hit at first). Esc, a click
 *  outside and scrolling close it. */
function AssignList({ anchor, people, onPick, onClose }: { anchor: HTMLElement | null; people: ImportState["persons"]; onPick: (id: string) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [at, setAt] = useState<{ left: number; top: number }>({ left: -9999, top: 0 });
  const box = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const shown = useMemo(() => matchPeople(people, query), [people, query]);

  const [listMax, setListMax] = useState(ASSIGN_LIST_MAX);

  // Below the link whenever the whole popover fits there; else above if it fits; else on the side with more room, with
  // a shorter list (it scrolls). The height is the real one: the chrome around the list plus the list as tall as it
  // wants to be (up to the maximum) — not what the last placement left of it.
  useLayoutEffect(() => {
    const rect = anchor?.getBoundingClientRect();
    const el = box.current;
    const listEl = list.current;
    if (!rect || !el || !listEl) return;
    const chrome = el.offsetHeight - listEl.offsetHeight;
    const wanted = Math.min(ASSIGN_LIST_MAX, listEl.scrollHeight);
    const spaceBelow = window.innerHeight - 8 - (rect.bottom + 6);
    const spaceAbove = rect.top - 6 - 8;
    const full = chrome + wanted;
    const below = full <= spaceBelow || (full > spaceAbove && spaceBelow >= spaceAbove);
    const space = below ? spaceBelow : spaceAbove;
    const fitted = full <= space ? wanted : Math.max(96, Math.min(ASSIGN_LIST_MAX, space - chrome));
    const left = Math.max(8, Math.min(rect.right - ASSIGN_W, window.innerWidth - ASSIGN_W - 8));
    const top = below ? rect.bottom + 6 : Math.max(8, rect.top - 6 - (chrome + fitted));
    setListMax(fitted === wanted ? ASSIGN_LIST_MAX : fitted);
    setAt((p) => (p.left === left && p.top === top ? p : { left, top }));
  }, [anchor, shown.length]);
  useEffect(() => {
    const down = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!box.current?.contains(t) && !anchor?.contains(t)) onClose();
    };
    const scroll = (e: Event) => {
      if (!box.current?.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
      anchor?.focus();
    };
    window.addEventListener("mousedown", down);
    window.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", onClose);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("keydown", key, true);
    };
  }, [anchor, onClose]);
  useEffect(() => {
    list.current?.querySelector<HTMLElement>(".menu-item.active")?.scrollIntoView({ block: "nearest" });
  }, [active, shown]);

  const move = (to: number) => setActive(shown.length === 0 ? 0 : (to + shown.length) % shown.length);
  return createPortal(
    <div ref={box} className="popover imp-assign-pop" role="dialog" aria-label="Przypisz osobę" style={{ left: at.left, top: at.top, width: ASSIGN_W }} onPointerDown={(e) => e.stopPropagation()}>
      <input
        className="input sm imp-assign-search"
        autoFocus
        placeholder="zacznij pisać…"
        aria-label="Szukaj osoby"
        value={query}
        onChange={(e) => (setQuery(e.target.value), setActive(0))}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") (e.preventDefault(), move(active + 1));
          else if (e.key === "ArrowUp") (e.preventDefault(), move(active - 1));
          else if (e.key === "Enter") {
            e.preventDefault();
            const hit = shown[Math.min(active, shown.length - 1)];
            if (hit) onPick(hit.id);
          }
        }}
      />
      <div ref={list} className="imp-assign-list scroll" style={{ maxHeight: listMax }} role="listbox">
        {shown.length === 0 && <div className="imp-assign-none">Nikogo takiego w paczce.</div>}
        {shown.map((p, i) => (
          <button key={p.id} type="button" role="option" aria-selected={i === active} className={`menu-item${i === active ? " active" : ""}`} onMouseMove={() => i !== active && setActive(i)} onClick={() => onPick(p.id)}>
            <span className="ellipsis">{p.name}</span>
            <span style={{ marginLeft: "auto", color: "var(--text3)", fontSize: 12, whiteSpace: "nowrap" }}>{p.years}</span>
          </button>
        ))}
      </div>
    </div>,
    document.body,
  );
}
