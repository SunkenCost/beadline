# Beadline Architecture & Product Specification

> Authoritative design specification locked during initial architecture review (2026-11-26).

## 1. Product Vision & Positioning

**Beadline** is an open-source, cross-platform (**Windows / macOS / Linux**) native desktop client for **Beads (`bd`)** — designed as a lightweight, refined, Windows-first-class alternative to Electron-based clients like Beadbox.

### Design Principles
1. **Featherweight Native Footprint**: Tauri v2 + system WebView2 / WKWebView (~10 MB bundle, ~50 MB RAM, instant cold start, zero local HTTP server / zero Python runtime).
2. **Event-Driven, Zero Idle Overhead**: OS filesystem watching (`notify`) instead of timer polling; zero lock contention with concurrent AI coding agents writing to Dolt.
3. **Designer-Grade Restraint**: Calm Warm Editorial / Linear typography using genuine **Coss UI (`@base-ui/react`)** primitives — zero nested box-in-box clutter, zero pill-badge walls.
4. **Zero Target-Repository Pollution**: All user settings, workspace lists, module aliases, and custom themes live exclusively in the OS standard app config directory.
5. **Bilingual & Open-Source Ready**: Native `zh-CN` and `en` localization with idiomatic phrasing (no literal translationese).

---

## 2. Technology Stack

| Layer | Technology | Rationale |
| :--- | :--- | :--- |
| **Desktop Shell & Backend** | **Tauri v2** (Rust, Edition 2021) | Native OS webview, small binary, safe concurrency, cross-platform Win/macOS/Linux |
| **Filesystem Watching** | `notify` + `notify-debouncer-mini` | Zero-polling OS-native events on `.beads/` |
| **Async & Serialization** | `tokio`, `serde`, `serde_json`, `thiserror`, `tracing` | Typed IPC commands, serialized CLI write queue, structured error/log handling |
| **Frontend Framework** | **React 19** + **TypeScript 5 (strict)** + **Vite 6** | Fast builds, strict Rust↔TS IPC contract typing |
| **Styling & Primitives** | **Tailwind CSS v4** + **Coss UI (`@base-ui/react`)** + **Lucide Icons** | Accessible headless primitives, semantic CSS variables, crisp geometric icons |
| **State Management** | **Zustand** (lightweight client/UI state) + Rust snapshot events | Clean separation between authoritative backend snapshots and transient UI filters |
| **Internationalization** | Lightweight typed dictionary (`zh-CN`, `en`) | Zero runtime bloat, compile-time key checking |

---

## 3. Window & Native Desktop Behavior

- **Frameless Custom Titlebar**:
  - `decorations: false` on Windows with crisp custom minimize / maximize-restore / close controls and `data-tauri-drag-region`.
  - `titleBarStyle: "Overlay"` on macOS with safe left inset for native traffic lights.
- **Persistent Window Geometry**: Save and restore window `x`, `y`, `width`, `height`, and `maximized` state across launches (clamped to current monitor bounds on restore).
- **Deliberately Quiet**: No system tray residency, no global OS hotkey hijacking, no unsolicited desktop notification popups. Closing the main window exits cleanly.

---

## 4. Information Architecture & Spatial Layout

### 4.1 Three-Pane Minimalist Workbench
1. **Collapsible Left Sidebar (`228px` / `52px` collapsed)**:
   - **Workspace Switcher Popover**: Lists initialized Beads repos, auto-discovered sibling repos, manual path picker (native folder dialog), and non-invasive one-click `bd init`.
   - **All Projects · Active** entry: Unified cross-workspace overview showing in-progress tasks across all registered repositories.
   - **4 Core Views** for the active project:
     1. `Board` (任务看板)
     2. `Epics` (主目标进度)
     3. `Dependency Graph` (依赖图谱)
     4. `All Issues` (全部清单)
   - **Modules Section**: Auto-derived from the project's configured module label prefix (default `mod:*`), showing colored 6px dots + counts.
   - **Active Heartbeat**: Compact list of currently `in_progress` sub-tasks in the active project/module.
   - **Footer Status**: Minimal live sync LED (`Dolt Ready` / `Syncing` / `Error`) + Settings button.

2. **Full-Bleed Main Stage**:
   - **Top Bar**: View heading + active module breadcrumb + borderless inline KPI counts + search input (`/`) + Board scope toggle (`Tasks Only` / `With Epics`) + GitHub Sync trigger + `+ New` (`C`).
   - **Board View**: 4 flat columns (`In Progress`, `Ready`, `Blocked`, `Closed`) separated by `1px` hairlines. Cards use a strict 3-line typographic layout; `Closed` lane is visually recessed; quick `✓ Complete` appears only on hover.
   - **Epics View**: Parent goal cards with thin 3px progress tracks and nested clickable child rows.
   - **DAG View**: Layered Sugiyama-style SVG graph with drag pan, wheel zoom, fit-to-screen, and hover highlighting of connected upstream/downstream chains.
   - **All Issues View**: Sticky-header high-density table with status, priority, type, epic, and assignee filters.

3. **Slide-Over Detail Drawer (`540px`)**:
   - Editable title, status/priority/assignee/labels/module picker, smart extracted chips (GitHub Issue, Git branch, Commit SHA, Worktree path, custom deep links), **Copy Agent Prompt** action, blocker/downstream dependency editor, Markdown preview/edit for description/acceptance criteria/notes, comments timeline, and Close/Reopen footer.

4. **Global Command Palette (`⌘K / Ctrl+K`)**:
   - Fuzzy jump to any issue (by ID, title, `#N`, branch, SHA), switch workspace, switch module, switch view, switch theme/language, create issue, or trigger GitHub sync.

---

## 5. Theme & Customization System

All UI colors derive from semantic CSS tokens (`--bg`, `--sidebar`, `--surface`, `--surface-subtle`, `--surface-hover`, `--surface-active`, `--fg`, `--fg-2`, `--fg-3`, `--border`, `--border-strong`, `--border-subtle`, `--primary`, `--primary-fg`, `--accent`, `--accent-soft`, `--accent-border`, `--success`, `--warning`, `--danger`, `--info`, `--radius-scale`, `--density`).

### Built-In Presets (Each Supporting `light` / `dark` / `system`)
1. **Warm Claude** *(default)* — Warm paper `#FAF9F6` / warm charcoal `#121211`, terracotta `#C96442` / `#D97757`.
2. **Linear Slate** — Crisp neutral slate `#FFFFFF` / `#0E1013`, indigo accent `#5E6AD2`.
3. **Pure Mono** — Editorial monochrome `#FAFAFA` / `#0C0C0C`, high-contrast ink accent.

### User Customization & Theme JSON
- Settings allow overriding accent color, corner radius (`sharp` / `default` / `soft`), and list/card density (`compact` / `comfortable`).
- Users can **export** and **import** validated `BeadlineThemeV1` JSON files (schema-validated on import; never executes arbitrary JS or unsafe CSS).

---

## 6. Agent-Native Features

1. **Smart Metadata Extraction** (computed in Rust from `description`, `notes`, `external_ref`, `comments`):
   - GitHub issue number & URL (resolved against repo's `github.repository` config or `git remote origin`).
   - Git branches (`feat/...`, `fix/...`, `release/...`, `integration/...`, `chore/...`).
   - Git commit SHAs (7–40 hex refs following `commit`, `SHA`, arrow chains `abc1234→def5678`, or backticks).
   - Local Git worktree paths matched against `git worktree list --porcelain` (open folder in OS file manager or copy path).
   - Configurable custom deep-link schemes (default includes `t3-thread://`, `vscode://`, `cursor://`).
2. **One-Click "Copy Agent Prompt"**:
   - Generates a clean Markdown brief containing task ID, title, status, parent epic, module, acceptance criteria, active blockers, linked branch/commit, and concise `bd` close/update instructions.
3. **Safe GitHub Issues Sync**:
   - Default pull-only (`bd github sync --pull-only --prefer-local`), automatically injecting `GITHUB_TOKEN` from `gh auth token` if `GITHUB_TOKEN` is unset in env.

---

## 7. Rust Hybrid Data Engine & Concurrency Model

```
[.beads/issues.jsonl & .beads/dolt/]
        │
        ▼ (OS filesystem events via `notify`, 250ms debounce)
[WorkspaceWatcher] ──► checks manifest/HEAD mtime vs issues.jsonl mtime
        │                   ├── if JSONL fresh: parse directly (<25ms, zero Dolt lock)
        │                   └── if Dolt newer / JSONL missing: debounced `bd export`
        ▼
[SnapshotBuilder] ──► builds normalized Issues, Lanes, Epics, DAG Edges, Modules, SmartMeta
        │
        ▼ (Tauri event `beadline://snapshot-updated` — emitted only on fingerprint diff)
[React Store]

[User Mutation (create/update/close/dep/comment/sync)]
        │
        ▼ (Tauri IPC invoke)
[Per-Workspace Serialized Write Queue (tokio::sync::Mutex)]
        │
        ▼ (`Command::new(bd_path).args([...])`, CREATE_NO_WINDOW on Windows)
[Immediate Snapshot Refresh] ──► returns fresh Snapshot directly to caller
```

### `bd` Binary Resolution Order
1. Explicit path configured by user in Settings (`bd_path`).
2. `bd` found in system `PATH`.
3. Platform fallback locations (Windows WinGet `GasTownHall.Beads`, Scoop, Cargo bin; macOS `/opt/homebrew/bin/bd`, `/usr/local/bin/bd`, `~/.local/bin/bd`).

---

## 8. Persistence & Module Rules (Zero Repo Pollution)

All application data lives under the OS config directory (`%APPDATA%\Beadline\` on Windows, `~/Library/Application Support/Beadline/` on macOS, `~/.config/beadline/` on Linux):

- `settings.json`:
  - `language`: `"zh-CN" | "en"`
  - `theme`: `{ preset, appearance, accentOverride?, radius, density, customThemes[] }`
  - `bdPath`: optional explicit CLI path
  - `useForceWrites`: `boolean` (default `true` for multi-agent assignee compatibility)
  - `deepLinkSchemes`: `string[]` (default `["t3-thread", "vscode", "cursor"]`)
  - `window`: `{ x, y, width, height, maximized }`
- `workspaces.json`:
  - Registered repository paths + per-workspace overrides:
    - `modulePrefix`: default `"mod:"` (overridable to `"area:"`, `"pkg:"`, etc.)
    - `moduleAliases`: `Record<string, { label?: string; color?: string }>`
    - `pinned`: `boolean`

---

## 9. Delivery Milestones

- **Milestone 0 (Current)**: Repository creation at `Y:\GitHub\beadline`, Git/Trellis/Beads initialization, full engineering & aesthetic specs.
- **Milestone 1**: Tauri v2 + React/Tailwind v4/Coss UI workspace, Rust hybrid `notify` engine + CLI write queue, frameless window + geometry persistence, Workspace & Module system, Theme/i18n engine, and all 4+1 views verified against real Beads repositories.
- **Milestone 2**: `⌘K` Command Palette, Theme JSON import/export, One-click Agent Prompt copy, GitHub sync, Settings view, automated tests (Vitest + Cargo test), and Windows `.exe`/`.msi` + macOS `.dmg` build configuration.
