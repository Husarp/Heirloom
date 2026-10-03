import { ALargeSmall, ArrowLeft, ArrowRight, BookOpen, ChevronRight, Lock, Pencil, Search, SunMoon } from "lucide-react";
import { useState } from "react";
import { call } from "../api/transport";
import type { AppState } from "../api/types";
import { useStore, type Route } from "../app/store";
import { TextSizePopover } from "./TextSizePopover";

const SECTION_LABEL: Record<string, string> = {
  start: "Start",
  tree: "Drzewo",
  people: "Osoby",
  person: "Osoby",
  edit: "Osoby",
  surnames: "Nazwiska",
  places: "Miejsca",
  stories: "Historie",
  media: "Media",
  missingFiles: "Media",
  sources: "Źródła",
  import: "Import",
  settings: "Ustawienia",
  activity: "Historia zmian",
};

function crumbsFor(route: Route, crumb: string | null, empty: boolean): { parent: string; parentRoute?: Route; current?: string } {
  if (empty && route.name === "start") return { parent: "Nowe archiwum" };
  switch (route.name) {
    case "tree":
      return { parent: "Drzewo", current: route.view === "overview" ? "Całe drzewo" : crumb ?? undefined };
    case "people":
      return { parent: "Osoby", current: "Wszystkie osoby" };
    case "person":
      return { parent: "Osoby", parentRoute: { name: "people" }, current: crumb ?? undefined };
    case "edit":
      return { parent: "Osoby", parentRoute: { name: "people" }, current: crumb ?? (route.id ? undefined : "Nowa osoba") };
    case "missingFiles":
      return { parent: "Media", parentRoute: { name: "media" }, current: "Brakujące pliki" };
    case "import":
      return { parent: "Import", current: crumb ?? undefined };
    default:
      return { parent: SECTION_LABEL[route.name] ?? "", current: crumb ?? undefined };
  }
}

export function TopBar() {
  const route = useStore((s) => s.route);
  const crumb = useStore((s) => s.crumb);
  const back = useStore((s) => s.back);
  const forward = useStore((s) => s.forward);
  const goBack = useStore((s) => s.goBack);
  const goForward = useStore((s) => s.goForward);
  const go = useStore((s) => s.go);
  const archive = useStore((s) => s.archive);
  const app = useStore((s) => s.app);
  const mode = useStore((s) => s.mode);
  const requireEdit = useStore((s) => s.requireEdit);
  const stopEditing = useStore((s) => s.stopEditing);
  const setPalette = useStore((s) => s.setPalette);
  const refreshApp = useStore((s) => s.refreshApp);
  const [sizeOpen, setSizeOpen] = useState(false);
  const [lockHint, setLockHint] = useState(false);

  const c = crumbsFor(route, crumb, (archive?.people ?? 0) === 0);
  const readOnly = archive?.readOnly ?? false;

  const cycleTheme = async () => {
    const order = ["system", "light", "dark"] as const;
    const current = app?.appearance.theme ?? "system";
    const next = order[(order.indexOf(current) + 1) % order.length];
    await call<AppState>("app.setAppearance", { theme: next });
    await refreshApp();
    useStore.getState().notify(`Motyw: ${next === "system" ? "systemowy" : next === "light" ? "jasny" : "ciemny"}`);
  };

  return (
    <header className="topbar">
      <div className="row" style={{ gap: 2 }}>
        <button className="icon-btn" title="Wstecz (Alt ←)" aria-label="Wstecz" disabled={back.length === 0} onClick={goBack}>
          <ArrowLeft size={17} />
        </button>
        <button className="icon-btn" title="Dalej (Alt →)" aria-label="Dalej" disabled={forward.length === 0} onClick={goForward}>
          <ArrowRight size={17} />
        </button>
      </div>
      <nav className="crumbs">
        {c.current ? (
          <>
            {c.parentRoute ? <button onClick={() => go(c.parentRoute!)}>{c.parent}</button> : <span>{c.parent}</span>}
            <ChevronRight size={13} color="var(--text3)" style={{ flex: "none" }} />
            <span className="current">{c.current}</span>
          </>
        ) : (
          <span className="current">{c.parent}</span>
        )}
      </nav>
      <div className="topbar-search">
        <button onClick={() => setPalette(true)}>
          <Search size={15} />
          <span className="grow">Szukaj osób, miejsc i poleceń…</span>
          <span className="kbd">Ctrl K</span>
        </button>
      </div>
      <div className="topbar-right">
        <div style={{ position: "relative" }}>
          <button className="icon-btn" title="Rozmiar tekstu" aria-label="Rozmiar tekstu" onClick={() => setSizeOpen((o) => !o)}>
            <ALargeSmall size={17} />
          </button>
          {sizeOpen && <TextSizePopover onClose={() => setSizeOpen(false)} />}
        </div>
        <button className="icon-btn" title="Motyw: jasny / ciemny / systemowy" aria-label="Motyw" onClick={cycleTheme}>
          <SunMoon size={17} />
        </button>
        <div
          className={`seg accent mode-toggle${readOnly ? " locked" : ""}`}
          style={{ position: "relative" }}
          onMouseEnter={() => readOnly && setLockHint(true)}
          onMouseLeave={() => setLockHint(false)}
        >
          <button
            className={mode === "browse" ? "on" : ""}
            onClick={() => mode === "edit" && useStore.getState().whenSaved(stopEditing, "Kończysz edycję")}
          >
            <BookOpen size={14} />
            Przeglądanie
          </button>
          <button className={mode === "edit" ? "on" : ""} disabled={readOnly} onClick={() => requireEdit()} title="Edycja (Ctrl E)">
            {readOnly ? <Lock size={14} /> : <Pencil size={14} />}
            Edycja
          </button>
          {lockHint && (
            <div className="tooltip" style={{ right: 0, top: 38, width: 250 }}>
              Edycja wyłączona, bo w tym archiwum włączono tryb tylko do odczytu. Wyłączysz go w pasku poniżej albo w Ustawienia › Archiwum.
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
