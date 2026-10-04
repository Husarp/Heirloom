// Small shared pieces from the design's component inventory (spec §5).

import { Check, ChevronDown } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { mediaUrl } from "../api/transport";
import type { CombinedArchive, DateText } from "../api/types";
import { useStore } from "../app/store";

/** Avatar: a photo, or initials on the branch colour's soft tint (spec §5.6). `from`: in archives opened together,
 *  the archive's dot in the corner (nothing in a single archive). */
export function Avatar({
  initials,
  branch,
  size = 32,
  photo,
  dashed,
  tint = 24,
  style,
  from,
}: {
  initials: string;
  branch?: number;
  size?: number;
  photo?: string | null;
  dashed?: boolean;
  tint?: number;
  style?: CSSProperties;
  from?: string[];
}) {
  const avatar = <AvatarImage initials={initials} branch={branch} size={size} photo={photo} dashed={dashed} tint={tint} style={style} />;
  if (!from?.length) return avatar;
  return (
    <span className="avatar-wrap">
      {avatar}
      <ArchiveDot from={from} size={size >= 64 ? 20 : size >= 40 ? 15 : 12} />
    </span>
  );
}

function AvatarImage({
  initials,
  branch,
  size,
  photo,
  dashed,
  tint,
  style,
}: {
  initials: string;
  branch?: number;
  size: number;
  photo?: string | null;
  dashed?: boolean;
  tint: number;
  style?: CSSProperties;
}) {
  const [failed, setFailed] = useState(false);
  const fontSize = size >= 48 ? 16 : size >= 44 ? 14 : size >= 40 ? 13 : size >= 34 ? 12 : size >= 26 ? 11 : 10;
  const background = branch ? `color-mix(in oklab, var(--b${branch}) ${tint}%, var(--surface))` : "var(--surface2)";
  return (
    <span
      className="avatar"
      style={{
        width: size,
        height: size,
        fontSize,
        background,
        border: dashed ? "1.5px dashed var(--accent)" : undefined,
        ...style,
      }}
    >
      {photo && !failed ? (
        <img src={mediaUrl(photo, "thumb", size > 64 ? 256 : 128)} alt="" loading="lazy" onError={() => setFailed(true)} />
      ) : (
        initials
      )}
    </span>
  );
}

/** The branch colour each archive of a set is drawn in (`--arch1` … in tokens.css, the tree's „Koloruj wg:
 *  archiwum”). */
export const ARCH_BRANCH = [5, 9, 3, 6, 7, 10];

/** The branch colour number of the archive with this 1-based colour. */
export function archBranch(colour: number): number {
  return ARCH_BRANCH[(colour - 1) % ARCH_BRANCH.length];
}

/** The archives of the set that is open, by key; empty for a single archive. */
export function useArchives(): Map<string, CombinedArchive> {
  const archives = useStore((s) => s.archive?.combined?.archives);
  return useMemo(() => new Map((archives ?? []).map((a) => [a.key, a])), [archives]);
}

/** „z archiwum Kowalscy”, „w archiwach Kowalscy i Nowakowie”. */
export function fromText(from: string[], archives: Map<string, CombinedArchive>): string {
  const names = from.map((k) => archives.get(k)?.name ?? k);
  if (names.length === 1) return `z archiwum ${names[0]}`;
  return `w archiwach ${names.slice(0, -1).join(", ")} i ${names[names.length - 1]}`;
}

/** A round dot in the colour of the archive a person comes from (with its initial when there's room); a person in
 *  several archives gets their colours side by side. */
export function ArchiveDot({ from, size = 12, style }: { from: string[] | undefined; size?: number; style?: CSSProperties }) {
  const archives = useArchives();
  if (!from?.length || archives.size === 0) return null;
  const colours = from.map((k) => archives.get(k)?.colour ?? 1);
  const background =
    colours.length === 1
      ? `var(--arch${((colours[0] - 1) % 6) + 1})`
      : `conic-gradient(${colours.map((c, i) => `var(--arch${((c - 1) % 6) + 1}) ${(i / colours.length) * 360}deg ${((i + 1) / colours.length) * 360}deg`).join(", ")})`;
  const letter = colours.length === 1 && size >= 15 ? (archives.get(from[0])?.name ?? "").charAt(0).toUpperCase() : "";
  const ink = [4, 6, 9, 10].includes(archBranch(colours[0])) ? "var(--lod-ink-dk)" : "var(--lod-ink)";
  return (
    <span className="arch-dot" title={fromText(from, archives)} style={{ width: size, height: size, background, color: ink, fontSize: Math.round(size * 0.6), ...style }}>
      {letter}
    </span>
  );
}

/** The archive (or archives) a person comes from, with the names: on the profile and in the tree's panel. */
export function ArchiveBadge({ from, style }: { from: string[] | undefined; style?: CSSProperties }) {
  const archives = useArchives();
  if (!from?.length || archives.size === 0) return null;
  return (
    <span className="arch-badge" title={fromText(from, archives)} style={style}>
      <ArchiveDot from={from} size={12} style={{ boxShadow: "none" }} />
      <span>{from.map((k) => archives.get(k)?.name ?? k).join(" + ")}</span>
    </span>
  );
}

/** The branch mark: circle, square or diamond by `b % 3` (spec §1.2), so colour isn't the only cue. */
export function BranchShape({ branch, size = 10, style }: { branch: number; size?: number; style?: CSSProperties }) {
  const shape = branch % 3;
  return (
    <span
      style={{
        display: "inline-block",
        width: size,
        height: size,
        flex: "none",
        background: `var(--b${branch})`,
        borderRadius: shape === 0 ? "50%" : shape === 1 ? 2 : 1,
        transform: shape === 2 ? "rotate(45deg) scale(0.85)" : undefined,
        ...style,
      }}
    />
  );
}

/** One date side ("ok. 1850") with the dotted underline when uncertain. */
export function DateBit({ date, field = "year" }: { date: DateText | null | undefined; field?: "year" | "short" | "text" }) {
  if (!date) return <span>?</span>;
  return <span className={date.uncertain ? "uncertain" : undefined}>{date[field]}</span>;
}

/** "1878–1951", "ok. 1850 – przed 1910", "ur. 1931 · żyje". */
export function Lifespan({
  birth,
  death,
  living,
  field = "year",
  livingWord = "żyje",
}: {
  birth: DateText | null;
  death: DateText | null;
  living?: boolean;
  field?: "year" | "short";
  livingWord?: string;
}) {
  if (!birth && !death) return living ? <span>{livingWord}</span> : null;
  const spaced = (birth?.[field].includes(" ") ?? false) || (death?.[field].includes(" ") ?? false) || !birth || (!death && !living);
  const dash = spaced ? " – " : "–";
  return (
    <span className="num">
      <DateBit date={birth} field={field} />
      {dash}
      {death ? <DateBit date={death} field={field} /> : living ? livingWord : "?"}
    </span>
  );
}

/** Years on a person card (design v2, A9): "1878 † 1951", "1931 · żyje", each side dotted when uncertain. */
export function CardYears({ birth, death, living }: { birth: DateText | null; death: DateText | null; living: boolean }) {
  if (!birth && !death) return living ? <span>żyje</span> : null;
  return (
    <span className="num card-years">
      {birth ? <DateBit date={birth} /> : <span>?</span>}
      {death ? (
        <span>
          † <DateBit date={death} />
        </span>
      ) : (
        <span>{living ? "· żyje" : "† ?"}</span>
      )}
    </span>
  );
}

export interface SegOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
  title?: string;
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  variant = "accent",
  full,
  size,
  style,
}: {
  options: SegOption<T>[];
  value: T;
  onChange: (value: T) => void;
  variant?: "accent" | "neutral";
  full?: boolean;
  size?: number;
  style?: CSSProperties;
}) {
  // One stop for Tab; ← → change the option (design 17g).
  const onKey = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    // Alt ← → is Back / Forward, not the next option.
    if ((e.key !== "ArrowLeft" && e.key !== "ArrowRight") || e.altKey || e.ctrlKey || e.metaKey) return;
    e.preventDefault();
    const at = options.findIndex((o) => o.value === value);
    const next = options[(at + (e.key === "ArrowLeft" ? options.length - 1 : 1)) % options.length];
    if (!next) return;
    onChange(next.value);
    const buttons = e.currentTarget.querySelectorAll<HTMLButtonElement>("button");
    buttons[options.indexOf(next)]?.focus();
  };
  return (
    <div className={`seg ${variant}${full ? " full" : ""}`} style={style} role="radiogroup" onKeyDown={onKey}>
      {options.map((o) => (
        <button
          key={o.value}
          className={o.value === value ? "on" : ""}
          onClick={() => onChange(o.value)}
          title={o.title}
          role="radio"
          aria-checked={o.value === value}
          tabIndex={o.value === value || !options.some((x) => x.value === value) ? 0 : -1}
          style={size ? { height: size } : undefined}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Checkbox({ on, onChange, disabled }: { on: boolean; onChange?: (on: boolean) => void; disabled?: boolean }) {
  return (
    <button
      className={`check${on ? " on" : ""}`}
      role="checkbox"
      aria-checked={on}
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onChange?.(!on);
      }}
    >
      {on && <Check size={12} strokeWidth={3} />}
    </button>
  );
}

export function Toggle({ on, onChange, disabled }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return <button className={`toggle${on ? " on" : ""}`} role="switch" aria-checked={on} disabled={disabled} onClick={() => onChange(!on)} />;
}

export function Tabs<T extends string>({
  tabs,
  value,
  onChange,
  right,
}: {
  tabs: { value: T; label: ReactNode; count?: number }[];
  value: T;
  onChange: (v: T) => void;
  right?: ReactNode;
}) {
  return (
    <div className="row" style={{ borderBottom: "1px solid var(--border)", gap: 4 }}>
      {tabs.map((t) => (
        <button
          key={t.value}
          onClick={() => onChange(t.value)}
          style={{
            height: 40,
            padding: "0 10px",
            fontSize: 14,
            color: t.value === value ? "var(--text)" : "var(--text2)",
            fontWeight: t.value === value ? 600 : 400,
            boxShadow: t.value === value ? "inset 0 -2px 0 var(--accent)" : undefined,
          }}
        >
          {t.label}
          {t.count != null && <span style={{ marginLeft: 6, color: "var(--text3)", fontWeight: 400 }}>{t.count}</span>}
        </button>
      ))}
      <span className="grow" />
      {right}
    </div>
  );
}

/** Closes on a click outside or Esc. */
export function useDismiss<T extends HTMLElement>(open: boolean, onClose: () => void) {
  const ref = useRef<T>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    // Esc closes the menu and nothing behind it (an open profile section, the tree's panel): first, and marked done.
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      onClose();
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [open, onClose]);
  return ref;
}

/** Click and keyboard for a clickable row that can't be a <button> (it holds buttons, or is a grid row): Tab reaches
 *  it, Enter or Space opens it. Keys pressed on a button inside the row are left to that button. */
export function rowButton(onOpen: () => void) {
  return {
    role: "button",
    tabIndex: 0,
    onClick: onOpen,
    onKeyDown: (e: ReactKeyboardEvent) => {
      if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
      e.preventDefault();
      onOpen();
    },
  };
}

/** A dropdown chip-button with a menu of options ("Koloruj wg: gałąź ▾"). */
export function Dropdown<T extends string>({
  label,
  value,
  options,
  onChange,
  icon,
  align = "left",
  width = 220,
}: {
  label: string;
  value: T;
  options: { value: T; label: string; note?: string }[];
  onChange: (v: T) => void;
  icon?: ReactNode;
  align?: "left" | "right";
  width?: number;
}) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const current = options.find((o) => o.value === value);
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button
        className="chip"
        onClick={() => setOpen((o) => !o)}
        style={open ? { border: "1.5px solid var(--accent)", boxShadow: "var(--ring)", color: "var(--text)" } : { color: "var(--text)" }}
      >
        {icon}
        <span>
          {label} <b style={{ fontWeight: 600 }}>{current?.label}</b>
        </span>
        <ChevronDown size={13} color="var(--text3)" style={{ transform: open ? "rotate(180deg)" : undefined }} />
      </button>
      {open && (
        <div className="popover" style={{ top: 36, [align]: 0, width, padding: "4px 0" }}>
          {options.map((o) => (
            <button
              key={o.value}
              className={`menu-item${o.value === value ? " on" : ""}`}
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
            >
              <span className="grow">{o.label}</span>
              {o.note && <span style={{ fontSize: 12, color: "var(--text3)" }}>{o.note}</span>}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** The Polish A–Z rail (all letters, including Ć, Ł, Ń, Ó, Ś, Ź, Ż; FEEDBACK). */
export const POLISH_LETTERS = ["A", "B", "C", "Ć", "D", "E", "F", "G", "H", "I", "J", "K", "L", "Ł", "M", "N", "O", "P", "R", "S", "Ś", "T", "U", "W", "Z", "Ź", "Ż"];

export function AzRail({ active, has, onPick, width = 28 }: { active?: string | null; has: Set<string>; onPick: (letter: string) => void; width?: number }) {
  return (
    <div
      style={{
        width,
        flex: "none",
        borderLeft: "1px solid var(--border)",
        padding: "8px 0",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "space-between",
        fontSize: 11,
        fontWeight: 600,
        color: "var(--text3)",
      }}
    >
      {POLISH_LETTERS.map((l) => {
        const on = has.has(l);
        return (
          <button
            key={l}
            disabled={!on}
            onClick={() => onPick(l)}
            style={{
              width: 22,
              height: 18,
              borderRadius: 4,
              background: l === active ? "var(--accent-soft)" : undefined,
              color: l === active ? "var(--accent-text)" : on ? "var(--text2)" : "var(--text3)",
              opacity: on ? 1 : 0.45,
            }}
          >
            {l}
          </button>
        );
      })}
    </div>
  );
}

/** A 4:3 image tile with a quiet placeholder (spec §4.30). */
export function Thumb({
  path,
  size = 256,
  icon,
  label,
  style,
  fit = "cover",
}: {
  path: string | null | undefined;
  size?: number;
  icon?: ReactNode;
  label?: string;
  style?: CSSProperties;
  fit?: "cover" | "contain";
}) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [path]);
  const showImage = path && !failed && /\.(jpe?g|png|gif|webp|bmp|tiff?|heic|avif)$/i.test(path);
  return (
    <div
      style={{
        background: "var(--surface2)",
        border: "1px solid var(--border)",
        borderRadius: "var(--r-ctl)",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 6,
        color: "var(--text3)",
        fontSize: 11,
        ...style,
      }}
    >
      {showImage ? (
        <img src={mediaUrl(path, "thumb", size)} alt="" loading="lazy" style={{ width: "100%", height: "100%", objectFit: fit }} onError={() => setFailed(true)} />
      ) : (
        <>
          {icon}
          {label && <span>{label}</span>}
        </>
      )}
    </div>
  );
}

export function EmptyState({ icon, title, text, action }: { icon?: ReactNode; title: string; text?: string; action?: ReactNode }) {
  return (
    <div className="col" style={{ alignItems: "center", justifyContent: "center", gap: 10, padding: 48, textAlign: "center", color: "var(--text2)" }}>
      {icon && <div style={{ color: "var(--text3)" }}>{icon}</div>}
      <div className="serif" style={{ fontSize: 21, fontWeight: 600, color: "var(--text)" }}>
        {title}
      </div>
      {text && <div style={{ maxWidth: 420, fontSize: 14, lineHeight: 1.5 }}>{text}</div>}
      {action}
    </div>
  );
}

/** Label + optional description on the left, a control on the right (Settings rows). `soon`: not built yet — the
 *  row is greyed with a „wkrótce” badge and its control is shown but can't be used (one look for all such rows). */
export function SettingRow({ label, note, children, last, soon }: { label: ReactNode; note?: ReactNode; children?: ReactNode; last?: boolean; soon?: boolean }) {
  return (
    <div
      className={`row${soon ? " set-soon" : ""}`}
      aria-disabled={soon || undefined}
      style={{ minHeight: 60, padding: "10px 18px", gap: 20, borderBottom: last ? undefined : "1px solid var(--border)" }}
    >
      <div className="col grow" style={{ gap: 2 }}>
        <span className="row" style={{ gap: 8, fontSize: 14, fontWeight: 500 }}>
          {label}
          {soon && (
            <span className="badge" title="Będzie w jednej z kolejnych wersji">
              wkrótce
            </span>
          )}
        </span>
        {note && <span style={{ fontSize: 12, color: "var(--text3)" }}>{note}</span>}
      </div>
      {soon ? (
        <span className="set-soon-control" inert>
          {children}
        </span>
      ) : (
        children
      )}
    </div>
  );
}

export function Spinner({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className="spin" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
      <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
    </svg>
  );
}
