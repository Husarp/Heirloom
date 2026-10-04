import { AppWindow, ArrowRight, FileText, FolderOpen, FolderSearch, Layers, Trash2, TriangleAlert } from "lucide-react";
import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus, CombinedArchive } from "../../api/types";
import { afterChange, lookForPairs, useStore } from "../../app/store";
import { ArchiveDot } from "../../components/bits";
import { count, displayPath, num, people as peopleCount } from "../../lib/format";
import { pickFolder, pickGedcom } from "../../lib/native";

/** Start in archives opened together (design §6.5): „W tym zestawie” — each archive with its colour, how many people,
 *  where it is, and what to do when it can't be found; adding an archive; the pairs waiting in „Do sprawdzenia”. */
export function SetCard() {
  const combined = useStore((s) => s.archive?.combined);
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const openElsewhere = useStore((s) => s.openElsewhere);
  if (!combined) return null;

  const run = async (method: string, args: object, done?: (result: ArchiveStatus & { different?: boolean }) => void) => {
    try {
      const status = await call<ArchiveStatus & { different?: boolean }>(method, args);
      afterChange(status);
      lookForPairs();
      done?.(status);
    } catch (e) {
      notify((e as ApiError).message, { kind: "err" });
    }
  };
  const add = async (gedcom: boolean) => {
    const path = gedcom ? await pickGedcom() : await pickFolder("Dodaj archiwum do zestawu");
    if (path) run("set.addArchive", { path }, (s) => notify(`Dodano do zestawu: ${s.combined?.archives.at(-1)?.name ?? path}.`));
  };
  const locate = async (a: CombinedArchive) => {
    const path = await pickFolder(`Gdzie jest teraz archiwum „${a.name}”?`);
    if (path)
      run("set.locate", { key: a.key, path }, (s) =>
        notify(s.different ? `Wskazany folder ma inne archiwum niż „${a.name}” — sprawdź, czy to właściwy folder.` : `Znaleziono: ${a.name}.`, s.different ? { kind: "err" } : undefined),
      );
  };
  const remove = (a: CombinedArchive) =>
    setAsk({
      title: `Usunąć „${a.name}” z zestawu?`,
      text: "Jego osoby znikną z tego widoku, a połączenia z nimi i odpowiedzi „Nie” zostaną usunięte z pliku zestawu. Samo archiwum zostaje na dysku bez zmian.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        { label: "Usuń z zestawu", kind: "danger", run: () => run("set.removeArchive", { key: a.key }) },
      ],
    });

  return (
    <div className="card">
      <div className="card-head">
        <Layers size={16} color="var(--accent-text)" />
        <span className="title">W tym zestawie</span>
        <span className="mono ellipsis" style={{ fontSize: 12, color: "var(--text3)", maxWidth: "55%" }} title={displayPath(combined.setPath)}>
          {displayPath(combined.setPath)}
        </span>
      </div>
      {combined.archives.map((a, i) => {
        const ok = a.state === "ok";
        return (
          <div key={a.key} className={`list-row set-archive${ok ? "" : " missing"}`} style={{ borderTop: i ? undefined : "none" }}>
            <ArchiveDot from={[a.key]} size={18} style={{ boxShadow: "none", opacity: ok ? 1 : 0.4 }} />
            <span className="col grow" style={{ minWidth: 0 }}>
              <span className="row" style={{ gap: 8 }}>
                <span className="serif ellipsis" style={{ fontSize: 16, fontWeight: 600 }}>
                  {a.name}
                </span>
                {ok && <span style={{ fontSize: 12, color: "var(--text2)", whiteSpace: "nowrap" }}>{peopleCount(a.people)}</span>}
              </span>
              {ok || a.state === "other" ? (
                <span className="mono ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                  {a.path ? displayPath(a.path) : ""}
                </span>
              ) : null}
              {!ok && (
                <span className="row" style={{ gap: 6, fontSize: 13, color: "var(--warn)" }}>
                  <TriangleAlert size={13} style={{ flex: "none" }} />
                  <span className="ellipsis">
                    {a.state === "missing"
                      ? `Nie znaleziono${a.path ? ` (${displayPath(a.path)})` : ""}. Jego osób nie widać.`
                      : a.state === "other"
                        ? "Ma inny identyfikator niż zapisany w zestawie — to może być inne archiwum."
                        : `Nie udało się odczytać${a.error ? `: ${a.error}` : "."}`}
                  </span>
                </span>
              )}
            </span>
            {a.state !== "ok" && (
              <button className="btn secondary sm" onClick={() => locate(a)}>
                <FolderSearch size={14} />
                Wskaż folder…
              </button>
            )}
            {ok && a.path && (
              <button className="icon-btn" title="Otwórz w nowym oknie (tam można je zmieniać)" aria-label={`${a.name}: otwórz w nowym oknie`} onClick={() => openElsewhere(a.path!)}>
                <AppWindow size={16} />
              </button>
            )}
            {/* A set is two archives or more: one would be just that archive. */}
            <button
              className="icon-btn"
              disabled={combined.archives.length <= 2}
              title={combined.archives.length <= 2 ? "W zestawie muszą zostać co najmniej dwa archiwa — najpierw dodaj inne" : "Usuń z zestawu (archiwum zostaje na dysku)"}
              aria-label={`${a.name}: usuń z zestawu`}
              onClick={() => remove(a)}
            >
              <Trash2 size={15} />
            </button>
          </div>
        );
      })}
      <div className="list-row" style={{ minHeight: 52, gap: 8, flexWrap: "wrap", padding: "8px 16px" }}>
        <button className="btn ghost sm" onClick={() => add(false)}>
          <FolderOpen size={14} />
          Dodaj archiwum…
        </button>
        <button className="btn ghost sm" onClick={() => add(true)}>
          <FileText size={14} />
          Dodaj plik GEDCOM…
        </button>
        <span className="grow" />
        <button className="link row" style={{ gap: 6, fontSize: 13 }} onClick={() => go({ name: "pairs" })}>
          Do sprawdzenia: {combined.pending == null ? "…" : count(combined.pending, "para", "pary", "par")} · połączonych osób: {num(combined.linked)}
          {combined.lost > 0 && <span style={{ color: "var(--warn)" }}> · {count(combined.lost, "połączenie", "połączenia", "połączeń")} bez osoby</span>}
          <ArrowRight size={13} />
        </button>
      </div>
    </div>
  );
}
