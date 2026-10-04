import { History, ImagePlus, Image as ImageIcon, Network, Pencil } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { call } from "../../api/transport";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { ArchiveBadge, BranchShape, EmptyState, Lifespan, Thumb } from "../../components/bits";
import { count } from "../../lib/format";
import { pickFiles } from "../../lib/native";
import { afterChange } from "../../app/store";
import { runEdit } from "../media/shared";
import { CombinedCard, EditInArchive, LinkWithOther } from "./CombinedParts";
import { PersonalEditor } from "./PersonalSection";
import { useSection } from "./sectionEdit";
import {
  DocumentsSection,
  FamilySection,
  BrokenLinksNotice,
  GallerySection,
  HistorySection,
  LinksSection,
  MentionedSection,
  NotesSection,
  OtherDataSection,
  SayingsSection,
  SourcesSection,
  StoriesSection,
  SummarySection,
  TimelineSection,
  TriviaSection,
  BioSection,
} from "./ProfileSections";
import type { Profile as ProfileData } from "./types";
import "./profile.css";

interface TocItem {
  id: string;
  label: string;
  sub?: boolean;
}

/** Profil osoby (spec §4.6–§4.7, design 17a): hero, a sticky table of contents and the sections. In browse mode empty
 *  sections are hidden and nothing can be changed; in edit mode each section is edited in place, one at a time.
 *  `open` opens a section for editing on arrival (the tree's „Edytuj” opens the personal data). */
export function Profile({ id, section, open }: { id: string; section?: string; open?: string }) {
  const { data, error } = useApi<ProfileData>("person.get", { id });
  const setCrumb = useStore((s) => s.setCrumb);
  const viewed = useStore((s) => s.viewed);
  const mode = useStore((s) => s.mode);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState<string>("sec-summary");
  const editing = mode === "edit";

  useEffect(() => {
    if (!data) return;
    setCrumb(data.person.name);
    viewed({ id: data.person.id, name: data.person.name, initials: data.person.initials, branch: data.person.branch, photo: data.person.photo });
  }, [data, setCrumb, viewed]);

  useEffect(() => {
    if (data && section) {
      requestAnimationFrame(() => document.getElementById(`sec-${section}`)?.scrollIntoView({ block: "start" }));
    }
    // Only when the profile first arrives.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data?.person.id, section]);

  // Opening the profile to edit a section (the tree's „Edytuj”, already in edit mode). Used once: the route loses
  // `open`, so Back and Forward don't open it again.
  const loaded = !!data;
  useEffect(() => {
    if (!loaded || !open) return;
    const store = useStore.getState();
    store.replaceRoute({ name: "person", id, section });
    if (store.mode !== "edit") return;
    const labels: Record<string, string> = { personal: "Dane osobowe", family: "Rodzina" };
    if (store.openSection(`${id}:${open}`, labels[open] ?? open)) {
      requestAnimationFrame(() => document.getElementById(open === "personal" ? "sec-personal" : `sec-${open}`)?.scrollIntoView({ block: "start" }));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loaded, open, id]);

  // Leaving the profile closes its open section; what it changed stays as unsaved changes.
  useEffect(
    () => () => {
      if (useStore.getState().section?.key.startsWith(`${id}:`)) useStore.getState().finishSection();
    },
    [id],
  );

  const toc: TocItem[] = useMemo(() => {
    if (!data) return [];
    const items: TocItem[] = [];
    const show = (has: boolean) => has || editing;
    if (editing) items.push({ id: "sec-personal", label: "Dane osobowe" });
    if (show(!!data.summary || data.facts.length > 0)) items.push({ id: "sec-summary", label: "W skrócie" });
    if (show(data.family.length > 0)) items.push({ id: "sec-family", label: "Rodzina" });
    if (show(data.bio.length > 0)) {
      items.push({ id: "sec-bio", label: "Życiorys" });
      for (const b of data.bio) if (b.title && b.id) items.push({ id: `sec-bio-${b.id}`, label: b.title, sub: true });
    }
    if (show(data.stories.length > 0)) items.push({ id: "sec-stories", label: "Historie" });
    if (show(data.sayings.length > 0)) items.push({ id: "sec-sayings", label: "Powiedzonka" });
    if (show(data.trivia.length > 0)) items.push({ id: "sec-trivia", label: "Ciekawostki" });
    if (show(data.gallery.length > 0)) items.push({ id: "sec-gallery", label: "Galeria" });
    if (data.documents.length > 0) items.push({ id: "sec-documents", label: "Dokumenty" });
    if (data.sources.length > 0) items.push({ id: "sec-sources", label: "Źródła" });
    if (show(data.links.length > 0)) items.push({ id: "sec-links", label: "Linki" });
    if (data.timeline.length > 0) items.push({ id: "sec-timeline", label: "Oś życia" });
    if (data.mentionedIn.length > 0) items.push({ id: "sec-mentioned", label: "Wspomniany w" });
    if (show(data.notes.length > 0)) items.push({ id: "sec-notes", label: "Uwagi badawcze" });
    if (data.history.groups.length > 0 || data.history.origin) items.push({ id: "sec-history", label: "Historia zmian" });
    if (data.other.length > 0) items.push({ id: "sec-other", label: "Inne dane z pliku" });
    return items;
  }, [data, editing]);

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    const onScroll = () => {
      const top = scroller.getBoundingClientRect().top + 90;
      let current = toc[0]?.id;
      for (const item of toc) {
        const el = document.getElementById(item.id);
        if (el && el.getBoundingClientRect().top <= top) current = item.id;
      }
      if (current) setActive(current);
    };
    onScroll();
    scroller.addEventListener("scroll", onScroll, { passive: true });
    return () => scroller.removeEventListener("scroll", onScroll);
  }, [toc]);

  if (error) return <EmptyState title="Nie ma tej osoby" text={error.message} />;
  if (!data) return <div className="page" />;

  return (
    <div className="page" ref={scrollRef}>
      <Hero data={data} />
      <div className="profile-body">
        <nav className="toc">
          <div className="toc-label">Spis treści</div>
          {toc.map((item) => (
            <button
              key={item.id}
              className={`toc-item${item.sub ? " sub" : ""}${active === item.id ? " active" : ""}`}
              onClick={() => document.getElementById(item.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
            >
              <span className="ellipsis">{item.label}</span>
            </button>
          ))}
        </nav>
        <article className="profile-article">
          <CombinedCard data={data} />
          <BrokenLinksNotice data={data} />
          <SummarySection data={data} />
          <FamilySection data={data} />
          <BioSection data={data} />
          <StoriesSection data={data} />
          <SayingsSection data={data} />
          <TriviaSection data={data} />
          <GallerySection data={data} />
          <DocumentsSection data={data} />
          <SourcesSection data={data} />
          <LinksSection data={data} />
          <TimelineSection data={data} />
          <MentionedSection data={data} />
          <NotesSection data={data} />
          <HistorySection data={data} />
          <OtherDataSection data={data} />
        </article>
      </div>
    </div>
  );
}

/** The top of the profile. In edit mode it is the „Dane osobowe” section: „Edytuj” opens it in place (design 17a). */
function Hero({ data }: { data: ProfileData }) {
  const go = useStore((s) => s.go);
  const mode = useStore((s) => s.mode);
  const notify = useStore((s) => s.notify);
  const p = data.person;
  const editing = mode === "edit";
  const combined = data.combined;
  const personal = useSection(`${p.id}:personal`, "Dane osobowe");
  const birthFact = data.facts.find((f) => f.key.startsWith("Urodzon") || f.key.startsWith("Ochrzczon"));
  const deathFact = data.facts.find((f) => f.key.startsWith("Zmarł") || f.key.startsWith("Pochowan"));

  if (editing && personal.open) {
    return (
      <div id="sec-personal">
        <PersonalEditor id={p.id} profile={data} onDone={personal.done} onCancel={personal.cancel} />
      </div>
    );
  }

  const addPhoto = () =>
    useStore.getState().outsideSection(() => runEdit(async () => {
      const paths = await pickFiles(`Dodaj zdjęcia: ${p.name}`, [{ name: "Zdjęcia i dokumenty", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif", "pdf"] }]);
      if (!paths.length) return;
      const added = await call<{ id: string; duplicate: boolean }[]>("media.add", { paths, person: p.id, profile: !p.photo });
      afterChange();
      const dup = added.filter((a) => a.duplicate).length;
      notify(`Dodano ${count(added.length - dup, "plik", "pliki", "plików")}${dup ? ` (${count(dup, "plik był", "pliki były", "plików było")} już w archiwum — połączono)` : ""}.`);
    }));

  return (
    <section id="sec-personal" className="hero">
      <div className="portrait">
        {data.portrait ? (
          <Thumb path={data.portrait.path} size={512} style={{ width: 200, height: 256, borderRadius: "var(--r-card)" }} />
        ) : (
          <div className="portrait-empty">
            <ImageIcon size={24} />
            <span>Brak zdjęcia</span>
          </div>
        )}
        {data.portrait?.caption && <span className="portrait-caption">{data.portrait.caption}</span>}
      </div>
      <div className="col" style={{ gap: 10, minWidth: 0 }}>
        {(data.generationBranch || combined) && (
          <span className="row" style={{ gap: 8, fontSize: 12, color: "var(--text2)", flexWrap: "wrap" }}>
            {data.generationBranch && (
              <>
                <BranchShape branch={p.branch} size={8} style={{ borderRadius: "50%", transform: "none" }} />
                {capitalizeFirst(data.generationBranch)}
              </>
            )}
            <ArchiveBadge from={p.from} />
          </span>
        )}
        <h1 className="hero-name">{p.name}</h1>
        {p.maiden && <span className="serif" style={{ marginTop: -6, fontSize: 18, color: "var(--text2)" }}>z d. {p.maiden}</span>}
        {p.nickname && <span className="hero-nickname">„{p.nickname}”</span>}
        <div className="row hero-life">
          <span>{birthFact ? birthFact.value.replace(/ (par. [^)]*)/, "") : <Lifespan birth={p.birth} death={null} living={p.living} />}</span>
          {(deathFact || !p.living) && <span style={{ color: "var(--text3)" }}>—</span>}
          {deathFact && <span>{deathFact.value.replace(/ (par. [^)]*)/, "")}</span>}
          {!deathFact && p.living && <span style={{ color: "var(--text3)" }}>żyje</span>}
          {data.age && <span style={{ color: "var(--text3)" }}>· {data.age}</span>}
        </div>
        {data.tags.length > 0 && (
          <div className="row" style={{ gap: 6, flexWrap: "wrap", marginTop: 4 }}>
            {data.tags.map((t) => (
              <span key={t} className="tag">
                {t}
              </span>
            ))}
          </div>
        )}
        <div className="row" style={{ gap: 8, marginTop: 14, flexWrap: "wrap" }}>
          {editing && (
            <>
              <button className="btn primary" disabled={personal.blocked} title={personal.blocked ? "Najpierw zakończ edycję otwartej sekcji" : "Imiona, daty, miejsca, tagi"} onClick={() => personal.start()}>
                <Pencil size={14} />
                Edytuj
              </button>
              <button className="btn secondary" onClick={addPhoto}>
                <ImagePlus size={14} />
                Dodaj zdjęcie
              </button>
            </>
          )}
          <button className={`btn ${editing ? "ghost" : "secondary"}`} onClick={() => go({ name: "tree", view: "family", person: p.id })}>
            <Network size={14} />
            Pokaż w drzewie
          </button>
          {combined ? (
            <>
              <EditInArchive members={combined.members} />
              <LinkWithOther person={p} />
            </>
          ) : (
            <button className="btn ghost" style={{ padding: "0 12px" }} onClick={() => document.getElementById("sec-history")?.scrollIntoView({ behavior: "smooth" })}>
              <History size={14} />
              Historia zmian
            </button>
          )}
        </div>
      </div>
    </section>
  );
}

function capitalizeFirst(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
