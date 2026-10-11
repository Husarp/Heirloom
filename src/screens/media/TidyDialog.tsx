// „Porządek w zdjęciach” (DESIGN B.5): what lies in media/ out of place or unknown to Heirloom, one calm section for
// each kind. Nothing is assigned or moved by itself: a suggestion is only prefilled, moves go through the
// final-change dialog, and every change to the family data asks „Kto edytuje?” first.

import { CircleCheck, FileQuestion } from "lucide-react";
import { useEffect, useState } from "react";
import { call, type ApiError } from "../../api/transport";
import type { LayoutPerson, MediaScan, PersonSummary, ScanFolder, ScanUnknown } from "../../api/types";
import { moveFiles, previewMove, scanSoon, tidyNotNow } from "../../app/layout";
import { useStore } from "../../app/store";
import { Dialog } from "../../components/Dialog";
import { Checkbox, Thumb } from "../../components/bits";
import { count, num } from "../../lib/format";
import {
  A1_NOTE,
  TIDY_TITLE,
  UNKNOWN_FOOTER,
  filesWord,
  FOLDERS_ONE_BY_ONE,
  looseSection,
  moreFoldersQuestion,
  movedSection,
  outsideSection,
  personLabel,
  tidyEmpty,
  tidySections,
  unknownSection,
  unpackQuestion,
  whoseQuestion,
  yearsOf,
} from "./layoutText";

export function TidyDialog() {
  const open = useStore((s) => s.tidyOpen);
  const scan = useStore((s) => s.scan);
  if (!open || !scan) return null;
  return <Body scan={scan} />;
}

/** One file's choice in the list of unknown files. */
type Choice = { kind: "person"; person: LayoutPerson } | { kind: "leave" };

const MAX_ROWS = 200;

const done = (e: unknown) => useStore.getState().notify((e as ApiError).message, { kind: "err" });

/** The person's name and years for a picked id (the people search gives the id only). */
async function personOf(id: string): Promise<LayoutPerson | null> {
  try {
    const p = await call<PersonSummary>("person.hover", { id });
    return { id, name: p.name, years: yearsOf(p.birth?.year, p.death?.year, p.living) };
  } catch {
    return null;
  }
}

/** Opens the people search; resolves with the person chosen, or null when it was closed without one. */
function pickPerson(placeholder: string): Promise<LayoutPerson | null> {
  return new Promise((resolve) => {
    let picked = false;
    useStore.getState().setPalette(true, {
      scope: "people",
      placeholder,
      pick: (id) => {
        picked = true;
        void personOf(id).then(resolve);
      },
    });
    // Closed without choosing: the palette clears its options.
    const off = useStore.subscribe((s) => {
      if (!s.paletteOpen) {
        off();
        setTimeout(() => !picked && resolve(null), 0);
      }
    });
  });
}

function Body({ scan }: { scan: MediaScan }) {
  const archive = useStore((s) => s.archive);
  const close = () => useStore.getState().setTidyOpen(false);
  // Esc belongs to the window on top (the people search, a question, the final-change dialog), not to this one below it;
  // Dialog's own Esc would swallow it for them, so it is handled here.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      if (e.key !== "Escape" || s.paletteOpen || s.ask || s.finalChange || s.whoEditsOpen || s.busy) return;
      e.stopPropagation();
      close();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
  const overlay = archive?.workMode === "overlay";
  const [dismissed, setDismissed] = useState<ReadonlySet<string>>(new Set());
  const sections = tidySections(scan, overlay, dismissed);
  const dismiss = (key: string) => setDismissed((d) => new Set(d).add(key));
  const empty = tidyEmpty(sections);

  const notNow = () => {
    close();
    void tidyNotNow(scan);
  };

  return (
    <Dialog width={760} labelledBy="tidy-title">
      <div className="dialog-body" style={{ gap: 6, paddingBottom: 6, flex: "none" }}>
        <div id="tidy-title" className="dialog-title">
          {TIDY_TITLE}
        </div>
        <div className="dialog-text">Nic nie zmieni się samo — każdą rzecz możesz zostawić, jak jest.</div>
      </div>
      <div className="col scroll" style={{ gap: 12, padding: "10px 24px 16px", overflow: "auto", minHeight: 0 }}>
        {overlay && <div className="banner info">{A1_NOTE}</div>}
        {empty && (
          <div className="row" style={{ gap: 10, color: "var(--text2)", fontSize: 14, padding: "12px 0" }}>
            <CircleCheck size={20} color="var(--accent-text)" />
            Wszystko jest na swoim miejscu.
          </div>
        )}
        {sections.moved && <MovedSection scan={scan} />}
        {sections.loose && <LooseSection scan={scan} onLeave={() => dismiss("loose")} />}
        {sections.unknown.length > 0 && <UnknownSection files={sections.unknown} />}
        {sections.unpack.slice(0, FOLDERS_ONE_BY_ONE).map((f) => (
          <UnpackSection key={f.path} folder={f} />
        ))}
        {sections.unpack.length > FOLDERS_ONE_BY_ONE && <UnpackMany folders={sections.unpack.slice(FOLDERS_ONE_BY_ONE)} />}
        {sections.whose.map((f) => (
          <WhoseSection key={f.path} folder={f} files={scan.unknown.filter((u) => u.folder === f.path)} onLeave={() => dismiss(f.path)} />
        ))}
        {sections.outside && <OutsideSection scan={scan} />}
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={notNow}>
          Nie teraz
        </button>
        <span className="grow" />
        <button className="btn primary" onClick={close}>
          Gotowe
        </button>
      </div>
    </Dialog>
  );
}

function Block({ children }: { children: React.ReactNode }) {
  return (
    <div className="col" style={{ gap: 10, padding: "12px 14px", border: "1px solid var(--border)", borderRadius: "var(--r-ctl)", background: "var(--surface)" }}>
      {children}
    </div>
  );
}

const Text = ({ children }: { children: React.ReactNode }) => <div style={{ fontSize: 14, lineHeight: 1.5, color: "var(--text)" }}>{children}</div>;

// ---- moved ----

function MovedSection({ scan }: { scan: MediaScan }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const n = scan.moved.length;
  const fix = () =>
    requireEdit(async () => {
      try {
        const r = await call<{ fixed: number }>("media.fixMoved");
        changed();
        scanSoon(0);
        notify(`Poprawiono ścieżki: ${filesWord(r.fixed)}.`, { detail: "Zmiana czeka na zapis." });
      } catch (e) {
        done(e);
      }
    });
  return (
    <Block>
      <Text>{movedSection(n)}</Text>
      <div className="row">
        <button className="btn secondary sm" onClick={fix}>
          Popraw ścieżki
        </button>
      </div>
    </Block>
  );
}

// ---- B2: loose files of people ----

function LooseSection({ scan, onLeave }: { scan: MediaScan; onLeave: () => void }) {
  const [busy, setBusy] = useState(false);
  const move = async () => {
    setBusy(true);
    try {
      const plan = await previewMove({ to: "perPerson" });
      moveFiles({ to: "perPerson" }, plan.count);
    } catch (e) {
      done(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Block>
      <Text>{looseSection(scan.looseLinked.length)}</Text>
      <div className="row" style={{ gap: 8 }}>
        <button className="btn secondary sm" disabled={busy} onClick={move}>
          Przenieś do folderów osób
        </button>
        <button className="btn ghost sm" onClick={onLeave}>
          Zostaw
        </button>
      </div>
    </Block>
  );
}

// ---- unknown files ----

function UnknownSection({ files }: { files: ScanUnknown[] }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const shown = files.slice(0, MAX_ROWS);
  // Prefilled from the scan's suggestion — never applied before the button is pressed.
  const [choice, setChoice] = useState<Record<string, Choice>>({});
  const [off, setOff] = useState<ReadonlySet<string>>(new Set());
  const [years, setYears] = useState<Record<string, string | null>>({});

  // The scan names a suggested person without years; they are looked up for the label.
  useEffect(() => {
    const need = shown.map((u) => u.suggestion?.id).filter((id): id is string => !!id && !(id in years));
    if (need.length === 0) return;
    let current = true;
    void Promise.all([...new Set(need)].map(personOf)).then((people) => {
      if (!current) return;
      setYears((y) => {
        const next = { ...y };
        for (const id of new Set(need)) next[id] = people.find((p) => p?.id === id)?.years ?? null;
        return next;
      });
    });
    return () => {
      current = false;
    };
  }, [files]); // eslint-disable-line react-hooks/exhaustive-deps

  const suggestion = (u: ScanUnknown): LayoutPerson | null => (u.suggestion ? { id: u.suggestion.id, name: u.suggestion.name, years: years[u.suggestion.id] ?? null } : null);
  const current = (u: ScanUnknown): Choice | null => choice[u.path] ?? (suggestion(u) ? { kind: "person", person: suggestion(u)! } : null);
  const ticked = (u: ScanUnknown) => current(u) != null && !off.has(u.path);
  const chosen = shown.filter(ticked);

  const set = (u: ScanUnknown, c: Choice) => {
    setChoice((x) => ({ ...x, [u.path]: c }));
    setOff((o) => {
      const n = new Set(o);
      n.delete(u.path);
      return n;
    });
  };
  const other = async (u: ScanUnknown) => {
    const p = await pickPerson("Do kogo należy ten plik?");
    if (p) set(u, { kind: "person", person: p });
  };

  const apply = () =>
    requireEdit(async () => {
      const assign = chosen.flatMap((u) => {
        const c = current(u);
        return c?.kind === "person" ? [{ path: u.path, person: c.person.id }] : [];
      });
      const leave = chosen.filter((u) => current(u)?.kind === "leave").map((u) => ({ path: u.path, kind: "file" }));
      try {
        if (assign.length) await call("media.assign", { items: assign });
        if (leave.length) await call("media.leaveAlone", { items: leave });
        changed();
        scanSoon(0);
        const text = [assign.length ? `Przypisano ${filesWord(assign.length)}` : null, leave.length ? `zostawiono ${filesWord(leave.length)}` : null].filter(Boolean).join(", ");
        notify(`${text.charAt(0).toUpperCase()}${text.slice(1)}.`, assign.length ? { detail: "Zmiana czeka na zapis." } : undefined);
      } catch (e) {
        done(e);
        scanSoon(0);
      }
    });

  return (
    <Block>
      <Text>{unknownSection(files.length)}</Text>
      <div className="col" style={{ gap: 0, border: "1px solid var(--border)", borderRadius: "var(--r-ctl)", maxHeight: 300, overflow: "auto" }}>
        {shown.map((u) => {
          const c = current(u);
          const sug = suggestion(u);
          const value = c ? (c.kind === "leave" ? "leave" : `p:${c.person.id}`) : "";
          const options: LayoutPerson[] = [];
          if (sug) options.push(sug);
          for (const p of u.candidates) if (!options.some((o) => o.id === p.id)) options.push(p);
          if (c?.kind === "person" && !options.some((o) => o.id === c.person.id)) options.push(c.person);
          const tag = c?.kind === "person" && u.suggestion?.id === c.person.id ? u.suggestion.why : null;
          return (
            <div key={u.path} className="row" style={{ gap: 10, padding: "8px 10px", borderBottom: "1px solid var(--border)", alignItems: "center" }}>
              <Checkbox on={ticked(u)} disabled={c == null} onChange={(on) => setOff((o) => { const n = new Set(o); if (on) n.delete(u.path); else n.add(u.path); return n; })} />
              <Thumb path={u.path} size={128} icon={<FileQuestion size={18} />} style={{ width: 44, height: 44, flex: "none" }} />
              <span className="col grow" style={{ minWidth: 0, gap: 1 }}>
                <span className="ellipsis" style={{ fontSize: 14, fontWeight: 500 }} title={u.path}>
                  {u.name}
                </span>
                {u.folder !== "media" && (
                  <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                    {u.folder.replace(/^media\//, "")}
                  </span>
                )}
              </span>
              {tag && <span className="badge">{tag}</span>}
              <select
                aria-label={`Co zrobić z plikiem ${u.name}`}
                value={value}
                style={{ width: 270, height: 32, flex: "none" }}
                onChange={(e) => {
                  const v = e.target.value;
                  if (v === "other") void other(u);
                  else if (v === "leave") set(u, { kind: "leave" });
                  else if (v.startsWith("p:")) set(u, { kind: "person", person: options.find((o) => o.id === v.slice(2))! });
                }}
              >
                {!c && <option value="">Wybierz…</option>}
                {options.map((p) => (
                  <option key={p.id} value={`p:${p.id}`}>
                    Przypisz do: {personLabel(p)}
                  </option>
                ))}
                <option value="other">Przypisz do innej osoby…</option>
                <option value="leave">Zostaw w folderze</option>
              </select>
            </div>
          );
        })}
      </div>
      {files.length > shown.length && <div style={{ fontSize: 12, color: "var(--text3)" }}>Pokazuję pierwsze {num(shown.length)} z {num(files.length)}; reszta pojawi się po przypisaniu tych.</div>}
      <div className="row" style={{ gap: 12 }}>
        <span className="grow" style={{ fontSize: 12, color: "var(--text3)" }}>
          {UNKNOWN_FOOTER}
        </span>
        <button className="btn primary sm" disabled={chosen.length === 0} onClick={apply}>
          Przypisz zaznaczone{chosen.length > 0 ? ` (${num(chosen.length)})` : ""}
        </button>
      </div>
    </Block>
  );
}

// ---- B1: a folder in media/ ----

function leaveFolder(f: ScanFolder) {
  return call("media.leaveAlone", { path: f.path, kind: "folder" })
    .then(() => scanSoon(0))
    .catch(done);
}

function UnpackSection({ folder }: { folder: ScanFolder }) {
  const [assign, setAssign] = useState(true);
  const [busy, setBusy] = useState(false);
  const withPerson = folder.person != null && folder.unknown > 0;
  const unpack = async () => {
    setBusy(true);
    try {
      const args = { to: "flat" as const, folders: [folder.path], only: true };
      const plan = await previewMove(args);
      moveFiles({ ...args, ...(withPerson && assign ? { assignTo: folder.person!.id } : {}) }, plan.count);
    } catch (e) {
      done(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Block>
      <Text>{unpackQuestion(folder)}</Text>
      {withPerson && (
        <label className="row" style={{ gap: 8, fontSize: 14 }}>
          <Checkbox on={assign} onChange={setAssign} />
          i przypisz nowe pliki do: {personLabel(folder.person!)}
        </label>
      )}
      <div className="row" style={{ gap: 8 }}>
        <button className="btn secondary sm" disabled={busy} onClick={unpack}>
          Wypakuj do folderu media
        </button>
        <button className="btn ghost sm" onClick={() => void leaveFolder(folder)}>
          Zostaw folder, jak jest
        </button>
      </div>
    </Block>
  );
}

/** The folders beyond the first few (a flat archive that kept a folder for each person): one question for all of them. */
function UnpackMany({ folders }: { folders: ScanFolder[] }) {
  const [busy, setBusy] = useState(false);
  const unpack = async () => {
    setBusy(true);
    try {
      const args = { to: "flat" as const, folders: folders.map((f) => f.path), only: true };
      const plan = await previewMove(args);
      moveFiles(args, plan.count);
    } catch (e) {
      done(e);
    } finally {
      setBusy(false);
    }
  };
  const leave = () =>
    call("media.leaveAlone", { items: folders.map((f) => ({ path: f.path, kind: "folder" })) })
      .then(() => scanSoon(0))
      .catch(done);
  return (
    <Block>
      <Text>{moreFoldersQuestion(folders)}</Text>
      <div className="row" style={{ gap: 8 }}>
        <button className="btn secondary sm" disabled={busy} onClick={unpack}>
          Wypakuj do folderu media
        </button>
        <button className="btn ghost sm" onClick={() => void leave()}>
          Zostaw te foldery, jak są
        </button>
      </div>
    </Block>
  );
}

// ---- B2: a folder of nobody ----

function WhoseSection({ folder, files, onLeave }: { folder: ScanFolder; files: ScanUnknown[]; onLeave: () => void }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const choose = async () => {
    const p = await pickPerson(`Czyj to folder „${folder.name}”?`);
    if (!p) return;
    // Asked once more with the name: it files 23 photos under one person.
    setAsk({
      title: `Przypisać pliki do: ${personLabel(p)}?`,
      text: `${filesWord(files.length)} z folderu „${folder.name}” ${files.length === 1 ? "pojawi się" : "pojawią się"} u tej osoby. Zmianę można cofnąć.`,
      icon: "info",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Przypisz",
          kind: "primary",
          run: () =>
            requireEdit(async () => {
              try {
                await call("media.assign", { items: files.map((u) => ({ path: u.path, person: p.id })) });
                changed();
                scanSoon(0);
                notify(`Przypisano ${filesWord(files.length)} do: ${p.name}.`, { detail: "Zmiana czeka na zapis." });
              } catch (e) {
                done(e);
              }
            }),
        },
      ],
    });
  };
  return (
    <Block>
      <Text>{whoseQuestion(folder)}</Text>
      <div className="row" style={{ gap: 8 }}>
        <button className="btn secondary sm" disabled={files.length === 0} onClick={() => void choose()}>
          Wybierz osobę…
        </button>
        <button className="btn ghost sm" onClick={() => void leaveFolder(folder).then(onLeave)}>
          Zostaw
        </button>
      </div>
    </Block>
  );
}

// ---- files outside the archive ----

function OutsideSection({ scan }: { scan: MediaScan }) {
  const requireEdit = useStore((s) => s.requireEdit);
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const copy = () =>
    requireEdit(async () => {
      const failed: string[] = [];
      let copied = 0;
      for (const [i, f] of scan.outside.entries()) {
        useStore.setState({ busy: { text: "Kopiuję pliki do archiwum…", done: i, total: scan.outside.length } });
        try {
          await call("media.relink", { id: f.id, path: f.file });
          copied++;
        } catch (e) {
          failed.push((e as ApiError).message);
        }
      }
      useStore.setState({ busy: null });
      changed();
      scanSoon(0);
      if (copied) notify(`Skopiowano ${filesWord(copied)} do archiwum.`, { detail: "Zmiana czeka na zapis." });
      if (failed.length) notify(failed[0], { kind: "err", detail: failed.length > 1 ? `I ${count(failed.length - 1, "inny plik", "inne pliki", "innych plików")}.` : undefined });
    });
  return (
    <Block>
      <Text>{outsideSection(scan.outside.length)}</Text>
      <div className="row">
        <button className="btn secondary sm" onClick={copy}>
          Skopiuj do archiwum
        </button>
      </div>
    </Block>
  );
}
