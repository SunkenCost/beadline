import { describe, expect, it } from "vitest";
import { buildHarnessSnippets } from "./mcp-snippets";

describe("buildHarnessSnippets", () => {
  it("uses absolute bl binary path and active repo across all harnesses", () => {
    const snippets = buildHarnessSnippets("C:\\Tools\\bl.exe", "Y:\\Work\\demo-repo");
    expect(snippets).toHaveLength(5);

    const claude = snippets.find((s) => s.id === "claude-code");
    expect(claude?.code).toContain("C:\\Tools\\bl.exe");
    expect(claude?.code).toContain("Y:\\Work\\demo-repo");
    expect(claude?.code).toContain("--harness claude-code");

    const codex = snippets.find((s) => s.id === "codex");
    expect(codex?.code).toContain("[mcp_servers.beadline]");
    expect(codex?.code).toContain("C:\\\\Tools\\\\bl.exe");

    const cursor = snippets.find((s) => s.id === "cursor");
    const parsed = JSON.parse(cursor?.code ?? "{}");
    expect(parsed.mcpServers.beadline.command).toBe("C:\\Tools\\bl.exe");
    expect(parsed.mcpServers.beadline.args).toEqual([
      "mcp",
      "--repo",
      "Y:\\Work\\demo-repo",
      "--harness",
      "cursor",
    ]);
  });
});
