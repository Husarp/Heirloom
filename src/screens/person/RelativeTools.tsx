// The tools for one relative in edit mode, the same on the tree's cards and in the profile's Rodzina: „Edytuj”, „Zmień
// pokrewieństwo” (what they are to the other person, and for a parent and child: biological, adopted…), „Odłącz”
// (only the link goes; the sentence in the question says exactly what changes) and „Usuń osobę z archiwum…”. Every
// change is one undoable edit like any other.

import { ArrowRightLeft, ChevronDown, Pencil, Trash2, Unlink } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { call } from "../../api/transport";
import { afterChange, useStore } from "../../app/store";
import { runEdit } from "../media/shared";
import type { RelationKind } from "./PersonalSection";
import "./profile.css";

/** A person as the tools name them. */
export interface RelativeRef {
  id: string;
  name: string;
  given: string;
  sex: string;
}

const KIND_WORDS: Record<RelationKind, [string, string, string]> = {
  parent: ["ojciec", "matka", "rodzic"],
  sibling: ["brat", "siostra", "rodzeństwo"],
  partner: ["partner, mąż", "partnerka, żona", "partner"],
  child: ["syn", "córka", "dziecko"],
};

/** „ojciec”, „siostra”…: the word for what `who` is, by their sex. */
export function kindWord(kind: RelationKind, sex: string): string {
  return KIND_WORDS[kind][sex === "M" ? 0 : sex === "F" ? 1 : 2];
}

const PEDIGREES: [string, string][] = [
  ["birth", "biologiczne"],
  ["adopted", "adopcja"],
  ["foster", "przybrane (wychowanie)"],
  ["unknown", "nieznane"],
];

/** The kind of a parent–child link as the tools use it, from GEDCOM's PEDI (the tree) or `person.relations`. */
export function pediKey(pedi: string | null | undefined): string {
  switch (pedi) {
    case "ADOPTED":
    case "adopted":
      return "adopted";
    case "FOSTER":
    case "foster":
      return "foster";
    case "OTHER":
    case "unknown":
      return "unknown";
    default:
      return "birth";
  }
}

const shortName = (p: RelativeRef) => p.given || p.name;

export function RelativeTools({
  of,
  relative,
  kind,
  family,
  pedi,
  variant,
  chip,
  // Elsewhere (the tree) an open profile section is finished first, so its „Anuluj” never takes these changes back.
  guard = (then) => useStore.getState().outsideSection(then),
  onGone,
}: {
  /** The person the relative is related to (the centre of the tree, the profile's person). */
  of: RelativeRef;
  relative: RelativeRef;
  /** What `relative` is to `of`. */
  kind: RelationKind;
  /** The family of a parent and child, for the kind of their link. */
  family?: string | null;
  pedi?: string | null;
  /** „strip”: icons attached under a tree card; „row”: the foot of a profile tile, with the kind as a chip. */
  variant: "strip" | "row";
  chip?: string;
  /** Runs an action when it may start (the profile opens its Rodzina section first). */
  guard?: (then: () => void) => void;
  /** The relative was unlinked or deleted. */
  onGone?: (id: string) => void;
}) {
  const go = useStore((s) => s.go);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  // Under a tree card the menu is laid over the whole window (the tree's world is zoomed and clipped), below the
  // button or above it when there is no room, and closes when the tree moves.
  const [spot, setSpot] = useState<{ left: number; top: number } | null>(null);
  useEffect(() => {
    if (!open) return;
    const inside = (e: Event) => ref.current?.contains(e.target as Node) || menuRef.current?.contains(e.target as Node);
    const onDown = (e: MouseEvent) => !inside(e) && setOpen(false);
    const onWheel = (e: WheelEvent) => !inside(e) && setOpen(false);
    // Esc closes the menu and nothing behind it: first, and marked done (as `useDismiss`).
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("wheel", onWheel, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("wheel", onWheel, true);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open]);
  useLayoutEffect(() => {
    if (!open || variant !== "strip") return setSpot(null);
    const button = ref.current?.querySelector<HTMLElement>("[aria-haspopup]")?.getBoundingClientRect();
    const menu = menuRef.current?.getBoundingClientRect();
    if (!button || !menu) return;
    const below = button.bottom + 4;
    const top = below + menu.height > window.innerHeight - 8 ? Math.max(8, button.top - 4 - menu.height) : below;
    setSpot({ left: Math.min(button.left, window.innerWidth - menu.width - 8), top });
  }, [open, variant]);
  const who = shortName(relative);
  const toWhom = shortName(of);
  // „Cofnij” in the message takes back this change only while nothing has changed since; otherwise it would take back
  // whatever came later (another message's change, say).
  const undoAction = () => {
    const version = useStore.getState().dataVersion;
    return {
      label: "Cofnij",
      run: () => {
        const s = useStore.getState();
        if (s.dataVersion === version) void s.undo();
        else s.notify("Ta wiadomość jest już nieaktualna: od tej zmiany było coś jeszcze. Cofaj przyciskiem „Cofnij” w pasku u góry (Ctrl Z).");
      },
    };
  };

  const edit = () => useStore.getState().requireEdit(() => go({ name: "person", id: relative.id, open: "personal" }));

  const change = (next: RelationKind) => {
    setOpen(false);
    if (next === kind) return;
    guard(() =>
      runEdit(async () => {
        await call("relation.change", { person: of.id, other: relative.id, kind: next });
        afterChange();
        useStore.getState().notify(`${who} to teraz: ${kindWord(next, relative.sex)} (dla: ${toWhom}).`, { action: undoAction() });
      }),
    );
  };

  const setPedi = (value: string) => {
    setOpen(false);
    if (!family || value === pediKey(pedi)) return;
    const child = kind === "child" ? relative.id : of.id;
    guard(() =>
      runEdit(async () => {
        await call("relation.setPedigree", { child, family, pedi: value });
        afterChange();
      }),
    );
  };

  const unlink = () =>
    guard(() =>
      runEdit(async () => {
        const preview = await call<{ title: string; text: string }>("relation.unlinkPreview", { person: of.id, other: relative.id });
        useStore.getState().setAsk({
          title: preview.title,
          text: preview.text,
          icon: "warn",
          buttons: [
            { label: "Anuluj", kind: "ghost" },
            {
              label: "Odłącz",
              kind: "danger",
              run: () =>
                runEdit(async () => {
                  await call("relation.remove", { person: of.id, other: relative.id });
                  afterChange();
                  onGone?.(relative.id);
                  useStore.getState().notify(`Odłączono: ${relative.name}.`, { action: undoAction() });
                }),
            },
          ],
        });
      }),
    );

  // Deleting takes the person out of every family (and texts only about them); photos and sources stay.
  const remove = () =>
    guard(() =>
      useStore.getState().setAsk({
        title: `Usunąć osobę ${relative.name} z archiwum?`,
        text: `To nie jest odłączenie: ${relative.name} zniknie z drzewa i list, razem ze wszystkimi powiązaniami w rodzinach i tekstami, które opisują tylko tę osobę. Zdjęcia i źródła zostaną w archiwum. Zmiana czeka na zapis — do tego czasu można ją cofnąć.`,
        icon: "warn",
        buttons: [
          { label: "Anuluj", kind: "ghost" },
          {
            label: "Usuń osobę",
            kind: "danger",
            run: () =>
              runEdit(async () => {
                await call("person.delete", { id: relative.id });
                afterChange();
                onGone?.(relative.id);
                useStore.getState().notify(`Usunięto: ${relative.name}.`, { action: undoAction() });
              }),
          },
        ],
      }),
    );

  const parentChild = (kind === "parent" || kind === "child") && !!family;
  const menuStyle: CSSProperties =
    variant === "strip" ? { position: "fixed", left: spot?.left ?? 0, top: spot?.top ?? 0, visibility: spot ? undefined : "hidden" } : { left: 0, top: 34 };
  const menu = open && (
    <div ref={menuRef} className="popover relative-menu" style={{ ...menuStyle, width: 240, padding: "4px 0", fontSize: 13 }} role="menu">
      <div className="relative-menu-question">
        Kim jest {who} dla: {toWhom}?
      </div>
      {(["parent", "sibling", "partner", "child"] as const).map((k) => (
        <button key={k} role="menuitemradio" aria-checked={k === kind} className={`menu-item${k === kind ? " on" : ""}`} onClick={() => change(k)}>
          {kindWord(k, relative.sex)}
        </button>
      ))}
      {parentChild && (
        <>
          <div className="menu-label" style={{ borderTop: "1px solid var(--border)", marginTop: 4 }}>
            Więź rodzic–dziecko
          </div>
          {PEDIGREES.map(([value, label]) => (
            <button key={value} role="menuitemradio" aria-checked={value === pediKey(pedi)} className={`menu-item${value === pediKey(pedi) ? " on" : ""}`} onClick={() => setPedi(value)}>
              {label}
            </button>
          ))}
        </>
      )}
    </div>
  );

  const changeTitle = `Zmień pokrewieństwo: kim ${who} jest dla: ${toWhom}`;
  return (
    <div ref={ref} className={`relative-tools ${variant}`} onClick={(e) => e.stopPropagation()} onDoubleClick={(e) => e.stopPropagation()}>
      {variant === "row" && (
        <button className="relation-chip" style={open ? { borderColor: "var(--accent)" } : undefined} onClick={() => setOpen((o) => !o)} aria-haspopup="menu" title={changeTitle}>
          <span className="ellipsis">{chip || kindWord(kind, relative.sex)}</span>
          <ChevronDown size={12} />
        </button>
      )}
      {variant === "row" && <span className="grow" />}
      <button className="icon-btn" title="Edytuj osobę" aria-label={`Edytuj: ${relative.name}`} onClick={edit}>
        <Pencil size={14} />
      </button>
      {variant === "strip" && (
        <button className={`icon-btn${open ? " on" : ""}`} title={changeTitle} aria-label={`Zmień pokrewieństwo: ${relative.name}`} aria-haspopup="menu" onClick={() => setOpen((o) => !o)}>
          <ArrowRightLeft size={14} />
        </button>
      )}
      <button className="icon-btn" title={`Odłącz od: ${toWhom} (osoba zostaje w archiwum)`} aria-label={`Odłącz: ${relative.name}`} onClick={unlink}>
        <Unlink size={14} />
      </button>
      <button className="icon-btn danger" title="Usuń osobę z archiwum…" aria-label={`Usuń z archiwum: ${relative.name}`} onClick={remove}>
        <Trash2 size={14} />
      </button>
      {variant === "strip" ? menu && createPortal(menu, document.body) : menu}
    </div>
  );
}
