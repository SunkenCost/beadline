import { describe, expect, it } from "vitest";
import { parseInlines, parseMarkdownBlocks } from "./markdown";

describe("safe markdown parser", () => {
  it("parses headings, checklists, bullets, codeblocks, and paragraphs", () => {
    const md = [
      "## Summary",
      "- [x] Ship **optimistic** UI",
      "- [ ] Wire `bd export` fallback",
      "- Plain bullet",
      "```bash",
      "bd close bl-1 --force",
      "```",
      "See [Thread](t3-thread://v2/env/abc) and <script>alert(1)</script>.",
    ].join("\n");

    const blocks = parseMarkdownBlocks(md);
    expect(blocks.map((b) => b.type)).toEqual([
      "heading",
      "checklist",
      "checklist",
      "bullet",
      "codeblock",
      "paragraph",
    ]);

    const firstCheck = blocks[1];
    expect(firstCheck?.type === "checklist" && firstCheck.checked).toBe(true);

    const para = blocks[5];
    expect(para?.type).toBe("paragraph");
    if (para?.type === "paragraph") {
      expect(
        para.inlines.some((n) => n.type === "link" && n.url === "t3-thread://v2/env/abc"),
      ).toBe(true);
      // Raw HTML stays harmless text
      expect(para.inlines.some((n) => n.type === "text" && n.text.includes("<script>"))).toBe(true);
    }
  });

  it("handles inline bold, code, and links cleanly", () => {
    const inlines = parseInlines(
      "Run `pnpm test` on **main** via [GitHub](https://github.com/a/b)",
    );
    expect(inlines.map((n) => n.type)).toEqual(["text", "code", "text", "bold", "text", "link"]);
  });
});
