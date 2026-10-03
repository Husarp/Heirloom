// The texts of biographies and stories: simple Markdown (paragraphs, **bold**, *italic*, lists, quotes, links) with
// person mentions `[Antoniego](person:@I12@)` shown as quiet links with a hover card (spec §5.8).

import { ArrowRight } from "lucide-react";
import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { call } from "../api/transport";
import type { DateText } from "../api/types";
import { useStore } from "../app/store";
import { openUrl } from "../lib/native";
import { Avatar, Lifespan } from "./bits";

interface HoverData {
  id: string;
  name: string;
  maiden: string | null;
  initials: string;
  branch: number;
  photo: string | null;
  birth: DateText | null;
  death: DateText | null;
  living: boolean;
  relation?: string | null;
  places: string;
  mentionCount: number;
}

const cache = new Map<string, HoverData>();

export function PersonMention({ id, from, children }: { id: string; from?: string; children: ReactNode }) {
  const go = useStore((s) => s.go);
  const dataVersion = useStore((s) => s.dataVersion);
  const [card, setCard] = useState<{ data: HoverData; x: number; y: number } | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const over = useRef(false);
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const show = () => {
    over.current = true;
    timer.current = window.setTimeout(async () => {
      const key = `${dataVersion}|${id}|${from ?? ""}`;
      let data = cache.get(key);
      if (!data) {
        try {
          data = await call<HoverData>("person.hover", { id, from });
          cache.set(key, data);
        } catch {
          return;
        }
      }
      // The pointer may have left while the card was loading; it would then stay open.
      if (!over.current) return;
      const rect = ref.current?.getBoundingClientRect();
      if (rect) setCard({ data, x: Math.min(rect.left, window.innerWidth - 320), y: rect.bottom + 8 });
    }, 400);
  };
  const hide = () => {
    over.current = false;
    window.clearTimeout(timer.current);
    setCard(null);
  };

  return (
    <>
      <span
        ref={ref}
        className="mention"
        tabIndex={0}
        role="link"
        onMouseEnter={show}
        onMouseLeave={hide}
        onFocus={show}
        onBlur={hide}
        onClick={() => {
          hide();
          go({ name: "person", id });
        }}
        onKeyDown={(e) => e.key === "Enter" && go({ name: "person", id })}
      >
        {children}
      </span>
      {card &&
        createPortal(
          <div className="hover-card" style={{ left: card.x, top: card.y }}>
            <div className="row" style={{ gap: 12 }}>
              <Avatar initials={card.data.initials} branch={card.data.branch} photo={card.data.photo} size={48} />
              <div className="col" style={{ minWidth: 0 }}>
                <span className="serif" style={{ fontSize: 16, fontWeight: 600 }}>
                  {card.data.name}
                </span>
                <span style={{ color: "var(--text2)" }}>
                  {card.data.maiden && <>z d. {card.data.maiden} · </>}
                  <Lifespan birth={card.data.birth} death={card.data.death} living={card.data.living} />
                </span>
              </div>
            </div>
            {card.data.relation && <span className="rel-chip">{card.data.relation}</span>}
            {card.data.places && <span style={{ color: "var(--text2)" }}>{card.data.places}</span>}
            <span className="link row" style={{ gap: 4 }}>
              Otwórz profil <ArrowRight size={13} />
            </span>
          </div>,
          document.body,
        )}
    </>
  );
}

/** Inline formatting: **bold**, *italic*, links and mentions. */
function inline(text: string, from: string | undefined, keyBase: string): ReactNode[] {
  const out: ReactNode[] = [];
  const pattern = /\[([^\]]+)\]\(([^)]+)\)|\*\*([^*]+)\*\*|\*([^*]+)\*|_([^_]+)_/g;
  let last = 0;
  let match: RegExpExecArray | null;
  let n = 0;
  while ((match = pattern.exec(text))) {
    if (match.index > last) out.push(text.slice(last, match.index));
    const key = `${keyBase}-${n++}`;
    if (match[1] != null) {
      const target = match[2].trim();
      if (target.startsWith("person:!")) {
        // Someone who isn't in the archive any more: the link leads nowhere, and says so.
        out.push(
          <span key={key} className="mention broken" title="Tej osoby nie ma już w archiwum — link prowadzi donikąd">
            {match[1]}
          </span>,
        );
      } else if (target.startsWith("person:")) {
        out.push(
          <PersonMention key={key} id={target.slice(7)} from={from}>
            {match[1]}
          </PersonMention>,
        );
      } else if (/^https?:\/\//.test(target)) {
        out.push(
          <span key={key} className="link" role="link" tabIndex={0} onClick={() => openUrl(target)} onKeyDown={(e) => e.key === "Enter" && openUrl(target)}>
            {match[1]} ↗
          </span>,
        );
      } else {
        out.push(match[1]);
      }
    } else if (match[3] != null) {
      out.push(<strong key={key}>{match[3]}</strong>);
    } else {
      out.push(<em key={key}>{match[4] ?? match[5]}</em>);
    }
    last = pattern.lastIndex;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

export function Markdown({ text, from, className }: { text: string; from?: string; className?: string }) {
  const blocks = text.replace(/\r\n/g, "\n").split(/\n{2,}/);
  return (
    <div className={className}>
      {blocks.map((block, b) => {
        const lines = block.split("\n");
        if (lines.every((l) => /^\s*([-*]|\d+\.)\s+/.test(l))) {
          const ordered = /^\s*\d+\./.test(lines[0]);
          const items = lines.map((l, i) => <li key={i}>{inline(l.replace(/^\s*([-*]|\d+\.)\s+/, ""), from, `${b}-${i}`)}</li>);
          return ordered ? <ol key={b}>{items}</ol> : <ul key={b}>{items}</ul>;
        }
        if (lines.every((l) => l.startsWith(">"))) {
          return <blockquote key={b}>{inline(lines.map((l) => l.replace(/^>\s?/, "")).join(" "), from, `${b}`)}</blockquote>;
        }
        if (/^#{1,4}\s/.test(lines[0])) {
          return (
            <Fragment key={b}>
              <h3>{inline(lines[0].replace(/^#{1,4}\s/, ""), from, `${b}h`)}</h3>
              {lines.length > 1 && <p>{inline(lines.slice(1).join(" "), from, `${b}p`)}</p>}
            </Fragment>
          );
        }
        return (
          <p key={b}>
            {lines.map((l, i) => (
              <Fragment key={i}>
                {i > 0 && <br />}
                {inline(l, from, `${b}-${i}`)}
              </Fragment>
            ))}
          </p>
        );
      })}
    </div>
  );
}

/** The text without Markdown (for excerpts). */
export function plainText(text: string): string {
  return text
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/[*_]/g, "")
    .replace(/^#+\s/gm, "");
}
