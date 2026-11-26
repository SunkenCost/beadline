<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline icon" />
  <h1>Beadline</h1>
  <p><strong>Local-first multi-agent Mission Control, task &amp; dependency board, and ultra-fast CLI / MCP engine.</strong></p>
  <p>Keep full visibility when running multiple AI coding agents in parallel across Claude Code, Codex, Cursor, Antigravity, and T3 Code — with zero external database daemons.</p>
  <p>
    <strong>English</strong> · <a href="./README.zh-CN.md">简体中文</a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-555" alt="Platforms" />
    <img src="https://img.shields.io/badge/Built%20with-Rust%20%2B%20Tauri%20v2%20%2B%20React%2019-D97757" alt="Rust + Tauri v2 + React 19" />
    <img src="https://img.shields.io/badge/License-MIT-46B574" alt="MIT License" />
  </p>
</div>

---

## What Is Beadline For?

When you run multiple AI coding agents concurrently across worktrees or repositories, it is easy to lose track of what is actually happening:

- **Silent file collisions** — two agents editing the same files or overlapping modules across separate worktrees.
- **Hidden waits & stalls** — not knowing whether an agent is making progress, stalled, or waiting for your input.
- **Lost task context & blocking chains** — tasks buried in long chat transcripts without a clear view of which single blocker unlocks everything downstream.
- **Heavy external CLI databases** — external `bd` / Dolt binaries (`~150 MB`) add startup overhead, lock contention, and branch divergence across Git worktrees.

**Beadline unifies the entire local stack into one lightweight Rust workspace (`beadline-core` + `bl` CLI/MCP + `beadline` Desktop):**

1. **Mission Control ("战情室")** — answers 4 questions in 3 seconds: *What needs my decision right now? Who is doing what (+ unattributed dirty worktrees)? Where are edits or declared scopes overlapping? Which single root blocker should I unblock first?*
2. **Universal Harness Engine (`bl` + `bl mcp`)** — a standalone `~1.6 MB` native Rust binary exposing **5 lean MCP tools** (`bl_ready`, `bl_show`, `bl_claim`, `bl_pulse`, `bl_finish`) plus a full CLI so Claude Code, Codex CLI, Cursor, Antigravity, T3 Code, and any terminal agent coordinate live.
3. **One Truth Across All Git Worktrees** — stores authoritative tasks in `<git-common-dir>/beadline/issues.jsonl` (OS file-locked, atomic replace, zero `git status` pollution) and per-session leases in `<git-common-dir>/beadline/runtime/pulses/<session-id>.json` so 5–10 parallel agents never fight over one heartbeat file.
4. **Beads (`bd`) Interop** — imports/exports `.beads/issues.jsonl` (`bl import --from` / `bl export --format bd-v1` with drift checks) and keeps legacy Dolt `.beads/` workspaces readable.

## Features

| | What you get |
|---|---|
| **Mission Control & Conflict Radar** | Evidence-graded 3-tier overlap radar (**same dirty file** > **dirty file hits declared scope** > **declared scope overlap**), passive `git --no-optional-locks status` worktree observation, unattributed edit alerts, and actionable root blocker ranking. |
| **Native `bl` CLI & `bl mcp` Server** | Standalone `~1.6 MB` Rust binary. Five minimal MCP tools (`bl_ready`, `bl_show`, `bl_claim`, `bl_pulse`, `bl_finish`) with idempotent `operationId`, optimistic `rev` checks, structured delivery proof, and 1-click config snippets in Settings. |
| **6 Focused Views** | **Mission Control**, **Kanban Board** (with single highest-priority runtime badge per card), **Epics**, interactive **Dependency Graph (DAG)**, **All Issues**, and cross-repo **All Active**. |
| **Cross-Worktree Shared Store** | All worktrees of a Git repo share one OS-locked store in `<git-common-dir>/beadline/` with safe atomic replacement (never deletes target before commit). |
| **AI Handoff & Context** | One-click **Copy Agent Prompt** embedding why/acceptance/scope plus `bl_claim` / `bl_pulse` / `bl_finish` protocol instructions; auto-detects branches, commits, PRs, worktrees, and deep links. |
| **Calm Themes & Bilingual UI** | **Warm Clay**, **Cool Slate**, and **Pure Mono** presets (`Light` / `Dark` / `System`), Theme JSON import/export, and instant **English / 简体中文** switching. |

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `M` | Mission Control |
| `0` – `4` | Switch views (`All Active`, `Board`, `Epics`, `DAG`, `List`) |
| `Ctrl+K` / `⌘K` | Open command palette |
| `Ctrl+,` / `⌘,` | Open settings (Appearance, Engine, AI MCP/CLI Setup) |
| `C` | New issue |
| `/` | Focus search filter |
| `Esc` | Close drawer or modal |

## Quick Start

**Requirements:** Node.js 20+, `pnpm`, and [Rust 1.89+](https://rustup.rs/).

```bash
git clone https://github.com/SunkenCost/beadline.git
cd beadline
pnpm install

# Build standalone `bl` CLI + MCP binary (target/release/bl[.exe])
cargo build --release -p beadline-cli

# Run desktop app in development mode
pnpm tauri dev

# Build release desktop executable
pnpm tauri build
```

### Connect Your AI Harness

Open **Settings → AI Setup (MCP / CLI)** inside Beadline to copy ready-to-paste absolute-path configs for **Claude Code**, **Codex CLI**, **Cursor**, **Antigravity**, or **T3 Code / Universal CLI**, or run directly:

```bash
bl init
bl doctor
bl ready
bl claim <issue-id> --summary "implementing auth" --scope "src/auth/**"
bl finish <issue-id> --summary "all unit tests green"
```

## Workspace Architecture

```text
Cargo Workspace
├── crates/beadline-core   # RepoLocator, OS-locked JSONL store, per-session leases, Git worktree conflict radar
├── crates/beadline-cli    # Standalone `bl` CLI & `bl mcp` stdio JSON-RPC 2.0 server (~1.6 MB)
└── src-tauri              # Tauri v2 desktop shell + dual-channel fs watcher (Board snapshot vs Radar telemetry)
```

## Development

```bash
pnpm typecheck && pnpm lint && pnpm test
cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## License

[MIT](./LICENSE)
