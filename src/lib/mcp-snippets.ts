export type HarnessId = "claude-code" | "codex" | "cursor" | "antigravity" | "t3-cli";

export interface HarnessSnippet {
  id: HarnessId;
  name: string;
  targetFile: string;
  language: "bash" | "json" | "toml";
  code: string;
  hookTargetFile?: string;
  hookCode?: string;
  hookNote?: string;
}

function escapeTomlString(s: string): string {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function quoteShell(s: string): string {
  if (/[\s"]/.test(s)) {
    return `"${s.replace(/"/g, '\\"')}"`;
  }
  return s;
}

export function buildHarnessSnippets(
  blBinaryPath: string | null,
  repoPath: string | null,
): HarnessSnippet[] {
  const blBin = blBinaryPath?.trim() || "bl";
  const repo = repoPath?.trim() || "<path-to-your-repo>";
  const blCmd = quoteShell(blBin);

  const claudeCmd = `claude mcp add beadline -- ${blCmd} mcp --repo ${quoteShell(repo)} --harness claude-code`;
  const claudeHook = JSON.stringify(
    {
      hooks: {
        SessionStart: [{ hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] }],
        PreToolUse: [
          { matcher: "*", hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] },
        ],
        PostToolUse: [
          { matcher: "*", hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] },
        ],
        PermissionRequest: [
          { matcher: "*", hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] },
        ],
        Notification: [{ hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] }],
        Stop: [{ hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] }],
        SessionEnd: [{ hooks: [{ type: "command", command: `${blCmd} hook claude-code` }] }],
      },
    },
    null,
    2,
  );

  const codexToml = `[mcp_servers.beadline]
command = "${escapeTomlString(blBin)}"
args = ["mcp", "--repo", "${escapeTomlString(repo)}", "--harness", "codex"]`;

  const codexHook = JSON.stringify(
    {
      hooks: {
        PermissionRequest: [
          { matcher: "*", hooks: [{ type: "command", command: `${blCmd} hook codex` }] },
        ],
        PostToolUse: [
          { matcher: "*", hooks: [{ type: "command", command: `${blCmd} hook codex` }] },
        ],
        Stop: [{ hooks: [{ type: "command", command: `${blCmd} hook codex` }] }],
      },
    },
    null,
    2,
  );

  const cursorJson = JSON.stringify(
    {
      mcpServers: {
        beadline: {
          command: blBin,
          args: ["mcp", "--repo", repo, "--harness", "cursor"],
        },
      },
    },
    null,
    2,
  );

  const cursorHook = JSON.stringify(
    {
      version: 1,
      hooks: {
        sessionStart: [{ type: "command", command: `${blCmd} hook cursor --event sessionStart` }],
        preToolUse: [{ type: "command", command: `${blCmd} hook cursor --event preToolUse` }],
        postToolUse: [{ type: "command", command: `${blCmd} hook cursor --event postToolUse` }],
        afterFileEdit: [{ type: "command", command: `${blCmd} hook cursor --event afterFileEdit` }],
        stop: [{ type: "command", command: `${blCmd} hook cursor --event stop` }],
        sessionEnd: [{ type: "command", command: `${blCmd} hook cursor --event sessionEnd` }],
      },
    },
    null,
    2,
  );

  const antigravityJson = JSON.stringify(
    {
      mcpServers: {
        beadline: {
          command: blBin,
          args: ["mcp", "--repo", repo, "--harness", "antigravity"],
        },
      },
    },
    null,
    2,
  );

  const antigravityHook = JSON.stringify(
    {
      hooks: {
        SessionStart: [{ type: "command", command: `${blCmd} hook antigravity` }],
        PreToolUse: [{ type: "command", command: `${blCmd} hook antigravity` }],
        PostToolUse: [{ type: "command", command: `${blCmd} hook antigravity` }],
        Stop: [{ type: "command", command: `${blCmd} hook antigravity` }],
      },
    },
    null,
    2,
  );

  const cliFlow = `# Run from any Git worktree of this repo
${blCmd} ready
${blCmd} claim <issue-id> --summary "implementing step 1" --scope "src/auth/**"
${blCmd} pulse --state waiting_human --waiting-reason "Confirm token TTL"
${blCmd} finish <issue-id> --summary "all unit tests green"`;

  return [
    {
      id: "claude-code",
      name: "Claude Code",
      targetFile: "CLI command (MCP)",
      language: "bash",
      code: claudeCmd,
      hookTargetFile: "~/.claude/settings.json",
      hookCode: claudeHook,
    },
    {
      id: "codex",
      name: "Codex CLI",
      targetFile: "~/.codex/config.toml (MCP)",
      language: "toml",
      code: codexToml,
      hookTargetFile: "~/.codex/hooks.json",
      hookCode: codexHook,
      hookNote: "codex-trust",
    },
    {
      id: "cursor",
      name: "Cursor",
      targetFile: ".cursor/mcp.json (MCP)",
      language: "json",
      code: cursorJson,
      hookTargetFile: "~/.cursor/hooks.json",
      hookCode: cursorHook,
    },
    {
      id: "antigravity",
      name: "Antigravity",
      targetFile: "mcp_config.json (MCP)",
      language: "json",
      code: antigravityJson,
      hookTargetFile: "~/.gemini/config/hooks.json",
      hookCode: antigravityHook,
    },
    {
      id: "t3-cli",
      name: "T3 Code / Universal CLI",
      targetFile: "Terminal / Handoff Prompt",
      language: "bash",
      code: cliFlow,
    },
  ];
}
