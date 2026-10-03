// What the import commands return (crates/heirloom-api/src/import/mod.rs) and the wizard's own state.

import { create } from "zustand";
import type { PersonSummary } from "../../api/types";

export interface ImportIssue {
  level: "error" | "warning" | "info";
  message: string;
  reference: string;
  /** `paste` (the missing part), `show`, `fix` (a date written as text). */
  action: "paste" | "show" | "skip" | "fix" | null;
  target: string | null;
}

export type Answer = "yes" | "no" | "unknown";

export interface ImportQuestion {
  id: string;
  text: string;
  about: { id: string; name: string | null }[];
  answer: Answer | null;
  note: string;
}

export interface CompareRow {
  field: string;
  label: string;
  archive: string | null;
  import: string | null;
  same: boolean;
  /** `same`, or one of `options`: keep, replace, variant, add, skip. */
  choice: string;
  options: string[];
  evidence?: { source: string | null; basis: string | null } | null;
}

export type Candidate = PersonSummary & { context: string; percent: number; score: number; reasons: string[]; years: string };

export interface ImportPerson {
  id: string;
  name: string;
  sex: string | null;
  maiden: string | null;
  years: string;
  initials: string;
  /** Decided: new / merged / skipped; undecided: match (≥ 8 points) or review. */
  status: "new" | "merged" | "skipped" | "match" | "review";
  decision: "new" | "merge" | "skip" | "undecided" | null;
  target: string | null;
  /** Others of the batch joined with the same archive person (only people who aren't relatives can be). */
  sameTarget: { id: string; name: string }[];
  candidates: Candidate[];
  compare: CompareRow[];
  facts: number;
  files: string[];
  profile: string | null;
  excluded: boolean;
  excludedFields: string[];
  summary: string | null;
}

export interface ImportFile {
  file: string;
  /** Position in the import's file list, for the preview URL. */
  index: number | null;
  name: string | null;
  path: string | null;
  size: number | null;
  status: "ok" | "dup" | "miss" | "und" | "skipped";
  kind: string;
  documentType?: string | null;
  caption: string | null;
  people: { id: string; name: string | null }[];
  transcription: boolean;
  image: boolean;
}

/** A described file's texts (`import.fileDetail`), read when its transcription is opened. */
export interface ImportFileDetail {
  file: string;
  caption: string | null;
  documentType: string | null;
  date: string | null;
  place: string | null;
  transcription: string | null;
  translation: string | null;
  note: string | null;
}

export interface ImportRelationship {
  type: string | null;
  parent: string | null;
  child: string | null;
  a: string | null;
  b: string | null;
  kind: string | null;
}

/** One line of step 1's list (design 17d): an AI answer or a file, what it is and what is still to do. */
export interface ImportInput {
  name: string;
  kind: "answer" | "photo" | "document" | "note" | "other";
  path: string | null;
  size: number | null;
  m: string | null;
  /** ok: recognised; assign: a person or a kind to point at (step 4 or here); skipped: left out; error: can't be read. */
  status: "ok" | "assign" | "skipped" | "error";
  detail: string;
  /** The key for `import.file` (files only). */
  file: string | null;
}

export interface ImportState {
  loadId: number;
  inputs: ImportInput[];
  batch: {
    name: string;
    author: string | null;
    created: string | null;
    parts: number[];
    totalParts: number | null;
    counts: { persons: number; events: number; relationships: number; texts: number; files: number; sources: number };
  };
  issues: ImportIssue[];
  result: "errors" | "warnings" | "ok";
  errors: number;
  warnings: number;
  questions: ImportQuestion[];
  persons: ImportPerson[];
  relationships: ImportRelationship[];
  files: ImportFile[];
  undecided: number;
  unresolved: { mentions: number; questions: number };
  summary: { new: number; merged: number; skipped: number; events: number; relationships: number; texts: number; files: number; duplicates: number; sources: number };
}

export interface PastImport {
  name: string;
  ts: string;
  author: string;
  people: number;
  files: number;
  /** False once it was undone (nothing of it is left to take back). */
  active: boolean;
}

export interface CommitResult {
  batch: string;
  people: number;
  merged: number;
  facts: number;
  texts: number;
  files: number;
  changes: number;
  message: string;
}

export type Step = 1 | 2 | 3 | 4 | 5;

/** The wizard's place, kept while the user looks at other screens (the batch itself lives in the backend). */
interface Wizard {
  /** The archive this import belongs to; another archive starts a new one. */
  archiveId: string | null;
  step: Step;
  /** The furthest step reached: past step 1, answers and decisions exist that a reload would drop. */
  furthest: Step;
  text: string;
  paths: string[];
  /** Files taken off step 1's list, even from inside a dropped folder. */
  exclude: string[];
  /** When files were last dropped or picked ("Upuszczone o 11:20"). */
  droppedAt: string | null;
  /** Kinds chosen by hand in step 1, by path: sent with every reload of the list, so they stay. */
  kinds: Record<string, string>;
  person: string | null;
  file: string | null;
  done: CommitResult | null;
  set: (patch: Partial<Omit<Wizard, "set" | "reset">>) => void;
  reset: () => void;
}

export const useWizard = create<Wizard>((set) => ({
  archiveId: null,
  step: 1,
  furthest: 1,
  text: "",
  paths: [],
  exclude: [],
  droppedAt: null,
  kinds: {},
  person: null,
  file: null,
  done: null,
  set: (patch) => set(patch),
  reset: () => set({ step: 1, furthest: 1, text: "", paths: [], exclude: [], droppedAt: null, kinds: {}, person: null, file: null, done: null }),
}));

/** Changes the batch through one of the `import.*` commands; they all answer with the new state. */
export type Act = (method: string, args?: object) => Promise<ImportState | null>;
