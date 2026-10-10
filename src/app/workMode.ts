// Choosing and switching how the archive is worked with („Pracuj na archiwum” / „Nie ruszaj oryginału”), discarding
// the user's changes and finding the original again (DESIGN A.5, A.6). The backend does the work; these are the
// questions in front of it and what follows.

import { call, type ApiError } from "../api/transport";
import type { ArchiveStatus } from "../api/types";
import { count } from "../lib/format";
import { pickFolder } from "../lib/native";
import {
  PENDING_SWITCH_TEXT,
  SWITCH_TO_OVERLAY,
  discardText,
  discardTitle,
  isoDay,
  leafOf,
  pendingTitle,
  type WorkMode,
} from "../screens/archive/workModes";
import { useWizard } from "../screens/import/types";
import { useStore } from "./store";

const store = () => useStore.getState();

/** Shows the busy dialog while `work` runs. */
export async function withBusy<T>(text: string, work: () => Promise<T>): Promise<T> {
  useStore.setState({ busy: { text } });
  try {
    return await work();
  } finally {
    useStore.setState({ busy: null });
  }
}

/** `archive.setWorkMode`. Making the folder of „Twoje zmiany” can take a moment. Errors: not_writable, pending_changes,
 *  inside_archive, unsaved, permission, io — the message is Polish. */
export function applyWorkMode(mode: WorkMode, folder?: string | null): Promise<ArchiveStatus> {
  const request = () => call<ArchiveStatus>("archive.setWorkMode", folder ? { mode, folder } : { mode });
  return mode === "overlay" ? withBusy("Przygotowuję folder na Twoje zmiany…", request) : request();
}

const fail = (e: unknown) => store().notify((e as ApiError).message, { kind: "err" });

/** Settings › „Sposób pracy” → „Nie ruszaj oryginału”. */
export function switchToOverlay() {
  store().whenSaved(async () => {
    try {
      const info = await call<{ folder: { label: string } }>("archive.modeInfo");
      store().setAsk({
        title: SWITCH_TO_OVERLAY.title,
        text: SWITCH_TO_OVERLAY.text(info.folder.label),
        icon: "info",
        buttons: [
          { label: "Anuluj", kind: "ghost" },
          {
            label: "Przełącz",
            kind: "primary",
            run: async () => {
              try {
                await store().adoptStatus(await applyWorkMode("overlay"));
              } catch (e) {
                fail(e);
              }
            },
          },
        ],
      });
    } catch (e) {
      fail(e);
    }
  }, "Zanim przełączysz");
}

/** Settings › „Sposób pracy” → „Pracuj na archiwum”: with changes waiting the backend refuses, and the three-way
 *  question comes; without, the folder of changes is retired and the original reopened. */
export function switchToInPlace() {
  store().whenSaved(async () => {
    try {
      store().adoptStatus(await applyWorkMode("inPlace"));
    } catch (e) {
      if ((e as ApiError).code === "pending_changes") await askPendingSwitch();
      else fail(e);
    }
  }, "Zanim przełączysz");
}

/** „Masz 14 zmian, których nie ma w archiwum”: move them first, keep them as a separate archive, or stay. */
export async function askPendingSwitch() {
  let pending = store().archive?.overlay?.pending ?? 0;
  try {
    pending = (await call<ArchiveStatus>("archive.status")).overlay?.pending ?? pending;
  } catch {
    /* the count from before will do */
  }
  store().setAsk({
    title: pendingTitle(pending),
    text: PENDING_SWITCH_TEXT,
    icon: "warn",
    wide: true,
    buttons: [
      { label: "Anuluj", kind: "ghost" },
      { label: "Zachowaj je jako osobne archiwum…", kind: "secondary", run: keepAsNewArchive },
      { label: "Przenieś do archiwum i przełącz", kind: "primary", run: () => store().startWriteBack(() => switchToInPlace()) },
    ],
  });
}

/** „Zachowaj je jako osobne archiwum…”: the user's version becomes an archive of its own; the folder of changes is retired. */
async function keepAsNewArchive() {
  const before = store().archive;
  const folder = await pickFolder("Nowe archiwum: wybierz pusty folder");
  if (!folder || !before) return;
  try {
    const result = await withBusy("Tworzę osobne archiwum…", () =>
      call<{ status: ArchiveStatus; copied: number; failed: number; missing: number }>("archive.saveAsNew", { folder, retire: true }),
    );
    if (useWizard.getState().archiveId === before.archiveId) useWizard.getState().set({ archiveId: result.status.archiveId });
    await store().adoptStatus(result.status);
    store().notify(`Utworzono nowe archiwum w ${folder}.`, {
      detail: [
        `Archiwum ${before.name} zostało bez zmian.`,
        result.copied ? `Skopiowano ${count(result.copied, "plik", "pliki", "plików")}.` : "",
        result.failed ? `${count(result.failed, "pliku", "plików", "plików")} nie udało się skopiować.` : "",
      ]
        .filter(Boolean)
        .join(" "),
    });
  } catch (e) {
    fail(e);
  }
}

/** Settings › „Zacznij od nowa” › „Porzuć moje zmiany…”. The folder is renamed, never deleted. */
export function discardMine() {
  store().whenSaved(async () => {
    const archive = store().archive;
    const overlay = archive?.overlay;
    if (!archive || !overlay) return;
    store().setAsk({
      title: discardTitle(overlay.pending),
      text: discardText(leafOf(overlay.folder), isoDay()),
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Porzuć",
          kind: "danger",
          run: async () => {
            try {
              await store().adoptStatus(await call<ArchiveStatus>("archive.discardOverlay"));
            } catch (e) {
              fail(e);
            }
          },
        },
      ],
    });
  }, "Zanim zaczniesz od nowa");
}

/** „Sprawdź ponownie”: opens the archive again the way it was opened; reachable now, it comes up with the changes found. */
export function recheckOriginal() {
  store().whenSaved(() => {
    const archive = store().archive;
    if (!archive) return;
    const recent = store().app?.recent ?? [];
    const path = recent.find((r) => r.path === archive.dataPath || r.path === archive.root)?.path ?? archive.root;
    void store().openArchive(path);
  }, "Zanim sprawdzisz ponownie");
}

/** „Wskaż, gdzie jest teraz…”: another archive than the one the changes come from asks first. */
export async function relocateOriginal(path?: string, force = false) {
  const folder = path ?? (await pickFolder("Wskaż folder archiwum, z którego pochodzą Twoje zmiany"));
  if (!folder) return;
  try {
    await store().adoptStatus(await call<ArchiveStatus>("archive.relocateOriginal", force ? { path: folder, force } : { path: folder }));
  } catch (e) {
    if ((e as ApiError).code !== "other_archive") return fail(e);
    store().setAsk({
      title: "To inne archiwum niż to, z którego pochodzą Twoje zmiany",
      text: "Użyć go mimo to?",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        { label: "Użyj mimo to", kind: "danger", run: () => relocateOriginal(folder, true) },
      ],
    });
  }
}

