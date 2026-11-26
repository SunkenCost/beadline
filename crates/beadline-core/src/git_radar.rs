use crate::fs_util::{
    canonical_path_key, configure_no_window, find_in_path, fnv1a64_hex, normalize_display_path,
};
use crate::locator::RepoLocation;
use crate::models::{
    AttentionItem, AttentionKind, BlockerHotspot, ConflictAlert, ConflictEvidence, ConflictKind,
    IssueDto, ParticipantKind, ParticipantRef, RadarCoverage, RadarSnapshot, ScanStatus,
    SessionLease, SessionState, WorktreeActivity,
};
use crate::runtime_store::list_leases;
use crate::scope::{is_lockfile_path, normalize_rel_path, path_matches_scope, scopes_overlap};
use crate::time::{now_unix_ms, parse_rfc3339_to_unix_ms};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_DIRTY_SAMPLE_PATHS: usize = 40;
const MAX_CONFLICT_EVIDENCE_PATHS: usize = 20;
const WORKTREE_STATUS_TIMEOUT_MS: u64 = 4_000;
const PROGRESS_STALE_MINUTES: u64 = 20;
const COVERAGE_NOTE_WORKTREE: &str = "Working-tree uncommitted changes only (excludes committed branch diffs)";

#[derive(Debug, Clone)]
pub struct ObservedWorktree {
    pub activity: WorktreeActivity,
    /// Full repo-relative dirty paths retained inside core for exact conflict checks.
    pub full_dirty_paths: Vec<String>,
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

    // Resolve stable per-worktree git_dir for each entry (worktree_id)
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
    let mut cmd = Command::new(git_bin);
    cmd.args(["rev-parse", "--path-format=absolute", "--git-dir"])
        .current_dir(wt_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_no_window(&mut cmd);
    let out = run_with_timeout(cmd, Duration::from_millis(1500)).ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
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
            let fallback_id = format!("wt_{}", fnv1a64_hex(canonical_path_key(Path::new(&display)).as_bytes()));
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

/// Scans a single worktree using `git --no-optional-locks status --porcelain=v2 -z --untracked-files=normal`.
/// Never takes Git optional locks, never blocks indefinitely, and preserves exact paths.
pub fn scan_worktree_status(wt: &WorktreeActivity) -> ObservedWorktree {
    let mut activity = wt.clone();
    activity.observed_at_ms = now_unix_ms();

    if activity.is_bare || activity.is_prunable {
        activity.scan_status = ScanStatus::Ok;
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
        };
    }

    let wt_path = Path::new(&activity.path);
    if !wt_path.is_dir() {
        activity.scan_status = ScanStatus::Failed;
        activity.scan_error = Some("worktree directory missing".into());
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
        };
    }

    let Some(git_bin) = find_in_path(if cfg!(windows) { "git.exe" } else { "git" }) else {
        activity.scan_status = ScanStatus::Failed;
        activity.scan_error = Some("git not found in PATH".into());
        return ObservedWorktree {
            activity,
            full_dirty_paths: Vec::new(),
        };
    };

    let mut cmd = Command::new(git_bin);
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

    match run_with_timeout(cmd, Duration::from_millis(WORKTREE_STATUS_TIMEOUT_MS)) {
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
            ObservedWorktree {
                activity,
                full_dirty_paths: all_paths,
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            activity.scan_status = ScanStatus::Failed;
            activity.scan_error = Some(if stderr.is_empty() {
                "git status exited with error".into()
            } else {
                stderr
            });
            ObservedWorktree {
                activity,
                full_dirty_paths: Vec::new(),
            }
        }
        Err(err) => {
            activity.scan_status = if err.contains("timed out") {
                ScanStatus::Timeout
            } else {
                ScanStatus::Failed
            };
            activity.scan_error = Some(err);
            ObservedWorktree {
                activity,
                full_dirty_paths: Vec::new(),
            }
        }
    }
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
                // `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>` -> 8 space-separated fields before path
                if let Some(path) = nth_space_tail(&s, 8) {
                    modified.push(normalize_rel_path(path));
                }
            }
            b'2' => {
                // `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path>\0<origPath>\0`
                if let Some(path) = nth_space_tail(&s, 9) {
                    modified.push(normalize_rel_path(path));
                }
                // Consume the NUL-terminated original path so we don't misparse it
                if i < parts.len() {
                    let orig = String::from_utf8_lossy(parts[i]);
                    if !orig.is_empty() {
                        modified.push(normalize_rel_path(&orig));
                    }
                    i += 1;
                }
            }
            b'u' => {
                // `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>` -> 10 fields before path
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

/// Computes the complete `RadarSnapshot` from live per-session leases, observed Git worktrees,
/// and the current persistent issue list.
///
/// When `observed_worktrees` is `None` and `scan_git == true`, performs a bounded local Git scan.
/// When `scan_git == false` and `cached_worktrees` is provided, reuses the cached Git observation
/// with ZERO subprocesses (used on heartbeat-only invalidations!).
pub fn compute_radar_snapshot(
    loc: &RepoLocation,
    issues: &[IssueDto],
    generation: u64,
    scan_git: bool,
    cached_worktrees: Option<&[ObservedWorktree]>,
) -> (RadarSnapshot, Vec<ObservedWorktree>) {
    let now_ms = now_unix_ms();

    let raw_leases = list_leases(&loc.runtime_dir).unwrap_or_default();
    let issue_by_id: HashMap<&str, &IssueDto> =
        issues.iter().map(|i| (i.id.as_str(), i)).collect();

    // Filter out leases bound to already-closed issues so finished work never keeps alerting
    let sessions: Vec<SessionLease> = raw_leases
        .into_iter()
        .filter(|l| {
            if l.state == SessionState::Released {
                return false;
            }
            if let Some(iid) = &l.issue_id {
                if let Some(issue) = issue_by_id.get(iid.as_str()) {
                    if issue.status == "closed" {
                        return false;
                    }
                }
            }
            true
        })
        .collect();

    // Step 1: Obtain Git worktree observations (bounded scan OR cached reuse)
    let (mut observed, list_err) = if scan_git {
        if loc.git_dir.is_some() {
            match list_worktrees_porcelain_z(&loc.worktree_root) {
                Ok(wts) => {
                    let scanned: Vec<ObservedWorktree> =
                        wts.iter().map(scan_worktree_status).collect();
                    (scanned, None)
                }
                Err(err) => {
                    // Retain prior cached worktrees marked stale rather than showing fake "no conflicts"!
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

    // Step 3: Evidence-graded 3-tier conflict radar
    let conflicts = detect_conflicts(&observed, &sessions, issues, now_ms);

    // Step 4: Actionable root blocker hotspots
    let top_blockers = compute_blocker_hotspots(issues);

    // Step 5: Honest Attention queue (WaitingHuman > FileOverlap > ProgressStale > HeartbeatStale)
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

    let coverage = RadarCoverage {
        scope_note: COVERAGE_NOTE_WORKTREE.to_string(),
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

        // Exact, unambiguous full issue ID match in branch name (e.g. `feat/bl-a1b2-auth`)
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
        let before_ok = idx == 0
            || !branch_lower.as_bytes()[idx - 1].is_ascii_alphanumeric();
        let after_idx = idx + token_lower.len();
        let after_ok = after_idx == branch_lower.len()
            || !branch_lower.as_bytes()[after_idx].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// Detects conflicts across all 3 evidence tiers, merging per participant pair so only the
/// strongest evidence tier is shown for any pair.
pub fn detect_conflicts(
    observed: &[ObservedWorktree],
    sessions: &[SessionLease],
    issues: &[IssueDto],
    observed_at_ms: u64,
) -> Vec<ConflictAlert> {
    let issue_by_id: HashMap<&str, &IssueDto> =
        issues.iter().map(|i| (i.id.as_str(), i)).collect();

    // Map pair key -> strongest ConflictAlert
    let mut best_by_pair: BTreeMap<(String, String), ConflictAlert> = BTreeMap::new();

    // Helper to build a ParticipantRef for a worktree
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

    // Tier 1: SameFileDirty across two distinct worktrees
    for i in 0..observed.len() {
        let a = &observed[i];
        if a.full_dirty_paths.is_empty() {
            continue;
        }
        let set_a: HashSet<&str> = a.full_dirty_paths.iter().map(String::as_str).collect();
        for b in observed.iter().skip(i + 1) {
            if b.full_dirty_paths.is_empty() || a.activity.worktree_id == b.activity.worktree_id {
                continue;
            }
            let mut shared: Vec<String> = b
                .full_dirty_paths
                .iter()
                .filter(|p| set_a.contains(p.as_str()))
                .cloned()
                .collect();
            if shared.is_empty() {
                continue;
            }
            shared.sort();
            shared.dedup();
            let lockfile_only = shared.iter().all(|p| is_lockfile_path(p));
            let total_paths = shared.len();
            let sample: Vec<String> = shared.into_iter().take(MAX_CONFLICT_EVIDENCE_PATHS).collect();

            let p_a = wt_participant(a);
            let p_b = wt_participant(b);
            let pair_key = ordered_pair(&p_a.id, &p_b.id);
            let alert_id = format!(
                "cfl_{}",
                fnv1a64_hex(format!("same:{}:{}", pair_key.0, pair_key.1).as_bytes())
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
                    },
                    observed_at_ms,
                },
            );
        }
    }

    // Collect effective declared scopes for active sessions (session scope merged with bound issue scope)
    let active_sessions: Vec<&SessionLease> = sessions
        .iter()
        .filter(|s| !s.stale_heartbeat && s.state != SessionState::Released)
        .collect();

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

    // Tier 2: DirtyHitsDeclaredScope (worktree dirty files hit another active session's scope)
    for wt in observed {
        if wt.full_dirty_paths.is_empty() {
            continue;
        }
        for s in &active_sessions {
            // Skip if this session is running in the exact same worktree
            if !s.worktree_id.is_empty() && s.worktree_id == wt.activity.worktree_id {
                continue;
            }
            // Skip if both are explicitly working on the exact same issue
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
            for dirty in &wt.full_dirty_paths {
                for pat in &scopes {
                    if path_matches_scope(dirty, pat) {
                        if !hit_paths.contains(dirty) {
                            hit_paths.push(dirty.clone());
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
                    },
                    observed_at_ms,
                },
            );
        }
    }

    // Tier 3: DeclaredScopeOverlap between two distinct active sessions
    for i in 0..active_sessions.len() {
        let sa = active_sessions[i];
        let scopes_a = effective_scope_for(sa);
        if scopes_a.is_empty() {
            continue;
        }
        for sb in active_sessions.iter().skip(i + 1) {
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

/// Computes open blocker hotspots along open `blocks` edges only.
/// Ranks actionable root blockers (`open_blocked_by.is_empty()`) first, then by `direct_unblocks`
/// (tasks immediately freed if this single issue closes), then transitive open downstream count.
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

        // BFS over open downstream issues only
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<&str> = Vec::new();
        let mut direct_unblocks = 0_usize;

        for child_id in &issue.blocks {
            if let Some(child) = open_by_id.get(child_id.as_str()) {
                if child.open_blocked_by.len() == 1
                    && child.open_blocked_by[0] == id
                {
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

    // 1. Explicit waiting_human requests (highest urgency)
    for s in sessions {
        if s.stale_heartbeat || s.state != SessionState::WaitingHuman {
            continue;
        }
        let age_min = age_minutes_since(&s.last_progress_at, now_ms);
        let reason = s
            .waiting_reason
            .clone()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| s.summary.clone());
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
        });
    }

    // 2. Non-lockfile SameFileDirty overlaps
    for c in conflicts {
        if c.kind != ConflictKind::SameFileDirty || c.evidence.lockfile_only {
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
        });
    }

    // 3. ProgressStale (alive heartbeat, but lastProgressAt hasn't moved for >= 20m)
    for s in sessions {
        if s.stale_heartbeat || s.state != SessionState::Working {
            continue;
        }
        let progress_age = age_minutes_since(&s.last_progress_at, now_ms);
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
            });
        }
    }

    // 4. HeartbeatStale (lease expired while bound issue is still open/in_progress — honest "telemetry lost")
    for s in sessions {
        if !s.stale_heartbeat {
            continue;
        }
        if let Some(iid) = &s.issue_id {
            if let Some(issue) = issue_by_id.get(iid.as_str()) {
                if issue.status == "closed" {
                    continue;
                }
            }
        }
        let seen_age = age_minutes_since(&s.last_seen_at, now_ms);
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
    use crate::models::{BoardLane, SmartMeta};

    fn dummy_issue(id: &str, status: &str, open_blocked_by: Vec<&str>, blocks: Vec<&str>) -> IssueDto {
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
        // root -> mid -> (leaf1, leaf2, leaf3)
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
        assert_eq!(hotspots[1].issue_id, "mid");
        assert!(!hotspots[1].actionable);
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
        };
        let wt2 = ObservedWorktree {
            activity: WorktreeActivity {
                worktree_id: "wt_2".into(),
                path: "/wt2".into(),
                branch: Some("feat/b".into()),
                ..WorktreeActivity::default()
            },
            full_dirty_paths: vec!["Cargo.lock".into()],
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
            scope: vec!["src/auth/**".into()],
            ..SessionLease::default()
        };

        let alerts = detect_conflicts(&[wt1, wt2], &[ses], &[], 1000);
        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].kind, ConflictKind::SameFileDirty);
        assert!(alerts[0].evidence.lockfile_only);
        assert_eq!(alerts[1].kind, ConflictKind::DirtyHitsDeclaredScope);
        assert_eq!(alerts[1].evidence.paths, vec!["src/auth/jwt.rs"]);
    }
}
