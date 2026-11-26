use crate::engine::bd_installer::managed_bd_binary_path;
use crate::error::{AppError, AppResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn configure_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

pub fn resolve_bd_with_source(
    explicit_path: Option<&str>,
    config_dir: Option<&Path>,
) -> Option<(PathBuf, &'static str)> {
    if let Some(raw) = explicit_path {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_file() {
                return Some((p, "custom"));
            }
        }
    }

    if let Ok(env_bin) = std::env::var("BD_BIN") {
        let trimmed = env_bin.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            if p.is_file() {
                return Some((p, "env"));
            }
        }
    }

    if let Some(dir) = config_dir {
        let managed = managed_bd_binary_path(dir);
        if managed.is_file() {
            return Some((managed, "managed"));
        }
    }

    if let Some(found) = find_in_path(if cfg!(windows) { "bd.exe" } else { "bd" }) {
        return Some((found, "system"));
    }

    // Platform fallback locations (WinGet, Scoop, Cargo, Homebrew)
    #[cfg(windows)]
    {
        if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
            let pkg_root = Path::new(&local_app)
                .join("Microsoft")
                .join("WinGet")
                .join("Packages");
            if let Ok(entries) = std::fs::read_dir(&pkg_root) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("GasTownHall.Beads_") {
                        let candidate = entry.path().join("bd.exe");
                        if candidate.is_file() {
                            return Some((candidate, "system"));
                        }
                    }
                }
            }
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            for rel in [
                "scoop\\shims\\bd.exe",
                ".cargo\\bin\\bd.exe",
                ".local\\bin\\bd.exe",
            ] {
                let candidate = Path::new(&profile).join(rel);
                if candidate.is_file() {
                    return Some((candidate, "system"));
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        for abs in ["/opt/homebrew/bin/bd", "/usr/local/bin/bd"] {
            let candidate = PathBuf::from(abs);
            if candidate.is_file() {
                return Some((candidate, "system"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            for rel in [".local/bin/bd", ".cargo/bin/bd", "go/bin/bd"] {
                let candidate = Path::new(&home).join(rel);
                if candidate.is_file() {
                    return Some((candidate, "system"));
                }
            }
        }
    }

    None
}

pub fn resolve_bd_binary(
    explicit_path: Option<&str>,
    config_dir: Option<&Path>,
) -> AppResult<PathBuf> {
    resolve_bd_with_source(explicit_path, config_dir)
        .map(|(p, _)| p)
        .ok_or(AppError::BdNotFound)
}

pub fn find_in_path(exe_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_bd(
    bd_bin: &Path,
    cwd: &Path,
    args: &[String],
    extra_env: Option<&HashMap<String, String>>,
    timeout_secs: u64,
) -> AppResult<CmdOutput> {
    let subcmd = args.first().map(String::as_str).unwrap_or("bd");
    let start = std::time::Instant::now();

    let mut cmd = Command::new(bd_bin);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    if let Some(env_map) = extra_env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let output = timeout(Duration::from_secs(timeout_secs), cmd.output())
        .await
        .map_err(|_| AppError::BdTimeout {
            seconds: timeout_secs,
        })?
        .map_err(|e| AppError::Internal(format!("failed to spawn bd: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));

    debug!(
        subcmd,
        ms = start.elapsed().as_millis() as u64,
        ok = output.status.success(),
        "bd finished"
    );

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        let msg = stderr
            .lines()
            .find(|l| !l.trim().is_empty())
            .or_else(|| stdout.lines().find(|l| !l.trim().is_empty()))
            .unwrap_or("unknown bd error")
            .trim()
            .to_string();
        return Err(AppError::BdCommandFailed {
            exit_code,
            stderr: msg,
        });
    }

    Ok(CmdOutput { stdout, stderr })
}

pub async fn fetch_gh_auth_token() -> Option<String> {
    if let Ok(tok) = std::env::var("GITHUB_TOKEN") {
        let trimmed = tok.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let gh_bin = find_in_path(if cfg!(windows) { "gh.exe" } else { "gh" })?;
    let mut cmd = Command::new(gh_bin);
    cmd.args(["auth", "token"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    let out = timeout(Duration::from_secs(5), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                let _ = chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
