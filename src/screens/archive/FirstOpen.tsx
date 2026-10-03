import { FileCode, ImageOff, Search } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus, PersonSummary } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { count, displayPath, num, plural } from "../../lib/format";
import "./archive.css";

interface Suggestion {
  person: { id: string; name: string };
  reason: string;
}

interface FirstOpenData {
  people: number;
  families: number;
  photos: number;
  sources: number;
  missingFiles: number;
  otherFields: number;
  suggestions: Suggestion[];
}

/** Pierwsze otwarcie (spec §4.18): what was found, what's missing, and whom the tree starts from. Skippable. */
export function FirstOpen() {
  const archive = useStore((s) => s.archive);
  const finish = useStore((s) => s.finishFirstOpen);
  const setArchive = useStore((s) => s.setArchive);
  const go = useStore((s) => s.go);
  const { data } = useApi<FirstOpenData>("archive.firstOpen");
  const [chosen, setChosen] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<(PersonSummary & { context: string })[]>([]);

  useEffect(() => {
    if (data?.suggestions[0] && !chosen) setChosen(data.suggestions[0].person.id);
  }, [data, chosen]);

  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    // Only the answer for the latest query counts (an earlier, slower one mustn't replace it).
    let latest = true;
    call<(PersonSummary & { context: string })[]>("people.search", { q: query, limit: 3 })
      .then((h) => latest && setHits(h))
      .catch(() => {});
    return () => {
      latest = false;
    };
  }, [query]);

  const open = async () => {
    if (chosen) {
      const status = await call<ArchiveStatus>("archive.setSettings", { startPerson: chosen });
      setArchive(status);
    }
    finish();
  };

  const options: { id: string; name: string; reason: string }[] = hits.length
    ? hits.map((h) => ({ id: h.id, name: h.name, reason: h.context || "wynik wyszukiwania" }))
    : (data?.suggestions ?? []).map((s) => ({ id: s.person.id, name: s.person.name, reason: s.reason }));

  return (
    <div className="fullscreen">
      <div className="first-open">
        <div className="col" style={{ gap: 4 }}>
          <span className="mono" style={{ fontSize: 13, color: "var(--text3)" }}>
            {archive ? displayPath(archive.root) : ""} · {archive?.dataFile}
          </span>
          <h1 className="serif" style={{ fontSize: 36, lineHeight: 1.15, fontWeight: 500 }}>
            Archiwum „{archive?.name}” jest gotowe
          </h1>
        </div>
        <div className="stat-block" style={{ gridTemplateColumns: "repeat(4, 1fr)" }}>
          {(
            [
              [data?.people ?? archive?.people ?? 0, ["osoba", "osoby", "osób"]],
              [data?.families ?? archive?.families ?? 0, ["rodzina", "rodziny", "rodzin"]],
              [data?.photos ?? 0, ["zdjęcie", "zdjęcia", "zdjęć"]],
              [data?.sources ?? 0, ["źródło", "źródła", "źródeł"]],
            ] as [number, [string, string, string]][]
          ).map(([value, words]) => (
            <div key={words[2]}>
              <span className="value" style={{ fontSize: 28 }}>
                {num(value)}
              </span>
              <span className="key">{plural(value, ...words)}</span>
            </div>
          ))}
        </div>
        {data && data.missingFiles > 0 && (
          <div className="banner warn" style={{ padding: "10px 10px 10px 14px" }}>
            <ImageOff size={16} color="var(--warn)" />
            <span className="grow">Nie znaleziono {count(data.missingFiles, "pliku", "plików", "plików")} — wskaż folder, w którym mogą być.</span>
            <button
              className="btn secondary sm"
              onClick={() => {
                finish();
                go({ name: "missingFiles" });
              }}
            >
              Wskaż folder…
            </button>
          </div>
        )}
        {data && data.otherFields > 0 && (
          <div className="banner info">
            <FileCode size={16} color="var(--text2)" />
            <span>
              {count(data.otherFields, "pole", "pola", "pól")} z innego programu zachowano; widać je w profilach jako „Inne dane z pliku”.
            </span>
          </div>
        )}
        {archive && archive.warnings.length > 0 && (
          <div className="banner info" style={{ alignItems: "flex-start" }}>
            <FileCode size={16} color="var(--text2)" style={{ marginTop: 2 }} />
            <span className="col" style={{ gap: 2 }}>
              {archive.warnings.slice(0, 4).map((w, i) => (
                <span key={i}>{w.line ? `Wiersz ${num(w.line)}: ` : ""}{w.message}</span>
              ))}
              {archive.warnings.length > 4 && <span style={{ color: "var(--text3)" }}>…i jeszcze {archive.warnings.length - 4}</span>}
            </span>
          </div>
        )}
        <div className="col" style={{ gap: 10 }}>
          <span style={{ fontSize: 15, fontWeight: 600 }}>Od kogo zacząć drzewo?</span>
          <div className="search-box" style={{ height: 40 }}>
            <Search size={15} />
            <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Szukaj osoby…" />
          </div>
          <div className="row" style={{ gap: 10 }}>
            {options.map((o) => (
              <button key={o.id} className={`option-card${chosen === o.id ? " on" : ""}`} onClick={() => setChosen(o.id)}>
                <span className={`radio${chosen === o.id ? " on" : ""}`} />
                <span className="col" style={{ minWidth: 0 }}>
                  <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
                    {o.name}
                  </span>
                  <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                    {o.reason}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </div>
        <div className="row" style={{ gap: 8, marginTop: 6 }}>
          <span className="grow" style={{ fontSize: 13, color: "var(--text3)" }}>
            Wszystko to zmienisz później w Ustawieniach.
          </span>
          <button className="btn ghost" onClick={finish}>
            Pomiń
          </button>
          <button className="btn primary" onClick={open}>
            Otwórz
          </button>
        </div>
      </div>
    </div>
  );
}
