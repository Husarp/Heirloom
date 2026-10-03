// Ustawienia › Import, Osoby edytujące and O programie (spec §4.34; updates: APP-STANDARDS.md §2–3).

import { Copy, Plus } from "lucide-react";
import { useEffect, useState } from "react";
import { call } from "../../api/transport";
import type { AppState, ArchiveStatus } from "../../api/types";
import { useStore } from "../../app/store";
import { applyUpdate, checkNow, downloadText, refreshUpdateStatus, useUpdates } from "../../app/updates";
import { useApi } from "../../app/useApi";
import { SettingRow, Spinner, Toggle } from "../../components/bits";
import { count, dayMonth, shortWhen } from "../../lib/format";
import { copyText, openUrl } from "../../lib/native";
import { ShortcutsDialog, WhatsNewDialog } from "./dialogs";
import { failed, Keycap, saveSettings, Section } from "./parts";

export function ImportSection() {
  const go = useStore((s) => s.go);
  const notify = useStore((s) => s.notify);
  const { data: instructions } = useApi<{ text: string; version: string }>("import.instructions");
  const { data: history } = useApi<{ active: boolean; undone: boolean }[]>("import.history");
  // Imports already undone can't be undone again: „3 importy, 1 cofnięty”.
  const undone = history?.filter((h) => h.undone).length ?? 0;

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
                : history.every((h) => h.active)
                  ? `${count(history.length, "import", "importy", "importów")} · każdy można cofnąć`
                  : count(history.length, "import", "importy", "importów")
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
      <UpdateRows version={version} />
      <SettingRow
        label="Działa bez internetu"
        note="Nic z archiwum nie opuszcza komputera. Heirloom łączy się z GitHubem tylko wtedy, gdy włączysz sprawdzanie aktualizacji albo klikniesz „Sprawdź teraz”. Czcionki Newsreader i IBM Plex Sans (SIL OFL) oraz ikony Lucide są wbudowane"
      />
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

/** „Sprawdzaj aktualizacje”, and the current version with „GitHub”, „Sprawdź teraz” and „Pobierz aktualizację”. */
function UpdateRows({ version }: { version: string }) {
  const on = useStore((s) => s.app?.updates.check ?? false);
  const { status, checking, said, failed: updateFailed, installing } = useUpdates();
  useEffect(() => void refreshUpdateStatus(), []);

  const setOn = async (check: boolean) => {
    try {
      const app = await call<AppState>("app.setUpdates", { check });
      useStore.setState({ app });
    } catch (e) {
      failed(e);
    }
  };

  const running = status?.download.state === "running";
  const note = updateFailed
    ? `Nie udało się zaktualizować. ${updateFailed}`
    : installing
      ? "Uruchamiam instalator — Heirloom zaraz się zamknie."
      : (status && downloadText(status)) ??
        said ??
        (status?.newer ? `Jest nowa wersja ${status.latest} — masz ${version}.` : status?.checked ? `Masz najnowszą wersję, ${version}.` : `Masz wersję ${version}.`);

  return (
    <>
      <SettingRow label="Sprawdzaj aktualizacje" note="Domyślnie wyłączone. Gdy jest włączone, Heirloom pyta GitHuba przy starcie i po powrocie do okna, najwyżej co 5 minut — wysyła tylko pytanie o numer wersji">
        <Toggle on={on} onChange={setOn} />
      </SettingRow>
      <SettingRow label="Aktualizacje" note={note}>
        <span className="row" style={{ gap: 6 }}>
          <button className="btn ghost set-btn" onClick={() => void openUrl(status?.page ?? "https://github.com/Husarp/Heirloom/releases")}>
            GitHub
          </button>
          <button className="btn secondary set-btn" disabled={checking} onClick={() => void checkNow()}>
            {checking && <Spinner size={14} />}
            Sprawdź teraz
          </button>
          {status?.newer && (
            <button className="btn primary set-btn" disabled={running || installing} onClick={() => void applyUpdate()}>
              {updateFailed ? "Spróbuj ponownie" : "Pobierz aktualizację"}
            </button>
          )}
        </span>
      </SettingRow>
    </>
  );
}
