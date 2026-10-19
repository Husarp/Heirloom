// The profile-photo star (a photo's top-right corner) and the people tag that marks whose profile photo a photo is.
// Used by the profile's gallery and the import's step 4. No API calls in here: the caller does them in `onSet` and
// `onUnset`, so each screen keeps its own way of saving (and its own undo).
//
// Put the star inside a positioned box with class `has-profile-star` (the outlined star shows while the box is hovered
// or focused) and next to — not inside — any button that opens the photo.

import { Check, Star } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { profileChange } from "./profileDiff";
import { clickRow } from "./tickRange";
import "./ProfileStar.css";

export interface StarPerson {
  id: string;
  name: string;
}

/** The tooltip of a people tag whose profile photo this is. */
export const PROFILE_TAG_TITLE = "Zdjęcie profilowe tej osoby";

/** A person's name under a photo. When it is their profile photo the tag gets the star's green border and a star
 *  before the name, with the tooltip „Zdjęcie profilowe tej osoby”. `compact`: a smaller one for a line under a tile. */
export function ProfileTag({ name, profile, compact }: { name: string; profile: boolean; compact?: boolean }) {
  return (
    <span className={`profile-tag${profile ? " on" : ""}${compact ? " compact" : ""}`} title={profile ? PROFILE_TAG_TITLE : name}>
      {profile && <Star size={compact ? 10 : 12} fill="currentColor" style={{ flex: "none" }} />}
      <span className="profile-tag-name">{name}</span>
    </span>
  );
}

/** `people`: everyone on the photo. `profileOf`: the ids of those whose profile photo it is. `current`: the person the
 *  screen is about (the star is filled when it is their profile photo, and they are ticked first); without it the star is
 *  filled when it is anybody's. `editable` false: only a filled star shows, and it does nothing.
 *  - With `current`: outlined star: one person on the photo → `onSet` at once; more → a list to tick („Dalej”,
 *    „Wszystkim”). Filled star: one person has it → `onUnset` at once; more → a list to tick whose it stops being.
 *  - Without `current` and with several people on the photo: either star opens one list of everyone, ticked = those
 *    who have it; „Dalej” applies the difference (`onSet` for the newly ticked, `onUnset` for the unticked). */
export function ProfileStar({
  people,
  profileOf,
  current,
  editable,
  onSet,
  onUnset,
}: {
  people: StarPerson[];
  profileOf: string[];
  current?: string;
  editable: boolean;
  onSet: (ids: string[]) => void;
  onUnset: (ids: string[]) => void;
}) {
  const [pop, setPop] = useState<"set" | "unset" | "diff" | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const filled = current ? profileOf.includes(current) : profileOf.length > 0;
  if (!filled && !editable) return null;
  if (!editable) {
    return (
      <span className="profile-star filled" title="Zdjęcie profilowe">
        <Star size={11} fill="currentColor" />
      </span>
    );
  }

  const click = () => {
    if (pop) return setPop(null);
    if (!current && people.length > 1) setPop("diff");
    else if (filled) {
      if (profileOf.length <= 1) onUnset(profileOf);
      else setPop("unset");
    } else if (people.length <= 1) {
      const who = people[0]?.id ?? current;
      if (who) onSet([who]);
    } else {
      setPop("set");
    }
  };
  const named = (id: string) => people.find((p) => p.id === id) ?? { id, name: id };
  const label = filled ? "Zdejmij zdjęcie profilowe" : "Ustaw jako zdjęcie profilowe";
  return (
    <>
      <button
        ref={button}
        type="button"
        className={`profile-star ${filled ? "filled" : "outline"}${pop ? " open" : ""}`}
        title={filled ? "Zdjęcie profilowe — kliknij, aby zdjąć" : "Ustaw jako zdjęcie profilowe"}
        aria-label={label}
        aria-haspopup={people.length > 1 || profileOf.length > 1 ? "dialog" : undefined}
        aria-expanded={pop ? true : undefined}
        onClick={click}
      >
        <Star size={11} fill={filled ? "currentColor" : "none"} />
      </button>
      {pop === "set" && (
        <PeoplePopover
          anchor={button.current}
          title="Zdjęcie profilowe dla:"
          people={people}
          marked={profileOf}
          initial={current ? [current] : []}
          actions={[
            { label: "Dalej", run: onSet, ticked: true },
            { label: "Wszystkim", run: onSet, all: true },
          ]}
          onClose={() => setPop(null)}
        />
      )}
      {pop === "diff" && (
        <PeoplePopover
          anchor={button.current}
          title="Zdjęcie profilowe dla:"
          people={people}
          marked={[]}
          initial={profileOf}
          actions={[
            { label: "Dalej", run: () => {}, diff: { profileOf, onSet, onUnset } },
            { label: "Wszystkim", run: onSet, all: true },
          ]}
          onClose={() => setPop(null)}
        />
      )}
      {pop === "unset" && (
        <PeoplePopover
          anchor={button.current}
          title="Przestaje być zdjęciem profilowym dla:"
          people={profileOf.map(named)}
          marked={[]}
          initial={profileOf}
          actions={[{ label: "Dalej", run: onUnset, ticked: true }]}
          onClose={() => setPop(null)}
        />
      )}
    </>
  );
}

interface PopAction {
  label: string;
  run: (ids: string[]) => void;
  /** Runs with the ticked people… */
  ticked?: boolean;
  /** …or with everyone. */
  all?: boolean;
  /** …or applies the difference between those who have it (`profileOf`) and the ticked ones. */
  diff?: { profileOf: string[]; onSet: (ids: string[]) => void; onUnset: (ids: string[]) => void };
}

const POP_W = 270;

/** The list with checkboxes, in a portal over everything, under the star (above it when there is no room). Click or
 *  Ctrl+click toggles a row, Shift+click ticks the range from the last clicked row. Esc and a click outside close it. */
function PeoplePopover({
  anchor,
  title,
  people,
  marked,
  initial,
  actions,
  onClose,
}: {
  anchor: HTMLElement | null;
  title: string;
  people: StarPerson[];
  /** Those who already have this as their profile photo (a small star beside the name). */
  marked: string[];
  initial: string[];
  actions: PopAction[];
  onClose: () => void;
}) {
  const ids = people.map((p) => p.id);
  const [ticked, setTicked] = useState<string[]>(() => initial.filter((id) => ids.includes(id)));
  const last = useRef<number | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number }>({ left: -9999, top: 0 });

  const place = () => {
    const rect = anchor?.getBoundingClientRect();
    const height = box.current?.offsetHeight ?? 0;
    if (!rect) return;
    const left = Math.max(8, Math.min(rect.right - POP_W, window.innerWidth - POP_W - 8));
    const below = rect.bottom + 6;
    const top = below + height > window.innerHeight - 8 ? Math.max(8, rect.top - 6 - height) : below;
    setAt((p) => (p.left === left && p.top === top ? p : { left, top }));
  };
  useLayoutEffect(place);
  useEffect(() => {
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    const down = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!box.current?.contains(t) && !anchor?.contains(t)) onClose();
    };
    // Esc closes the list and nothing behind it (the open section watches for a key already handled).
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      onClose();
      anchor?.focus();
    };
    window.addEventListener("mousedown", down);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("mousedown", down);
      window.removeEventListener("keydown", key, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [anchor, onClose]);
  useEffect(() => box.current?.querySelector<HTMLElement>("[role=checkbox]")?.focus(), []);

  const change = (a: PopAction) => (a.diff ? profileChange(a.diff.profileOf, ticked) : null);
  const run = (a: PopAction) => {
    const diff = change(a);
    if (a.diff && diff) {
      if (!diff.set.length && !diff.unset.length) return;
      onClose();
      if (diff.unset.length) a.diff.onUnset(diff.unset);
      if (diff.set.length) a.diff.onSet(diff.set);
      return;
    }
    const who = a.all ? ids : ticked;
    if (!who.length) return;
    onClose();
    a.run(who);
  };
  return createPortal(
    <div ref={box} className="popover profile-star-pop" role="dialog" aria-label={title} style={{ left: at.left, top: at.top, width: POP_W }} onPointerDown={(e) => e.stopPropagation()}>
      <div className="ps-title">{title}</div>
      <div className="ps-list">
        {people.map((p, i) => {
          const on = ticked.includes(p.id);
          return (
            <button
              key={p.id}
              type="button"
              role="checkbox"
              aria-checked={on}
              className={`ps-row${on ? " on" : ""}`}
              onClick={(e) => {
                setTicked(clickRow(ids, ticked, i, last.current, e.shiftKey));
                last.current = i;
              }}
            >
              <span className="ps-box">{on && <Check size={12} strokeWidth={3} />}</span>
              <span className="ellipsis grow">{p.name}</span>
              {marked.includes(p.id) && <Star size={11} fill="currentColor" className="ps-had" aria-label="ma już to zdjęcie profilowe" />}
            </button>
          );
        })}
      </div>
      <div className="ps-actions">
        {actions.map((a, n) => (
          <button key={a.label} type="button" className={`btn sm ${n === 0 ? "primary" : "secondary"}`} disabled={a.diff ? !change(a)!.set.length && !change(a)!.unset.length : !a.all && ticked.length === 0} onClick={() => run(a)}>
            {a.label}
          </button>
        ))}
      </div>
    </div>,
    document.body,
  );
}
