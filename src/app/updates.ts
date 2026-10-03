// Update checks and updates from inside the app (APP-STANDARDS.md §2–3); the asking and downloading happen in Rust
// (crates/heirloom-api/src/update.rs). GitHub is asked at start and whenever the window comes back only while
// „Sprawdzaj aktualizacje” is on (it is off until switched on); Rust keeps automatic checks to one per 5 minutes.
// „Sprawdź teraz” and „GitHub” work either way. An automatic check that fails
// says nothing (it is almost always „no internet”); „Sprawdź teraz” says in words what went wrong.

import { useEffect } from "react";
import { create } from "zustand";
import { ApiError, call } from "../api/transport";
import type { UpdateStatus } from "../api/types";
import { bytes } from "../lib/format";
import { useStore } from "./store";
import { closeWhenSaved } from "./useCloseGuard";

interface Updates {
  status: UpdateStatus | null;
  /** „Sprawdź teraz” is waiting for GitHub. */
  checking: boolean;
  /** What the last „Sprawdź teraz” found, in words. */
  said: string | null;
  /** The banner's ✕, for this version: hidden until Heirloom next starts (kept in memory only, never saved). */
  closedFor: string | null;
  /** Why the download or the installer's start failed; shown with „Spróbuj ponownie” and „GitHub”. */
  failed: string | null;
  /** The installer has been started and Heirloom is closing. */
  installing: boolean;
}

export const useUpdates = create<Updates>(() => ({ status: null, checking: false, said: null, closedFor: null, failed: null, installing: false }));

const RETRY_MS = 30_000;
let retry: ReturnType<typeof setTimeout> | undefined;

/** At start, on coming back and when the connection returns. One that couldn't reach GitHub tries again in 30 s
 *  (it cost GitHub nothing), so a computer that comes online later still hears about an update. */
function autoCheck() {
  clearTimeout(retry);
  if (!useStore.getState().app?.updates.check) return;
  call<UpdateStatus>("update.check", { manual: false })
    // A newer answer replaces what the last „Sprawdź teraz” said (an error from when there was no internet, say).
    .then((status) => useUpdates.setState({ status, said: null }))
    .catch((e: ApiError) => {
      if (e.code === "offline" || e.code === "timeout") retry = setTimeout(autoCheck, RETRY_MS);
    });
}

/** Mounted once, in App. */
export function useUpdateChecks() {
  const on = useStore((s) => s.app?.updates.check ?? false);
  useEffect(() => {
    if (!on) return;
    // A moment after start, so opening the archive isn't also waiting on the network.
    const first = setTimeout(autoCheck, 3000);
    const back = () => {
      if (document.visibilityState === "visible") autoCheck();
    };
    document.addEventListener("visibilitychange", back);
    window.addEventListener("focus", back);
    window.addEventListener("online", autoCheck);
    return () => {
      clearTimeout(first);
      clearTimeout(retry);
      document.removeEventListener("visibilitychange", back);
      window.removeEventListener("focus", back);
      window.removeEventListener("online", autoCheck);
    };
  }, [on]);
}

/** „Sprawdź teraz”: always asks GitHub, and says what it found. */
export async function checkNow() {
  useUpdates.setState({ checking: true, said: null });
  try {
    const status = await call<UpdateStatus>("update.check", { manual: true });
    const said = status.newer ? `Jest nowa wersja ${status.latest} — masz ${status.current}.` : `Masz najnowszą wersję, ${status.current}.`;
    // A newer version found by hand shows its banner again (while „Sprawdzaj aktualizacje” is on; otherwise it is offered
    // in Ustawienia › O programie).
    useUpdates.setState({ status, said, closedFor: null });
  } catch (e) {
    useUpdates.setState({ said: (e as ApiError).message });
  } finally {
    useUpdates.setState({ checking: false });
  }
}

export async function refreshUpdateStatus() {
  const status = await call<UpdateStatus>("update.status").catch(() => null);
  if (status) useUpdates.setState({ status });
}

/** „Aktualizuj”: downloads the installer with the progress shown, asks about unsaved work, then starts the installer
 *  in update mode (`--update`: it only shows its progress and starts Heirloom again) and Heirloom closes so it can be
 *  replaced. A failure stays on screen; nothing opens by itself. */
let applying = false;
export async function applyUpdate() {
  const version = useUpdates.getState().status?.latest;
  // One run at a time: the banner and Ustawienia share it.
  if (!version || applying || useUpdates.getState().installing) return;
  applying = true;
  useUpdates.setState({ failed: null });
  try {
    let status = await call<UpdateStatus>("update.download");
    useUpdates.setState({ status });
    while (status.download.state === "running") {
      await new Promise((r) => setTimeout(r, 400));
      status = await call<UpdateStatus>("update.status");
      useUpdates.setState({ status });
    }
    if (status.download.state === "failed") throw new Error(status.download.message);
    if (status.download.state !== "ready") throw new Error("Pobieranie się nie udało.");
    closeWhenSaved(() => void install(), version);
  } catch (e) {
    useUpdates.setState({ failed: (e as Error).message });
  } finally {
    applying = false;
  }
}

async function install() {
  useUpdates.setState({ installing: true, failed: null });
  try {
    await call("update.install");
  } catch (e) {
    useUpdates.setState({ installing: false, failed: (e as ApiError).message });
  }
}

/** „Pobieram… 42%”, or how much has come while the size isn't known: never a 0% that looks frozen. */
export function downloadText(status: UpdateStatus): string | null {
  const d = status.download;
  if (d.state !== "running") return null;
  if (d.total > 0) return `Pobieram Heirloom ${d.version}… ${Math.round((d.done / d.total) * 100)}%`;
  return d.done > 0 ? `Pobieram Heirloom ${d.version}… ${bytes(d.done)}` : `Pobieram Heirloom ${d.version}…`;
}
