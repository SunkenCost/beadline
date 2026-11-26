# Frontend Development Guidelines (`Beadline`)

> Engineering and visual standards for the React 19 + TypeScript + Tailwind CSS v4 + Coss UI (`@base-ui/react`) frontend of Beadline.

---

## Pre-Development Checklist

Before modifying or adding frontend code, confirm:

- [ ] Target UI obeys the **Anti-AI-Slop Visual Laws** in [Component Guidelines](./component-guidelines.md) (no boxes-in-boxes, no badge soup, hover-only secondary actions, semantic CSS tokens only).
- [ ] All user-facing strings use the typed i18n dictionary (`zh-CN` & `en`) with idiomatic Chinese (never `史诗` for Epics — always `主目标`).
- [ ] All Tauri IPC calls go through typed wrappers in `src/lib/ipc.ts` (never raw `invoke()` inside UI components).
- [ ] No periodic `setInterval` polling is introduced; live updates subscribe to Rust `beadline://snapshot-updated` events.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | `src/` module layout, feature boundaries, and naming rules | Active |
| [Component Guidelines](./component-guidelines.md) | Coss UI (`@base-ui/react`) composition, Anti-AI-Slop visual laws, theme tokens, frameless titlebar | Active |
| [Hook Guidelines](./hook-guidelines.md) | Tauri event listeners, IPC mutation hooks, keyboard shortcuts & `⌘K` palette hooks | Active |
| [State Management](./state-management.md) | Zustand client/UI state vs authoritative Rust snapshots, cross-workspace state | Active |
| [Type Safety](./type-safety.md) | Strict TS rules, Rust↔TS IPC DTO parity, Theme JSON validation | Active |
| [Quality Guidelines](./quality-guidelines.md) | Verification commands, accessibility, performance, forbidden patterns | Active |

---

## Quality Check (Before Finishing Any Frontend Task)

```bash
pnpm typecheck && pnpm lint && pnpm test && pnpm build
```

**Language**: All spec documentation is written in **English**.
