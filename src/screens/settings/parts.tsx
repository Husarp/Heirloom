// Pieces shared by the Settings sections (spec §4.34).

import { ChevronDown } from "lucide-react";
import type { ReactNode } from "react";
import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { useStore } from "../../app/store";
import { clock, relativeTime } from "../../lib/format";
import { revealPath } from "../../lib/native";

/** A section: a serif heading over a card of rows. */
export function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={`set-${id}`} className="set-section">
      <h2>{title}</h2>
      <div className="card">{children}</div>
    </section>
  );
}

/** The settings select: 34 px, at least 180 px wide. */
export function Select<T extends string>({
  value,
  options,
  onChange,
  disabled,
  label,
  title,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  disabled?: boolean;
  label: string;
  title?: string;
}) {
  return (
    <span className="set-select" title={title}>
      <select value={value} disabled={disabled} aria-label={label} onChange={(e) => onChange(e.target.value as T)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown size={13} />
    </span>
  );
}

/** The keycap badge: „GEDCOM 7”, „1.0”. */
export function Keycap({ children }: { children: ReactNode }) {
  return <span className="keycap">{children}</span>;
}

/** Shows why a command failed. */
export function failed(e: unknown) {
  useStore.getState().notify((e as ApiError).message, { kind: "err" });
}

/** Changes the open archive's settings (display choices, start person, editors…). They are not family data, so this
 *  works in browse mode too. */
export async function saveSettings(patch: object): Promise<boolean> {
  try {
    // `changed`, not just the new status: name order, date format and surname joins change what every screen shows.
    useStore.getState().changed(await call<ArchiveStatus>("archive.setSettings", patch));
    return true;
  } catch (e) {
    failed(e);
    return false;
  }
}

export const setDisplay = (key: string, value: unknown) => saveSettings({ display: { [key]: value } });

/** The toast action after writing a file. */
export const showInFolder = (path: string) => ({ label: "Pokaż w folderze", run: () => revealPath(path).catch(failed) });

/** A file name made from the archive's name (without characters Windows doesn't allow). */
export function fileSafe(name: string): string {
  return name.replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, " ").trim() || "Archiwum";
}

/** "2026-09-28", today. */
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** "dziś 11:04", "wczoraj 18:20", "3 dni temu". */
export function whenDone(iso: string): string {
  const text = relativeTime(iso);
  if (/^\d\d:\d\d$/.test(text)) return `dziś ${text}`;
  return text === "wczoraj" ? `wczoraj ${clock(iso)}` : text;
}
