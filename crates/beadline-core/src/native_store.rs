use crate::error::{CoreError, CoreResult};
use crate::fs_util::{atomic_write_bytes, fnv1a64, fnv1a64_hex, FileLockGuard};
use crate::jsonl_reader::{make_short_id, read_beads_config_values};
use crate::locator::{detect_storage_backend_in_dir, RepoLocation, RepoLocator};
use crate::models::{
    CommentDto, CreateIssueRequest, DoctorReport, ExportReport, FinishResult, ImportReport,
    IssueProof, StorageBackend, UnblockedSummary, UpdateIssueRequest,
};
use crate::runtime_store::list_leases;
use crate::scope::normalize_scope_list;
use crate::time::now_rfc3339_utc;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

pub fn detect_storage_backend(repo_root: &Path) -> StorageBackend {
    if let Ok(loc) = RepoLocator::locate(repo_root) {
        return loc.storage_backend;
    }
    detect_storage_backend_in_dir(&repo_root.join(".beads"))
}

/// Initializes a Beadline v2 native workspace.
///
/// Inside a Git repository, creates `<git-common-dir>/beadline/` so all worktrees share one
/// authoritative store without polluting `git status` or `.gitignore`.
/// Outside Git, creates `<repo_root>/.beads/`.
pub fn init_native_workspace(repo_root: &Path, prefix: &str) -> CoreResult<RepoLocation> {
    let loc = RepoLocator::locate(repo_root)?;
    let target_dir = loc.shared_v2_dir();
    init_store_dir(&target_dir, prefix)?;
    RepoLocator::locate(repo_root)
}

fn init_store_dir(store_dir: &Path, prefix: &str) -> CoreResult<()> {
    let clean_prefix = sanitize_prefix(prefix);
    fs::create_dir_all(store_dir)
        .map_err(|e| CoreError::Io(format!("failed to create store dir: {e}")))?;

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let config_path = store_dir.join("config.yaml");
    if !config_path.exists() {
        let cfg = format!(
            "# Beadline / Beads Workspace Configuration\n\
             issue-prefix: \"{clean_prefix}\"\n\
             no-db: true\n"
        );
        atomic_write_bytes(&config_path, cfg.as_bytes())?;
    }

    let meta_path = store_dir.join("metadata.json");
    if !meta_path.exists() {
        let meta = json!({
            "backend": "jsonl",
            "engine": "beadline-v2",
            "prefix": clean_prefix,
            "schema_version": 1
        });
        let pretty = serde_json::to_string_pretty(&meta)
            .map_err(|e| CoreError::Internal(format!("failed to serialize metadata.json: {e}")))?;
        atomic_write_bytes(&meta_path, pretty.as_bytes())?;
    }

    let jsonl_path = store_dir.join("issues.jsonl");
    if !jsonl_path.exists() {
        atomic_write_bytes(&jsonl_path, b"")?;
    }

    let runtime_dir = store_dir.join("runtime").join("pulses");
    fs::create_dir_all(&runtime_dir)
        .map_err(|e| CoreError::Io(format!("failed to create runtime dir: {e}")))?;

    Ok(())
}

pub fn derive_prefix(repo_root: &Path) -> String {
    let name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bd");
    let slug: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(16)
        .collect();
    if slug.is_empty() {
        "bd".to_string()
    } else {
        slug
    }
}

pub fn read_inline_comments(repo_root: &Path, issue_id: &str) -> CoreResult<Vec<CommentDto>> {
    let loc = RepoLocator::locate(repo_root)?;
    read_inline_comments_in(&loc.store_dir, issue_id)
}

pub fn read_inline_comments_in(store_dir: &Path, issue_id: &str) -> CoreResult<Vec<CommentDto>> {
    let records = load_jsonl_lines(store_dir)?;
    for line in records {
        if let JsonlLine::Issue(obj) = line {
            if obj.get("id").and_then(Value::as_str) == Some(issue_id) {
                return Ok(parse_comments_from_obj(&obj));
            }
        }
    }
    Ok(Vec::new())
}

pub fn create_issue_native(
    repo_root: &Path,
    req: &CreateIssueRequest,
    prefix: &str,
) -> CoreResult<String> {
    let loc = RepoLocator::locate(repo_root)?;
    create_issue_in(&loc.store_dir, req, prefix)
}

pub fn create_issue_in(
    store_dir: &Path,
    req: &CreateIssueRequest,
    prefix: &str,
) -> CoreResult<String> {
    let title = req.title.trim();
    if title.is_empty() {
        return Err(CoreError::InvalidOperation(
            "issue title cannot be empty".into(),
        ));
    }
    let normalized_scope = normalize_scope_list(&req.scope)?;

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let existing_ids = collect_existing_ids(&lines);

    let parent_id = req
        .parent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    if let Some(pid) = &parent_id {
        if !existing_ids.contains(pid) {
            return Err(CoreError::NotFound(format!("parent issue not found: {pid}")));
        }
    }

    let now = now_rfc3339_utc();
    let actor = current_actor();
    let id = generate_issue_id(prefix, title, parent_id.as_deref(), &existing_ids);

    let status = if req.status.trim().is_empty() {
        "open".to_string()
    } else {
        req.status.trim().to_string()
    };
    let issue_type = if req.issue_type.trim().is_empty() {
        "task".to_string()
    } else {
        req.issue_type.trim().to_string()
    };

    let mut deps: Vec<Value> = Vec::new();
    if let Some(pid) = &parent_id {
        deps.push(make_dep_value(&id, pid, "parent-child", &now, &actor));
    }
    let mut seen_blockers = HashSet::new();
    for raw_b in &req.blocked_by {
        let b = raw_b.trim();
        if b.is_empty() || b == id || !existing_ids.contains(b) || !seen_blockers.insert(b) {
            continue;
        }
        deps.push(make_dep_value(&id, b, "blocks", &now, &actor));
    }

    let labels: Vec<Value> = dedup_strings(
        req.labels
            .iter()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
    )
    .into_iter()
    .map(Value::String)
    .collect();

    let mut obj = Map::new();
    obj.insert("_type".into(), Value::String("issue".into()));
    obj.insert("id".into(), Value::String(id.clone()));
    obj.insert("title".into(), Value::String(title.to_string()));
    if let Some(desc) = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("description".into(), Value::String(desc.to_string()));
    }
    if let Some(acc) = req
        .acceptance_criteria
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("acceptance_criteria".into(), Value::String(acc.to_string()));
    }
    if let Some(notes) = req
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("notes".into(), Value::String(notes.to_string()));
    }
    obj.insert("status".into(), Value::String(status.clone()));
    obj.insert("priority".into(), Value::Number(req.priority.min(4).into()));
    obj.insert("issue_type".into(), Value::String(issue_type));
    if let Some(assignee) = req
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        obj.insert("assignee".into(), Value::String(assignee.to_string()));
    }
    obj.insert("created_at".into(), Value::String(now.clone()));
    obj.insert("created_by".into(), Value::String(actor));
    obj.insert("updated_at".into(), Value::String(now.clone()));
    if status == "in_progress" {
        obj.insert("started_at".into(), Value::String(now.clone()));
    } else if status == "closed" {
        obj.insert("closed_at".into(), Value::String(now));
    }
    if !labels.is_empty() {
        obj.insert("labels".into(), Value::Array(labels));
    }
    if !deps.is_empty() {
        obj.insert("dependencies".into(), Value::Array(deps));
    }
    obj.insert("dependency_count".into(), Value::Number(0_u64.into()));
    obj.insert("dependent_count".into(), Value::Number(0_u64.into()));
    obj.insert("comment_count".into(), Value::Number(0_u64.into()));

    let mut bl_meta = Map::new();
    bl_meta.insert("schemaVersion".into(), Value::Number(1_u64.into()));
    bl_meta.insert("rev".into(), Value::Number(1_u64.into()));
    if !normalized_scope.is_empty() {
        bl_meta.insert(
            "scope".into(),
            Value::Array(normalized_scope.into_iter().map(Value::String).collect()),
        );
    }
    set_beadline_meta(&mut obj, bl_meta);

    lines.push(JsonlLine::Issue(obj));
    recompute_dep_counts(&mut lines);
    save_jsonl_lines(store_dir, &lines, Some(&id))?;
    Ok(id)
}

pub fn update_issue_native(repo_root: &Path, req: &UpdateIssueRequest) -> CoreResult<u64> {
    let loc = RepoLocator::locate(repo_root)?;
    update_issue_in(&loc.store_dir, req)
}

pub fn update_issue_in(store_dir: &Path, req: &UpdateIssueRequest) -> CoreResult<u64> {
    let normalized_scope = match &req.scope {
        Some(items) => Some(normalize_scope_list(items)?),
        None => None,
    };

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let now = now_rfc3339_utc();
    let mut next_rev = None;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(req.issue_id.as_str()) {
            continue;
        }

        let cur_rev = read_issue_rev(obj);
        if let Some(expected) = req.expected_rev {
            if expected != cur_rev {
                return Err(CoreError::StaleRevision {
                    issue_id: req.issue_id.clone(),
                    expected,
                    actual: cur_rev,
                });
            }
        }

        if let Some(title) = &req.title {
            let clean = title.trim();
            if !clean.is_empty() {
                obj.insert("title".into(), Value::String(clean.to_string()));
            }
        }
        if let Some(status) = &req.status {
            let clean = status.trim();
            if !clean.is_empty() {
                obj.insert("status".into(), Value::String(clean.to_string()));
                if clean == "in_progress" && !obj.contains_key("started_at") {
                    obj.insert("started_at".into(), Value::String(now.clone()));
                } else if clean == "closed" {
                    obj.insert("closed_at".into(), Value::String(now.clone()));
                } else {
                    obj.remove("closed_at");
                }
            }
        }
        if let Some(priority) = req.priority {
            obj.insert("priority".into(), Value::Number(priority.min(4).into()));
        }
        if let Some(assignee) = &req.assignee {
            let clean = assignee.trim();
            if clean.is_empty() {
                obj.remove("assignee");
            } else {
                obj.insert("assignee".into(), Value::String(clean.to_string()));
            }
        }
        if let Some(labels) = &req.labels {
            let clean: Vec<Value> = dedup_strings(
                labels
                    .iter()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty()),
            )
            .into_iter()
            .map(Value::String)
            .collect();
            if clean.is_empty() {
                obj.remove("labels");
            } else {
                obj.insert("labels".into(), Value::Array(clean));
            }
        }
        if let Some(desc) = &req.description {
            set_optional_string(obj, "description", desc);
        }
        if let Some(acc) = &req.acceptance_criteria {
            set_optional_string(obj, "acceptance_criteria", acc);
        }
        if let Some(notes) = &req.notes {
            set_optional_string(obj, "notes", notes);
        }

        let mut bl_meta = get_beadline_meta(obj);
        if let Some(scope) = normalized_scope {
            if scope.is_empty() {
                bl_meta.remove("scope");
            } else {
                bl_meta.insert(
                    "scope".into(),
                    Value::Array(scope.into_iter().map(Value::String).collect()),
                );
            }
        }
        let bumped = cur_rev + 1;
        bl_meta.insert("schemaVersion".into(), Value::Number(1_u64.into()));
        bl_meta.insert("rev".into(), Value::Number(bumped.into()));
        set_beadline_meta(obj, bl_meta);

        obj.insert("updated_at".into(), Value::String(now.clone()));
        next_rev = Some(bumped);
        break;
    }

    let Some(rev) = next_rev else {
        return Err(CoreError::NotFound(format!(
            "issue not found: {}",
            req.issue_id
        )));
    };

    save_jsonl_lines(store_dir, &lines, Some(&req.issue_id))?;
    Ok(rev)
}

pub fn close_issue_native(
    repo_root: &Path,
    issue_id: &str,
    reason: Option<&str>,
) -> CoreResult<()> {
    let loc = RepoLocator::locate(repo_root)?;
    let summary = reason
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("closed");
    let _ = finish_issue_in(&loc.store_dir, issue_id, None, None, summary, &[], None)?;
    Ok(())
}

/// Atomically closes an issue with structured delivery proof (`metadata.beadline.proof`),
/// optional optimistic concurrency (`expected_rev`), idempotency (`operation_id`), and returns
/// the list of downstream issues that became immediately unblocked.
pub fn finish_issue_in(
    store_dir: &Path,
    issue_id: &str,
    expected_rev: Option<u64>,
    operation_id: Option<&str>,
    proof_summary: &str,
    proof_refs: &[String],
    session_id: Option<&str>,
) -> CoreResult<FinishResult> {
    let clean_summary = proof_summary.trim();
    if clean_summary.is_empty() {
        return Err(CoreError::InvalidOperation(
            "proof summary cannot be empty".into(),
        ));
    }

    let clean_op = operation_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let now = now_rfc3339_utc();
    let actor = current_actor();

    // Snapshot open blockers BEFORE closing so we compute exact newly-unblocked tasks.
    let pre_statuses = collect_status_map(&lines);
    let mut found_idx = None;

    for (idx, line) in lines.iter().enumerate() {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) == Some(issue_id) {
            found_idx = Some(idx);
            break;
        }
    }

    let Some(idx) = found_idx else {
        return Err(CoreError::NotFound(format!("issue not found: {issue_id}")));
    };

    if let JsonlLine::Issue(obj) = &mut lines[idx] {
        let cur_rev = read_issue_rev(obj);
        let bl_meta = get_beadline_meta(obj);

        // Idempotent replay check via operationId
        if let Some(op_id) = &clean_op {
            let prev_op = bl_meta
                .get("proof")
                .and_then(|p| p.get("operationId"))
                .and_then(Value::as_str);
            if prev_op == Some(op_id.as_str())
                && obj.get("status").and_then(Value::as_str) == Some("closed")
            {
                return Ok(FinishResult {
                    id: issue_id.to_string(),
                    status: "closed".into(),
                    rev: cur_rev,
                    idempotent_replay: true,
                    unblocked: Vec::new(),
                });
            }
        }

        if let Some(expected) = expected_rev {
            if expected != cur_rev {
                return Err(CoreError::StaleRevision {
                    issue_id: issue_id.to_string(),
                    expected,
                    actual: cur_rev,
                });
            }
        }

        let clean_refs: Vec<String> = dedup_strings(
            proof_refs
                .iter()
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty()),
        );
        let proof = IssueProof {
            summary: clean_summary.to_string(),
            refs: clean_refs,
            operation_id: clean_op,
            submitted_at: now.clone(),
            session_id: session_id.map(String::from),
            actor: Some(actor),
        };

        let next_rev = cur_rev + 1;
        let mut next_bl = bl_meta;
        next_bl.insert("schemaVersion".into(), Value::Number(1_u64.into()));
        next_bl.insert("rev".into(), Value::Number(next_rev.into()));
        next_bl.insert(
            "proof".into(),
            serde_json::to_value(&proof)
                .map_err(|e| CoreError::Internal(format!("failed to serialize proof: {e}")))?,
        );
        set_beadline_meta(obj, next_bl);

        obj.insert("status".into(), Value::String("closed".into()));
        obj.insert("closed_at".into(), Value::String(now.clone()));
        obj.insert("updated_at".into(), Value::String(now));
        obj.insert("close_reason".into(), Value::String(clean_summary.to_string()));
    }

    let post_statuses = collect_status_map(&lines);
    let unblocked = compute_newly_unblocked(&lines, issue_id, &pre_statuses, &post_statuses);

    save_jsonl_lines(store_dir, &lines, Some(issue_id))?;

    let final_rev = match &lines[idx] {
        JsonlLine::Issue(obj) => read_issue_rev(obj),
        _ => 1,
    };

    Ok(FinishResult {
        id: issue_id.to_string(),
        status: "closed".into(),
        rev: final_rev,
        idempotent_replay: false,
        unblocked,
    })
}

pub fn reopen_issue_native(
    repo_root: &Path,
    issue_id: &str,
    _reason: Option<&str>,
) -> CoreResult<u64> {
    let loc = RepoLocator::locate(repo_root)?;
    reopen_issue_in(&loc.store_dir, issue_id)
}

pub fn reopen_issue_in(store_dir: &Path, issue_id: &str) -> CoreResult<u64> {
    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let now = now_rfc3339_utc();
    let mut new_rev = None;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(issue_id) {
            continue;
        }
        let next = read_issue_rev(obj) + 1;
        let mut bl = get_beadline_meta(obj);
        bl.insert("schemaVersion".into(), Value::Number(1_u64.into()));
        bl.insert("rev".into(), Value::Number(next.into()));
        set_beadline_meta(obj, bl);

        obj.insert("status".into(), Value::String("open".into()));
        obj.remove("closed_at");
        obj.insert("updated_at".into(), Value::String(now.clone()));
        new_rev = Some(next);
        break;
    }

    let Some(rev) = new_rev else {
        return Err(CoreError::NotFound(format!("issue not found: {issue_id}")));
    };

    save_jsonl_lines(store_dir, &lines, Some(issue_id))?;
    Ok(rev)
}

pub fn mutate_dependency_native(
    repo_root: &Path,
    blocked_id: &str,
    blocker_id: &str,
    add: bool,
) -> CoreResult<()> {
    let loc = RepoLocator::locate(repo_root)?;
    mutate_dependency_in(&loc.store_dir, blocked_id, blocker_id, add)
}

pub fn mutate_dependency_in(
    store_dir: &Path,
    blocked_id: &str,
    blocker_id: &str,
    add: bool,
) -> CoreResult<()> {
    if blocked_id == blocker_id {
        return Err(CoreError::InvalidOperation(
            "an issue cannot depend on itself".into(),
        ));
    }

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let existing = collect_existing_ids(&lines);
    if !existing.contains(blocked_id) {
        return Err(CoreError::NotFound(format!(
            "issue not found: {blocked_id}"
        )));
    }
    if add && !existing.contains(blocker_id) {
        return Err(CoreError::NotFound(format!(
            "blocker issue not found: {blocker_id}"
        )));
    }

    if add && would_create_cycle(&lines, blocked_id, blocker_id) {
        return Err(CoreError::InvalidOperation(format!(
            "adding dependency {blocked_id} -> {blocker_id} would create a cycle"
        )));
    }

    let now = now_rfc3339_utc();
    let actor = current_actor();

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(blocked_id) {
            continue;
        }
        let mut deps = obj
            .get("dependencies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if add {
            let already_exists = deps.iter().any(|d| {
                d.get("type").and_then(Value::as_str) == Some("blocks")
                    && d.get("depends_on_id").and_then(Value::as_str) == Some(blocker_id)
            });
            if !already_exists {
                deps.push(make_dep_value(
                    blocked_id, blocker_id, "blocks", &now, &actor,
                ));
            }
        } else {
            deps.retain(|d| {
                !(d.get("type").and_then(Value::as_str) == Some("blocks")
                    && d.get("depends_on_id").and_then(Value::as_str) == Some(blocker_id))
            });
        }

        if deps.is_empty() {
            obj.remove("dependencies");
        } else {
            obj.insert("dependencies".into(), Value::Array(deps));
        }

        let next = read_issue_rev(obj) + 1;
        let mut bl = get_beadline_meta(obj);
        bl.insert("schemaVersion".into(), Value::Number(1_u64.into()));
        bl.insert("rev".into(), Value::Number(next.into()));
        set_beadline_meta(obj, bl);

        obj.insert("updated_at".into(), Value::String(now.clone()));
        break;
    }

    recompute_dep_counts(&mut lines);
    save_jsonl_lines(store_dir, &lines, Some(blocked_id))
}

pub fn add_comment_native(repo_root: &Path, issue_id: &str, text: &str) -> CoreResult<()> {
    let loc = RepoLocator::locate(repo_root)?;
    add_comment_in(&loc.store_dir, issue_id, text)
}

pub fn add_comment_in(store_dir: &Path, issue_id: &str, text: &str) -> CoreResult<()> {
    let clean = text.trim();
    if clean.is_empty() {
        return Err(CoreError::InvalidOperation(
            "comment text cannot be empty".into(),
        ));
    }

    let lock_path = store_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = load_jsonl_lines(store_dir)?;
    let now = now_rfc3339_utc();
    let actor = current_actor();
    let mut found = false;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(issue_id) {
            continue;
        }
        found = true;

        let mut comments = obj
            .get("comments")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let comment_id = format!("c-{}", comments.len() + 1);
        comments.push(json!({
            "id": comment_id,
            "issue_id": issue_id,
            "author": actor,
            "text": clean,
            "created_at": now,
        }));

        let count = comments.len() as u64;
        obj.insert("comments".into(), Value::Array(comments));
        obj.insert("comment_count".into(), Value::Number(count.into()));
        obj.insert("updated_at".into(), Value::String(now.clone()));
        break;
    }

    if !found {
        return Err(CoreError::NotFound(format!("issue not found: {issue_id}")));
    }

    save_jsonl_lines(store_dir, &lines, Some(issue_id))
}

/// Imports an external `.beads/issues.jsonl` into the authoritative `<git-common-dir>/beadline/`
/// store with strict diagnostics (malformed lines, duplicate IDs, dangling deps) and records the
/// exchange baseline digest in `metadata.json`.
pub fn import_jsonl_into_shared(
    repo_root: &Path,
    source_path: &Path,
    replace_existing: bool,
) -> CoreResult<ImportReport> {
    if !source_path.is_file() {
        return Err(CoreError::NotFound(format!(
            "import source does not exist: {}",
            source_path.display()
        )));
    }
    let raw_bytes = fs::read(source_path)
        .map_err(|e| CoreError::Io(format!("failed to read import source: {e}")))?;
    let source_digest = fnv1a64_hex(&raw_bytes);
    let raw_str = String::from_utf8_lossy(&raw_bytes);

    let mut parsed_issues: Vec<Map<String, Value>> = Vec::new();
    let mut skipped_malformed_lines = Vec::new();
    let mut seen_in_source = HashSet::new();
    let mut duplicate_ids = Vec::new();

    for (idx, line) in raw_str.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(Value::Object(obj)) => {
                let rec_type = obj.get("_type").and_then(Value::as_str).unwrap_or("");
                if (rec_type.is_empty() || rec_type == "issue")
                    && obj.get("id").and_then(Value::as_str).is_some()
                {
                    let id = obj.get("id").and_then(Value::as_str).unwrap().to_string();
                    if !seen_in_source.insert(id.clone()) {
                        duplicate_ids.push(id);
                        continue;
                    }
                    parsed_issues.push(obj);
                }
            }
            _ => {
                skipped_malformed_lines.push(idx + 1);
            }
        }
    }

    // Detect dangling dependencies within source
    let mut dangling_dependencies = Vec::new();
    for obj in &parsed_issues {
        let id = obj.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(deps) = obj.get("dependencies").and_then(Value::as_array) {
            for dep in deps {
                if let Some(target) = dep.get("depends_on_id").and_then(Value::as_str) {
                    if !target.is_empty() && !seen_in_source.contains(target) {
                        dangling_dependencies.push(format!("{id} -> {target}"));
                    }
                }
            }
        }
    }

    let loc = RepoLocator::locate(repo_root)?;
    let target_dir = loc.shared_v2_dir();
    let (src_prefix, _) = source_path
        .parent()
        .map(read_beads_config_values)
        .unwrap_or((None, None));
    let prefix = src_prefix.unwrap_or_else(|| derive_prefix(&loc.worktree_root));
    init_store_dir(&target_dir, &prefix)?;

    let lock_path = target_dir.join("issues.lock");
    let _guard = FileLockGuard::acquire(&lock_path, LOCK_TIMEOUT)?;

    let mut lines = if replace_existing {
        Vec::new()
    } else {
        load_jsonl_lines(&target_dir)?
    };

    let mut index_by_id: HashMap<String, usize> = HashMap::new();
    for (idx, line) in lines.iter().enumerate() {
        if let JsonlLine::Issue(obj) = line {
            if let Some(id) = obj.get("id").and_then(Value::as_str) {
                index_by_id.insert(id.to_string(), idx);
            }
        }
    }

    let mut imported_count = 0_usize;
    let mut updated_count = 0_usize;

    for incoming in parsed_issues {
        let id = incoming
            .get("id")
            .and_then(Value::as_str)
            .unwrap()
            .to_string();
        if let Some(&existing_idx) = index_by_id.get(&id) {
            // Preserve existing Beadline metadata extensions unless incoming also has them
            let mut merged = incoming;
            if let JsonlLine::Issue(existing_obj) = &lines[existing_idx] {
                let existing_bl = get_beadline_meta(existing_obj);
                if !existing_bl.is_empty() && get_beadline_meta(&merged).is_empty() {
                    set_beadline_meta(&mut merged, existing_bl);
                }
            }
            lines[existing_idx] = JsonlLine::Issue(merged);
            updated_count += 1;
        } else {
            let idx = lines.len();
            lines.push(JsonlLine::Issue(incoming));
            index_by_id.insert(id, idx);
            imported_count += 1;
        }
    }

    recompute_dep_counts(&mut lines);
    save_jsonl_lines(&target_dir, &lines, None)?;
    record_exchange_baseline(&target_dir, source_path, &source_digest)?;

    Ok(ImportReport {
        source_path: source_path.to_string_lossy().to_string(),
        store_path: target_dir.join("issues.jsonl").to_string_lossy().to_string(),
        imported_count,
        updated_count,
        skipped_malformed_lines,
        duplicate_ids,
        dangling_dependencies,
        source_digest,
    })
}

/// Exports the authoritative task store to `dest_path` in either `bd-v1` (Beads JSONL v1
/// compatible, mapping `proof.summary` to `close_reason`) or `native` (full lossless) format.
///
/// Refuses to overwrite a modified destination whose digest drifted from the recorded exchange
/// baseline unless `force = true`.
pub fn export_jsonl_from_store(
    repo_root: &Path,
    dest_path: &Path,
    format: &str,
    force: bool,
) -> CoreResult<ExportReport> {
    if format != "bd-v1" && format != "native" {
        return Err(CoreError::InvalidOperation(format!(
            "unsupported export format '{format}'; expected 'bd-v1' or 'native'"
        )));
    }

    let loc = RepoLocator::locate(repo_root)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }

    if dest_path.exists() && !force {
        if let Some(expected_digest) = read_exchange_baseline(&loc.store_dir, dest_path) {
            let current_bytes = fs::read(dest_path)
                .map_err(|e| CoreError::Io(format!("failed to read destination: {e}")))?;
            let current_digest = fnv1a64_hex(&current_bytes);
            if current_digest != expected_digest {
                return Err(CoreError::InvalidOperation(format!(
                    "destination {} changed since last import/export (baseline {expected_digest}, current {current_digest}); inspect diff or pass --force",
                    dest_path.display()
                )));
            }
        }
    }

    let lines = load_jsonl_lines(&loc.store_dir)?;
    let mut out = String::new();
    let mut exported_count = 0_usize;
    let mut stripped_extensions = 0_usize;

    for line in &lines {
        match line {
            JsonlLine::Issue(obj) => {
                exported_count += 1;
                let mut record = obj.clone();
                if format == "bd-v1" {
                    let bl = get_beadline_meta(&record);
                    if !bl.is_empty() {
                        stripped_extensions += 1;
                        // Map proof summary into close_reason if not already set
                        if !record.contains_key("close_reason") {
                            if let Some(summary) = bl
                                .get("proof")
                                .and_then(|p| p.get("summary"))
                                .and_then(Value::as_str)
                                .filter(|s| !s.trim().is_empty())
                            {
                                record.insert(
                                    "close_reason".into(),
                                    Value::String(summary.to_string()),
                                );
                            }
                        }
                        if let Some(Value::Object(meta)) = record.get_mut("metadata") {
                            meta.remove("beadline");
                            if meta.is_empty() {
                                record.remove("metadata");
                            }
                        }
                    }
                }
                let serialized = serde_json::to_string(&record)
                    .map_err(|e| CoreError::Internal(format!("failed to serialize issue: {e}")))?;
                out.push_str(&serialized);
                out.push('\n');
            }
            JsonlLine::Raw(raw) => {
                out.push_str(raw);
                out.push('\n');
            }
        }
    }

    let bytes = out.as_bytes();
    let exported_digest = fnv1a64_hex(bytes);
    atomic_write_bytes(dest_path, bytes)?;
    let _ = record_exchange_baseline(&loc.store_dir, dest_path, &exported_digest);

    let mut warnings = Vec::new();
    if format == "bd-v1" && stripped_extensions > 0 {
        warnings.push(format!(
            "bd-v1 export omitted metadata.beadline (scope/proof/rev) on {stripped_extensions} issue(s); proof summaries were mapped to close_reason when absent"
        ));
    }

    Ok(ExportReport {
        format: format.to_string(),
        dest_path: dest_path.to_string_lossy().to_string(),
        exported_count,
        exported_digest,
        warnings,
    })
}

pub fn run_doctor(repo_root: &Path, version: &str) -> CoreResult<DoctorReport> {
    let loc = RepoLocator::locate(repo_root)?;
    let mut warnings = Vec::new();

    if !loc.has_store {
        warnings.push("Workspace is not initialized yet. Run `bl init` or import `.beads/issues.jsonl`.".into());
    }
    if loc.storage_backend == StorageBackend::DoltCli {
        warnings.push(
            "Workspace still points to a legacy Dolt .beads directory. Export from bd once and run `bl import --from .beads/issues.jsonl` to promote to the shared native Rust store.".into(),
        );
    }

    let issue_count = if loc.has_store {
        load_jsonl_lines(&loc.store_dir)
            .map(|l| l.iter().filter(|x| matches!(x, JsonlLine::Issue(_))).count())
            .unwrap_or(0)
    } else {
        0
    };

    let active_sessions = list_leases(&loc.runtime_dir)
        .map(|s| {
            s.into_iter()
                .filter(|l| !l.stale_heartbeat && l.state != crate::models::SessionState::Released)
                .count()
        })
        .unwrap_or(0);

    Ok(DoctorReport {
        ok: loc.has_store && loc.storage_backend == StorageBackend::NativeJsonl,
        version: version.to_string(),
        repo_id: loc.repo_id,
        repo_name: loc.repo_name,
        worktree_root: loc.worktree_display,
        worktree_id: loc.worktree_id,
        branch: loc.branch,
        git_common_dir: loc
            .git_common_dir
            .map(|p| p.to_string_lossy().to_string()),
        store_dir: loc.store_dir.to_string_lossy().to_string(),
        store_kind: loc.store_kind,
        storage_backend: loc.storage_backend,
        has_store: loc.has_store,
        runtime_dir: loc.runtime_dir.to_string_lossy().to_string(),
        issue_count,
        active_sessions,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

enum JsonlLine {
    Issue(Map<String, Value>),
    Raw(String),
}

fn load_jsonl_lines(store_dir: &Path) -> CoreResult<Vec<JsonlLine>> {
    let jsonl_path = store_dir.join("issues.jsonl");
    if !jsonl_path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&jsonl_path)
        .map_err(|e| CoreError::Io(format!("failed to read issues.jsonl: {e}")))?;

    let mut lines = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(Value::Object(obj)) => {
                let rec_type = obj.get("_type").and_then(Value::as_str).unwrap_or("");
                if (rec_type.is_empty() || rec_type == "issue")
                    && obj.get("id").and_then(Value::as_str).is_some()
                {
                    lines.push(JsonlLine::Issue(obj));
                } else {
                    lines.push(JsonlLine::Raw(trimmed.to_string()));
                }
            }
            _ => lines.push(JsonlLine::Raw(trimmed.to_string())),
        }
    }
    Ok(lines)
}

fn save_jsonl_lines(
    store_dir: &Path,
    lines: &[JsonlLine],
    touched_id: Option<&str>,
) -> CoreResult<()> {
    fs::create_dir_all(store_dir)
        .map_err(|e| CoreError::Io(format!("failed to create store dir: {e}")))?;

    let mut out = String::new();
    for line in lines {
        match line {
            JsonlLine::Issue(obj) => {
                let serialized = serde_json::to_string(obj).map_err(|e| {
                    CoreError::Internal(format!("failed to serialize issue record: {e}"))
                })?;
                out.push_str(&serialized);
                out.push('\n');
            }
            JsonlLine::Raw(raw) => {
                out.push_str(raw);
                out.push('\n');
            }
        }
    }

    let jsonl_path = store_dir.join("issues.jsonl");
    atomic_write_bytes(&jsonl_path, out.as_bytes())?;

    if let Some(id) = touched_id {
        let _ = fs::write(store_dir.join("last-touched"), format!("{id}\n"));
    }

    Ok(())
}

fn get_beadline_meta(obj: &Map<String, Value>) -> Map<String, Value> {
    obj.get("metadata")
        .and_then(Value::as_object)
        .and_then(|m| m.get("beadline"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn set_beadline_meta(obj: &mut Map<String, Value>, bl: Map<String, Value>) {
    let mut meta = obj
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    meta.insert("beadline".into(), Value::Object(bl));
    obj.insert("metadata".into(), Value::Object(meta));
}

fn read_issue_rev(obj: &Map<String, Value>) -> u64 {
    obj.get("metadata")
        .and_then(Value::as_object)
        .and_then(|m| m.get("beadline"))
        .and_then(Value::as_object)
        .and_then(|b| b.get("rev"))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1)
}

fn collect_status_map(lines: &[JsonlLine]) -> HashMap<String, String> {
    lines
        .iter()
        .filter_map(|l| match l {
            JsonlLine::Issue(obj) => Some((
                obj.get("id").and_then(Value::as_str)?.to_string(),
                obj.get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("open")
                    .to_string(),
            )),
            JsonlLine::Raw(_) => None,
        })
        .collect()
}

fn compute_newly_unblocked(
    lines: &[JsonlLine],
    closed_id: &str,
    pre: &HashMap<String, String>,
    post: &HashMap<String, String>,
) -> Vec<UnblockedSummary> {
    let mut result = Vec::new();
    for line in lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id == closed_id {
            continue;
        }
        let status = obj.get("status").and_then(Value::as_str).unwrap_or("open");
        if status == "closed" {
            continue;
        }
        let Some(deps) = obj.get("dependencies").and_then(Value::as_array) else {
            continue;
        };
        let blockers: Vec<&str> = deps
            .iter()
            .filter(|d| d.get("type").and_then(Value::as_str) == Some("blocks"))
            .filter_map(|d| d.get("depends_on_id").and_then(Value::as_str))
            .collect();

        if !blockers.contains(&closed_id) {
            continue;
        }
        let had_open_before = blockers
            .iter()
            .any(|b| pre.get(*b).map(String::as_str) != Some("closed"));
        let has_open_after = blockers
            .iter()
            .any(|b| post.get(*b).map(String::as_str) != Some("closed"));
        if had_open_before && !has_open_after {
            let ext_ref = obj
                .get("external_ref")
                .and_then(Value::as_str)
                .unwrap_or("");
            let title = obj
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            result.push(UnblockedSummary {
                id: id.to_string(),
                short_id: make_short_id(id, ext_ref),
                title,
            });
        }
    }
    result
}

fn record_exchange_baseline(store_dir: &Path, path: &Path, digest: &str) -> CoreResult<()> {
    let meta_path = store_dir.join("metadata.json");
    let mut meta = fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let key = crate::fs_util::canonical_path_key(path);
    let mut baselines = meta
        .get("exchangeBaselines")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    baselines.insert(key, Value::String(digest.to_string()));
    meta.insert("exchangeBaselines".into(), Value::Object(baselines));

    let pretty = serde_json::to_string_pretty(&Value::Object(meta))
        .map_err(|e| CoreError::Internal(format!("failed to serialize metadata.json: {e}")))?;
    atomic_write_bytes(&meta_path, pretty.as_bytes())
}

fn read_exchange_baseline(store_dir: &Path, path: &Path) -> Option<String> {
    let meta_path = store_dir.join("metadata.json");
    let raw = fs::read_to_string(meta_path).ok()?;
    let val = serde_json::from_str::<Value>(&raw).ok()?;
    let key = crate::fs_util::canonical_path_key(path);
    val.get("exchangeBaselines")?
        .get(&key)?
        .as_str()
        .map(String::from)
}

fn collect_existing_ids(lines: &[JsonlLine]) -> HashSet<String> {
    lines
        .iter()
        .filter_map(|line| match line {
            JsonlLine::Issue(obj) => obj.get("id").and_then(Value::as_str).map(String::from),
            JsonlLine::Raw(_) => None,
        })
        .collect()
}

fn would_create_cycle(lines: &[JsonlLine], blocked_id: &str, blocker_id: &str) -> bool {
    let mut deps_of: HashMap<&str, Vec<&str>> = HashMap::new();
    for line in lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(arr) = obj.get("dependencies").and_then(Value::as_array) {
            for dep in arr {
                if dep.get("type").and_then(Value::as_str) == Some("blocks") {
                    if let Some(target) = dep.get("depends_on_id").and_then(Value::as_str) {
                        deps_of.entry(id).or_default().push(target);
                    }
                }
            }
        }
    }

    let mut stack = vec![blocker_id];
    let mut visited = HashSet::new();
    while let Some(cur) = stack.pop() {
        if cur == blocked_id {
            return true;
        }
        if !visited.insert(cur) {
            continue;
        }
        if let Some(nexts) = deps_of.get(cur) {
            for &nxt in nexts {
                stack.push(nxt);
            }
        }
    }
    false
}

fn recompute_dep_counts(lines: &mut [JsonlLine]) {
    let mut dep_counts: HashMap<String, u64> = HashMap::new();
    let mut dependent_counts: HashMap<String, u64> = HashMap::new();

    for line in lines.iter() {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(arr) = obj.get("dependencies").and_then(Value::as_array) {
            for dep in arr {
                if dep.get("type").and_then(Value::as_str) == Some("blocks") {
                    if let Some(target) = dep.get("depends_on_id").and_then(Value::as_str) {
                        *dep_counts.entry(id.to_string()).or_insert(0) += 1;
                        *dependent_counts.entry(target.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    for line in lines.iter_mut() {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str).map(String::from) else {
            continue;
        };
        let dc = dep_counts.get(&id).copied().unwrap_or(0);
        let dpc = dependent_counts.get(&id).copied().unwrap_or(0);
        obj.insert("dependency_count".into(), Value::Number(dc.into()));
        obj.insert("dependent_count".into(), Value::Number(dpc.into()));
    }
}

fn parse_comments_from_obj(obj: &Map<String, Value>) -> Vec<CommentDto> {
    let Some(arr) = obj.get("comments").and_then(Value::as_array) else {
        return Vec::new();
    };
    arr.iter()
        .enumerate()
        .map(|(idx, item)| CommentDto {
            id: item
                .get("id")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| format!("c-{}", idx + 1)),
            author: item
                .get("author")
                .and_then(Value::as_str)
                .unwrap_or("note")
                .to_string(),
            text: item
                .get("text")
                .or_else(|| item.get("body"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            created_at: item
                .get("created_at")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .collect()
}

fn make_dep_value(
    issue_id: &str,
    depends_on_id: &str,
    dep_type: &str,
    now: &str,
    actor: &str,
) -> Value {
    json!({
        "issue_id": issue_id,
        "depends_on_id": depends_on_id,
        "type": dep_type,
        "created_at": now,
        "created_by": actor,
        "metadata": "{}",
    })
}

fn set_optional_string(obj: &mut Map<String, Value>, key: &str, val: &str) {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        obj.remove(key);
    } else {
        obj.insert(key.into(), Value::String(trimmed.to_string()));
    }
}

fn sanitize_prefix(raw: &str) -> String {
    let clean: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if clean.is_empty() {
        "bd".to_string()
    } else {
        clean
    }
}

fn generate_issue_id(
    prefix: &str,
    title: &str,
    parent_id: Option<&str>,
    existing_ids: &HashSet<String>,
) -> String {
    if let Some(pid) = parent_id {
        let dot_prefix = format!("{pid}.");
        let max_num = existing_ids
            .iter()
            .filter_map(|id| id.strip_prefix(&dot_prefix))
            .filter_map(|rest| rest.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let mut next = max_num + 1;
        loop {
            let candidate = format!("{pid}.{next}");
            if !existing_ids.contains(&candidate) {
                return candidate;
            }
            next += 1;
        }
    }

    let clean_prefix = sanitize_prefix(prefix);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for attempt in 0_u64..1000 {
        let seed = format!("{clean_prefix}:{title}:{nanos}:{attempt}:{}", std::process::id());
        let h = fnv1a64(seed.as_bytes());
        let code = encode_base36(h, 4);
        let candidate = format!("{clean_prefix}-{code}");
        if !existing_ids.contains(&candidate) {
            return candidate;
        }
    }
    format!("{clean_prefix}-{:08x}", nanos as u32)
}

fn encode_base36(mut value: u64, len: usize) -> String {
    const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut chars = Vec::with_capacity(len);
    for _ in 0..len {
        chars.push(ALPHABET[(value % 36) as usize] as char);
        value /= 36;
    }
    chars.into_iter().rev().collect()
}

fn dedup_strings(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

fn current_actor() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "beadline".to_string())
}

/// Helper to locate canonical store path for a repository without exposing internal details.
pub fn resolve_store_dir(repo_root: &Path) -> CoreResult<PathBuf> {
    Ok(RepoLocator::locate(repo_root)?.store_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonl_reader::read_workspace_snapshot;
    use std::sync::Arc;
    use std::thread;

    fn sample_req(ws: &Path, title: &str) -> CreateIssueRequest {
        CreateIssueRequest {
            workspace_path: ws.to_string_lossy().to_string(),
            title: title.to_string(),
            issue_type: "task".into(),
            priority: 1,
            status: "open".into(),
            assignee: None,
            parent_id: None,
            labels: vec!["mod:core".into()],
            description: Some("Why it exists".into()),
            acceptance_criteria: Some("Tests pass".into()),
            notes: None,
            scope: vec!["src/core/**".into()],
            blocked_by: vec![],
        }
    }

    #[test]
    fn finish_is_idempotent_checks_rev_and_returns_unblocked() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_native_workspace(dir.path(), "bl").unwrap();
        let a = create_issue_native(dir.path(), &sample_req(dir.path(), "Blocker A"), "bl").unwrap();
        let mut req_b = sample_req(dir.path(), "Downstream B");
        req_b.blocked_by = vec![a.clone()];
        let b = create_issue_native(dir.path(), &req_b, "bl").unwrap();

        let loc = RepoLocator::locate(dir.path()).unwrap();

        // Stale rev rejected
        let err = finish_issue_in(&loc.store_dir, &a, Some(99), Some("op-1"), "done", &[], None)
            .unwrap_err();
        assert_eq!(err.code(), "STALE_REV");

        // Proper finish unlocks B
        let res = finish_issue_in(
            &loc.store_dir,
            &a,
            Some(1),
            Some("op-1"),
            "all tests pass",
            &["commit:abc1234".into()],
            Some("ses_1"),
        )
        .unwrap();
        assert!(!res.idempotent_replay);
        assert_eq!(res.rev, 2);
        assert_eq!(res.unblocked.len(), 1);
        assert_eq!(res.unblocked[0].id, b);

        // Replay with same operationId succeeds idempotently without bumping rev
        let replay = finish_issue_in(
            &loc.store_dir,
            &a,
            Some(1),
            Some("op-1"),
            "all tests pass",
            &[],
            None,
        )
        .unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(replay.rev, 2);
    }

    #[test]
    fn ten_concurrent_writers_never_lose_updates() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_native_workspace(dir.path(), "conc").unwrap();
        let root = Arc::new(dir.path().to_path_buf());

        let mut handles = Vec::new();
        for worker in 0..10 {
            let r = Arc::clone(&root);
            handles.push(thread::spawn(move || {
                for n in 0..3 {
                    let req = sample_req(&r, &format!("Task w{worker}-{n}"));
                    create_issue_native(&r, &req, "conc").unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        let snap = read_workspace_snapshot(&root, root.to_string_lossy().into(), "mod:", None, &[], &[])
            .unwrap();
        assert_eq!(snap.issues.len(), 30);
    }

    #[test]
    fn import_and_bd_v1_export_protect_against_drift() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Make it a real git repo so shared_v2_dir is .git/beadline
        let mut cmd = std::process::Command::new("git");
        cmd.args(["init", "-b", "main"]).current_dir(dir.path());
        crate::fs_util::configure_no_window(&mut cmd);
        assert!(cmd.output().unwrap().status.success());

        let legacy_beads = dir.path().join(".beads");
        fs::create_dir_all(&legacy_beads).unwrap();
        let exchange_file = legacy_beads.join("issues.jsonl");
        fs::write(
            &exchange_file,
            "{\"_type\":\"issue\",\"id\":\"bd-1\",\"title\":\"Imported\",\"status\":\"open\",\"priority\":1,\"issue_type\":\"task\",\"dependencies\":[{\"depends_on_id\":\"missing-9\",\"type\":\"blocks\"}]}\nnot-json-line\n",
        )
        .unwrap();

        let rep = import_jsonl_into_shared(dir.path(), &exchange_file, false).unwrap();
        assert_eq!(rep.imported_count, 1);
        assert_eq!(rep.skipped_malformed_lines, vec![2]);
        assert_eq!(rep.dangling_dependencies, vec!["bd-1 -> missing-9"]);

        // Export back succeeds when destination matches baseline
        let exp = export_jsonl_from_store(dir.path(), &exchange_file, "bd-v1", false).unwrap();
        assert_eq!(exp.exported_count, 1);

        // External edit causes baseline drift -> refused without --force
        fs::write(&exchange_file, "{\"id\":\"external\"}\n").unwrap();
        assert!(export_jsonl_from_store(dir.path(), &exchange_file, "bd-v1", false).is_err());
        assert!(export_jsonl_from_store(dir.path(), &exchange_file, "bd-v1", true).is_ok());
    }
}
