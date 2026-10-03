import { Archive, BookOpen, ChevronsUpDown, House, Images, Import, Library, MapPin, Network, Settings, Signature, Users } from "lucide-react";
import type { ComponentType } from "react";
import { useStore, type Route, type TreeView } from "../app/store";
import { displayPath, people as peopleCount, relativeTime } from "../lib/format";

interface NavItem {
  icon: ComponentType<{ size?: number }>;
  label: string;
  route: Route;
  /** Usable in an empty archive. */
  always?: boolean;
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
  { icon: Import, label: "Import", route: { name: "import" }, always: true },
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
  const closeArchive = useStore((s) => s.closeArchive);
  const whenSaved = useStore((s) => s.whenSaved);
  const empty = (archive?.people ?? 0) === 0;
  const active = navSection(route);

  const status = !archive
    ? ""
    : empty
      ? `Brak osób · ${displayPath(archive.root)}`
      : `${peopleCount(archive.people)}${lastSaved ? ` · zapisano ${relativeTime(lastSaved)}` : ""}`;

  return (
    <aside className="sidebar">
      <button className="sidebar-head" title="Zmień archiwum" onClick={() => whenSaved(closeArchive, "Zmieniasz archiwum")}>
        <div className="logo-tile">H</div>
        <div className="grow">
          <div className="wordmark">Heirloom</div>
          <div className="archive-name ellipsis">{archive?.name || "Nowe archiwum"}</div>
        </div>
        <ChevronsUpDown size={16} className="switcher" />
      </button>
      {NAV.map((item) => {
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
            {item.label}
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
