import { AppWindow, Archive, BookOpen, ChevronsUpDown, FolderOpen, GitMerge, House, Images, Import, Layers, Library, MapPin, Network, Settings, Signature, Users } from "lucide-react";
import { useState, type ComponentType } from "react";
import type { CombinedArchive } from "../api/types";
import { useStore, type Route, type TreeView } from "../app/store";
import { ArchiveDot, BrandMark, useDismiss } from "../components/bits";
import { displayPath, num, people as peopleCount, relativeTime } from "../lib/format";
import { OpenTogether } from "../screens/archive/OpenTogether";

interface NavItem {
  icon: ComponentType<{ size?: number }>;
  label: string;
  route: Route;
  /** Usable in an empty archive. */
  always?: boolean;
  /** Only for one archive (archives opened together are only browsed), or only for archives opened together. */
  only?: "archive" | "set";
}

const NAV: NavItem[] = [
  { icon: House, label: "Start", route: { name: "start" }, always: true },
  { icon: Network, label: "Drzewo", route: { name: "tree", view: "family" } },
  { icon: Users, label: "Osoby", route: { name: "people" } },
  { icon: Signature, label: "Nazwiska", route: { name: "surnames" } },
  { icon: MapPin, label: "Miejsca", route: { name: "places" } },
  { icon: BookOpen, label: "Historie", route: { name: "stories" } },
  { icon: Images, label: "Media", route: { name: "media" } },
  { icon: Library, label: "Źródła", route: { name: "sources" } },
  { icon: GitMerge, label: "Do sprawdzenia", route: { name: "pairs" }, always: true, only: "set" },
  { icon: Import, label: "Import", route: { name: "import" }, always: true, only: "archive" },
  { icon: Settings, label: "Ustawienia", route: { name: "settings" }, always: true },
];

/** Which nav item a screen belongs to (spec §2.8). */
export function navSection(route: Route): string {
  switch (route.name) {
    case "start":
    case "activity":
      return "start";
    case "person":
    case "edit":
      return "people";
    case "missingFiles":
      return "media";
    default:
      return route.name;
  }
}

export function Sidebar() {
  const archive = useStore((s) => s.archive);
  const route = useStore((s) => s.route);
  const go = useStore((s) => s.go);
  const lastSaved = useStore((s) => s.lastSaved);
  const empty = (archive?.people ?? 0) === 0;
  const active = navSection(route);
  const combined = archive?.combined;

  const status = !archive
    ? ""
    : empty
      ? `Brak osób · ${displayPath(archive.root)}`
      : `${peopleCount(archive.people)}${lastSaved ? ` · zapisano ${relativeTime(lastSaved)}` : ""}`;

  return (
    <aside className="sidebar">
      <Switcher />
      {NAV.filter((item) => !item.only || item.only === (combined ? "set" : "archive")).map((item) => {
        const key = navSection(item.route);
        const disabled = empty && !item.always && key !== active;
        return (
          <button
            key={item.label}
            className={`nav-item${key === active ? " active" : ""}`}
            disabled={disabled}
            onClick={() => go(item.route.name === "tree" ? { name: "tree", view: (archive?.display.treeDefaultView as TreeView) ?? "family" } : item.route)}
          >
            <item.icon size={17} />
            <span className="grow">{item.label}</span>
            {item.route.name === "pairs" && combined?.pending ? <span className="nav-count">{num(combined.pending)}</span> : null}
          </button>
        );
      })}
      <div className="sidebar-foot" title={archive ? displayPath(archive.dataPath) : undefined}>
        <Archive size={15} style={{ flex: "none" }} />
        <span className="ellipsis">{status}</span>
      </div>
    </aside>
  );
}

/** The archive's name at the top, with a menu: another archive, „Otwórz razem…”; for archives opened together each
 *  archive in its colour, to open by itself here or in a new window (design §6.2). */
function Switcher() {
  const archive = useStore((s) => s.archive);
  const closeArchive = useStore((s) => s.closeArchive);
  const whenSaved = useStore((s) => s.whenSaved);
  const openArchive = useStore((s) => s.openArchive);
  const openElsewhere = useStore((s) => s.openElsewhere);
  const [open, setOpen] = useState(false);
  const [together, setTogether] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const combined = archive?.combined;
  const run = (then: () => void) => {
    setOpen(false);
    then();
  };
  const alone = (a: CombinedArchive) => a.path && run(() => openArchive(a.path!));
  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button className="sidebar-head" title="Zmień archiwum" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <BrandMark size={36} />
        <div className="grow" style={{ minWidth: 0 }}>
          <div className="wordmark">Heirloom</div>
          <div className="archive-name ellipsis">{archive?.name || "Nowe archiwum"}</div>
          {combined && (
            <div className="row" style={{ gap: 4, marginTop: 4 }}>
              {combined.archives.map((a) => (
                <ArchiveDot key={a.key} from={[a.key]} size={10} style={{ boxShadow: "none", opacity: a.state === "ok" ? 1 : 0.35 }} />
              ))}
            </div>
          )}
        </div>
        <ChevronsUpDown size={16} className="switcher" />
      </button>
      {open && (
        <div className="popover switcher-menu" style={{ top: 58, left: 0, width: 300, padding: "4px 0" }}>
          {combined && (
            <>
              <div className="menu-label" style={{ padding: "8px 14px 4px" }}>
                W tym zestawie
              </div>
              {combined.archives.map((a) => (
                <div key={a.key} className="row" style={{ paddingRight: 6 }}>
                  <button className="menu-item grow" style={{ minWidth: 0 }} disabled={!a.path || a.state !== "ok"} onClick={() => alone(a)} title="Otwórz tylko to archiwum">
                    <ArchiveDot from={[a.key]} size={12} style={{ boxShadow: "none" }} />
                    <span className="col grow" style={{ minWidth: 0, textAlign: "left" }}>
                      <span className="ellipsis">{a.name}</span>
                      <span style={{ fontSize: 12, color: "var(--text3)" }}>{a.state === "ok" ? peopleCount(a.people) : "nie znaleziono"}</span>
                    </span>
                  </button>
                  <button className="icon-btn" style={{ width: 30, height: 30 }} disabled={!a.path || a.state !== "ok"} title="Otwórz w nowym oknie" aria-label={`${a.name}: otwórz w nowym oknie`} onClick={() => run(() => openElsewhere(a.path!))}>
                    <AppWindow size={15} />
                  </button>
                </div>
              ))}
              <div style={{ borderTop: "1px solid var(--border)", margin: "4px 0" }} />
            </>
          )}
          <button className="menu-item" onClick={() => run(() => whenSaved(closeArchive, "Zmieniasz archiwum"))}>
            <FolderOpen size={15} />
            Zmień archiwum…
          </button>
          <button className="menu-item" onClick={() => run(() => whenSaved(() => setTogether(true), "Otwierasz archiwa razem"))}>
            <Layers size={15} />
            {combined ? "Otwórz razem inne archiwa…" : "Otwórz razem z innym archiwum…"}
          </button>
        </div>
      )}
      {together && <OpenTogether with={combined ? undefined : openedPath(archive)} onClose={() => setTogether(false)} />}
    </div>
  );
}

/** How the open archive was opened (its folder, or its .ged file), as in the recent list. */
function openedPath(archive: { root: string; dataPath: string } | null): string | undefined {
  if (!archive) return undefined;
  const recent = useStore.getState().app?.recent ?? [];
  return recent.find((r) => r.path === archive.dataPath || r.path === archive.root)?.path ?? archive.root;
}
