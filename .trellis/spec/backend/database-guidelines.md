# Data Engine, Concurrency & Storage Guidelines (`Beadline`)

> Beadline does not own its own SQL database. Its authoritative data source is each repository's local **Beads (`.beads/` embedded Dolt + `issues.jsonl`)**, paired with atomic JSON config files in the OS user config directory.

---

## 1. Hybrid Read Strategy (Zero Dolt Lock Contention)

AI coding agents (Claude Code, Codex, T3 Code, Cursor) frequently write to `.beads/dolt/` concurrently while Beadline is open. Calling `bd list --json` or `bd export` synchronously on every refresh causes multi-second Dolt lock contention.

Therefore, snapshot reads follow a strict two-tier path:

1. **Fast Path (<25ms, Lock-Free)**:
   - Parse `.beads/issues.jsonl` directly in Rust (`engine/jsonl_reader.rs`).
   - Compute a deterministic snapshot `fingerprint` (hash of JSONL mtime/size + Dolt HEAD/manifest mtime + relevant workspace config).
2. **Coalesced Background Export Fallback**:
   - Compare `.beads/dolt/` manifest/HEAD mtime against `.beads/issues.jsonl` mtime.
   - If `issues.jsonl` is missing or older than Dolt state (e.g., a repo without `export.auto = true`), schedule **at most one** background `bd --sandbox export -o .beads/issues.jsonl` task per workspace (coalesced via an `AtomicBool` flag).
   - Once the export writes `.beads/issues.jsonl`, the `notify` watcher naturally picks up the fresh file and emits `beadline://snapshot-updated`.
3. **Never Bypass `bd` for Writes**:
   - Do **not** directly mutate `.beads/issues.jsonl` or `.beads/dolt/` bytes from Rust. All mutations go through `bd` CLI so Beads enforces DAG cycle detection, status transitions, Dolt transactions, and hooks/export semantics.

---

## 2. Serialized Write Queue & Multi-Agent `--force` Handling

- Maintain a `HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>` in `WorkspaceManager`.
- Every mutating command (`create`, `update`, `close`, `reopen`, `dep add/remove`, `comments add`, `github sync`) acquires the workspace's async mutex before running `bd`.
- **Multi-Agent Actor Compatibility**: Beads rejects `bd close` / `bd update` when `issue.assignee` (e.g., `gpt-6.2-sol`, `gemini-4-neon`) differs from `git user.name` unless `--force` is supplied. Respect `settings.useForceWrites` (default `true`) by appending `--force` on `update`, `close`, and `reopen`.
- **Non-Invasive Initialization**: `init_beads` always invokes:
  ```text
  bd init --prefix <slug> --setup-exclude --skip-hooks --skip-agents --non-interactive
  ```
  followed by `bd config set export.auto true` so target repositories stay free of unsolicited Git hooks and untracked `.beads/` noise.

---

## 3. Event-Driven Watcher (`notify`)

- Watch the active workspace's `.beads/` directory (and optionally registered workspaces' `.beads/issues.jsonl` for the **All Projects · Active** view) with a `250ms` debounce window.
- Ignore temporary/lock churn (`*.lock`, `*.tmp`, Dolt `LOCK`, journal scratch files) to avoid watch storms.
- Emit `beadline://snapshot-updated` **only** when the computed `fingerprint` actually changes.
- Cleanly drop old watchers when switching away or removing a workspace.

---

## 4. App Config Persistence (Zero Target-Repo Pollution)

- Persist `settings.json` and `workspaces.json` strictly inside `app.path().app_config_dir()` (Windows `%APPDATA%\Beadline\`, macOS `~/Library/Application Support/Beadline/`, Linux `~/.config/beadline/`).
- Use **atomic write-and-rename** (`write` to `.tmp` in same directory → `rename` over target) so crashes never corrupt user settings.
- Validate workspace paths before registering (canonicalize, confirm directory exists; never auto-write files inside the workspace unless user explicitly clicks "Initialize Beads").
