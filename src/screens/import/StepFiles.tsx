// Import · 4 Zdjęcia i pliki (spec §4.13): every file of the batch with its status, kind, caption and people; the
// profile photo per person. Files are copied into media/ only when the import is confirmed.

import { ArrowRight, Check, CircleCheck, CircleHelp, Copy, ExternalLink, FileText, Image as ImageIcon, ImageOff, Plus, Star, TriangleAlert, Upload, UserPlus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { call, importFileUrl, type ApiError } from "../../api/transport";
import { useStore } from "../../app/store";
import { Dialog } from "../../components/Dialog";
import { Avatar, Spinner, useDismiss } from "../../components/bits";
import { count } from "../../lib/format";
import { openPath, pickFiles } from "../../lib/native";
import { Footer } from "./ImportWizard";
import { useWizard, type Act, type ImportFile, type ImportFileDetail, type ImportState, type Step } from "./types";

const STATUS: Record<ImportFile["status"], { label: string; icon: typeof Check; className: string }> = {
  ok: { label: "dopasowany", icon: Check, className: "accent" },
  dup: { label: "już w archiwum", icon: Copy, className: "plain" },
  miss: { label: "brak pliku", icon: TriangleAlert, className: "warn" },
  und: { label: "plik nieopisany", icon: CircleHelp, className: "warn" },
  skipped: { label: "pominięty", icon: Check, className: "outline" },
};

export function StepFiles({ state, act, goStep }: { state: ImportState; act: Act; goStep: (s: Step) => void }) {
  const highlight = useWizard((w) => w.file);
  const setWizard = useWizard((w) => w.set);
  const paths = useWizard((w) => w.paths);
  const [open, setOpen] = useState<string | null>(state.persons.find((p) => p.profile)?.id ?? null);
  const files = state.files;
  const n = (s: ImportFile["status"]) => files.filter((f) => f.status === s).length;
  const attention = n("miss") + n("und");
  const profiles = new Map(state.persons.filter((p) => p.profile).map((p) => [p.profile!, p]));
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
                        {profile?.index != null && profile.image ? <img src={importFileUrl(profile.index, 128, state.loadId)} alt="" /> : <ImageIcon size={18} />}
                      </span>
                      <span className="col" style={{ gap: 3, fontSize: 13 }}>
                        <span>
                          Zdjęcie profilowe: <b>{p.profile ?? "brak"}</b>
                        </span>
                        <ProfilePicker current={p.profile} images={images} onPick={(file) => act("import.profile", { person: p.id, file })} />
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
                <FileCard key={f.file} f={f} state={state} act={act} profileOf={profiles.get(f.file)?.name ?? null} highlighted={highlight === f.file} />
              ))}
            </div>
          )}
        </section>
      </div>
      <Footer status={<>Krok 4 z 5 · {attention > 0 ? <><b>{count(attention, "plik wymaga", "pliki wymagają", "plików wymaga")} uwagi</b> · można je pominąć</> : "pliki gotowe"}</>}>
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

function FileCard({ f, state, act, profileOf, highlighted }: { f: ImportFile; state: ImportState; act: Act; profileOf: string | null; highlighted: boolean }) {
  const [caption, setCaption] = useState(f.caption ?? "");
  const [reading, setReading] = useState(false);
  useEffect(() => setCaption(f.caption ?? ""), [f.caption]);
  const status = STATUS[f.status];
  const skipped = f.status === "skipped";
  const ext = (f.name ?? "").split(".").pop()?.toUpperCase();

  const attach = async () => {
    const [path] = await pickFiles(`Wskaż plik ${f.file}`, undefined, false);
    if (path) await act("import.attach", { file: f.file, path });
  };

  return (
    <div className={`imp-file-card${highlighted ? " highlighted" : ""}${skipped ? " skipped" : ""}`} data-file={f.file}>
      <div className={`imp-file-thumb${f.status === "miss" ? " missing" : ""}`}>
        {f.index != null && f.image ? (
          <img src={importFileUrl(f.index, 256, state.loadId)} alt="" loading="lazy" />
        ) : f.status === "miss" ? (
          <>
            <ImageOff size={22} />
            <span>Nie dołączono pliku</span>
          </>
        ) : (
          <>
            <FileText size={22} />
            <span>{ext}</span>
          </>
        )}
        <span className={`imp-file-status ${status.className}`}>
          <status.icon size={12} />
          {status.label}
        </span>
        {profileOf && (
          <span className="imp-file-profile" title={`Zdjęcie profilowe: ${profileOf}`}>
            <Star size={11} fill="currentColor" />
            profilowe
          </span>
        )}
      </div>
      <div className="imp-file-body">
        <div className="row" style={{ gap: 8 }}>
          <span className="imp-file-name ellipsis grow" title={f.name ?? f.file}>
            {f.name ?? f.file}
          </span>
          <select className="imp-kind" value={f.kind} disabled={skipped} onChange={(e) => act("import.file", { file: f.file, kind: e.target.value })}>
            <option value="photo">zdjęcie</option>
            <option value="document">dokument</option>
            <option value="other">inne</option>
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
          {f.people.map((p) => (
            <span key={p.id} className="imp-chip">
              {p.name ?? p.id}
            </span>
          ))}
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
  const ref = useDismiss<HTMLSpanElement>(open, () => setOpen(false));
  const on = new Set(f.people.map((p) => p.id));
  const others = state.persons.filter((p) => !on.has(p.id) && p.decision !== "skip");
  if (others.length === 0) return null;
  return (
    <span ref={ref} style={{ position: "relative" }}>
      <button className="link" onClick={() => setOpen((o) => !o)}>
        <UserPlus size={12} style={{ verticalAlign: -2 }} /> Przypisz
      </button>
      {open && (
        <div className="popover" style={{ bottom: 22, right: 0, width: 220, maxHeight: 260, overflow: "auto", padding: "4px 0" }}>
          {others.map((p) => (
            <button key={p.id} className="menu-item" onClick={() => (act("import.file", { file: f.file, people: [...on, p.id] }), setOpen(false))}>
              <span className="ellipsis">{p.name}</span>
              <span style={{ marginLeft: "auto", color: "var(--text3)", fontSize: 12 }}>{p.years}</span>
            </button>
          ))}
        </div>
      )}
    </span>
  );
}
