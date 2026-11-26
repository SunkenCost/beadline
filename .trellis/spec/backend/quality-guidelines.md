# Backend Quality & Security Guidelines (`Beadline`)

> Verification gates, cross-platform Windows/macOS portability, and subprocess security rules.

## Mandatory Verification Commands

Run before completing any Rust/Tauri task:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Security Boundaries (Non-Negotiable)

1. **Zero Shell Injection**:
   - Always spawn `bd`, `git`, and `gh` directly with `tokio::process::Command::new(&binary).args(&args)`.
   - Never invoke `cmd /C`, `powershell -Command`, or `sh -c` with user-supplied arguments.
2. **Strict External Link Allowlist (`open_external_link`)**:
   - Before invoking OS opener on any link extracted from issue text or notes, validate the URI scheme against `https`, `http`, or the user's configured `settings.deepLinkSchemes` (default `["t3-thread", "vscode", "cursor"]`).
   - Always reject dangerous schemes (`file://`, `javascript:`, `ms-msdt:`, `vbscript:`, `data:`).
3. **Path Validation for Reveal/Worktrees**:
   - Only reveal directories that exist on disk and are canonicalized (`std::fs::canonicalize`).
4. **Minimal Tauri v2 Capabilities**:
   - Keep `src-tauri/capabilities/default.json` tightly scoped — do **not** enable broad shell-execute or unrestricted filesystem permissions for the webview; keep privileged filesystem and CLI access strictly inside Rust IPC commands.

## Cross-Platform Rules (Windows + macOS + Linux)

1. **Windows Console Suppression**:
   ```rust
   #[cfg(windows)]
   {
       const CREATE_NO_WINDOW: u32 = 0x08000000;
       cmd.creation_flags(CREATE_NO_WINDOW);
   }
   ```
   Required on every child process (`bd`, `git`, `gh`) so background exports and syncs never flash a terminal window.
2. **Path Normalization**:
   - Always use `Path` / `PathBuf` rather than string concatenation with `/` or `\`.
   - Normalize Windows extended-length prefixes (`\\?\`) when displaying paths to the UI or comparing workspace keys.
3. **Timeout Discipline**:
   - Every external CLI call must run under `tokio::time::timeout` (e.g., 20s for local `bd` commands, 60s for `bd github sync`) and kill on drop (`cmd.kill_on_drop(true)`).
4. **Unit Test Coverage Required**:
   - `jsonl_reader.rs`: lane derivation (`in_progress` / `ready` / `blocked` / `closed`), transitive blocker resolution, epic children rollup, module prefix extraction.
   - `smart_meta.rs`: branch/SHA/GitHub/deep-link extraction without false positives.
   - `config/store.rs`: round-trip serialization, default fallback, corrupted-file recovery.
