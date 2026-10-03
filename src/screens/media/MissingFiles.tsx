import { ChevronRight, CircleCheck, FileSearch, FolderSearch, Info } from "lucide-react";
import { useCallback, useState } from "react";
import { call, type ApiError } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Checkbox, EmptyState, Spinner, useDismiss } from "../../components/bits";
import { count, num, plural } from "../../lib/format";
import { pickFiles, pickFolder } from "../../lib/native";
import { filtersLike, runEdit } from "./shared";
import "./media.css";

interface MissingRow {
  id: string;
  /** The file name ("M0142.jpg"). */
  file: string;
  /** As written in the archive ("media/M0142.jpg"). */
  path: string;
  title: string | null;
}

interface FindResult {
  rows: { id: string; file: string; title: string | null; found: string[] }[];
  folder: string;
}

/** Brakujące pliki (spec §4.33): files the archive describes that aren't in its folder. „Szukaj w folderze…” matches
 *  them by name; a file found once is ticked, several need a look („sprawdź”), none can be pointed to by hand.
 *  Nothing changes until „Zastosuj”, which copies the chosen files back into media/. */
export function MissingFiles() {
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const { data, error, loading } = useApi<{ rows: MissingRow[] }>("media.missing");
  const rows = data?.rows ?? [];

  /** Files with the same name found by the searches, per record. */
  const [candidates, setCandidates] = useState<Record<string, string[]>>({});
  /** The file that will be copied back, per record. */
  const [chosen, setChosen] = useState<Record<string, string>>({});
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const [lastSearch, setLastSearch] = useState<{ folder: string; found: number; total: number } | null>(null);
  const [searching, setSearching] = useState(false);
  const [applying, setApplying] = useState<{ done: number; total: number } | null>(null);
  const [menu, setMenu] = useState<string | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);

  const choose = (id: string, path: string) => {
    setChosen((c) => ({ ...c, [id]: path }));
    setChecked((s) => new Set(s).add(id));
    setMenu(null);
  };

  const searchFolder = async () => {
    const folder = await pickFolder("Wybierz folder, w którym mogą być brakujące pliki");
    if (!folder) return;
    setSearching(true);
    try {
      const result = await call<FindResult>("media.findMissing", { folder });
      // A later search in another folder only changes the files it found; earlier matches stay.
      const nextCandidates = { ...candidates };
      const nextChosen = { ...chosen };
      const nextChecked = new Set(checked);
      let found = 0;
      for (const r of result.rows) {
        if (!r.found.length) continue;
        found++;
        nextCandidates[r.id] = r.found;
        if (r.found.length === 1) {
          nextChosen[r.id] = r.found[0];
          nextChecked.add(r.id);
        } else {
          delete nextChosen[r.id];
          nextChecked.delete(r.id);
        }
      }
      setCandidates(nextCandidates);
      setChosen(nextChosen);
      setChecked(nextChecked);
      setLastSearch({ folder: result.folder, found, total: result.rows.length });
    } catch (e) {
      notify((e as ApiError).message, { kind: "err" });
    } finally {
      setSearching(false);
    }
  };

  const pickFor = async (row: MissingRow) => {
    setMenu(null);
    const [path] = await pickFiles(`Wskaż plik ${row.file}`, filtersLike(row.file), false);
    if (path) choose(row.id, path);
  };

  const toApply = rows.filter((r) => checked.has(r.id) && chosen[r.id]);
  const apply = () =>
    runEdit(async () => {
      const failed: string[] = [];
      let restored = 0;
      setApplying({ done: 0, total: toApply.length });
      try {
        for (const [i, r] of toApply.entries()) {
          try {
            await call("media.relink", { id: r.id, path: chosen[r.id] });
            restored++;
          } catch (e) {
            failed.push(`${r.file}: ${(e as ApiError).message}`);
          }
          setApplying({ done: i + 1, total: toApply.length });
        }
      } finally {
        setApplying(null);
      }
      if (restored) {
        afterChange();
        notify(`Przywrócono ${count(restored, "plik", "pliki", "plików")} do folderu media/.`);
      }
      if (failed.length) {
        const more = failed.length > 1 ? ` (i ${count(failed.length - 1, "inny", "inne", "innych")})` : "";
        notify(`Nie udało się przywrócić ${failed[0]}${more}`, { kind: "err" });
      }
    });

  const choosable = rows.filter((r) => chosen[r.id]);
  const allChecked = choosable.length > 0 && choosable.every((r) => checked.has(r.id));
  const toggleAll = () =>
    setChecked((s) => {
      const next = new Set(s);
      for (const r of choosable) {
        if (allChecked) next.delete(r.id);
        else next.add(r.id);
      }
      return next;
    });
  const manualTarget = rows.find((r) => r.id === selected) ?? (rows.length === 1 ? rows[0] : undefined);

  const crumb = (
    <div className="missing-crumb">
      <button onClick={() => go({ name: "media" })}>Media</button>
      <ChevronRight size={12} color="var(--text3)" />
      <b style={{ color: "var(--text)", fontWeight: 600 }}>Brakujące pliki</b>
    </div>
  );

  if (!data) {
    return (
      <div className="page">
        <div className="missing-page">
          {crumb}
          <h1 className="page-title">Brakujące pliki</h1>
          {loading ? (
            <div className="row" style={{ gap: 10, color: "var(--text3)", fontSize: 13 }}>
              <Spinner size={16} />
              Sprawdzam, czy wszystkie pliki są na miejscu…
            </div>
          ) : (
            <div className="banner err">{error?.message ?? "Nie udało się sprawdzić plików."}</div>
          )}
        </div>
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="page">
        <div className="missing-page">
          {crumb}
          <h1 className="page-title">Brakujące pliki</h1>
          <div className="card">
            <EmptyState
              icon={<CircleCheck size={32} color="var(--accent-text)" />}
              title="Wszystkie pliki są na miejscu."
              text="Każdy plik opisany w archiwum jest w jego folderze."
              action={
                <button className="link" style={{ fontSize: 13 }} onClick={() => go({ name: "media" })}>
                  Wróć do Mediów
                </button>
              }
            />
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="missing-page">
        {crumb}
        <div className="row" style={{ alignItems: "flex-end", gap: 12 }}>
          <div className="col grow" style={{ gap: 4 }}>
            <h1 className="page-title">
              Brakujące pliki <span style={{ fontSize: 20, color: "var(--text3)" }}>{num(rows.length)}</span>
            </h1>
            <span style={{ fontSize: 14, color: "var(--text2)" }}>
              Te pliki są opisane w archiwum, ale nie ma ich w folderze media/. Zwykle ktoś je przeniósł poza programem.
            </span>
          </div>
          <button
            className="btn secondary"
            onClick={() => manualTarget && pickFor(manualTarget)}
            disabled={!manualTarget}
            title={manualTarget ? `Wskaż plik ${manualTarget.file}` : "Najpierw zaznacz wiersz w tabeli"}
          >
            <FileSearch size={15} />
            Wskaż ręcznie
          </button>
          <button className="btn primary" style={{ padding: "0 16px" }} onClick={searchFolder} disabled={searching}>
            {searching ? <Spinner size={15} /> : <FolderSearch size={15} />}
            {searching ? "Szukam…" : "Szukaj w folderze…"}
          </button>
        </div>

        {lastSearch &&
          (lastSearch.found > 0 ? (
            <div className="banner success">
              <CircleCheck size={16} color="var(--accent-text)" style={{ flex: "none" }} />
              <span className="grow">
                W folderze{" "}
                <b className="mono" style={{ fontWeight: 600 }}>
                  {lastSearch.folder}
                </b>{" "}
                znaleziono{" "}
                <b style={{ fontWeight: 600 }}>
                  {num(lastSearch.found)} z {num(lastSearch.total)}
                </b>{" "}
                {plural(lastSearch.total, "pliku", "plików", "plików")} po nazwie.
              </span>
            </div>
          ) : (
            <div className="banner info">
              <Info size={16} color="var(--text2)" style={{ flex: "none" }} />
              <span className="grow">
                W folderze{" "}
                <b className="mono" style={{ fontWeight: 600 }}>
                  {lastSearch.folder}
                </b>{" "}
                nie ma żadnego z brakujących plików.
              </span>
            </div>
          ))}

        <div className="missing-table">
          <div className="missing-head">
            <span className="row">{choosable.length > 0 && <Checkbox on={allChecked} onChange={toggleAll} />}</span>
            <span>Plik</span>
            <span>Znaleziony w</span>
            <span>Opisany jako</span>
            <span>Stan</span>
          </div>
          {rows.map((r) => {
            const found = candidates[r.id] ?? [];
            const path = chosen[r.id] ?? found[0];
            const several = found.length > 1;
            const status = chosen[r.id]
              ? { text: "dopasowano", color: "var(--accent-text)" }
              : several
                ? { text: "sprawdź", color: "var(--warn)" }
                : { text: "Wskaż ręcznie", color: "var(--text2)" };
            return (
              <div key={r.id} className={`missing-row${r.id === selected ? " selected" : ""}`} onClick={() => setSelected(r.id)}>
                <span className="row">
                  <Checkbox
                    on={checked.has(r.id) && !!chosen[r.id]}
                    disabled={!chosen[r.id]}
                    onChange={(on) =>
                      setChecked((s) => {
                        const next = new Set(s);
                        if (on) next.add(r.id);
                        else next.delete(r.id);
                        return next;
                      })
                    }
                  />
                </span>
                <span className="mono ellipsis" title={r.path}>
                  {r.file}
                </span>
                <span className="mono ellipsis" style={{ fontSize: 12, color: path ? "var(--text2)" : "var(--text3)" }} title={path}>
                  {path ?? (lastSearch ? "nie znaleziono" : "—")}
                </span>
                <span className="ellipsis" style={{ color: "var(--text2)" }} title={r.title ?? undefined}>
                  {r.title || "—"}
                  {several && !chosen[r.id] && <span style={{ color: "var(--text3)" }}> ({count(found.length, "plik", "pliki", "plików")} o tej nazwie)</span>}
                </span>
                <span>
                  {chosen[r.id] && !several ? (
                    <span className="missing-status" style={{ color: status.color }}>
                      {status.text}
                    </span>
                  ) : (
                    <button
                      className="missing-status"
                      style={{ color: status.color }}
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelected(r.id);
                        if (several) setMenu(menu === r.id ? null : r.id);
                        else pickFor(r);
                      }}
                    >
                      {status.text}
                    </button>
                  )}
                </span>
                {menu === r.id && <CandidateMenu found={found} chosen={chosen[r.id]} onPick={(p) => choose(r.id, p)} onManual={() => pickFor(r)} onClose={closeMenu} />}
              </div>
            );
          })}
        </div>

        <div className="row" style={{ gap: 10, fontSize: 13 }}>
          <span className="grow" style={{ color: "var(--text2)" }}>
            {applying
              ? `Kopiuję pliki… ${num(applying.done)} z ${num(applying.total)}`
              : "Pliki zostaną skopiowane z powrotem do media/. Oryginały zostają na miejscu."}
          </span>
          <button className="btn primary" style={{ padding: "0 16px" }} onClick={apply} disabled={toApply.length === 0 || applying != null}>
            {applying && <Spinner size={15} />}
            Zastosuj {count(toApply.length, "dopasowanie", "dopasowania", "dopasowań")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** „sprawdź”: several files with the same name were found; one is picked here (or another one by hand). */
function CandidateMenu({
  found,
  chosen,
  onPick,
  onManual,
  onClose,
}: {
  found: string[];
  chosen?: string;
  onPick: (path: string) => void;
  onManual: () => void;
  onClose: () => void;
}) {
  const ref = useDismiss<HTMLDivElement>(true, onClose);
  return (
    <div ref={ref} className="popover missing-menu" onClick={(e) => e.stopPropagation()}>
      <div className="menu-label">Który plik przywrócić?</div>
      {found.map((p) => (
        <button key={p} className={`menu-item${p === chosen ? " on" : ""}`} onClick={() => onPick(p)} title={p}>
          <span className="mono ellipsis" style={{ fontSize: 12 }}>
            {p}
          </span>
        </button>
      ))}
      <button className="menu-item" style={{ borderTop: "1px solid var(--border)", color: "var(--accent-text)" }} onClick={onManual}>
        <FileSearch size={14} />
        Wskaż inny plik…
      </button>
    </div>
  );
}
