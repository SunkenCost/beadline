<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline icon" />
  <h1>Beadline</h1>
  <p><strong>Local-first desktop task &amp; dependency board for AI-assisted coding.</strong></p>
  <p>Turn local Git project folders into a clear visual workbench for humans, while staying seamlessly compatible with <a href="https://github.com/steveyegge/beads">Beads (<code>bd</code>)</a> and AI coding agents.</p>
  <p>
    <strong>English</strong> · <a href="./README.zh-CN.md">简体中文</a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-555" alt="Platforms" />
    <img src="https://img.shields.io/badge/Built%20with-Tauri%20v2%20%2B%20React%2019-D97757" alt="Tauri v2 + React 19" />
    <img src="https://img.shields.io/badge/License-MIT-46B574" alt="MIT License" />
  </p>
</div>

---

## What Is Beadline For?

When you build software with AI coding agents (Claude Code, Codex, Cursor, T3 Code, etc.) across multiple threads or repositories, work quickly gets hard to follow:

- **Tasks get buried in chat history** — after a few rounds, it's hard to tell what's done, what's running, and what's blocked.
- **Cloud trackers (Jira / Linear / GitHub Projects) add friction** — they live outside your local repo and slow down agent handoffs.
- **CLI issue trackers like [Beads (`bd`)](https://github.com/steveyegge/beads) are great for agents, hard for humans to scan** — reading 40 nested subtasks in a terminal gets tiring fast.

**Beadline bridges humans and AI coding agents in one lightweight local desktop app:**

1. **Plan & visualize locally** — break goals into epics, subtasks, and blocking dependencies across multiple Git repositories.
2. **Watch agents work in real time** — as agents create, update, or close `.beads/` issues from the terminal, your board, epic progress, and dependency graph update live.
3. **Hand off tasks in one click** — copy a structured prompt (goal context, acceptance criteria, active blockers, Git branch) straight into your next AI session.
4. **Works standalone, zero setup** — includes a built-in Rust JSONL engine (`~5 MB` binary, `~90 MB` RAM on Windows), so you don't even need `bd` installed to get started, while staying 100% compatible with existing `bd` / Dolt repos.

## Features

| | What you get |
|---|---|
| **Built-In Engine + `bd` Dual Mode** | Works standalone with Beadline's built-in Rust JSONL engine (`<2ms` atomic writes, cycle-checked dependencies, comments). Automatically syncs with Dolt-backed repos when `bd` is installed — or download official `bd` in one click inside Settings. |
| **5 Focused Views** | **Kanban Board** (flat or grouped by epic), **Epics** progress, interactive **Dependency Graph (DAG)**, filterable **All Issues** table, and **All Active** across every connected repo. |
| **Multi-Repo & Modules** | Switch between local repos instantly. Slice tasks by module labels (`mod:*`, or custom prefixes like `area:` / `pkg:`) with custom display names and color dots. |
| **AI Agent Handoff Tools** | One-click **Copy Agent Prompt**. Auto-detects Git branches, commit SHAs, GitHub `#issues`, local worktree folders, and deep links (`t3-thread://`, `vscode://`, `cursor://`). |
| **Readable Task Context** | Parent epic summaries and close reasons appear at the top of the drawer, with a toggle to fold completed AI iteration rounds on the board. |
| **Themes & Bilingual UI** | Built-in **Warm Clay**, **Cool Slate**, and **Pure Mono** presets (`Light` / `Dark` / `System`), accent/radius/density controls, Theme JSON import/export, and instant **English / 简体中文** switching. |
| **Zero Repo Pollution** | App settings, workspace list, and module aliases stay strictly in your OS config folder — nothing extra is ever written to your Git repositories. |

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+K` / `⌘K` | Open command palette |
| `Ctrl+,` / `⌘,` | Open settings |
| `C` | New issue |
| `/` | Focus search filter |
| `0` – `4` | Switch views (`All Active`, `Board`, `Epics`, `DAG`, `List`) |
| `Esc` | Close drawer or modal |

## Quick Start

**Requirements:** Node.js 20+, `pnpm`, and [Rust](https://rustup.rs/). ([`bd` CLI](https://github.com/steveyegge/beads) is optional — only needed for Dolt repos or GitHub sync.)

```bash
git clone https://github.com/SunkenCost/beadline.git
cd beadline
pnpm install

# Run in development mode
pnpm tauri dev

# Build release executable / installer
pnpm tauri build
```

## How It Works

```text
Local Git Repos (.beads/)
   │
   ├── Native Mode ──▶ Built-in Rust JSONL engine (<2ms atomic writes, zero external binary, bd-import compatible)
   ├── Dolt Mode   ──▶ Rust `notify` watcher + direct `issues.jsonl` reads + serialized `bd` CLI write queue
   └── App Config  ──▶ Stored strictly in OS config dir (e.g. %APPDATA%\dev.beadline.desktop\)
```

- **No lock contention:** Reads parse `issues.jsonl` directly with content-based fingerprinting, so browsing never fights concurrent AI agents running `bd`.
- **Safe initialization & sync:** Non-invasive init writes `.beads/` to `.git/info/exclude` (no Git hooks, `AGENTS.md` untouched) and supports one-click `bd github sync` on Dolt repos.

## Development

```bash
pnpm typecheck && pnpm lint && pnpm test
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

See [docs/design/architecture.md](./docs/design/architecture.md) for technical architecture details.

## License

[MIT](./LICENSE)
