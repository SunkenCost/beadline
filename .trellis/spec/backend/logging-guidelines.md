# Backend Logging Guidelines (`Beadline`)

> Lightweight, structured diagnostics via `tracing` without noisy stdout spam or leaked credentials.

## Framework & Levels

Use `tracing` (with `tracing-subscriber` env filter, defaulting to `info` in dev and `warn` in release builds):

| Level | When to Use |
|-------|-------------|
| `error` | Unrecoverable config write failure, watcher startup failure, unexpected panic boundary |
| `warn` | Malformed JSONL line retried, `bd export` timeout/fallback failure, missing optional `gh` CLI |
| `info` | App boot, resolved `bd` binary path, workspace watcher attached/detached |
| `debug` | `bd` subcommand name + duration (ms), snapshot rebuild time (ms), fingerprint transitions |
| `trace` | Raw filesystem watcher events (off by default) |

## Strict Redaction Rules

1. **Never log `GITHUB_TOKEN`**, `gh auth token` output, or full environment variable dumps.
2. **Never log full issue descriptions/notes** at `info`/`warn` level (user repositories may contain private business data); log only issue IDs, counts, and durations.
3. **No `println!` / `dbg!`** in committed Rust code — always use `tracing::{debug, info, warn, error}!`.
