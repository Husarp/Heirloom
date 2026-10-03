import { FileCode, ImageOff } from "lucide-react";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { count, displayPath, num, plural } from "../../lib/format";
import "./archive.css";

interface FirstOpenData {
  people: number;
  families: number;
  photos: number;
  sources: number;
  missingFiles: number;
  otherFields: number;
}

/** Pierwsze otwarcie (spec §4.18): what was found and what's missing; „Otwórz” goes to Start. The tree starts from
 *  a suggested person on its own (no question about it any more). */
export function FirstOpen() {
  const archive = useStore((s) => s.archive);
  const finish = useStore((s) => s.finishFirstOpen);
  const go = useStore((s) => s.go);
  const { data } = useApi<FirstOpenData>("archive.firstOpen");

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
        <div className="row" style={{ gap: 8, marginTop: 6, justifyContent: "flex-end" }}>
          <button className="btn primary" onClick={finish}>
            Otwórz
          </button>
        </div>
      </div>
    </div>
  );
}
