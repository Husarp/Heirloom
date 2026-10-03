// Shapes of what the Rust side returns (crates/heirloom-api). Kept in one place so the screens agree.

export interface RecentArchive {
  path: string;
  name: string;
  openedAt: string;
  people: number;
}

export interface Appearance {
  theme: "system" | "light" | "dark";
  textSize: number;
  density: "comfortable" | "compact";
  animations: boolean;
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
  startPerson: string | null;
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
  archive: ArchiveStatus | null;
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
}
