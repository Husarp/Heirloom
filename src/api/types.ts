// Shapes of what the Rust side returns (crates/heirloom-api). Kept in one place so the screens agree.

export interface RecentArchive {
  path: string;
  name: string;
  openedAt: string;
  people: number;
  /** A set of archives opened together („Otwórz razem…”, a `.heirloom-zestaw` file); older entries have no kind. */
  kind?: "archive" | "set";
  /** A set's number of archives. */
  archives?: number;
}

export interface Appearance {
  theme: "system" | "light" | "dark";
  textSize: number;
  density: "comfortable" | "compact";
  animations: boolean;
  /** What opening an archive shows: the Start screen, or the place where it was left („Po otwarciu archiwum”). */
  startIn: "start" | "last";
}

export interface FileWarning {
  line: number;
  message: string;
}

export interface ArchiveStatus {
  archiveId: string;
  name: string;
  root: string;
  dataFile: string;
  dataPath: string;
  origin: string | null;
  foreign: boolean;
  foreignConfirmed: boolean;
  readOnly: boolean;
  unsavedChanges: number;
  canUndo: boolean;
  canRedo: boolean;
  /** How many steps can be undone (a profile section undoes back to where it was opened). */
  undoDepth: number;
  changedOnDisk: boolean;
  warnings: FileWarning[];
  encoding: string;
  people: number;
  families: number;
  editors: Editor[];
  /** Display choices stored with the archive (tree, names, surname joins …). */
  display: Record<string, unknown>;
  backupsToKeep: number;
  /** A copy of the data file in .heirloom/kopie before each save (Ustawienia › Kopie zapasowe). */
  backupBeforeSave: boolean;
  /** When the newest of those copies was made. */
  lastBackup: string | null;
  /** The data file's size. */
  dataBytes: number | null;
  /** HEAD.GEDC.VERS: "7.0", "5.5.1". */
  gedcomVersion: string | null;
  sidecar: string;
  lastSaved: string | null;
  /** Only when archives are opened together (a set): then the status is the set's, and `readOnly` is always on. */
  combined?: CombinedInfo;
}

/** Archives opened together („Otwórz razem…”, crates/heirloom-api/src/combined). */
export interface CombinedInfo {
  setPath: string;
  archives: CombinedArchive[];
  /** People who stand for records in more than one archive. */
  linked: number;
  /** Links in the set file whose people are no longer found. */
  lost: number;
  /** Pairs „Do sprawdzenia”; null until they were looked for (`set.pairs`). */
  pending: number | null;
}

export interface CombinedArchive {
  /** The archive's key in the set („a”, „b” …), the one in `PersonSummary.from` and in combined ids (`@a~I12@`). */
  key: string;
  id: string;
  name: string;
  /** 1-based: `--arch1` … */
  colour: number;
  state: "ok" | "missing" | "unreadable" | "other";
  error: string | null;
  people: number;
  root: string | null;
  /** The folder or .ged to open the archive by itself. */
  path: string | null;
  dataPath: string | null;
}

/** A name offered by „Kto edytuje?”. */
export interface Editor {
  name: string;
  lastEdited: string | null;
}

export interface AppState {
  version: string;
  recent: RecentArchive[];
  appearance: Appearance;
  lastEditor: string | null;
  /** „Sprawdzaj aktualizacje” (this computer's setting). */
  updates: { check: boolean };
  archive: ArchiveStatus | null;
  /** Where the open archive was left on this computer (aplikacja.json); null before anything was remembered. */
  place: Place | null;
}

/** An archive's last place: the screen and the tree as last seen are routes of the UI (app/store.ts `Route`). */
export interface Place {
  route: unknown;
  tree: unknown;
  /** „Ostatnio oglądane”, newest first. */
  viewed: string[];
  at: string;
}

/** What the update checks know (crates/heirloom-api/src/update.rs). */
export interface UpdateStatus {
  /** False in the self-test, which never goes online. */
  enabled: boolean;
  current: string;
  /** GitHub has answered since the start. */
  checked: boolean;
  latest: string | null;
  newer: boolean;
  /** The newest release has a Windows installer. */
  installer: boolean;
  /** The newest release's page, or the list of releases. */
  page: string;
  download:
    | { state: "none" }
    | { state: "running"; version: string; done: number; total: number }
    | { state: "ready"; version: string }
    | { state: "failed"; version: string; message: string };
}

export interface SaveResult {
  backup: string | null;
  changes: number;
  savedAt: string;
  /** What names this save's history entries, for „Cofnij zapis”; null when the history couldn't be written. */
  history: { ts: string; author: string } | null;
  status: ArchiveStatus;
}

/** A date as the interface shows it. */
export interface DateText {
  /** Long form: "12 marca 1878", "ok. 1850". */
  text: string;
  /** Short form: "12.03.1878". */
  short: string;
  /** For cards: "1878", "ok. 1850". */
  year: string;
  uncertain: boolean;
  sort: number | null;
}

/** One person in lists, cards and pickers. */
export interface PersonSummary {
  id: string;
  name: string;
  given: string;
  surname: string;
  maiden: string | null;
  nickname: string | null;
  sex: "M" | "F" | "X" | "U";
  birth: DateText | null;
  death: DateText | null;
  birthPlace: string | null;
  deathPlace: string | null;
  living: boolean;
  /** 1–12, the colour of the person's birth family (`--b1` …). */
  branch: number;
  /** The colour of the surname borne now: a wife who took her husband's name has his family's colour. */
  surnameBranch: number;
  branchName: string;
  generation: number | null;
  initials: string;
  photo: string | null;
  photoCount: number;
  changed: string | null;
  created: string | null;
  /** Archives opened together: the keys of the archives the person is in (two or more for a linked person). */
  from?: string[];
}
