use crate::error::AppResult;
use crate::models::settings::{AppSettings, PlatformInfo};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    let guard = state.settings.lock().await;
    Ok(guard.clone())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    mut settings: AppSettings,
) -> AppResult<AppSettings> {
    let mut guard = state.settings.lock().await;
    // Preserve backend-managed window geometry when frontend updates preferences.
    if settings.window.is_none() {
        settings.window = guard.window;
    }
    state.config_store.save_settings(&settings)?;
    *guard = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub fn get_platform_info() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        version: env!("CARGO_PKG_VERSION"),
    }
}
