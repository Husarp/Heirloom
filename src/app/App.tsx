import { useEffect } from "react";
import { useStore } from "./store";
import { Sidebar } from "../shell/Sidebar";
import { TopBar } from "../shell/TopBar";
import { AskDialog, ConflictDialog, ForeignConfirmDialog, WhoEditsDialog } from "../shell/Dialogs";
import { CombinedBar, EditModeBar, ReadOnlyBar } from "../shell/Bars";
import { Toasts } from "../shell/Toasts";
import { CommandPalette } from "../shell/CommandPalette";
import { useAppearance } from "./useAppearance";
import { useShortcuts } from "./useShortcuts";
import { useCloseGuard } from "./useCloseGuard";
import { useUpdateChecks } from "./updates";
import { useSetRefresh } from "./useSetRefresh";
import { Screen } from "./Screen";
import { ArchivePicker } from "../screens/archive/ArchivePicker";
import { LoadingArchive } from "../screens/archive/LoadingArchive";
import { FirstOpen } from "../screens/archive/FirstOpen";

export function App() {
  const phase = useStore((s) => s.phase);
  const boot = useStore((s) => s.boot);
  const mode = useStore((s) => s.mode);
  const readOnly = useStore((s) => s.archive?.readOnly ?? false);
  const combined = useStore((s) => s.archive?.combined != null);
  const whoEditsOpen = useStore((s) => s.whoEditsOpen);
  const conflictOpen = useStore((s) => s.conflictOpen);
  const foreignConfirmOpen = useStore((s) => s.foreignConfirmOpen);
  const paletteOpen = useStore((s) => s.paletteOpen);
  useAppearance();
  useShortcuts();
  useCloseGuard();
  useUpdateChecks();
  useSetRefresh();

  useEffect(() => {
    boot().catch(() => useStore.setState({ phase: "picker" }));
  }, [boot]);

  if (phase === "boot") return <div className="app" />;

  return (
    <>
      {phase === "ready" && (
        <div className="app">
          <Sidebar />
          <div className="main">
            <TopBar />
            {mode === "edit" && <EditModeBar />}
            {combined ? <CombinedBar /> : readOnly && <ReadOnlyBar />}
            <div className="content">
              <Screen />
            </div>
          </div>
        </div>
      )}
      {phase === "picker" && <ArchivePicker />}
      {phase === "loading" && <LoadingArchive />}
      {phase === "firstOpen" && <FirstOpen />}
      {whoEditsOpen && <WhoEditsDialog />}
      {conflictOpen && <ConflictDialog />}
      {foreignConfirmOpen && <ForeignConfirmDialog />}
      {paletteOpen && phase === "ready" && <CommandPalette />}
      <AskDialog />
      <Toasts />
    </>
  );
}
