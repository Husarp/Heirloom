// Our texts are simple Markdown with person mentions (`[Antoniego](person:@I12@)`); the editor (TipTap) works on a
// document tree. These two functions translate between them, for exactly the subset the app uses: paragraphs,
// ### headings, bullet and numbered lists, quotes, **bold**, *italic*, links and mentions.

export interface PMNode {
  type: string;
  attrs?: Record<string, unknown>;
  content?: PMNode[];
  text?: string;
  marks?: { type: string; attrs?: Record<string, unknown> }[];
}

function inlineNodes(text: string): PMNode[] {
  const out: PMNode[] = [];
  const pattern = /\[([^\]]+)\]\(([^)]+)\)|\*\*([^*]+)\*\*|\*([^*]+)\*|_([^_]+)_/g;
  let last = 0;
  let match: RegExpExecArray | null;
  const push = (t: string, marks?: PMNode["marks"]) => {
    if (t) out.push(marks ? { type: "text", text: t, marks } : { type: "text", text: t });
  };
  while ((match = pattern.exec(text))) {
    push(text.slice(last, match.index));
    if (match[1] != null) {
      const target = match[2].trim();
      if (target.startsWith("person:")) {
        out.push({ type: "mention", attrs: { id: target.slice(7), label: match[1] } });
      } else {
        push(match[1], [{ type: "link", attrs: { href: target } }]);
      }
    } else if (match[3] != null) {
      push(match[3], [{ type: "bold" }]);
    } else {
      push(match[4] ?? match[5], [{ type: "italic" }]);
    }
    last = pattern.lastIndex;
  }
  push(text.slice(last));
  return out;
}

function paragraph(text: string): PMNode {
  const content = inlineNodes(text);
  return content.length ? { type: "paragraph", content } : { type: "paragraph" };
}

export function markdownToDoc(markdown: string): PMNode {
  const blocks = markdown.replace(/\r\n/g, "\n").split(/\n{2,}/).filter((b) => b.trim());
  const content: PMNode[] = [];
  for (const block of blocks) {
    const lines = block.split("\n");
    if (lines.every((l) => /^\s*[-*]\s+/.test(l))) {
      content.push({ type: "bulletList", content: lines.map((l) => ({ type: "listItem", content: [paragraph(l.replace(/^\s*[-*]\s+/, ""))] })) });
    } else if (lines.every((l) => /^\s*\d+\.\s+/.test(l))) {
      content.push({ type: "orderedList", content: lines.map((l) => ({ type: "listItem", content: [paragraph(l.replace(/^\s*\d+\.\s+/, ""))] })) });
    } else if (lines.every((l) => l.startsWith(">"))) {
      content.push({ type: "blockquote", content: [paragraph(lines.map((l) => l.replace(/^>\s?/, "")).join(" "))] });
    } else if (/^#{1,4}\s/.test(lines[0])) {
      content.push({ type: "heading", attrs: { level: 3 }, content: inlineNodes(lines[0].replace(/^#{1,4}\s/, "")) });
      if (lines.length > 1) content.push(paragraph(lines.slice(1).join("\n")));
    } else {
      const inline: PMNode[] = [];
      lines.forEach((l, i) => {
        if (i > 0) inline.push({ type: "hardBreak" });
        inline.push(...inlineNodes(l));
      });
      content.push(inline.length ? { type: "paragraph", content: inline } : { type: "paragraph" });
    }
  }
  return { type: "doc", content: content.length ? content : [{ type: "paragraph" }] };
}

function inlineToMarkdown(nodes: PMNode[] | undefined): string {
  let out = "";
  for (const node of nodes ?? []) {
    if (node.type === "mention") {
      out += `[${String(node.attrs?.label ?? "")}](person:${String(node.attrs?.id ?? "")})`;
    } else if (node.type === "hardBreak") {
      out += "\n";
    } else if (node.type === "text") {
      let text = node.text ?? "";
      const marks = node.marks ?? [];
      const link = marks.find((m) => m.type === "link");
      if (marks.some((m) => m.type === "italic")) text = `*${text}*`;
      if (marks.some((m) => m.type === "bold")) text = `**${text}**`;
      if (link) text = `[${text}](${String(link.attrs?.href ?? "")})`;
      out += text;
    }
  }
  return out;
}

export function docToMarkdown(doc: PMNode): string {
  const blocks: string[] = [];
  for (const node of doc.content ?? []) {
    switch (node.type) {
      case "heading":
        blocks.push(`### ${inlineToMarkdown(node.content)}`);
        break;
      case "bulletList":
        blocks.push((node.content ?? []).map((item) => `- ${(item.content ?? []).map((p) => inlineToMarkdown(p.content)).join(" ")}`).join("\n"));
        break;
      case "orderedList":
        blocks.push((node.content ?? []).map((item, i) => `${i + 1}. ${(item.content ?? []).map((p) => inlineToMarkdown(p.content)).join(" ")}`).join("\n"));
        break;
      case "blockquote":
        blocks.push((node.content ?? []).map((p) => `> ${inlineToMarkdown(p.content)}`).join("\n"));
        break;
      default: {
        const text = inlineToMarkdown(node.content);
        if (text.trim()) blocks.push(text);
      }
    }
  }
  return blocks.join("\n\n");
}
