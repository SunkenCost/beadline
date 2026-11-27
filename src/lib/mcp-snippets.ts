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

function isWindowsContext(binPath: string, platformOs?: string): boolean {
  if (platformOs === "windows") return true;
  if (platformOs === "macos" || platformOs === "linux") return false;
  return /^[A-Za-z]:[\\/]/.test(binPath) || binPath.startsWith("\\\\");
}

/**
 * Formats an executable path for POSIX shells (`sh`, `bash`, and Git Bash on Windows).
 * Converts Windows drive backslashes to forward slashes so Git Bash never strips them,
 * and uses single quotes whenever spaces or shell metacharacters (including `$`) appear.
 */
function quotePosixExe(exe: string): string {
  const normalized = /^[A-Za-z]:\\/.test(exe) ? exe.replace(/\\/g, "/") : exe;
  if (/[\s"'$`\\;&|<>(){}*?#~!]/.test(normalized)) {
    return `'${normalized.replace(/'/g, `'\\''`)}'`;
  }
  return normalized;
}

/**
 * Formats an executable + arguments for Windows `cmd /c` (or POSIX `sh -c` on Unix).
 * Note: `cmd.exe` does not expand `$`, so double quotes safely preserve spaces and `$`.
 */
function formatCmdOrPosix(exe: string, args: string, isWindows: boolean): string {
  if (!isWindows) {
    return `${quotePosixExe(exe)} ${args}`;
  }
  if (/[\s"&|<>^()%!]/.test(exe)) {
    return `"${exe.replace(/"/g, '""')}" ${args}`;
  }
  return `${exe} ${args}`;
}

/**
 * Formats an executable + arguments for Windows PowerShell (or POSIX `sh -c` on Unix).
 * PowerShell requires the call operator `& '...'` when invoking a quoted executable path;
 * single quotes keep spaces and `$` literal without variable interpolation.
 */
function formatPowerShellOrPosix(exe: string, args: string, isWindows: boolean): string {
  if (!isWindows) {
    return `${quotePosixExe(exe)} ${args}`;
  }
  if (/[\s"'$`&|;(){}@#]/.test(exe)) {
    return `& '${exe.replace(/'/g, "''")}' ${args}`;
  }
  return `${exe} ${args}`;
}

export function buildHarnessSnippets(
  blBinaryPath: string | null,
  repoPath: string | null,
  platformOs?: string,
): HarnessSnippet[] {
  const blBin = blBinaryPath?.trim() || "bl";
  const isWin = isWindowsContext(blBin, platformOs);

  // Claude Code runs hook commands in bash / Git Bash on Windows.
  const blPosix = quotePosixExe(blBin);
  const claudeHookCmd = `${blPosix} hook claude-code`;
  // Global MCP configs intentionally omit `--repo` so `bl mcp` resolves the live
  // worktree & branch from each agent session's actual working directory (`cwd`).
  const claudeCmd = `claude mcp add beadline -- ${blPosix} mcp --harness claude-code`;
  const claudeHook = JSON.stringify(
    {
      hooks: {
        SessionStart: [{ hooks: [{ type: "command", command: claudeHookCmd }] }],
        PreToolUse: [{ matcher: "*", hooks: [{ type: "command", command: claudeHookCmd }] }],
        PostToolUse: [{ matcher: "*", hooks: [{ type: "command", command: claudeHookCmd }] }],
        PermissionRequest: [{ matcher: "*", hooks: [{ type: "command", command: claudeHookCmd }] }],
        Notification: [{ hooks: [{ type: "command", command: claudeHookCmd }] }],
        Stop: [{ hooks: [{ type: "command", command: claudeHookCmd }] }],
        SessionEnd: [{ hooks: [{ type: "command", command: claudeHookCmd }] }],
      },
    },
    null,
    2,
  );

  // Codex MCP uses native argv spawning (no shell); hooks run via PowerShell on Windows / sh on Unix.
  const codexToml = `[mcp_servers.beadline]
command = "${escapeTomlString(blBin)}"
args = ["mcp", "--harness", "codex"]`;

  const codexHookCmd = formatPowerShellOrPosix(blBin, "hook codex", isWin);
  const codexHook = JSON.stringify(
    {
      hooks: {
        SessionStart: [{ hooks: [{ type: "command", command: codexHookCmd }] }],
        PreToolUse: [{ matcher: "*", hooks: [{ type: "command", command: codexHookCmd }] }],
        PermissionRequest: [{ matcher: "*", hooks: [{ type: "command", command: codexHookCmd }] }],
        PostToolUse: [{ matcher: "*", hooks: [{ type: "command", command: codexHookCmd }] }],
        Stop: [{ hooks: [{ type: "command", command: codexHookCmd }] }],
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
          args: ["mcp", "--harness", "cursor"],
        },
      },
    },
    null,
    2,
  );

  // Cursor's `preToolUse` is a permission-gating hook where empty stdout blocks tool calls.
  // Register only passive, non-blocking observation hooks so telemetry can never block Cursor.
  const cursorHook = JSON.stringify(
    {
      version: 1,
      hooks: {
        sessionStart: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook cursor --event sessionStart", isWin),
          },
        ],
        postToolUse: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook cursor --event postToolUse", isWin),
          },
        ],
        afterFileEdit: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook cursor --event afterFileEdit", isWin),
          },
        ],
        stop: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook cursor --event stop", isWin),
          },
        ],
        sessionEnd: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook cursor --event sessionEnd", isWin),
          },
        ],
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
          args: ["mcp", "--harness", "antigravity"],
        },
      },
    },
    null,
    2,
  );

  // Antigravity `hooks.json` uses top-level named hook keys, flat arrays for session/turn events,
  // grouped `{ matcher, hooks }` for tool events, and runs commands via `cmd /c` on Windows / `sh -c` on Unix.
  // Omit `PreToolUse` (permission gate) and pass explicit `--event` so all Antigravity payload variants match.
  const antigravityHook = JSON.stringify(
    {
      beadline: {
        SessionStart: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook antigravity --event SessionStart", isWin),
          },
        ],
        PreInvocation: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook antigravity --event PreInvocation", isWin),
          },
        ],
        PostToolUse: [
          {
            matcher: "*",
            hooks: [
              {
                type: "command",
                command: formatCmdOrPosix(blBin, "hook antigravity --event PostToolUse", isWin),
              },
            ],
          },
        ],
        Stop: [
          {
            type: "command",
            command: formatCmdOrPosix(blBin, "hook antigravity --event Stop", isWin),
          },
        ],
      },
    },
    null,
    2,
  );

  const repoComment = repoPath?.trim()
    ? `# Run from any Git worktree (e.g. ${repoPath.trim()})`
    : "# Run from any Git worktree of this repo";
  const cliFlow = `${repoComment}
${formatPowerShellOrPosix(blBin, "ready", isWin)}
${formatPowerShellOrPosix(blBin, 'claim <issue-id> --summary "implementing step 1" --scope "src/auth/**"', isWin)}
${formatPowerShellOrPosix(blBin, 'pulse --state waiting_human --waiting-reason "Confirm token TTL"', isWin)}
${formatPowerShellOrPosix(blBin, 'finish <issue-id> --summary "all unit tests green"', isWin)}`;

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
