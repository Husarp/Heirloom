// A profile in archives opened together (design §1.5, §4.4, §6.4): the person's records in each archive and where they
// differ, „Edytuj w jego archiwum” (another window), „Połącz z osobą z innego archiwum…” and „Rozłącz”.

import { ChevronDown, GitMerge, Layers, Pencil, TriangleAlert, Unlink } from "lucide-react";
import { useState } from "react";
import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus, PersonSummary } from "../../api/types";
import { afterChange, lookForPairs, useStore } from "../../app/store";
import { ArchiveDot, useDismiss } from "../../components/bits";
import { Dialog } from "../../components/Dialog";
import { count } from "../../lib/format";
import { DeciderField, PairCard, useDecider } from "../pairs/PairCard";
import type { Compare } from "../pairs/types";
import type { CombinedMember, Profile } from "./types";

/** „Edytuj w jego archiwum”: the person's own archive in a new window, at this person. A person in several archives
 *  gets a menu of them. */
export function EditInArchive({ members, kind = "secondary" }: { members: CombinedMember[]; kind?: "primary" | "secondary" }) {
  const openElsewhere = useStore((s) => s.openElsewhere);
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  if (members.length === 0) return null;
  const edit = (m: CombinedMember) => {
    setOpen(false);
    openElsewhere(m.path, m.id);
  };
  const title = "Otwiera archiwum tej osoby w nowym oknie — tam można ją zmienić";
  if (members.length === 1)
    return (
      <button className={`btn ${kind}`} title={title} onClick={() => edit(members[0])}>
        <Pencil size={14} />
        Edytuj w jego archiwum
      </button>
    );
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className={`btn ${kind}`} title={title} aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <Pencil size={14} />
        Edytuj w jego archiwum
        <ChevronDown size={13} />
      </button>
      {open && (
        <div className="popover" style={{ top: 40, left: 0, width: 280, padding: "4px 0", zIndex: 5 }}>
          {members.map((m) => (
            <button key={m.combinedId} className="menu-item" onClick={() => edit(m)}>
              <ArchiveDot from={[m.archive]} size={12} style={{ boxShadow: "none" }} />
              <span className="grow ellipsis">{m.archiveName}</span>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>{m.years}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** The members of a person in archives opened together, from an id like `@a~I12@` when there's no profile (the tree's
 *  panel): a person in one archive only. */
export function memberOf(person: PersonSummary): CombinedMember | null {
  const archives = useStore.getState().archive?.combined?.archives ?? [];
  const match = /^@([^~@]+)~(.+)@$/.exec(person.id);
  const archive = match && archives.find((a) => a.key === match[1]);
  if (!match || !archive?.path) return null;
  return { archive: archive.key, archiveName: archive.name, path: archive.path, id: `@${match[2]}@`, combinedId: person.id, name: person.name, years: "" };
}

/** „Ta osoba jest w 2 archiwach”: each archive's record, „Rozłącz”, and the facts the archives give differently. */
export function CombinedCard({ data }: { data: Profile }) {
  const setAsk = useStore((s) => s.setAsk);
  const notify = useStore((s) => s.notify);
  const members = data.combined?.members ?? [];
  if (members.length < 2) return null;
  const differences = data.combined?.differences ?? [];

  const unlink = (m: CombinedMember) =>
    setAsk({
      title: members.length === 2 ? "Rozłączyć te dwie osoby?" : `Odłączyć zapis z archiwum ${m.archiveName}?`,
      text: `Zapis „${m.name}” z archiwum ${m.archiveName} nie będzie już połączony z tą osobą — w tym widoku znów będą to ${members.length === 2 ? "dwie osoby" : "osobne osoby"}. Archiwa się nie zmienią. Jeśli para wróci do „Do sprawdzenia”, odpowiedz tam „Nie”.`,
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: members.length === 2 ? "Rozłącz" : "Odłącz",
          kind: "danger",
          run: async () => {
            try {
              afterChange(await call<ArchiveStatus>("set.unlink", { id: m.combinedId }));
              lookForPairs();
              notify(`Rozłączono: ${m.name} (${m.archiveName}).`);
            } catch (e) {
              notify((e as ApiError).message, { kind: "err" });
            }
          },
        },
      ],
    });

  return (
    <section className="combined-card">
      <div className="row" style={{ gap: 10 }}>
        <Layers size={17} color="var(--accent-text)" style={{ flex: "none" }} />
        <span className="grow" style={{ fontSize: 15, fontWeight: 600 }}>
          Ta osoba jest w {count(members.length, "archiwum", "archiwach", "archiwach")}
        </span>
        {members.length === 2 && (
          <button className="btn ghost sm" onClick={() => unlink(members[1])}>
            <Unlink size={13} />
            Rozłącz…
          </button>
        )}
      </div>
      <div className="col" style={{ gap: 2 }}>
        {members.map((m) => (
          <div key={m.combinedId} className="row combined-member">
            <ArchiveDot from={[m.archive]} size={12} style={{ boxShadow: "none" }} />
            <span style={{ fontWeight: 600 }}>{m.archiveName}</span>
            <span className="grow ellipsis" style={{ color: "var(--text2)" }}>
              {m.name}
              {m.years && ` · ${m.years}`}
            </span>
            {members.length > 2 && (
              <button className="btn ghost xs" onClick={() => unlink(m)}>
                <Unlink size={12} />
                Odłącz…
              </button>
            )}
            <button className="btn ghost xs" onClick={() => useStore.getState().openElsewhere(m.path, m.id)} title="Otwiera to archiwum w nowym oknie">
              <Pencil size={12} />
              Edytuj w tym archiwum
            </button>
          </div>
        ))}
      </div>
      {differences.length > 0 && (
        <div className="col" style={{ gap: 6 }}>
          <span className="label-caps">Różnice</span>
          <table className="combined-diff">
            <tbody>
              {differences.map((d) => (
                <tr key={d.label}>
                  <th>{d.label}</th>
                  {d.values.map((v) => (
                    <td key={v.archive}>
                      <span className="row" style={{ gap: 6 }}>
                        <ArchiveDot from={[v.archive]} size={10} style={{ boxShadow: "none" }} />
                        <span>{v.text}</span>
                      </span>
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** „Połącz z osobą z innego archiwum…”: the people search narrowed to the other archives, then the two side by side
 *  and „Tak, to ta sama osoba” (design §4.4). */
export function LinkWithOther({ person }: { person: PersonSummary }) {
  const setPalette = useStore((s) => s.setPalette);
  const notify = useStore((s) => s.notify);
  const [compare, setCompare] = useState<{ result: Compare; other: string } | null>(null);
  const mine = person.from ?? [];

  const pick = async (other: string) => {
    try {
      const result = await call<Compare>("set.compare", { a: person.id, b: other });
      if (result.sameArchive) notify("To dwie osoby z jednego archiwum — połącz je w tym archiwum.");
      else if (result.linked) notify("Te osoby są już połączone.");
      else setCompare({ result, other });
    } catch (e) {
      notify((e as ApiError).message, { kind: "err" });
    }
  };

  return (
    <>
      <button
        className="btn ghost"
        title="Ta sama osoba jest też w innym archiwum zestawu"
        onClick={() =>
          setPalette(true, {
            scope: "people",
            pick,
            placeholder: "Z kim połączyć? Szukaj w innych archiwach…",
            only: (p) => !(p.from ?? []).some((k) => mine.includes(k)),
          })
        }
      >
        <GitMerge size={14} />
        Połącz z osobą z innego archiwum…
      </button>
      {compare && <CompareDialog person={person} compare={compare.result} other={compare.other} onClose={() => setCompare(null)} />}
    </>
  );
}

function CompareDialog({ person, compare, other, onClose }: { person: PersonSummary; compare: Compare; other: string; onClose: () => void }) {
  const notify = useStore((s) => s.notify);
  const [by, setBy, names] = useDecider();
  const [busy, setBusy] = useState(false);
  const sexes = [compare.a.person.sex, compare.b.person.sex];
  const differentSex = sexes.every((s) => s === "M" || s === "F") && sexes[0] !== sexes[1];
  const link = async () => {
    if (!by.trim()) {
      notify("Wpisz, kto decyduje — zapisze się przy połączeniu.");
      return;
    }
    setBusy(true);
    try {
      const result = await call<{ status: ArchiveStatus }>("set.decide", { a: person.id, b: other, answer: "yes", by: by.trim(), how: "manual", percent: compare.percent });
      afterChange(result.status);
      lookForPairs();
      notify(`Połączono: ${person.name}.`, { detail: "Teraz to jedna osoba w tym widoku. Archiwa zostały bez zmian." });
      onClose();
    } catch (e) {
      notify((e as ApiError).message, { kind: "err" });
      setBusy(false);
    }
  };
  return (
    <Dialog width={900} onClose={onClose} labelledBy="compare-title">
      <div className="dialog-body" style={{ gap: 14 }}>
        <div className="row" style={{ gap: 12 }}>
          <div className="dialog-title grow" id="compare-title">
            To ta sama osoba?
          </div>
          <DeciderField by={by} setBy={setBy} names={names} />
        </div>
        <PairCard sides={[compare.a, compare.b]} percent={compare.percent} reasons={compare.reasons.length ? compare.reasons : ["mało wspólnego"]} />
        {differentSex && (
          <span className="row" style={{ gap: 6, fontSize: 13, color: "var(--warn)" }}>
            <TriangleAlert size={14} style={{ flex: "none" }} />
            Jedna z tych osób to kobieta, a druga mężczyzna — sprawdź, czy to na pewno ta sama osoba.
          </span>
        )}
        <span style={{ fontSize: 13, color: "var(--text2)" }}>„Tak” połączy te dwa zapisy w jedną osobę w tym widoku. Archiwa się nie zmienią; rozłączysz je na profilu.</span>
      </div>
      <div className="dialog-foot">
        <button className="btn ghost" onClick={onClose}>
          Anuluj
        </button>
        <button className="btn primary" disabled={busy} onClick={link}>
          Tak, to ta sama osoba
        </button>
      </div>
    </Dialog>
  );
}
