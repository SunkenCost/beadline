use crate::config::store::{normalize_display_path, ConfigStore};
use crate::engine::workspace_mgr::{log_watch_warning, WorkspaceManager};
use crate::models::settings::AppSettings;
use beadline_core::fs_util::fnv1a64;
use beadline_core::locator::{has_any_store, RepoLocator};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify};
use tokio::task::JoinHandle;

pub const SNAPSHOT_UPDATED_EVENT: &str = "beadline://snapshot-updated";
pub const RADAR_UPDATED_EVENT: &str = "beadline://radar-updated";
pub const ALL_ACTIVE_UPDATED_EVENT: &str = "beadline://all-active-updated";

struct ActiveWatch {
    path: String,
    _store_watcher: RecommendedWatcher,
    _runtime_watcher: Option<RecommendedWatcher>,
    _git_meta_watcher: Option<RecommendedWatcher>,
    snapshot_task: JoinHandle<()>,
    radar_task: JoinHandle<()>,
    git_radar_task: JoinHandle<()>,
}

impl Drop for ActiveWatch {
    fn drop(&mut self) {
        self.snapshot_task.abort();
        self.radar_task.abort();
        self.git_radar_task.abort();
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
                        if path.strip_prefix(root).is_ok_and(is_relevant_store_path) {
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
        let registry = config_store.load_workspaces();
        let active = registry
            .active_path
            .as_deref()
            .filter(|path| Path::new(path).is_dir())
            .or(fallback_active.filter(|_| !registry.entries.is_empty()))
            .map(normalize_display_path)
            .filter(|path| has_any_store(Path::new(path)));

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
            .filter(|path| Some(path) != active.as_ref() && has_any_store(Path::new(path)))
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
        let loc = RepoLocator::locate(Path::new(&path)).ok()?;
        let store_dir = loc.store_dir.clone();
        let runtime_dir = loc.runtime_dir.clone();
        if !store_dir.is_dir() {
            return None;
        }
        // Ensure per-session runtime directory exists so live agent pulses are detected immediately
        let _ = fs::create_dir_all(&runtime_dir);

        let mut last_fp = workspace_mgr.fingerprint_for(&path);
        let mut last_rt_fp = workspace_mgr.runtime_fingerprint_for(&path);

        let (snap_tx, mut snap_rx) = mpsc::channel(1);
        let (radar_tx, mut radar_rx) = mpsc::channel(1);
        let (git_tx, mut git_rx) = mpsc::channel(1);

        let runtime_inside_store = runtime_dir.starts_with(&store_dir);

        let snap_tx_cb = snap_tx.clone();
        let radar_tx_cb = radar_tx.clone();
        let store_watcher = watch_store_dir(
            &store_dir,
            move || {
                let _ = snap_tx_cb.try_send(());
            },
            move || {
                let _ = radar_tx_cb.try_send(());
            },
        )?;

        let runtime_watcher = if !runtime_inside_store && runtime_dir.is_dir() {
            let radar_tx_ext = radar_tx.clone();
            watch_runtime_dir(&runtime_dir, move || {
                let _ = radar_tx_ext.try_send(());
            })
        } else {
            None
        };

        // Lightweight Git ref & worktree metadata watcher (ONLY refs/heads + worktrees + HEAD,
        // never recursive over source files) so commits & branch switches update committed-diff radar!
        let git_meta_watcher = loc.git_common_dir.as_deref().and_then(|common| {
            let git_tx_cb = git_tx.clone();
            watch_git_refs(common, move || {
                let _ = git_tx_cb.try_send(());
            })
        });

        let watch_path_snap = path.clone();
        let mgr_snap = Arc::clone(&workspace_mgr);
        let settings_snap = Arc::clone(&settings);
        let app_snap = app.clone();
        let snapshot_task = tokio::spawn(async move {
            while snap_rx.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let _ = snap_rx.try_recv();

                if mgr_snap.fingerprint_for(&watch_path_snap) == last_fp {
                    continue;
                }
                let cfg = settings_snap.lock().await.clone();
                if let Ok(snap) = mgr_snap.get_snapshot(&watch_path_snap, &cfg, false).await {
                    if snap.fingerprint != last_fp {
                        last_fp.clone_from(&snap.fingerprint);
                        let _ = app_snap.emit(SNAPSHOT_UPDATED_EVENT, &snap);
                    }
                }
            }
        });

        // Independent lease channel: NEVER triggers full Board snapshot or Git subprocesses!
        let watch_path_radar = path.clone();
        let mgr_radar = Arc::clone(&workspace_mgr);
        let settings_radar = Arc::clone(&settings);
        let app_radar = app.clone();
        let radar_task = tokio::spawn(async move {
            while radar_rx.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let _ = radar_rx.try_recv();

                let cur_rt = mgr_radar.runtime_fingerprint_for(&watch_path_radar);
                if cur_rt == last_rt_fp {
                    continue;
                }
                last_rt_fp = cur_rt;
                let cfg = settings_radar.lock().await.clone();
                if let Ok(radar) = mgr_radar
                    .get_radar_snapshot(&watch_path_radar, &cfg, false)
                    .await
                {
                    let _ = app_radar.emit(RADAR_UPDATED_EVENT, &radar);
                }
            }
        });

        // Debounced Git ref/worktree refresh channel (updates committed + dirty overlap truth on commit/branch switch)
        let watch_path_git = path.clone();
        let git_radar_task = tokio::spawn(async move {
            while git_rx.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(450)).await;
                let _ = git_rx.try_recv();
                let cfg = settings.lock().await.clone();
                if let Ok(radar) = workspace_mgr
                    .get_radar_snapshot(&watch_path_git, &cfg, true)
                    .await
                {
                    let _ = app.emit(RADAR_UPDATED_EVENT, &radar);
                }
            }
        });

        Some(Self {
            path,
            _store_watcher: store_watcher,
            _runtime_watcher: runtime_watcher,
            _git_meta_watcher: git_meta_watcher,
            snapshot_task,
            radar_task,
            git_radar_task,
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
        drop(paths);
        for root in removed {
            if let Err(err) = self.watcher.unwatch(&root) {
                log_watch_warning(&format!("failed to unwatch {}: {err}", root.display()));
            }
        }

        for path in desired {
            let root = RepoLocator::locate(Path::new(path))
                .map(|l| l.store_dir)
                .unwrap_or_else(|_| Path::new(path).join(".beads"));
            if !root.is_dir() {
                continue;
            }
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

fn watch_store_dir(
    store_dir: &Path,
    on_store_change: impl Fn() + Send + 'static,
    on_runtime_change: impl Fn() + Send + 'static,
) -> Option<RecommendedWatcher> {
    let event_root = store_dir.to_path_buf();
    let handler = move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result {
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let mut hit_store = false;
            let mut hit_runtime = false;
            for p in &event.paths {
                if let Ok(rel) = p.strip_prefix(&event_root) {
                    if is_relevant_store_path(rel) {
                        hit_store = true;
                    } else if is_relevant_pulse_path(rel) {
                        hit_runtime = true;
                    }
                }
            }
            if hit_store {
                on_store_change();
            }
            if hit_runtime {
                on_runtime_change();
            }
        }
    };
    let result = notify::recommended_watcher(handler).and_then(|mut watcher| {
        watcher.watch(store_dir, RecursiveMode::Recursive)?;
        Ok(watcher)
    });
    match result {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            log_watch_warning(&format!("failed to watch {}: {err}", store_dir.display()));
            None
        }
    }
}

fn watch_git_refs(
    git_common_dir: &Path,
    on_git_change: impl Fn() + Send + 'static,
) -> Option<RecommendedWatcher> {
    let refs_heads = git_common_dir.join("refs").join("heads");
    let worktrees_dir = git_common_dir.join("worktrees");
    let root_clone = git_common_dir.to_path_buf();

    let handler = move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result {
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            let relevant = event.paths.iter().any(|p| {
                let Ok(rel) = p.strip_prefix(&root_clone) else {
                    return false;
                };
                if is_ignored_temp_or_lock(rel) {
                    return false;
                }
                let text = rel.to_string_lossy().replace('\\', "/");
                text == "HEAD"
                    || text == "index"
                    || text.starts_with("refs/heads/")
                    || (text.starts_with("worktrees/")
                        && (text.ends_with("/HEAD") || text.ends_with("/index")))
            });
            if relevant {
                on_git_change();
            }
        }
    };

    let mut watcher = notify::recommended_watcher(handler).ok()?;
    let mut attached = false;
    if watcher
        .watch(git_common_dir, RecursiveMode::NonRecursive)
        .is_ok()
    {
        attached = true;
    }
    if refs_heads.is_dir() && watcher.watch(&refs_heads, RecursiveMode::Recursive).is_ok() {
        attached = true;
    }
    if worktrees_dir.is_dir() && watcher.watch(&worktrees_dir, RecursiveMode::Recursive).is_ok() {
        attached = true;
    }
    if attached { Some(watcher) } else { None }
}

fn watch_runtime_dir(
    runtime_dir: &Path,
    on_runtime_change: impl Fn() + Send + 'static,
) -> Option<RecommendedWatcher> {
    let event_root = runtime_dir.to_path_buf();
    let handler = move |result: Result<Event, notify::Error>| {
        if let Ok(event) = result {
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            if event
                .paths
                .iter()
                .any(|p| p.strip_prefix(&event_root).is_ok_and(is_relevant_pulse_file))
            {
                on_runtime_change();
            }
        }
    };
    let result = notify::recommended_watcher(handler).and_then(|mut watcher| {
        watcher.watch(runtime_dir, RecursiveMode::NonRecursive)?;
        Ok(watcher)
    });
    match result {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            log_watch_warning(&format!(
                "failed to watch runtime dir {}: {err}",
                runtime_dir.display()
            ));
            None
        }
    }
}

fn is_ignored_temp_or_lock(relative: &Path) -> bool {
    relative.components().any(|part| {
        let Some(name) = part.as_os_str().to_str() else {
            return false;
        };
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".lock")
            || lower.ends_with(".tmp")
            || lower == "lock"
            || lower == "tempfiles"
            || lower == "temptf"
    })
}

fn is_relevant_store_path(relative: &Path) -> bool {
    if is_ignored_temp_or_lock(relative) {
        return false;
    }
    relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "issues.jsonl" | "manifest" | "HEAD"))
}

fn is_relevant_pulse_path(relative: &Path) -> bool {
    if is_ignored_temp_or_lock(relative) {
        return false;
    }
    let text = relative.to_string_lossy().replace('\\', "/");
    if !text.starts_with("runtime/pulses/") {
        return false;
    }
    is_relevant_pulse_file(relative)
}

fn is_relevant_pulse_file(relative: &Path) -> bool {
    if is_ignored_temp_or_lock(relative) {
        return false;
    }
    relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| !name.starts_with('.') && name.ends_with(".json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_store_and_runtime_pulse_events() {
        for path in [
            "issues.jsonl",
            "manifest",
            "HEAD",
            "embeddeddolt/repo/.dolt/noms/manifest",
        ] {
            assert!(is_relevant_store_path(Path::new(path)), "{path}");
            assert!(!is_relevant_pulse_path(Path::new(path)), "{path}");
        }

        assert!(is_relevant_pulse_path(Path::new("runtime/pulses/ses_abc.json")));
        assert!(!is_relevant_store_path(Path::new("runtime/pulses/ses_abc.json")));
        assert!(!is_relevant_pulse_path(Path::new("runtime/pulses/.ses_abc.lock")));
        assert!(!is_relevant_pulse_path(Path::new("runtime/pulses/.ses_abc.tmp")));
    }

    #[test]
    fn rejects_lock_temp_and_unrelated_paths() {
        for path in [
            "issues.jsonl.lock",
            "issues.jsonl.tmp",
            "dolt/repo/LOCK",
            "dolt/tempfiles/manifest",
            "dolt/TempFiles/HEAD",
            "config.yaml",
        ] {
            assert!(!is_relevant_store_path(Path::new(path)), "{path}");
        }
    }
}
