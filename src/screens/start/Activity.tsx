// Historia zmian (spec §4.21): every saved change in the archive — what, who and when — newest first, by day.
// Each change, or a whole import, can be taken back in edit mode; that is itself a new change waiting for Save.

import {
  BookOpen,
  Check,
  FileCode,
  History,
  Image as ImageIcon,
  ImagePlus,
  Import,
  Info,
  Library,
  Link,
  Pencil,
  Plus,
  Trash2,
  TriangleAlert,
  Undo2,
  UserPlus,
  type LucideIcon,
} from "lucide-react";
import { useMemo, useState } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { afterChange, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Dialog } from "../../components/Dialog";
import { EmptyState, Spinner } from "../../components/bits";
import { clock, count, dayMonth, plural } from "../../lib/format";
import { askUndoImport } from "../import/undoImport";
import "./activity.css";

type Filter = "all" | "person" | "media" | "relation" | "import" | "deleted";

interface FeedRow {
  index: number;
  ts: string;
  icon: string;
  action: string;
  subject: string;
  subjectId: string | null;
  detail: string;
  who: string;
  batch: string | null;
  note: string | null;
  undo: string;
  /** False when the record was changed again later (undoing would also drop those changes). Missing for imports. */
  current?: boolean;
}

const TABS: { value: Filter; label: string }[] = [
  { value: "all", label: "Wszystko" },
  { value: "person", label: "Osoby" },
  { value: "media", label: "Zdjęcia" },
  { value: "relation", label: "Relacje" },
  { value: "import", label: "Import" },
  { value: "deleted", label: "Usunięte" },
];

const ICONS: Record<string, LucideIcon> = {
  "user-plus": UserPlus,
  pencil: Pencil,
  "trash-2": Trash2,
  link: Link,
  "book-open": BookOpen,
  "image-plus": ImagePlus,
  image: ImageIcon,
  library: Library,
  "file-code": FileCode,
  plus: Plus,
  import: Import,
};

const DONE: Record<string, string> = { Cofnij: "Cofnięto", "Cofnij import": "Cofnięto import", Przywróć: "Przywrócono" };

const PAGE = 200;

/** „Dziś”, „Wczoraj”, „26 września” (with the year when it isn't this year); upper-cased by CSS. */
function dayLabel(date: Date, now = new Date()): string {
  if (Number.isNaN(date.getTime())) return "Bez daty";
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((start(now) - start(date)) / 86_400_000);
  if (days === 0) return "Dziś";
  if (days === 1) return "Wczoraj";
  return date.getFullYear() === now.getFullYear() ? dayMonth(date) : `${dayMonth(date)} ${date.getFullYear()}`;
}

export function Activity() {
  const [filter, setFilter] = useState<Filter>("all");
  const [limit, setLimit] = useState(PAGE);
  const { data, error } = useApi<{ rows: FeedRow[]; total: number }>("history.feed", { filter, limit });
  const go = useStore((s) => s.go);
  const editing = useStore((s) => s.mode === "edit");
  const unsaved = useStore((s) => s.archive?.unsavedChanges ?? 0);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const [confirm, setConfirm] = useState<FeedRow | null>(null);
  // The feed changes only on Save, so a row undone just now would still offer „Cofnij” (and look "changed later").
  const [undone, setUndone] = useState<ReadonlySet<number>>(() => new Set());

  const rows = data?.rows ?? [];
  const days = useMemo(() => {
    const out: { key: string; label: string; rows: FeedRow[] }[] = [];
    for (const row of data?.rows ?? []) {
      const date = new Date(row.ts);
      const key = date.toDateString();
      const last = out[out.length - 1];
      if (last?.key === key) last.rows.push(row);
      else out.push({ key, label: dayLabel(date), rows: [row] });
    }
    return out;
  }, [data]);

  const undo = (row: FeedRow) =>
    requireEdit(async () => {
      try {
        const status = await call<ArchiveStatus>("history.undo", row.batch ? { batch: row.batch } : { index: row.index });
        afterChange(status);
        setUndone((s) => new Set(s).add(row.index));
        notify("Cofnięto — zmiana czeka na zapis.");
      } catch (e) {
        notify((e as Error).message, { kind: "err" });
      }
    });

  return (
    <div className="page">
      <div className="activity">
        <div className="col" style={{ gap: 2 }}>
          <h1 className="page-title">Historia zmian</h1>
          <span className="activity-sub">
            Każdą zmianę można cofnąć. Przy każdej widać, kto edytował (imię z „Kto edytuje?”) albo z której paczki pochodzi.
          </span>
        </div>

        <div className="row activity-tabs">
          {TABS.map((t) => (
            <button
              key={t.value}
              className={`activity-tab${filter === t.value ? " on" : ""}`}
              onClick={() => {
                setFilter(t.value);
                setLimit(PAGE);
              }}
            >
              {t.label}
            </button>
          ))}
        </div>

        {unsaved > 0 && (
          <div className="banner info">
            <Info size={16} color="var(--text2)" />
            {count(unsaved, "niezapisana zmiana", "niezapisane zmiany", "niezapisanych zmian")} {plural(unsaved, "pojawi", "pojawią", "pojawi")} się tutaj po
            zapisie.
          </div>
        )}

        {!editing && rows.length > 0 && (
          <div className="row activity-hint">
            <Undo2 size={14} />
            Cofanie zmian jest dostępne w trybie edycji.
            <button className="link" onClick={() => requireEdit()}>
              Włącz edycję
            </button>
          </div>
        )}

        {error && <div className="activity-none">{error.message}</div>}
        {!data && !error && (
          <div className="activity-none">
            <Spinner size={18} />
          </div>
        )}
        {data && rows.length === 0 && filter === "all" && (
          <EmptyState
            icon={<History size={28} />}
            title="Historia zmian jest pusta"
            text="Każda zmiana trafia tutaj przy zapisie: co zmieniono, kto to zrobił i kiedy. Import pojawi się jako jeden wpis."
          />
        )}
        {data && rows.length === 0 && filter !== "all" && <div className="activity-none">Brak zmian tego rodzaju.</div>}

        {days.map((day) => (
          <div key={day.key} className="col">
            <span className="activity-day">{day.label}</span>
            <div className="activity-list">
              {day.rows.map((row) => {
                const Icon = ICONS[row.icon] ?? Pencil;
                const subjectId = row.subjectId;
                return (
                  <div key={row.index} className="activity-row">
                    <span className="activity-icon">
                      <Icon size={15} />
                    </span>
                    <div className="col grow" style={{ lineHeight: 1.35 }}>
                      <span>
                        <b style={{ fontWeight: 600 }}>{row.action}</b>{" "}
                        {subjectId ? (
                          <button className="activity-subject" onClick={() => go({ name: "person", id: subjectId })}>
                            {row.subject}
                          </button>
                        ) : (
                          <span className="activity-subject">{row.subject}</span>
                        )}
                      </span>
                      {row.detail && <span style={{ color: "var(--text2)" }}>{row.detail}</span>}
                      {row.note && <span className="activity-note">„{row.note}”</span>}
                    </div>
                    <span className="activity-when">
                      {clock(row.ts)} · {row.who}
                    </span>
                    {editing &&
                      (undone.has(row.index) ? (
                        <button className="btn secondary sm activity-undo" disabled>
                          <Check size={13} />
                          {DONE[row.undo] ?? "Cofnięto"}
                        </button>
                      ) : (
                        <button className="btn secondary sm activity-undo" onClick={() => (row.batch ? askUndoImport(row.batch, () => undo(row)) : row.current === false ? setConfirm(row) : undo(row))}>
                          <Undo2 size={13} />
                          {row.undo}
                        </button>
                      ))}
                  </div>
                );
              })}
            </div>
          </div>
        ))}

        {data && data.rows.length >= limit && (
          <button className="btn ghost" style={{ alignSelf: "flex-start" }} onClick={() => setLimit(limit + PAGE)}>
            Pokaż starsze zmiany
          </button>
        )}
      </div>

      {confirm && (
        <ChangedLater
          row={confirm}
          onCancel={() => setConfirm(null)}
          onConfirm={() => {
            setConfirm(null);
            undo(confirm);
          }}
        />
      )}
    </div>
  );
}

/** Asked before undoing a change when the record was edited again afterwards. */
function ChangedLater({ row, onCancel, onConfirm }: { row: FeedRow; onCancel: () => void; onConfirm: () => void }) {
  const noun = row.undo === "Przywróć" ? "Przywrócenie" : "Cofnięcie";
  return (
    <Dialog width={480} onClose={onCancel}>
      <div className="dialog-body">
        <div className="dialog-icon">
          <TriangleAlert size={19} />
        </div>
        <div className="dialog-title">Ten wpis zmieniono później</div>
        <div className="dialog-text">
          Wpis „{row.subject}” zmieniono jeszcze po tej zmianie. {noun} przywróci go do stanu sprzed niej, więc późniejsze zmiany też znikną. Do zapisu
          możesz to odwrócić (Ctrl Z).
        </div>
      </div>
      <div className="dialog-foot">
        <button className="btn secondary" onClick={onCancel}>
          Anuluj
        </button>
        <button className="btn primary" onClick={onConfirm}>
          {row.undo} mimo to
        </button>
      </div>
    </Dialog>
  );
}
