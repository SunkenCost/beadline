use crate::config::store::{normalize_display_path, ConfigStore};
use crate::engine::jsonl_reader::fnv1a64;
use crate::engine::workspace_mgr::{log_watch_warning, WorkspaceManager};
use crate::models::settings::AppSettings;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};
use tokio::task::JoinHandle;

pub const SNAPSHOT_UPDATED_EVENT: &str = "beadline://snapshot-updated";
pub const ALL_ACTIVE_UPDATED_EVENT: &str = "beadline://all-active-updated";

struct ActiveWatch {
    path: String,
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

impl Drop for ActiveWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Default)]
struct BackgroundChanges {
    paths: Mutex<BackgroundPaths>,
    ready: Notify,
}

#[derive(Default)]
struct BackgroundPaths {
    watched: HashMap<PathBuf, String>,
    dirty: HashSet<String>,
}

impl BackgroundChanges {
    fn native_watcher(self: &Arc<Self>) -> notify::Result<RecommendedWatcher> {
        let changes = Arc::clone(self);
        notify::recommended_watcher(move |result: notify::Result<Event>| {
            if let Ok(event) = result {
                changes.mark_event(event);
            }
        })
    }

    fn mark_event(&self, event: Event) {
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        if let Ok(mut paths) = self.paths.lock() {
            let was_empty = paths.dirty.is_empty();
            let BackgroundPaths { watched, dirty } = &mut *paths;
            for path in event.paths {
                for root in path.ancestors().skip(1) {
                    if let Some(workspace) = watched.get(root) {
                        if path.strip_prefix(root).is_ok_and(is_relevant_path) {
                            dirty.insert(workspace.clone());
                        }
                        break;
                    }
                }
            }
            if was_empty && !dirty.is_empty() {
                self.ready.notify_one();
            }
        }
    }

    fn take(&self) -> HashSet<String> {
        self.paths
            .lock()
            .map(|mut paths| std::mem::take(&mut paths.dirty))
            .unwrap_or_default()
    }
}

struct BackgroundWatch {
    watcher: RecommendedWatcher,
    changes: Arc<BackgroundChanges>,
    task: JoinHandle<()>,
}

impl Drop for BackgroundWatch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Default)]
struct WatchState {
    active: Option<ActiveWatch>,
    background: Option<BackgroundWatch>,
}

#[derive(Default)]
pub struct FsWatcherManager {
    state: Mutex<WatchState>,
}

impl FsWatcherManager {
    pub fn sync_workspaces(
        &self,
        app: &AppHandle,
        config_store: &ConfigStore,
        workspace_mgr: &Arc<WorkspaceManager>,
        settings: &Arc<AsyncMutex<AppSettings>>,
        fallback_active: Option<&str>,
    ) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        // Read the current registry after taking the lock so overlapping commands
        // never install watches using an older registry.
        let registry = config_store.load_workspaces();
        let active = registry
            .active_path
            .as_deref()
            .filter(|path| Path::new(path).is_dir())
            .or(fallback_active.filter(|_| !registry.entries.is_empty()))
            .map(normalize_display_path)
            .filter(|path| Path::new(path).join(".beads").is_dir());

        if active.as_deref() != state.active.as_ref().map(|watch| watch.path.as_str()) {
            state.active = None;
            if let Some(path) = &active {
                state.active = ActiveWatch::start(
                    path.clone(),
                    app.clone(),
                    Arc::clone(workspace_mgr),
                    Arc::clone(settings),
                );
            }
        }

        let desired: HashSet<_> = registry
            .entries
            .iter()
            .map(|entry| normalize_display_path(&entry.path))
            .filter(|path| Some(path) != active.as_ref() && Path::new(path).join(".beads").is_dir())
            .collect();

        if desired.is_empty() {
            state.background = None;
            return;
        }
        if state.background.is_none() {
            state.background = BackgroundWatch::start(
                app.clone(),
                Arc::clone(workspace_mgr),
                Arc::clone(settings),
            );
        }
        if state
            .background
            .as_mut()
            .is_some_and(|watch| !watch.sync(&desired))
        {
            state.background = None;
        }
    }

    pub fn shutdown(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = WatchState::default();
        }
    }
}

impl ActiveWatch {
    fn start(
        path: String,
        app: AppHandle,
        workspace_mgr: Arc<WorkspaceManager>,
        settings: Arc<AsyncMutex<AppSettings>>,
    ) -> Option<Self> {
        let mut last_fp = workspace_mgr.fingerprint_for(&path);
        let (tx, mut rx) = mpsc::channel(1);
        let watcher = watch_beads(&path, move || {
            let _ = tx.try_send(());
        })?;
        let watch_path = path.clone();
        let task = tokio::spawn(async move {
            while rx.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(250)).await;
                // Consume the trailing wake-up, if any, from the same burst.
                let _ = rx.try_recv();

                if workspace_mgr.fingerprint_for(&watch_path) == last_fp {
                    continue;
                }
                let cfg = settings.lock().await.clone();
                if let Ok(snap) = workspace_mgr.get_snapshot(&watch_path, &cfg, false).await {
                    if snap.fingerprint != last_fp {
                        last_fp.clone_from(&snap.fingerprint);
                        let _ = app.emit(SNAPSHOT_UPDATED_EVENT, &snap);
                    }
                }
            }
        });
        Some(Self {
            path,
            _watcher: watcher,
            task,
        })
    }
}

impl BackgroundWatch {
    fn start(
        app: AppHandle,
        workspace_mgr: Arc<WorkspaceManager>,
        settings: Arc<AsyncMutex<AppSettings>>,
    ) -> Option<Self> {
        let changes = Arc::new(BackgroundChanges::default());
        let pending = Arc::clone(&changes);
        let watcher = changes
            .native_watcher()
            .map_err(|err| {
                log_watch_warning(&format!("failed to create background watcher: {err}"))
            })
            .ok()?;
        // One debouncer for all background repos keeps a burst of edits from
        // triggering one complete cross-project scan per repository.
        let task = tokio::spawn(async move {
            let mut last_fps: HashMap<String, String> = HashMap::new();
            let mut last_payload_hash: Option<u64> = None;

            loop {
                pending.ready.notified().await;
                tokio::time::sleep(Duration::from_millis(350)).await;
                let paths = pending.take();
                if paths.is_empty() {
                    continue;
                }

                // Ignore spurious mtime-only touches (such as `bd --readonly` or self-exports)
                // whose content-based Dolt/JSONL fingerprint did not actually change.
                let mut changed_paths = Vec::new();
                for path in paths {
                    let fp = workspace_mgr.fingerprint_for(&path);
                    if last_fps.get(&path).is_some_and(|prev| prev == &fp) {
                        continue;
                    }
                    last_fps.insert(path.clone(), fp);
                    changed_paths.push(path);
                }
                if changed_paths.is_empty() {
                    continue;
                }

                let cfg = settings.lock().await.clone();
                for path in changed_paths {
                    workspace_mgr.refresh_stale_jsonl(path, &cfg).await;
                }
                if let Ok(items) = workspace_mgr.get_all_active(&cfg).await {
                    let payload_hash = serde_json::to_vec(&items).ok().map(|b| fnv1a64(&b));
                    if payload_hash.is_none() || payload_hash != last_payload_hash {
                        last_payload_hash = payload_hash;
                        let _ = app.emit(ALL_ACTIVE_UPDATED_EVENT, &items);
                    }
                }
            }
        });
        Some(Self {
            watcher,
            changes,
            task,
        })
    }

    fn sync(&mut self, desired: &HashSet<String>) -> bool {
        let Ok(mut paths) = self.changes.paths.lock() else {
            return false;
        };
        let removed: Vec<_> = paths
            .watched
            .iter()
            .filter(|(_, workspace)| !desired.contains(*workspace))
            .map(|(root, _)| root.clone())
            .collect();
        for root in &removed {
            paths.watched.remove(root);
        }
        paths.dirty.retain(|workspace| desired.contains(workspace));
        // notify can call the event handler while processing watch/unwatch.
        drop(paths);
        for root in removed {
            if let Err(err) = self.watcher.unwatch(&root) {
                log_watch_warning(&format!("failed to unwatch {}: {err}", root.display()));
            }
        }

        for path in desired {
            let root = Path::new(path).join(".beads");
            let Ok(mut paths) = self.changes.paths.lock() else {
                return false;
            };
            if paths.watched.contains_key(&root) {
                continue;
            }
            paths.watched.insert(root.clone(), path.clone());
            drop(paths);
            if let Err(err) = self.watcher.watch(&root, RecursiveMode::Recursive) {
                if let Ok(mut paths) = self.changes.paths.lock() {
                    paths.watched.remove(&root);
                    paths.dirty.remove(path);
                }
                log_watch_warning(&format!("failed to watch {}: {err}", root.display()));
            }
        }
        self.changes
            .paths
            .lock()
            .is_ok_and(|paths| !paths.watched.is_empty())
    }
}

fn watch_beads(path: &str, on_change: impl Fn() + Send + 'static) -> Option<RecommendedWatcher> {
    let beads_dir = PathBuf::from(path).join(".beads");
    let event_root = beads_dir.clone();
    let handler = move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result {
            // Reads can generate access events on some platforms. Never feed
            // our own JSONL reads back into either refresh loop.
            if !matches!(event.kind, EventKind::Access(_))
                && event
                    .paths
                    .iter()
                    .any(|path| path.strip_prefix(&event_root).is_ok_and(is_relevant_path))
            {
                on_change();
            }
        }
    };
    let result = notify::recommended_watcher(handler).and_then(|mut watcher| {
        watcher.watch(&beads_dir, RecursiveMode::Recursive)?;
        Ok(watcher)
    });
    match result {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            log_watch_warning(&format!("failed to watch {}: {err}", beads_dir.display()));
            None
        }
    }
}

// Accept paths relative to .beads, including nested Dolt manifests and HEADs.
fn is_relevant_path(relative: &Path) -> bool {
    if relative.components().any(|part| {
        let Some(name) = part.as_os_str().to_str() else {
            return false;
        };
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".lock")
            || lower.ends_with(".tmp")
            || lower == "lock"
            || lower == "tempfiles"
            || lower == "temptf"
    }) {
        return false;
    }
    relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "issues.jsonl" | "manifest" | "HEAD"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_jsonl_and_dolt_metadata() {
        for path in [
            "issues.jsonl",
            "manifest",
            "HEAD",
            "embeddeddolt/repo/.dolt/noms/manifest",
            "dolt/repo/.dolt/HEAD",
        ] {
            assert!(is_relevant_path(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn rejects_lock_temp_and_unrelated_paths() {
        for path in [
            "issues.jsonl.lock",
            "issues.jsonl.tmp",
            "dolt/repo/LOCK",
            "dolt/tempfiles/manifest",
            "dolt/TempFiles/HEAD",
            "dolt/repo/atomic.tmp/HEAD",
            "dolt/repo/manifest.lock/HEAD",
            "dolt/repo/noms/journal",
            "config.yaml",
        ] {
            assert!(!is_relevant_path(Path::new(path)), "{path}");
        }
    }

    #[tokio::test]
    async fn background_watcher_routes_non_active_files_and_drops_removed_roots() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = |name: &str| {
            let root = dir.path().join(name);
            std::fs::create_dir_all(root.join(".beads")).expect("beads dir");
            normalize_display_path(&root.to_string_lossy())
        };
        let first = workspace("first");
        let second = workspace("second");
        let changes = Arc::new(BackgroundChanges::default());
        let mut background = BackgroundWatch {
            watcher: changes.native_watcher().expect("native watcher"),
            changes: Arc::clone(&changes),
            task: tokio::spawn(std::future::pending()),
        };
        assert!(background.sync(&HashSet::from([first.clone(), second.clone()])));

        std::fs::write(Path::new(&first).join(".beads/issues.jsonl"), "first")
            .expect("first write");
        tokio::time::timeout(Duration::from_secs(5), changes.ready.notified())
            .await
            .expect("first workspace event");
        assert_eq!(changes.take(), HashSet::from([first.clone()]));

        assert!(background.sync(&HashSet::from([second.clone()])));
        std::fs::write(Path::new(&first).join(".beads/HEAD"), "removed").expect("removed write");
        std::fs::write(Path::new(&second).join(".beads/manifest"), "second").expect("second write");
        // The first write can generate multiple events. An existing wake-up
        // from the removed workspace may be observed with no pending work.
        let observed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                changes.ready.notified().await;
                let paths = changes.take();
                if !paths.is_empty() {
                    break paths;
                }
            }
        })
        .await
        .expect("retained workspace event");
        assert_eq!(observed, HashSet::from([second]));
    }
}
