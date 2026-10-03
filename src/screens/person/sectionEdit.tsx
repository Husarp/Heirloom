// Editing a profile section in place (design 17a): in edit mode each section has „Edytuj sekcję”; one section is open
// at a time; „Gotowe” keeps its changes (as unsaved changes, saved with „Zapisz” in the bar) and „Anuluj” or Esc takes
// back what was changed while it was open. A text still being typed in the section is never dropped without asking.

import { Check, Pencil } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";
import { useStore } from "../../app/store";

export interface SectionEdit {
  /** This section is being edited. */
  open: boolean;
  /** Another section is open, so this one waits. */
  blocked: boolean;
  /** Opens the section, then runs `then` (e.g. starts adding a text). */
  start: (then?: () => void) => void;
  done: () => void;
  cancel: () => void;
}

/** Asks before a text being typed (it sets the leave guard) is dropped; then runs `then`. */
function afterDraft(then: () => void) {
  const { leaveGuard, setAsk, setLeaveGuard } = useStore.getState();
  if (!leaveGuard) return then();
  setAsk({
    title: leaveGuard,
    text: "Tekst wpisany w tej sekcji nie został jeszcze dodany. Wróć i kliknij „Dodaj tekst” albo „Zmień tekst”, żeby go zachować.",
    icon: "warn",
    buttons: [
      { label: "Wróć do sekcji", kind: "ghost" },
      {
        label: "Odrzuć tekst",
        kind: "danger",
        run: () => {
          setLeaveGuard(null);
          then();
        },
      },
    ],
  });
}

/** `key` is unique on the page (the person's id and the section). */
export function useSection(key: string, label: string): SectionEdit {
  const section = useStore((s) => s.section);
  const open = section?.key === key;
  return {
    open,
    blocked: !!section && !open,
    start: (then) =>
      useStore.getState().requireEdit(() => {
        if (useStore.getState().section?.key === key || useStore.getState().openSection(key, label)) then?.();
      }),
    done: () => afterDraft(() => useStore.getState().finishSection()),
    cancel: () => afterDraft(() => void useStore.getState().cancelSection()),
  };
}

/** „✎ Edytuj sekcję” in the header in edit mode, „edytujesz tę sekcję” while open. */
export function SectionEditButton({ edit }: { edit: SectionEdit }) {
  const mode = useStore((s) => s.mode);
  const openLabel = useStore((s) => s.section?.label);
  if (mode !== "edit") return null;
  if (edit.open) return <span className="editing-tag">edytujesz tę sekcję</span>;
  return (
    <button
      className="section-edit"
      disabled={edit.blocked}
      title={edit.blocked ? `Najpierw zakończ edycję sekcji „${openLabel}”` : undefined}
      onClick={() => edit.start()}
    >
      <Pencil size={14} />
      Edytuj sekcję
    </button>
  );
}

/** Something else on screen takes Esc and Ctrl Enter first: a dialog, the search, an open menu. */
function somethingOnTop(): boolean {
  const s = useStore.getState();
  return !!s.ask || s.conflictOpen || s.foreignConfirmOpen || s.whoEditsOpen || s.paletteOpen || !!document.querySelector(".popover, .dialog");
}

/** The foot of an open section: the keys, „Anuluj” and „Gotowe”. Esc and Ctrl Enter work anywhere on the page except
 *  inside a text being written or the relative being added (they have their own buttons), and not while a dialog, the
 *  search or a menu is open. */
export function SectionFoot({
  onCancel,
  onDone,
  busy,
  doneLabel = "Gotowe",
  left,
}: {
  onCancel: () => void;
  onDone: () => void;
  busy?: boolean;
  doneLabel?: string;
  left?: ReactNode;
}) {
  const handlers = useRef({ onCancel, onDone });
  handlers.current = { onCancel, onDone };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || somethingOnTop()) return;
      if ((e.target as HTMLElement | null)?.closest?.(".text-form, .add-relative")) return;
      if (e.key === "Escape") {
        e.preventDefault();
        handlers.current.onCancel();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handlers.current.onDone();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <div className="section-foot">
      {left}
      <span className="section-foot-hint">Esc anuluje sekcję · Ctrl Enter gotowe · plik zapisujesz w pasku u góry</span>
      <span className="grow" />
      <button className="btn secondary" disabled={busy} onClick={onCancel}>
        Anuluj
      </button>
      <button className="btn primary" disabled={busy} onClick={onDone}>
        <Check size={14} />
        {doneLabel}
      </button>
    </div>
  );
}
