// The app's state: which archive is open, which screen is shown (with back/forward like a browser), browse or
// edit mode and who edits, and short messages. Screen data is fetched by the screens themselves (useApi).

import { create } from "zustand";
import { call, ApiError } from "../api/transport";
import type { AppState, ArchiveStatus, SaveResult } from "../api/types";
import { count } from "../lib/format";

export type TreeView = "family" | "ancestors" | "descendants" | "overview";

export type Route =
  | { name: "start" }
  | { name: "tree"; view: TreeView; person?: string }
  | { name: "people" }
  /** `open`: the section to open for editing (design 17a), e.g. "personal" from the tree's „Edytuj”. */
  | { name: "person"; id: string; section?: string; open?: string }
  /** A new person (optionally a relative of `relation.of`); existing people are edited on their profile. */
  | { name: "edit"; id: null; relation?: { kind: "parent" | "partner" | "child" | "sibling"; of: string } }
  | { name: "surnames"; key?: string }
  | { name: "places"; place?: string }
  /** `q`: a search in the texts, e.g. from Ctrl K when nothing else matched. */
  | { name: "stories"; id?: string; q?: string }
  | { name: "media"; id?: string }
  | { name: "missingFiles" }
  | { name: "sources"; id?: string }
  | { name: "import" }
  | { name: "settings"; section?: string }
  | { name: "activity" };

/** Full-window states before an archive is ready. */
export type Phase = "boot" | "picker" | "loading" | "firstOpen" | "ready";

export interface ViewedPerson {
  id: string;
  name: string;
  initials: string;
  branch: number;
  photo: string | null;
}

export interface Toast {
  id: number;
  text: string;
  /** A second, smaller line ("Nowakowie z Ciechanek · 4 zdjęcia, 3 źródła"). */
  detail?: string;
  kind?: "info" | "err";
  action?: { label: string; run: () => void };
  /** More than one action ("Cofnij import", "Pokaż w drzewie"). */
  actions?: { label: string; run: () => void }[];
}

/** The last save in this edit session: its time, and what names its history entries for „Cofnij zapis”. */
export interface LastSave {
  at: string;
  ts: string;
  author: string;
}

/** The profile section being edited (one at a time, design 17a) and the undo depth when it was opened. */
export interface OpenSection {
  key: string;
  label: string;
  base: number;
}

/** A question with buttons (e.g. unsaved changes before leaving edit mode). */
export interface Ask {
  title: string;
  text?: string;
  icon?: "warn" | "info";
  buttons: { label: string; kind?: "primary" | "secondary" | "ghost" | "danger"; run?: () => void | Promise<void> }[];
}

interface Store {
  phase: Phase;
  app: AppState | null;
  archive: ArchiveStatus | null;
  /** The path being opened, for the loading screen. */
  opening: string | null;
  openError: ApiError | null;
  route: Route;
  back: Route[];
  forward: Route[];
  mode: "browse" | "edit";
  editor: string | null;
  /** Something that needs "Kto edytuje?" first; it runs after a name is chosen. */
  pendingEdit: (() => void) | null;
  whoEditsOpen: boolean;
  conflictOpen: boolean;
  foreignConfirmOpen: boolean;
  /** Bumped after every change, so screens fetch fresh data. */
  dataVersion: number;
  lastSaved: string | null;
  lastSave: LastSave | null;
  section: OpenSection | null;
  recentlyViewed: ViewedPerson[];
  toasts: Toast[];
  paletteOpen: boolean;
  /** „Skocz do osoby…” is the same window, only for people, and picking someone does `pick` (design 17e). */
  palette: { scope: "all" | "people"; pick?: (id: string) => void } | null;
  /** The last part of the breadcrumb, set by the screen (e.g. the person's name). */
  crumb: string | null;
  ask: Ask | null;

  boot: () => Promise<void>;
  refreshApp: () => Promise<void>;
  openArchive: (path: string) => Promise<boolean>;
  createArchive: (folder: string, name: string) => Promise<boolean>;
  closeArchive: () => Promise<void>;
  finishFirstOpen: () => void;
  setArchive: (status: ArchiveStatus) => void;
  go: (route: Route) => void;
  goBack: () => void;
  goForward: () => void;
  /** Set by a screen with unsaved form changes (the person editor): leaving it asks first. */
  leaveGuard: string | null;
  setLeaveGuard: (question: string | null) => void;
  /** Runs `action` in edit mode, asking "Kto edytuje?" first if needed. */
  requireEdit: (action?: () => void) => void;
  startEditing: (name: string) => void;
  cancelWhoEdits: () => void;
  stopEditing: () => void;
  changed: (status?: ArchiveStatus) => void;
  save: (allowForeign?: boolean) => Promise<boolean>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  /** „Cofnij zapis”: the last save's changes come back as unsaved ones. */
  undoSave: () => Promise<void>;
  /** „Anuluj” in the edit bar: drops the unsaved changes and ends editing. */
  discardChanges: () => Promise<void>;
  /** Opens a profile section for editing; false when another one is still open. */
  openSection: (key: string, label: string) => boolean;
  /** „Gotowe”: the section's changes stay (as unsaved changes). */
  finishSection: () => void;
  /** „Anuluj” (Esc): undoes what was changed while the section was open. */
  cancelSection: () => Promise<void>;
  closeConflict: () => void;
  closeForeignConfirm: () => void;
  notify: (text: string, options?: Omit<Toast, "id" | "text">) => void;
  dismissToast: (id: number) => void;
  setPalette: (open: boolean, options?: { scope: "all" | "people"; pick?: (id: string) => void }) => void;
  viewed: (person: ViewedPerson) => void;
  setCrumb: (crumb: string | null) => void;
  setAsk: (ask: Ask | null) => void;
  /** Runs `then` when there are no unsaved changes, or after the user saved or dropped them. */
  whenSaved: (then: () => void, what?: string) => void;
  /** Before an edit made outside the open section: the section is finished first (its changes kept), after asking
   *  about a text still being typed in it. */
  outsideSection: (then: () => void) => void;
  /** A route without pushing it to Back (e.g. the same profile once its `open` was used). */
  replaceRoute: (route: Route) => void;
}

let toastId = 0;

function sameRoute(a: Route, b: Route): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

export const useStore = create<Store>((set, get) => ({
  phase: "boot",
  app: null,
  archive: null,
  opening: null,
  openError: null,
  route: { name: "start" },
  back: [],
  forward: [],
  mode: "browse",
  editor: null,
  pendingEdit: null,
  whoEditsOpen: false,
  conflictOpen: false,
  foreignConfirmOpen: false,
  dataVersion: 0,
  lastSaved: null,
  lastSave: null,
  section: null,
  recentlyViewed: [],
  toasts: [],
  paletteOpen: false,
  palette: null,
  crumb: null,
  ask: null,

  boot: async () => {
    const app = await call<AppState>("app.state");
    set({ app });
    // The app opens on Start with the last archive (PLAN §11.1); if it can't be opened, the picker shows why.
    const last = app.recent[0];
    if (last && (await get().openArchive(last.path))) return;
    set({ phase: "picker" });
  },

  refreshApp: async () => {
    const app = await call<AppState>("app.state");
    set({ app, archive: app.archive ?? get().archive });
  },

  openArchive: async (path) => {
    set({ opening: path, openError: null, phase: get().phase === "boot" ? "boot" : "loading" });
    // Known from before opening: opening puts the archive at the top of the recent list.
    const seenBefore = get().app?.recent.some((r) => r.path === path) ?? false;
    try {
      const status = await call<ArchiveStatus>("archive.open", { path });
      const app = await call<AppState>("app.state");
      const firstTime = status.startPerson == null && status.people > 0 && !seenBefore;
      set({
        app,
        archive: status,
        opening: null,
        phase: firstTime ? "firstOpen" : "ready",
        route: { name: "start" },
        back: [],
        forward: [],
        mode: "browse",
        editor: null,
        dataVersion: get().dataVersion + 1,
        lastSaved: status.lastSaved,
        lastSave: null,
        section: null,
        recentlyViewed: [],
      });
      return true;
    } catch (e) {
      set({ opening: null, openError: e as ApiError, phase: "picker" });
      return false;
    }
  },

  createArchive: async (folder, name) => {
    set({ openError: null });
    try {
      const status = await call<ArchiveStatus>("archive.create", { folder, name });
      const app = await call<AppState>("app.state");
      set({
        app,
        archive: status,
        phase: "ready",
        route: { name: "start" },
        back: [],
        forward: [],
        mode: "browse",
        editor: null,
        dataVersion: get().dataVersion + 1,
        lastSaved: null,
        lastSave: null,
        section: null,
        recentlyViewed: [],
      });
      return true;
    } catch (e) {
      set({ openError: e as ApiError });
      return false;
    }
  },

  closeArchive: async () => {
    await call("archive.close");
    const app = await call<AppState>("app.state");
    set({ app, archive: null, phase: "picker", mode: "browse", editor: null, lastSave: null, section: null });
  },

  finishFirstOpen: () => set({ phase: "ready" }),

  setArchive: (status) => set({ archive: status }),

  go: (route) => {
    const { route: current, back } = get();
    if (sameRoute(current, route)) return;
    // An edit that waited for „Kto edytuje?” belongs to the screen being left.
    leaving(() => set({ route, back: [...back, current].slice(-100), forward: [], pendingEdit: null }));
  },

  replaceRoute: (route) => set({ route }),

  goBack: () => {
    const { back, route, forward } = get();
    const previous = back[back.length - 1];
    if (!previous) return;
    leaving(() => set({ route: previous, back: back.slice(0, -1), forward: [route, ...forward], pendingEdit: null }));
  },

  goForward: () => {
    const { back, route, forward } = get();
    const next = forward[0];
    if (!next) return;
    leaving(() => set({ route: next, back: [...back, route], forward: forward.slice(1), pendingEdit: null }));
  },

  leaveGuard: null,
  setLeaveGuard: (question) => set({ leaveGuard: question }),

  requireEdit: (action) => {
    const { mode, archive } = get();
    if (archive?.readOnly) {
      get().notify("Archiwum jest tylko do odczytu. Wyłącz to w Ustawienia › Archiwum.");
      return;
    }
    if (mode === "edit") {
      action?.();
      return;
    }
    set({ whoEditsOpen: true, pendingEdit: action ?? null });
  },

  startEditing: (name) => {
    const pending = get().pendingEdit;
    set({ mode: "edit", editor: name, whoEditsOpen: false, pendingEdit: null, lastSave: null });
    call("archive.setEditor", { name }).catch(() => {});
    pending?.();
  },

  cancelWhoEdits: () => set({ whoEditsOpen: false, pendingEdit: null }),

  stopEditing: () => leaving(() => set({ mode: "browse", section: null, lastSave: null })),

  changed: (status) => {
    set({ dataVersion: get().dataVersion + 1, ...(status ? { archive: status } : {}) });
    if (!status) {
      call<ArchiveStatus>("archive.status")
        .then((s) => set({ archive: s }))
        .catch(() => {});
    }
  },

  save: async (allowForeign = false) => {
    const { editor, archive } = get();
    if (!archive) return false;
    try {
      const result = await call<SaveResult>("archive.save", { author: editor ?? "?", allowForeign });
      const open = get().section;
      set({
        archive: result.status,
        lastSaved: result.savedAt,
        lastSave: result.changes > 0 ? (result.history ? { at: result.savedAt, ...result.history } : null) : get().lastSave,
        // What a section changed is saved now: its „Anuluj” no longer takes it back.
        section: open ? { ...open, base: result.status.undoDepth } : null,
        dataVersion: get().dataVersion + 1,
      });
      return true;
    } catch (e) {
      const error = e as ApiError;
      if (error.code === "conflict") set({ conflictOpen: true });
      else if (error.code === "foreign") set({ foreignConfirmOpen: true });
      else get().notify(error.message, { kind: "err" });
      return false;
    }
  },

  undo: async () => {
    const status = await call<ArchiveStatus>("archive.undo");
    // Undone below where the open section started: its „Anuluj” must not reach further back than that.
    const open = get().section;
    if (open && status.undoDepth < open.base) set({ section: { ...open, base: status.undoDepth } });
    get().changed(status);
  },

  redo: async () => {
    const status = await call<ArchiveStatus>("archive.redo");
    get().changed(status);
  },

  undoSave: async () => {
    const last = get().lastSave;
    if (!last) return;
    const status = await call<ArchiveStatus & { undoSkipped?: number }>("history.undo", { ts: last.ts, author: last.author });
    set({ lastSave: null });
    get().changed(status);
    const skipped = status.undoSkipped ?? 0;
    get().notify("Cofnięto zapis — zmiany czekają na ponowny zapis.", {
      detail: skipped ? `${count(skipped, "wpis zmieniony", "wpisy zmienione", "wpisów zmienionych")} później ${skipped === 1 ? "został" : "zostały"} bez zmian.` : undefined,
    });
  },

  discardChanges: async () => {
    const status = await call<ArchiveStatus>("archive.reload");
    set({ leaveGuard: null, section: null, lastSave: null });
    get().changed(status);
    set({ mode: "browse" });
  },

  outsideSection: (then) => {
    if (!get().section) return then();
    const finish = () => {
      set({ section: null, leaveGuard: null });
      then();
    };
    const question = get().leaveGuard;
    if (!question) return finish();
    set({
      ask: {
        title: question,
        text: "Najpierw zakończę edycję otwartej sekcji — to, co w niej wpisano, a nie zatwierdzono, przepadnie.",
        icon: "warn",
        buttons: [
          { label: "Wróć do sekcji", kind: "ghost" },
          { label: "Odrzuć i kontynuuj", kind: "danger", run: finish },
        ],
      },
    });
  },

  openSection: (key, label) => {
    const open = get().section;
    if (open && open.key !== key) {
      get().notify(`Najpierw zakończ edycję sekcji „${open.label}” (Gotowe albo Anuluj).`);
      return false;
    }
    set({ section: { key, label, base: get().archive?.undoDepth ?? 0 } });
    return true;
  },

  finishSection: () => set({ section: null, leaveGuard: null }),

  cancelSection: async () => {
    const open = get().section;
    set({ section: null, leaveGuard: null });
    if (!open) return;
    let depth = get().archive?.undoDepth ?? 0;
    let status: ArchiveStatus | null = null;
    while (depth > open.base) {
      status = await call<ArchiveStatus>("archive.undo");
      if (status.undoDepth >= depth) break;
      depth = status.undoDepth;
    }
    if (status) get().changed(status);
  },

  closeConflict: () => set({ conflictOpen: false }),
  closeForeignConfirm: () => set({ foreignConfirmOpen: false }),

  // At most 3 at once, the newest at the bottom; each one times itself out (shell/Toasts.tsx).
  notify: (text, options) => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, text, ...options }].slice(-3) });
  },

  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),

  setPalette: (open, options) => set({ paletteOpen: open, palette: open ? (options ?? null) : null }),

  viewed: (person) => {
    const list = [person, ...get().recentlyViewed.filter((x) => x.id !== person.id)].slice(0, 8);
    set({ recentlyViewed: list });
  },

  setCrumb: (crumb) => {
    if (get().crumb !== crumb) set({ crumb });
  },

  setAsk: (ask) => set({ ask }),

  whenSaved: (then, what = "Zanim przejdziesz dalej") => {
    // Text typed in an open section and not yet confirmed comes first (it isn't in the archive yet).
    const draft = get().leaveGuard;
    if (draft) {
      set({
        ask: {
          title: draft,
          text: "To, co wpisano w otwartej sekcji, a nie zatwierdzono przyciskiem „Gotowe”, przepadnie.",
          icon: "warn",
          buttons: [
            { label: "Wróć do sekcji", kind: "ghost" },
            {
              label: "Odrzuć i kontynuuj",
              kind: "danger",
              run: () => {
                set({ leaveGuard: null, section: null });
                get().whenSaved(then, what);
              },
            },
          ],
        },
      });
      return;
    }
    const unsaved = get().archive?.unsavedChanges ?? 0;
    if (unsaved === 0) {
      then();
      return;
    }
    const changes = count(unsaved, "niezapisaną zmianę", "niezapisane zmiany", "niezapisanych zmian");
    set({
      ask: {
        title: `Masz ${changes}`,
        text: `${what}: zapisać je w pliku ${get().archive?.dataFile ?? ""}?`,
        icon: "warn",
        buttons: [
          { label: "Anuluj", kind: "ghost" },
          {
            label: "Odrzuć zmiany",
            kind: "secondary",
            run: async () => {
              const status = await call<ArchiveStatus>("archive.reload");
              // The undo steps and the last save's changes are gone with the reload.
              set({ section: null, lastSave: null });
              get().changed(status);
              then();
            },
          },
          {
            label: "Zapisz",
            kind: "primary",
            run: async () => {
              if (!(await get().save())) return;
              then();
              // Leaving edit mode hides the bar that would show it.
              if (get().mode !== "edit") get().notify(`Zapisano w ${get().archive?.dataFile ?? "pliku"}.`);
            },
          },
        ],
      },
    });
  },
}));

/** Moves on, unless a screen has unsaved form changes: then it asks first. */
function leaving(move: () => void) {
  const question = useStore.getState().leaveGuard;
  if (!question) return move();
  useStore.setState({
    ask: {
      title: question,
      text: "Zmiany wpisane w formularzu nie zostaną zapisane.",
      icon: "warn",
      buttons: [
        { label: "Wróć do formularza", kind: "ghost" },
        {
          label: "Odrzuć zmiany",
          kind: "danger",
          run: () => {
            useStore.setState({ leaveGuard: null });
            move();
          },
        },
      ],
    },
  });
}

/** Screens call this after a change that the API has already applied. */
export function afterChange(status?: ArchiveStatus) {
  useStore.getState().changed(status);
}
