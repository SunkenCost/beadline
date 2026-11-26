export type InlineNode =
  | { type: "text"; text: string }
  | { type: "bold"; text: string }
  | { type: "code"; text: string }
  | { type: "link"; label: string; url: string };

export type BlockNode =
  | { type: "heading"; level: 2 | 3 | 4; inlines: InlineNode[] }
  | { type: "checklist"; checked: boolean; inlines: InlineNode[] }
  | { type: "bullet"; inlines: InlineNode[] }
  | { type: "codeblock"; lang: string; code: string }
  | { type: "paragraph"; inlines: InlineNode[] };

const INLINE_RE =
  /(`[^`\n]+`|\*\*[^*\n]+\*\*|\[([^\]\n]+)\]\(([a-z][a-z0-9+.-]{1,20}:\/\/[^\s)]+)\))/g;

export function parseInlines(text: string): InlineNode[] {
  const nodes: InlineNode[] = [];
  let lastIdx = 0;

  for (const match of text.matchAll(INLINE_RE)) {
    const idx = match.index ?? 0;
    if (idx > lastIdx) {
      nodes.push({ type: "text", text: text.slice(lastIdx, idx) });
    }
    const token = match[0];
    if (token.startsWith("`") && token.endsWith("`")) {
      nodes.push({ type: "code", text: token.slice(1, -1) });
    } else if (token.startsWith("**") && token.endsWith("**")) {
      nodes.push({ type: "bold", text: token.slice(2, -2) });
    } else if (match[2] && match[3]) {
      nodes.push({ type: "link", label: match[2], url: match[3] });
    } else {
      nodes.push({ type: "text", text: token });
    }
    lastIdx = idx + token.length;
  }

  if (lastIdx < text.length) {
    nodes.push({ type: "text", text: text.slice(lastIdx) });
  }
  return nodes;
}

export function parseMarkdownBlocks(raw: string): BlockNode[] {
  const lines = raw.replace(/\r\n/g, "\n").split("\n");
  const blocks: BlockNode[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i] ?? "";
    const trimmed = line.trim();

    if (!trimmed) {
      i += 1;
      continue;
    }

    if (trimmed.startsWith("```")) {
      const lang = trimmed.slice(3).trim();
      const codeLines: string[] = [];
      i += 1;
      while (i < lines.length && !(lines[i] ?? "").trim().startsWith("```")) {
        codeLines.push(lines[i] ?? "");
        i += 1;
      }
      if (i < lines.length) i += 1;
      blocks.push({ type: "codeblock", lang, code: codeLines.join("\n") });
      continue;
    }

    const hMatch = /^(#{2,4})\s+(.+)$/.exec(trimmed);
    if (hMatch?.[1] && hMatch[2]) {
      const level = Math.min(4, Math.max(2, hMatch[1].length)) as 2 | 3 | 4;
      blocks.push({ type: "heading", level, inlines: parseInlines(hMatch[2]) });
      i += 1;
      continue;
    }

    const chkMatch = /^[-*]\s+\[([ xX])\]\s+(.+)$/.exec(trimmed);
    if (chkMatch?.[1] && chkMatch[2]) {
      blocks.push({
        type: "checklist",
        checked: chkMatch[1].toLowerCase() === "x",
        inlines: parseInlines(chkMatch[2]),
      });
      i += 1;
      continue;
    }

    const bulMatch = /^[-*]\s+(.+)$/.exec(trimmed);
    if (bulMatch?.[1]) {
      blocks.push({ type: "bullet", inlines: parseInlines(bulMatch[1]) });
      i += 1;
      continue;
    }

    const paraLines: string[] = [line];
    i += 1;
    while (i < lines.length) {
      const nxt = lines[i] ?? "";
      const nxtTrim = nxt.trim();
      if (
        !nxtTrim ||
        nxtTrim.startsWith("```") ||
        /^#{2,4}\s+/.test(nxtTrim) ||
        /^[-*]\s+/.test(nxtTrim)
      ) {
        break;
      }
      paraLines.push(nxt);
      i += 1;
    }
    blocks.push({ type: "paragraph", inlines: parseInlines(paraLines.join("\n")) });
  }

  return blocks;
}
