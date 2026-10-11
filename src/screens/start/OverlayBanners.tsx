import { ArchiveRestore, FolderSync, TriangleAlert, Unplug, X } from "lucide-react";
import { useStore } from "../../app/store";
import { recheckOriginal, relocateOriginal } from "../../app/workMode";
import { BANNER_TEXT, startBanners, type BannerKind } from "../archive/workModes";

/** „Nie ruszaj oryginału” on Start (DESIGN A.6): what the archive doesn't have yet, what changed in it, and when it
 *  can't be reached. ✕ hides one until the next start. */
export function OverlayBanners() {
  const overlay = useStore((s) => s.archive?.overlay);
  const hidden = useStore((s) => s.hiddenBanners);
  const kinds = startBanners(overlay, hidden);
  if (!overlay || kinds.length === 0) return null;
  return (
    <div className="col" style={{ gap: 8 }}>
      {kinds.map((kind) => (
        <Banner key={kind} kind={kind} />
      ))}
    </div>
  );
}

function Banner({ kind }: { kind: BannerKind }) {
  const archive = useStore((s) => s.archive);
  const hide = useStore((s) => s.hideBanner);
  const startWriteBack = useStore((s) => s.startWriteBack);
  const startRebase = useStore((s) => s.startRebase);
  const o = archive?.overlay;
  if (!archive || !o) return null;

  if (kind === "originalMissing") return <OriginalMissingBanner onHide={() => hide(kind)} />;

  const shown =
    kind === "interrupted"
      ? { icon: <TriangleAlert size={16} color="var(--warn)" />, tone: "warn", text: o.interrupted ?? "", actions: [["Spróbuj ponownie", () => startWriteBack()]] as const }
      : kind === "originalChanged"
        ? { icon: <FolderSync size={16} color="var(--warn)" />, tone: "warn", text: BANNER_TEXT.originalChanged, actions: [["Wczytaj zmiany", startRebase]] as const }
        : { icon: <ArchiveRestore size={16} color="var(--accent-text)" />, tone: "info", text: BANNER_TEXT.pending(o.pending), actions: [["Przenieś do archiwum…", () => startWriteBack()]] as const };
  return (
    <div className={`banner ${shown.tone} overlay-banner`} role="status">
      <span style={{ flex: "none", display: "flex" }}>{shown.icon}</span>
      <span className="grow">{shown.text}</span>
      {shown.actions.map(([label, run]) => (
        <button key={label} className="btn secondary sm" onClick={run}>
          {label}
        </button>
      ))}
      <HideButton onClick={() => hide(kind)} />
    </div>
  );
}

function HideButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="icon-btn" style={{ width: 28, height: 28, flex: "none" }} title="Ukryj do następnego uruchomienia" aria-label="Ukryj" onClick={onClick}>
      <X size={14} />
    </button>
  );
}

/** The archive can't be opened (a pendrive unplugged): said on Start, and on Brakujące pliki instead of „Wskaż folder”. */
export function OriginalMissingBanner({ onHide }: { onHide?: () => void }) {
  const archive = useStore((s) => s.archive);
  if (!archive?.overlay?.originalMissing) return null;
  return (
    <div className="banner warn overlay-banner" role="status">
      <span style={{ flex: "none", display: "flex" }}>
        <Unplug size={16} color="var(--warn)" />
      </span>
      <span className="grow">{BANNER_TEXT.originalMissing(archive.root)}</span>
      <button className="btn secondary sm" onClick={recheckOriginal}>
        Sprawdź ponownie
      </button>
      <button className="btn secondary sm" onClick={() => void relocateOriginal()}>
        Wskaż, gdzie jest teraz…
      </button>
      {onHide && <HideButton onClick={onHide} />}
    </div>
  );
}
