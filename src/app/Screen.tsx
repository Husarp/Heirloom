import { useLayoutEffect, useState } from "react";
import { useStore } from "./store";
import { Start } from "../screens/start/Start";
import { Activity } from "../screens/start/Activity";
import { People } from "../screens/people/People";
import { Profile } from "../screens/person/Profile";
import { NewPerson } from "../screens/person/NewPerson";
import { Tree } from "../screens/tree/Tree";
import { ImportWizard } from "../screens/import/ImportWizard";
import { Surnames } from "../screens/surnames/Surnames";
import { Places } from "../screens/places/Places";
import { Stories } from "../screens/stories/Stories";
import { Media } from "../screens/media/Media";
import { MissingFiles } from "../screens/media/MissingFiles";
import { Sources } from "../screens/sources/Sources";
import { Settings } from "../screens/settings/Settings";
import { Pairs } from "../screens/pairs/Pairs";

/** The screen for the current route. The tree is created on first visit and then kept (hidden and idle) so
 *  coming back is instant (PLAN §11.1). */
export function Screen() {
  const route = useStore((s) => s.route);
  const setCrumb = useStore((s) => s.setCrumb);
  const [treeVisited, setTreeVisited] = useState(false);

  // A layout effect, so the crumb is cleared before the new screen sets its own in a normal effect.
  useLayoutEffect(() => {
    if (route.name === "tree") setTreeVisited(true);
    setCrumb(null);
  }, [route, setCrumb]);

  const other = (() => {
    switch (route.name) {
      case "start":
        return <Start />;
      case "activity":
        return <Activity />;
      case "people":
        return <People />;
      case "person":
        return <Profile key={route.id} id={route.id} section={route.section} open={route.open} />;
      case "edit":
        return <NewPerson key={JSON.stringify(route.relation ?? null)} relation={route.relation} />;
      case "import":
        return <ImportWizard />;
      case "surnames":
        return <Surnames selected={route.key} />;
      case "places":
        return <Places selected={route.place} />;
      case "stories":
        return <Stories selected={route.id} initialQuery={route.q} />;
      case "media":
        return <Media selected={route.id} />;
      case "missingFiles":
        return <MissingFiles />;
      case "sources":
        return <Sources selected={route.id} />;
      case "settings":
        return <Settings section={route.section} />;
      case "pairs":
        return <Pairs />;
      default:
        return null;
    }
  })();

  return (
    <>
      {treeVisited && <Tree hidden={route.name !== "tree"} />}
      {route.name !== "tree" && other}
    </>
  );
}
