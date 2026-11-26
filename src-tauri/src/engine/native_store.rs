use crate::error::{AppError, AppResult};
use crate::models::issue::{CommentDto, CreateIssueRequest, StorageBackend, UpdateIssueRequest};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Detects whether a `.beads` workspace uses an external/embedded Dolt database
/// or Beadline's built-in Rust JSONL engine.
pub fn detect_storage_backend(repo_root: &Path) -> StorageBackend {
    let beads_dir = repo_root.join(".beads");
    if !beads_dir.is_dir() {
        return StorageBackend::NativeJsonl;
    }

    if beads_dir.join("embeddeddolt").is_dir() || beads_dir.join("dolt").is_dir() {
        return StorageBackend::DoltCli;
    }

    let meta_path = beads_dir.join("metadata.json");
    if let Ok(raw) = fs::read_to_string(meta_path) {
        if let Ok(val) = serde_json::from_str::<Value>(&raw) {
            if val
                .get("backend")
                .and_then(Value::as_str)
                .is_some_and(|b| b.eq_ignore_ascii_case("dolt"))
            {
                return StorageBackend::DoltCli;
            }
        }
    }

    StorageBackend::NativeJsonl
}

/// Non-invasively initializes a `.beads/` workspace powered by Beadline's built-in
/// Rust JSONL engine. Writes `.beads/` to `.git/info/exclude` if inside a Git repo,
/// never installs Git hooks, and never modifies `AGENTS.md` or `.gitignore`.
pub fn init_native_workspace(repo_root: &Path, prefix: &str) -> AppResult<()> {
    let clean_prefix = sanitize_prefix(prefix);
    let beads_dir = repo_root.join(".beads");
    fs::create_dir_all(&beads_dir)
        .map_err(|e| AppError::Internal(format!("failed to create .beads directory: {e}")))?;

    let config_path = beads_dir.join("config.yaml");
    if !config_path.exists() {
        let cfg = format!(
            "# Beads / Beadline Workspace Configuration\n\
             issue-prefix: \"{clean_prefix}\"\n\
             no-db: true\n"
        );
        atomic_write_bytes(&config_path, cfg.as_bytes())?;
    }

    let meta_path = beads_dir.join("metadata.json");
    if !meta_path.exists() {
        let meta = json!({
            "backend": "jsonl",
            "engine": "beadline-native",
            "prefix": clean_prefix,
            "schema_version": 1
        });
        let pretty = serde_json::to_string_pretty(&meta)
            .map_err(|e| AppError::Internal(format!("failed to serialize metadata.json: {e}")))?;
        atomic_write_bytes(&meta_path, pretty.as_bytes())?;
    }

    let gitignore_path = beads_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let content = "*.tmp\n*.lock\n";
        atomic_write_bytes(&gitignore_path, content.as_bytes())?;
    }

    let jsonl_path = beads_dir.join("issues.jsonl");
    if !jsonl_path.exists() {
        atomic_write_bytes(&jsonl_path, b"")?;
    }

    ensure_git_info_exclude(repo_root);
    Ok(())
}

/// Reads inline `comments` directly from `.beads/issues.jsonl` without spawning `bd`.
pub fn read_inline_comments(repo_root: &Path, issue_id: &str) -> AppResult<Vec<CommentDto>> {
    let records = load_jsonl_lines(repo_root)?;
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
) -> AppResult<String> {
    let title = req.title.trim();
    if title.is_empty() {
        return Err(AppError::InvalidOperation(
            "issue title cannot be empty".into(),
        ));
    }

    let mut lines = load_jsonl_lines(repo_root)?;
    let existing_ids = collect_existing_ids(&lines);

    let parent_id = req
        .parent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    if let Some(pid) = &parent_id {
        if !existing_ids.contains(pid) {
            return Err(AppError::InvalidOperation(format!(
                "parent issue not found: {pid}"
            )));
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

    lines.push(JsonlLine::Issue(obj));
    recompute_dep_counts(&mut lines);
    save_jsonl_lines(repo_root, &lines, Some(&id))?;
    Ok(id)
}

pub fn update_issue_native(repo_root: &Path, req: &UpdateIssueRequest) -> AppResult<()> {
    let mut lines = load_jsonl_lines(repo_root)?;
    let now = now_rfc3339_utc();
    let mut found = false;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(req.issue_id.as_str()) {
            continue;
        }
        found = true;

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
        obj.insert("updated_at".into(), Value::String(now.clone()));
        break;
    }

    if !found {
        return Err(AppError::InvalidOperation(format!(
            "issue not found: {}",
            req.issue_id
        )));
    }

    save_jsonl_lines(repo_root, &lines, Some(&req.issue_id))
}

pub fn close_issue_native(repo_root: &Path, issue_id: &str, reason: Option<&str>) -> AppResult<()> {
    let mut lines = load_jsonl_lines(repo_root)?;
    let now = now_rfc3339_utc();
    let mut found = false;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(issue_id) {
            continue;
        }
        found = true;
        obj.insert("status".into(), Value::String("closed".into()));
        obj.insert("closed_at".into(), Value::String(now.clone()));
        obj.insert("updated_at".into(), Value::String(now.clone()));
        if let Some(r) = reason.map(str::trim).filter(|s| !s.is_empty()) {
            obj.insert("close_reason".into(), Value::String(r.to_string()));
        }
        break;
    }

    if !found {
        return Err(AppError::InvalidOperation(format!(
            "issue not found: {issue_id}"
        )));
    }

    save_jsonl_lines(repo_root, &lines, Some(issue_id))
}

pub fn reopen_issue_native(
    repo_root: &Path,
    issue_id: &str,
    _reason: Option<&str>,
) -> AppResult<()> {
    let mut lines = load_jsonl_lines(repo_root)?;
    let now = now_rfc3339_utc();
    let mut found = false;

    for line in &mut lines {
        let JsonlLine::Issue(obj) = line else {
            continue;
        };
        if obj.get("id").and_then(Value::as_str) != Some(issue_id) {
            continue;
        }
        found = true;
        obj.insert("status".into(), Value::String("open".into()));
        obj.remove("closed_at");
        obj.insert("updated_at".into(), Value::String(now.clone()));
        break;
    }

    if !found {
        return Err(AppError::InvalidOperation(format!(
            "issue not found: {issue_id}"
        )));
    }

    save_jsonl_lines(repo_root, &lines, Some(issue_id))
}

pub fn mutate_dependency_native(
    repo_root: &Path,
    blocked_id: &str,
    blocker_id: &str,
    add: bool,
) -> AppResult<()> {
    if blocked_id == blocker_id {
        return Err(AppError::InvalidOperation(
            "an issue cannot depend on itself".into(),
        ));
    }

    let mut lines = load_jsonl_lines(repo_root)?;
    let existing = collect_existing_ids(&lines);
    if !existing.contains(blocked_id) {
        return Err(AppError::InvalidOperation(format!(
            "issue not found: {blocked_id}"
        )));
    }
    if add && !existing.contains(blocker_id) {
        return Err(AppError::InvalidOperation(format!(
            "blocker issue not found: {blocker_id}"
        )));
    }

    if add && would_create_cycle(&lines, blocked_id, blocker_id) {
        return Err(AppError::InvalidOperation(format!(
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
        obj.insert("updated_at".into(), Value::String(now.clone()));
        break;
    }

    recompute_dep_counts(&mut lines);
    save_jsonl_lines(repo_root, &lines, Some(blocked_id))
}

pub fn add_comment_native(repo_root: &Path, issue_id: &str, text: &str) -> AppResult<()> {
    let clean = text.trim();
    if clean.is_empty() {
        return Err(AppError::InvalidOperation(
            "comment text cannot be empty".into(),
        ));
    }

    let mut lines = load_jsonl_lines(repo_root)?;
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
        return Err(AppError::InvalidOperation(format!(
            "issue not found: {issue_id}"
        )));
    }

    save_jsonl_lines(repo_root, &lines, Some(issue_id))
}

enum JsonlLine {
    Issue(Map<String, Value>),
    Raw(String),
}

fn load_jsonl_lines(repo_root: &Path) -> AppResult<Vec<JsonlLine>> {
    let jsonl_path = repo_root.join(".beads").join("issues.jsonl");
    if !jsonl_path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&jsonl_path)
        .map_err(|e| AppError::Internal(format!("failed to read issues.jsonl: {e}")))?;

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
    repo_root: &Path,
    lines: &[JsonlLine],
    touched_id: Option<&str>,
) -> AppResult<()> {
    let beads_dir = repo_root.join(".beads");
    fs::create_dir_all(&beads_dir)
        .map_err(|e| AppError::Internal(format!("failed to create .beads dir: {e}")))?;

    let mut out = String::new();
    for line in lines {
        match line {
            JsonlLine::Issue(obj) => {
                let serialized = serde_json::to_string(obj).map_err(|e| {
                    AppError::Internal(format!("failed to serialize issue record: {e}"))
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

    let jsonl_path = beads_dir.join("issues.jsonl");
    atomic_write_bytes(&jsonl_path, out.as_bytes())?;

    if let Some(id) = touched_id {
        let _ = fs::write(beads_dir.join("last-touched"), format!("{id}\n"));
    }

    Ok(())
}

fn atomic_write_bytes(target: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::Internal("missing parent directory".into()))?;
    fs::create_dir_all(parent)
        .map_err(|e| AppError::Internal(format!("failed to create directory: {e}")))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".{}.{}.{}.tmp",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("beadline"),
        std::process::id(),
        stamp
    );
    let tmp_path = parent.join(tmp_name);
    fs::write(&tmp_path, bytes)
        .map_err(|e| AppError::Internal(format!("failed to write temp file: {e}")))?;

    if let Err(rename_err) = fs::rename(&tmp_path, target) {
        #[cfg(windows)]
        {
            if target.exists() {
                let _ = fs::remove_file(target);
                if fs::rename(&tmp_path, target).is_ok() {
                    return Ok(());
                }
            }
        }
        let _ = fs::remove_file(&tmp_path);
        return Err(AppError::Internal(format!(
            "failed to commit file {}: {rename_err}",
            target.display()
        )));
    }
    Ok(())
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
    // Edge direction: `issue` depends on `depends_on_id`.
    // Adding `blocked_id -> blocker_id` creates a cycle iff `blocked_id` is already
    // reachable from `blocker_id` following `blocks` dependencies.
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

    let slug = sanitize_prefix(prefix);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    for salt in 0_u64..1024 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for b in title.as_bytes() {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for b in nanos.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for b in salt.to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let code = encode_base36(hash, if salt < 64 { 3 } else { 4 });
        let candidate = format!("{slug}-{code}");
        if !existing_ids.contains(&candidate) {
            return candidate;
        }
    }
    format!("{slug}-{}", existing_ids.len() + 1)
}

fn encode_base36(mut value: u64, len: usize) -> String {
    const CHARS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = vec![b'0'; len];
    for slot in buf.iter_mut().rev() {
        *slot = CHARS[(value % 36) as usize];
        value /= 36;
    }
    String::from_utf8(buf).unwrap_or_else(|_| "000".to_string())
}

fn current_actor() -> String {
    std::env::var("BEADS_ACTOR")
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| std::env::var("USER"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "beadline".to_string())
}

fn dedup_strings(iter: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in iter {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

fn ensure_git_info_exclude(repo_root: &Path) {
    let Some(git_dir) = resolve_git_dir(repo_root) else {
        return;
    };
    let info_dir = git_dir.join("info");
    if fs::create_dir_all(&info_dir).is_err() {
        return;
    }
    let exclude_path = info_dir.join("exclude");
    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| matches!(line.trim(), ".beads" | ".beads/" | "/.beads/"))
    {
        return;
    }
    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str("# Beadline local Beads directory (non-invasive)\n.beads/\n");
    let _ = fs::write(exclude_path, next);
}

fn resolve_git_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    if dot_git.is_file() {
        let content = fs::read_to_string(&dot_git).ok()?;
        let rest = content.trim().strip_prefix("gitdir:")?.trim();
        let p = PathBuf::from(rest);
        return Some(if p.is_absolute() {
            p
        } else {
            repo_root.join(p)
        });
    }
    None
}

/// Formats current UTC time as RFC3339 (`YYYY-MM-DDTHH:MM:SSZ`) without extra crates.
pub fn now_rfc3339_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_rfc3339(secs)
}

fn format_unix_rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    // Howard Hinnant's civil-from-days algorithm
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + i64::from(m <= 2);

    format!("{year:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::jsonl_reader::read_workspace_snapshot;

    #[test]
    fn formats_known_unix_timestamps_as_rfc3339() {
        assert_eq!(format_unix_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_rfc3339(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn native_engine_full_crud_cycle_detection_and_lossless_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/info")).expect("git info");

        init_native_workspace(root, "app").expect("init native");
        assert_eq!(detect_storage_backend(root), StorageBackend::NativeJsonl);
        let exclude = std::fs::read_to_string(root.join(".git/info/exclude")).expect("exclude");
        assert!(exclude.contains(".beads/"));

        // 1. Create epic
        let epic_id = create_issue_native(
            root,
            &CreateIssueRequest {
                workspace_path: root.to_string_lossy().to_string(),
                title: "Ship native Beads engine".into(),
                issue_type: "epic".into(),
                priority: 0,
                status: "in_progress".into(),
                assignee: Some("alice".into()),
                parent_id: None,
                labels: vec!["mod:engine".into()],
                description: Some("Zero external binary dependency".into()),
                acceptance_criteria: Some("CRUD passes".into()),
                notes: None,
                blocked_by: vec![],
            },
            "app",
        )
        .expect("create epic");
        assert!(epic_id.starts_with("app-"));

        // 2. Create two child tasks (should sequence .1 and .2)
        let t1 = create_issue_native(
            root,
            &CreateIssueRequest {
                workspace_path: root.to_string_lossy().to_string(),
                title: "Implement writer".into(),
                issue_type: "task".into(),
                priority: 1,
                status: "open".into(),
                assignee: None,
                parent_id: Some(epic_id.clone()),
                labels: vec!["mod:engine".into()],
                description: None,
                acceptance_criteria: None,
                notes: None,
                blocked_by: vec![],
            },
            "app",
        )
        .expect("create t1");
        assert_eq!(t1, format!("{epic_id}.1"));

        let t2 = create_issue_native(
            root,
            &CreateIssueRequest {
                workspace_path: root.to_string_lossy().to_string(),
                title: "Add tests".into(),
                issue_type: "task".into(),
                priority: 1,
                status: "open".into(),
                assignee: None,
                parent_id: Some(epic_id.clone()),
                labels: vec!["mod:engine".into()],
                description: None,
                acceptance_criteria: None,
                notes: None,
                blocked_by: vec![t1.clone()],
            },
            "app",
        )
        .expect("create t2");
        assert_eq!(t2, format!("{epic_id}.2"));

        // 3. Cycle detection rejects t1 blocked-by t2
        let cycle_err = mutate_dependency_native(root, &t1, &t2, true);
        assert!(cycle_err.is_err(), "dependency cycle must be rejected");

        // 4. Add comment & verify inline read
        add_comment_native(root, &t1, "Native comment works!").expect("add comment");
        let comments = read_inline_comments(root, &t1).expect("read comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "Native comment works!");

        // 5. Update, close, reopen & verify jsonl_reader parses everything cleanly
        update_issue_native(
            root,
            &UpdateIssueRequest {
                workspace_path: root.to_string_lossy().to_string(),
                issue_id: t1.clone(),
                title: Some("Implement lossless JSONL writer".into()),
                status: Some("in_progress".into()),
                priority: Some(0),
                assignee: Some("bob".into()),
                labels: Some(vec!["mod:engine".into(), "priority".into()]),
                description: Some("Atomic rename".into()),
                acceptance_criteria: None,
                notes: None,
            },
        )
        .expect("update t1");

        close_issue_native(root, &t1, Some("Verified")).expect("close t1");
        reopen_issue_native(root, &t1, None).expect("reopen t1");

        let snap = read_workspace_snapshot(
            root,
            root.to_string_lossy().to_string(),
            "mod:",
            None,
            &[],
            &[],
        )
        .expect("read snapshot");
        assert_eq!(snap.storage_backend, StorageBackend::NativeJsonl);
        assert_eq!(snap.stats.total, 3);
        assert_eq!(snap.edges.len(), 1);
        let read_t1 = snap.issues.iter().find(|i| i.id == t1).expect("t1");
        assert_eq!(read_t1.title, "Implement lossless JSONL writer");
        assert_eq!(read_t1.comment_count, 1);
        assert_eq!(read_t1.status, "open");
    }
}
