# Frontend Hook Guidelines (`Beadline`)

> Conventions for React hooks managing Tauri IPC subscriptions, mutations, keyboard shortcuts, and window controls.

## Naming & Placement

- Shared app hooks live in `src/hooks/` and use the `use<Domain>` naming convention (`useSnapshotEvents.ts`, `useGlobalShortcuts.ts`, `useWindowState.ts`).
- Feature-specific hooks stay colocated only when used by a single view.

## Tauri Event Subscription Pattern

Always clean up Tauri event listeners safely, handling React 19 StrictMode double-mount cleanly:

```ts
useEffect(() => {
  let active = true;
  let unlisten: (() => void) | undefined;

  void subscribeSnapshotUpdated((payload) => {
    if (active) applySnapshot(payload);
  }).then((fn) => {
    if (!active) fn();
    else unlisten = fn;
  });

  return () => {
    active = false;
    unlisten?.();
  };
}, [applySnapshot]);
```

## Keyboard Shortcut Rules (`useGlobalShortcuts`)

1. Ignore single-key shortcuts (`C`, `1–4`, `P`, `/`) whenever focus is inside `input`, `textarea`, `select`, or `[contenteditable="true"]`.
2. Always allow `⌘K` / `Ctrl+K` (Command Palette) and `Escape` (layered dismissal: Command Palette → Modal → Popover → Drawer → Search Blur).
3. Never bind shortcuts that conflict with OS window management or standard text editing (`Ctrl+C`, `Ctrl+V`, `Ctrl+A`, `Alt+F4`).

## Forbidden Hook Patterns

- **No `setInterval` data polling** for snapshots, versions, or project lists. Live synchronization is strictly push-based from Rust `notify`.
- **No direct `invoke()` inside `useEffect` without cancellation guards** — stale workspace switch responses must check whether `workspacePath === currentActivePath` before writing to store state.
- **No duplicated side effects**: mount `useSnapshotEvents` and `useGlobalShortcuts` once at the root `App.tsx` shell.
