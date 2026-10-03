import { Archive, Calendar, Import, Info, Network, Palette, Search, Users } from "lucide-react";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import { call } from "../../api/transport";
import type { AppState, Appearance, ArchiveStatus, PersonSummary } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, Segmented, SettingRow, Toggle, useDismiss } from "../../components/bits";
import { ArchiveSections } from "./archive";
import { AboutSection, EditorsSection, ImportSection } from "./more";
import { failed, saveSettings, Section, Select, setDisplay } from "./parts";
import "./settings.css";
import { cardYears } from "../../lib/format";

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
const NAV_OF: Record<string, string> = { settingsFiles: "archive", backup: "archive", export: "archive", tools: "archive" };

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

  return (
    <div className="page" ref={scrollRef}>
      <div className="settings">
        <nav className="set-nav">
          <h1>Ustawienia</h1>
          {NAV.map((item) => (
            <button key={item.id} className={`set-nav-item${active === item.id ? " active" : ""}`} onClick={() => show(item.id, true)}>
              <item.icon size={16} />
              {item.label}
            </button>
          ))}
        </nav>
        <div className="set-sections">
          <AppearanceSection />
          <TreeSection archive={archive} />
          <NamesSection display={archive.display} />
          <ArchiveSections archive={archive} />
          <ImportSection />
          <EditorsSection archive={archive} />
          <AboutSection />
        </div>
      </div>
    </div>
  );
}

function AppearanceSection() {
  const appearance = useStore((s) => s.app?.appearance);
  if (!appearance) return null;
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
      <SettingRow label="Kolor akcentu">
        <Select
          label="Kolor akcentu"
          title="Inne kolory będą w kolejnej wersji"
          value="green"
          options={[{ value: "green", label: "Zieleń (domyślny)" }]}
          onChange={() => {}}
          disabled
        />
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
  const [picking, setPicking] = useState(false);
  const { data: start, error: startError } = useApi<PersonSummary>(archive.startPerson ? "person.hover" : null, { id: archive.startPerson });
  const startName = start?.id === archive.startPerson ? start.name : startError ? "nie ma tej osoby" : "…";
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
      <SettingRow label="Osoba startowa" note="Od niej otwiera się drzewo">
        <div style={{ position: "relative" }}>
          <Segmented
            size={28}
            value={archive.startPerson ? "chosen" : "recent"}
            onChange={(v) => (v === "recent" ? saveSettings({ startPerson: null }) : setPicking(true))}
            options={[
              {
                value: "chosen",
                label: archive.startPerson ? <span className="ellipsis" style={{ maxWidth: 220 }}>Wybrana: {startName}</span> : "Wybrana…",
                title: "Wybierz osobę",
              },
              { value: "recent", label: "Ostatnio oglądana" },
            ]}
          />
          {picking && (
            <PersonPicker
              onClose={() => setPicking(false)}
              onPick={(id) => {
                setPicking(false);
                saveSettings({ startPerson: id });
              }}
            />
          )}
        </div>
      </SettingRow>
      <SettingRow label="Styl karty">
        <Segmented
          size={28}
          value={(display.cardStyle as string) === "plain" ? "plain" : "photo"}
          onChange={(v) => setDisplay("cardStyle", v)}
          options={[
            { value: "photo", label: "Ze zdjęciem" },
            { value: "plain", label: "Bez zdjęcia" },
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
      {/* Manual order (dragging siblings in the tree) isn't built yet, so there is nothing to choose. */}
      <SettingRow label="Kolejność rodzeństwa" note="Ręczne układanie w drzewie — w przygotowaniu">
        <span style={{ fontSize: 13, color: "var(--text2)" }}>Według daty</span>
      </SettingRow>
      <SettingRow label="Kolory gałęzi" note="Każda gałąź ma swój kolor i kształt" last>
        <span className="swatches">
          {Array.from({ length: 12 }, (_, i) => i + 1).map((n) => (
            <span key={n} style={{ background: `var(--b${n})`, borderRadius: n % 3 === 0 ? "50%" : 3 }} />
          ))}
        </span>
      </SettingRow>
    </Section>
  );
}

type Hit = PersonSummary & { context: string };

/** Choosing the start person: a search over everyone in the archive. */
function PersonPicker({ onPick, onClose }: { onPick: (id: string) => void; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const ref = useDismiss<HTMLDivElement>(true, onClose);
  useEffect(() => {
    if (!query.trim()) {
      setHits([]);
      return;
    }
    let cancelled = false;
    call<Hit[]>("people.search", { q: query, limit: 8 })
      .then((list) => !cancelled && setHits(list))
      .catch(failed);
    return () => {
      cancelled = true;
    };
  }, [query]);
  return (
    <div ref={ref} className="popover" style={{ top: 40, right: 0, width: 320 }}>
      <div style={{ padding: 8 }}>
        <div className="search-box">
          <Search size={14} />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && hits[0] && onPick(hits[0].id)}
            placeholder="Szukaj osoby…"
          />
        </div>
      </div>
      {hits.length > 0 && (
        <div style={{ paddingBottom: 4 }}>
          {hits.map((h) => (
            <button key={h.id} className="menu-item" style={{ minHeight: 46 }} onClick={() => onPick(h.id)}>
              <Avatar initials={h.initials} branch={h.branch} photo={h.photo} size={28} />
              <span className="col grow" style={{ minWidth: 0 }}>
                <span className="ellipsis" style={{ fontWeight: 500 }}>
                  {h.name}
                </span>
                <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                  {h.context}
                </span>
              </span>
              <span className="num" style={{ fontSize: 12, color: "var(--text2)" }}>
                {cardYears(h.birth?.year, h.death?.year, h.living)}
              </span>
            </button>
          ))}
        </div>
      )}
      {query.trim() && hits.length === 0 && <div style={{ padding: "4px 12px 12px", fontSize: 13, color: "var(--text3)" }}>Nikogo nie znaleziono.</div>}
    </div>
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
