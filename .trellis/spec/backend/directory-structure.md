# Backend Directory Structure (`src-tauri/`)

> Canonical Rust module layout for Beadline's Tauri v2 backend.

```text
src-tauri/
├── Cargo.toml
├── tauri.conf.json              # Frameless window config, bundle targets (nsis/msi/dmg), capabilities
├── capabilities/
│   └── default.json             # Minimal Tauri v2 ACL permissions (window controls, dialog, clipboard, opener)
├── icons/                       # Cross-platform Beadline icons (.ico, .icns, .png)
└── src/
    ├── main.rs                  # Binary entry point (delegates to lib::run, hides Windows console in release)
    ├── lib.rs                   # Tauri Builder setup, managed AppState, window state restore/save, command registration
    ├── error.rs                 # Structured `AppError` + `AppErrorDto` (`Serialize` for Tauri IPC)
    ├── models/
    │   ├── mod.rs
    │   ├── issue.rs             # Raw JSONL structs + normalized `IssueDto`, `SmartMeta`, `BoardLane`, `WorkspaceSnapshot`
    │   ├── workspace.rs         # `WorkspaceSummary`, `WorkspaceConfig`, `CrossProjectActiveItem`
    │   └── settings.rs          # `AppSettings`, `ThemeSettings`, `WindowGeometry`, `BeadlineThemeV1`
    ├── config/
    │   ├── mod.rs
    │   └── store.rs             # Atomic read/write of `settings.json` & `workspaces.json` in `app_config_dir()`
    ├── engine/
    │   ├── mod.rs
    │   ├── bd_cli.rs            # `bd` binary discovery, safe subprocess execution (`CREATE_NO_WINDOW`), timeout & `--force` handling
    │   ├── jsonl_reader.rs      # Fast `.beads/issues.jsonl` parser, lane/epic/DAG/module computation, fingerprint hashing
    │   ├── smart_meta.rs        # Extracts GitHub Issue URLs, branches, commit SHAs, worktrees, configurable deep links
    │   ├── git_meta.rs          # Lightweight `git remote` & `git worktree list --porcelain` inspection
    │   └── workspace_mgr.rs     # Per-workspace write mutexes, coalesced background `bd export`, snapshot cache
    ├── watcher/
    │   ├── mod.rs
    │   └── fs_watcher.rs        # `notify` + debouncer watching `.beads/` and emitting `beadline://snapshot-updated`
    └── commands/
        ├── mod.rs
        ├── workspaces.rs        # list_workspaces, add_workspace, remove_workspace, init_beads, pick_folder
        ├── issues.rs            # get_snapshot, get_issue_detail, create/update/close/reopen, deps, comments, all_active
        ├── sync.rs              # github_sync (pull/push with automatic `gh auth token` injection)
        ├── settings.rs          # get_settings, update_settings, import_theme, export_theme, detect_bd_path
        └── system.rs            # open_external_link (scheme allowlist), reveal_path_in_file_manager
```

## Layer Dependency Rules

- `commands/` are thin IPC adapters: validate arguments, call `engine/` or `config/`, map errors into `AppError`, return DTOs. Never spawn `Command` directly inside `commands/`.
- `engine/jsonl_reader.rs` and `engine/smart_meta.rs` are pure functions/modules testable via `cargo test` with fixture files and zero Tauri runtime dependency.
- `models/` has zero dependency on `tauri`, `notify`, or `tokio::process`.
