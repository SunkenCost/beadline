use crate::config::store::{normalize_display_path, paths_equal, ConfigStore};
use crate::engine::bd_cli::{fetch_gh_auth_token, resolve_bd_binary, run_bd};
use crate::engine::git_meta::{detect_github_repo, list_git_worktrees, read_beads_config_values};
use crate::engine::jsonl_reader::{
    compute_fingerprint, file_mtime_ms, latest_dolt_mtime_ms, read_workspace_snapshot,
};
use crate::error::{AppError, AppResult};
use crate::models::issue::{
    BoardLane, CommentDto, CreateIssueRequest, CreateIssueResponse, GithubSyncResponse,
    UpdateIssueRequest, WorkspaceSnapshot, WorktreeRef,
};
use crate::models::settings::AppSettings;
use crate::models::workspace::{
    CrossProjectActiveItem, ModuleAliasConfig, WorkspaceEntry, WorkspaceListResponse,
    WorkspaceSummary,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

const GIT_METADATA_TTL: Duration = Duration::from_secs(30);
type GitMetadata = (Option<String>, Vec<WorktreeRef>);
type GitMetadataSlot = Arc<Mutex<Option<CachedGitMetadata>>>;

struct CachedGitMetadata {
    refreshed_at: Instant,
    value: GitMetadata,
}

pub struct WorkspaceManager {
    config_store: ConfigStore,
    write_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    export_in_flight: Mutex<HashMap<String, Arc<AtomicBool>>>,
    git_metadata: Mutex<HashMap<String, GitMetadataSlot>>,
}

impl WorkspaceManager {
    pub fn new(config_store: ConfigStore) -> Self {
        Self {
            config_store,
            write_locks: Mutex::new(HashMap::new()),
            export_in_flight: Mutex::new(HashMap::new()),
            git_metadata: Mutex::new(HashMap::new()),
        }
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

    pub async fn list_workspaces(
        &self,
        settings: &AppSettings,
    ) -> AppResult<WorkspaceListResponse> {
        let store_data = self.config_store.load_workspaces();
        let mut seen = HashSet::new();
        let mut summaries = Vec::new();

        // 1. Registered workspaces
        for entry in &store_data.entries {
            let norm = normalize_display_path(&entry.path);
            let key = norm.to_lowercase();
            if !Path::new(&norm).is_dir() || !seen.insert(key) {
                continue;
            }
            summaries.push(self.summarize_workspace(
                Path::new(&norm),
                norm.clone(),
                true,
                entry.pinned,
                &entry.module_prefix,
                entry.module_aliases.clone(),
                settings,
            ));
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
                let has_beads = p.join(".beads").is_dir();
                let has_git = p.join(".git").exists();
                if !has_beads && !has_git {
                    continue;
                }
                let norm = normalize_display_path(&p.to_string_lossy());
                if !seen.insert(norm.to_lowercase()) {
                    continue;
                }
                summaries.push(self.summarize_workspace(
                    &p,
                    norm,
                    false,
                    false,
                    "mod:",
                    BTreeMap::new(),
                    settings,
                ));
            }
        }

        // Sort: pinned desc, has_beads desc, registered desc, name asc
        summaries.sort_by(|a, b| {
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
                summaries
                    .iter()
                    .find(|s| s.has_beads)
                    .or_else(|| summaries.first())
                    .map(|s| s.path.clone())
            });

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
        let has_beads = root.join(".beads").is_dir();
        if !has_beads {
            return WorkspaceSummary {
                name,
                path: norm_path,
                has_beads: false,
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
        let snap = read_workspace_snapshot(
            root,
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

        let beads_dir = root.join(".beads");
        if beads_dir.is_dir() {
            let jsonl = beads_dir.join("issues.jsonl");
            let dolt_mt = latest_dolt_mtime_ms(&beads_dir);
            let jsonl_mt = file_mtime_ms(&jsonl);
            let needs_export = force_export || !jsonl.exists() || (dolt_mt > jsonl_mt + 250);

            if needs_export {
                if !jsonl.exists() || force_export {
                    // Blocking quick export when JSONL does not exist yet or user explicitly requested force refresh
                    let _ = self.export_jsonl_locked(&root, &norm, settings).await;
                } else {
                    // Coalesced non-blocking background export so UI stays <25ms
                    self.schedule_background_export(root.clone(), norm.clone(), settings.clone())
                        .await;
                }
            }
        }

        let (gh_repo, worktrees) = self
            .cached_git_metadata(&norm, force_export, || async {
                tokio::join!(detect_github_repo(&root), list_git_worktrees(&root))
            })
            .await;

        read_workspace_snapshot(
            &root,
            norm,
            &module_prefix,
            gh_repo,
            &worktrees,
            &settings.deep_link_schemes,
        )
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
        // Only serialize refreshes of the same workspace, not Git reads in other repos.
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

    pub(crate) async fn refresh_stale_jsonl(
        self: &Arc<Self>,
        normalized_path: String,
        settings: &AppSettings,
    ) {
        let root = PathBuf::from(&normalized_path);
        let beads_dir = root.join(".beads");
        let jsonl = beads_dir.join("issues.jsonl");
        if beads_dir.is_dir()
            && (!jsonl.exists() || latest_dolt_mtime_ms(&beads_dir) > file_mtime_ms(&jsonl) + 250)
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

    async fn export_jsonl_locked(
        &self,
        root: &Path,
        norm: &str,
        settings: &AppSettings,
    ) -> AppResult<()> {
        let Ok(bd_bin) = resolve_bd_binary(settings.bd_path.as_deref()) else {
            return Ok(());
        };
        let lock = self.lock_for(norm).await;
        let _guard = lock.lock().await;
        let args = vec![
            "--sandbox".into(),
            "export".into(),
            "-o".into(),
            ".beads/issues.jsonl".into(),
        ];
        if let Err(err) = run_bd(&bd_bin, root, &args, None, 15).await {
            debug!("background bd export skipped/failed for {norm}: {err}");
        }
        Ok(())
    }

    pub async fn get_all_active(
        self: &Arc<Self>,
        settings: &AppSettings,
    ) -> AppResult<Vec<CrossProjectActiveItem>> {
        let list = self.list_workspaces(settings).await?;
        let mut results = Vec::new();

        for ws in list.workspaces.into_iter().filter(|w| w.has_beads) {
            if let Ok(snap) = read_workspace_snapshot(
                Path::new(&ws.path),
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
        settings: &AppSettings,
    ) -> AppResult<WorkspaceSnapshot> {
        let norm = normalize_display_path(workspace_path);
        let root = PathBuf::from(&norm);
        if !root.is_dir() {
            return Err(AppError::WorkspaceNotFound(norm));
        }

        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;
        let slug = prefix
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| derive_prefix(&root));

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
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

            let exp_args = vec![
                "--sandbox".into(),
                "export".into(),
                "-o".into(),
                ".beads/issues.jsonl".into(),
            ];
            let _ = run_bd(&bd_bin, &root, &exp_args, None, 15).await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        let created_id = {
            let _guard = lock.lock().await;
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
            let parsed: serde_json::Value = serde_json::from_str(out.stdout.trim())
                .map_err(|e| AppError::Internal(format!("failed to parse bd create json: {e}")))?;
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

            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;

            id
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
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
            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            let mut args = vec!["close".into(), issue_id.to_string()];
            if settings.use_force_writes {
                args.push("--force".into());
            }
            if let Some(r) = reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                args.push("--reason".into());
                args.push(r.to_string());
            }
            run_bd(&bd_bin, &root, &args, None, 20).await?;
            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
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
            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            let args = vec![
                "dep".into(),
                if add { "add" } else { "remove" }.into(),
                blocked_id.to_string(),
                blocker_id.to_string(),
            ];
            run_bd(&bd_bin, &root, &args, None, 20).await?;
            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

        let lock = self.lock_for(&norm).await;
        {
            let _guard = lock.lock().await;
            let args = vec![
                "comments".into(),
                "add".into(),
                issue_id.to_string(),
                text.trim().to_string(),
            ];
            run_bd(&bd_bin, &root, &args, None, 20).await?;
            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
            .await;
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
        let bd_bin = resolve_bd_binary(settings.bd_path.as_deref())?;

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

            let _ = run_bd(
                &bd_bin,
                &root,
                &[
                    "--sandbox".into(),
                    "export".into(),
                    "-o".into(),
                    ".beads/issues.jsonl".into(),
                ],
                None,
                15,
            )
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
        compute_fingerprint(&PathBuf::from(&norm).join(".beads"), module_prefix)
    }
}

fn derive_prefix(root: &Path) -> String {
    let raw = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bd")
        .to_lowercase();
    let alpha: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if alpha.len() >= 3 {
        alpha[..3].to_string()
    } else if !alpha.is_empty() {
        alpha
    } else {
        "bd".to_string()
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
}
