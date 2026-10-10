// „Ułożyć pliki w folderach osób?” / „Wyjąć pliki z folderów osób?” (DESIGN B.7): the question when the layout of
// media/ is switched. „Tylko nowe pliki” stores the choice and moves nothing; „Przenieś pliki” goes on to the
// final-change dialog.

import { FolderTree } from "lucide-react";
import { useEffect, useState } from "react";
import type { ApiError } from "../../api/transport";
import type { MediaLayout, RestructurePlan } from "../../api/types";
import { moveFiles, previewMove, setLayoutOnly } from "../../app/layout";
import { useStore } from "../../app/store";
import { Dialog } from "../../components/Dialog";
import { Spinner } from "../../components/bits";
import { switchBullets, switchText, switchTitle } from "./layoutText";

export function SwitchLayoutDialog({ to, onClose }: { to: MediaLayout; onClose: () => void }) {
  const dataFile = useStore((s) => s.archive?.dataFile ?? "");
  const notify = useStore((s) => s.notify);
  const [plan, setPlan] = useState<RestructurePlan | null>(null);
  useEffect(() => {
    let current = true;
    previewMove({ to })
      .then((p) => current && setPlan(p))
      .catch((e: ApiError) => {
        if (!current) return;
        notify(e.message, { kind: "err" });
        onClose();
      });
    return () => {
      current = false;
    };
  }, [to]); // eslint-disable-line react-hooks/exhaustive-deps

  const storeOnly = async () => {
    if (await setLayoutOnly(to)) onClose();
  };
  const move = () => {
    if (!plan) return;
    onClose();
    moveFiles({ to }, plan.count);
  };
  const bullets = plan ? switchBullets(to, plan, dataFile) : [];

  return (
    <Dialog width={560} onClose={onClose} labelledBy="switch-title">
      <div className="dialog-body" style={{ flexDirection: "row", alignItems: "flex-start", gap: 14, overflow: "auto" }}>
        <div className="dialog-icon" style={{ flex: "none" }}>
          <FolderTree size={19} />
        </div>
        <div className="col" style={{ gap: 8 }}>
          <div id="switch-title" className="dialog-title">
            {switchTitle(to)}
          </div>
          {plan ? (
            <>
              <div className="dialog-text">{switchText(to, plan)}</div>
              {bullets.length > 0 && (
                <ul className="dialog-text" style={{ margin: 0, paddingLeft: 20, display: "flex", flexDirection: "column", gap: 4 }}>
                  {bullets.map((b) => (
                    <li key={b}>{b}</li>
                  ))}
                </ul>
              )}
            </>
          ) : (
            <div className="row dialog-text" style={{ gap: 8 }}>
              <Spinner size={14} /> Sprawdzam, ile plików jest do przeniesienia…
            </div>
          )}
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={onClose}>
          Anuluj
        </button>
        <span className="grow" />
        <button className="btn secondary" disabled={!plan} onClick={storeOnly}>
          Tylko nowe pliki
        </button>
        {plan && plan.count > 0 && (
          <button className="btn primary" onClick={move}>
            Przenieś pliki
          </button>
        )}
      </div>
    </Dialog>
  );
}
