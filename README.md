<div align="center">
  <img src="./assets/icon.svg" width="64" height="64" alt="Beadline icon" />
  <h1>Beadline</h1>
  <p>A fast, quiet desktop GUI for <a href="https://github.com/steveyegge/beads">Beads (<code>bd</code>)</a> — manage issues, epics, and dependency graphs across local Git repos.</p>
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

Beads (`bd`) makes it easy for AI coding agents to track tasks inside a Git repository, but reading dozens of nested tasks in a terminal gets old fast.

**Beadline** opens a native desktop window directly over your local `.beads/` folders — **~5 MB binary**, **~90 MB RAM on Windows**, instant file-watch sync, and **zero config files written into your repos**.

## Features

| | What you get |
|---|---|
| **5 Focused Views** | **Kanban Board** (flat or grouped by epic), **Epics** progress, interactive **Dependency Graph (DAG)**, filterable **All Issues** table, and **All Active** across every connected repo. |
| **Multi-Repo & Modules** | Switch between local repos instantly. Slice tasks by module labels (`mod:*`, or any custom prefix like `area:` / `pkg:`) with custom display names and colors. |
| **Built for AI Workflows** | One-click **Copy Agent Prompt** (bundles context, acceptance criteria, blockers, and branches). Auto-detects Git branches, commit SHAs, GitHub `#issues`, local worktree folders, and deep links (`t3-thread://`, `vscode://`, `cursor://`). |
| **Readable Task Context** | Parent epic summaries and close reasons surface at the top of the drawer, with an optional toggle to fold completed AI iteration rounds on the board. |
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

**Requirements:** Node.js 20+, `pnpm`, [Rust](https://rustup.rs/), and [`bd` CLI](https://github.com/steveyegge/beads).

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
   ├── Read  ──▶ Rust `notify` watcher + direct `.beads/issues.jsonl` parse (milliseconds, no polling)
   ├── Write ──▶ Serialized `bd` CLI queue with optimistic UI updates & automatic rollback
   └── State ──▶ Stored strictly in OS app config dir (e.g. %APPDATA%\dev.beadline.desktop\)
```

- **No lock contention:** Reads parse `issues.jsonl` directly with content-based fingerprinting, so browsing never fights concurrent AI agents running `bd`.
- **Safe `bd init` & GitHub sync:** Non-invasive initialization (`.git/info/exclude`, no Git hooks, `AGENTS.md` untouched) and one-click `bd github sync`.

## Development

```bash
pnpm typecheck && pnpm lint && pnpm test
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

See [docs/design/architecture.md](./docs/design/architecture.md) for the technical architecture.

## License

[MIT](./LICENSE)
