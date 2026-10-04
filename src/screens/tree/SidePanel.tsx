import { Briefcase, Handshake, Image as ImageIcon, Pencil, Plus, X } from "lucide-react";
import { mediaUrl } from "../../api/transport";
import type { PersonSummary } from "../../api/types";
import { useStore } from "../../app/store";
import { useApi } from "../../app/useApi";
import { Avatar } from "../../components/bits";

interface PanelData {
  person: PersonSummary;
  photo: { path: string; caption: string | null; date: string | null } | null;
  generationBranch: string;
  birth: string | null;
  death: string | null;
  deathAge: string | null;
  occupation: string | null;
  relatives: (PersonSummary & { label: string | null })[];
}

/** „Wybrana osoba” (spec §4.1, design 17h): a mini profile with relatives. Browsing shows no edit actions; in edit mode
 *  „Edytuj” opens the profile's personal data and „Dodaj krewnego” adds a relative. */
export function SidePanel({ id, onClose, onFocus, relativeTitle = "Pokaż w centrum drzewa" }: { id: string; onClose: () => void; onFocus: (id: string) => void; relativeTitle?: string }) {
  const { data } = useApi<PanelData>("person.panel", { id });
  const go = useStore((s) => s.go);
  const mode = useStore((s) => s.mode);
  const requireEdit = useStore((s) => s.requireEdit);
  const editing = mode === "edit";
  const p = data?.person;
  const add = (kind: "parent" | "partner" | "child" | "sibling") => requireEdit(() => go({ name: "edit", id: null, relation: { kind, of: id } }));

  return (
    <aside className="tree-panel">
      <div className="tree-panel-head">
        <span className="label-caps" style={{ fontSize: 12 }}>
          Wybrana osoba
        </span>
        <button className="icon-btn" style={{ width: 28, height: 28 }} onClick={onClose} title="Zamknij">
          <X size={16} />
        </button>
      </div>
      {p && data && (
        <div className="tree-panel-body">
          <div className="panel-photo">
            {data.photo ? (
              <img src={mediaUrl(data.photo.path, "thumb", 512)} alt="" style={{ width: "100%", height: "100%", objectFit: "cover" }} />
            ) : (
              <>
                <ImageIcon size={22} />
                <span>Brak zdjęcia</span>
              </>
            )}
          </div>
          {data.photo?.caption && <span style={{ marginTop: -10, fontSize: 12, color: "var(--text3)" }}>{data.photo.caption}</span>}
          <div className="col" style={{ gap: 4 }}>
            <span className="serif" style={{ fontSize: 26, lineHeight: 1.15, fontWeight: 500 }}>
              {p.name}
            </span>
            {p.maiden && <span style={{ fontSize: 14, color: "var(--text2)" }}>z d. {p.maiden}</span>}
            {p.nickname && (
              <span className="serif" style={{ fontSize: 16, fontStyle: "italic", color: "var(--text2)" }}>
                „{p.nickname}”
              </span>
            )}
            {data.generationBranch && (
              <span style={{ alignSelf: "flex-start", marginTop: 6, padding: "3px 9px", borderRadius: "var(--r-ctl)", background: "var(--surface2)", color: "var(--text2)", fontSize: 12, fontWeight: 600 }}>
                {data.generationBranch}
              </span>
            )}
          </div>
          <div className="col" style={{ gap: 7 }}>
            {data.birth && (
              <div className="panel-fact">
                <span>ur.</span>
                <span>{data.birth}</span>
              </div>
            )}
            {data.death && (
              <div className="panel-fact">
                <span>zm.</span>
                <span>
                  {data.death}
                  {data.deathAge && <span style={{ color: "var(--text3)" }}> ({data.deathAge})</span>}
                </span>
              </div>
            )}
            {!data.death && p.living && (
              <div className="panel-fact">
                <span />
                <span style={{ color: "var(--text3)" }}>żyje</span>
              </div>
            )}
            {data.occupation && (
              <div className="panel-fact">
                <span>
                  <Briefcase size={13} />
                </span>
                <span>{data.occupation}</span>
              </div>
            )}
          </div>
          {data.relatives.length > 0 && (
            <div className="col" style={{ gap: 2, borderTop: "1px solid var(--border)", paddingTop: 12 }}>
              {data.relatives.map((r) => (
                <button key={`${r.id}-${r.label}`} className="panel-relative" onClick={() => onFocus(r.id)} title={relativeTitle}>
                  <Avatar initials={r.initials} branch={r.branch} photo={r.photo} size={26} />
                  {/* The name keeps its room; the maiden name shrinks first. */}
                  <span className="row grow" style={{ gap: 6, minWidth: 0 }}>
                    <span className="ellipsis" style={{ fontSize: 13, fontWeight: 500, flex: "0 1 auto", minWidth: 60 }}>
                      {r.name}
                    </span>
                    {r.maiden && (
                      <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)", flex: "0 8 auto", minWidth: 24 }}>
                        z d. {r.maiden}
                      </span>
                    )}
                  </span>
                  <span style={{ fontSize: 12, color: "var(--text3)", whiteSpace: "nowrap" }}>{r.label}</span>
                </button>
              ))}
            </div>
          )}
          <div className="row" style={{ gap: 8 }}>
            <button className="btn primary grow" onClick={() => go({ name: "person", id })}>
              Otwórz profil
            </button>
            {editing && (
              <button className="btn secondary" onClick={() => requireEdit(() => go({ name: "person", id, open: "personal" }))}>
                <Pencil size={14} />
                Edytuj
              </button>
            )}
          </div>
          {editing && (
            <div className="col" style={{ gap: 6 }}>
              <span style={{ fontSize: 12, color: "var(--text3)" }}>Dodaj krewnego</span>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>
                {(
                  [
                    ["parent", "Rodzic"],
                    ["partner", "Partner"],
                    ["child", "Dziecko"],
                    ["sibling", "Rodzeństwo"],
                  ] as const
                ).map(([kind, label]) => (
                  <button key={kind} className="btn secondary sm" style={{ height: 32, justifyContent: "flex-start", color: "var(--text2)" }} onClick={() => add(kind)}>
                    <Plus size={13} />
                    {label}
                  </button>
                ))}
              </div>
              <button className="btn dashed sm" style={{ height: 32 }} onClick={() => requireEdit(() => go({ name: "person", id, open: "family" }))}>
                <Handshake size={14} />
                Relacja spoza rodziny…
              </button>
            </div>
          )}
        </div>
      )}
    </aside>
  );
}
