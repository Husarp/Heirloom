// Import · 1 Wczytaj (spec §4.10, design 17d): paste the AI answer, or drop everything at once — answers, photos, PDFs
// and notes — and see what each file was recognised as. Nothing is saved here.

import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  CircleCheck,
  CircleX,
  ClipboardPaste,
  Copy,
  Download,
  FileJson,
  FileQuestion,
  Files,
  FileText,
  FolderDown,
  Image as ImageIcon,
  Plus,
  StickyNote,
  TriangleAlert,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { call } from "../../api/transport";
import { useStore } from "../../app/store";
import { useCategories } from "../../app/categories";
import { useApi } from "../../app/useApi";
import { Spinner, useDismiss } from "../../components/bits";
import { CategoryDialog } from "../../components/CategoryDialog";
import { BASE_LIKE, type Base, type Category } from "../../lib/categories";
import { bytes, clock, count, people, shortWhen } from "../../lib/format";
import { copyText, onFileDrop, pickFiles, pickFolders, pickSavePath, revealPath } from "../../lib/native";
import { GUIDE_STEPS, guideOpenByDefault } from "./aiGuide";
import { useWizard, type Act, type ImportInput, type ImportState, type PastImport } from "./types";

const SAMPLE = `Oto część 1 z 2 pliku importu.
\`\`\`json
{
  "format": "heirloom-import",
  "batch": "Nowakowie-Ciechanki-2026-10-01",
  "part": 1,
  "persons": [ … ],
  …
  "end": { "part": 1, "final": false, …, "marker": "END-HEIRLOOM-IMPORT" }
}
\`\`\``;

const KIND_LABEL: Record<ImportInput["kind"], string> = { answer: "Odpowiedź AI", photo: "Zdjęcie", document: "Dokument", note: "Notatka", other: "Nie rozpoznano" };

export function StepLoad({ state, setState, act, next }: { state: ImportState | null; setState: (s: ImportState | null) => void; act: Act; next: () => void }) {
  const text = useWizard((w) => w.text);
  const paths = useWizard((w) => w.paths);
  const exclude = useWizard((w) => w.exclude);
  const droppedAt = useWizard((w) => w.droppedAt);
  const set = useWizard((w) => w.set);
  const furthest = useWizard((w) => w.furthest);
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const [busy, setBusy] = useState(false);
  // Changed after the later steps were started: reading it again would drop their answers and decisions, so it waits
  // for „Wczytaj od nowa”.
  const [stale, setStale] = useState(false);
  const [over, setOver] = useState(false);
  const { data: history } = useApi<PastImport[]>("import.history");
  const own = useCategories();
  // „+ Dodaj kategorię…” in a file's kind menu: the file the new category is for.
  const [adding, setAdding] = useState<ImportInput | null>(null);
  // The guide is open at first while the archive has had no import; a click decides from then on.
  const [guide, setGuide] = useState<boolean | null>(null);
  const guideOpen = guide ?? guideOpenByDefault(history);

  // Reads what was pasted and dropped a moment after the last change. Coming back to this step with a batch already
  // loaded keeps it (and the decisions made in the later steps).
  const load = async () => {
    const { text, paths, exclude, kinds, categories } = useWizard.getState();
    if (!text.trim() && paths.length === 0) {
      if (state) {
        await call("import.cancel").catch(() => {});
        setState(null);
      }
      return;
    }
    setBusy(true);
    await act("import.load", { texts: [text], paths, exclude, kinds, categories });
    setBusy(false);
  };
  const first = useRef(true);
  useEffect(() => {
    const skip = first.current && state != null;
    first.current = false;
    if (skip) return;
    if (state && furthest > 1) {
      setStale(true);
      return;
    }
    const timer = setTimeout(load, 350);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [text, paths, exclude]);
  const reload = () =>
    setAsk({
      title: "Wczytać paczkę od nowa?",
      text: "Odpowiedzi na pytania i decyzje z kroków 2–5 zostaną wyczyszczone.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Wczytaj od nowa",
          kind: "danger",
          run: async () => {
            setStale(false);
            set({ furthest: 1 });
            await load();
          },
        },
      ],
    });

  /** „Wyczyść listę”: everything loaded so far goes (the archive isn't touched); it asks only when answers and
   *  decisions from the later steps would be lost. */
  const clearAll = () => {
    const run = async () => {
      setStale(false);
      await call("import.cancel").catch(() => {});
      useWizard.getState().reset();
      setState(null);
      notify("Lista wyczyszczona.");
    };
    if (furthest <= 1) {
      run();
      return;
    }
    setAsk({
      title: "Wyczyścić listę?",
      text: "Odpowiedzi i decyzje z kroków 2–5 przepadną. Archiwum się nie zmieni.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        { label: "Wyczyść listę", kind: "danger", run },
      ],
    });
  };

  const addPaths = (more: string[]) => {
    const { paths: current, exclude: out } = useWizard.getState();
    const fresh = more.filter((p) => !current.includes(p));
    // Dropped again: a file taken off the list inside it comes back.
    const under = (x: string) => more.some((p) => x === p || x.startsWith(`${p}\\`) || x.startsWith(`${p}/`));
    const kept = out.filter((x) => !under(x));
    if (fresh.length || kept.length !== out.length) set({ paths: [...current, ...fresh], exclude: kept, droppedAt: new Date().toISOString() });
  };

  // Files dropped onto the window (the app gives real paths; a browser doesn't).
  useEffect(() => {
    let off: (() => void) | null = null;
    let gone = false;
    onFileDrop(addPaths, setOver).then((unlisten) => {
      if (gone) unlisten();
      else off = unlisten;
    });
    return () => {
      gone = true;
      off?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Another pasted answer goes after the ones already there. */
  const appendText = (pasted: string) => {
    const current = useWizard.getState().text;
    set({ text: current.trim() ? `${current}\n\n${pasted}` : pasted });
  };

  // With the list shown, the answer's text can still be pasted anywhere on the step (Ctrl V).
  const listed = paths.length > 0;
  useEffect(() => {
    if (!listed) return;
    const onPaste = (e: ClipboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target && (target.isContentEditable || ["INPUT", "TEXTAREA"].includes(target.tagName))) return;
      const pasted = e.clipboardData?.getData("text") ?? "";
      if (!pasted.trim()) return;
      e.preventDefault();
      appendText(pasted);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [listed, set]);

  const copyInstructions = async () => {
    try {
      const { text: instructions, version } = await call<{ text: string; version: string }>("import.instructions");
      if (await copyText(instructions)) notify(`Skopiowano instrukcję dla AI (format ${version}). Przekaż ją osobie, która szuka w aktach — w środku jest opisane, którą część wkleić do czatu.`);
      else notify("Nie udało się skopiować do schowka.", { kind: "err" });
    } catch {
      notify("Nie udało się wczytać instrukcji.", { kind: "err" });
    }
  };

  const pickMany = () => pickFiles("Wybierz pliki").then(addPaths);

  /** „Pobierz .txt / .md”: the AI instructions as a file to keep or send on. */
  const saveInstructions = async (ext: "txt" | "md") => {
    try {
      const path = await pickSavePath(`Instrukcja dla AI (Heirloom).${ext}`, [ext === "txt" ? { name: "Tekst", extensions: ["txt"] } : { name: "Markdown", extensions: ["md"] }]);
      if (!path) return;
      await call("import.saveInstructions", { path });
      notify("Zapisano instrukcję dla AI.", { action: { label: "Pokaż w folderze", run: () => void revealPath(path).catch(() => {}) } });
    } catch (e) {
      notify((e as { message?: string }).message ?? "Nie udało się zapisać instrukcji.", { kind: "err" });
    }
  };

  /** Takes a line off the list: the pasted text, a dropped file or folder, or a file from inside a folder. */
  const remove = (input: ImportInput) => {
    if (input.kind === "answer" && !input.path) {
      set({ text: "" });
      return;
    }
    if (!input.path) return;
    const { paths: current, exclude: out } = useWizard.getState();
    if (current.includes(input.path)) set({ paths: current.filter((p) => p !== input.path) });
    else set({ exclude: [...out, input.path] });
  };

  /** Gives a file the kind (and own category) chosen in its menu. Step 1's choices go with every reload of the list. */
  const chooseKind = (input: ImportInput, kind: Base | "note", category: string | null) => {
    if (input.path) {
      const { kinds, categories } = useWizard.getState();
      const rest = { ...categories };
      if (category) rest[input.path] = category;
      else delete rest[input.path];
      set({ kinds: { ...kinds, [input.path]: kind }, categories: rest });
    }
    void act("import.file", { file: input.file, kind, ...(category ? { category } : {}) });
  };

  const counts = state?.batch.counts;
  const firstError = state?.issues.find((i) => i.level === "error");
  const recognised = counts ? counts.persons > 0 || state!.files.length > 0 : false;
  const parts = state && state.batch.parts.length > 0 && (state.batch.totalParts ?? 0) > 1 ? `, część ${state.batch.parts.join(", ")} z ${state.batch.totalParts}` : "";
  const summary =
    state && counts
      ? [
          counts.persons > 0 && people(counts.persons),
          counts.events > 0 && count(counts.events, "fakt", "fakty", "faktów"),
          state.questions.length > 0 && count(state.questions.length, "pytanie", "pytania", "pytań"),
          counts.files > 0 && count(counts.files, "plik", "pliki", "plików"),
        ]
          .filter(Boolean)
          .join(", ") + parts
      : "";
  const inputs = state?.inputs ?? [];
  const toAssign = inputs.filter((i) => i.status === "assign").length;

  const checkButton = stale ? (
    <button className="btn primary" disabled={busy} onClick={reload} title="Tekst albo pliki zmieniły się po rozpoczęciu importu">
      Wczytaj od nowa
    </button>
  ) : (
    <button className="btn primary" disabled={!recognised || busy} onClick={next}>
      Sprawdź
      <ArrowRight size={15} />
    </button>
  );

  return (
    <div className="imp-body imp-load">
      {listed ? (
        <div className="imp-recog">
          <div className="imp-recog-top">
            <span className="icon-tile" style={{ width: 40, height: 40 }}>
              <Files size={20} />
            </span>
            <span className="col grow imp-recog-title" style={{ minWidth: 0 }}>
              <span className="serif" style={{ fontSize: 22, fontWeight: 600, lineHeight: 1.2, whiteSpace: "nowrap" }}>
                Rozpoznane pliki <span style={{ fontSize: 16, color: "var(--text2)", fontWeight: 400 }}>{inputs.length}</span>
              </span>
              <span style={{ fontSize: 13, color: "var(--text2)" }}>
                {droppedAt ? `Upuszczone o ${clock(droppedAt)} · ` : ""}nic nie zostało jeszcze zapisane
              </span>
            </span>
            <span className="imp-recog-actions">
              <button className="btn ghost" onClick={clearAll}>
                Wyczyść listę
              </button>
              <button className="btn secondary" onClick={() => pickFolders("Dodaj foldery").then(addPaths)}>
                <Plus size={14} />
                Dodaj folder
              </button>
              <button className="btn secondary" onClick={() => pickFiles("Dodaj pliki").then(addPaths)}>
                <Plus size={14} />
                Dodaj pliki
              </button>
            </span>
          </div>
          <div className="imp-recog-row head">
            <span>Plik</span>
            <span>Rodzaj</span>
            <span>Część · osoba</span>
            <span>Stan</span>
            <span />
          </div>
          <div className="imp-recog-list">
            {busy && inputs.length === 0 && (
              <div className="row" style={{ gap: 8, padding: "14px 20px", color: "var(--text3)", fontSize: 13 }}>
                <Spinner size={14} /> Czytam pliki…
              </div>
            )}
            {inputs.map((input, k) => (
              <InputRow key={`${input.path ?? input.name}-${k}`} input={input} categories={own} onChoose={chooseKind} onAddCategory={() => setAdding(input)} onRemove={() => remove(input)} />
            ))}
          </div>
          {/* Stays after the list appears, so it's clear that more answers can still be pasted. The textarea is
              invisible and covers the box: it only takes the paste, which goes into the text and is not kept here. */}
          <label className="imp-paste-mini">
            <ClipboardPaste size={18} />
            <span>Wklej tutaj kolejną odpowiedź AI (Ctrl V)</span>
            <textarea
              aria-label="Wklej tutaj kolejną odpowiedź AI"
              value=""
              spellCheck={false}
              onChange={() => {}}
              onPaste={(e) => {
                e.preventDefault();
                const pasted = e.clipboardData.getData("text");
                if (pasted.trim()) appendText(pasted);
              }}
            />
          </label>
          <div className="imp-recog-foot">
            <span className="grow" />
            {busy ? (
              <span className="row" style={{ gap: 7, color: "var(--text3)" }}>
                <Spinner size={14} />
                Czytam…
              </span>
            ) : stale ? (
              <span className="row" style={{ gap: 7, color: "var(--warn)", fontWeight: 500 }}>
                <TriangleAlert size={15} style={{ flex: "none" }} />
                Pliki zmieniły się po rozpoczęciu importu.
              </span>
            ) : (
              <>
                {(state?.errors ?? 0) > 0 && (
                  <span style={{ color: "var(--err)", fontWeight: 500 }} title="Szczegóły pokaże krok Sprawdź">
                    {count(state!.errors, "błąd", "błędy", "błędów")} w paczce
                  </span>
                )}
                {toAssign > 0 && <span style={{ color: "var(--warn)", fontWeight: 500 }}>{count(toAssign, "plik do wskazania", "pliki do wskazania", "plików do wskazania")}</span>}
              </>
            )}
            {checkButton}
          </div>
        </div>
      ) : (
        <div className="imp-paste">
          <div className="row" style={{ gap: 12 }}>
            <span className="icon-tile" style={{ width: 40, height: 40 }}>
              <ClipboardPaste size={20} />
            </span>
            <span className="col" style={{ gap: 2 }}>
              <span className="serif" style={{ fontSize: 22, fontWeight: 600 }}>
                Wklej odpowiedź AI
              </span>
              <span style={{ fontSize: 13, color: "var(--text2)" }}>Skopiuj całą odpowiedź z czatu i wklej tutaj. Nic nie musisz poprawiać.</span>
            </span>
          </div>
          <textarea className="imp-paste-area" value={text} onChange={(e) => set({ text: e.target.value })} placeholder={SAMPLE} spellCheck={false} />
          <div className="row" style={{ gap: 10, minHeight: 36 }}>
            {busy ? (
              <span className="row" style={{ gap: 7, color: "var(--text3)", fontSize: 13 }}>
                <Spinner size={14} />
                Czytam…
              </span>
            ) : stale ? (
              <span className="row" style={{ gap: 7, color: "var(--warn)", fontSize: 13, fontWeight: 500 }}>
                <TriangleAlert size={15} style={{ flex: "none" }} />
                Tekst albo pliki zmieniły się po rozpoczęciu importu.
              </span>
            ) : firstError ? (
              <span className="row" style={{ gap: 7, color: "var(--err)", fontSize: 13, fontWeight: 500 }}>
                <CircleX size={15} style={{ flex: "none" }} />
                {firstError.message}
              </span>
            ) : recognised ? (
              <span className="row" style={{ gap: 7, color: "var(--accent-text)", fontSize: 13, fontWeight: 500 }}>
                <CircleCheck size={15} style={{ flex: "none" }} />
                Rozpoznano: {summary}
              </span>
            ) : (
              <span style={{ color: "var(--text3)", fontSize: 13 }}>Na tym etapie nic się nie zapisuje — zmiany zatwierdzasz w podsumowaniu.</span>
            )}
            <span className="grow" />
            {checkButton}
          </div>
        </div>
      )}

      <div className="col" style={{ gap: 16 }}>
        {/* The whole box opens the file picker (several files at once); one Windows dialog can't pick files and folders
            together, so a folder has its own link. */}
        <div
          className={`imp-drop${over ? " over" : ""}`}
          role="button"
          tabIndex={0}
          aria-label="Wybierz pliki"
          onClick={pickMany}
          onKeyDown={(e) => {
            if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
            e.preventDefault();
            pickMany();
          }}
        >
          <FolderDown size={26} color="var(--text2)" />
          <span style={{ fontSize: 15, fontWeight: 600 }}>Upuść wszystko naraz</span>
          <span style={{ fontSize: 13, lineHeight: 1.5, color: "var(--text2)", textAlign: "center" }}>
            Odpowiedzi AI (.json lub tekst), zdjęcia, PDF-y i notatki .txt. Heirloom rozpozna każdy plik i pokaże listę.
          </span>
          <span className="row" style={{ gap: 6, fontSize: 13, color: "var(--text2)" }}>
            Kliknij, aby wybrać pliki ·
            <button
              className="link"
              onClick={(e) => {
                e.stopPropagation();
                pickFolders("Wybierz foldery").then(addPaths);
              }}
            >
              lub foldery…
            </button>
          </span>
        </div>

        <AiGuide open={guideOpen} onToggle={() => setGuide(!guideOpen)} onCopy={copyInstructions} onSave={saveInstructions} />

        <div className="card">
          <div className="imp-list-label">Poprzednie importy</div>
          {history && history.length > 0 ? (
            history.slice(0, 6).map((h) => (
              <div key={h.name} className="imp-past-row">
                <span className="ellipsis grow" style={{ fontWeight: 500 }}>
                  {h.name}
                </span>
                {h.undone && <span style={{ color: "var(--text3)" }}>cofnięty</span>}
                <span style={{ color: "var(--text3)" }}>{shortWhen(h.ts)}</span>
                <span style={{ color: "var(--text2)", minWidth: 64, textAlign: "right" }}>{h.people > 0 ? people(h.people) : count(h.files, "plik", "pliki", "plików")}</span>
              </div>
            ))
          ) : (
            <div className="imp-past-row" style={{ color: "var(--text3)" }}>
              {history ? "Jeszcze nic nie zaimportowano." : "…"}
            </div>
          )}
        </div>
      </div>
      {adding && (
        <CategoryDialog
          base={adding.kind === "photo" || adding.kind === "document" ? adding.kind : "other"}
          onClose={() => setAdding(null)}
          onAdded={(name, base) => chooseKind(adding, base, name)}
        />
      )}
    </div>
  );
}

/** „Jak przygotować paczkę z pomocą AI”: the steps from the researcher's material to this screen, with the button
 *  that copies the instructions. Folds away once imports are routine. */
function AiGuide({ open, onToggle, onCopy, onSave }: { open: boolean; onToggle: () => void; onCopy: () => void; onSave: (ext: "txt" | "md") => void }) {
  return (
    <div className="imp-info">
      <button className="imp-guide-head" aria-expanded={open} onClick={onToggle}>
        {open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
        <span style={{ fontSize: 14, fontWeight: 600 }}>Jak przygotować paczkę z pomocą AI</span>
      </button>
      {open ? (
        <ol className="imp-guide">
          {GUIDE_STEPS.map((step) => (
            <li key={step.title}>
              <span>
                <b>{step.title}</b> {step.text}
              </span>
              {step.copy && (
                <span className="row" style={{ gap: 8, alignSelf: "flex-start", marginTop: 6, flexWrap: "wrap" }}>
                  <CopyButtons onCopy={onCopy} onSave={onSave} />
                </span>
              )}
            </li>
          ))}
        </ol>
      ) : (
        <>
          <span style={{ color: "var(--text2)" }}>Instrukcja mówi AI, w jakim formacie przygotować osoby, fakty i źródła.</span>
          <span className="row" style={{ gap: 8, alignSelf: "flex-start", marginTop: 2, flexWrap: "wrap" }}>
            <CopyButtons onCopy={onCopy} onSave={onSave} />
          </span>
        </>
      )}
    </div>
  );
}

function CopyButtons({ onCopy, onSave }: { onCopy: () => void; onSave: (ext: "txt" | "md") => void }) {
  return (
    <>
      <button className="btn secondary" style={{ height: 34 }} onClick={onCopy}>
        <Copy size={14} />
        Kopiuj instrukcję dla AI
      </button>
      <button className="btn secondary" style={{ height: 34 }} onClick={() => onSave("txt")} title="Zapisz instrukcję jako plik tekstowy (Notatnik)">
        <Download size={14} />
        Pobierz .txt
      </button>
      <button className="btn secondary" style={{ height: 34 }} onClick={() => onSave("md")} title="Zapisz instrukcję jako plik Markdown">
        <Download size={14} />
        Pobierz .md
      </button>
    </>
  );
}

function kindIcon(input: ImportInput) {
  if (input.kind === "answer") return input.name.toLowerCase().endsWith(".json") ? <FileJson size={16} /> : <FileText size={16} />;
  if (input.kind === "photo") return <ImageIcon size={16} />;
  if (input.kind === "document") return <FileText size={16} />;
  if (input.kind === "note") return <StickyNote size={16} />;
  return <FileQuestion size={16} />;
}

const STATUS: Record<ImportInput["status"], { label: string; icon: React.ReactNode; className: string }> = {
  ok: { label: "Rozpoznano", icon: <CircleCheck size={14} />, className: "ok" },
  assign: { label: "Do wskazania", icon: <CircleAlert size={14} />, className: "assign" },
  skipped: { label: "Pominięty", icon: <Copy size={14} />, className: "skipped" },
  error: { label: "Nie do odczytu", icon: <CircleX size={14} />, className: "error" },
};

/** One recognised file: its kind (a file's can be changed), what it holds or whom it shows, its state, „usuń”. */
function InputRow({
  input,
  categories,
  onChoose,
  onAddCategory,
  onRemove,
}: {
  input: ImportInput;
  categories: Category[];
  onChoose: (input: ImportInput, kind: Base | "note", category: string | null) => void;
  onAddCategory: () => void;
  onRemove: () => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const status = STATUS[input.status];
  const changeable = input.kind !== "answer" && input.status !== "skipped" && !!input.file;
  const size = input.size != null ? bytes(input.size) : "";
  const choose = (kind: Base | "note", category: string | null) => {
    setOpen(false);
    onChoose(input, kind, category);
  };
  return (
    <div className={`imp-recog-row${input.status === "skipped" ? " skipped" : ""}`}>
      <span className="row" style={{ gap: 8, minWidth: 0 }}>
        <span style={{ color: "var(--text2)", display: "flex", flex: "none" }}>{kindIcon(input)}</span>
        <span className="col" style={{ minWidth: 0, lineHeight: 1.3 }}>
          <span className="ellipsis" style={{ fontWeight: 500 }} title={input.path ?? undefined}>
            {input.name}
          </span>
          <span style={{ fontSize: 12, color: "var(--text2)" }}>{size}</span>
        </span>
      </span>
      <div ref={ref} style={{ position: "relative", minWidth: 0 }}>
        <button
          className={`imp-kind${input.kind === "other" && !input.category ? " warn" : ""}`}
          disabled={!changeable}
          onClick={() => setOpen((o) => !o)}
          aria-haspopup={changeable ? "menu" : undefined}
        >
          <span className="ellipsis">{input.category ?? KIND_LABEL[input.kind]}</span>
          {changeable && <ChevronDown size={13} />}
        </button>
        {open && (
          <div className="popover" style={{ top: 36, left: 0, width: 190, maxHeight: 300, overflow: "auto", padding: "4px 0" }}>
            {(["photo", "document", "note"] as const).map((k) => (
              <button key={k} className={`menu-item${input.kind === k && !input.category ? " on" : ""}`} onClick={() => choose(k, null)}>
                {KIND_LABEL[k]}
              </button>
            ))}
            {categories.map((c) => (
              <button key={c.name} className={`menu-item${input.category === c.name ? " on" : ""}`} title={`${c.name} (${BASE_LIKE[c.base]})`} onClick={() => choose(c.base, c.name)}>
                <span className="ellipsis">{c.name}</span>
              </button>
            ))}
            <button
              className="menu-item"
              onClick={() => {
                setOpen(false);
                onAddCategory();
              }}
            >
              <Plus size={13} />
              Dodaj kategorię…
            </button>
          </div>
        )}
      </div>
      <span className={`imp-recog-detail ${status.className}`} title={input.detail}>
        {input.detail}
      </span>
      <span className={`row imp-recog-status ${status.className}`}>
        {status.icon}
        {status.label}
      </span>
      <button className="icon-btn" style={{ width: 32, height: 32 }} title="Usuń z listy" aria-label={`Usuń z listy: ${input.name}`} onClick={onRemove}>
        <X size={15} />
      </button>
    </div>
  );
}
