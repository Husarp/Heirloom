// The text editor for biographies, stories and notes (spec §5.12): headings, bold, italic, lists, quotes, links and
// "@" to mention a person. Texts are stored as Markdown (richtext.ts translates).

import Mention from "@tiptap/extension-mention";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor, type Editor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Bold, Heading2, Italic, Link as LinkIcon, List, ListOrdered, Quote, UserPlus } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { call } from "../api/transport";
import type { PersonSummary } from "../api/types";
import { Avatar } from "./bits";
import { docToMarkdown, markdownToDoc, type PMNode } from "./richtext";
import { cardYears } from "../lib/format";

type Hit = PersonSummary & { context: string };

interface PickerState {
  items: Hit[];
  query: string;
  rect: DOMRect | null;
  active: number;
  command: ((attrs: { id: string; label: string }) => void) | null;
}

export function RichEditor({
  value,
  onChange,
  placeholder,
  minHeight = 160,
  autofocus,
}: {
  value: string;
  onChange: (markdown: string) => void;
  placeholder?: string;
  minHeight?: number;
  autofocus?: boolean;
}) {
  const [picker, setPicker] = useState<PickerState | null>(null);
  const pickerRef = useRef<PickerState | null>(null);
  pickerRef.current = picker;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ heading: { levels: [3] }, code: false, codeBlock: false, horizontalRule: false, strike: false }),
      Placeholder.configure({ placeholder: placeholder ?? "Pisz tutaj… „@” wstawia osobę" }),
      Mention.configure({
        HTMLAttributes: { class: "mention" },
        renderText: ({ node }) => String(node.attrs.label ?? ""),
        renderHTML: ({ node, options }) => ["span", options.HTMLAttributes, String(node.attrs.label ?? "")],
        suggestion: {
          char: "@",
          items: async ({ query }) => {
            if (!query.trim()) return [];
            try {
              return await call<Hit[]>("people.search", { q: query, limit: 6 });
            } catch {
              return [];
            }
          },
          render: () => ({
            onStart: (props) => setPicker({ items: props.items as Hit[], query: props.query, rect: props.clientRect?.() ?? null, active: 0, command: props.command }),
            onUpdate: (props) =>
              setPicker((p) => ({ items: props.items as Hit[], query: props.query, rect: props.clientRect?.() ?? null, active: Math.min(p?.active ?? 0, Math.max((props.items as Hit[]).length - 1, 0)), command: props.command })),
            onKeyDown: ({ event }) => {
              const p = pickerRef.current;
              if (!p) return false;
              if (event.key === "ArrowDown") {
                setPicker({ ...p, active: Math.min(p.active + 1, p.items.length - 1) });
                return true;
              }
              if (event.key === "ArrowUp") {
                setPicker({ ...p, active: Math.max(p.active - 1, 0) });
                return true;
              }
              if (event.key === "Enter" && p.items[p.active]) {
                choose(p, p.items[p.active]);
                return true;
              }
              if (event.key === "Escape") {
                setPicker(null);
                return true;
              }
              return false;
            },
            onExit: () => setPicker(null),
          }),
        },
      }),
    ],
    content: markdownToDoc(value) as never,
    autofocus: autofocus ? "end" : false,
    onUpdate: ({ editor: e }) => onChangeRef.current(docToMarkdown(e.getJSON() as PMNode)),
  });

  // The mention text is the name as it will read in the sentence ("Antoniego"); it can be edited later.
  function choose(p: PickerState, hit: Hit) {
    p.command?.({ id: hit.id, label: hit.given || hit.name });
    setPicker(null);
  }

  useEffect(() => {
    if (!editor) return;
    const current = docToMarkdown(editor.getJSON() as PMNode);
    if (current !== value) editor.commands.setContent(markdownToDoc(value) as never, { emitUpdate: false });
  }, [value, editor]);

  return (
    <div className="rich-editor">
      <Toolbar editor={editor} />
      <EditorContent editor={editor} className="rich-body reading" style={{ minHeight }} />
      {picker &&
        picker.rect &&
        createPortal(
          <div className="popover mention-picker" style={{ position: "fixed", left: picker.rect.left, top: picker.rect.bottom + 6, width: 360 }}>
            {picker.items.length === 0 && <div style={{ padding: "10px 14px", fontSize: 13, color: "var(--text3)" }}>{picker.query ? "Brak takiej osoby w archiwum." : "Wpisz imię lub nazwisko…"}</div>}
            {picker.items.map((hit, i) => (
              <button
                key={hit.id}
                className={`menu-item${i === picker.active ? " active" : ""}`}
                style={{ minHeight: 48, gap: 10, boxShadow: i === picker.active ? "inset 3px 0 0 var(--accent)" : undefined, background: i === picker.active ? "var(--accent-soft)" : undefined }}
                onMouseDown={(e) => {
                  e.preventDefault();
                  choose(picker, hit);
                }}
              >
                <Avatar initials={hit.initials} branch={hit.branch} photo={hit.photo} size={30} />
                <span className="col grow" style={{ minWidth: 0 }}>
                  <span style={{ fontSize: 14, fontWeight: 500 }}>{hit.name}</span>
                  <span className="ellipsis" style={{ fontSize: 12, color: "var(--text3)" }}>
                    {hit.context}
                  </span>
                </span>
                <span className="num" style={{ fontSize: 12, color: "var(--text2)" }}>
                  {cardYears(hit.birth?.year, hit.death?.year, hit.living)}
                </span>
              </button>
            ))}
            <div className="row" style={{ height: 40, padding: "0 14px", gap: 8, borderTop: "1px solid var(--border)", fontSize: 13, color: "var(--text2)" }}>
              <UserPlus size={14} />
              <span className="grow">Osoby spoza archiwum zostaw jako zwykły tekst</span>
              <span style={{ fontSize: 11, color: "var(--text3)" }}>↑↓ Enter · Esc</span>
            </div>
          </div>,
          document.body,
        )}
    </div>
  );
}

function Toolbar({ editor }: { editor: Editor | null }) {
  if (!editor) return <div className="rich-toolbar" />;
  const tool = (title: string, icon: React.ReactNode, active: boolean, run: () => void) => (
    <button
      type="button"
      className={`icon-btn${active ? " on" : ""}`}
      title={title}
      onMouseDown={(e) => {
        e.preventDefault();
        run();
      }}
      style={active ? { background: "var(--accent-soft)", color: "var(--accent-text)" } : undefined}
    >
      {icon}
    </button>
  );
  return (
    <div className="rich-toolbar">
      {tool("Nagłówek", <Heading2 size={16} />, editor.isActive("heading"), () => editor.chain().focus().toggleHeading({ level: 3 }).run())}
      {tool("Pogrubienie", <Bold size={16} />, editor.isActive("bold"), () => editor.chain().focus().toggleBold().run())}
      {tool("Kursywa", <Italic size={16} />, editor.isActive("italic"), () => editor.chain().focus().toggleItalic().run())}
      {tool("Lista", <List size={16} />, editor.isActive("bulletList"), () => editor.chain().focus().toggleBulletList().run())}
      {tool("Lista numerowana", <ListOrdered size={16} />, editor.isActive("orderedList"), () => editor.chain().focus().toggleOrderedList().run())}
      {tool("Cytat", <Quote size={16} />, editor.isActive("blockquote"), () => editor.chain().focus().toggleBlockquote().run())}
      {tool("Link", <LinkIcon size={16} />, editor.isActive("link"), () => {
        const previous = editor.getAttributes("link").href as string | undefined;
        const url = window.prompt("Adres strony (https://…)", previous ?? "https://");
        if (url === null) return;
        if (!url.trim() || url === "https://") editor.chain().focus().unsetLink().run();
        else editor.chain().focus().extendMarkRange("link").setLink({ href: url.trim() }).run();
      })}
      <span className="grow" />
      <span style={{ fontSize: 12, color: "var(--text3)", paddingRight: 6 }}>„@” wstawia osobę</span>
    </div>
  );
}
