// Ustawienia › Import, Osoby edytujące and O programie (spec §4.34).

import { Copy, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "../../api/transport";
import type { ArchiveStatus } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { SettingRow } from "../../components/bits";
import { count, dayMonth, shortWhen } from "../../lib/format";
import { copyText } from "../../lib/native";
import { ShortcutsDialog, WhatsNewDialog } from "./dialogs";
import { failed, Keycap, saveSettings, Section } from "./parts";

export function ImportSection() {
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const { data: instructions } = useApi<{ text: string; version: string }>("import.instructions");
  const { data: history } = useApi<{ active: boolean }[]>("import.history");
  // Imports already undone can't be undone again: „3 importy, 1 cofnięty”.
  const undone = history?.filter((h) => !h.active).length ?? 0;

  const copy = async () => {
    if (!instructions) return;
    if (await copyText(instructions.text)) notify("Skopiowano instrukcję dla AI — wklej ją osobie, która przygotowuje paczkę.");
    else notify("Nie udało się skopiować instrukcji.", { kind: "err" });
  };

  return (
    <Section id="import" title="Import">
      <SettingRow label="Instrukcja dla AI" note="Dla osoby przygotowującej paczkę">
        <button className="btn secondary set-btn" disabled={!instructions} onClick={copy}>
          <Copy size={15} />
          Kopiuj instrukcję dla AI
        </button>
      </SettingRow>
      <SettingRow label="Wersja formatu">
        <Keycap>{instructions?.version ?? "…"}</Keycap>
      </SettingRow>
      <SettingRow
        label="Historia importów"
        note={
          !history
            ? "…"
            : !history.length
              ? "Nie było jeszcze importów"
              : undone
                ? `${count(history.length, "import", "importy", "importów")}, ${count(undone, "cofnięty", "cofnięte", "cofniętych")}`
                : `${count(history.length, "import", "importy", "importów")} · każdy można cofnąć`
        }
        last
      >
        <button className="btn secondary set-btn" disabled={!history?.length} onClick={() => go({ name: "import" })}>
          Pokaż
        </button>
      </SettingRow>
    </Section>
  );
}

interface EditorStats {
  name: string;
  lastEdited: string | null;
  changes: number;
}

/** The names „Kto edytuje?” offers. Renaming or removing one leaves the change history as it is. */
export function EditorsSection({ archive }: { archive: ArchiveStatus }) {
  const mode = useStore((s) => s.mode);
  const editor = useStore((s) => s.editor);
  const startEditing = useStore((s) => s.startEditing);
  const setAsk = useStore((s) => s.setAsk);
  const notify = useStore((s) => s.notify);
  const [stats, setStats] = useState<EditorStats[] | null>(null);
  /** The name being renamed, or "" while a new one is typed. */
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const names = archive.editors.map((e) => e.name);
  const listKey = JSON.stringify(archive.editors);

  useEffect(() => {
    call<EditorStats[]>("archive.editors").then(setStats).catch(failed);
  }, [listKey]);

  const start = (name: string) => {
    setEditing(name);
    setDraft(name);
  };

  const commit = async () => {
    const name = draft.trim();
    if (editing === null || !name || name === editing) {
      setEditing(null);
      return;
    }
    if (names.includes(name)) {
      notify("To imię już jest na liście.", { kind: "err" });
      return;
    }
    const next = editing === "" ? [...names, name] : names.map((n) => (n === editing ? name : n));
    if (!(await saveSettings({ editors: next }))) return;
    // Whoever is editing now goes on under the new name.
    if (editing && mode === "edit" && editor === editing) startEditing(name);
    setEditing(null);
  };

  const remove = (name: string) =>
    setAsk({
      title: `Usunąć „${name}” z listy?`,
      text: "Imię zniknie z okna „Kto edytuje?”. Historia zmian zostaje, jak była.",
      icon: "warn",
      buttons: [
        { label: "Anuluj", kind: "ghost" },
        {
          label: "Usuń",
          kind: "danger",
          run: async () => {
            await saveSettings({ editors: names.filter((n) => n !== name) });
          },
        },
      ],
    });

  const input = (placeholder: string, last?: boolean) => (
    <div className="row set-row" style={last ? { borderBottom: "none" } : undefined}>
      <input
        className="input sm"
        autoFocus
        value={draft}
        placeholder={placeholder}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") setEditing(null);
        }}
        style={{ maxWidth: 280 }}
      />
      <span className="grow" />
      <button className="btn ghost set-btn" onClick={() => setEditing(null)}>
        Anuluj
      </button>
      <button className="btn primary set-btn" disabled={!draft.trim()} onClick={commit}>
        {editing === "" ? "Dodaj" : "Zapisz"}
      </button>
    </div>
  );

  return (
    <Section id="editors" title="Osoby edytujące">
      {archive.editors.map((e) => {
        if (editing === e.name) return <div key={e.name}>{input("Imię")}</div>;
        const mine = stats?.find((s) => s.name === e.name);
        const last = mine?.lastEdited ?? e.lastEdited;
        const note = !stats
          ? "…"
          : mine?.changes || last
            ? `ostatnio: ${shortWhen(last)} · ${count(mine?.changes ?? 0, "zmiana", "zmiany", "zmian")}`
            : "jeszcze nie edytował(a)";
        return (
          <SettingRow key={e.name} label={e.name} note={note}>
            <span className="row" style={{ gap: 6 }}>
              <button className="btn ghost set-btn" onClick={() => remove(e.name)}>
                usuń
              </button>
              <button className="btn secondary set-btn" onClick={() => start(e.name)}>
                Zmień
              </button>
            </span>
          </SettingRow>
        );
      })}
      {editing === "" ? (
        input("Imię, np. Ewa albo Ciocia Helena", true)
      ) : (
        <SettingRow label="Dodaj imię" note="Pojawi się w oknie „Kto edytuje?”" last>
          <button className="btn secondary set-btn" onClick={() => start("")}>
            <Plus size={15} />
            Dodaj
          </button>
        </SettingRow>
      )}
    </Section>
  );
}

export function AboutSection() {
  const version = useStore((s) => s.app?.version ?? "");
  const { data: about } = useApi<{ version: string; buildDate: string }>("app.about");
  const [open, setOpen] = useState<"whatsNew" | "shortcuts" | null>(null);
  const built = about ? new Date(`${about.buildDate}T12:00:00`) : null;
  return (
    <Section id="about" title="O programie">
      <SettingRow label={`Heirloom ${version}`} note={built ? `${dayMonth(built)} ${built.getFullYear()}` : "…"}>
        <button className="btn secondary set-btn" onClick={() => setOpen("whatsNew")}>
          Co nowego
        </button>
      </SettingRow>
      <SettingRow label="Działa w pełni offline" note="Czcionki Newsreader i IBM Plex Sans (SIL OFL) oraz ikony Lucide są wbudowane" />
      <SettingRow label="Licencja" note="MIT" />
      <SettingRow label="Skróty klawiszowe" note="Ctrl K szukaj · Ctrl E edycja · Ctrl S zapisz · Ctrl Z cofnij" last>
        <button className="btn secondary set-btn" onClick={() => setOpen("shortcuts")}>
          Pokaż ściągawkę
        </button>
      </SettingRow>
      {open === "whatsNew" && <WhatsNewDialog version={version} onClose={() => setOpen(null)} />}
      {open === "shortcuts" && <ShortcutsDialog onClose={() => setOpen(null)} />}
    </Section>
  );
}
