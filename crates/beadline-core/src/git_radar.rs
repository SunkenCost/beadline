use crate::fs_util::{
    canonical_path_key, configure_no_window, find_in_path, fnv1a64_hex, normalize_display_path,
};
use crate::locator::RepoLocation;
use crate::models::{
    AttentionItem, AttentionKind, BlockerHotspot, ConflictAlert, ConflictEvidence, ConflictKind,
    IssueDto, ParticipantKind, ParticipantRef, RadarCoverage, RadarSnapshot, ScanStatus,
    SessionLease, SessionReachability, SessionState, TurnState, WorktreeActivity,
};
use crate::runtime_store::list_leases_at;
use crate::scope::{is_lockfile_path, normalize_rel_path, path_matches_scope, scopes_overlap};
use crate::time::{now_unix_ms, parse_rfc3339_to_unix_ms};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_DIRTY_SAMPLE_PATHS: usize = 40;
const MAX_CONFLICT_EVIDENCE_PATHS: usize = 20;
const WORKTREE_STATUS_TIMEOUT_MS: u64 = 4_000;
const PROGRESS_STALE_MINUTES: u64 = 30;

#[derive(Debug, Clone)]
pub struct ObservedWorktree {
    pub activity: WorktreeActivity,
    /// Full uncommitted repo-relative dirty paths retained inside core for exact conflict checks.
    pub full_dirty_paths: Vec<String>,
    /// Full committed paths since `merge-base(base_ref, HEAD)..HEAD`.
    pub full_committed_paths: Vec<String>,
    /// Full 40-char HEAD SHA (used for pairwise stacked-branch ancestry check).
    pub head_full_sha: Option<String>,
}

/// Resolves the repository's default integration baseline (`origin/HEAD` -> `main` -> `master`),
/// returning `(ref_name, resolved_full_sha)`.
pub fn resolve_default_base_ref(repo_root: &Path) -> Option<(String, String)> {
    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;

    // 1. Try remote default branch (`refs/remotes/origin/HEAD`)
    if let Some(sym) = run_git_line(
        &git_bin,
        repo_root,
        &["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        1500,
    ) {
        if let Some(sha) = run_git_line(&git_bin, repo_root, &["rev-parse", "--verify", "--quiet", &sym], 1500) {
            return Some((sym, sha));
        }
    }

    // 2. Fallback to common local default branch names
    for candidate in ["main", "master"] {
        if let Some(sha) = run_git_line(
            &git_bin,
            repo_root,
            &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{candidate}")],
            1500,
        ) {
            return Some((candidate.to_string(), sha));
        }
    }

    None
}

fn run_git_line(git_bin: &Path, dir: &Path, args: &[&str], timeout_ms: u64) -> Option<String> {
    let mut cmd = Command::new(git_bin);
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_no_window(&mut cmd);
    let out = run_with_timeout(cmd, Duration::from_millis(timeout_ms)).ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Parses `git worktree list --porcelain -z` (lossless NUL-separated output, preserves detached
/// HEAD, bare, locked, and prunable worktrees).
pub fn list_worktrees_porcelain_z(repo_root: &Path) -> Result<Vec<WorktreeActivity>, String> {
    let Some(git_bin) = find_in_path(if cfg!(windows) { "git.exe" } else { "git" }) else {
        return Err("git binary not found in PATH".into());
    };
    let mut cmd = Command::new(&git_bin);
    cmd.args(["worktree", "list", "--porcelain", "-z"])
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_no_window(&mut cmd);

    let out = run_with_timeout(cmd, Duration::from_millis(WORKTREE_STATUS_TIMEOUT_MS))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git worktree list failed".into()
        } else {
            stderr
        });
    }

    let now_ms = now_unix_ms();
    let mut raw_entries = parse_worktree_porcelain_z_bytes(&out.stdout, now_ms);

    for (idx, wt) in raw_entries.iter_mut().enumerate() {
        wt.is_main = idx == 0;
        let wt_path = Path::new(&wt.path);
        let git_dir_key = resolve_worktree_git_dir(&git_bin, wt_path)
            .unwrap_or_else(|| canonical_path_key(wt_path));
        wt.worktree_id = format!("wt_{}", fnv1a64_hex(git_dir_key.as_bytes()));
    }

    Ok(raw_entries)
}

fn resolve_worktree_git_dir(git_bin: &Path, wt_path: &Path) -> Option<String> {
    if !wt_path.is_dir() {
        return None;
    }
    let raw = run_git_line(
        git_bin,
        wt_path,
        &["rev-parse", "--path-format=absolute", "--git-dir"],
        1500,
    )?;
    Some(canonical_path_key(Path::new(&raw)))
}

pub fn parse_worktree_porcelain_z_bytes(bytes: &[u8], observed_at_ms: u64) -> Vec<WorktreeActivity> {
    let mut results = Vec::new();
    let mut current: Option<WorktreeActivity> = None;

    for raw_record in bytes.split(|&b| b == 0) {
        if raw_record.is_empty() {
            if let Some(wt) = current.take() {
                results.push(wt);
            }
            continue;
        }
        let line = String::from_utf8_lossy(raw_record);
        if let Some(path_raw) = line.strip_prefix("worktree ") {
            if let Some(wt) = current.take() {
                results.push(wt);
            }
            let display = normalize_display_path(path_raw);
            let fallback_id = format!(
                "wt_{}",
                fnv1a64_hex(canonical_path_key(Path::new(&display)).as_bytes())
            );
            current = Some(WorktreeActivity {
                worktree_id: fallback_id,
                path: display,
                branch: None,
                head: None,
                is_main: results.is_empty(),
                is_bare: false,
                is_locked: false,
                is_prunable: false,
                scan_status: ScanStatus::Ok,
                scan_error: None,
                observed_at_ms,
                modified_count: 0,
                untracked_count: 0,
                dirty_sample: Vec::new(),
                truncated: false,
                committed_count: 0,
                committed_sample: Vec::new(),
                committed_truncated: false,
                base_ref: None,
                branch_diff_status: ScanStatus::Ok,
                attribution: crate::models::AttributionSource::Unattributed,
                session_ids: Vec::new(),
                linked_issue_ids: Vec::new(),
            });
        } else if let Some(wt) = current.as_mut() {
            if let Some(head) = line.strip_prefix("HEAD ") {
                let short = head.chars().take(8).collect::<String>();
                wt.head = if short.is_empty() { None } else { Some(short) };
            } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                wt.branch = Some(branch.to_string());
            } else if let Some(branch) = line.strip_prefix("branch ") {
                wt.branch = Some(branch.to_string());
            } else if line == "bare" {
                wt.is_bare = true;
            } else if line == "locked" || line.starts_with("locked ") {
                wt.is_locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                wt.is_prunable = true;
            }
        }
    }
    if let Some(wt) = current {
        results.push(wt);
    }
    results
}

/// Scans a single worktree for BOTH:
/// 1. Uncommitted working-tree edits (`git --no-optional-locks status --porcelain=v2 -z`), AND
/// 2. Committed branch edits relative to `base_ref` (`git --no-optional-locks diff --name-only -z <merge-base>..HEAD`),
///    using a SHA-keyed cache so unchanged branches pay 0ms on rescan!
pub fn scan_worktree_full(
    wt: &WorktreeActivity,
    base_info: Option<&(String, String)>,
    committed_cache: &mut HashMap<(String, String), Vec<String>>,
) -> ObservedWorktree {
    let mut activity = wt.clone();
    activity.observed_at_ms = now_unix_ms();
    activity.base_ref = base_info.map(|(r, _)| r.clone());

    if activity.is_bare || activity.is_prunable {
        activity.scan_status = ScanStatus::Ok;
        activity.branch_diff_status = ScanStatus::Ok;
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
            full_committed_paths: Vec::new(),
            head_full_sha: None,
        };
    }

    let wt_path = Path::new(&activity.path);
    if !wt_path.is_dir() {
        activity.scan_status = ScanStatus::Failed;
        activity.branch_diff_status = ScanStatus::Failed;
        activity.scan_error = Some("worktree directory missing".into());
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
            full_committed_paths: Vec::new(),
            head_full_sha: None,
        };
    }

    let Some(git_bin) = find_in_path(if cfg!(windows) { "git.exe" } else { "git" }) else {
        activity.scan_status = ScanStatus::Failed;
        activity.branch_diff_status = ScanStatus::Failed;
        activity.scan_error = Some("git not found in PATH".into());
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
            full_committed_paths: Vec::new(),
            head_full_sha: None,
        };
    };

    // 1. Uncommitted dirty working tree scan
    let mut cmd = Command::new(&git_bin);
    cmd.args([
        "--no-optional-locks",
        "status",
        "--porcelain=v2",
        "-z",
        "--untracked-files=normal",
    ])
    .current_dir(wt_path)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    configure_no_window(&mut cmd);

    let full_dirty_paths = match run_with_timeout(cmd, Duration::from_millis(WORKTREE_STATUS_TIMEOUT_MS)) {
        Ok(out) if out.status.success() => {
            let (modified_paths, untracked_paths) = parse_status_porcelain_v2_z(&out.stdout);
            activity.modified_count = modified_paths.len();
            activity.untracked_count = untracked_paths.len();
            let mut all_paths = modified_paths;
            all_paths.extend(untracked_paths);
            all_paths.sort();
            all_paths.dedup();
            activity.truncated = all_paths.len() > MAX_DIRTY_SAMPLE_PATHS;
            activity.dirty_sample = all_paths
                .iter()
                .take(MAX_DIRTY_SAMPLE_PATHS)
                .cloned()
                .collect();
            activity.scan_status = ScanStatus::Ok;
            activity.scan_error = None;
            all_paths
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            activity.scan_status = ScanStatus::Failed;
            activity.scan_error = Some(if stderr.is_empty() {
                "git status exited with error".into()
            } else {
                stderr
            });
            Vec::new()
        }
        Err(err) => {
            activity.scan_status = if err.contains("timed out") {
                ScanStatus::Timeout
            } else {
                ScanStatus::Failed
            };
            activity.scan_error = Some(err);
            Vec::new()
        }
    };

    // 2. Committed branch diff vs baseline (`merge-base(base, HEAD)..HEAD`)
    let head_full_sha =
        run_git_line(&git_bin, wt_path, &["rev-parse", "--verify", "--quiet", "HEAD"], 1500);
    let full_committed_paths = if let Some((base_ref, base_sha)) = base_info {
        match &head_full_sha {
            Some(head_sha) if head_sha == base_sha => {
                activity.branch_diff_status = ScanStatus::Ok;
                Vec::new()
            }
            Some(head_sha) => {
                let cache_key = (base_sha.clone(), head_sha.clone());
                if let Some(cached) = committed_cache.get(&cache_key) {
                    activity.branch_diff_status = ScanStatus::Ok;
                    cached.clone()
                } else {
                    match compute_branch_committed_paths(&git_bin, wt_path, base_ref, head_sha) {
                        Ok(paths) => {
                            committed_cache.insert(cache_key, paths.clone());
                            activity.branch_diff_status = ScanStatus::Ok;
                            paths
                        }
                        Err(_) => {
                            activity.branch_diff_status = ScanStatus::Partial;
                            Vec::new()
                        }
                    }
                }
            }
            None => {
                activity.branch_diff_status = ScanStatus::Ok;
                Vec::new()
            }
        }
    } else {
        activity.branch_diff_status = ScanStatus::Stale;
        Vec::new()
    };

    activity.committed_count = full_committed_paths.len();
    activity.committed_truncated = full_committed_paths.len() > MAX_DIRTY_SAMPLE_PATHS;
    activity.committed_sample = full_committed_paths
        .iter()
        .take(MAX_DIRTY_SAMPLE_PATHS)
        .cloned()
        .collect();

    ObservedWorktree {
        activity,
        full_dirty_paths,
        full_committed_paths,
        head_full_sha,
    }
}

fn compute_branch_committed_paths(
    git_bin: &Path,
    wt_path: &Path,
    base_ref: &str,
    head_sha: &str,
) -> Result<Vec<String>, String> {
    let Some(mb_sha) = run_git_line(
        git_bin,
        wt_path,
        &["--no-optional-locks", "merge-base", base_ref, head_sha],
        2000,
    ) else {
        return Err("merge-base unavailable".into());
    };
    if mb_sha == head_sha {
        return Ok(Vec::new());
    }

    let range = format!("{mb_sha}..{head_sha}");
    let mut cmd = Command::new(git_bin);
    cmd.args(["--no-optional-locks", "diff", "--name-only", "-z", &range])
        .current_dir(wt_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_no_window(&mut cmd);

    let out = run_with_timeout(cmd, Duration::from_millis(2500))?;
    if !out.status.success() {
        return Err("git diff failed".into());
    }

    let mut paths: Vec<String> = out
        .stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| normalize_rel_path(&String::from_utf8_lossy(s)))
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Parses `git status --porcelain=v2 -z` bytes into `(modified_or_staged, untracked)` lists.
pub fn parse_status_porcelain_v2_z(bytes: &[u8]) -> (Vec<String>, Vec<String>) {
    let mut modified = Vec::new();
    let mut untracked = Vec::new();
    let parts: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    let mut i = 0;

    while i < parts.len() {
        let rec = parts[i];
        i += 1;
        if rec.is_empty() {
            continue;
        }
        let s = String::from_utf8_lossy(rec);
        let Some(first) = s.as_bytes().first().copied() else {
            continue;
        };
        match first {
            b'1' => {
                if let Some(path) = nth_space_tail(&s, 8) {
                    modified.push(normalize_rel_path(path));
                }
            }
            b'2' => {
                if let Some(path) = nth_space_tail(&s, 9) {
                    modified.push(normalize_rel_path(path));
                }
                if i < parts.len() {
                    let orig = String::from_utf8_lossy(parts[i]);
                    if !orig.is_empty() {
                        modified.push(normalize_rel_path(&orig));
                    }
                    i += 1;
                }
            }
            b'u' => {
                if let Some(path) = nth_space_tail(&s, 10) {
                    modified.push(normalize_rel_path(path));
                }
            }
            b'?' => {
                if let Some(path) = s.strip_prefix("? ") {
                    untracked.push(normalize_rel_path(path));
                }
            }
            _ => {}
        }
    }

    (modified, untracked)
}

fn nth_space_tail(s: &str, fields_before: usize) -> Option<&str> {
    let mut spaces = 0_usize;
    for (idx, b) in s.bytes().enumerate() {
        if b == b' ' {
            spaces += 1;
            if spaces == fields_before {
                return s.get(idx + 1..);
            }
        }
    }
    None
}

/// Computes the complete `RadarSnapshot` from live per-session leases, observed Git worktrees
/// (committed + dirty), and the current persistent issue list.
pub fn compute_radar_snapshot(
    loc: &RepoLocation,
    issues: &[IssueDto],
    generation: u64,
    scan_git: bool,
    cached_worktrees: Option<&[ObservedWorktree]>,
) -> (RadarSnapshot, Vec<ObservedWorktree>) {
    let mut ephemeral_cache = HashMap::new();
    compute_radar_snapshot_with_cache(
        loc,
        issues,
        generation,
        scan_git,
        cached_worktrees,
        &mut ephemeral_cache,
    )
}

pub fn compute_radar_snapshot_with_cache(
    loc: &RepoLocation,
    issues: &[IssueDto],
    generation: u64,
    scan_git: bool,
    cached_worktrees: Option<&[ObservedWorktree]>,
    committed_cache: &mut HashMap<(String, String), Vec<String>>,
) -> (RadarSnapshot, Vec<ObservedWorktree>) {
    let now_ms = now_unix_ms();

    let raw_leases = list_leases_at(&loc.runtime_dir, now_ms).unwrap_or_default();
    let issue_by_id: HashMap<&str, &IssueDto> =
        issues.iter().map(|i| (i.id.as_str(), i)).collect();

    // Filter out released/ended sessions or leases bound to already-closed issues (unless pending_request exists)
    let sessions: Vec<SessionLease> = raw_leases
        .into_iter()
        .filter(|l| {
            if l.state == SessionState::Released || l.turn_state == TurnState::Ended {
                return false;
            }
            if l.pending_request.is_none() {
                if let Some(iid) = &l.issue_id {
                    if let Some(issue) = issue_by_id.get(iid.as_str()) {
                        if issue.status == "closed" {
                            return false;
                        }
                    }
                }
            }
            true
        })
        .collect();

    // Step 1: Git observation (committed + dirty)
    let base_info = if scan_git && loc.git_dir.is_some() {
        resolve_default_base_ref(&loc.worktree_root)
    } else {
        cached_worktrees
            .and_then(|c| c.first())
            .and_then(|o| o.activity.base_ref.clone())
            .map(|r| (r, String::new()))
    };

    let (mut observed, list_err) = if scan_git {
        if loc.git_dir.is_some() {
            match list_worktrees_porcelain_z(&loc.worktree_root) {
                Ok(wts) => {
                    let scanned: Vec<ObservedWorktree> = wts
                        .iter()
                        .map(|wt| scan_worktree_full(wt, base_info.as_ref(), committed_cache))
                        .collect();
                    (scanned, None)
                }
                Err(err) => {
                    let retained = cached_worktrees
                        .map(|prev| {
                            prev.iter()
                                .cloned()
                                .map(|mut o| {
                                    o.activity.scan_status = ScanStatus::Stale;
                                    o.activity.scan_error = Some(err.clone());
                                    o
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (retained, Some(err))
                }
            }
        } else {
            (Vec::new(), None)
        }
    } else {
        (
            cached_worktrees.map(|c| c.to_vec()).unwrap_or_default(),
            None,
        )
    };

    // Step 2: Attribute worktrees strictly (ExplicitLease -> exact BranchIssueHint -> Unattributed)
    attribute_worktrees(&mut observed, &sessions, issues);

    // Step 3: Evidence-graded conflict radar (dirty + committed + shared checkout + scope)
    let base_ref_name = base_info.as_ref().map(|(r, _)| r.clone());
    let conflicts = detect_conflicts(&observed, &sessions, issues, base_ref_name.as_deref(), now_ms);

    // Step 4: Actionable root blocker hotspots
    let top_blockers = compute_blocker_hotspots(issues);

    // Step 5: Honest Attention queue (WaitingHuman NEVER dropped on reachability timeout!)
    let attention = compute_attention_items(&sessions, &conflicts, &issue_by_id, now_ms);

    // Step 6: Coverage honesty
    let total_worktrees = observed.len();
    let failed_worktrees = observed
        .iter()
        .filter(|o| {
            matches!(
                o.activity.scan_status,
                ScanStatus::Failed | ScanStatus::Timeout
            )
        })
        .count();
    let scanned_worktrees = total_worktrees.saturating_sub(failed_worktrees);
    let coverage_status = if list_err.is_some() {
        if observed.is_empty() {
            ScanStatus::Failed
        } else {
            ScanStatus::Stale
        }
    } else if failed_worktrees > 0 {
        if scanned_worktrees == 0 {
            ScanStatus::Failed
        } else {
            ScanStatus::Partial
        }
    } else if observed
        .iter()
        .any(|o| o.activity.scan_status == ScanStatus::Stale)
    {
        ScanStatus::Stale
    } else {
        ScanStatus::Ok
    };

    let observed_at_ms = observed
        .iter()
        .map(|o| o.activity.observed_at_ms)
        .max()
        .unwrap_or(now_ms);

    let scope_note = match &base_ref_name {
        Some(base) => format!("Committed ({base}..HEAD) + uncommitted working-tree edits"),
        None => "Uncommitted working-tree edits only (baseline ref unavailable)".to_string(),
    };

    let coverage = RadarCoverage {
        scope_note,
        base_ref: base_ref_name,
        scanned_worktrees,
        total_worktrees,
        failed_worktrees,
        scan_status: coverage_status,
        observed_at_ms,
    };

    let worktrees: Vec<WorktreeActivity> = observed.iter().map(|o| o.activity.clone()).collect();

    let radar = RadarSnapshot {
        repo_id: loc.repo_id.clone(),
        repo_path: loc.worktree_display.clone(),
        generation,
        sessions,
        worktrees,
        conflicts,
        top_blockers,
        attention,
        coverage,
    };

    (radar, observed)
}

fn attribute_worktrees(
    observed: &mut [ObservedWorktree],
    sessions: &[SessionLease],
    issues: &[IssueDto],
) {
    let open_issues: Vec<&IssueDto> = issues.iter().filter(|i| i.status != "closed").collect();

    for obs in observed.iter_mut() {
        obs.activity.session_ids.clear();
        obs.activity.linked_issue_ids.clear();

        let mut matched_sessions = Vec::new();
        let mut linked_issues = Vec::new();
        for s in sessions {
            if !s.worktree_id.is_empty() && s.worktree_id == obs.activity.worktree_id {
                matched_sessions.push(s.session_id.clone());
                if let Some(iid) = &s.issue_id {
                    if !linked_issues.contains(iid) {
                        linked_issues.push(iid.clone());
                    }
                }
            }
        }

        if !matched_sessions.is_empty() {
            obs.activity.attribution = crate::models::AttributionSource::ExplicitLease;
            obs.activity.session_ids = matched_sessions;
            obs.activity.linked_issue_ids = linked_issues;
            continue;
        }

        if let Some(branch) = &obs.activity.branch {
            let branch_lower = branch.to_ascii_lowercase();
            let mut hint_matches = Vec::new();
            for issue in &open_issues {
                if branch_contains_exact_token(&branch_lower, &issue.id.to_ascii_lowercase()) {
                    hint_matches.push(issue.id.clone());
                }
            }
            if hint_matches.len() == 1 {
                obs.activity.attribution = crate::models::AttributionSource::BranchIssueHint;
                obs.activity.linked_issue_ids = hint_matches;
                continue;
            }
        }

        obs.activity.attribution = crate::models::AttributionSource::Unattributed;
    }
}

fn branch_contains_exact_token(branch_lower: &str, token_lower: &str) -> bool {
    if token_lower.len() < 3 {
        return false;
    }
    for (idx, _) in branch_lower.match_indices(token_lower) {
        let before_ok = idx == 0 || !branch_lower.as_bytes()[idx - 1].is_ascii_alphanumeric();
        let after_idx = idx + token_lower.len();
        let after_ok = after_idx == branch_lower.len()
            || !branch_lower.as_bytes()[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Detects conflicts across all evidence tiers:
/// 1. `SameFileDirty` (uncommitted vs uncommitted across 2 worktrees)
/// 2. `SameFileCommitted` (committed + dirty overlap across 2 worktrees relative to `base_ref`)
/// 3. `SharedWorktree` (2+ active sessions sharing the exact same checkout directory)
/// 4. `DirtyHitsDeclaredScope` (dirty or committed paths hitting another session's declared scope)
/// 5. `DeclaredScopeOverlap`
pub fn detect_conflicts(
    observed: &[ObservedWorktree],
    sessions: &[SessionLease],
    issues: &[IssueDto],
    base_ref: Option<&str>,
    observed_at_ms: u64,
) -> Vec<ConflictAlert> {
    let issue_by_id: HashMap<&str, &IssueDto> =
        issues.iter().map(|i| (i.id.as_str(), i)).collect();

    let mut best_by_pair: BTreeMap<(String, String), ConflictAlert> = BTreeMap::new();

    let wt_participant = |o: &ObservedWorktree| -> ParticipantRef {
        let label = o
            .activity
            .branch
            .clone()
            .or_else(|| {
                Path::new(&o.activity.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| o.activity.worktree_id.clone());
        let first_session = o
            .activity
            .session_ids
            .first()
            .and_then(|sid| sessions.iter().find(|s| &s.session_id == sid));
        ParticipantRef {
            kind: ParticipantKind::Worktree,
            id: o.activity.worktree_id.clone(),
            label,
            issue_id: o.activity.linked_issue_ids.first().cloned(),
            worktree_id: Some(o.activity.worktree_id.clone()),
            branch: o.activity.branch.clone(),
            harness: first_session.map(|s| s.harness.clone()),
        }
    };

    let session_participant = |s: &SessionLease| -> ParticipantRef {
        let label = s
            .issue_id
            .as_deref()
            .and_then(|iid| issue_by_id.get(iid).map(|i| i.short_id.clone()))
            .unwrap_or_else(|| s.session_id.clone());
        ParticipantRef {
            kind: ParticipantKind::Session,
            id: s.session_id.clone(),
            label,
            issue_id: s.issue_id.clone(),
            worktree_id: if s.worktree_id.is_empty() {
                None
            } else {
                Some(s.worktree_id.clone())
            },
            branch: s.branch.clone(),
            harness: Some(s.harness.clone()),
        }
    };

    // Tier 1a & 1b: SameFileDirty AND SameFileCommitted across distinct worktrees
    for i in 0..observed.len() {
        let a = &observed[i];
        let dirty_a: HashSet<&str> = a.full_dirty_paths.iter().map(String::as_str).collect();
        let all_a: HashSet<&str> = dirty_a
            .iter()
            .copied()
            .chain(a.full_committed_paths.iter().map(String::as_str))
            .collect();
        if all_a.is_empty() {
            continue;
        }

        for b in observed.iter().skip(i + 1) {
            if a.activity.worktree_id == b.activity.worktree_id {
                continue;
            }
            let dirty_b: HashSet<&str> = b.full_dirty_paths.iter().map(String::as_str).collect();
            let all_b: HashSet<&str> = dirty_b
                .iter()
                .copied()
                .chain(b.full_committed_paths.iter().map(String::as_str))
                .collect();
            if all_b.is_empty() {
                continue;
            }

            let mut both_dirty: Vec<String> = dirty_a
                .intersection(&dirty_b)
                .map(|s| (*s).to_string())
                .collect();
            both_dirty.sort();

            let mut any_shared: Vec<String> = all_a
                .intersection(&all_b)
                .map(|s| (*s).to_string())
                .collect();
            any_shared.sort();

            if any_shared.is_empty() {
                continue;
            }

            let p_a = wt_participant(a);
            let p_b = wt_participant(b);
            let pair_key = ordered_pair(&p_a.id, &p_b.id);

            if !both_dirty.is_empty() {
                let lockfile_only = both_dirty.iter().all(|p| is_lockfile_path(p));
                let total_paths = both_dirty.len();
                let sample: Vec<String> =
                    both_dirty.into_iter().take(MAX_CONFLICT_EVIDENCE_PATHS).collect();
                let alert_id = format!(
                    "cfl_{}",
                    fnv1a64_hex(format!("same-dirty:{}:{}", pair_key.0, pair_key.1).as_bytes())
                );
                insert_strongest(
                    &mut best_by_pair,
                    pair_key,
                    ConflictAlert {
                        id: alert_id,
                        kind: ConflictKind::SameFileDirty,
                        participants: vec![p_a, p_b],
                        evidence: ConflictEvidence {
                            paths: sample,
                            matched_scopes: Vec::new(),
                            total_paths,
                            lockfile_only,
                            base_ref: base_ref.map(String::from),
                            evidence_note: Some("uncommitted edits on both worktrees".into()),
                        },
                        observed_at_ms,
                    },
                );
            } else {
                // Stacked-branch de-noising: if one branch is a direct ancestor of the other
                // (e.g. feat/c branched off feat/a), only compare edits made AFTER their pairwise
                // merge-base so inherited ancestor commits never create false overlap alarms!
                let effective_shared = filter_stacked_ancestor_overlap(a, b, any_shared);
                if effective_shared.is_empty() {
                    continue;
                }
                let lockfile_only = effective_shared.iter().all(|p| is_lockfile_path(p));
                let total_paths = effective_shared.len();
                let sample: Vec<String> = effective_shared
                    .into_iter()
                    .take(MAX_CONFLICT_EVIDENCE_PATHS)
                    .collect();
                let alert_id = format!(
                    "cfl_{}",
                    fnv1a64_hex(format!("same-branch:{}:{}", pair_key.0, pair_key.1).as_bytes())
                );
                let note = match base_ref {
                    Some(bref) => format!("branch diff vs {bref} (committed + working tree)"),
                    None => "committed + working tree overlap".into(),
                };
                insert_strongest(
                    &mut best_by_pair,
                    pair_key,
                    ConflictAlert {
                        id: alert_id,
                        kind: ConflictKind::SameFileCommitted,
                        participants: vec![p_a, p_b],
                        evidence: ConflictEvidence {
                            paths: sample,
                            matched_scopes: Vec::new(),
                            total_paths,
                            lockfile_only,
                            base_ref: base_ref.map(String::from),
                            evidence_note: Some(note),
                        },
                        observed_at_ms,
                    },
                );
            }
        }
    }

    // Active/reachable sessions for shared-checkout & declared-scope checks
    let live_sessions: Vec<&SessionLease> = sessions
        .iter()
        .filter(|s| {
            s.state != SessionState::Released
                && s.turn_state != TurnState::Ended
                && s.reachability != SessionReachability::Unconfirmed
        })
        .collect();

    // SharedWorktree alert when 2+ live sessions run in the exact same checkout directory
    let mut by_worktree: BTreeMap<&str, Vec<&SessionLease>> = BTreeMap::new();
    for s in &live_sessions {
        if !s.worktree_id.is_empty() {
            by_worktree.entry(s.worktree_id.as_str()).or_default().push(s);
        }
    }
    for (wt_id, group) in by_worktree {
        if group.len() < 2 {
            continue;
        }
        let participants: Vec<ParticipantRef> =
            group.iter().map(|s| session_participant(s)).collect();
        let wt_path = group[0].worktree_path.clone();
        let alert_id = format!("cfl_{}", fnv1a64_hex(format!("shared-wt:{wt_id}").as_bytes()));
        let pair_key = (format!("shared:{wt_id}"), format!("shared:{wt_id}"));
        insert_strongest(
            &mut best_by_pair,
            pair_key,
            ConflictAlert {
                id: alert_id,
                kind: ConflictKind::SharedWorktree,
                participants,
                evidence: ConflictEvidence {
                    paths: vec![wt_path],
                    matched_scopes: Vec::new(),
                    total_paths: 1,
                    lockfile_only: false,
                    base_ref: None,
                    evidence_note: Some(
                        "Multiple active sessions share one checkout directory; split into separate Git worktrees for attribution".into(),
                    ),
                },
                observed_at_ms,
            },
        );
    }

    let effective_scope_for = |s: &SessionLease| -> Vec<String> {
        let mut scopes = s.scope.clone();
        if let Some(iid) = &s.issue_id {
            if let Some(issue) = issue_by_id.get(iid.as_str()) {
                for pat in &issue.scope {
                    if !scopes.contains(pat) {
                        scopes.push(pat.clone());
                    }
                }
            }
        }
        scopes
    };

    // Tier 2: Dirty or committed branch paths hit another active session's declared write scope
    for wt in observed {
        let combined_paths: Vec<&String> = wt
            .full_dirty_paths
            .iter()
            .chain(wt.full_committed_paths.iter())
            .collect();
        if combined_paths.is_empty() {
            continue;
        }
        for s in &live_sessions {
            if !s.worktree_id.is_empty() && s.worktree_id == wt.activity.worktree_id {
                continue;
            }
            if let Some(iid) = &s.issue_id {
                if wt.activity.linked_issue_ids.contains(iid) {
                    continue;
                }
            }
            let scopes = effective_scope_for(s);
            if scopes.is_empty() {
                continue;
            }

            let mut hit_paths = Vec::new();
            let mut hit_scopes = Vec::new();
            for changed in &combined_paths {
                for pat in &scopes {
                    if path_matches_scope(changed, pat) {
                        if !hit_paths.contains(*changed) {
                            hit_paths.push((*changed).clone());
                        }
                        if !hit_scopes.contains(pat) {
                            hit_scopes.push(pat.clone());
                        }
                    }
                }
            }
            if hit_paths.is_empty() {
                continue;
            }

            let lockfile_only = hit_paths.iter().all(|p| is_lockfile_path(p));
            let total_paths = hit_paths.len();
            let p_wt = wt_participant(wt);
            let p_ses = session_participant(s);
            let pair_key = ordered_pair(&p_wt.id, &p_ses.id);
            let alert_id = format!(
                "cfl_{}",
                fnv1a64_hex(format!("dirty-scope:{}:{}", pair_key.0, pair_key.1).as_bytes())
            );
            insert_strongest(
                &mut best_by_pair,
                pair_key,
                ConflictAlert {
                    id: alert_id,
                    kind: ConflictKind::DirtyHitsDeclaredScope,
                    participants: vec![p_wt, p_ses],
                    evidence: ConflictEvidence {
                        paths: hit_paths
                            .into_iter()
                            .take(MAX_CONFLICT_EVIDENCE_PATHS)
                            .collect(),
                        matched_scopes: hit_scopes,
                        total_paths,
                        lockfile_only,
                        base_ref: base_ref.map(String::from),
                        evidence_note: None,
                    },
                    observed_at_ms,
                },
            );
        }
    }

    // Tier 3: DeclaredScopeOverlap between two distinct active sessions
    for i in 0..live_sessions.len() {
        let sa = live_sessions[i];
        let scopes_a = effective_scope_for(sa);
        if scopes_a.is_empty() {
            continue;
        }
        for sb in live_sessions.iter().skip(i + 1) {
            if sa.issue_id.is_some() && sa.issue_id == sb.issue_id {
                continue;
            }
            let scopes_b = effective_scope_for(sb);
            if scopes_b.is_empty() {
                continue;
            }

            let mut matched = Vec::new();
            for pa in &scopes_a {
                for pb in &scopes_b {
                    if scopes_overlap(pa, pb) {
                        let desc = if pa == pb {
                            pa.clone()
                        } else {
                            format!("{pa} <-> {pb}")
                        };
                        if !matched.contains(&desc) {
                            matched.push(desc);
                        }
                    }
                }
            }
            if matched.is_empty() {
                continue;
            }

            let p_a = session_participant(sa);
            let p_b = session_participant(sb);
            let pair_key = ordered_pair(&p_a.id, &p_b.id);
            let alert_id = format!(
                "cfl_{}",
                fnv1a64_hex(format!("scope:{}:{}", pair_key.0, pair_key.1).as_bytes())
            );
            insert_strongest(
                &mut best_by_pair,
                pair_key,
                ConflictAlert {
                    id: alert_id,
                    kind: ConflictKind::DeclaredScopeOverlap,
                    participants: vec![p_a, p_b],
                    evidence: ConflictEvidence {
                        paths: Vec::new(),
                        matched_scopes: matched,
                        total_paths: 0,
                        lockfile_only: false,
                        base_ref: None,
                        evidence_note: None,
                    },
                    observed_at_ms,
                },
            );
        }
    }

    let mut alerts: Vec<ConflictAlert> = best_by_pair.into_values().collect();
    alerts.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.evidence.lockfile_only.cmp(&b.evidence.lockfile_only))
            .then_with(|| b.evidence.total_paths.cmp(&a.evidence.total_paths))
            .then_with(|| a.id.cmp(&b.id))
    });
    alerts
}

fn filter_stacked_ancestor_overlap(
    a: &ObservedWorktree,
    b: &ObservedWorktree,
    shared: Vec<String>,
) -> Vec<String> {
    let (Some(sha_a), Some(sha_b)) = (&a.head_full_sha, &b.head_full_sha) else {
        return shared;
    };
    if sha_a == sha_b {
        // Two worktrees at the exact same commit only conflict if at least one has dirty edits on a shared file
        let dirty_a: HashSet<&str> = a.full_dirty_paths.iter().map(String::as_str).collect();
        let dirty_b: HashSet<&str> = b.full_dirty_paths.iter().map(String::as_str).collect();
        return shared
            .into_iter()
            .filter(|p| dirty_a.contains(p.as_str()) || dirty_b.contains(p.as_str()))
            .collect();
    }

    let Some(git_bin) = find_in_path(if cfg!(windows) { "git.exe" } else { "git" }) else {
        return shared;
    };
    let wt_dir = Path::new(&a.activity.path);
    let Some(mb) = run_git_line(
        &git_bin,
        wt_dir,
        &["--no-optional-locks", "merge-base", sha_a, sha_b],
        1500,
    ) else {
        return shared;
    };

    // If A is a strict ancestor of B (B is stacked on A), only flag files B actually modified
    // AFTER A (committed since A or dirty in B), or files A modified in its dirty tree!
    if mb == *sha_a {
        let child_new = compute_branch_committed_paths(&git_bin, wt_dir, sha_a, sha_b)
            .unwrap_or_default();
        let child_touched: HashSet<&str> = child_new
            .iter()
            .chain(b.full_dirty_paths.iter())
            .map(String::as_str)
            .collect();
        let parent_dirty: HashSet<&str> = a.full_dirty_paths.iter().map(String::as_str).collect();
        return shared
            .into_iter()
            .filter(|p| child_touched.contains(p.as_str()) || parent_dirty.contains(p.as_str()))
            .collect();
    }
    if mb == *sha_b {
        let child_new = compute_branch_committed_paths(&git_bin, wt_dir, sha_b, sha_a)
            .unwrap_or_default();
        let child_touched: HashSet<&str> = child_new
            .iter()
            .chain(a.full_dirty_paths.iter())
            .map(String::as_str)
            .collect();
        let parent_dirty: HashSet<&str> = b.full_dirty_paths.iter().map(String::as_str).collect();
        return shared
            .into_iter()
            .filter(|p| child_touched.contains(p.as_str()) || parent_dirty.contains(p.as_str()))
            .collect();
    }

    shared
}

fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn insert_strongest(
    map: &mut BTreeMap<(String, String), ConflictAlert>,
    key: (String, String),
    candidate: ConflictAlert,
) {
    match map.get(&key) {
        Some(existing) if existing.kind <= candidate.kind => {}
        _ => {
            map.insert(key, candidate);
        }
    }
}

pub fn compute_blocker_hotspots(issues: &[IssueDto]) -> Vec<BlockerHotspot> {
    let open_by_id: HashMap<&str, &IssueDto> = issues
        .iter()
        .filter(|i| i.status != "closed")
        .map(|i| (i.id.as_str(), i))
        .collect();

    let mut hotspots = Vec::new();
    for (&id, issue) in &open_by_id {
        if issue.blocks.is_empty() {
            continue;
        }

        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<&str> = Vec::new();
        let mut direct_unblocks = 0_usize;

        for child_id in &issue.blocks {
            if let Some(child) = open_by_id.get(child_id.as_str()) {
                if child.open_blocked_by.len() == 1 && child.open_blocked_by[0] == id {
                    direct_unblocks += 1;
                }
                if visited.insert(child_id.clone()) {
                    stack.push(child_id.as_str());
                }
            }
        }

        while let Some(cur) = stack.pop() {
            if let Some(node) = open_by_id.get(cur) {
                for nxt in &node.blocks {
                    if open_by_id.contains_key(nxt.as_str()) && visited.insert(nxt.clone()) {
                        stack.push(nxt.as_str());
                    }
                }
            }
        }

        if visited.is_empty() {
            continue;
        }

        hotspots.push(BlockerHotspot {
            issue_id: issue.id.clone(),
            short_id: issue.short_id.clone(),
            title: issue.title.clone(),
            lane: issue.lane,
            actionable: issue.open_blocked_by.is_empty(),
            direct_unblocks,
            total_downstream: visited.len(),
            downstream_ids: visited.into_iter().collect(),
        });
    }

    hotspots.sort_by(|a, b| {
        b.actionable
            .cmp(&a.actionable)
            .then_with(|| b.direct_unblocks.cmp(&a.direct_unblocks))
            .then_with(|| b.total_downstream.cmp(&a.total_downstream))
            .then_with(|| a.issue_id.cmp(&b.issue_id))
    });
    hotspots.truncate(6);
    hotspots
}

fn compute_attention_items(
    sessions: &[SessionLease],
    conflicts: &[ConflictAlert],
    issue_by_id: &HashMap<&str, &IssueDto>,
    now_ms: u64,
) -> Vec<AttentionItem> {
    let mut items = Vec::new();
    let mut sessions_with_pending = HashSet::new();

    // 1. Persistent PendingHumanRequest / WaitingHuman — NEVER hidden or downgraded when reachability expires!
    for s in sessions {
        let pending_reason = s
            .pending_request
            .as_ref()
            .map(|r| (r.reason.clone(), r.created_at.clone()))
            .or_else(|| {
                if s.state == SessionState::WaitingHuman {
                    Some((
                        s.waiting_reason
                            .clone()
                            .filter(|r| !r.trim().is_empty())
                            .unwrap_or_else(|| s.summary.clone()),
                        s.last_progress_at.clone(),
                    ))
                } else {
                    None
                }
            });

        let Some((reason, created_at)) = pending_reason else {
            continue;
        };
        sessions_with_pending.insert(s.session_id.as_str());
        let age_min = age_minutes_since(&created_at, now_ms);
        items.push(AttentionItem {
            id: format!("att_wait_{}", s.session_id),
            kind: AttentionKind::WaitingHuman,
            issue_id: s.issue_id.clone(),
            session_id: Some(s.session_id.clone()),
            conflict_id: None,
            harness: Some(s.harness.clone()),
            agent: Some(s.agent.clone()),
            reason,
            age_minutes: age_min,
            thread_url: s.thread_url.clone(),
            unconfirmed: s.reachability == SessionReachability::Unconfirmed,
        });
    }

    // 2. Non-lockfile SameFileDirty AND SameFileCommitted overlaps + SharedWorktree
    for c in conflicts {
        match c.kind {
            ConflictKind::SameFileDirty | ConflictKind::SameFileCommitted => {
                if c.evidence.lockfile_only {
                    continue;
                }
                let first_path = c
                    .evidence
                    .paths
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "shared file".into());
                let extra = c.evidence.total_paths.saturating_sub(1);
                let reason = if extra > 0 {
                    format!("{first_path} (+{extra})")
                } else {
                    first_path
                };
                let first_issue = c.participants.iter().find_map(|p| p.issue_id.clone());
                items.push(AttentionItem {
                    id: format!("att_cfl_{}", c.id),
                    kind: AttentionKind::FileOverlap,
                    issue_id: first_issue,
                    session_id: None,
                    conflict_id: Some(c.id.clone()),
                    harness: None,
                    agent: None,
                    reason,
                    age_minutes: 0,
                    thread_url: None,
                    unconfirmed: false,
                });
            }
            ConflictKind::SharedWorktree => {
                let names: Vec<String> = c
                    .participants
                    .iter()
                    .map(|p| p.harness.clone().unwrap_or_else(|| p.label.clone()))
                    .collect();
                items.push(AttentionItem {
                    id: format!("att_shared_{}", c.id),
                    kind: AttentionKind::SharedWorktree,
                    issue_id: c.participants.iter().find_map(|p| p.issue_id.clone()),
                    session_id: None,
                    conflict_id: Some(c.id.clone()),
                    harness: None,
                    agent: None,
                    reason: format!("{} sessions sharing same checkout ({})", c.participants.len(), names.join(", ")),
                    age_minutes: 0,
                    thread_url: None,
                    unconfirmed: false,
                });
            }
            _ => {}
        }
    }

    // 3. ProgressStale: only while actively Running with >= 30m unchanged activity
    for s in sessions {
        if sessions_with_pending.contains(s.session_id.as_str())
            || s.turn_state != TurnState::Running
            || s.reachability == SessionReachability::Unconfirmed
        {
            continue;
        }
        let progress_age = age_minutes_since(&s.last_activity_at, now_ms);
        if progress_age >= PROGRESS_STALE_MINUTES {
            items.push(AttentionItem {
                id: format!("att_prog_{}", s.session_id),
                kind: AttentionKind::ProgressStale,
                issue_id: s.issue_id.clone(),
                session_id: Some(s.session_id.clone()),
                conflict_id: None,
                harness: Some(s.harness.clone()),
                agent: Some(s.agent.clone()),
                reason: s.summary.clone(),
                age_minutes: progress_age,
                thread_url: s.thread_url.clone(),
                unconfirmed: false,
            });
        }
    }

    // 4. HeartbeatStale (Unconfirmed > 45m while still Running, without pending_request)
    for s in sessions {
        if sessions_with_pending.contains(s.session_id.as_str())
            || s.reachability != SessionReachability::Unconfirmed
            || s.turn_state == TurnState::IdleBetweenTurns
        {
            continue;
        }
        if let Some(iid) = &s.issue_id {
            if let Some(issue) = issue_by_id.get(iid.as_str()) {
                if issue.status == "closed" {
                    continue;
                }
            }
        }
        let seen_age = age_minutes_since(&s.last_activity_at, now_ms);
        items.push(AttentionItem {
            id: format!("att_hb_{}", s.session_id),
            kind: AttentionKind::HeartbeatStale,
            issue_id: s.issue_id.clone(),
            session_id: Some(s.session_id.clone()),
            conflict_id: None,
            harness: Some(s.harness.clone()),
            agent: Some(s.agent.clone()),
            reason: s.summary.clone(),
            age_minutes: seen_age,
            thread_url: s.thread_url.clone(),
            unconfirmed: true,
        });
    }

    items.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| b.age_minutes.cmp(&a.age_minutes))
            .then_with(|| a.id.cmp(&b.id))
    });
    items
}

fn age_minutes_since(iso: &str, now_ms: u64) -> u64 {
    parse_rfc3339_to_unix_ms(iso)
        .map(|ts| now_ms.saturating_sub(ts) / 60_000)
        .unwrap_or(0)
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> Result<std::process::Output, String> {
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                return child
                    .wait_with_output()
                    .map_err(|e| format!("failed to read git output: {e}"));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "git command timed out after {}ms",
                        timeout.as_millis()
                    ));
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("git wait failed: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::RepoLocator;
    use crate::models::{BoardLane, PendingHumanRequest, PendingRequestKind, SmartMeta};
    use std::fs;

    fn run_git(dir: &Path, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com");
        configure_no_window(&mut cmd);
        let out = cmd.output().expect("git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn dummy_issue(
        id: &str,
        status: &str,
        open_blocked_by: Vec<&str>,
        blocks: Vec<&str>,
    ) -> IssueDto {
        IssueDto {
            id: id.into(),
            short_id: id.into(),
            title: format!("Issue {id}"),
            description: String::new(),
            notes: String::new(),
            acceptance_criteria: String::new(),
            close_reason: String::new(),
            scope: Vec::new(),
            proof: None,
            rev: 1,
            status: status.into(),
            lane: if status == "closed" {
                BoardLane::Closed
            } else if !open_blocked_by.is_empty() {
                BoardLane::Blocked
            } else {
                BoardLane::Ready
            },
            priority: 1,
            issue_type: "task".into(),
            assignee: String::new(),
            owner: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            closed_at: String::new(),
            external_ref: String::new(),
            labels: Vec::new(),
            modules: Vec::new(),
            parent_id: None,
            parent_short_id: None,
            parent_title: None,
            children_ids: Vec::new(),
            blocked_by: open_blocked_by.iter().map(|s| s.to_string()).collect(),
            open_blocked_by: open_blocked_by.into_iter().map(String::from).collect(),
            blocks: blocks.into_iter().map(String::from).collect(),
            comment_count: 0,
            smart: SmartMeta::default(),
        }
    }

    #[test]
    fn catches_committed_branch_overlap_even_after_agent_a_commits_cleanly() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main_repo = tmp.path().join("repo");
        let wt_a = tmp.path().join("wt-a");
        let wt_b = tmp.path().join("wt-b");
        fs::create_dir_all(&main_repo).unwrap();

        run_git(&main_repo, &["init", "-b", "main"]);
        fs::create_dir_all(main_repo.join("src")).unwrap();
        fs::write(main_repo.join("src/auth.rs"), "fn auth() {}\n").unwrap();
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "base"]);

        run_git(&main_repo, &["worktree", "add", "-b", "feat/a", wt_a.to_str().unwrap()]);
        run_git(&main_repo, &["worktree", "add", "-b", "feat/b", wt_b.to_str().unwrap()]);

        // Agent A modifies src/auth.rs AND commits it -> working tree is 100% clean!
        fs::write(wt_a.join("src/auth.rs"), "fn auth() { /* A */ }\n").unwrap();
        run_git(&wt_a, &["add", "src/auth.rs"]);
        run_git(&wt_a, &["commit", "-m", "agent A committed"]);

        // Agent B modifies src/auth.rs (uncommitted)
        fs::write(wt_b.join("src/auth.rs"), "fn auth() { /* B */ }\n").unwrap();

        let loc = RepoLocator::locate(&main_repo).unwrap();
        let (radar, _) = compute_radar_snapshot(&loc, &[], 1, true, None);

        assert_eq!(radar.coverage.base_ref.as_deref(), Some("main"));
        let committed_cfl = radar
            .conflicts
            .iter()
            .find(|c| c.kind == ConflictKind::SameFileCommitted)
            .expect("must detect committed vs dirty overlap on src/auth.rs");
        assert_eq!(committed_cfl.evidence.paths, vec!["src/auth.rs"]);
        assert!(radar
            .attention
            .iter()
            .any(|a| a.kind == AttentionKind::FileOverlap));

        // Stacked child branch `feat/c` branched off `feat/a` touching only `src/child.rs`
        // must NOT trigger a false overlap with its parent `feat/a`!
        let wt_c = tmp.path().join("wt-c");
        run_git(
            &main_repo,
            &["worktree", "add", "-b", "feat/c", wt_c.to_str().unwrap(), "feat/a"],
        );
        fs::write(wt_c.join("src/child.rs"), "fn child() {}\n").unwrap();
        run_git(&wt_c, &["add", "src/child.rs"]);
        run_git(&wt_c, &["commit", "-m", "stacked child only touches child.rs"]);

        let (radar2, _) = compute_radar_snapshot(&loc, &[], 2, true, None);
        let false_parent_child = radar2.conflicts.iter().any(|c| {
            let branches: Vec<_> = c
                .participants
                .iter()
                .filter_map(|p| p.branch.as_deref())
                .collect();
            branches.contains(&"feat/a") && branches.contains(&"feat/c")
        });
        assert!(!false_parent_child, "stacked child touching unrelated file must not alarm against parent");
    }

    #[test]
    fn shared_worktree_and_unconfirmed_pending_request_both_surface_honestly() {
        let s1 = SessionLease {
            session_id: "ses_1".into(),
            issue_id: Some("bl-1".into()),
            worktree_id: "wt_same".into(),
            worktree_path: "/repo".into(),
            harness: "claude-code".into(),
            agent: "claude".into(),
            state: SessionState::WaitingHuman,
            turn_state: TurnState::WaitingHuman,
            reachability: SessionReachability::Unconfirmed,
            stale_heartbeat: true,
            pending_request: Some(PendingHumanRequest {
                request_id: "perm_1".into(),
                kind: PendingRequestKind::Approval,
                reason: "Allow bash?".into(),
                created_at: "2026-11-26T00:00:00Z".into(),
                ..PendingHumanRequest::default()
            }),
            ..SessionLease::default()
        };
        let s2 = SessionLease {
            session_id: "ses_2".into(),
            issue_id: Some("bl-2".into()),
            worktree_id: "wt_same".into(),
            worktree_path: "/repo".into(),
            harness: "codex".into(),
            agent: "gpt".into(),
            state: SessionState::Working,
            turn_state: TurnState::Running,
            reachability: SessionReachability::Active,
            ..SessionLease::default()
        };
        let s3 = SessionLease {
            session_id: "ses_3".into(),
            issue_id: Some("bl-3".into()),
            worktree_id: "wt_same".into(),
            worktree_path: "/repo".into(),
            harness: "cursor".into(),
            agent: "cursor".into(),
            state: SessionState::Working,
            turn_state: TurnState::Running,
            reachability: SessionReachability::Active,
            ..SessionLease::default()
        };

        let conflicts = detect_conflicts(&[], &[s1.clone(), s2.clone(), s3.clone()], &[], None, 1000);
        assert!(conflicts.iter().any(|c| c.kind == ConflictKind::SharedWorktree));

        let attention = compute_attention_items(&[s1, s2, s3], &conflicts, &HashMap::new(), 1_800_000_000_000);
        let wait = attention
            .iter()
            .find(|a| a.kind == AttentionKind::WaitingHuman)
            .expect("pending human request must stay visible even when unconfirmed");
        assert!(wait.unconfirmed);
        assert_eq!(wait.reason, "Allow bash?");
    }

    #[test]
    fn parses_detached_worktrees_and_porcelain_v2_renames_accurately() {
        let wt_bytes = b"worktree /repo/main\0HEAD 1111222233334444\0branch refs/heads/main\0\0worktree /repo/wt-det\0HEAD aaaa999988887777\0detached\0locked\0\0";
        let wts = parse_worktree_porcelain_z_bytes(wt_bytes, 100);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].branch.as_deref(), Some("main"));
        assert!(wts[1].branch.is_none());
        assert_eq!(wts[1].head.as_deref(), Some("aaaa9999"));
        assert!(wts[1].is_locked);

        let status_bytes = b"1 .M N... 100644 100644 100644 111 222 src/auth/jwt.rs\x002 R. N... 100644 100644 100644 111 222 R100 src/new.rs\0src/old.rs\0? untracked_dir/\0";
        let (modified, untracked) = parse_status_porcelain_v2_z(status_bytes);
        assert_eq!(modified, vec!["src/auth/jwt.rs", "src/new.rs", "src/old.rs"]);
        assert_eq!(untracked, vec!["untracked_dir/"]);
    }

    #[test]
    fn ranks_actionable_root_blocker_above_blocked_intermediate_node() {
        let issues = vec![
            dummy_issue("root", "open", vec![], vec!["mid"]),
            dummy_issue("mid", "open", vec!["root"], vec!["leaf1", "leaf2", "leaf3"]),
            dummy_issue("leaf1", "open", vec!["mid"], vec![]),
            dummy_issue("leaf2", "open", vec!["mid"], vec![]),
            dummy_issue("leaf3", "open", vec!["mid"], vec![]),
        ];
        let hotspots = compute_blocker_hotspots(&issues);
        assert_eq!(hotspots[0].issue_id, "root");
        assert!(hotspots[0].actionable);
        assert_eq!(hotspots[0].direct_unblocks, 1);
        assert_eq!(hotspots[0].total_downstream, 4);
    }

    #[test]
    fn detects_same_file_and_dirty_hits_scope_and_flags_lockfiles() {
        let wt1 = ObservedWorktree {
            activity: WorktreeActivity {
                worktree_id: "wt_1".into(),
                path: "/wt1".into(),
                branch: Some("feat/a".into()),
                ..WorktreeActivity::default()
            },
            full_dirty_paths: vec!["src/auth/jwt.rs".into(), "Cargo.lock".into()],
            full_committed_paths: Vec::new(),
            head_full_sha: None,
        };
        let wt2 = ObservedWorktree {
            activity: WorktreeActivity {
                worktree_id: "wt_2".into(),
                path: "/wt2".into(),
                branch: Some("feat/b".into()),
                ..WorktreeActivity::default()
            },
            full_dirty_paths: vec!["Cargo.lock".into()],
            full_committed_paths: Vec::new(),
            head_full_sha: None,
        };
        let ses = SessionLease {
            session_id: "ses_2".into(),
            issue_id: Some("bl-2".into()),
            worktree_id: "wt_2".into(),
            worktree_path: "/wt2".into(),
            branch: Some("feat/b".into()),
            harness: "codex".into(),
            agent: "gpt".into(),
            state: SessionState::Working,
            reachability: SessionReachability::Active,
            scope: vec!["src/auth/**".into()],
            ..SessionLease::default()
        };

        let alerts = detect_conflicts(&[wt1, wt2], &[ses], &[], Some("main"), 1000);
        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].kind, ConflictKind::SameFileDirty);
        assert!(alerts[0].evidence.lockfile_only);
        assert_eq!(alerts[1].kind, ConflictKind::DirtyHitsDeclaredScope);
        assert_eq!(alerts[1].evidence.paths, vec!["src/auth/jwt.rs"]);
    }
}
