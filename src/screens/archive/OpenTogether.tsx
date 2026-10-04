import { FileText, FolderOpen, Layers } from "lucide-react";
import { useMemo, useState } from "react";
import type { RecentArchive } from "../../api/types";
import { isSetPath, useStore } from "../../app/store";
import { Checkbox, rowButton } from "../../components/bits";
import { Dialog } from "../../components/Dialog";
import { displayPath, people as peopleCount } from "../../lib/format";
import { pickFolder, pickGedcom, pickSavePath } from "../../lib/native";

/** One archive to choose from: from the recent list, or added here with „Dodaj folder…”. */
interface Choice {
  path: string;
  name: string;
  people: number | null;
}

/** „Otwórz razem…” (design §6.2): two or more archives shown as one tree. The archives are only read; the choice and
 *  the people found to be the same person are kept in a set file (`.heirloom-zestaw`) saved next to them. `with`: an
 *  archive chosen from the start (the one open now). */
export function OpenTogether({ onClose, with: first }: { onClose: () => void; with?: string }) {
  const app = useStore((s) => s.app);
  const createSet = useStore((s) => s.createSet);
  const [added, setAdded] = useState<Choice[]>([]);
  const [chosen, setChosen] = useState<string[]>(first ? [first] : []);
  const [name, setName] = useState<string | null>(null);
  const [place, setPlace] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const choices: Choice[] = useMemo(() => {
    const recent = (app?.recent ?? []).filter((r: RecentArchive) => r.kind !== "set" && !isSetPath(r.path)).map((r) => ({ path: r.path, name: r.name, people: r.people }));
    return [...recent, ...added.filter((a) => !recent.some((r) => r.path === a.path))];
  }, [app, added]);
  const picked = chosen.map((p) => choices.find((c) => c.path === p)).filter((c): c is Choice => c != null);
  const autoName = picked.map((c) => c.name).join(" + ");
  const title = name ?? autoName;
  const folder = place ? null : commonFolder(picked.map((c) => c.path));
  const target = place ?? (folder ? joinPath(folder, `${fileName(title || "Rodzina razem")}.heirloom-zestaw`) : "");

  const toggle = (path: string) => setChosen((c) => (c.includes(path) ? c.filter((p) => p !== path) : [...c, path]));
  const add = (path: string | null) => {
    if (!path) return;
    if (!choices.some((c) => c.path === path)) setAdded((a) => [...a, { path, name: baseName(path), people: null }]);
    setChosen((c) => (c.includes(path) ? c : [...c, path]));
  };
  const choosePlace = async () => {
    const path = await pickSavePath(target || `${fileName(title)}.heirloom-zestaw`, [{ name: "Zestaw archiwów Heirloom", extensions: ["heirloom-zestaw"] }]);
    if (path) setPlace(path);
  };
  const open = async () => {
    setBusy(true);
    const ok = await createSet(target, title.trim(), picked.map((c) => c.path), place != null);
    setBusy(false);
    if (ok) onClose();
  };

  const ready = picked.length >= 2 && title.trim() !== "" && target !== "";

  return (
    <Dialog width={620} onClose={onClose} labelledBy="together-title">
      <div className="dialog-body">
        <div className="dialog-icon" style={{ color: "var(--accent-text)", background: "var(--accent-soft)" }}>
          <Layers size={19} />
        </div>
        <div className="dialog-title" id="together-title">
          Otwórz razem
        </div>
        <div className="dialog-text">
          Wybierz dwa albo więcej archiwów, a Heirloom pokaże je jako jedno drzewo. Pliki archiwów zostają osobno i nic się w
          nich nie zmienia. Wybór i to, kto jest tą samą osobą, zapiszesz w pliku zestawu obok archiwów — otworzysz go potem
          jak archiwum.
        </div>
        <div className="card" style={{ maxHeight: 260, overflow: "auto" }}>
          {choices.length === 0 && <div style={{ padding: 16, fontSize: 13, color: "var(--text3)" }}>Dodaj archiwa przyciskami poniżej.</div>}
          {choices.map((c, i) => {
            const on = chosen.includes(c.path);
            return (
              <div key={c.path} className={`list-row clickable${on ? " selected" : ""}`} style={{ minHeight: 52, gap: 12, borderTop: i ? undefined : "none" }} {...rowButton(() => toggle(c.path))}>
                <Checkbox on={on} onChange={() => toggle(c.path)} />
                <span className="icon-tile neutral" style={{ width: 32, height: 32 }}>
                  {c.path.toLowerCase().endsWith(".ged") ? <FileText size={15} /> : <FolderOpen size={15} />}
                </span>
                <span className="col grow" style={{ minWidth: 0 }}>
                  <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
                    {c.name}
                  </span>
                  <span className="mono ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                    {displayPath(c.path)}
                  </span>
                </span>
                <span style={{ fontSize: 12, color: "var(--text2)", whiteSpace: "nowrap" }}>{c.people ? peopleCount(c.people) : ""}</span>
              </div>
            );
          })}
        </div>
        <div className="row" style={{ gap: 8, marginTop: -4 }}>
          <button className="btn secondary sm" onClick={async () => add(await pickFolder("Dodaj folder archiwum"))}>
            <FolderOpen size={14} />
            Dodaj folder…
          </button>
          <button className="btn secondary sm" onClick={async () => add(await pickGedcom())}>
            <FileText size={14} />
            Dodaj plik GEDCOM…
          </button>
          <span className="grow" />
          <span style={{ fontSize: 12, color: picked.length >= 2 ? "var(--text3)" : "var(--warn)" }}>
            {picked.length >= 2 ? `wybrano ${picked.length}` : "wybierz co najmniej dwa"}
          </span>
        </div>
        <label className="field">
          <span className="field-label">Nazwa zestawu</span>
          <input className="input" value={title} onChange={(e) => setName(e.target.value)} placeholder="np. Kowalscy + Nowakowie" />
        </label>
        <div className="field">
          <span className="field-label">Plik zestawu</span>
          <div className="row" style={{ gap: 8 }}>
            <span className="input mono ellipsis" style={{ fontSize: 13, display: "flex", alignItems: "center", color: target ? "var(--text)" : "var(--text3)" }} title={target}>
              {target ? displayPath(target) : picked.length ? "wskaż miejsce: Zmień…" : "wybierz archiwa"}
            </span>
            <button className="btn secondary" disabled={picked.length === 0} onClick={choosePlace}>
              Zmień…
            </button>
          </div>
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" disabled={!ready || busy} onClick={open}>
          Zapisz zestaw i otwórz
        </button>
      </div>
    </Dialog>
  );
}

const separator = (path: string) => (path.includes("\\") ? "\\" : "/");

/** The folder a set file goes in by default: the folder holding all the archives (a .ged file's own folder). */
function commonFolder(paths: string[]): string | null {
  if (paths.length === 0) return null;
  const sep = separator(paths[0]);
  const parents = paths.map((p) => p.replace(/[\\/]+$/, "").split(/[\\/]/).slice(0, -1));
  const common: string[] = [];
  for (let i = 0; i < parents[0].length; i++) {
    const part = parents[0][i];
    if (!parents.every((p) => p[i]?.toLowerCase() === part.toLowerCase())) break;
    common.push(part);
  }
  if (common.length === 0) return null;
  // "C:" alone is the drive's root; "" alone is "/".
  return common.length === 1 && common[0] === "" ? sep : common.join(sep) + (common.length === 1 ? sep : "");
}

function joinPath(folder: string, name: string): string {
  const sep = separator(folder);
  return folder.endsWith(sep) ? folder + name : folder + sep + name;
}

/** A folder's or a .ged file's name, for an archive added here. */
function baseName(path: string): string {
  const last = path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? path;
  return last.replace(/\.ged$/i, "");
}

/** A set's name as a file name: without the characters Windows doesn't allow. */
function fileName(name: string): string {
  return name.replace(/[<>:"/\\|?*]+/g, " ").replace(/\s+/g, " ").trim() || "Rodzina razem";
}
