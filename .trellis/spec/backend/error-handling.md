# Backend Error Handling Guidelines (`Beadline`)

> Every fallible backend operation returns `Result<T, AppError>`, serialized into a structured `AppErrorDto` for clean, localized frontend display.

## Structured Error Design (`src-tauri/src/error.rs`)

Use `thiserror::Error` with stable machine-readable error codes so the frontend can display localized messages and actionable hints (e.g., "Install `bd` or set its path in Settings"):

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bd executable not found")]
    BdNotFound,

    #[error("workspace path does not exist: {0}")]
    WorkspaceNotFound(String),

    #[error("workspace has not initialized Beads")]
    BeadsNotInitialized,

    #[error("bd command failed ({exit_code}): {stderr}")]
    BdCommandFailed { exit_code: i32, stderr: String },

    #[error("bd command timed out after {seconds}s")]
    BdTimeout { seconds: u64 },

    #[error("invalid theme file: {0}")]
    InvalidTheme(String),

    #[error("disallowed external URL scheme: {0}")]
    DisallowedScheme(String),

    #[error("config storage error: {0}")]
    ConfigIo(String),

    #[error("internal error: {0}")]
    Internal(String),
}
```

Serialize `AppError` for Tauri IPC as:

```json
{
  "code": "BD_NOT_FOUND",
  "message": "bd executable not found",
  "detail": null
}
```

## Error Handling Rules

1. **No `.unwrap()` or `.expect()` in Runtime Paths**: Never panic on missing files, malformed JSONL lines, non-UTF8 stdout, or unavailable `git`/`gh` binaries. Reserve `.expect()` strictly for compile-time-verified static regexes or unit tests.
2. **Tolerant JSONL Parsing**: If a single line in `.beads/issues.jsonl` is partially written during a concurrent export, log a `tracing::warn!` and retry once after 100ms before failing.
3. **Actionable CLI Stderr**: Capture both `stdout` and `stderr` from `bd`, trim ANSI escape sequences, and return the concise root error line rather than raw stack traces.
4. **Never Leak Secrets**: When injecting `GITHUB_TOKEN` from `gh auth token` into a child `bd github sync` environment, never include the token value in any error message or log string.
