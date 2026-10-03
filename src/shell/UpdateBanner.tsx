import { Download, X } from "lucide-react";
import { useStore } from "../app/store";
import { applyUpdate, downloadText, useUpdates } from "../app/updates";
import { openUrl } from "../lib/native";

/** A newer Heirloom on GitHub (APP-STANDARDS.md §2): the version, „Aktualizuj”, and ✕, which hides it until Heirloom
 *  next starts. Shown on Start and on the archive picker, only while „Sprawdzaj aktualizacje” is on (otherwise an
 *  update found by „Sprawdź teraz” is offered in Ustawienia › O programie). */
export function UpdateBanner() {
  const on = useStore((s) => s.app?.updates.check ?? false);
  const status = useUpdates((s) => s.status);
  const closedFor = useUpdates((s) => s.closedFor);
  const failed = useUpdates((s) => s.failed);
  const installing = useUpdates((s) => s.installing);
  if (!on || !status?.newer || !status.latest || closedFor === status.latest) return null;
  const progress = downloadText(status);
  const d = status.download;
  return (
    <div className="banner info update-banner" role="status">
      <Download size={16} style={{ flex: "none", color: "var(--accent-text)" }} />
      <span className="grow" style={{ minWidth: 0 }}>
        {failed ? (
          <>
            <b>Nie udało się zaktualizować.</b> {failed}
          </>
        ) : installing ? (
          "Uruchamiam instalator — Heirloom zaraz się zamknie."
        ) : progress ? (
          progress
        ) : (
          <>
            <b>Jest nowa wersja Heirloom {status.latest}.</b> Masz {status.current}.
          </>
        )}
      </span>
      {d.state === "running" && d.total > 0 && (
        <span className="update-progress" aria-hidden>
          <span style={{ width: `${Math.round((d.done / d.total) * 100)}%` }} />
        </span>
      )}
      {failed && (
        <button className="btn ghost sm" onClick={() => void openUrl(status.page)}>
          GitHub
        </button>
      )}
      <button className="btn primary sm" disabled={d.state === "running" || installing} onClick={() => void applyUpdate()}>
        {failed ? "Spróbuj ponownie" : "Aktualizuj"}
      </button>
      <button className="icon-btn" title="Ukryj do następnego uruchomienia" aria-label="Ukryj" onClick={() => useUpdates.setState({ closedFor: status.latest })}>
        <X size={16} />
      </button>
    </div>
  );
}
