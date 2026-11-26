use crate::error::AppResult;
use crate::models::issue::{
    CommentDto, CreateIssueRequest, CreateIssueResponse, DoctorReport, RadarSnapshot,
    UpdateIssueRequest, WorkspaceSnapshot,
};
use crate::models::workspace::CrossProjectActiveItem;
use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace_path: String,
    force: Option<bool>,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    let snap = state
        .workspace_mgr
        .get_snapshot(&workspace_path, &settings, force.unwrap_or(false))
        .await?;
    state.sync_workspace_watchers(&app, Some(&snap.repo_path));
    Ok(snap)
}

#[tauri::command]
pub async fn get_radar_snapshot(
    state: State<'_, AppState>,
    workspace_path: String,
    scan_git: Option<bool>,
) -> AppResult<RadarSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .get_radar_snapshot(&workspace_path, &settings, scan_git.unwrap_or(true))
        .await
}

#[tauri::command]
pub async fn clear_session_waiting(
    state: State<'_, AppState>,
    workspace_path: String,
    session_id: String,
) -> AppResult<RadarSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .clear_session_waiting(&workspace_path, &session_id, &settings)
        .await
}

#[tauri::command]
pub async fn release_session(
    state: State<'_, AppState>,
    workspace_path: String,
    session_id: String,
) -> AppResult<RadarSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .release_session(&workspace_path, &session_id, &settings)
        .await
}

#[tauri::command]
pub async fn run_workspace_doctor(
    state: State<'_, AppState>,
    workspace_path: String,
) -> AppResult<DoctorReport> {
    state.workspace_mgr.run_doctor(&workspace_path).await
}

#[tauri::command]
pub async fn get_all_active(state: State<'_, AppState>) -> AppResult<Vec<CrossProjectActiveItem>> {
    let settings = state.settings.lock().await.clone();
    state.workspace_mgr.get_all_active(&settings).await
}

#[tauri::command]
pub async fn get_issue_comments(
    state: State<'_, AppState>,
    workspace_path: String,
    issue_id: String,
) -> AppResult<Vec<CommentDto>> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .get_issue_comments(&workspace_path, &issue_id, &settings)
        .await
}

#[tauri::command]
pub async fn create_issue(
    state: State<'_, AppState>,
    request: CreateIssueRequest,
) -> AppResult<CreateIssueResponse> {
    let settings = state.settings.lock().await.clone();
    state.workspace_mgr.create_issue(request, &settings).await
}

#[tauri::command]
pub async fn update_issue(
    state: State<'_, AppState>,
    request: UpdateIssueRequest,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state.workspace_mgr.update_issue(request, &settings).await
}

#[tauri::command]
pub async fn close_issue(
    state: State<'_, AppState>,
    workspace_path: String,
    issue_id: String,
    reason: Option<String>,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .close_issue(&workspace_path, &issue_id, reason, &settings)
        .await
}

#[tauri::command]
pub async fn reopen_issue(
    state: State<'_, AppState>,
    workspace_path: String,
    issue_id: String,
    reason: Option<String>,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .reopen_issue(&workspace_path, &issue_id, reason, &settings)
        .await
}

#[tauri::command]
pub async fn add_dependency(
    state: State<'_, AppState>,
    workspace_path: String,
    blocked_id: String,
    blocker_id: String,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .mutate_dependency(&workspace_path, &blocked_id, &blocker_id, true, &settings)
        .await
}

#[tauri::command]
pub async fn remove_dependency(
    state: State<'_, AppState>,
    workspace_path: String,
    blocked_id: String,
    blocker_id: String,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .mutate_dependency(&workspace_path, &blocked_id, &blocker_id, false, &settings)
        .await
}

#[tauri::command]
pub async fn add_comment(
    state: State<'_, AppState>,
    workspace_path: String,
    issue_id: String,
    text: String,
) -> AppResult<WorkspaceSnapshot> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .add_comment(&workspace_path, &issue_id, &text, &settings)
        .await
}
