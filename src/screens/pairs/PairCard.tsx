import { useState } from "react";
import { useStore } from "../../app/store";
import { ArchiveBadge, Avatar } from "../../components/bits";
import type { Kin, Side } from "./types";
import "./pairs.css";

/** „To ta sama osoba?”: the two people side by side with their closest family, and between them how sure Heirloom is
 *  and why (design §4.3). Names that appear on both sides are marked. */
export function PairCard({ sides, percent, reasons }: { sides: [Side, Side]; percent: number; reasons: string[] }) {
  const names = (s: Side) => new Set([...s.parents, ...s.partners, ...s.children].map((k) => k.name));
  const [left, right] = [names(sides[0]), names(sides[1])];
  return (
    <div className="pair-card">
      <SideColumn side={sides[0]} other={right} />
      <div className="pair-middle">
        <span className={`pair-percent${percent >= 90 ? " high" : ""}`}>{percent}%</span>
        <span className="pair-verdict">{percent >= 90 ? "prawie na pewno" : percent >= 70 ? "prawdopodobnie" : percent >= 40 ? "może" : "raczej nie"}</span>
        <div className="pair-reasons">
          {reasons.map((r) => (
            <span key={r} className="pair-reason">
              {r}
            </span>
          ))}
        </div>
      </div>
      <SideColumn side={sides[1]} other={left} />
    </div>
  );
}

function SideColumn({ side, other }: { side: Side; other: Set<string> }) {
  const p = side.person;
  const places = [p.birthPlace && `ur. ${p.birthPlace}`, p.deathPlace && `zm. ${p.deathPlace}`].filter(Boolean).join(" · ");
  return (
    <div className="pair-side">
      <ArchiveBadge from={p.from} style={{ alignSelf: "flex-start" }} />
      <div className="row" style={{ gap: 12, alignItems: "flex-start" }}>
        <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={56} />
        <div className="col" style={{ gap: 2, minWidth: 0 }}>
          <span className="pair-name">{p.name}</span>
          {p.maiden && <span style={{ fontSize: 14, color: "var(--text2)" }}>z d. {p.maiden}</span>}
          <span className="num" style={{ fontSize: 14 }}>
            {side.years || "—"}
          </span>
          {places && <span style={{ fontSize: 13, color: "var(--text2)" }}>{places}</span>}
        </div>
      </div>
      <KinList label="Rodzice" people={side.parents} other={other} />
      <KinList label={side.partners.length > 1 ? "Małżonkowie" : "Małżonek"} people={side.partners} other={other} />
      <KinList label="Dzieci" people={side.children} other={other} />
    </div>
  );
}

function KinList({ label, people, other }: { label: string; people: Kin[]; other: Set<string> }) {
  return (
    <div className="col" style={{ gap: 3 }}>
      <span className="label-caps">{label}</span>
      {people.length === 0 ? (
        <span style={{ fontSize: 13, color: "var(--text3)" }}>brak w archiwum</span>
      ) : (
        people.map((k) => (
          <span key={k.id} className={`pair-kin${other.has(k.name) ? " same" : ""}`} title={other.has(k.name) ? "Jest też po drugiej stronie" : undefined}>
            <span className="ellipsis">{k.name}</span>
            <span className="num" style={{ color: "var(--text3)", whiteSpace: "nowrap" }}>
              {k.years}
            </span>
          </span>
        ))
      )}
    </div>
  );
}

const DECIDES_KEY = "heirloom.decides";

/** „Decyduje”: who answers „To ta sama osoba?” (written with the link in the set file). No „Kto edytuje?”, as no
 *  archive is edited; the last name is remembered on this computer. */
export function useDecider(): [string, (name: string) => void, string[]] {
  const archive = useStore((s) => s.archive);
  const lastEditor = useStore((s) => s.app?.lastEditor ?? null);
  const names = [...new Set([...(lastEditor ? [lastEditor] : []), ...(archive?.editors ?? []).map((e) => e.name)])];
  const [by, setBy] = useState<string>(() => {
    try {
      return localStorage.getItem(DECIDES_KEY) || names[0] || "";
    } catch {
      return names[0] || "";
    }
  });
  const set = (name: string) => {
    setBy(name);
    try {
      localStorage.setItem(DECIDES_KEY, name);
    } catch {
      // Not remembered; it still works for now.
    }
  };
  return [by, set, names];
}

export function DeciderField({ by, setBy, names }: { by: string; setBy: (name: string) => void; names: string[] }) {
  return (
    <label className="row" style={{ gap: 8, fontSize: 13, color: "var(--text2)" }}>
      Decyduje:
      <input className="input" style={{ width: 170, height: 32 }} list="pair-deciders" value={by} onChange={(e) => setBy(e.target.value)} placeholder="Twoje imię" />
      <datalist id="pair-deciders">
        {names.map((n) => (
          <option key={n} value={n} />
        ))}
      </datalist>
    </label>
  );
}
