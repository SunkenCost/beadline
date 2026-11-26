use crate::config::store::normalize_display_path;
use crate::engine::workspace_mgr::{log_watch_warning, WorkspaceManager};
use crate::models::settings::AppSettings;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info};

pub const SNAPSHOT_UPDATED_EVENT: &str = "beadline://snapshot-updated";

struct ActiveWatch {
    path: String,
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

pub struct FsWatcherManager {
    active: Mutex<Option<ActiveWatch>>,
}

impl Default for FsWatcherManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FsWatcherManager {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub async fn watch_workspace(
        &self,
        app: AppHandle,
        workspace_path: String,
        workspace_mgr: Arc<WorkspaceManager>,
        settings: Arc<Mutex<AppSettings>>,
    ) {
        let norm = normalize_display_path(&workspace_path);
        let mut guard = self.active.lock().await;
        if let Some(prev) = guard.as_ref() {
            if prev.path.eq_ignore_ascii_case(&norm) {
                return;
            }
        }
        if let Some(prev) = guard.take() {
            prev.task.abort();
            debug!(path = %prev.path, "detached previous workspace watcher");
        }

        let beads_dir = PathBuf::from(&norm).join(".beads");
        if !beads_dir.is_dir() {
            return;
        }

        let (tx, mut rx) = mpsc::channel::<()>(16);

        let watcher_res = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(ev) = res {
                let relevant = ev.paths.iter().any(|p| {
                    let name = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if name.ends_with(".lock")
                        || name.ends_with(".tmp")
                        || name == "lock"
                        || name.contains("tempfiles")
                    {
                        return false;
                    }
                    name == "issues.jsonl" || name == "manifest" || name == "head"
                });
                if relevant {
                    let _ = tx.try_send(());
                }
            }
        });

        let mut watcher = match watcher_res {
            Ok(w) => w,
            Err(err) => {
                log_watch_warning(&format!("failed to create notify watcher: {err}"));
                return;
            }
        };

        if let Err(err) = watcher.watch(&beads_dir, RecursiveMode::Recursive) {
            log_watch_warning(&format!("failed to watch {}: {err}", beads_dir.display()));
            return;
        }

        info!(path = %norm, "watching .beads directory");

        let watch_path = norm.clone();
        let task = tokio::spawn(async move {
            let mut last_fp = workspace_mgr.fingerprint_for(&watch_path);

            while rx.recv().await.is_some() {
                // Coalesce bursty filesystem events over a 250ms debounce window
                tokio::time::sleep(Duration::from_millis(250)).await;
                while rx.try_recv().is_ok() {}

                let current_fp = workspace_mgr.fingerprint_for(&watch_path);
                if current_fp == last_fp {
                    continue;
                }
                last_fp = current_fp;

                let cfg = settings.lock().await.clone();
                if let Ok(snap) = workspace_mgr.get_snapshot(&watch_path, &cfg, false).await {
                    let _ = app.emit(SNAPSHOT_UPDATED_EVENT, &snap);
                }
            }
        });

        *guard = Some(ActiveWatch {
            path: norm,
            _watcher: watcher,
            task,
        });
    }
}
