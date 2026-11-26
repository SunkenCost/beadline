use crate::config::store::normalize_display_path;
use crate::error::{AppError, AppResult};
use crate::models::issue::WorkspaceSnapshot;
use crate::models::workspace::{ModuleAliasConfig, WorkspaceEntry, WorkspaceListResponse};
use crate::AppState;
use std::collections::BTreeMap;
use std::path::Path;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn list_workspaces(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<WorkspaceListResponse> {
    list_and_sync(&app, &state).await
}

async fn list_and_sync(app: &AppHandle, state: &AppState) -> AppResult<WorkspaceListResponse> {
    let settings = state.settings.lock().await.clone();
    let list = state.workspace_mgr.list_workspaces(&settings).await?;
    state.sync_workspace_watchers(app, list.active_path.as_deref());
    Ok(list)
}

#[tauri::command]
pub async fn add_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<WorkspaceListResponse> {
    let trimmed = path.trim();
    if trimmed.is_empty() || !Path::new(trimmed).is_dir() {
        return Err(AppError::WorkspaceNotFound(trimmed.to_string()));
    }
    state
        .config_store
        .upsert_workspace(WorkspaceEntry::new(trimmed.to_owned()), true)?;
    list_and_sync(&app, &state).await
}

#[tauri::command]
pub async fn select_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<WorkspaceSnapshot> {
    let norm = normalize_display_path(path.trim());
    if !Path::new(&norm).is_dir() {
        return Err(AppError::WorkspaceNotFound(norm));
    }
    state.config_store.set_active_workspace(&norm)?;
    state.sync_workspace_watchers(&app, None);

    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .get_snapshot(&norm, &settings, false)
        .await
}

#[tauri::command]
pub async fn remove_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<WorkspaceListResponse> {
    state.config_store.remove_workspace(path.trim())?;
    list_and_sync(&app, &state).await
}

#[tauri::command]
pub async fn update_workspace_config(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    module_prefix: String,
    module_aliases: BTreeMap<String, ModuleAliasConfig>,
    pinned: bool,
) -> AppResult<WorkspaceListResponse> {
    let norm = normalize_display_path(path.trim());
    if !Path::new(&norm).is_dir() {
        return Err(AppError::WorkspaceNotFound(norm));
    }
    let clean_prefix = if module_prefix.trim().is_empty() {
        "mod:".to_string()
    } else {
        module_prefix.trim().to_string()
    };
    let entry = WorkspaceEntry {
        path: norm,
        module_prefix: clean_prefix,
        module_aliases,
        pinned,
    };
    state.config_store.upsert_workspace(entry, false)?;
    list_and_sync(&app, &state).await
}

#[tauri::command]
pub async fn init_beads(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    prefix: Option<String>,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    let snap = state
        .workspace_mgr
        .init_beads(path.trim(), prefix, &settings)
        .await?;
    state.sync_workspace_watchers(&app, None);
    Ok(snap)
}
