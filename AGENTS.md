<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->

---

## Beadline Project Constraints & Engineering Guardrails

**Beadline** is a lightweight, cross-platform (Windows / macOS / Linux), open-source desktop client for **Beads (`bd`)**, built with **Tauri v2 (Rust) + React 19 + TypeScript + Tailwind CSS v4 + Coss UI (`@base-ui/react`)**.

Before making changes, read [docs/design/architecture.md](docs/design/architecture.md) and the matching layer specs under `.trellis/spec/frontend/` and `.trellis/spec/backend/`.

### 1. Non-Negotiable Architecture Rules

1. **Zero Polling / Event-Driven Sync**:
   - Never introduce periodic `setInterval` polling loops for Beads data.
   - All live updates flow from Rust `notify` filesystem watching on `.beads/` (`issues.jsonl` and Dolt HEAD/manifest changes) via Tauri events to React.
2. **Dolt Lock Safety (Hybrid Engine)**:
   - Reads parse `.beads/issues.jsonl` directly in Rust (<25ms, zero Dolt contention with concurrent AI coding agents), with debounced background `bd export` fallback when Dolt state is newer or JSONL is missing.
   - All writes (`create`, `update`, `close`, `reopen`, `dep`, `comment`, `github sync`, `init`) execute through a **per-workspace serialized async queue** invoking the `bd` CLI with explicit argument arrays (`Command::new(&bd_path).args(...)`).
   - **Never** construct shell strings (`sh -c` / `cmd /C`) from user input, task titles, descriptions, labels, or paths.
   - On Windows, every spawned `bd` / `git` / `gh` process **must** set `CREATE_NO_WINDOW` (`0x08000000`) so no console window flashes.
3. **Zero Target-Repository Pollution**:
   - Beadline must **never** write app config, theme files, workspace lists, or module aliases into a user's Git repository.
   - All persistent app state lives in the OS standard app config directory (`dirs::config_dir()` / Tauri `app_config_dir()`).
   - When initializing Beads for a user repository via the UI, always use non-invasive flags (`--setup-exclude --skip-hooks --skip-agents --non-interactive`).
4. **Native Desktop Restraint**:
   - Frameless custom titlebar (native drag region + Windows caption controls / macOS traffic-light inset) and persistent window geometry.
   - Keep the app quiet and lightweight: no system tray bloat, no unsolicited OS notification popups, no embedded terminal.

### 2. Visual & UX Guardrails (Anti-AI-Slop Law)

Beadline's flagship aesthetic is **Warm Clay × Coss UI × Linear** (with switchable **Linear Slate** and **Pure Mono** presets + Theme JSON customization). Every UI contribution must obey:

1. **No Boxes Inside Boxes**: Kanban lanes sit directly on `--bg` separated by 1px subtle hairlines — never wrap lanes in bordered/rounded cards.
2. **No Badge / Pill Soup on Cards**: Kanban cards use a crisp 3-line typographic hierarchy (Linear-style SVG priority signal + muted mono ID + optional `#N`; clean 2-line title; borderless inline module dot + plain text blocker/commit). Never decorate cards with 6–10 colored bordered pill rectangles, bright white `#N` badges, or thick colored left stripes.
3. **Hover-Only Secondary Actions**: Quick actions like `✓ Complete` appear quietly on card hover (swapping cleanly with the top-right slot) instead of occupying a permanent footer bar.
4. **Visual Hierarchy by State**: Closed-column cards use muted contrast so the eye naturally lands on `In Progress` and `Ready`.
5. **Semantic Tokens Only**: Never hardcode hex colors or raw Tailwind grays in feature components. Always consume theme CSS custom properties (`--bg`, `--surface`, `--fg`, `--accent`, etc.) so all presets (`Warm Clay`, `Linear Slate`, `Pure Mono`), `light/dark/system` modes, and imported Theme JSONs work seamlessly.
6. **Bilingual Parity (`zh-CN` & `en`)**: No hardcoded user-facing strings in components. Use idiomatic Chinese terminology (e.g., `主目标` / `就绪可派发` / `被阻塞` — **never** translationese like `史诗`).

### 3. Dogfooding Beads (`bd`) in This Repository

- This repo tracks its own milestones in local Beads (prefix `bl`, `.beads/` excluded via `.git/info/exclude`, zero `.git/hooks`).
- Module taxonomy for Beadline itself:
  - `mod:engine` — Rust Beads reader, `notify` watcher, CLI write queue, GitHub sync
  - `mod:desktop` — Tauri v2 shell, frameless window, geometry persistence, OS config & packaging
  - `mod:ui` — React + Coss UI design system, views, `⌘K` command palette, theme & i18n
  - `mod:agent` — Smart link/Worktree parser & Agent Prompt builder
  - `mod:docs` — Open-source docs, specs, CI & release engineering
- When closing or updating tasks assigned to sub-agents/models, pass `--force` to `bd`.
- **Human-Readable Issue Writing Law (No Machine-Log Spam)**:
  - **Titles (<= 25 CJK chars / <= 65 Latin chars)**: State the user-facing outcome or clear engineering goal — never stuff internal function names, file paths, branch names, or model names into titles.
  - **Mandatory Plain-Language Description**: Every `bd create` must include `-d` starting with a one-sentence plain-language summary ("What this does & why it matters"), followed by acceptance expectations. Never leave `description` blank.
  - **Technical Metadata Goes in `notes`**: Put `branch`, `commit`, `model`, `wave`, and `t3-thread://` links into `--notes` / `comments`, keeping titles and descriptions readable for humans.
  - **No Per-Round Card Spam**: For multi-round review/fix loops (`r2/r3/r4`) on the same scope, append round logs via `bd comments add` or `--append-notes` on the existing issue instead of spawning duplicate cards per round.

### 4. Verification Gates (Before Marking Any Task Done)

Once the Tauri + Vite workspace is scaffolded, run before concluding any implementation task:
- Frontend: `pnpm typecheck && pnpm lint && pnpm test && pnpm build`
- Backend (Rust): `cargo fmt --check && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml`
