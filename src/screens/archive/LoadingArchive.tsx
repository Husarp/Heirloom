import { Circle } from "lucide-react";
import { useStore } from "../../app/store";
import { Spinner } from "../../components/bits";
import "./archive.css";

/** Wczytywanie (spec §4.17). Opening takes one step on the Rust side, so the progress is shown as ongoing. */
export function LoadingArchive() {
  const opening = useStore((s) => s.opening);
  const name = opening?.split(/[\\/]/).filter(Boolean).pop() ?? "";
  return (
    <div className="fullscreen" style={{ alignItems: "center" }}>
      <div className="col" style={{ width: 520, gap: 18 }}>
        <div className="col" style={{ gap: 4 }}>
          <span style={{ fontSize: 13, color: "var(--text3)" }}>Otwieranie · {name}</span>
          <span className="page-title">Wczytuję archiwum…</span>
        </div>
        <div style={{ height: 8, borderRadius: 4, background: "var(--surface2)", overflow: "hidden" }}>
          <div className="loading-bar" />
        </div>
        <div className="card col" style={{ padding: "14px 16px", gap: 8, fontSize: 14 }}>
          {["Osoby", "Rodziny", "Zdjęcia i pliki", "Źródła"].map((step, i) => (
            <div key={step} className="row" style={{ gap: 10, color: i === 0 ? "var(--text)" : "var(--text3)" }}>
              {i === 0 ? (
                <span style={{ color: "var(--accent-text)", display: "flex" }}>
                  <Spinner size={15} />
                </span>
              ) : (
                <Circle size={15} />
              )}
              <span className="grow">{step}</span>
              <span style={{ fontSize: 12 }}>{i === 0 ? "w toku" : ""}</span>
            </div>
          ))}
        </div>
        <span style={{ fontSize: 13, color: "var(--text3)" }}>Duże archiwa (kilka tysięcy osób) wczytują się w sekundę lub dwie.</span>
      </div>
    </div>
  );
}
