use crate::engine::bd_cli::{configure_no_window, find_in_path, resolve_bd_binary, resolve_bd_with_source};
use crate::engine::bd_installer::{install_managed_bd as download_managed_bd, probe_bd_version};
use crate::error::{AppError, AppResult};
use crate::models::issue::{BlCliStatus, ManagedBdStatus};
use crate::AppState;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::process::Command;
use tokio::time::timeout;

const BLOCKED_SCHEMES: &[&str] = &["file", "javascript", "data", "vbscript", "ms-msdt", "cmd"];

#[tauri::command]
pub async fn open_external_link(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> AppResult<()> {
    let trimmed = url.trim();
    let Some((scheme_raw, _)) = trimmed.split_once("://") else {
        return Err(AppError::DisallowedScheme(trimmed.to_string()));
    };
    let scheme = scheme_raw.to_lowercase();
    if BLOCKED_SCHEMES.contains(&scheme.as_str()) {
        return Err(AppError::DisallowedScheme(scheme));
    }

    let settings = state.settings.lock().await;
    let allowed = scheme == "https"
        || scheme == "http"
        || settings
            .deep_link_schemes
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&scheme));
    if !allowed {
        return Err(AppError::DisallowedScheme(scheme));
    }

    app.opener()
        .open_url(trimmed, None::<&str>)
        .map_err(|e| AppError::Internal(format!("failed to open external URL: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn reveal_path(app: AppHandle, path: String) -> AppResult<()> {
    let p = Path::new(path.trim());
    if !p.exists() {
        return Err(AppError::WorkspaceNotFound(path));
    }
    let canon = std::fs::canonicalize(p)
        .map_err(|e| AppError::Internal(format!("failed to resolve path: {e}")))?;
    app.opener()
        .reveal_item_in_dir(canon)
        .map_err(|e| AppError::Internal(format!("failed to reveal path: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn detect_bd_path(state: State<'_, AppState>) -> AppResult<Option<String>> {
    let settings = state.settings.lock().await;
    let config_dir = state.config_store.config_dir();
    match resolve_bd_binary(settings.bd_path.as_deref(), Some(config_dir)) {
        Ok(p) => Ok(Some(p.to_string_lossy().to_string())),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn get_bd_status(state: State<'_, AppState>) -> AppResult<ManagedBdStatus> {
    let bd_path = state.settings.lock().await.bd_path.clone();
    let config_dir = state.config_store.config_dir().to_path_buf();
    let Some((path, source)) = resolve_bd_with_source(bd_path.as_deref(), Some(&config_dir)) else {
        return Ok(ManagedBdStatus {
            installed: false,
            path: None,
            source: None,
            version: None,
        });
    };
    let version = probe_bd_version(&path).await;
    Ok(ManagedBdStatus {
        installed: true,
        path: Some(path.to_string_lossy().to_string()),
        source: Some(source.to_string()),
        version,
    })
}

#[tauri::command]
pub async fn install_managed_bd(state: State<'_, AppState>) -> AppResult<ManagedBdStatus> {
    let config_dir = state.config_store.config_dir().to_path_buf();
    download_managed_bd(&config_dir).await
}

#[tauri::command]
pub async fn get_bl_status() -> AppResult<BlCliStatus> {
    let Some(path) = resolve_bl_binary() else {
        return Ok(BlCliStatus {
            installed: false,
            path: None,
            version: None,
        });
    };
    let version = probe_bl_version(&path).await;
    let display = beadline_core::fs_util::normalize_display_path(&path.to_string_lossy());
    Ok(BlCliStatus {
        installed: true,
        path: Some(display),
        version,
    })
}

fn resolve_bl_binary() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) { "bl.exe" } else { "bl" };

    // 1. Sibling binary right next to the running `beadline` executable (release & dev)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(bin_name);
            if sibling.is_file() {
                return Some(sibling);
            }
            // In `target/debug/beadline.exe`, also check `target/release/bl.exe`
            if let Some(target_root) = dir.parent() {
                let rel = target_root.join("release").join(bin_name);
                if rel.is_file() {
                    return Some(rel);
                }
            }
        }
    }

    // 2. PATH lookup
    if let Some(in_path) = find_in_path(bin_name) {
        return Some(in_path);
    }

    // 3. Common user bin directories
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    if let Some(h) = home {
        let home_dir = PathBuf::from(h);
        for rel in [".cargo/bin", ".local/bin", "bin"] {
            let cand = home_dir.join(rel).join(bin_name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }

    None
}

async fn probe_bl_version(path: &Path) -> Option<String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);
    let out = timeout(Duration::from_secs(2), cmd.output()).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
