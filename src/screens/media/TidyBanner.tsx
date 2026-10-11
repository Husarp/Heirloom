// Start: the two calm notes about the files in media/ (DESIGN B.5, B.6). Neither is a modal and neither moves anything.

import { FolderSearch, TriangleAlert } from "lucide-react";
import { askUndo, finishMove, tidyNotNow } from "../../app/layout";
import { useStore } from "../../app/store";
import { bannerShows, bannerText, interruptedText } from "./layoutText";

export function TidyBanners() {
  const scan = useStore((s) => s.scan);
  const archive = useStore((s) => s.archive);
  const setTidyOpen = useStore((s) => s.setTidyOpen);
  const combined = archive?.combined != null;
  // The scan is the fresher of the two; the status says it from the opening until the first scan.
  const left = (scan ? scan.interrupted : archive?.layoutInterrupted) ?? null;
  const showTidy = bannerShows(scan, combined);
  if (combined || (!left && !showTidy)) return null;
  return (
    <div className="col" style={{ gap: 8 }}>
      {left && (
        <div className="banner warn overlay-banner" role="status">
          <span style={{ flex: "none", display: "flex" }}>
            <TriangleAlert size={16} color="var(--warn)" />
          </span>
          <span className="grow">{interruptedText(left)}</span>
          <button className="btn secondary sm" onClick={() => finishMove(left.total - left.done)}>
            Dokończ
          </button>
          <button className="btn secondary sm" onClick={() => askUndo(left.journal)}>
            Cofnij przeniesione
          </button>
        </div>
      )}
      {showTidy && scan && (
        <div className="banner info overlay-banner" role="status">
          <span style={{ flex: "none", display: "flex" }}>
            <FolderSearch size={16} color="var(--accent-text)" />
          </span>
          <span className="grow">{bannerText(scan)}</span>
          <button className="btn secondary sm" onClick={() => setTidyOpen(true)}>
            Przejrzyj
          </button>
          <button className="btn secondary sm" onClick={() => void tidyNotNow(scan)}>
            Nie teraz
          </button>
        </div>
      )}
    </div>
  );
}
