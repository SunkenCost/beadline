export type HarnessId = "claude-code" | "codex" | "cursor" | "antigravity" | "t3-cli";

export interface HarnessSnippet {
  id: HarnessId;
  name: string;
  targetFile: string;
  language: "bash" | "json" | "toml";
  code: string;
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

  const claudeCmd = `claude mcp add beadline -- ${quoteShell(blBin)} mcp --repo ${quoteShell(repo)} --harness claude-code`;

  const codexToml = `[mcp_servers.beadline]
command = "${escapeTomlString(blBin)}"
args = ["mcp", "--repo", "${escapeTomlString(repo)}", "--harness", "codex"]`;

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

  const cliFlow = `# Run from any Git worktree of this repo
${quoteShell(blBin)} ready
${quoteShell(blBin)} claim <issue-id> --summary "implementing step 1" --scope "src/auth/**"
${quoteShell(blBin)} pulse --state waiting_human --waiting-reason "Confirm token TTL"
${quoteShell(blBin)} finish <issue-id> --summary "all unit tests green"`;

  return [
    {
      id: "claude-code",
      name: "Claude Code",
      targetFile: "CLI command",
      language: "bash",
      code: claudeCmd,
    },
    {
      id: "codex",
      name: "Codex CLI",
      targetFile: "~/.codex/config.toml",
      language: "toml",
      code: codexToml,
    },
    {
      id: "cursor",
      name: "Cursor",
      targetFile: ".cursor/mcp.json",
      language: "json",
      code: cursorJson,
    },
    {
      id: "antigravity",
      name: "Antigravity",
      targetFile: "mcp_config.json",
      language: "json",
      code: antigravityJson,
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
