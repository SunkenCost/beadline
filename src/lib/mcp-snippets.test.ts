import { describe, expect, it } from "vitest";
import { buildHarnessSnippets } from "./mcp-snippets";

describe("buildHarnessSnippets", () => {
  it("omits hardcoded --repo from global MCP configs and uses safe non-gating hooks", () => {
    const snippets = buildHarnessSnippets("C:\\Tools\\bl.exe", "Y:\\Work\\demo-repo", "windows");
    expect(snippets).toHaveLength(5);

    const claude = snippets.find((s) => s.id === "claude-code");
    expect(claude?.code).toContain("C:/Tools/bl.exe mcp --harness claude-code");
    expect(claude?.code).not.toContain("--repo");
    expect(claude?.hookTargetFile).toBe("~/.claude/settings.json");
    const claudeHooks = JSON.parse(claude?.hookCode ?? "{}");
    expect(claudeHooks.hooks.PermissionRequest).toBeTruthy();
    expect(claudeHooks.hooks.SessionStart[0].hooks[0].command).toBe(
      "C:/Tools/bl.exe hook claude-code",
    );

    const codex = snippets.find((s) => s.id === "codex");
    expect(codex?.code).toContain("[mcp_servers.beadline]");
    expect(codex?.code).toContain("C:\\\\Tools\\\\bl.exe");
    expect(codex?.code).not.toContain("--repo");
    const codexHooks = JSON.parse(codex?.hookCode ?? "{}");
    expect(codexHooks.hooks.SessionStart).toBeTruthy();
    expect(codexHooks.hooks.PreToolUse).toBeTruthy();
    expect(codexHooks.hooks.PermissionRequest).toBeTruthy();
    expect(codexHooks.hooks.PostToolUse).toBeTruthy();
    expect(codexHooks.hooks.Stop).toBeTruthy();

    // Cursor MUST NOT register preToolUse (permission decision gate where empty stdout blocks tools)
    const cursor = snippets.find((s) => s.id === "cursor");
    const cursorMcp = JSON.parse(cursor?.code ?? "{}");
    expect(cursorMcp.mcpServers.beadline.command).toBe("C:\\Tools\\bl.exe");
    expect(cursorMcp.mcpServers.beadline.args).toEqual(["mcp", "--harness", "cursor"]);
    expect(cursor?.hookTargetFile).toBe("~/.cursor/hooks.json");
    const cursorHooks = JSON.parse(cursor?.hookCode ?? "{}");
    expect(cursorHooks.hooks.preToolUse).toBeUndefined();
    expect(cursorHooks.hooks.sessionStart[0].command).toContain("--event sessionStart");
    expect(cursorHooks.hooks.postToolUse[0].command).toContain("--event postToolUse");

    // Antigravity uses official top-level named hook schema with explicit --event and grouped PostToolUse
    const agy = snippets.find((s) => s.id === "antigravity");
    const agyMcp = JSON.parse(agy?.code ?? "{}");
    expect(agyMcp.mcpServers.beadline.args).toEqual(["mcp", "--harness", "antigravity"]);
    const agyHooks = JSON.parse(agy?.hookCode ?? "{}");
    expect(agyHooks.beadline).toBeTruthy();
    expect(agyHooks.beadline.PreToolUse).toBeUndefined();
    expect(agyHooks.beadline.SessionStart[0].command).toContain("--event SessionStart");
    expect(agyHooks.beadline.PreInvocation[0].command).toContain("--event PreInvocation");
    expect(agyHooks.beadline.PostToolUse[0].matcher).toBe("*");
    expect(agyHooks.beadline.PostToolUse[0].hooks[0].command).toContain("--event PostToolUse");
    expect(agyHooks.beadline.Stop[0].command).toContain("--event Stop");
  });

  it("safely quotes Windows paths containing spaces and $ across Git Bash, PowerShell, and cmd", () => {
    const trickyBin = "C:\\Program Files\\$dev\\bl.exe";
    const snippets = buildHarnessSnippets(trickyBin, "Y:\\Work\\demo-repo", "windows");

    const claude = snippets.find((s) => s.id === "claude-code");
    const claudeHooks = JSON.parse(claude?.hookCode ?? "{}");
    // Git Bash: forward slashes + single quotes prevent backslash loss AND $dev expansion
    expect(claudeHooks.hooks.SessionStart[0].hooks[0].command).toBe(
      "'C:/Program Files/$dev/bl.exe' hook claude-code",
    );

    const codex = snippets.find((s) => s.id === "codex");
    const codexHooks = JSON.parse(codex?.hookCode ?? "{}");
    // PowerShell: call operator `& '...'` avoids `Unexpected token 'hook'` AND $dev expansion
    expect(codexHooks.hooks.SessionStart[0].hooks[0].command).toBe(
      "& 'C:\\Program Files\\$dev\\bl.exe' hook codex",
    );

    const agy = snippets.find((s) => s.id === "antigravity");
    const agyHooks = JSON.parse(agy?.hookCode ?? "{}");
    // cmd /c: double quotes preserve spaces and literal $
    expect(agyHooks.beadline.SessionStart[0].command).toBe(
      '"C:\\Program Files\\$dev\\bl.exe" hook antigravity --event SessionStart',
    );
  });
});
