// The Settings screen's dialogs: work in progress, the archive check, „Co nowego” and the keyboard cheat sheet.

import { CircleCheck } from "lucide-react";
import { Fragment } from "react";
import { useStore } from "../../app/store";
import { Spinner } from "../../components/bits";
import { Dialog } from "../../components/Dialog";
import { count } from "../../lib/format";
import { WHATS_NEW } from "./whatsNew";

/** Blocks the window while a backup or an export is written (it can't be cancelled halfway). */
export function BusyDialog({ text, note }: { text: string; note?: string }) {
  return (
    <Dialog width={400}>
      <div className="dialog-body" style={{ flexDirection: "row", alignItems: "center", gap: 14 }}>
        <span style={{ color: "var(--accent-text)", display: "flex" }}>
          <Spinner size={22} />
        </span>
        <span className="col" style={{ gap: 2 }}>
          <span style={{ fontSize: 15, fontWeight: 600 }}>{text}</span>
          {note && <span style={{ fontSize: 13, color: "var(--text2)" }}>{note}</span>}
        </span>
      </div>
    </Dialog>
  );
}

export interface CheckProblem {
  kind: "brokenLink" | "missingFile" | "noName" | "dates" | "ownAncestor" | "emptyFamily";
  message: string;
  /** The person to open, when the problem is about one. */
  id: string | null;
  name: string | null;
}

const KINDS: { kind: CheckProblem["kind"]; label: string }[] = [
  { kind: "brokenLink", label: "Odwołania do rekordów, których nie ma" },
  { kind: "missingFile", label: "Brakujące pliki" },
  { kind: "noName", label: "Osoby bez imienia i nazwiska" },
  { kind: "dates", label: "Daty urodzenia a daty rodziców" },
  { kind: "ownAncestor", label: "Pętle w drzewie" },
  { kind: "emptyFamily", label: "Puste rodziny" },
];

const SHOWN = 100;

/** „Sprawdź archiwum”: the problems grouped by kind, with links to the people they are about. */
export function CheckDialog({ problems, onClose }: { problems: CheckProblem[]; onClose: () => void }) {
  const go = useStore((s) => s.go);
  const open = (route: Parameters<typeof go>[0]) => {
    onClose();
    go(route);
  };
  return (
    <Dialog width={600} onClose={onClose}>
      <div className="dialog-body">
        {problems.length === 0 ? (
          <>
            <div className="dialog-icon" style={{ background: "var(--accent-soft)", color: "var(--accent-text)" }}>
              <CircleCheck size={19} />
            </div>
            <div className="dialog-title">Nie znaleziono problemów</div>
            <div className="dialog-text">Powiązania, daty i pliki w archiwum są spójne.</div>
          </>
        ) : (
          <>
            <div className="dialog-title">Znaleziono {count(problems.length, "problem", "problemy", "problemów")}</div>
            <div className="dialog-text">Nic nie zostało zmienione. Popraw je w profilach osób albo w programie, z którego pochodzi plik.</div>
            {KINDS.map(({ kind, label }) => {
              const items = problems.filter((p) => p.kind === kind);
              if (items.length === 0) return null;
              return (
                <div key={kind} className="check-group">
                  <div className="label-caps">
                    {label} · {items.length}
                  </div>
                  {items.slice(0, SHOWN).map((p, i) => (
                    <div key={i} className="check-row">
                      {p.id && (
                        <button className="link" onClick={() => open({ name: "person", id: p.id! })}>
                          {p.name ?? p.id}
                        </button>
                      )}
                      <span>{p.message}</span>
                    </div>
                  ))}
                  {items.length > SHOWN && <div className="check-row faint">…i jeszcze {items.length - SHOWN}</div>}
                  {kind === "missingFile" && (
                    <button className="link" style={{ alignSelf: "flex-start", fontSize: 13 }} onClick={() => open({ name: "missingFiles" })}>
                      Otwórz Brakujące pliki →
                    </button>
                  )}
                </div>
              );
            })}
          </>
        )}
      </div>
      <div className="dialog-foot">
        <button className="btn primary" onClick={onClose}>
          Zamknij
        </button>
      </div>
    </Dialog>
  );
}

export function WhatsNewDialog({ version, onClose }: { version: string; onClose: () => void }) {
  return (
    <Dialog width={560} onClose={onClose}>
      <div className="dialog-body">
        <div className="dialog-title">Co nowego w Heirloom {version}</div>
        <ul className="whats-new">
          {WHATS_NEW.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </div>
      <div className="dialog-foot">
        <button className="btn primary" onClick={onClose}>
          Zamknij
        </button>
      </div>
    </Dialog>
  );
}

/** The keys that exist (useShortcuts, the tree's keyboard handler, the command palette). */
const SHORTCUTS: { title: string; keys: [string[][], string][] }[] = [
  {
    title: "Wszędzie",
    keys: [
      [[["Ctrl", "K"]], "Szukaj osób, miejsc i poleceń"],
      [[["Ctrl", "E"]], "Włącz albo zakończ edycję"],
      [[["Ctrl", "S"]], "Zapisz zmiany (w trybie edycji)"],
      [[["Ctrl", "Z"]], "Cofnij ostatnią zmianę (w trybie edycji)"],
      [[["Ctrl", "Y"], ["Ctrl", "Shift", "Z"]], "Ponów cofniętą zmianę"],
      [[["Alt", "←"]], "Wstecz (także boczny przycisk myszy)"],
      [[["Alt", "→"]], "Dalej"],
      [[["Esc"]], "Zamknij okno lub menu"],
    ],
  },
  {
    title: "Drzewo",
    keys: [
      [[["↑"]], "Do rodzica"],
      [[["↓"]], "Do dziecka"],
      [[["←"], ["→"]], "Wybierz osobę obok: rodzeństwo, partnera"],
      [[["Enter"]], "Wybraną osobę na środek"],
      [[["Home"]], "Do osoby z największą liczbą powiązań"],
    ],
  },
  {
    title: "Wyszukiwanie (Ctrl K)",
    keys: [
      [[["↑"], ["↓"]], "Wybierz wynik"],
      [[["Enter"]], "Otwórz"],
    ],
  },
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog width={520} onClose={onClose}>
      <div className="dialog-body">
        <div className="dialog-title">Skróty klawiszowe</div>
        {SHORTCUTS.map((group) => (
          <div key={group.title} className="check-group">
            <div className="label-caps">{group.title}</div>
            {group.keys.map(([combos, text]) => (
              <div key={text} className="shortcut-row">
                <span className="shortcut-keys">
                  {combos.map((combo, i) => (
                    <Fragment key={combo.join("+")}>
                      {i > 0 && <span className="faint">lub</span>}
                      {combo.map((key) => (
                        <span key={key} className="kbd">
                          {key}
                        </span>
                      ))}
                    </Fragment>
                  ))}
                </span>
                <span>{text}</span>
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className="dialog-foot">
        <button className="btn primary" onClick={onClose}>
          Zamknij
        </button>
      </div>
    </Dialog>
  );
}
