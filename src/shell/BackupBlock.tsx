// The backup block of the final-change dialog and of the foreign-file question (DESIGN A.7): what backup there is, where
// the next one goes, and making it with the numbers shown.

import { Archive, FolderOpen, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { call, type ApiError } from "../api/transport";
import type { BackupInfo, BackupResult } from "../api/types";
import { cancelJob, trackProgress } from "../app/progress";
import { useStore } from "../app/store";
import { Spinner } from "../components/bits";
import { pickFolder, revealPath } from "../lib/native";
import { backupNotes, backupSizeText, backupStatusLine, CANCELLED_TEXT, noRoom, readyText, runningText } from "../screens/archive/backupText";

export type BackupPhase = "idle" | "running" | "done" | "cancelled";

export interface BackupBlockState {
  info: BackupInfo | null;
  phase: BackupPhase;
  progress: { done: number; total: number } | null;
  result: BackupResult | null;
  /** The message of a failed backup (the backup folder is full, FAT32 …) or of info that couldn't be read. */
  error: string | null;
  /** Nothing new to back up: the last backup is the archive as it is. */
  current: boolean;
  /** Making a backup is possible (info is known and the disk has room). */
  canStart: boolean;
  /** The backup is made (or not needed): the main action may be offered without the question. */
  covered: boolean;
  start: () => void;
  cancel: () => void;
  chooseFolder: () => void;
}

/** The state of the backup block for `scope` ("archive": the user's version; "original": the archive itself). */
export function useBackupBlock(scope: "archive" | "original"): BackupBlockState {
  const [info, setInfo] = useState<BackupInfo | null>(null);
  const [phase, setPhase] = useState<BackupPhase>("idle");
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [result, setResult] = useState<BackupResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => void (alive.current = false);
  }, []);

  const load = useCallback(async () => {
    try {
      const next = await call<BackupInfo>("archive.backupInfo", { scope });
      if (alive.current) {
        setInfo(next);
        setError(null);
      }
    } catch (e) {
      if (alive.current) setError((e as ApiError).message);
    }
  }, [scope]);
  useEffect(() => {
    void load();
  }, [load]);

  const start = () => {
    setPhase("running");
    setProgress(null);
    setError(null);
    trackProgress(call<BackupResult>("archive.backupAuto", { scope }), (p) => alive.current && setProgress({ done: p.done, total: p.total }))
      .then((r) => {
        if (!alive.current) return;
        setResult(r);
        setPhase("done");
      })
      .catch((e: ApiError) => {
        if (!alive.current) return;
        setPhase(e.code === "cancelled" ? "cancelled" : "idle");
        if (e.code !== "cancelled") setError(e.message);
        void load();
      });
  };

  const chooseFolder = async () => {
    const folder = await pickFolder("Gdzie zapisywać kopie zapasowe?");
    if (!folder) return;
    try {
      await call("app.setBackupFolder", { folder });
      setError(null);
      await load();
    } catch (e) {
      useStore.getState().notify((e as ApiError).message, { kind: "err" });
    }
  };

  const current = !!info?.current && phase === "idle";
  return {
    info,
    phase,
    progress,
    result,
    error,
    current,
    canStart: !!info && !noRoom(info),
    covered: phase === "done" || current,
    start,
    cancel: () => void cancelJob(),
    chooseFolder,
  };
}

const note = { fontSize: 13, lineHeight: 1.5, color: "var(--text2)" } as const;

export function BackupBlock({ bb }: { bb: BackupBlockState }) {
  const { info, phase } = bb;
  const notes = info && phase !== "done" ? backupNotes(info) : [];
  return (
    <div className="col" style={{ margin: "0 24px 16px", padding: "12px 14px", gap: 8, border: "1px solid var(--border)", borderRadius: "var(--r-ctl)", ...note }}>
      <span className="row" style={{ gap: 8, alignItems: "flex-start", color: "var(--text)", fontWeight: 600 }}>
        {phase === "running" ? (
          <span style={{ color: "var(--accent-text)", display: "flex", marginTop: 1 }}>
            <Spinner size={15} />
          </span>
        ) : (
          <Archive size={15} style={{ marginTop: 3, flex: "none" }} />
        )}
        <span className="grow">
          {phase === "running"
            ? runningText(bb.progress)
            : phase === "done" && bb.result
              ? readyText(bb.result)
              : phase === "cancelled"
                ? CANCELLED_TEXT
                : info
                  ? backupStatusLine(info)
                  : bb.error
                    ? "Nie udało się sprawdzić kopii zapasowych."
                    : "Sprawdzam kopie zapasowe…"}
        </span>
        {phase === "running" && (
          <button className="btn secondary sm" onClick={bb.cancel}>
            Przerwij
          </button>
        )}
        {phase === "done" && bb.result && (
          <button className="btn secondary sm" onClick={() => revealPath(bb.result!.path).catch(() => {})}>
            <FolderOpen size={14} />
            Pokaż w folderze
          </button>
        )}
      </span>
      {info && phase !== "running" && phase !== "done" && !bb.current && (
        <span>
          Kopia trafi do: <b style={{ fontWeight: 600, color: "var(--text)" }}>{info.folderLabel}</b>{" "}
          <button className="link" onClick={bb.chooseFolder}>
            Zmień…
          </button>{" "}
          <span style={{ whiteSpace: "nowrap" }}>· {backupSizeText(info)}</span>
        </span>
      )}
      {bb.error && (
        <span className="row" style={{ gap: 8, alignItems: "flex-start", color: "var(--err)" }}>
          <TriangleAlert size={14} style={{ marginTop: 3, flex: "none" }} />
          {bb.error}
        </span>
      )}
      {notes.map((n) => (
        <span key={n.kind}>
          {n.text}
          {n.choose && (
            <>
              {" "}
              <button className="link" onClick={bb.chooseFolder}>
                Wybierz miejsce…
              </button>
            </>
          )}
        </span>
      ))}
    </div>
  );
}
