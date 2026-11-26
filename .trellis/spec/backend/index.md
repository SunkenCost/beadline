# Backend Development Guidelines (`Beadline` — Tauri v2 / Rust)

> Engineering standards for the Rust backend (`src-tauri/`) powering Beadline's hybrid Beads engine, OS filesystem watcher, serialized `bd` CLI queue, and native desktop shell.

---

## Pre-Development Checklist

Before modifying or adding Rust backend code, confirm:

- [ ] Every spawned CLI process (`bd`, `git`, `gh`) uses explicit `Command::new(bin).args([...])` — **never** shell string interpolation (`cmd /C`, `sh -c`).
- [ ] On Windows, every spawned child process sets `CREATE_NO_WINDOW` (`0x08000000`) via `CommandExt::creation_flags`.
- [ ] All IPC DTOs use `#[serde(rename_all = "camelCase")]` and stay in 1:1 sync with `src/types/ipc.ts`.
- [ ] Zero files are written into a user's Git repository; all app state persists under Tauri's `app_config_dir()`.
- [ ] All `bd` mutations acquire the per-workspace async write mutex and return a fresh `WorkspaceSnapshot`.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | `src-tauri/src/` module layout and ownership boundaries | Active |
| [Database & Data Engine Guidelines](./database-guidelines.md) | Hybrid JSONL/Dolt reader, `notify` watcher, serialized `bd` write queue, OS config store | Active |
| [Error Handling](./error-handling.md) | Structured `AppError` enum, Tauri IPC error serialization, recovery hints | Active |
| [Logging Guidelines](./logging-guidelines.md) | `tracing` structured logs, secret/token redaction, silent production output | Active |
| [Quality Guidelines](./quality-guidelines.md) | `cargo fmt` / `clippy` / `test` gates, cross-platform Windows/macOS rules, security boundaries | Active |

---

## Quality Check (Before Finishing Any Backend Task)

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

**Language**: All spec documentation is written in **English**.
