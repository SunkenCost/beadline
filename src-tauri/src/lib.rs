pub mod commands;
pub mod config;
pub mod engine;
pub mod error;
pub mod models;
pub mod watcher;

use crate::config::store::ConfigStore;
use crate::engine::workspace_mgr::WorkspaceManager;
use crate::models::settings::{AppSettings, WindowGeometry};
use crate::watcher::fs_watcher::FsWatcherManager;
use std::sync::Arc;
use tauri::{Manager, PhysicalPosition, PhysicalSize, WindowEvent};
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct AppState {
    pub config_store: ConfigStore,
    pub settings: Arc<Mutex<AppSettings>>,
    pub workspace_mgr: Arc<WorkspaceManager>,
    pub watcher: Arc<FsWatcherManager>,
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            tracing_subscriber::EnvFilter::new("beadline=debug,info")
        } else {
            tracing_subscriber::EnvFilter::new("warn")
        }
    });

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn restore_window_geometry(window: &tauri::WebviewWindow, geometry: Option<WindowGeometry>) {
    if let Some(geo) = geometry {
        let min_w = 960_u32;
        let min_h = 620_u32;
        let width = geo.width.max(min_w);
        let height = geo.height.max(min_h);

        let _ = window.set_size(PhysicalSize::new(width, height));

        let mut inside_monitor = false;
        if let Ok(monitors) = window.available_monitors() {
            for mon in monitors {
                let pos = mon.position();
                let size = mon.size();
                let in_x = geo.x >= pos.x - 100 && geo.x < pos.x + size.width as i32 - 120;
                let in_y = geo.y >= pos.y && geo.y < pos.y + size.height as i32 - 80;
                if in_x && in_y {
                    inside_monitor = true;
                    break;
                }
            }
        }

        if inside_monitor {
            let _ = window.set_position(PhysicalPosition::new(geo.x, geo.y));
        } else {
            let _ = window.center();
        }

        if geo.maximized {
            let _ = window.maximize();
        }
    } else {
        let _ = window.center();
    }

    let _ = window.show();
}

fn capture_window_geometry(window: &tauri::WebviewWindow) -> Option<WindowGeometry> {
    let maximized = window.is_maximized().ok()?;
    let size = window.outer_size().ok()?;
    let pos = window.outer_position().ok()?;
    if size.width < 400 || size.height < 300 {
        return None;
    }
    Some(WindowGeometry {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
        maximized,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let config_store = ConfigStore::new(config_dir)?;
            let initial_settings = config_store.load_settings();
            let saved_geometry = initial_settings.window;

            info!("Beadline initialized (v{})", env!("CARGO_PKG_VERSION"));

            let workspace_mgr = Arc::new(WorkspaceManager::new(config_store.clone()));
            let watcher = Arc::new(FsWatcherManager::new());

            let state = AppState {
                config_store: config_store.clone(),
                settings: Arc::new(Mutex::new(initial_settings)),
                workspace_mgr,
                watcher,
            };
            app.manage(state);

            if let Some(window) = app.get_webview_window("main") {
                restore_window_geometry(&window, saved_geometry);

                let last_normal_geo = Arc::new(std::sync::Mutex::new(
                    saved_geometry.filter(|g| !g.maximized),
                ));
                let store_for_events = config_store;
                let window_for_events = window.clone();

                window.on_window_event(move |event| match event {
                    WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                        if let Some(geo) = capture_window_geometry(&window_for_events) {
                            if !geo.maximized {
                                if let Ok(mut guard) = last_normal_geo.lock() {
                                    *guard = Some(geo);
                                }
                            }
                        }
                    }
                    WindowEvent::CloseRequested { .. } => {
                        if let Some(current) = capture_window_geometry(&window_for_events) {
                            let to_save = if current.maximized {
                                if let Ok(guard) = last_normal_geo.lock() {
                                    guard
                                        .map(|g| WindowGeometry {
                                            maximized: true,
                                            ..g
                                        })
                                        .unwrap_or(current)
                                } else {
                                    current
                                }
                            } else {
                                current
                            };
                            if let Err(err) = store_for_events.save_window_geometry(to_save) {
                                warn!("failed to persist window geometry on close: {err}");
                            }
                        }
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::get_platform_info,
            commands::workspaces::list_workspaces,
            commands::workspaces::add_workspace,
            commands::workspaces::select_workspace,
            commands::workspaces::remove_workspace,
            commands::workspaces::update_workspace_config,
            commands::workspaces::init_beads,
            commands::issues::get_snapshot,
            commands::issues::get_all_active,
            commands::issues::get_issue_comments,
            commands::issues::create_issue,
            commands::issues::update_issue,
            commands::issues::close_issue,
            commands::issues::reopen_issue,
            commands::issues::add_dependency,
            commands::issues::remove_dependency,
            commands::issues::add_comment,
            commands::sync::github_sync,
            commands::system::open_external_link,
            commands::system::reveal_path,
            commands::system::detect_bd_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Beadline");
}
