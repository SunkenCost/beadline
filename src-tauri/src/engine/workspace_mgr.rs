use crate::config::store::{normalize_display_path, paths_equal, ConfigStore};
use crate::engine::bd_cli::{fetch_gh_auth_token, resolve_bd_binary, run_bd};
use crate::engine::git_meta::{detect_github_repo, list_git_worktrees, read_beads_config_values};
use crate::error::{AppError, AppResult};
use crate::models::issue::{
    BoardLane, CommentDto, CreateIssueRequest, CreateIssueResponse, DoctorReport,
    GithubSyncResponse, RadarSnapshot, StorageBackend, UpdateIssueRequest, WorkspaceSnapshot,
    WorktreeRef,
};
use crate::models::settings::AppSettings;
use crate::models::workspace::{
    CrossProjectActiveItem, ModuleAliasConfig, WorkspaceEntry, WorkspaceListResponse,
    WorkspaceSummary,
};
use beadline_core::git_radar::{compute_radar_snapshot_with_cache, ObservedWorktree};
use beadline_core::jsonl_reader::{
    compute_fingerprint, dolt_state_token, file_mtime_ms, latest_dolt_mtime_ms,
    read_workspace_snapshot_for_location,
};
use beadline_core::locator::{has_any_store, RepoLocator};
use beadline_core::native_store::{
    add_comment_native, close_issue_native, create_issue_native, derive_prefix,
    detect_storage_backend, init_native_workspace, mutate_dependency_native, read_inline_comments,
    reopen_issue_native, run_doctor as core_run_doctor, update_issue_native,
};
use beadline_core::runtime_store::{
    clear_waiting_human as core_clear_waiting_human, release_session_lease, runtime_fingerprint,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

const GIT_METADATA_TTL: Duration = Duration::from_secs(30);
const EXPORT_COALESCE_COOLDOWN: Duration = Duration::from_secs(5);
const EXPORT_FAILURE_BACKOFF: Duration = Duration::from_secs(30);
type GitMetadata = (Option<String>, Vec<WorktreeRef>);
type GitMetadataSlot = Arc<Mutex<Option<CachedGitMetadata>>>;

struct CachedGitMetadata {
    refreshed_at: Instant,
    value: GitMetadata,
}

#[derive(Debug, Clone)]
struct ExportRecord {
    exported_dolt_token: String,
    last_attempt: Instant,
    last_failed: bool,
}

struct CachedRadar {
    generation: u64,
    observed_worktrees: Vec<ObservedWorktree>,
    committed_diff_cache: HashMap<(String, String), Vec<String>>,
    last_snapshot: RadarSnapshot,
}

struct DiscoveredWorkspace {
    name: String,
    path: String,
    has_beads: bool,
    is_registered: bool,
    pinned: bool,
    module_prefix: String,
    module_aliases: BTreeMap<String, ModuleAliasConfig>,
}

pub struct WorkspaceManager {
    config_store: ConfigStore,
    write_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    export_in_flight: Mutex<HashMap<String, Arc<AtomicBool>>>,
    export_records: Mutex<HashMap<String, ExportRecord>>,
    git_metadata: Mutex<HashMap<String, GitMetadataSlot>>,
    /// Per-repo radar serialization lock so concurrent lease updates and Git scans
    /// never read stale `observed_worktrees` and overwrite a fresher scan.
    radar_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// Single global serialized Git scan worker lock (prevents parallel Git subprocess storms).
    git_scan_lock: Arc<Mutex<()>>,
    radar_cache: Mutex<HashMap<String, CachedRadar>>,
    radar_gen: AtomicU64,
}

impl WorkspaceManager {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            write_locks: Mutex::new(HashMap::new()),
            export_in_flight: Mutex::new(HashMap::new()),
            export_records: Mutex::new(HashMap::new()),
            git_metadata: Mutex::new(HashMap::new()),
            radar_locks: Mutex::new(HashMap::new()),
            git_scan_lock: Arc::new(Mutex::new(())),
            radar_cache: Mutex::new(HashMap::new()),
            radar_gen: AtomicU64::new(1),
        }
    }

    async fn radar_lock_for(&self, repo_id: &str) -> Arc<Mutex<()>> {
        let mut map = self.radar_locks.lock().await;
        map.entry(repo_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn resolve_bd(&self, settings: &AppSettings) -> AppResult<PathBuf> {
        resolve_bd_binary(
            settings.bd_path.as_deref(),
            Some(self.config_store.config_dir()),
        )
    }

    async fn lock_for(&self, norm_path: &str) -> Arc<Mutex<()>> {
        let mut map = self.write_locks.lock().await;
        map.entry(norm_path.to_lowercase())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn export_flag_for(&self, norm_path: &str) -> Arc<AtomicBool> {
        let mut map = self.export_in_flight.lock().await;
        map.entry(norm_path.to_lowercase())
            .or_insert_with(|| Arc::new(AtomicBool::new(false)))
            .clone()
    }

    fn discover_workspaces(&self) -> (Option<String>, Vec<DiscoveredWorkspace>) {
        let store_data = self.config_store.load_workspaces();
        let mut seen = HashSet::new();
        let mut found = Vec::new();

        // 1. Registered workspaces
        for entry in &store_data.entries {
            let norm = normalize_display_path(&entry.path);
            let key = norm.to_lowercase();
            let root = Path::new(&norm);
            if !root.is_dir() || !seen.insert(key) {
                continue;
            }
            let name = root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string();
            found.push(DiscoveredWorkspace {
                name,
                has_beads: has_any_store(root),
                path: norm,
                is_registered: true,
                pinned: entry.pinned,
                module_prefix: entry.module_prefix.clone(),
                module_aliases: entry.module_aliases.clone(),
            });
        }

        // 2. Auto-discover immediate sibling Git/Beads repositories of registered workspaces
        let mut parent_dirs = HashSet::new();
        for entry in &store_data.entries {
            if let Some(parent) = Path::new(&entry.path).parent() {
                parent_dirs.insert(parent.to_path_buf());
            }
        }
        for parent in parent_dirs {
            let Ok(rd) = fs::read_dir(&parent) else {
                continue;
            };
            for child in rd.flatten() {
                let p = child.path();
                if !p.is_dir() {
                    continue;
                }
                let has_git = p.join(".git").exists();
                let has_beads = p.join(".beads").is_dir() || (has_git && has_any_store(&p));
                if !has_beads && !has_git {
                    continue;
                }
                let norm = normalize_display_path(&p.to_string_lossy());
                if !seen.insert(norm.to_lowercase()) {
                    continue;
                }
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace")
                    .to_string();
                found.push(DiscoveredWorkspace {
                    name,
                    path: norm,
                    has_beads,
                    is_registered: false,
                    pinned: false,
                    module_prefix: "mod:".to_string(),
                    module_aliases: BTreeMap::new(),
                });
            }
        }

        // Sort: pinned desc, has_beads desc, registered desc, name asc
        found.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.has_beads.cmp(&a.has_beads))
                .then_with(|| b.is_registered.cmp(&a.is_registered))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let active = store_data
            .active_path
            .filter(|p| Path::new(p).is_dir())
            .or_else(|| {
                found
                    .iter()
                    .find(|s| s.has_beads)
                    .or_else(|| found.first())
                    .map(|s| s.path.clone())
            });

        (active, found)
    }

    pub async fn list_workspaces(
        &self,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceListResponse> {
        let (active, discovered) = self.discover_workspaces();
        let summaries = discovered
            .into_iter()
            .map(|ws| {
                self.summarize_workspace(
                    Path::new(&ws.path),
                    ws.path.clone(),
                    ws.is_registered,
                    ws.pinned,
                    &ws.module_prefix,
                    ws.module_aliases,
                    settings,
                )
            })
            .collect();

        Ok(WorkspaceListResponse {
            active_path: active,
            workspaces: summaries,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn summarize_workspace(
        &self,
        root: &Path,
        norm_path: String,
        is_registered: bool,
        pinned: bool,
        module_prefix: &str,
        module_aliases: BTreeMap<String, ModuleAliasConfig>,
        settings: &AppSettings,
    ) -> WorkspaceSummary {
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        let Ok(loc) = RepoLocator::locate(root) else {
            return WorkspaceSummary {
                name,
                path: norm_path,
                has_beads: false,
                storage_backend: StorageBackend::NativeJsonl,
                prefix: None,
                issue_count: 0,
                in_progress_count: 0,
                is_registered,
                pinned,
                module_prefix: module_prefix.to_string(),
                module_aliases,
            };
        };

        if !loc.has_store {
            return WorkspaceSummary {
                name,
                path: norm_path,
                has_beads: false,
                storage_backend: loc.storage_backend,
                prefix: None,
                issue_count: 0,
                in_progress_count: 0,
                is_registered,
                pinned,
                module_prefix: module_prefix.to_string(),
                module_aliases,
            };
        }

        let (prefix, _) = read_beads_config_values(root);
        let snap = read_workspace_snapshot_for_location(
            &loc,
            norm_path.clone(),
            module_prefix,
            None,
            &[],
            &settings.deep_link_schemes,
        )
        .ok();
        let (issue_count, in_progress_count) = snap
            .map(|s| (s.stats.total, s.stats.in_progress))
            .unwrap_or((0, 0));

        WorkspaceSummary {
            name,
            path: norm_path,
            has_beads: true,
            storage_backend: loc.storage_backend,
            prefix,
            issue_count,
            in_progress_count,
            is_registered,
            pinned,
            module_prefix: module_prefix.to_string(),
            module_aliases,
        }
    }

    pub async fn get_snapshot(
        self: &Arc<Self>,
        workspace_path: &str,
        settings: &AppSettings,
        force_export: bool,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        if !root.is_dir() {
            return Err(AppError::WorkspaceNotFound(norm));
        }

        let store_data = self.config_store.load_workspaces();
        let module_prefix = store_data
            .entries
            .iter()
            .find(|e| paths_equal(&e.path, &norm))
            .map(|e| e.module_prefix.clone())
            .unwrap_or_else(|| "mod:".to_string());

        let loc_root = root.clone();
        let loc = tokio::task::spawn_blocking(move || RepoLocator::locate(&loc_root))
            .await
            .map_err(|e| AppError::Internal(format!("locator task join error: {e}")))??;

        if loc.has_store && loc.storage_backend == StorageBackend::DoltCli {
            let beads_dir = loc.store_dir.clone();
            let jsonl = loc.issues_jsonl_path();
            if !jsonl.exists() || force_export {
                let _ = self.export_jsonl_locked(&root, &norm, settings).await;
            } else if self
                .should_background_export(&norm, &beads_dir, &jsonl)
                .await
            {
                self.schedule_background_export(root.clone(), norm.clone(), settings.clone())
                    .await;
            }
        }

        let (gh_repo, worktrees) = self
            .cached_git_metadata(&norm, force_export, || async {
                tokio::join!(detect_github_repo(&root), list_git_worktrees(&root))
            })
            .await;

        let schemes = settings.deep_link_schemes.clone();
        tokio::task::spawn_blocking(move || {
            read_workspace_snapshot_for_location(
                &loc,
                norm,
                &module_prefix,
                gh_repo,
                &worktrees,
                &schemes,
            )
        })
        .await
        .map_err(|e| AppError::Internal(format!("snapshot join error: {e}")))?
        .map_err(Into::into)
    }

    /// Computes or updates the independent `RadarSnapshot` (sessions, worktrees, conflicts,
    /// root blockers, attention queue, and scan coverage).
    ///
    /// When `scan_git` is false (e.g. on live lease heartbeat events), reuses cached Git
    /// worktree observations with ZERO Git subprocesses!
    pub async fn get_radar_snapshot(
        self: &Arc<Self>,
        workspace_path: &str,
        settings: &AppSettings,
        scan_git: bool,
    ) -> AppResult<RadarSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        if !root.is_dir() {
            return Err(AppError::WorkspaceNotFound(norm));
        }

        let store_data = self.config_store.load_workspaces();
        let module_prefix = store_data
            .entries
            .iter()
            .find(|e| paths_equal(&e.path, &norm))
            .map(|e| e.module_prefix.clone())
            .unwrap_or_else(|| "mod:".to_string());

        let loc_root = root.clone();
        let loc = tokio::task::spawn_blocking(move || RepoLocator::locate(&loc_root))
            .await
            .map_err(|e| AppError::Internal(format!("locator join error: {e}")))??;

        let cache_key = loc.repo_id.clone();
        // Serialize radar computations per repo_id so a lease-only refresh never clobbers
        // a concurrent full Git scan's fresher `observed_worktrees` or emits out-of-order generations.
        let repo_radar_lock = self.radar_lock_for(&cache_key).await;
        let _repo_guard = repo_radar_lock.lock().await;

        let (prev_observed, mut committed_cache) = {
            let cache = self.radar_cache.lock().await;
            match cache.get(&cache_key) {
                Some(c) => (Some(c.observed_worktrees.clone()), c.committed_diff_cache.clone()),
                None => (None, HashMap::new()),
            }
        };

        let effective_scan_git = scan_git || prev_observed.is_none();
        let generation = self.radar_gen.fetch_add(1, Ordering::Relaxed);
        let schemes = settings.deep_link_schemes.clone();
        let display = norm.clone();

        // Serialize Git scans through one global worker lock to prevent subprocess storms
        let _git_guard = if effective_scan_git {
            Some(self.git_scan_lock.lock().await)
        } else {
            None
        };

        let (radar, observed, next_committed_cache) = tokio::task::spawn_blocking(move || {
            let issues = if loc.has_store {
                read_workspace_snapshot_for_location(
                    &loc,
                    display,
                    &module_prefix,
                    None,
                    &[],
                    &schemes,
                )
                .map(|s| s.issues)
                .unwrap_or_default()
            } else {
                Vec::new()
            };
            let (radar, observed) = compute_radar_snapshot_with_cache(
                &loc,
                &issues,
                generation,
                effective_scan_git,
                prev_observed.as_deref(),
                &mut committed_cache,
            );
            // Bound cache size so long-lived sessions stay lean
            if committed_cache.len() > 256 {
                committed_cache.clear();
            }
            (radar, observed, committed_cache)
        })
        .await
        .map_err(|e| AppError::Internal(format!("radar compute join error: {e}")))?;

        {
            let mut cache = self.radar_cache.lock().await;
            let should_write = cache
                .get(&cache_key)
                .is_none_or(|existing| generation >= existing.generation);
            if should_write {
                cache.insert(
                    cache_key,
                    CachedRadar {
                        generation,
                        observed_worktrees: observed,
                        committed_diff_cache: next_committed_cache,
                        last_snapshot: radar.clone(),
                    },
                );
            }
        }

        Ok(radar)
    }

    pub async fn clear_session_waiting(
        self: &Arc<Self>,
        workspace_path: &str,
        session_id: &str,
        settings: &AppSettings,
    ) -> AppResult<RadarSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let sid = session_id.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            let loc = RepoLocator::locate(&root)?;
            let _ = core_clear_waiting_human(&loc.runtime_dir, &sid)?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("clear waiting join error: {e}")))??;

        self.get_radar_snapshot(&norm, settings, false).await
    }

    pub async fn release_session(
        self: &Arc<Self>,
        workspace_path: &str,
        session_id: &str,
        settings: &AppSettings,
    ) -> AppResult<RadarSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let sid = session_id.to_string();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            let loc = RepoLocator::locate(&root)?;
            let _ = release_session_lease(&loc.runtime_dir, &sid)?;
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("release session join error: {e}")))??;

        self.get_radar_snapshot(&norm, settings, false).await
    }

    pub async fn run_doctor(&self, workspace_path: &str) -> AppResult<DoctorReport> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        tokio::task::spawn_blocking(move || core_run_doctor(&root, env!("CARGO_PKG_VERSION")))
            .await
            .map_err(|e| AppError::Internal(format!("doctor join error: {e}")))?
            .map_err(Into::into)
    }

    async fn cached_git_metadata<F, Fut>(
        &self,
        normalized_path: &str,
        force_refresh: bool,
        load: F,
    ) -> GitMetadata
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = GitMetadata>,
    {
        let slot = {
            let mut cache = self.git_metadata.lock().await;
            Arc::clone(cache.entry(normalized_path.to_lowercase()).or_default())
        };
        let mut entry = slot.lock().await;
        if let Some(cached) = entry
            .as_ref()
            .filter(|cached| !force_refresh && cached.refreshed_at.elapsed() < GIT_METADATA_TTL)
        {
            return cached.value.clone();
        }

        let value = load().await;
        *entry = Some(CachedGitMetadata {
            refreshed_at: Instant::now(),
            value: value.clone(),
        });
        value
    }

    pub(crate) async fn should_background_export(
        &self,
        norm: &str,
        beads_dir: &Path,
        jsonl: &Path,
    ) -> bool {
        let key = norm.to_lowercase();
        let dolt_token = dolt_state_token(beads_dir);

        if !jsonl.exists() {
            let records = self.export_records.lock().await;
            if let Some(rec) = records.get(&key) {
                if rec.last_failed && rec.last_attempt.elapsed() < EXPORT_FAILURE_BACKOFF {
                    return false;
                }
            }
            return true;
        }

        if dolt_token.is_empty() {
            return false;
        }

        let dolt_mt = latest_dolt_mtime_ms(beads_dir);
        let jsonl_mt = file_mtime_ms(jsonl);
        if dolt_mt <= jsonl_mt + 500 {
            let mut records = self.export_records.lock().await;
            records
                .entry(key)
                .and_modify(|rec| {
                    rec.exported_dolt_token.clone_from(&dolt_token);
                    rec.last_failed = false;
                })
                .or_insert_with(|| ExportRecord {
                    exported_dolt_token: dolt_token,
                    last_attempt: Instant::now(),
                    last_failed: false,
                });
            return false;
        }

        let records = self.export_records.lock().await;
        if let Some(rec) = records.get(&key) {
            if rec.exported_dolt_token == dolt_token {
                return false;
            }
            let cooldown = if rec.last_failed {
                EXPORT_FAILURE_BACKOFF
            } else {
                EXPORT_COALESCE_COOLDOWN
            };
            if rec.last_attempt.elapsed() < cooldown {
                return false;
            }
        }

        true
    }

    pub(crate) async fn refresh_stale_jsonl(
        self: &Arc<Self>,
        normalized_path: String,
        settings: &AppSettings,
    ) {
        let root = PathBuf::from(&normalized_path);
        if detect_storage_backend(&root) != StorageBackend::DoltCli {
            return;
        }
        let beads_dir = root.join(".beads");
        let jsonl = beads_dir.join("issues.jsonl");
        if beads_dir.is_dir()
            && self
                .should_background_export(&normalized_path, &beads_dir, &jsonl)
                .await
        {
            self.schedule_background_export(root, normalized_path, settings.clone())
                .await;
        }
    }

    async fn schedule_background_export(
        self: &Arc<Self>,
        root: PathBuf,
        norm: String,
        settings: AppSettings,
    ) {
        let flag = self.export_flag_for(&norm).await;
        if flag
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let mgr = Arc::clone(self);
        tokio::spawn(async move {
            let _ = mgr.export_jsonl_locked(&root, &norm, &settings).await;
            flag.store(false, Ordering::SeqCst);
        });
    }

    async fn record_export_success(&self, norm: &str, beads_dir: &Path) {
        let token = dolt_state_token(beads_dir);
        let mut records = self.export_records.lock().await;
        records.insert(
            norm.to_lowercase(),
            ExportRecord {
                exported_dolt_token: token,
                last_attempt: Instant::now(),
                last_failed: false,
            },
        );
    }

    async fn export_jsonl_already_locked(&self, bd_bin: &Path, root: &Path, norm: &str) {
        let key = norm.to_lowercase();
        let beads_dir = root.join(".beads");
        let args = vec![
            "--sandbox".into(),
            "export".into(),
            "-o".into(),
            ".beads/issues.jsonl".into(),
        ];
        match run_bd(bd_bin, root, &args, None, 30).await {
            Ok(_) => {
                self.record_export_success(norm, &beads_dir).await;
            }
            Err(err) => {
                let mut records = self.export_records.lock().await;
                records
                    .entry(key)
                    .and_modify(|rec| {
                        rec.last_attempt = Instant::now();
                        rec.last_failed = true;
                    })
                    .or_insert_with(|| ExportRecord {
                        exported_dolt_token: String::new(),
                        last_attempt: Instant::now(),
                        last_failed: true,
                    });
                debug!("bd export skipped/failed for {norm}: {err}");
            }
        }
    }

    async fn export_jsonl_locked(
        &self,
        root: &Path,
        norm: &str,
        settings: &AppSettings,
    ) -> AppResult<()> {
        let Ok(bd_bin) = self.resolve_bd(settings) else {
            return Ok(());
        };
        let lock = self.lock_for(norm).await;
        let _guard = lock.lock().await;
        self.export_jsonl_already_locked(&bd_bin, root, norm).await;
        Ok(())
    }

    pub async fn get_all_active(
        self: &Arc<Self>,
        settings: &AppSettings,
    ) -> AppResult<Vec<CrossProjectActiveItem>> {
        let (_, discovered) = self.discover_workspaces();
        let mut results = Vec::new();

        for ws in discovered.into_iter().filter(|w| w.has_beads) {
            if let Ok(loc) = RepoLocator::locate(Path::new(&ws.path)) {
                if let Ok(snap) = read_workspace_snapshot_for_location(
                    &loc,
                    ws.path.clone(),
                    &ws.module_prefix,
                    None,
                    &[],
                    &settings.deep_link_schemes,
                ) {
                    for issue in snap.issues {
                        if issue.lane == BoardLane::InProgress && issue.issue_type != "epic" {
                            results.push(CrossProjectActiveItem {
                                workspace_name: ws.name.clone(),
                                workspace_path: ws.path.clone(),
                                issue,
                            });
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| {
            a.issue
                .priority
                .cmp(&b.issue.priority)
                .then_with(|| b.issue.updated_at.cmp(&a.issue.updated_at))
        });

        Ok(results)
    }

    pub async fn init_beads(
        self: &Arc<Self>,
        workspace_path: &str,
        prefix: Option<String>,
        mode: Option<&str>,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        if !root.is_dir() {
            return Err(AppError::WorkspaceNotFound(norm));
        }

        let slug = prefix
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| derive_prefix(&root));

        let use_dolt = matches!(mode, Some("dolt"));

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if use_dolt {
                let bd_bin = self.resolve_bd(settings)?;
                let init_args = vec![
                    "init".into(),
                    "--prefix".into(),
                    slug,
                    "--setup-exclude".into(),
                    "--skip-hooks".into(),
                    "--skip-agents".into(),
                    "--non-interactive".into(),
                ];
                run_bd(&bd_bin, &root, &init_args, None, 30).await?;

                let cfg_args = vec![
                    "config".into(),
                    "set".into(),
                    "export.auto".into(),
                    "true".into(),
                ];
                let _ = run_bd(&bd_bin, &root, &cfg_args, None, 15).await;

                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            } else {
                let r = root.clone();
                let s = slug.clone();
                tokio::task::spawn_blocking(move || init_native_workspace(&r, &s))
                    .await
                    .map_err(|e| AppError::Internal(format!("init join error: {e}")))??;
            }
        }

        self.config_store
            .upsert_workspace(WorkspaceEntry::new(norm.clone()), true)?;
        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn get_issue_comments(
        &self,
        workspace_path: &str,
        issue_id: &str,
        settings: &AppSettings,
    ) -> AppResult<Vec<CommentDto>> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);

        if detect_storage_backend(&root) == StorageBackend::NativeJsonl {
            let r = root.clone();
            let iid = issue_id.to_string();
            return tokio::task::spawn_blocking(move || read_inline_comments(&r, &iid))
                .await
                .map_err(|e| AppError::Internal(format!("comments join error: {e}")))?
                .map_err(Into::into);
        }

        if let Ok(inline) = read_inline_comments(&root, issue_id) {
            if !inline.is_empty() {
                return Ok(inline);
            }
        }

        let bd_bin = self.resolve_bd(settings)?;
        let lock = self.lock_for(&norm).await;
        let _guard = lock.lock().await;
        let args = vec![
            "--readonly".into(),
            "comments".into(),
            issue_id.to_string(),
            "--json".into(),
        ];
        let out = run_bd(&bd_bin, &root, &args, None, 15).await?;
        let trimmed = out.stdout.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            return Ok(Vec::new());
        };
        let Some(arr) = val.as_array() else {
            return Ok(Vec::new());
        };
        let mut comments = Vec::new();
        for (idx, item) in arr.iter().enumerate() {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("c-{idx}"));
            let author = item
                .get("author")
                .and_then(|v| v.as_str())
                .unwrap_or("note")
                .to_string();
            let text = item
                .get("text")
                .or_else(|| item.get("body"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let created_at = item
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            comments.push(CommentDto {
                id,
                author,
                text,
                created_at,
            });
        }
        Ok(comments)
    }

    pub async fn create_issue(
        self: &Arc<Self>,
        req: CreateIssueRequest,
        settings: &AppSettings,
    ) -> AppResult<CreateIssueResponse> {
        let norm = normalize_display_path(&req.workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        let created_id = {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let req_clone = req.clone();
                tokio::task::spawn_blocking(move || {
                    let (cfg_prefix, _) = read_beads_config_values(&r);
                    let prefix = cfg_prefix.unwrap_or_else(|| derive_prefix(&r));
                    create_issue_native(&r, &req_clone, &prefix)
                })
                .await
                .map_err(|e| AppError::Internal(format!("create join error: {e}")))??
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let mut args = vec![
                    "create".into(),
                    req.title.trim().to_string(),
                    "-t".into(),
                    req.issue_type.clone(),
                    "-p".into(),
                    req.priority.to_string(),
                    "--json".into(),
                ];
                if let Some(parent) = req
                    .parent_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    args.push("--parent".into());
                    args.push(parent.to_string());
                }
                if let Some(assignee) = req
                    .assignee
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    args.push("--assignee".into());
                    args.push(assignee.to_string());
                }
                let labels: Vec<String> = req
                    .labels
                    .iter()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                if !labels.is_empty() {
                    args.push("-l".into());
                    args.push(labels.join(","));
                }
                if let Some(desc) = req
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    args.push("-d".into());
                    args.push(desc.to_string());
                }
                if let Some(acc) = req
                    .acceptance_criteria
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    args.push("--acceptance".into());
                    args.push(acc.to_string());
                }
                if let Some(notes) = req
                    .notes
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    args.push("--notes".into());
                    args.push(notes.to_string());
                }

                let out = run_bd(&bd_bin, &root, &args, None, 20).await?;
                let parsed: serde_json::Value =
                    serde_json::from_str(out.stdout.trim()).map_err(|e| {
                        AppError::Internal(format!("failed to parse bd create json: {e}"))
                    })?;
                let id = parsed
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AppError::Internal("bd create returned no id".into()))?
                    .to_string();

                if req.status != "open" && !req.status.is_empty() {
                    let mut st_args = vec![
                        "update".into(),
                        id.clone(),
                        "--status".into(),
                        req.status.clone(),
                    ];
                    if settings.use_force_writes {
                        st_args.push("--force".into());
                    }
                    run_bd(&bd_bin, &root, &st_args, None, 20).await?;
                }

                for blocker in &req.blocked_by {
                    let b = blocker.trim();
                    if !b.is_empty() && b != id {
                        let dep_args = vec!["dep".into(), "add".into(), id.clone(), b.to_string()];
                        let _ = run_bd(&bd_bin, &root, &dep_args, None, 15).await;
                    }
                }

                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;

                id
            }
        };

        let snapshot = self.get_snapshot(&norm, settings, false).await?;
        Ok(CreateIssueResponse {
            id: created_id,
            snapshot,
        })
    }

    pub async fn update_issue(
        self: &Arc<Self>,
        req: UpdateIssueRequest,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(&req.workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let req_clone = req.clone();
                tokio::task::spawn_blocking(move || update_issue_native(&r, &req_clone))
                    .await
                    .map_err(|e| AppError::Internal(format!("update join error: {e}")))??;
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let mut args = vec!["update".into(), req.issue_id.clone()];
                if settings.use_force_writes {
                    args.push("--force".into());
                }
                if let Some(title) = req.title {
                    args.push("--title".into());
                    args.push(title);
                }
                if let Some(status) = req.status {
                    args.push("--status".into());
                    args.push(status);
                }
                if let Some(priority) = req.priority {
                    args.push("-p".into());
                    args.push(priority.to_string());
                }
                if let Some(assignee) = req.assignee {
                    args.push("--assignee".into());
                    args.push(assignee);
                }
                if let Some(labels) = req.labels {
                    args.push("--set-labels".into());
                    args.push(labels.join(","));
                }
                if let Some(desc) = req.description {
                    args.push("-d".into());
                    args.push(desc);
                }
                if let Some(acc) = req.acceptance_criteria {
                    args.push("--acceptance".into());
                    args.push(acc);
                }
                if let Some(notes) = req.notes {
                    args.push("--notes".into());
                    args.push(notes);
                }

                run_bd(&bd_bin, &root, &args, None, 20).await?;
                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            }
        }

        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn close_issue(
        self: &Arc<Self>,
        workspace_path: &str,
        issue_id: &str,
        reason: Option<String>,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let iid = issue_id.to_string();
                let rsn = reason.clone();
                tokio::task::spawn_blocking(move || close_issue_native(&r, &iid, rsn.as_deref()))
                    .await
                    .map_err(|e| AppError::Internal(format!("close join error: {e}")))??;
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let mut args = vec!["close".into(), issue_id.to_string()];
                if settings.use_force_writes {
                    args.push("--force".into());
                }
                if let Some(r) = reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    args.push("--reason".into());
                    args.push(r.to_string());
                }
                run_bd(&bd_bin, &root, &args, None, 20).await?;
                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            }
        }

        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn reopen_issue(
        self: &Arc<Self>,
        workspace_path: &str,
        issue_id: &str,
        reason: Option<String>,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let iid = issue_id.to_string();
                let rsn = reason.clone();
                tokio::task::spawn_blocking(move || reopen_issue_native(&r, &iid, rsn.as_deref()))
                    .await
                    .map_err(|e| AppError::Internal(format!("reopen join error: {e}")))??;
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let mut args = vec!["reopen".into(), issue_id.to_string()];
                if let Some(r) = reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    args.push("--reason".into());
                    args.push(r.to_string());
                }
                let res = run_bd(&bd_bin, &root, &args, None, 20).await;
                if res.is_err() {
                    let mut fb = vec![
                        "update".into(),
                        issue_id.to_string(),
                        "--status".into(),
                        "open".into(),
                    ];
                    if settings.use_force_writes {
                        fb.push("--force".into());
                    }
                    run_bd(&bd_bin, &root, &fb, None, 20).await?;
                }
                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            }
        }

        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn mutate_dependency(
        self: &Arc<Self>,
        workspace_path: &str,
        blocked_id: &str,
        blocker_id: &str,
        add: bool,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let b1 = blocked_id.to_string();
                let b2 = blocker_id.to_string();
                tokio::task::spawn_blocking(move || mutate_dependency_native(&r, &b1, &b2, add))
                    .await
                    .map_err(|e| AppError::Internal(format!("dep join error: {e}")))??;
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let args = vec![
                    "dep".into(),
                    if add { "add" } else { "remove" }.into(),
                    blocked_id.to_string(),
                    blocker_id.to_string(),
                ];
                run_bd(&bd_bin, &root, &args, None, 20).await?;
                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            }
        }

        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn add_comment(
        self: &Arc<Self>,
        workspace_path: &str,
        issue_id: &str,
        text: &str,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        let backend = detect_storage_backend(&root);

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            if backend == StorageBackend::NativeJsonl {
                let r = root.clone();
                let iid = issue_id.to_string();
                let txt = text.to_string();
                tokio::task::spawn_blocking(move || add_comment_native(&r, &iid, &txt))
                    .await
                    .map_err(|e| AppError::Internal(format!("comment join error: {e}")))??;
            } else {
                let bd_bin = self.resolve_bd(settings)?;
                let args = vec![
                    "comments".into(),
                    "add".into(),
                    issue_id.to_string(),
                    text.trim().to_string(),
                ];
                run_bd(&bd_bin, &root, &args, None, 20).await?;
                self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                    .await;
            }
        }

        self.get_snapshot(&norm, settings, false).await
    }

    pub async fn github_sync(
        self: &Arc<Self>,
        workspace_path: &str,
        pull_only: bool,
        settings: &AppSettings,
    ) -> AppResult<GithubSyncResponse> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        if detect_storage_backend(&root) != StorageBackend::DoltCli {
            return Err(AppError::InvalidOperation(
                "GitHub sync is a legacy Dolt (bd) command and is disabled in native shared mode"
                    .into(),
            ));
        }
        let bd_bin = self.resolve_bd(settings)?;

        let mut env_map = HashMap::new();
        if let Some(token) = fetch_gh_auth_token().await {
            env_map.insert("GITHUB_TOKEN".to_string(), token);
        }

        let lock = self.lock_for(&norm).await;
        let output_text = {
            let _guard = lock.lock().await;
            let mut args = vec!["github".into(), "sync".into()];
            if pull_only {
                args.push("--pull-only".into());
                args.push("--prefer-local".into());
            }
            let out = run_bd(
                &bd_bin,
                &root,
                &args,
                if env_map.is_empty() {
                    None
                } else {
                    Some(&env_map)
                },
                60,
            )
            .await?;

            self.export_jsonl_already_locked(&bd_bin, &root, &norm)
                .await;

            out.stdout.trim().to_string()
        };

        let snapshot = self.get_snapshot(&norm, settings, false).await?;
        Ok(GithubSyncResponse {
            output: output_text,
            snapshot,
        })
    }

    pub fn fingerprint_for(&self, workspace_path: &str) -> String {
        let norm = normalize_display_path(workspace_path);
        let store_data = self.config_store.load_workspaces();
        let module_prefix = store_data
            .entries
            .iter()
            .find(|e| paths_equal(&e.path, &norm))
            .map(|e| e.module_prefix.as_str())
            .unwrap_or("mod:");
        let store_dir = RepoLocator::locate(Path::new(&norm))
            .map(|l| l.store_dir)
            .unwrap_or_else(|_| PathBuf::from(&norm).join(".beads"));
        compute_fingerprint(&store_dir, module_prefix)
    }

    pub fn runtime_fingerprint_for(&self, workspace_path: &str) -> String {
        let norm = normalize_display_path(workspace_path);
        let rt_dir = RepoLocator::locate(Path::new(&norm))
            .map(|l| l.runtime_dir)
            .unwrap_or_else(|_| PathBuf::from(&norm).join(".beads/runtime/pulses"));
        runtime_fingerprint(&rt_dir)
    }

    pub fn cached_radar_snapshot(&self, repo_id: &str) -> Option<RadarSnapshot> {
        self.radar_cache
            .try_lock()
            .ok()
            .and_then(|c| c.get(repo_id).map(|r| r.last_snapshot.clone()))
    }
}

pub fn log_watch_warning(msg: &str) {
    warn!("{msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, WorkspaceManager) {
        let dir = tempfile::tempdir().expect("tempdir");
        let mgr = WorkspaceManager::new(ConfigStore::new(dir.path().into()).expect("config"));
        (dir, mgr)
    }

    fn git_metadata(repo: &str) -> GitMetadata {
        (
            Some(repo.into()),
            vec![WorktreeRef {
                path: "workspace".into(),
                branch: repo.into(),
            }],
        )
    }

    #[tokio::test]
    async fn git_metadata_cache_hits_the_same_workspace_key() {
        let (_dir, mgr) = fixture();
        let expected = git_metadata("owner/cached");
        mgr.cached_git_metadata("Workspace", false, || async { expected.clone() })
            .await;
        let result = mgr
            .cached_git_metadata("workspace", false, || async { panic!("cache miss") })
            .await;
        assert_eq!(result, expected);
        assert_eq!(
            mgr.cached_git_metadata("other-workspace", false, || async { (None, Vec::new()) })
                .await,
            (None, Vec::new())
        );
    }

    #[tokio::test]
    async fn git_metadata_cache_expires_after_ttl() {
        let (_dir, mgr) = fixture();
        mgr.cached_git_metadata("workspace", false, || async { git_metadata("owner/old") })
            .await;
        let slot = mgr.git_metadata.lock().await["workspace"].clone();
        slot.lock()
            .await
            .as_mut()
            .expect("cached value")
            .refreshed_at -= GIT_METADATA_TTL;
        let expected = git_metadata("owner/fresh");
        let result = mgr
            .cached_git_metadata("workspace", false, || async { expected.clone() })
            .await;
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn git_metadata_force_refresh_replaces_a_fresh_entry() {
        let (_dir, mgr) = fixture();
        mgr.cached_git_metadata("workspace", false, || async { git_metadata("owner/old") })
            .await;
        let expected = git_metadata("owner/fresh");
        let result = mgr
            .cached_git_metadata("workspace", true, || async { expected.clone() })
            .await;
        assert_eq!(result, expected);
        assert_eq!(
            mgr.cached_git_metadata("workspace", false, || async { panic!("cache miss") })
                .await,
            expected
        );
    }

    #[tokio::test]
    async fn should_background_export_skips_already_exported_token_and_backs_off_on_failure() {
        let (dir, mgr) = fixture();
        let ws = dir.path().join("repo");
        let beads = ws.join(".beads");
        let noms = beads.join("embeddeddolt/bl/.dolt/noms");
        std::fs::create_dir_all(&noms).expect("mkdir");
        let jsonl = beads.join("issues.jsonl");
        let manifest = noms.join("manifest");
        std::fs::write(&jsonl, "{}\n").expect("write jsonl");
        std::thread::sleep(Duration::from_millis(650));
        std::fs::write(&manifest, "5:__DOLT__:lock1:root_v1:0000:vvvv:1").expect("write manifest");

        let ws_str = ws.to_string_lossy().to_string();
        assert!(mgr.should_background_export(&ws_str, &beads, &jsonl).await);

        let token = dolt_state_token(&beads);
        mgr.export_records.lock().await.insert(
            ws_str.to_lowercase(),
            ExportRecord {
                exported_dolt_token: token,
                last_attempt: Instant::now(),
                last_failed: false,
            },
        );
        assert!(!mgr.should_background_export(&ws_str, &beads, &jsonl).await);

        std::fs::write(&manifest, "5:__DOLT__:lock2:root_v2:0000:vvvv:2")
            .expect("write manifest 2");
        mgr.export_records.lock().await.insert(
            ws_str.to_lowercase(),
            ExportRecord {
                exported_dolt_token: String::new(),
                last_attempt: Instant::now(),
                last_failed: true,
            },
        );
        assert!(!mgr.should_background_export(&ws_str, &beads, &jsonl).await);
    }

    #[tokio::test]
    async fn native_workspace_manager_operates_completely_without_bd_binary() {
        let (dir, mgr) = fixture();
        let mgr = Arc::new(mgr);
        let repo = dir.path().join("standalone-app");
        std::fs::create_dir_all(&repo).expect("mkdir");
        let repo_str = repo.to_string_lossy().to_string();

        let settings = AppSettings {
            bd_path: Some("/nonexistent/bd".into()),
            ..Default::default()
        };

        let snap = mgr
            .init_beads(&repo_str, Some("app".into()), Some("native"), &settings)
            .await
            .expect("native init");
        assert!(snap.has_beads);
        assert_eq!(snap.storage_backend, StorageBackend::NativeJsonl);

        let created = mgr
            .create_issue(
                CreateIssueRequest {
                    workspace_path: repo_str.clone(),
                    title: "Standalone native issue".into(),
                    issue_type: "task".into(),
                    priority: 1,
                    status: "open".into(),
                    assignee: Some("agent".into()),
                    parent_id: None,
                    labels: vec!["mod:ui".into()],
                    description: Some("Created with zero external bd".into()),
                    acceptance_criteria: None,
                    notes: None,
                    scope: vec!["src/ui/**".into()],
                    blocked_by: vec![],
                },
                &settings,
            )
            .await
            .expect("native create");
        assert_eq!(created.snapshot.stats.total, 1);

        let after_comment = mgr
            .add_comment(&repo_str, &created.id, "Looks great", &settings)
            .await
            .expect("native comment");
        assert_eq!(after_comment.issues[0].comment_count, 1);

        let comments = mgr
            .get_issue_comments(&repo_str, &created.id, &settings)
            .await
            .expect("read comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "Looks great");

        let closed = mgr
            .close_issue(&repo_str, &created.id, Some("Done".into()), &settings)
            .await
            .expect("native close");
        assert_eq!(closed.stats.closed, 1);
    }
}
