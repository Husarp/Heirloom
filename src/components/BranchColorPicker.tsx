// Choosing a branch's colour (Ustawienia › Drzewo › Kolory gałęzi, and the Nazwiska page of a surname): the picker of
// the 12 colours, and the reading and writing of the choices kept in the archive's display settings.

import { Check } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { call } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { useStore } from "../app/store";
import { BranchShape, useDismiss } from "./bits";

/** Branch colour names (spec §1.2) for „Gałąź 1 · Terakota”. */
export const BRANCH_NAMES = ["Terakota", "Morska", "Fiolet", "Ochra", "Błękit", "Oliwka", "Morwa", "Łupek", "Bursztyn", "Turkus", "Wiśnia", "Szałwia"];

/** The colours the user chose, from the archive's display settings `branchColors` (surname group key → 1–12). */
export function chosenBranchColors(display: Record<string, unknown> | undefined): Record<string, number> {
  const raw = display?.branchColors;
  const out: Record<string, number> = {};
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return out;
  for (const [key, value] of Object.entries(raw)) {
    if (typeof value === "number" && Number.isInteger(value) && value >= 1 && value <= 12) out[key] = value;
  }
  return out;
}

/** The chosen colours of the open archive and how to change them. Display settings aren't family data, so they can
 *  be changed in browse mode too; archives opened together can't (`canChange` false). */
export function useBranchColors() {
  const display = useStore((s) => s.archive?.display);
  const combined = useStore((s) => s.archive?.combined != null);
  const chosen = useMemo(() => chosenBranchColors(display), [display]);
  const write = async (next: Record<string, number>) => {
    if (combined) return;
    try {
      // The whole object is sent: the backend replaces a display key, it doesn't merge into it.
      useStore.getState().changed(await call<ArchiveStatus>("archive.setSettings", { display: { branchColors: next } }));
    } catch (e) {
      useStore.getState().notify((e as Error).message, { kind: "err" });
    }
  };
  return {
    chosen,
    canChange: !combined,
    set: (key: string, colour: number) => write({ ...chosen, [key]: colour }),
    /** Back to the automatic colour of one branch. */
    clear: (key: string) => {
      const { [key]: _removed, ...rest } = chosen;
      return write(rest);
    },
    /** Back to the automatic colours of all branches. */
    reset: () => write({}),
  };
}

/** A button that opens the 12 branch colours to choose from (each as its shape), and „Automatycznie” to give the
 *  choice up. `value` is the colour the branch has now; `chosen` says whether it was chosen (else it is automatic). */
export function BranchColorPicker({
  value,
  chosen,
  onPick,
  onAuto,
  disabled,
  label,
  align = "right",
  className = "chip",
  children,
}: {
  value: number;
  chosen: boolean;
  onPick: (colour: number) => void;
  onAuto: () => void;
  disabled?: boolean;
  /** For screen readers and the tooltip: „Kolor gałęzi Kowalscy”. */
  label: string;
  align?: "left" | "right";
  className?: string;
  /** What the button shows. */
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  return (
    <div ref={ref} className="bcp">
      <button className={className} disabled={disabled} title={label} aria-label={label} aria-haspopup="true" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        {children}
      </button>
      {open && (
        <div className="popover bcp-pop" style={{ [align]: 0 }}>
          <div className="bcp-grid" role="group" aria-label={label}>
            {Array.from({ length: 12 }, (_, i) => i + 1).map((n) => (
              <button
                key={n}
                className={`bcp-swatch${chosen && n === value ? " on" : ""}`}
                title={`${n} · ${BRANCH_NAMES[n - 1]}`}
                aria-label={`${n} · ${BRANCH_NAMES[n - 1]}`}
                aria-pressed={chosen && n === value}
                onClick={() => {
                  onPick(n);
                  setOpen(false);
                }}
              >
                <BranchShape branch={n} size={16} />
              </button>
            ))}
          </div>
          <button
            className={`menu-item${chosen ? "" : " on"}`}
            onClick={() => {
              onAuto();
              setOpen(false);
            }}
          >
            {!chosen && <Check size={14} />}
            <span className="grow">Automatycznie</span>
          </button>
        </div>
      )}
    </div>
  );
}
