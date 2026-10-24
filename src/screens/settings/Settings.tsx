import { Archive, Calendar, Check, ChevronDown, Import, Info, Network, Palette, Users } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { call } from "../../api/transport";
import type { AppState, Appearance, ArchiveStatus } from "../../api/types";
import { ACCENTS, knownAccent, knownScheme, SCHEMES } from "../../app/colorLooks";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { BRANCH_NAMES, BranchColorPicker, useBranchColors } from "../../components/BranchColorPicker";
import { BranchShape, Segmented, SettingRow, Toggle } from "../../components/bits";
import { num, people } from "../../lib/format";
import { collator } from "../surnames/split";
import { zoomLookOf } from "../tree/zoomLook";
import { ArchiveSections } from "./archive";
import { AboutSection, EditorsSection, ImportSection } from "./more";
import { failed, Section, Select, setDisplay } from "./parts";
import "./settings.css";

const NAV = [
  { id: "appearance", label: "Wygląd", icon: Palette },
  { id: "tree", label: "Drzewo", icon: Network },
  { id: "names", label: "Osoby i daty", icon: Calendar },
  { id: "archive", label: "Archiwum", icon: Archive },
  { id: "import", label: "Import", icon: Import },
  { id: "editors", label: "Osoby edytujące", icon: Users },
  { id: "about", label: "O programie", icon: Info },
];

/** Sections without their own nav item belong to Archiwum. */
const NAV_OF: Record<string, string> = { overlay: "archive", categories: "archive", settingsFiles: "archive", backup: "archive", export: "archive", tools: "archive" };

/** Ustawienia (spec §4.34): a sticky nav that follows the scroll, and sections of rows. `section` (from the route)
 *  is scrolled to on open: a nav id or `settingsFiles`, `backup`, `export`, `tools`. */
export function Settings({ section }: { section?: string }) {
  const archive = useStore((s) => s.archive);
  const ready = archive != null;
  const animations = useStore((s) => s.app?.appearance.animations ?? true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState("appearance");
  /** Until then the nav keeps the item that was clicked (a smooth scroll passes other sections). */
  const holdUntil = useRef(0);

  const show = (id: string, smooth: boolean) => {
    const el = document.getElementById(`set-${id}`);
    if (!el) return;
    holdUntil.current = Date.now() + 900;
    setActive(NAV_OF[id] ?? id);
    el.scrollIntoView({ behavior: smooth && animations ? "smooth" : "auto", block: "start" });
  };

  useEffect(() => {
    if (section) requestAnimationFrame(() => show(section, false));
    // Only when the route asks for another section.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section]);

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    const onScroll = () => {
      if (Date.now() < holdUntil.current) return;
      const top = scroller.getBoundingClientRect().top + 80;
      let current = "appearance";
      for (const el of scroller.querySelectorAll<HTMLElement>(".set-section")) {
        if (el.getBoundingClientRect().top <= top) current = el.id.slice(4);
      }
      // At the very end the last section is the one being read, even if it can't reach the top.
      if (scroller.scrollTop > 0 && scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 4) current = "about";
      setActive(NAV_OF[current] ?? current);
    };
    onScroll();
    scroller.addEventListener("scroll", onScroll, { passive: true });
    return () => scroller.removeEventListener("scroll", onScroll);
  }, [ready]);

  if (!archive) return <div className="page" />;
  // Archives opened together are only read: only this computer's settings, not the archives' (design §6.4).
  const combined = archive.combined != null;
  const nav = combined ? NAV.filter((item) => item.id === "appearance" || item.id === "about") : NAV;

  return (
    <div className="page" ref={scrollRef}>
      <div className="settings">
        <nav className="set-nav">
          <h1>Ustawienia</h1>
          {nav.map((item) => (
            <button key={item.id} className={`set-nav-item${active === item.id ? " active" : ""}`} onClick={() => show(item.id, true)}>
              <item.icon size={16} />
              {item.label}
            </button>
          ))}
        </nav>
        <div className="set-sections">
          <AppearanceSection />
          {combined ? (
            <div className="banner info">Ustawienia drzewa, osób i dat oraz archiwum zmienisz w każdym archiwum osobno — archiwa otwarte razem są tylko do przeglądania.</div>
          ) : (
            <>
              <TreeSection archive={archive} />
              <NamesSection display={archive.display} />
              <ArchiveSections archive={archive} />
              <ImportSection />
              <EditorsSection archive={archive} />
            </>
          )}
          <AboutSection />
        </div>
      </div>
    </div>
  );
}

function AppearanceSection() {
  const appearance = useStore((s) => s.app?.appearance);
  if (!appearance) return null;
  const scheme = knownScheme(appearance.scheme);
  const accent = knownAccent(appearance.accent);
  const set = async (patch: Partial<Appearance>) => {
    try {
      const app = await call<AppState>("app.setAppearance", patch);
      useStore.setState({ app });
    } catch (e) {
      failed(e);
    }
  };
  return (
    <Section id="appearance" title="Wygląd">
      <SettingRow label="Motyw">
        <Segmented
          size={28}
          value={appearance.theme}
          onChange={(theme) => set({ theme })}
          options={[
            { value: "light", label: "Jasny" },
            { value: "dark", label: "Ciemny" },
            { value: "system", label: "Systemowy" },
          ]}
        />
      </SettingRow>
      <SettingRow wide label="Kolorystyka" note="Tło, panele, boczny pasek i płótno drzewa; każda ma jasną i ciemną wersję">
        <div className="pal-options" role="radiogroup" aria-label="Kolorystyka">
          {SCHEMES.map((s) => (
            <button key={s.value} className={`pal-option${scheme === s.value ? " on" : ""}`} role="radio" aria-checked={scheme === s.value} title={s.note} onClick={() => set({ scheme: s.value })}>
              <span className="pal-check">{scheme === s.value && <Check size={14} />}</span>
              <span className="pal-name">{s.label}</span>
              <span className={`sch-prev sch-${s.value}`} aria-hidden>
                <span className="sch-side" />
                <span className="sch-main">
                  <span className="sch-card" />
                  <span className="sch-card" />
                  <span className="sch-dot" />
                </span>
              </span>
            </button>
          ))}
        </div>
      </SettingRow>
      <SettingRow wide label="Kolor akcentu" note="Przyciski, odnośniki, zaznaczenia i wskaźnik klawiatury">
        <div className="pal-options acc-options" role="radiogroup" aria-label="Kolor akcentu">
          {ACCENTS.map((a) => (
            <button key={a.value} className={`pal-option acc-option${accent === a.value ? " on" : ""}`} role="radio" aria-checked={accent === a.value} onClick={() => set({ accent: a.value })}>
              {/* „Domyślny” shows the accent the scheme has on its own; the others, their own. */}
              <span className={`acc-sw ${a.value === "default" ? `sch-${scheme}` : `acc-${a.value}`}`}>{accent === a.value && <Check size={12} strokeWidth={3} />}</span>
              <span>{a.label}</span>
            </button>
          ))}
        </div>
      </SettingRow>
      <SettingRow label="Wielkość tekstu" note="100–150%, powiększa cały program">
        <TextSize value={appearance.textSize} onChange={(textSize) => set({ textSize })} />
      </SettingRow>
      <SettingRow label="Gęstość list i tabel">
        <Segmented
          size={28}
          value={appearance.density}
          onChange={(density) => set({ density })}
          options={[
            { value: "comfortable", label: "Wygodna" },
            { value: "compact", label: "Zwarta" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Animacje" last>
        <Toggle on={appearance.animations} onChange={(animations) => set({ animations })} />
      </SettingRow>
    </Section>
  );
}

/** „A” 12 px, the slider, „A” 18 px and the value. The size is applied when the slider is let go: the whole window
 *  zooms, and zooming while dragging would move the slider under the pointer. */
function TextSize({ value, onChange }: { value: number; onChange: (value: number) => void }) {
  const [draft, setDraft] = useState(value);
  const ref = useRef<HTMLInputElement>(null);
  const latest = useRef(onChange);
  latest.current = onChange;
  useEffect(() => setDraft(value), [value]);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    // The native `change` event fires on release (and on each arrow key), unlike React's onChange.
    const commit = () => latest.current(Number(el.value));
    el.addEventListener("change", commit);
    return () => el.removeEventListener("change", commit);
  }, []);
  return (
    <div className="text-size">
      <span style={{ fontSize: 12 }}>A</span>
      <input
        ref={ref}
        type="range"
        min={100}
        max={150}
        step={5}
        value={draft}
        aria-label="Wielkość tekstu"
        style={{ "--fill": `${((draft - 100) / 50) * 100}%` } as CSSProperties}
        onChange={(e) => setDraft(Number(e.target.value))}
      />
      <span style={{ fontSize: 18 }}>A</span>
      <span className="text-size-value">{draft}%</span>
    </div>
  );
}

function TreeSection({ archive }: { archive: ArchiveStatus }) {
  const display = archive.display;
  return (
    <Section id="tree" title="Drzewo">
      <SettingRow label="Widok domyślny">
        <Select
          label="Widok domyślny"
          value={(display.treeDefaultView as string) ?? "family"}
          onChange={(v) => setDisplay("treeDefaultView", v)}
          options={[
            { value: "family", label: "Rodzina" },
            { value: "ancestors", label: "Przodkowie" },
            { value: "descendants", label: "Potomkowie" },
            { value: "overview", label: "Całe drzewo" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Styl karty">
        <Segmented
          size={28}
          value={(display.cardStyle as string) === "plain" ? "plain" : "photo"}
          onChange={(v) => setDisplay("cardStyle", v)}
          options={[
            { value: "photo", label: "Ze zdjęciem" },
            { value: "plain", label: "Inicjały" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Kolorowanie" note="gałąź, nazwisko, strona ojca–matki, pokolenie lub brak">
        <Select
          label="Kolorowanie"
          value={(display.treeColor as string) ?? "branch"}
          onChange={(v) => setDisplay("treeColor", v)}
          options={[
            { value: "branch", label: "Gałąź" },
            { value: "surname", label: "Nazwisko" },
            { value: "side", label: "Strona ojca–matki" },
            { value: "generation", label: "Pokolenie" },
            { value: "none", label: "Brak" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Linie">
        <Segmented
          size={28}
          value={(display.treeLines as string) ?? "straight"}
          onChange={(v) => setDisplay("treeLines", v)}
          options={[
            { value: "straight", label: "Proste" },
            { value: "rounded", label: "Zaokrąglone" },
          ]}
        />
      </SettingRow>
      <SettingRow wide label="Wygląd przy oddaleniu" note="Jak wyglądają osoby, gdy drzewo jest bardzo pomniejszone">
        <Select
          label="Wygląd przy oddaleniu"
          value={zoomLookOf(display.treeZoomLook)}
          onChange={(v) => setDisplay("treeZoomLook", v)}
          options={[
            { value: "shapes", label: "Kształty" },
            { value: "outline", label: "Obwódka" },
            { value: "tiles", label: "Kafelki" },
            { value: "cards", label: "Mini-karty" },
          ]}
        />
      </SettingRow>
      <SettingRow wide label="Rozwijanie potomków (widok Rodzina)" note="Co robi „+N” pod dzieckiem, gdy jego linia ma więcej niż 2 pokolenia">
        <Segmented
          size={28}
          value={(display.familyUnfold as string) ?? "ask"}
          onChange={(v) => setDisplay("familyUnfold", v)}
          options={[
            { value: "ask", label: "Pytaj" },
            { value: "all", label: "Wszyscy" },
            { value: "two", label: "2 pokolenia" },
          ]}
        />
      </SettingRow>
      {/* Manual order (dragging siblings in the tree) isn't built yet: siblings follow their birth dates. */}
      <SettingRow label="Kolejność rodzeństwa" soon>
        <Segmented
          size={28}
          value="date"
          onChange={() => {}}
          options={[
            { value: "date", label: "Według daty" },
            { value: "manual", label: "Ręczna" },
          ]}
        />
      </SettingRow>
      <BranchColorSettings display={display} />
    </Section>
  );
}

/** Wersje zestawu kolorów gałęzi (tokens.css: `.pal-*`). */
const PALETTES = [
  { value: "muted", label: "Stonowane" },
  { value: "light", label: "Jasne" },
  { value: "bright", label: "Żywe" },
] as const;

interface BranchGroup {
  key: string;
  name: string;
  plural: string;
  branch: number;
  born: number;
  married: number;
}

/** „Kolory gałęzi”: the set of colours, and the colour of each branch (surname group, the biggest first). */
function BranchColorSettings({ display }: { display: Record<string, unknown> }) {
  const { data } = useApi<{ groups: BranchGroup[] }>("surnames.list");
  const { chosen, canChange, set, clear, reset } = useBranchColors();
  const readOnlyNote = "Archiwa otwarte razem są tylko do przeglądania";
  const [all, setAll] = useState(false);
  const palette = display.branchPalette === "light" || display.branchPalette === "bright" ? display.branchPalette : "muted";
  // Largest family first, as on the Nazwiska screen.
  const groups = useMemo(() => [...(data?.groups ?? [])].sort((a, b) => b.born - a.born || collator.compare(a.name, b.name)), [data]);
  const shown = all ? groups : groups.slice(0, 12);
  return (
    <>
      <SettingRow wide label="Zestaw kolorów" note={canChange ? "Jak mocne są kolory gałęzi w drzewie i na listach" : readOnlyNote}>
        <div className="pal-options" role="radiogroup" aria-label="Zestaw kolorów">
          {PALETTES.map((p) => (
            <button key={p.value} className={`pal-option${palette === p.value ? " on" : ""}`} role="radio" aria-checked={palette === p.value} disabled={!canChange} onClick={() => setDisplay("branchPalette", p.value)}>
              <span className="pal-check">{palette === p.value && <Check size={14} />}</span>
              <span className="pal-name">{p.label}</span>
              <span className={`pal-colours pal-${p.value}`}>
                {Array.from({ length: 12 }, (_, i) => i + 1).map((n) => (
                  <BranchShape key={n} branch={n} size={12} />
                ))}
              </span>
            </button>
          ))}
        </div>
      </SettingRow>
      <SettingRow wide label="Kolor każdej gałęzi" note={canChange ? "Gałąź to rodzina o jednym nazwisku. Bez wyboru kolory dostają po kolei, od największej" : readOnlyNote} last>
        <button className="btn secondary set-btn" disabled={!canChange || Object.keys(chosen).length === 0} onClick={reset}>
          Przywróć automatyczne kolory
        </button>
      </SettingRow>
      {shown.length > 0 && (
        <div className="bc-list" style={{ borderTop: "1px solid var(--border)" }}>
          {shown.map((g, i) => {
            const picked = g.key in chosen;
            return (
              <div key={g.key} className="bc-row" style={i === shown.length - 1 && groups.length <= shown.length ? { borderBottom: "none" } : undefined}>
                <BranchShape branch={g.branch} size={14} />
                <span className="name ellipsis">{g.plural}</span>
                <span className="count grow">{g.born > 0 ? people(g.born) : `${people(g.married)} przez małżeństwo`}</span>
                <BranchColorPicker
                  value={g.branch}
                  chosen={picked}
                  disabled={!canChange}
                  onPick={(n) => set(g.key, n)}
                  onAuto={() => clear(g.key)}
                  label={`Kolor gałęzi ${g.plural}`}
                >
                  <span>{picked ? `${g.branch} · ${BRANCH_NAMES[g.branch - 1]}` : "Automatycznie"}</span>
                  <ChevronDown size={13} color="var(--text3)" />
                </BranchColorPicker>
              </div>
            );
          })}
        </div>
      )}
      {groups.length > 12 && (
        <div className="bc-foot">
          <button className="btn ghost" onClick={() => setAll((a) => !a)}>
            {all ? "Pokaż tylko największe" : `Pokaż wszystkie (${num(groups.length)})`}
          </button>
        </div>
      )}
    </>
  );
}

function NamesSection({ display }: { display: Record<string, unknown> }) {
  return (
    <Section id="names" title="Osoby i daty">
      <SettingRow label="Kolejność">
        <Segmented
          size={28}
          value={(display.nameOrder as string) ?? "given"}
          onChange={(v) => setDisplay("nameOrder", v)}
          options={[
            { value: "given", label: "Imię Nazwisko" },
            { value: "surname", label: "Nazwisko Imię" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Format daty">
        <Segmented
          size={28}
          value={(display.dateFormat as string) ?? "long"}
          onChange={(v) => setDisplay("dateFormat", v)}
          options={[
            { value: "long", label: "12 marca 1878" },
            { value: "numeric", label: "12.03.1878" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Daty juliańskie" note="Obok gregoriańskich w aktach z zaboru rosyjskiego">
        <Toggle on={(display.julian as boolean | undefined) ?? true} onChange={(v) => setDisplay("julian", v)} />
      </SettingRow>
      <SettingRow label="Nazwisko panieńskie">
        {/* The same choice as the People screen's „Nazwiska” popover. */}
        <Segmented
          size={28}
          value={(display.maidenStyle as string) ?? "zd"}
          onChange={(v) => setDisplay("maidenStyle", v)}
          options={[
            { value: "zd", label: "z d. Kowalska", title: "Helena Wiśniewska z d. Kowalska" },
            { value: "zKowalskich", label: "z Kowalskich", title: "Helena z Kowalskich Wiśniewska" },
            { value: "paren", label: "(Kowalska)", title: "Helena Wiśniewska (Kowalska)" },
          ]}
        />
      </SettingRow>
      <SettingRow label="Kobiety po ślubie na listach" last>
        <Segmented
          size={28}
          value={(display.marriedWomen as string) ?? "both"}
          onChange={(v) => setDisplay("marriedWomen", v)}
          options={[
            { value: "both", label: "W obu grupach" },
            { value: "birth", label: "Rodowe" },
            { value: "current", label: "Obecne" },
          ]}
        />
      </SettingRow>
    </Section>
  );
}
