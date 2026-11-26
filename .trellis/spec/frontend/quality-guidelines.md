# Frontend Quality Guidelines (`Beadline`)

> Verification gates, accessibility standards, performance targets, and forbidden patterns.

## Mandatory Verification Commands

Run before completing any frontend task:

```bash
pnpm typecheck   # tsc --noEmit
pnpm lint        # eslint / biome check
pnpm test        # vitest run (pure logic: dag-layout, prompt-builder, theme validator, modules)
pnpm build       # vite build (ensures production bundle compiles cleanly)
```

## Performance Targets

- First interactive paint on cached snapshot: **< 150ms**.
- Workspace switch on a 200-issue repository: **< 50ms** render time.
- Typing in search or `⌘K` command palette: **0 dropped frames** (memoized selectors; avoid rebuilding DAG SVG when DAG pane is not mounted).
- Idle CPU usage when app is open in background: **0%** (zero `setInterval` timers, zero infinite CSS animations on static cards).

## Accessibility & Keyboard-First Standards

- Full keyboard operation for core workflows: `⌘K`/`Ctrl+K` command palette, `C` create issue, `1–4` switch views, `P` switch workspace, `/` focus search, `Esc` dismiss top overlay.
- Visible focus rings (`2px solid var(--accent)` with `outline-offset: 2px`) on `:focus-visible` for all interactive elements.
- Every icon-only button has an accessible `aria-label` and tooltip.

## Forbidden Patterns

- Hardcoding colors (`#...`, `rgb(...)`, raw Tailwind palette classes) in feature components instead of semantic tokens.
- Hardcoding user-facing strings without adding both `zh-CN` and `en` keys.
- Using `window.alert`, `window.confirm`, or `window.prompt` (use Coss UI `Dialog`).
- Rendering untrusted Markdown with `dangerouslySetInnerHTML` without strict escaping/sanitization (never execute raw HTML tags from issue descriptions or GitHub issue bodies).
- Calling `setInterval` to poll backend state.
- Reintroducing nested lane borders, pill walls, or thick left card stripes on Kanban cards.
