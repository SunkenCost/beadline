use crate::error::AppResult;
use crate::models::issue::GithubSyncResponse;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn github_sync(
    state: State<'_, AppState>,
    workspace_path: String,
    pull_only: Option<bool>,
) -> AppResult<GithubSyncResponse> {
    let settings = state.settings.lock().await.clone();
    state
        .workspace_mgr
        .github_sync(&workspace_path, pull_only.unwrap_or(true), &settings)
        .await
}
