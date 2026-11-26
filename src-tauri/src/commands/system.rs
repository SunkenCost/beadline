use crate::engine::bd_cli::resolve_bd_binary;
use crate::error::{AppError, AppResult};
use crate::AppState;
use std::path::Path;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

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
    match resolve_bd_binary(settings.bd_path.as_deref()) {
        Ok(p) => Ok(Some(p.to_string_lossy().to_string())),
        Err(_) => Ok(None),
    }
}
