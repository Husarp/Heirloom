import { Check, ChevronRight, GitMerge, Link2Off, SkipForward, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { call, type ApiError } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { lookForPairs, useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { EmptyState, Spinner } from "../../components/bits";
import { count, num } from "../../lib/format";
import { DeciderField, PairCard, useDecider } from "./PairCard";
import type { Pair, PairsData } from "./types";
import "./pairs.css";

const pairKey = (p: { a: string; b: string }) => `${p.a}|${p.b}`;

/** The list an answer gives back (null when it has to be looked for again: the reload after `changed` does that). */
function withPairs(data: PairsData | null, pairs: Pair[] | null): PairsData | null {
  if (!data || !pairs) return data;
  return { ...data, pairs, certain: pairs.filter((p) => p.percent >= 90).length };
}

/** „Do sprawdzenia” (design §4.3): one pair at a time — „Tak, to ta sama osoba” joins the two records in this view
 *  (the archives stay as they are), „Nie” is remembered and not asked again, „Pomiń” leaves it for later. */
export function Pairs() {
  const { data: loaded, error } = useApi<PairsData>("set.pairs", {});
  const changed = useStore((s) => s.changed);
  const notify = useStore((s) => s.notify);
  const setAsk = useStore((s) => s.setAsk);
  const go = useStore((s) => s.go);
  const [by, setBy, names] = useDecider();
  // The answers' own lists replace the loaded one until the archive changes again.
  const [data, setData] = useState<PairsData | null>(null);
  useEffect(() => setData(loaded), [loaded]);
  const [skipped, setSkipped] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);

  const order = useMemo(() => {
    const pairs = data?.pairs ?? [];
    const later = pairs.filter((p) => skipped.includes(pairKey(p)));
    return [...pairs.filter((p) => !skipped.includes(pairKey(p))), ...later];
  }, [data, skipped]);
  const current: Pair | undefined = order[0];

  const failed = useCallback((e: unknown) => notify((e as ApiError).message, { kind: "err" }), [notify]);

  const answer = useCallback(
    async (yes: boolean) => {
      if (!current || busy) return;
      if (!by.trim()) {
        notify("Wpisz, kto decyduje — zapisze się przy połączeniu.");
        return;
      }
      setBusy(true);
      try {
        const result = await call<{ status: ArchiveStatus; pairs: Pair[] | null }>("set.decide", { a: current.a, b: current.b, answer: yes ? "yes" : "no", by: by.trim(), percent: current.percent });
        setData((d) => withPairs(d, result.pairs));
        setSkipped((s) => s.filter((k) => k !== pairKey(current)));
        changed(result.status);
        const name = current.sides[0].person.name;
        if (yes) {
          notify(`Połączono: ${name}.`, {
            detail: "Teraz to jedna osoba w tym widoku. Archiwa zostały bez zmian.",
            actions: [
              { label: "Pokaż profil", run: () => go({ name: "person", id: current.a }) },
              {
                label: "Cofnij",
                run: () =>
                  call<ArchiveStatus>("set.unlink", { id: current.b })
                    .then((status) => {
                      changed(status);
                      lookForPairs();
                    })
                    .catch(failed),
              },
            ],
          });
        } else notify(`Zapamiętano: ${name} i ${current.sides[1].person.name} to różne osoby.`);
      } catch (e) {
        failed(e);
      } finally {
        setBusy(false);
      }
    },
    [current, busy, by, notify, changed, go, failed],
  );

  const skip = useCallback(() => {
    if (!current) return;
    setSkipped((s) => (order.length > 1 && s.length + 1 >= order.length ? [] : [...s.filter((k) => k !== pairKey(current)), pairKey(current)]));
  }, [current, order.length]);

  // T, N and → (design §4.3), not while typing or under a dialog.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const s = useStore.getState();
      const el = e.target as HTMLElement | null;
      if (s.ask || s.paletteOpen || e.ctrlKey || e.altKey || e.metaKey || (el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName)))) return;
      const key = e.key.toLowerCase();
      if (key === "t") answer(true);
      else if (key === "n") answer(false);
      else if (e.key === "ArrowRight") skip();
      else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [answer, skip]);

  const linkAll = () => {
    const certain = data?.certain ?? 0;
    if (!by.trim()) {
      notify("Wpisz, kto decyduje — zapisze się przy połączeniach.");
      return;
    }
    setAsk({
      title: `Połączyć ${count(certain, "pewną parę", "pewne pary", "pewnych par")}?`,
      text: "Każda para z co najmniej 90% podobieństwa stanie się w tym widoku jedną osobą. Archiwa się nie zmienią, a każde połączenie rozłączysz potem na profilu osoby.",
      icon: "info",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Połącz wszystkie",
          kind: "primary",
          run: async () => {
            try {
              const result = await call<{ linked: number; status: ArchiveStatus; pairs: Pair[] | null }>("set.linkAll", { minPercent: 90, by: by.trim() });
              setData((d) => withPairs(d, result.pairs));
              setSkipped([]);
              changed(result.status);
              notify(`Połączono ${count(result.linked, "parę", "pary", "par")}.`);
            } catch (e) {
              failed(e);
            }
          },
        },
      ],
    });
  };

  const forgetLost = async (index: number) => {
    try {
      changed(await call<ArchiveStatus>("set.forgetLost", { index }));
      setData((d) => (d ? { ...d, lost: d.lost.filter((l) => l.index !== index).map((l) => (l.index > index ? { ...l, index: l.index - 1 } : l)) } : d));
    } catch (e) {
      failed(e);
    }
  };

  if (error) return <EmptyState title="Nie da się sprawdzić par" text={error.message} />;
  if (!data)
    return (
      <div className="page">
        <div className="pairs" style={{ alignItems: "center", paddingTop: 120, color: "var(--text2)" }}>
          <Spinner size={22} />
          <span>Szukam osób, które mogą być tą samą osobą…</span>
        </div>
      </div>
    );

  const position = current ? data.pairs.findIndex((p) => pairKey(p) === pairKey(current)) : -1;

  return (
    <div className="page">
      <div className="pairs">
        <div className="row" style={{ gap: 16, alignItems: "flex-end", flexWrap: "wrap" }}>
          <div className="col grow" style={{ gap: 6, minWidth: 280 }}>
            <h1 className="page-title">To ta sama osoba?</h1>
            <span style={{ fontSize: 14, color: "var(--text2)", maxWidth: 640, lineHeight: 1.5 }}>
              Osoby z różnych archiwów, które wyglądają na tę samą. „Tak” łączy je w tym widoku — drzewa się zejdą, a pliki
              archiwów zostaną bez zmian. Decyzje zapisują się w pliku zestawu.
            </span>
          </div>
          <DeciderField by={by} setBy={setBy} names={names} />
        </div>

        {current ? (
          <>
            <div className="row" style={{ gap: 10, fontSize: 13, color: "var(--text2)" }}>
              <GitMerge size={15} color="var(--accent-text)" />
              <span>
                Para {num(position + 1)} z {num(data.pairs.length)}
              </span>
              {skipped.includes(pairKey(current)) && <span className="badge">pominięta wcześniej</span>}
            </div>
            <PairCard sides={current.sides} percent={current.percent} reasons={current.reasons} />
            <div className="row pair-actions">
              <button className="btn primary" disabled={busy} onClick={() => answer(true)}>
                <Check size={15} />
                Tak, to ta sama osoba <span className="kbd-hint">T</span>
              </button>
              <button className="btn secondary" disabled={busy} onClick={() => answer(false)}>
                <X size={15} />
                Nie <span className="kbd-hint">N</span>
              </button>
              <button className="btn ghost" disabled={busy || data.pairs.length < 2} onClick={skip}>
                <SkipForward size={15} />
                Pomiń <span className="kbd-hint">→</span>
              </button>
            </div>
          </>
        ) : (
          <div className="card">
            <EmptyState
              icon={<Check size={28} />}
              title="Nic nie czeka na sprawdzenie"
              text="Heirloom nie znalazł więcej osób, które mogą być tą samą osobą. Kogoś pominiętego połączysz ręcznie: na profilu osoby wybierz „Połącz z osobą z innego archiwum…”."
            />
          </div>
        )}

        {data.certain > 1 && (
          <div className="card row" style={{ padding: "12px 16px", gap: 12 }}>
            <span className="grow" style={{ fontSize: 14 }}>
              Pewne pary (co najmniej 90%): <b>{num(data.certain)}</b>
              <span style={{ color: "var(--text3)" }}> — przy dwóch kopiach tej samej rodziny nie trzeba klikać każdej osobno.</span>
            </span>
            <button className="btn secondary" onClick={linkAll}>
              Połącz wszystkie…
            </button>
          </div>
        )}

        {data.lost.length > 0 && (
          <div className="card">
            <div className="card-head" style={{ padding: "12px 16px", borderBottom: "1px solid var(--border)" }}>
              <Link2Off size={16} color="var(--warn)" />
              <span className="title" style={{ fontWeight: 600 }}>
                Nie znaleziono
              </span>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>połączenia z pliku zestawu, których osób już nie ma w archiwach</span>
            </div>
            {data.lost.map((l) => (
              <div key={l.index} className="list-row" style={{ minHeight: 48, gap: 10 }}>
                <span className="grow" style={{ fontSize: 14 }}>
                  {l.a.name} <span style={{ color: "var(--text3)" }}>({l.a.archiveName})</span>
                  <ChevronRight size={13} style={{ verticalAlign: -2, margin: "0 6px", color: "var(--text3)" }} />
                  {l.b.name} <span style={{ color: "var(--text3)" }}>({l.b.archiveName})</span>
                </span>
                <span style={{ fontSize: 12, color: "var(--text3)" }}>{l.reason === "same_archive" ? "obie osoby są teraz w jednym archiwum" : "nie znaleziono osoby"}</span>
                <button className="btn ghost sm" onClick={() => forgetLost(l.index)}>
                  Usuń z zestawu
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
