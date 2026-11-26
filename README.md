# Beadline

> **A lightweight, native desktop client for [Beads (`bd`)](https://github.com/steveyegge/beads) — built for the AI coding agent era.**

[简体中文](./README.zh-CN.md)

Beadline is a fast, open-source, cross-platform (**Windows / macOS / Linux**) desktop GUI for Beads issue graphs. Built with **Tauri v2 (Rust)**, **React 19**, **Tailwind CSS v4**, and **Coss UI (`@base-ui/react`)**, it starts instantly, stays lightweight (~5 MB binary, ~90 MB RAM on Windows WebView2), and stays out of your way while AI agents work in parallel across your repositories.

## Why Beadline?

- **Truly Native & Lightweight**: ~10 MB installer powered by Tauri v2 and system WebView2 / WKWebView — no bundled Electron Chromium, no local Python runtime, no port conflicts.
- **Zero-Polling Event-Driven Sync**: Rust watches `.beads/` via OS filesystem events (`notify`). Reads parse `.beads/issues.jsonl` in milliseconds without contending with concurrent AI agents writing to Dolt, while writes flow through a serialized `bd` CLI queue.
- **Calm, Intentional Design**: Default **Warm Clay × Coss UI** aesthetic, plus switchable **Cool Slate** and **Pure Mono** presets (light / dark / system), accent/radius/density controls, and shareable **Theme JSON** import/export.
- **Multi-Project × Multi-Module**: Switch seamlessly across local repositories, monitor an **All Projects · Active** overview, and slice each repository by configurable module prefixes (default `mod:*`, customizable to `area:*`, `pkg:*`, etc.) with custom aliases and color dots — **zero files written to your Git repos**.
- **Agent-Native Sweet Spot**:
  - Automatic detection of Git branches, commits, GitHub Issues, local Worktree paths, and custom deep links (e.g., `t3-thread://`).
  - One-click **Copy Agent Prompt** (bundles issue title, ID, acceptance criteria, blockers, and branch context ready to paste into Claude Code, Codex, Cursor, or T3 Code).
  - Safe GitHub Issues pull/push sync (`bd github sync`) and non-invasive `bd init`.
- **Keyboard-First & Bilingual**: Full **English / 简体中文** support, global `⌘K / Ctrl+K` command palette, and Linear-style single-key navigation (`C`, `1–4`, `P`, `/`, `Esc`).

## Core Views

1. **Kanban Board** — Clean 4-column typographic layout (`In Progress`, `Ready`, `Blocked`, `Closed`) with drag-and-drop transitions and hover-revealed quick completion.
2. **Epics Progress** — Rollup completion bars and child task breakdown per parent goal.
3. **Dependency Graph (DAG)** — Interactive pan/zoom dependency graph with upstream/downstream critical-path highlighting.
4. **All Issues** — High-density filterable table across status, priority, type, module, epic, and assignee.
5. **All Projects · Active** — Cross-repository live view of in-progress agent work.

## Architecture

See [docs/design/architecture.md](./docs/design/architecture.md) for the full design specification and [.trellis/spec/](./.trellis/spec/) for frontend and Rust backend engineering guidelines.

## Quick Start

### Prerequisites
- Node.js 20+ & `pnpm`
- Rust toolchain (`rustup`)
- [Beads CLI (`bd`)](https://github.com/steveyegge/beads)

```bash
pnpm install
pnpm tauri dev          # Run desktop app in dev mode
pnpm tauri build        # Build native release binary & installer
```

## Roadmap

- [x] Architecture specification, engineering constraints, and Trellis + Beads workspace initialization
- [x] **Milestone 1**: Tauri v2 + React/Coss UI frameless shell, Rust event-driven Beads engine, multi-workspace/module navigation, themes + bilingual i18n, 4+1 core views
- [x] **Milestone 2 (Core)**: `⌘K / Ctrl+K` command palette, Theme JSON import/export, 1-click Copy Agent Prompt, GitHub sync, epic swimlanes & context cards, WebView2 memory optimization
- [ ] **Milestone 2 (Release)**: Signed Windows NSIS/MSI & macOS DMG installers + GitHub Actions release workflow

## License

[MIT](./LICENSE)
