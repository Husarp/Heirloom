// The profile as `person.get` returns it (crates/heirloom-api/src/people.rs, fn `profile`).

import type { PersonSummary } from "../../api/types";

/** One fact of a person or a family as the profile's in-place editors change it (`fact.update`, `fact.delete`). */
export interface FactRef {
  /** The person's or the family's record. */
  record: string;
  /** Its place among the record's events. */
  index: number;
  tag: string;
  kind: string | null;
  /** „Urodzenie”, „Praca”… */
  label: string;
  /** As it can be typed back into the date field. */
  date: string;
  place: string;
  /** The occupation (OCCU), the residence…, or a note on a birth or an event. */
  text: string;
  textField: "value" | "note";
  /** A family event: the partner who sees it on their profile too. */
  shared: string | null;
  /** A description worked out from elsewhere („syn Antoniego i Marianny”, from the parents). */
  derived?: string;
}

export interface Fact {
  key: string;
  value: string;
  uncertain: boolean;
  sources: number[];
  /** The facts behind the cell, and the kind it can take one more of (a second occupation). */
  edit: { facts: FactRef[]; add: { record: string; tag: string; kind: string | null } | null };
}

/** `line`: who they are („żona · ślub 1904”); under it the maiden name and `years` („1882 † 1950”). */
export type Relative = PersonSummary & { label: string | null; line: string; years: string };

export interface FamilyGroup {
  title: string;
  people: Relative[];
}

export interface TextItem {
  id: string | null;
  kind: "summary" | "bio" | "story" | "saying" | "trivia" | "note";
  title: string | null;
  body: string;
  date: string | null;
  place: string | null;
  certainty: "high" | "medium" | "low" | null;
  inferred: boolean;
  sources: number[];
  people: { id: string; name: string; initials: string; branch: number }[];
}

export interface GalleryItem {
  id: string;
  path: string | null;
  caption: string | null;
  date: string | null;
  uncertain: boolean;
  profile: boolean;
}

export interface DocumentItem {
  id: string;
  path: string | null;
  kind: string;
  title: string;
  meta: string | null;
  tags: string[];
  year: string | null;
}

export interface SourceItem {
  n: number;
  id: string;
  title: string;
  page: string | null;
  repository: string | null;
}

export interface LinkItem {
  url: string;
  title: string;
  kind: string | null;
  domain: string;
}

export interface TimelineRow {
  /** Empty for a fact without a date („bez daty”, listed last). */
  date: string;
  uncertain: boolean;
  age: string | null;
  type: string;
  place: string | null;
  description: string | null;
  sources: number[];
  family: boolean;
  undated: boolean;
  /** The fact behind the row, edited in place… */
  edit: FactRef | null;
  /** …or the relative whose profile it comes from (a child's birth, a parent's death). */
  from: { id: string; name: string } | null;
}

export interface MentionItem {
  id: string | null;
  kind: string;
  title: string;
  quote: string;
  owner: string | null;
}

export interface HistoryLine {
  field: string;
  old: string | null;
  new: string | null;
  index: number;
}

export interface HistoryGroup {
  from: string;
  to: string;
  who: string;
  batch: string | null;
  kind: "import" | "manual";
  count: number;
  lines: HistoryLine[];
}

/** A link to a record that is no longer in the archive (an undone import, a file from another program). */
export interface BrokenLink {
  record: string;
  tag: string;
  xref: string;
  what: string;
}

/** One archive's record of a person, in archives opened together. */
export interface CombinedMember {
  archive: string;
  archiveName: string;
  /** The archive's folder or .ged, to open it by itself. */
  path: string;
  /** The record's id in its own archive. */
  id: string;
  /** The record's id in this view (for „Rozłącz”). */
  combinedId: string;
  name: string;
  years: string;
}

export interface Profile {
  brokenLinks?: BrokenLink[];
  /** Archives opened together: the person's records in each archive and where they differ. */
  combined?: {
    members: CombinedMember[];
    differences: { label: string; values: { archive: string; text: string }[] }[];
  };
  person: PersonSummary;
  uid: string | null;
  names: { kind: string; given: string; surname: string; nickname: string | null; orig: string | null; origLang: string | null }[];
  age: string | null;
  generationBranch: string;
  tags: string[];
  portrait: { path: string; caption: string | null; date: string | null } | null;
  summary: TextItem | null;
  facts: Fact[];
  family: FamilyGroup[];
  bio: TextItem[];
  stories: TextItem[];
  sayings: TextItem[];
  trivia: TextItem[];
  notes: TextItem[];
  gallery: GalleryItem[];
  documents: DocumentItem[];
  sources: SourceItem[];
  personSources: number[];
  links: LinkItem[];
  timeline: TimelineRow[];
  mentionedIn: MentionItem[];
  history: { origin: { ts: string; batch: string | null; who: string } | null; groups: HistoryGroup[] };
  other: { tag: string; value: string; more: string; origin: string | null }[];
  created: string | null;
  changed: string | null;
  batch: string | null;
}
