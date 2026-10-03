import { ArrowRight, Cake, CalendarHeart, Flower2, HardDriveDownload, Heart, History, ImagePlus, Import, UserPlus, UserRoundPlus } from "lucide-react";
import type { ReactNode } from "react";
import { call } from "../../api/transport";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar, rowButton } from "../../components/bits";
import { dayMonth, num, plural, relativeTime, todayLong } from "../../lib/format";
import { pickFiles } from "../../lib/native";
import { runEdit } from "../media/shared";
import { EmptyArchive } from "./EmptyArchive";
import "./start.css";

interface Ref {
  id: string;
  name: string;
  initials: string;
  branch: number;
  photo: string | null;
}

interface StartData {
  title: string;
  stats: { people: number; photos: number; documents: number; generations: number };
  onThisDay: {
    birthdays: { person: Ref; context: string; value: string }[];
    deaths: { person: Ref; context: string; value: string }[];
    weddings: { person: Ref; label: string; context: string; value: string }[];
  };
  recentlyAdded: { person: Ref; origin: string; when: string }[];
  recentlyEdited: { person: Ref; what: string; who: string; when: string }[];
}

/** Start (spec §4.19): the app opens here (PLAN §11.1). */
export function Start() {
  const archive = useStore((s) => s.archive);
  const { data } = useApi<StartData>("start.data");
  if (archive && archive.people === 0) return <EmptyArchive />;
  return (
    <div className="page">
      <div className="start">
        <div className="col" style={{ gap: 4 }}>
          <span style={{ fontSize: 13, color: "var(--text3)" }}>{todayLong()}</span>
          <h1 className="serif" style={{ fontSize: 38, lineHeight: 1.1, fontWeight: 500 }}>
            {data?.title || archive?.name}
          </h1>
        </div>
        <div className="stat-block" style={{ gridTemplateColumns: "repeat(4, 1fr)" }}>
          <Stat value={data?.stats.people ?? archive?.people ?? 0} words={["osoba", "osoby", "osób"]} />
          <Stat value={data?.stats.photos ?? 0} words={["zdjęcie", "zdjęcia", "zdjęć"]} />
          <Stat value={data?.stats.documents ?? 0} words={["dokument", "dokumenty", "dokumentów"]} />
          <Stat value={data?.stats.generations ?? 0} words={["pokolenie", "pokolenia", "pokoleń"]} />
        </div>
        <div className="start-grid">
          <OnThisDay data={data} />
          <div className="col" style={{ gap: 18 }}>
            <RecentlyAdded data={data} />
            <RecentlyEdited data={data} />
          </div>
          <div className="col" style={{ gap: 18 }}>
            <QuickActions />
            <RecentlyViewed />
          </div>
        </div>
      </div>
    </div>
  );
}

function Stat({ value, words }: { value: number; words: [string, string, string] }) {
  return (
    <div>
      <span className="value">{num(value)}</span>
      <span className="key">{plural(value, ...words)}</span>
    </div>
  );
}

function Card({ icon, title, right, children, footer }: { icon?: ReactNode; title: string; right?: ReactNode; children: ReactNode; footer?: ReactNode }) {
  return (
    <div className="card">
      <div className="card-head">
        {icon}
        <span className="title">{title}</span>
        {right}
      </div>
      {children}
      {footer}
    </div>
  );
}

function PersonLine({ person, title, sub, value, onClick }: { person: Ref; title?: string; sub: string; value?: string; onClick: () => void }) {
  return (
    <button className="row person-line" onClick={onClick}>
      <Avatar initials={person.initials} branch={person.branch} photo={person.photo} size={32} />
      <span className="col grow" style={{ minWidth: 0, textAlign: "left" }}>
        <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
          {title ?? person.name}
        </span>
        <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
          {sub}
        </span>
      </span>
      {value && (
        <span className="num" style={{ fontSize: 13, fontWeight: 600, color: "var(--accent-text)", whiteSpace: "nowrap" }}>
          {value}
        </span>
      )}
    </button>
  );
}

function OnThisDay({ data }: { data: StartData | null }) {
  const go = useStore((s) => s.go);
  const day = data?.onThisDay;
  const empty = day && day.birthdays.length + day.deaths.length + day.weddings.length === 0;
  const group = (icon: ReactNode, label: string, rows: ReactNode) => (
    <div style={{ padding: "10px 16px 6px", borderBottom: "1px solid var(--border)" }}>
      <div className="row label-caps" style={{ gap: 6, marginBottom: 4 }}>
        {icon}
        {label}
      </div>
      {rows}
    </div>
  );
  return (
    <Card icon={<CalendarHeart size={16} color="var(--accent-text)" />} title="W tym dniu" right={<span style={{ fontSize: 12, color: "var(--text3)" }}>{dayMonth(new Date())}</span>}>
      {empty && <div style={{ padding: "16px", fontSize: 13, color: "var(--text3)" }}>Dziś nie ma rocznic w archiwum.</div>}
      {day && day.birthdays.length > 0 &&
        group(
          <Cake size={13} />,
          "Urodziny",
          day.birthdays.slice(0, 4).map((b) => <PersonLine key={b.person.id} person={b.person} sub={b.context} value={b.value} onClick={() => go({ name: "person", id: b.person.id })} />),
        )}
      {day && day.deaths.length > 0 &&
        group(
          <Flower2 size={13} />,
          "Rocznica śmierci",
          day.deaths.slice(0, 4).map((b) => <PersonLine key={b.person.id} person={b.person} sub={b.context} value={b.value} onClick={() => go({ name: "person", id: b.person.id })} />),
        )}
      {day && day.weddings.length > 0 &&
        group(
          <Heart size={13} />,
          "Rocznica ślubu",
          day.weddings.slice(0, 4).map((b) => <PersonLine key={b.person.id} person={b.person} title={b.label} sub={b.context} value={b.value} onClick={() => go({ name: "person", id: b.person.id })} />),
        )}
    </Card>
  );
}

function RecentlyAdded({ data }: { data: StartData | null }) {
  const go = useStore((s) => s.go);
  return (
    <Card
      icon={<UserRoundPlus size={16} color="var(--text2)" />}
      title="Ostatnio dodane"
      right={
        <button className="link row" style={{ fontSize: 12, gap: 4 }} onClick={() => go({ name: "people" })}>
          Wszystkie osoby <ArrowRight size={12} />
        </button>
      }
    >
      {data?.recentlyAdded.length === 0 && <div style={{ padding: 16, fontSize: 13, color: "var(--text3)" }}>Nikt nie został jeszcze dodany w Heirloom.</div>}
      {data?.recentlyAdded.map((r) => (
        <div key={r.person.id} className="list-row clickable" style={{ height: 48, gap: 10 }} {...rowButton(() => go({ name: "person", id: r.person.id }))}>
          <Avatar initials={r.person.initials} branch={r.person.branch} photo={r.person.photo} size={30} />
          <span className="col grow" style={{ minWidth: 0 }}>
            <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600 }}>
              {r.person.name}
            </span>
            <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
              {r.origin}
            </span>
          </span>
          <span style={{ fontSize: 12, color: "var(--text3)" }}>{relativeTime(r.when)}</span>
        </div>
      ))}
    </Card>
  );
}

function RecentlyEdited({ data }: { data: StartData | null }) {
  const go = useStore((s) => s.go);
  return (
    <Card
      icon={<History size={16} color="var(--text2)" />}
      title="Ostatnio edytowane"
      right={
        <button className="link row" style={{ fontSize: 12, gap: 4 }} onClick={() => go({ name: "activity" })}>
          Historia zmian <ArrowRight size={12} />
        </button>
      }
    >
      {data?.recentlyEdited.length === 0 && <div style={{ padding: 16, fontSize: 13, color: "var(--text3)" }}>Brak zmian w historii.</div>}
      {data?.recentlyEdited.map((r) => (
        <div key={r.person.id} className="list-row clickable" style={{ height: 44, gap: 10 }} {...rowButton(() => go({ name: "person", id: r.person.id }))}>
          <span className="serif ellipsis" style={{ fontSize: 15, fontWeight: 600, flex: "none", maxWidth: "45%" }}>
            {r.person.name}
          </span>
          <span className="ellipsis grow" style={{ fontSize: 13, color: "var(--text2)" }}>
            {r.what}
          </span>
          <span style={{ fontSize: 12, color: "var(--text3)", whiteSpace: "nowrap" }}>
            {r.who} · {relativeTime(r.when)}
          </span>
        </div>
      ))}
    </Card>
  );
}

function QuickActions() {
  const go = useStore((s) => s.go);
  const requireEdit = useStore((s) => s.requireEdit);
  const notify = useStore((s) => s.notify);
  const changed = useStore((s) => s.changed);
  const addPhotos = () =>
    runEdit(async () => {
      const paths = await pickFiles("Dodaj zdjęcia do archiwum", [{ name: "Zdjęcia i dokumenty", extensions: ["jpg", "jpeg", "png", "webp", "tif", "tiff", "bmp", "gif", "pdf"] }]);
      if (!paths.length) return;
      const added = await call<{ id: string; duplicate: boolean }[]>("media.add", { paths });
      changed();
      notify(`Dodano ${added.filter((a) => !a.duplicate).length} z ${paths.length} ${plural(paths.length, "pliku", "plików", "plików")} do Mediów. Przypisz je do osób w Mediach.`, {
        action: { label: "Otwórz Media", run: () => go({ name: "media" }) },
      });
    });
  const actions = [
    { icon: <UserPlus size={16} />, title: "Dodaj osobę", text: "włącza tryb edycji", run: () => requireEdit(() => go({ name: "edit", id: null })) },
    { icon: <Import size={16} />, title: "Importuj paczkę", text: "odpowiedź AI lub folder", run: () => go({ name: "import" }) },
    { icon: <ImagePlus size={16} />, title: "Dodaj zdjęcia", text: "wybierz pliki z dysku", run: addPhotos },
    { icon: <HardDriveDownload size={16} />, title: "Utwórz kopię zapasową", text: "cały folder archiwum jako .zip", run: () => go({ name: "settings", section: "backup" }) },
  ];
  return (
    <Card title="Szybkie akcje">
      {actions.map((a) => (
        <button key={a.title} className="list-row clickable quick-action" onClick={a.run}>
          <span className="icon-tile" style={{ width: 34, height: 34 }}>
            {a.icon}
          </span>
          <span className="col" style={{ textAlign: "left" }}>
            <span style={{ fontSize: 14, fontWeight: 600 }}>{a.title}</span>
            <span style={{ fontSize: 12, color: "var(--text3)" }}>{a.text}</span>
          </span>
        </button>
      ))}
    </Card>
  );
}

function RecentlyViewed() {
  const viewed = useStore((s) => s.recentlyViewed);
  const go = useStore((s) => s.go);
  return (
    <div className="card" style={{ padding: "14px 16px", display: "flex", flexDirection: "column", gap: 10 }}>
      <span style={{ fontSize: 14, fontWeight: 600 }}>Ostatnio oglądane</span>
      {viewed.length === 0 ? (
        <span style={{ fontSize: 13, color: "var(--text3)" }}>Tu pojawią się osoby, których profile otworzysz.</span>
      ) : (
        <div className="row" style={{ flexWrap: "wrap", gap: 6 }}>
          {viewed.map((p) => (
            <button key={p.id} className="person-chip" onClick={() => go({ name: "person", id: p.id })}>
              <Avatar initials={p.initials} branch={p.branch} photo={p.photo} size={24} />
              {p.name.split(" ")[0]}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
