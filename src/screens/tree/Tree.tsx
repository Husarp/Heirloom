import { Crosshair, GitCommitVertical, IdCard, LocateFixed, Maximize, Minus, Palette, Plus, RotateCcw } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { rememberPlace, useStore, type TreeCamera, type TreeRoute } from "../../app/store";
import { useApi } from "../../app/useApi";
import { useDark } from "../../app/useAppearance";
import { Dropdown, Segmented, Spinner } from "../../components/bits";
import { people as peopleCount } from "../../lib/format";
import { FocusCanvas, type FocusCanvasHandle } from "./FocusCanvas";
import { SIDE, sideColors } from "./colors";
import { parentsOrdered, type Graph, type GraphPerson } from "./graph";
import { directLine, layoutAncestors, layoutDescendants, layoutFamily, pathBetween } from "./layout";
import { OverviewCanvas, type OverviewData, type OverviewHandle } from "./OverviewCanvas";
import { SidePanel } from "./SidePanel";
import "./tree.css";

type View = "family" | "ancestors" | "descendants" | "overview";
type ColorMode = "branch" | "surname" | "side" | "generation" | "none";

const DEPTH: Record<Exclude<View, "overview">, { up: number; down: number }> = {
  family: { up: 1, down: 1 },
  ancestors: { up: 3, down: 0 },
  descendants: { up: 0, down: 2 },
};

/** Drzewo (spec §3, §4.1–§4.5): loaded only when opened, idle while hidden (PLAN §11.1). */
export function Tree({ hidden }: { hidden: boolean }) {
  const route = useStore((s) => s.route);
  const archive = useStore((s) => s.archive);
  const viewedList = useStore((s) => s.recentlyViewed);
  const mode = useStore((s) => s.mode);
  const go = useStore((s) => s.go);
  const replaceRoute = useStore((s) => s.replaceRoute);
  const setCrumb = useStore((s) => s.setCrumb);
  const requireEdit = useStore((s) => s.requireEdit);
  const animations = useStore((s) => s.app?.appearance.animations ?? true);
  const dark = useDark();
  const view: View = route.name === "tree" ? route.view : "family";
  const [focus, setFocus] = useState<string | null>(route.name === "tree" && route.person ? route.person : null);
  const [selected, setSelected] = useState<string | null>(route.name === "tree" ? (route.sel ?? route.person ?? null) : null);
  // A camera to come back to (Back, „Ostatnie miejsce”), handed to the canvas of that view and that person.
  const [restore, setRestore] = useState<{ view: View; person: string; cam: TreeCamera } | null>(null);
  const [panelOpen, setPanelOpen] = useState(true);
  const [hovered, setHovered] = useState<string | null>(null);
  const [siblingsOpen, setSiblingsOpen] = useState(false);
  const display = (archive?.display ?? {}) as Record<string, unknown>;
  const [colorMode, setColorMode] = useState<ColorMode>(((display.treeColor as ColorMode) ?? "branch"));
  const [lineOn, setLineOn] = useState(true);
  const [photos, setPhotos] = useState((display.cardStyle as string) !== "plain");
  // Changed in Ustawienia while the tree stays mounted in the background.
  useEffect(() => setColorMode((display.treeColor as ColorMode) ?? "branch"), [display.treeColor]);
  useEffect(() => setPhotos((display.cardStyle as string) !== "plain"), [display.cardStyle]);
  const [zoom, setZoom] = useState(1);
  const canvas = useRef<FocusCanvasHandle>(null);
  const overview = useRef<OverviewHandle>(null);

  // The route leads: a new person in it (Back, Forward, a link from another screen) becomes the centre, and a route
  // that was left carries the camera and the selection to come back to.
  useEffect(() => {
    if (route.name !== "tree") return;
    if (route.person && route.person !== focus) {
      setFocus(route.person);
      setSelected(route.sel ?? route.person);
      setSiblingsOpen(false);
    } else if (route.sel) setSelected(route.sel);
    if (route.cam && route.person) setRestore({ view: route.view, person: route.person, cam: route.cam });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route]);
  // …and follows the tree: a new centre chosen here (or below) is written into the current route, so coming back
  // to it lands on the same person.
  useEffect(() => {
    const current = useStore.getState().route;
    if (hidden || !focus || current.name !== "tree" || current.person === focus) return;
    replaceRoute({ name: "tree", view: current.view, person: focus });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focus]);
  // Opened without a person: where the tree was last seen in this archive (at its zoom and place), the last person
  // viewed, else a suggestion.
  useEffect(() => {
    if (focus) return;
    const place = useStore.getState().treePlace;
    const start = place?.person ?? viewedList[0]?.id;
    if (start) {
      setFocus(start);
      setSelected(place?.person ? (place.sel ?? start) : start);
      if (place?.person && place.cam && place.view === view) setRestore({ view, person: place.person, cam: place.cam });
      return;
    }
    suggested()
      .then((s) => {
        if (s) {
          setFocus(s.id);
          setSelected(s.id);
        }
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focus, viewedList]);

  // Przodkowie and Potomkowie use the full width (spec §4.2, §4.3; the panel opens when someone is chosen there);
  // Rodzina shows it (§4.1).
  useEffect(() => setPanelOpen(view === "family"), [view]);

  const depth = view === "overview" ? null : DEPTH[view];
  // Paused while hidden: coming back fetches nothing (and redraws nothing) unless the archive changed meanwhile.
  const { data: graphData, loading, error: graphError } = useApi<Graph>(focus && depth ? "tree.graph" : null, { id: focus, up: depth?.up, down: depth?.down }, !hidden);
  // While another person loads, the previous graph stays on screen; a graph fetched for another view does not.
  const graph = graphData && depth && graphData.up === depth.up && graphData.down === depth.down ? graphData : null;
  // The person in the middle is gone (deleted, or their adding undone): start again from a suggestion.
  useEffect(() => {
    if (graphError?.code !== "not_found") return;
    suggested(focus)
      .then((s) => {
        if (s) {
          setFocus(s.id);
          setSelected(s.id);
        }
      })
      .catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [graphError]);
  // Everyone at once; the centre person's line is drawn from it on screen, so a new centre fetches nothing, and
  // neither does coming back from another view while the archive is unchanged.
  const { data: overviewData, loading: overviewLoading } = useApi<OverviewData>("tree.overview", undefined, !hidden && view === "overview");

  useEffect(() => {
    if (hidden) return;
    const name = focus && graph?.people[focus]?.name;
    setCrumb(view === "overview" ? null : name || null);
  }, [hidden, focus, graph, view, setCrumb]);

  const rounded = display.treeLines === "rounded";
  const scene = useMemo(() => {
    if (!graph || !focus || !graph.people[focus]) return null;
    if (view === "ancestors") return layoutAncestors(graph, focus, 3, rounded);
    if (view === "descendants") return layoutDescendants(graph, focus, 2, rounded);
    return layoutFamily(graph, focus, { siblingsOpen, editing: mode === "edit", rounded });
  }, [graph, focus, view, siblingsOpen, mode, rounded]);

  // Colours by the chosen mode (spec §3.2 „Koloruj wg”); „strona” is relative to the person in the centre.
  const sides = useMemo(
    () => (graph && focus && colorMode === "side" ? sideColors(focus, (id) => parentsOrdered(graph, id), (id) => graph.people[id]?.children ?? []) : null),
    [graph, focus, colorMode],
  );
  const colorOf = useCallback(
    (p: GraphPerson): number | null => {
      switch (colorMode) {
        case "none":
          return null;
        case "generation":
          return p.generation ? ((p.generation - 1) % 12) + 1 : null;
        case "side":
          return sides?.get(p.id) ?? null;
        case "surname":
          return p.surnameBranch;
        default:
          return p.branch;
      }
    },
    [colorMode, sides],
  );

  const highlighted = useMemo(() => {
    if (!graph || !focus) return new Set<string>();
    if (view === "family") return directLine(graph, selected && graph.people[selected] ? selected : focus);
    if (hovered && graph.people[hovered]) return pathBetween(graph, focus, hovered);
    if (view === "ancestors") {
      // The paternal line (spec §4.2).
      const line = new Set<string>([focus]);
      let at: string | null = focus;
      while (at) {
        const [father]: (string | null)[] = parentsOrdered(graph, at);
        if (father) line.add(father);
        at = father;
      }
      return line;
    }
    return new Set<string>([focus]);
  }, [graph, focus, view, selected, hovered]);

  const lineSet = useMemo(() => (graph && lineOn && selected && graph.people[selected] ? directLine(graph, selected) : null), [graph, lineOn, selected]);
  const dimmed = useCallback(
    (id: string) => {
      const p = graph?.people[id];
      if (!p) return false;
      if (lineSet && view === "family" && lineOn && selected !== focus && !lineSet.has(id)) return true;
      return false;
    },
    [graph, lineSet, view, lineOn, selected, focus],
  );

  // A new person in the centre is a step in Back/Forward history; keyboard steps (`replace`) only update the current
  // one, so Back doesn't walk back key by key.
  const refocus = useCallback(
    (id: string, replace = false) => {
      setSelected(id);
      setSiblingsOpen(false);
      setRestore(null);
      if (id === focus) return;
      const next: TreeRoute = { name: "tree", view, person: id };
      if (replace) replaceRoute(next);
      else go(next);
    },
    [focus, view, go, replaceRoute],
  );

  // Back and Forward (and the last place) keep the camera and the selection of the tree as it was left.
  const snapshot = useRef<() => Pick<TreeRoute, "person" | "cam" | "sel">>(() => ({}));
  snapshot.current = () => ({
    person: focus ?? undefined,
    sel: selected ?? undefined,
    cam: (view === "overview" ? overview.current?.getView() : canvas.current?.getView()) ?? restore?.cam,
  });
  useEffect(() => {
    const take = () => snapshot.current();
    useStore.getState().setTreeSnapshot(take);
    return () => {
      if (useStore.getState().treeSnapshot === take) useStore.getState().setTreeSnapshot(null);
    };
  }, []);

  // Keyboard (design 17g, PLAN §4.2): ↑ a parent (↑ again: the other parent), ↓ the first child, ← → siblings and
  // partners in the row, Enter shows the panel, Shift Enter opens the profile, Home goes to the start person, Esc
  // closes the panel. A person outside the part of the tree on screen becomes its centre.
  const upFrom = useRef<{ child: string; parent: string } | null>(null);
  const [announce, setAnnounce] = useState("");
  useEffect(() => {
    if (hidden || view === "overview") return;
    const onKey = (e: KeyboardEvent) => {
      // The keys belong to the tree, not to a field or a control of the toolbar (a switch uses ← → itself).
      const active = document.activeElement as HTMLElement | null;
      if (active && active !== document.body && !active.classList.contains("tree-canvas")) return;
      if (e.ctrlKey || e.altKey || e.metaKey || !graph || !focus) return;
      const at = selected && graph.people[selected] ? selected : focus;
      const p = graph.people[at];
      if (!p) return;
      const parentsOf = (id: string): string[] => {
        const loaded = parentsOrdered(graph, id).filter(Boolean) as string[];
        return [...loaded, ...(graph.people[id]?.parents ?? []).filter((x) => !loaded.includes(x))];
      };
      const move = (next: string | undefined) => {
        if (!next) return;
        if (scene?.cards.some((c) => c.id === next && !c.stub)) {
          setSelected(next);
          canvas.current?.reveal(next);
        } else refocus(next, true);
        setAnnounce(describe(graph, next));
      };
      switch (e.key) {
        case "ArrowUp": {
          const last = upFrom.current;
          const other = last && last.parent === at ? parentsOf(last.child).find((x) => x !== at) : undefined;
          if (other) {
            upFrom.current = null;
            move(other);
          } else {
            const first = parentsOf(at)[0];
            upFrom.current = first ? { child: at, parent: first } : null;
            move(first);
          }
          break;
        }
        case "ArrowDown": {
          const kids = [...p.children].sort((a, b) => (graph.people[a]?.birth?.sort ?? Number.MAX_SAFE_INTEGER) - (graph.people[b]?.birth?.sort ?? Number.MAX_SAFE_INTEGER));
          upFrom.current = null;
          move(kids[0]);
          break;
        }
        case "ArrowLeft":
        case "ArrowRight": {
          const y = scene?.cards.find((x) => x.id === at)?.y ?? 0;
          const row = scene?.cards.filter((c) => !c.stub && Math.abs(c.y - y) < 1).sort((a, b) => a.x - b.x) ?? [];
          const k = row.findIndex((c) => c.id === at);
          upFrom.current = null;
          move(row[k + (e.key === "ArrowLeft" ? -1 : 1)]?.id);
          break;
        }
        case "Enter":
          if (e.shiftKey) go({ name: "person", id: at });
          else {
            setSelected(at);
            setPanelOpen(true);
          }
          break;
        case "Home":
          // The suggested central person (the one with the most links).
          suggested()
            .then((s) => {
              if (!s) return;
              refocus(s.id, true);
              setAnnounce(graph.people[s.id] ? describe(graph, s.id) : s.name);
            })
            .catch(() => {});
          break;
        case "Escape":
          setPanelOpen(false);
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hidden, view, graph, focus, selected, scene, refocus, go]);

  // Całe drzewo has no keyboard walk; Esc still closes the panel there.
  useEffect(() => {
    if (hidden || view !== "overview") return;
    const onKey = (e: KeyboardEvent) => {
      const active = document.activeElement as HTMLElement | null;
      if (e.key !== "Escape" || (active && active !== document.body && !active.classList.contains("overview-host"))) return;
      setPanelOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hidden, view]);

  const setView = (v: View) => {
    setRestore(null);
    go({ name: "tree", view: v, person: focus ?? undefined });
  };

  const saveDisplay = (key: string, value: string) => {
    call<ArchiveStatus>("archive.setSettings", { display: { [key]: value } })
      .then((status) => useStore.getState().setArchive(status))
      .catch(() => {});
  };

  // Całe drzewo: „strona” is relative to the clicked person, else to the one the tree was opened at. The canvas
  // re-tints when this changes, without rebuilding.
  const sideCentre = view === "overview" && colorMode === "side" ? (selected ?? focus) : null;
  const overviewSides = useMemo(() => {
    if (!overviewData || !sideCentre) return null;
    const people = overviewData.people;
    const centre = people.findIndex((p) => p[0] === sideCentre);
    if (centre < 0) return null;
    const children: number[][] = people.map(() => []);
    people.forEach((p, i) => p[13].forEach((parent) => parent != null && children[parent].push(i)));
    return sideColors(centre, (i) => people[i][13], (i) => children[i]);
  }, [overviewData, sideCentre]);
  const overviewColor = useCallback(
    (index: number) => {
      const p = overviewData?.people[index];
      if (!p) return 0;
      switch (colorMode) {
        case "none":
          return 0;
        case "generation":
          return p[11] ? ((p[11] - 1) % 12) + 1 : 0;
        case "side":
          return overviewSides?.get(index) ?? 0;
        case "surname":
          return p[12];
        default:
          return p[3];
      }
    },
    [overviewData, colorMode, overviewSides],
  );

  const showPanel = panelOpen && selected;
  const firstLoad = !scene && (loading || !focus) && view !== "overview";

  return (
    <div className="tree-screen" style={{ display: hidden ? "none" : undefined }}>
      <div className="tree-main">
        {view === "overview" ? (
          overviewData ? (
            <OverviewCanvas
              ref={overview}
              data={overviewData}
              dark={dark}
              hidden={hidden}
              focus={focus}
              selected={selected}
              colorFor={overviewColor}
              onSelect={(id) => {
                setSelected(id);
                setPanelOpen(true);
              }}
              onOpen={(id) => {
                setRestore(null);
                go({ name: "tree", view: "family", person: id });
              }}
              onZoom={setZoom}
              restore={restore?.view === "overview" ? restore.cam : null}
              onRestored={() => setRestore(null)}
              onCamera={rememberPlace}
            />
          ) : (
            <Loading count={archive?.people ?? 0} busy={overviewLoading} />
          )
        ) : scene && graph && focus ? (
          <FocusCanvas
            key={view}
            anchor={view === "ancestors" ? "left" : view === "descendants" ? "top" : "center"}
            ref={canvas}
            scene={scene}
            graph={graph}
            focusId={focus}
            selected={selected}
            hidden={hidden}
            animate={animations}
            photos={photos}
            style={{ color: colorOf, dimmed, highlighted }}
            onSelect={(id) => {
              setSelected(id);
              if (id) setPanelOpen(true);
            }}
            onOpen={refocus}
            onHover={setHovered}
            onPill={refocus}
            onBox={(action, of) => {
              if (action === "siblings-open") setSiblingsOpen(true);
              else if (action === "siblings-close") setSiblingsOpen(false);
              else if (action === "add-parents" && of) requireEdit(() => go({ name: "edit", id: null, relation: { kind: "parent", of } }));
            }}
            onZoom={setZoom}
            restore={restore && restore.view === view && restore.person === focus ? restore.cam : null}
            onRestored={() => setRestore(null)}
            onCamera={rememberPlace}
          />
        ) : (
          firstLoad && <Loading count={archive?.people ?? 0} busy />
        )}
        <div className="tree-toolbar row1">
          <Segmented
            value={view}
            onChange={setView}
            style={{ padding: 3 }}
            options={[
              { value: "family", label: "Rodzina" },
              { value: "ancestors", label: "Przodkowie" },
              { value: "descendants", label: "Potomkowie" },
              { value: "overview", label: "Całe drzewo" },
            ]}
          />
          <JumpBox
            onPick={(id) => {
              if (view === "overview") {
                setSelected(id);
                overview.current?.centerOn(id);
              } else refocus(id);
            }}
          />
        </div>
        <div className="tree-toolbar row2">
          <Dropdown
            label="Koloruj wg:"
            icon={<Palette size={14} color="var(--text2)" />}
            value={colorMode}
            onChange={(v) => {
              setColorMode(v);
              saveDisplay("treeColor", v);
            }}
            width={360}
            options={[
              { value: "branch", label: "gałąź", note: "rodzina, w której ktoś się urodził" },
              { value: "surname", label: "nazwisko", note: "noszone teraz (żona w kolorze męża)" },
              { value: "side", label: "strona ojca–matki", note: view === "overview" ? "względem wybranej osoby" : view === "descendants" ? "tu tylko osoba w centrum" : "względem osoby w centrum" },
              { value: "generation", label: "pokolenie" },
              { value: "none", label: "brak" },
            ]}
          />
          {view === "family" && (
            <button className={`chip${lineOn ? " on" : ""}`} onClick={() => setLineOn((o) => !o)} title="Przygaś osoby spoza linii wybranej osoby">
              <GitCommitVertical size={14} />
              Linia bezpośrednia
            </button>
          )}
          {/* Całe drzewo has no photos on its cards yet, so there is nothing to switch there. */}
          {view !== "overview" && (
            <Dropdown
              label="Karty:"
              icon={<IdCard size={14} color="var(--text2)" />}
              value={photos ? "photo" : "plain"}
              onChange={(v) => {
                setPhotos(v === "photo");
                saveDisplay("cardStyle", v);
              }}
              width={180}
              options={[
                { value: "photo", label: "ze zdjęciem" },
                { value: "plain", label: "inicjały" },
              ]}
            />
          )}
          <span className="grow" />
          {view === "family" && siblingsOpen && (
            <button className="btn ghost sm" onClick={() => setSiblingsOpen(false)}>
              <RotateCcw size={14} />
              Przywróć automatyczny układ
            </button>
          )}
        </div>
        {view !== "overview" && scene && (
          <>
            <div className="zoom-stack">
              <button title="Przybliż" onClick={() => canvas.current?.zoomBy(1.25)}>
                <Plus size={16} />
              </button>
              <span className="zoom-readout">{Math.round(zoom * 100)}%</span>
              <button title="Oddal" onClick={() => canvas.current?.zoomBy(0.8)}>
                <Minus size={16} />
              </button>
              <button title="Dopasuj do ekranu" onClick={() => canvas.current?.fit()}>
                <Maximize size={15} />
              </button>
              <button title="Wyśrodkuj na wybranej osobie" style={{ color: "var(--accent-text)" }} onClick={() => canvas.current?.centerOn(selected ?? focus ?? "")}>
                <LocateFixed size={15} />
              </button>
            </div>
            <Legend />
          </>
        )}
        {colorMode === "side" && (view === "overview" ? overviewData : scene) && <SideKey view={view} />}
      </div>
      {showPanel && selected && (
        <SidePanel
          id={selected}
          onClose={() => setPanelOpen(false)}
          // Całe drzewo: a relative is found on the map (no new centre, which would lay out everyone again).
          onFocus={
            view === "overview"
              ? (id) => {
                  setSelected(id);
                  overview.current?.centerOn(id);
                }
              : refocus
          }
          relativeTitle={view === "overview" ? "Pokaż w drzewie" : undefined}
        />
      )}
      <div className="sr-only" aria-live="polite">
        {announce}
      </div>
    </div>
  );
}

/** The suggested central person (archive.firstOpen: the one with the most links first), other than `except`. */
async function suggested(except?: string | null): Promise<{ id: string; name: string } | undefined> {
  const d = await call<{ suggestions: { person: { id: string; name: string } }[] }>("archive.firstOpen");
  return d.suggestions.find((s) => s.person.id !== except)?.person;
}

/** What a screen reader hears after a keyboard step (design 17g): „Józef Kowalski, 1878–1951. Rodzice: Antoni
 *  Kowalski i Agnieszka Kowalska. Dzieci: 2.” */
function describe(graph: Graph, id: string): string {
  const p = graph.people[id];
  if (!p) return "";
  const years = [p.birth?.year ?? (p.death ? "?" : ""), p.death?.year ?? (p.living ? "żyje" : "")].filter(Boolean).join("–");
  const parents = p.parents.map((x) => graph.people[x]?.name).filter(Boolean);
  const parts = [`${p.name}${years ? `, ${years}` : ""}.`];
  if (parents.length) parts.push(`Rodzice: ${parents.join(" i ")}.`);
  if (p.children.length) parts.push(`Dzieci: ${p.children.length}.`);
  return parts.join(" ");
}

function Loading({ count, busy }: { count: number; busy: boolean }) {
  const positions = [
    [50, 150],
    [298, 150],
    [174, 380],
    [426, 380],
    [76, 610],
    [300, 610],
    [524, 610],
  ];
  return (
    <div className="tree-canvas" style={{ backgroundSize: "24px 24px", cursor: "default" }}>
      {positions.map(([x, y]) => (
        <div key={`${x}-${y}`} className="skeleton-card" style={{ left: x + 160, top: y }}>
          <span className="skel" style={{ width: 40, height: 40, borderRadius: "50%", flex: "none" }} />
          <span className="col grow" style={{ gap: 8 }}>
            <span className="skel" style={{ width: "80%", height: 10 }} />
            <span className="skel" style={{ width: "50%", height: 8 }} />
          </span>
        </div>
      ))}
      {busy && (
        <div className="tree-loading-card">
          <span style={{ color: "var(--accent-text)", display: "flex" }}>
            <Spinner size={20} />
          </span>
          <span className="col">
            <span style={{ fontSize: 15, fontWeight: 600 }}>Układam drzewo…</span>
            <span style={{ fontSize: 12, color: "var(--text3)" }}>{peopleCount(count)} · zwykle poniżej sekundy</span>
          </span>
        </div>
      )}
    </div>
  );
}

/** „Skocz do osoby…”: the search window (Ctrl K) narrowed to people; the chosen person goes to the middle (design 17e). */
function JumpBox({ onPick }: { onPick: (id: string) => void }) {
  const setPalette = useStore((s) => s.setPalette);
  return (
    <button className="search-box jump-box" style={{ height: 38, background: "var(--surface)" }} onClick={() => setPalette(true, { scope: "people", pick: onPick })}>
      <Crosshair size={15} />
      <span className="grow" style={{ textAlign: "left" }}>
        Skocz do osoby…
      </span>
    </button>
  );
}

/** The key for „Koloruj wg: strona ojca–matki”, above the line legend. */
function SideKey({ view }: { view: View }) {
  const swatch = (color: number, label: string) => (
    <span className="item">
      <span style={{ width: 10, height: 10, borderRadius: 2, background: `var(--b${color})`, flex: "none" }} />
      {label}
    </span>
  );
  return (
    <div className={`tree-legend side-key${view === "overview" ? " overview" : ""}`}>
      <span className="label-caps">Strona</span>
      {view === "descendants" ? (
        // Everyone in Potomkowie descends from the centre person, so only they have a side colour.
        <span className="item" style={{ color: "var(--text3)" }}>w Potomkach kolor ma tylko osoba w centrum</span>
      ) : (
        <>
          {swatch(SIDE.father, "ojca")}
          {swatch(SIDE.mother, "matki")}
          {swatch(SIDE.centre, view === "overview" ? "wybrana osoba i rodzeństwo" : "osoba w centrum i rodzeństwo")}
        </>
      )}
    </div>
  );
}

function Legend() {
  const sample = (style: React.CSSProperties, dot?: boolean) => (
    <svg width="24" height="10" style={{ flex: "none" }}>
      <line x1="0" y1="5" x2="24" y2="5" style={{ stroke: "var(--line)", strokeWidth: 1.5, ...style }} />
      {dot && <circle cx="12" cy="5" r="3.5" style={{ fill: "var(--surface)", stroke: "var(--line)", strokeWidth: 1.5 }} />}
    </svg>
  );
  return (
    <div className="tree-legend">
      <span className="label-caps">Linie</span>
      <span className="item">
        {sample({}, true)}małżeństwo
      </span>
      <span className="item">
        {sample({})}biologiczne
      </span>
      <span className="item">
        {sample({ strokeDasharray: "6 4" })}adopcja
      </span>
      <span className="item">
        {sample({ strokeDasharray: "1.5 3.5" })}niepewne
      </span>
      <span className="item">
        {sample({ stroke: "var(--accent)", strokeWidth: 2.5 })}linia wybranej
      </span>
    </div>
  );
}

