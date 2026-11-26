# Frontend Component & Visual Design Guidelines (`Beadline`)

> Beadline's primary differentiator is designer-grade restraint: **Warm Claude × Coss UI × Linear**. Every component must actively reject "AI-slop" visual clutter.

---

## 1. Anti-AI-Slop Visual Laws (Mandatory)

1. **No Boxes Inside Boxes**:
   - Kanban columns (`.lane`) sit flat on `var(--bg)` separated solely by `1px solid var(--border-subtle)`. Never wrap lanes in rounded cards or elevated containers.
   - Topbar KPI counters are plain inline text + 5px status dots + tabular mono numerals — never bordered pills or metric cards.
2. **No Badge / Pill Soup on Cards**:
   - Kanban cards follow a strict **3-row typographic hierarchy**:
     - **Row 1**: 13px Linear-style SVG priority icon + muted mono ID + optional muted type marker (`Epic`/`Bug` only; omit default `Task`) + optional muted `#N` + right-aligned assignee dot/name.
     - **Row 2**: Crisp `13px`, `font-weight: 500` title (max 2 lines).
     - **Row 3** *(optional)*: Borderless inline metadata — 5.5px colored module dot + label, clickable red blocker text (`Blocked by bl-xxx`), green unlock count, or muted mono commit SHA.
   - **Forbidden on cards**: bordered colored priority pills, bright white `#N` pills, thick colored left accent stripes, bordered module rectangles, or permanent footer action bars.
3. **Hover-Only Secondary Actions**:
   - Quick completion (`✓ Done`) on `in_progress` cards sits in the top-right slot and reveals cleanly on `:hover` (fading out the assignee label in place so layout never shifts).
4. **State-Driven Visual Hierarchy**:
   - Cards in the `Closed` lane use transparent background, subtle border (`var(--border-subtle)`), and secondary text (`var(--fg-2)`) until hovered, keeping visual focus on `In Progress` and `Ready`.
5. **Minimal Elevation & Motion**:
   - Prefer `1px` crisp hairlines over heavy drop shadows; reserve shadows strictly for floating popovers, dialogs, and the `⌘K` command palette.
   - Transitions stay fast and tactile (`120–180ms cubic-bezier(0.2, 0.8, 0.2, 1)`). Zero bounce, zero glow blobs, zero gradient neon borders.

---

## 2. Coss UI (`@base-ui/react`) Primitives

- Build all interactive overlays (Dialog, Popover, Menu, Select, Switch, Tooltip, Command Palette) on **`@base-ui/react`** styled via Tailwind v4 utility classes referencing semantic CSS tokens.
- Preserve built-in keyboard navigation, focus trapping, Escape dismissal, and ARIA roles.
- Never replace native-feeling Coss primitives with raw `<div onClick>` dropdowns or browser `window.prompt()` / `window.confirm()` dialogs (use proper Coss UI confirmation/input dialogs instead).

---

## 3. Frameless Titlebar Contract

- The top 36–40px shell bar carries `data-tauri-drag-region` on non-interactive surface areas.
- Every button, input, or clickable trigger inside the titlebar must opt out of dragging (`data-tauri-drag-region="false"` or CSS `app-region: no-drag`).
- Platform adaptation:
  - **Windows / Linux**: Right-aligned custom caption buttons (Minimize, Maximize/Restore, Close — close highlights `var(--danger)` on hover).
  - **macOS**: Left safe padding (`72px`) for native traffic lights; omit right-aligned Windows caption controls.

---

## 4. Semantic Theme Tokens & Density

- Never hardcode hex values (`#111`, `#fff`, `bg-zinc-900`, `text-orange-500`) inside feature components.
- Always use semantic tokens defined in `src/styles/tokens.css`:
  `--bg`, `--sidebar`, `--surface`, `--surface-subtle`, `--surface-hover`, `--surface-active`, `--fg`, `--fg-2`, `--fg-3`, `--border`, `--border-strong`, `--border-subtle`, `--primary`, `--primary-fg`, `--accent`, `--accent-soft`, `--accent-border`, `--success`, `--warning`, `--danger`, `--info`.
- Support 3 built-in presets (`warm-claude`, `linear-slate`, `pure-mono`) × 3 appearance modes (`light`, `dark`, `system`), plus user overrides (`accentOverride`, `radius`, `density`) and validated `BeadlineThemeV1` JSON imports.

---

## 5. Localization (i18n) Rules

- Every visible label, tooltip, placeholder, toast, and empty state must go through `useT()`.
- Keep Chinese terminology natural and non-translated:
  - `epic` → `主目标` (never `史诗`)
  - `ready` → `就绪可派发`
  - `blocked` → `被阻塞`
  - `in_progress` → `进行中`
  - `closed` → `已完成`
  - `chore` → `收口`
