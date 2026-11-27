import { describe, expect, it } from "vitest";
import { buildHarnessSnippets } from "./mcp-snippets";

describe("buildHarnessSnippets", () => {
  it("uses absolute bl binary path and generates both MCP and native Hook configs", () => {
    const snippets = buildHarnessSnippets("C:\\Tools\\bl.exe", "Y:\\Work\\demo-repo");
    expect(snippets).toHaveLength(5);

    const claude = snippets.find((s) => s.id === "claude-code");
    expect(claude?.code).toContain("C:\\Tools\\bl.exe");
    expect(claude?.code).toContain("Y:\\Work\\demo-repo");
    expect(claude?.hookTargetFile).toBe("~/.claude/settings.json");
    const claudeHooks = JSON.parse(claude?.hookCode ?? "{}");
    expect(claudeHooks.hooks.PermissionRequest).toBeTruthy();
    expect(claudeHooks.hooks.SessionStart[0].hooks[0].command).toContain("hook claude-code");

    const codex = snippets.find((s) => s.id === "codex");
    expect(codex?.code).toContain("[mcp_servers.beadline]");
    expect(codex?.code).toContain("C:\\\\Tools\\\\bl.exe");
    const codexHooks = JSON.parse(codex?.hookCode ?? "{}");
    expect(codexHooks.hooks.PermissionRequest).toBeTruthy();

    const cursor = snippets.find((s) => s.id === "cursor");
    const parsed = JSON.parse(cursor?.code ?? "{}");
    expect(parsed.mcpServers.beadline.command).toBe("C:\\Tools\\bl.exe");
    expect(cursor?.hookTargetFile).toBe("~/.cursor/hooks.json");
  });
});
