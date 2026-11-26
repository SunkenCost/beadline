# Frontend State Management (`Beadline`)

> Clear separation between authoritative Rust backend data and lightweight Zustand client UI state.

## State Ownership Boundaries

| State Category | Owner | Persistence |
|----------------|-------|-------------|
| Beads issues, lanes, epics, DAG edges, smart metadata, fingerprint | **Rust Engine** (cached in `useWorkspaceStore`) | `.beads/` via `bd` (authoritative) |
| Registered workspaces, per-project module prefix & aliases | **Rust Config** (mirrored in `useWorkspaceStore`) | OS config dir (`workspaces.json`) |
| Language, theme preset/overrides/custom themes, `bdPath`, `useForceWrites`, `deepLinkSchemes` | **Rust Config** (mirrored in `useSettingsStore`) | OS config dir (`settings.json`) |
| Active view, active module filter, board scope, search text, selected issue ID, open drawer/modal/palette | **Client UI** (`useUiStore`) | In-memory (session) |
| Transient form inputs (create modal, comment draft, inline edit buffer) | **Local React `useState`** | Component-local |

## Mutation & Freshness Rules

1. **Snapshot-Returning Mutations**: Every Rust write command (`create_issue`, `update_issue`, `close_issue`, `reopen_issue`, `add_dependency`, `remove_dependency`, `add_comment`, `github_sync`, `init_beads`) returns the updated `WorkspaceSnapshot` directly. Apply it immediately to `useWorkspaceStore` without waiting for a watcher tick.
2. **Stale-Response Guard**: When switching workspaces rapidly, attach the target `workspacePath` to async requests and ignore responses that no longer match `useWorkspaceStore.getState().activePath`.
3. **Editing Protection**: Incoming `beadline://snapshot-updated` events update store data silently without clobbering uncommitted text in an open comment box, create modal, or active inline description editor.
4. **No Derived Data Duplication**: Store the flat `issues: IssueDto[]` and `byId: Record<string, IssueDto>` index once per snapshot; derive filtered board lanes, module lists, and KPI counts via pure memoized selectors.
