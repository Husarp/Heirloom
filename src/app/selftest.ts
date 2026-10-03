// `heirloom.exe --selftest <report>` (scripts/build.ps1, before packaging): the hidden window checks that the
// interface runs, the bundled fonts load without the internet, and the core can make an archive, add a person, save
// and read it back — in a throwaway folder with throwaway settings. The verdict goes to the report and the app quits.

import { invoke } from "@tauri-apps/api/core";
import { call, inTauri } from "../api/transport";
import type { AppState } from "../api/types";

export async function selfTestIfAsked() {
  if (!inTauri) return;
  const folder = await invoke<string | null>("selftest_folder").catch(() => null);
  if (!folder) return;
  const done: string[] = [];
  try {
    const app = await call<AppState>("app.state");
    done.push("interfejs");
    await document.fonts.ready;
    // load() lists the faces it found; check() alone says yes when no face matches at all.
    for (const font of ["16px 'Newsreader Variable'", "600 16px 'IBM Plex Sans'"]) {
      const faces = await document.fonts.load(font);
      if (faces.length === 0) throw new Error(`the bundled font is missing: ${font}`);
    }
    done.push("czcionki");
    await call("archive.create", { folder, name: "Test" });
    const { id } = await call<{ id: string }>("person.create", { given: "Józef", surname: "Kowalski", events: { birth: { date: "12.03.1878", place: "Wólka" } } });
    await call("archive.save", { author: "test" });
    const profile = await call<{ person: { name: string }; facts: { key: string; value: string }[] }>("person.get", { id });
    if (profile.person.name !== "Józef Kowalski") throw new Error(`read back „${profile.person.name}”`);
    if (!profile.facts.some((f) => f.value.startsWith("12 marca 1878"))) throw new Error("the birth date did not come back");
    await call("archive.close");
    done.push("archiwum");
    await invoke("selftest_done", { ok: true, text: `OK ${app.version} · ${done.join(" · ")}` });
  } catch (e) {
    await invoke("selftest_done", { ok: false, text: `FAILED after ${done.join(", ") || "start"}: ${e instanceof Error ? e.message : String(e)}` });
  }
}
