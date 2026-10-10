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
  /** How this person works with the archive on this computer: „Pracuj na archiwum”, „Nie ruszaj oryginału”, or null
   *  until asked (the question on first open). Sets have none. */
  workMode?: "inPlace" | "overlay" | null;
  /** „Nie ruszaj oryginału”: where the user's changes are and what the archive doesn't have yet. `root`, `dataFile`
   *  and `dataPath` above are the original's. */
  overlay?: OverlayStatus | null;
  /** Said once after opening (changes found again, a write-back finished, the archive's changes taken in). */
  notice?: string;
  /** A „Uporządkuj” run that was cut off: the banner „Przenoszenie plików zostało przerwane (przeniesiono 120 z 340)”. */
  layoutInterrupted?: LayoutInterrupted | null;
}

export interface OverlayStatus {
  /** The folder of „Twoje zmiany”. */
  folder: string;
  /** „Dokumenty › Heirloom › Moje zmiany › Rodzina Kowalskich”. */
  folderLabel: string;
  dataPath: string;
  /** Changes (records) the archive doesn't have yet. */
  pending: number;
  /** Someone changed the archive since the last sync. */
  originalChanged: boolean;
  /** The archive can't be opened now (a pendrive unplugged). */
  originalMissing: boolean;
  lastWriteBack: string | null;
  canUndoWriteBack: boolean;
  /** A write-back that didn't finish: why, for the banner. */
  interrupted: string | null;
  /** The archive and the user's folder are on one disk (no close reminder then). */
  sameDrive: boolean;
}

/** What the question „Jak chcesz pracować z tym archiwum?” shows. */
export interface ModeInfo {
  writable: boolean;
  /** The program that made the file, when it isn't Heirloom. */
  foreign: string | null;
  usedBefore: boolean;
  editors: Editor[];
  folder: { path: string; label: string };
  suggested: "inPlace" | "overlay";
}

/** Ustawienia › Archiwum › Twoje zmiany. */
export interface OverlayInfo {
  folder: string;
  folderLabel: string;
  pending: number;
  people: number;
  newFiles: number;
  newBytes: number;
  writable: boolean;
  lockedOriginal: boolean;
  lastWriteBack: string | null;
  canUndoWriteBack: boolean;
  originalChanged: boolean;
  originalMissing: boolean;
}

/** What „Przenieś zmiany do archiwum” would do. */
export interface WriteBackPreview {
  changes: number;
  people: number;
  files: number;
  bytes: number;
  originalChanged: boolean;
  foreign: { origin: string; version: string | null; dataFile: string } | null;
  lockedOriginal: boolean;
  writable: boolean;
  originalMissing: boolean;
  root: string;
  name: string;
}

/** The automatic backup (.zip) of the archive: where it goes and what to warn about. */
export interface BackupInfo {
  folder: string;
  folderLabel: string;
  files: number;
  bytes: number;
  needed: number;
  free: number | null;
  lastZip: { path: string; at: string; bytes: number; exists: boolean } | null;
  /** The last backup has exactly what the archive has now. */
  current: boolean;
  inOneDrive: boolean;
  sameDisk: boolean;
  cloudOnly: { files: number; bytes: number };
}

/** `archive.backupAuto`: the .zip that was made. */
export interface BackupResult {
  path: string;
  bytes: number;
  files: number;
}

/** `job.progress` / `job.cancel` (answered without waiting for the running job). */
export interface JobProgress {
  running: boolean;
  label: string;
  done: number;
  total: number;
  cancelling: boolean;
}

/** `archive.writeBack` → { result, status }. */
export interface WriteBackResult {
  result: { changes: number; files: number; bytes: number };
  status: ArchiveStatus;
}

/** One record changed on both sides (`archive.rebase`). Texts are the record as GEDCOM lines; null: deleted there. */
export interface RebaseConflict {
  key: string;
  tag: string;
  name: string;
  /** The side kept when no choice is given. */
  default: "mine" | "theirs";
  mine: string | null;
  theirs: string | null;
  base: string | null;
}

/** `archive.rebase` → { result, status }. `merged: false` with `conflicts`: nothing was changed, choose and call again with
 *  `choices` (or `preview: true` first, which never changes anything). */
export interface RebaseResult {
  result:
    | { merged: true; fastForward: boolean; changes: number; conflicts: [] }
    | { merged: false; conflicts: RebaseConflict[]; theirChanges: number; myChanges: number };
  status: ArchiveStatus;
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
  /** Shown in the user's version („Nie ruszaj oryginału” on this computer). */
  withChanges?: boolean;
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

// ---------- Part B: where the photos lie in media/ (media.scan and the commands that move or assign files) ----------

export type MediaLayout = "flat" | "perPerson";

export interface LayoutInterrupted {
  /** The journal's name: give it to media.finishRestructure / media.undoRestructure. */
  journal: string;
  done: number;
  total: number;
  layout: MediaLayout | null;
}

/** A person as the scan names them („Józef Kowalski (1878–1944)” = name + years). */
export interface LayoutPerson {
  id: string;
  name: string;
  /** „1878–1944”, „1878–”, „–1944”, or null. */
  years: string | null;
}

export interface ScanUnknown {
  /** Path inside the archive, `/` separators: give it to media.assign / media.leaveAlone. */
  path: string;
  name: string;
  size: number;
  /** `media` or `media/<folder>`. */
  folder: string;
  /** Prefilled when exactly one name fits, or the file lies in a person's folder. `why`: „pasuje nazwa pliku” | „w folderze osoby”. */
  suggestion: { id: string; name: string; why: string } | null;
  /** Two or three people that fit the name (nothing prefilled). */
  candidates: LayoutPerson[];
}

export interface ScanFolder {
  path: string;
  name: string;
  files: number;
  linked: number;
  unknown: number;
  person: LayoutPerson | null;
  /** Left alone earlier and not changed since. */
  leftAlone: boolean;
  /** Left alone earlier but its files changed: asked again. */
  changed: boolean;
  /** `unpack`: in a flat archive (B1) a folder to take out or leave; `whose`: in B2 a folder of nobody with unknown files. */
  question: "unpack" | "whose" | null;
}

export interface JournalSummary {
  name: string;
  ts: string;
  state: "saved" | "interrupted";
  moved: number;
  total: number;
}

export interface MediaScan {
  layout: {
    setting: MediaLayout | null;
    guess: MediaLayout;
    /** The setting, else the guess. */
    effective: MediaLayout;
    rootOwned: number;
    folderOwned: number;
    personFolders: number;
  };
  /** FILE values that point at nothing but whose file is elsewhere in the archive. */
  moved: { id: string; file: string; found: string }[];
  /** B2: files of people lying loose in media/ (`ids`: every record that names the file). */
  looseLinked: { id: string; ids: string[]; file: string; owner: LayoutPerson }[];
  unknown: ScanUnknown[];
  folders: ScanFolder[];
  /** Absolute FILE paths outside the archive that exist. */
  outside: { id: string; file: string }[];
  counts: { moved: number; looseLinked: number; unknown: number; folders: number; outside: number; missing: number; files: number };
  /** Everything to tidy: the banner shows it when > 0 and `!seen`. */
  total: number;
  /** Give it to media.tidySeen („Nie teraz”). */
  fingerprint: string;
  /** „Nie teraz” was said to exactly these findings (on this computer). */
  seen: boolean;
  /** More than 50 000 files: the scan stopped. */
  truncated: boolean;
  /** The last saved or open „Uporządkuj” run (Settings: „Ostatnie porządkowanie”), else null. */
  journal: JournalSummary | null;
  interrupted: LayoutInterrupted | null;
}

export interface RestructurePlan {
  to: MediaLayout;
  count: number;
  bytes: number;
  /** The first 300 moves. */
  moves: { from: string; to: string; objects: string[]; unknown: boolean }[];
  unknownMoves: number;
  /** Files in the media root that belong to nobody: they stay. */
  unassigned: number;
  /** Files of several people (go to one folder). */
  groupFiles: number;
  /** Files several records point at (moved once). */
  shared: number;
  alreadyInPlace: number;
  newFolders: string[];
  shortened: { from: string; to: string }[];
  longPaths: number;
}

export interface RestructureResult {
  moved: number;
  /** Files not moved: `reason` is `held` (open in another program), `locked`, `missing` or `error`. */
  skipped: { from: string; reason: "held" | "locked" | "missing" | "error" }[];
  /** Skipped for a reason other than `missing`: „6 plików było otwartych w innym programie”. */
  held: number;
  /** The journal's name, for „Cofnij porządkowanie”; null when nothing moved. */
  journal: string | null;
  batch?: string;
  layout: MediaLayout | null;
  removedFolders: string[];
  /** Unknown files that came along (unpacking a folder): offer them in the assign list. */
  unknownMoved: string[];
  /** Of those, assigned to `assignTo`. */
  assigned: number;
  /** A fresh media.scan. */
  report: MediaScan;
  status: ArchiveStatus;
}

export interface UndoRestructureResult {
  movedBack: number;
  notBack: number;
  journal: string;
  removedFolders: string[];
  report: MediaScan;
  status: ArchiveStatus;
}
