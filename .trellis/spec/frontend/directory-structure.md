# Frontend Directory Structure (`Beadline`)

> Planned canonical layout for `src/` in the Vite + React 19 + TypeScript frontend.

## Top-Level Layout

```text
src/
├── main.tsx                     # React entry + early theme boot (prevents light/dark flash)
├── App.tsx                      # Root shell: frameless titlebar + 3-pane workbench + overlays
├── styles/
│   ├── tokens.css               # Semantic CSS variables & preset definitions (Warm Claude / Linear Slate / Pure Mono)
│   └── globals.css              # Tailwind v4 @import + base typography + subtle scrollbars
├── types/
│   ├── ipc.ts                   # Exact 1:1 mirror of Rust IPC DTOs (Snapshot, Issue, Workspace, Settings, Errors)
│   └── theme.ts                 # BeadlineThemeV1 schema & preset types
├── i18n/
│   ├── index.ts                 # Typed `useT()` hook & `t()` helper
│   ├── zh-CN.ts                 # Idiomatic Simplified Chinese catalog
│   └── en.ts                    # English catalog
├── lib/
│   ├── ipc.ts                   # Typed wrappers around `@tauri-apps/api/core::invoke` & events
│   ├── theme.ts                 # Theme token application, system-appearance listener, Theme JSON import/export validator
│   ├── prompt-builder.ts        # Pure function: builds structured "Copy Agent Prompt" Markdown from an Issue + Snapshot
│   ├── dag-layout.ts            # Pure Sugiyama-style layered DAG layout math (DOM-free, unit-tested in Vitest)
│   └── modules.ts               # Module prefix stripping, alias lookup, deterministic muted dot color palette
├── stores/
│   ├── useWorkspaceStore.ts     # Registered workspaces, active workspace path, per-workspace snapshots, cross-workspace active list
│   ├── useUiStore.ts            # Active view, selected issue ID, active module filter, board scope, search query, sidebar collapse, palette/modal open states
│   └── useSettingsStore.ts      # Persisted language, theme settings, bdPath, deepLinkSchemes, forceWrites
├── hooks/
│   ├── useSnapshotEvents.ts     # Subscribes to `beadline://snapshot-updated` & `beadline://watcher-error`
│   ├── useGlobalShortcuts.ts    # Keyboard navigation (`⌘K`/`Ctrl+K`, `C`, `1–4`, `P`, `/`, `Esc`)
│   └── useWindowState.ts        # Maximized state sync for frameless titlebar controls
└── components/
    ├── ui/                      # Thin Coss UI (`@base-ui/react`) styled primitives (Button, Dialog, Popover, Select, Switch, Tooltip, Toast, Kbd)
    ├── shell/                   # Titlebar, Sidebar, WorkspaceSwitcher, Topbar, CommandPalette (`⌘K`)
    ├── views/
    │   ├── BoardView.tsx        # 4-column hairline Kanban + BoardCard + drag-and-drop
    │   ├── EpicsView.tsx        # Parent goal progress cards + child rows
    │   ├── DagView.tsx          # Interactive SVG pan/zoom dependency graph
    │   ├── ListView.tsx         # Sticky-header filterable table
    │   └── AllActiveView.tsx    # Cross-project in-progress overview
    ├── drawer/                  # IssueDrawer, SmartChips, DependencyEditor, CommentsSection, AgentPromptButton
    ├── modals/                  # CreateIssueModal, InitBeadsModal
    └── settings/                # SettingsModal (General/Language, Theme & JSON import/export, Workspaces & Module Rules, Beads CLI)
```

## Module Boundary Rules

1. **Pure Logic Stays DOM-Free**: Graph layout (`dag-layout.ts`), prompt construction (`prompt-builder.ts`), theme validation (`theme.ts`), and module resolution (`modules.ts`) must not import React or DOM APIs so Vitest can test them in milliseconds.
2. **UI Primitives Live in `components/ui/`**: Feature components compose `components/ui/` wrappers built on `@base-ui/react` rather than importing `@base-ui/react` directly in every view.
3. **Single IPC Gateway**: Only `src/lib/ipc.ts` imports `@tauri-apps/api/core` (`invoke`) or `@tauri-apps/api/event` (`listen`). Views and stores never call `invoke()` directly.
