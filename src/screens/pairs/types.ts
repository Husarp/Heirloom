// „To ta sama osoba?” in archives opened together, as the `set.*` commands return it (crates/heirloom-api/src/combined).

import type { PersonSummary } from "../../api/types";

/** A parent, partner or child on one side of a pair. */
export interface Kin {
  id: string;
  name: string;
  years: string;
}

/** One person of a pair with their closest family. */
export interface Side {
  person: PersonSummary;
  years: string;
  parents: Kin[];
  partners: Kin[];
  children: Kin[];
}

/** Two people of different archives who may be the same person; `a` is from the earlier archive. */
export interface Pair {
  a: string;
  b: string;
  percent: number;
  score: number;
  status: "match" | "review";
  reasons: string[];
  sides: [Side, Side];
}

/** A link kept in the set file whose person is no longer found. */
export interface Lost {
  index: number;
  a: LostRef;
  b: LostRef;
  reason: "not_found" | "same_archive";
}

export interface LostRef {
  archive: string;
  archiveName: string;
  xref: string;
  name: string;
}

export interface PairsData {
  pairs: Pair[];
  lost: Lost[];
  /** Pairs at 90% or more. */
  certain: number;
}

/** `set.compare`: any two people, for „Połącz z osobą z innego archiwum…”. */
export interface Compare {
  a: Side;
  b: Side;
  percent: number;
  score: number;
  reasons: string[];
  sameArchive: boolean;
  linked: boolean;
}
