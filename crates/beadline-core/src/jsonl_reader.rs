use crate::error::{CoreError, CoreResult};
use crate::fs_util::fnv1a64;
use crate::locator::{RepoLocation, RepoLocator};
use crate::models::{
    BoardLane, BoardStats, DagEdge, IssueDto, IssueProof, ModuleCount, StorageBackend, StoreKind,
    WorkspaceSnapshot, WorktreeRef,
};
use crate::smart_meta::extract_smart_meta;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RawDep {
    #[serde(default)]
    depends_on_id: String,
    #[serde(rename = "type", default)]
    dep_type: String,
}

#[derive(Debug, Deserialize)]
struct RawIssue {
    #[serde(rename = "_type", default)]
    record_type: String,
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    acceptance_criteria: String,
    #[serde(default)]
    close_reason: String,
    #[serde(default = "default_open")]
    status: String,
    #[serde(default = "default_priority")]
    priority: u8,
    #[serde(default = "default_task")]
    issue_type: String,
    #[serde(default)]
    assignee: String,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    closed_at: String,
    #[serde(default)]
    external_ref: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    dependencies: Vec<RawDep>,
    #[serde(default)]
    comments: Vec<Value>,
    #[serde(default)]
    comment_count: usize,
    #[serde(default)]
    metadata: Option<Value>,
}

fn default_open() -> String {
    "open".to_string()
}

fn default_priority() -> u8 {
    2
}

fn default_task() -> String {
    "task".to_string()
}

#[derive(Debug, Default)]
struct BeadlineExt {
    rev: u64,
    scope: Vec<String>,
    proof: Option<IssueProof>,
}

fn parse_beadline_ext(metadata: &Option<Value>) -> BeadlineExt {
    let mut ext = BeadlineExt {
        rev: 1,
        ..BeadlineExt::default()
    };
    let Some(bl) = metadata
        .as_ref()
        .and_then(|m| m.get("beadline"))
        .and_then(Value::as_object)
    else {
        return ext;
    };

    if let Some(rev) = bl.get("rev").and_then(Value::as_u64) {
        ext.rev = rev.max(1);
    }
    if let Some(arr) = bl.get("scope").and_then(Value::as_array) {
        ext.scope = arr
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
    }
    if let Some(proof_val) = bl.get("proof") {
        if let Ok(proof) = serde_json::from_value::<IssueProof>(proof_val.clone()) {
            if !proof.summary.trim().is_empty() || !proof.refs.is_empty() {
                ext.proof = Some(proof);
            }
        }
    }
    ext
}

pub fn read_beads_config_values(store_dir: &Path) -> (Option<String>, Option<String>) {
    let cfg_path = store_dir.join("config.yaml");
    let Ok(text) = fs::read_to_string(cfg_path) else {
        return (None, None);
    };

    let mut prefix = None;
    let mut gh_repo = None;
    let mut in_github = false;

    for raw_line in text.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            in_github = trimmed == "github:";
            if let Some(val) = trimmed
                .strip_prefix("issue-prefix:")
                .or_else(|| trimmed.strip_prefix("prefix:"))
            {
                let clean = val.trim().trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    prefix = Some(clean.to_string());
                }
            } else if let Some(val) = trimmed.strip_prefix("github.repository:") {
                let clean = val.trim().trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    gh_repo = Some(clean.to_string());
                }
            }
        } else if in_github {
            if let Some(val) = trimmed.strip_prefix("repository:") {
                let clean = val.trim().trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    gh_repo = Some(clean.to_string());
                }
            }
        }
    }

    (prefix, gh_repo)
}

/// Content-based Dolt state token derived from the actual bytes of `manifest` / `HEAD`
/// files inside `.beads/embeddeddolt` or `.beads/dolt`.
pub fn dolt_state_token(store_dir: &Path) -> String {
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    for sub in ["embeddeddolt", "dolt"] {
        let dir = store_dir.join(sub);
        if dir.exists() {
            collect_dolt_markers(&dir, store_dir, 0, &mut parts);
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash: u64 = 0xcbf29ce484222325;
    for (rel, bytes) in parts {
        for &b in rel.as_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
        for &b in &bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn collect_dolt_markers(
    dir: &Path,
    beads_root: &Path,
    depth: usize,
    out: &mut Vec<(String, Vec<u8>)>,
) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !name.eq_ignore_ascii_case("tempfiles")
                && !name.eq_ignore_ascii_case("temptf")
                && !name.eq_ignore_ascii_case("stats")
            {
                collect_dolt_markers(&path, beads_root, depth + 1, out);
            }
        } else if name == "manifest" || name == "HEAD" {
            if let Ok(bytes) = fs::read(&path) {
                let rel = path
                    .strip_prefix(beads_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, bytes));
            }
        }
    }
}

pub fn latest_dolt_mtime_ms(store_dir: &Path) -> u128 {
    let mut latest = 0_u128;
    for sub in ["embeddeddolt", "dolt"] {
        let dir = store_dir.join(sub);
        if dir.exists() {
            scan_dolt_mtimes(&dir, 0, &mut latest);
        }
    }
    latest
}

fn scan_dolt_mtimes(dir: &Path, depth: usize, latest: &mut u128) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if !name.eq_ignore_ascii_case("tempfiles")
                && !name.eq_ignore_ascii_case("temptf")
                && !name.eq_ignore_ascii_case("stats")
            {
                scan_dolt_mtimes(&path, depth + 1, latest);
            }
        } else if name == "manifest" || name == "HEAD" {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                        *latest = (*latest).max(dur.as_millis());
                    }
                }
            }
        }
    }
}

pub fn file_mtime_ms(path: &Path) -> u128 {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn compute_fingerprint(store_dir: &Path, module_prefix: &str) -> String {
    if !store_dir.exists() {
        return "missing".to_string();
    }
    let jsonl = store_dir.join("issues.jsonl");
    let (jsonl_len, jsonl_hash) = match fs::read(&jsonl) {
        Ok(bytes) => (bytes.len() as u64, fnv1a64(&bytes)),
        Err(_) => (0, 0),
    };
    let dolt_token = dolt_state_token(store_dir);
    format!("{jsonl_len}:{jsonl_hash:016x}:{dolt_token}:{module_prefix}")
}

pub struct RawIssueRecord(RawIssue);

/// Lenient UI-oriented JSONL parser (skips malformed lines so a single broken line never
/// bricks the whole board).
pub fn parse_jsonl_str(raw: &str) -> Vec<RawIssueRecord> {
    let mut records = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(issue) = serde_json::from_str::<RawIssue>(trimmed) {
            if issue.record_type.is_empty() || issue.record_type == "issue" {
                records.push(RawIssueRecord(issue));
            }
        }
    }
    records
}

#[allow(clippy::too_many_arguments)]
pub fn build_snapshot_from_records(
    repo_id: String,
    repo_name: String,
    repo_path: String,
    store_path: String,
    store_kind: StoreKind,
    storage_backend: StorageBackend,
    prefix: Option<String>,
    github_repo: Option<String>,
    module_prefix: &str,
    fingerprint: String,
    raw_records: Vec<RawIssueRecord>,
    repo_worktrees: &[WorktreeRef],
    allowed_schemes: &[String],
) -> WorkspaceSnapshot {
    let items: Vec<RawIssue> = raw_records.into_iter().map(|r| r.0).collect();

    let status_map: HashMap<String, String> = items
        .iter()
        .map(|r| (r.id.clone(), r.status.clone()))
        .collect();
    let title_map: HashMap<String, String> = items
        .iter()
        .map(|r| (r.id.clone(), r.title.clone()))
        .collect();
    let short_map: HashMap<String, String> = items
        .iter()
        .map(|r| (r.id.clone(), make_short_id(&r.id, &r.external_ref)))
        .collect();

    let mut parent_of: HashMap<String, String> = HashMap::new();
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new();
    let mut blocked_by_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut blocks_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut edge_set: HashSet<(String, String)> = HashSet::new();
    let mut edges: Vec<DagEdge> = Vec::new();

    for r in &items {
        for dep in &r.dependencies {
            if dep.depends_on_id.is_empty() || !status_map.contains_key(&dep.depends_on_id) {
                continue;
            }
            if dep.dep_type == "parent-child" {
                parent_of.insert(r.id.clone(), dep.depends_on_id.clone());
                children_of
                    .entry(dep.depends_on_id.clone())
                    .or_default()
                    .push(r.id.clone());
            } else if dep.dep_type == "blocks" {
                blocked_by_map
                    .entry(r.id.clone())
                    .or_default()
                    .push(dep.depends_on_id.clone());
                blocks_map
                    .entry(dep.depends_on_id.clone())
                    .or_default()
                    .push(r.id.clone());
                if edge_set.insert((dep.depends_on_id.clone(), r.id.clone())) {
                    edges.push(DagEdge {
                        from: dep.depends_on_id.clone(),
                        to: r.id.clone(),
                    });
                }
            }
        }
    }

    let mut mod_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut stats = BoardStats::default();
    let mut issues: Vec<IssueDto> = Vec::with_capacity(items.len());

    let effective_prefix = if module_prefix.is_empty() {
        "mod:"
    } else {
        module_prefix
    };

    for r in items {
        let blocked_by = dedup_vec(blocked_by_map.remove(&r.id).unwrap_or_default());
        let open_blocked_by: Vec<String> = blocked_by
            .iter()
            .filter(|bid| status_map.get(*bid).map(String::as_str) != Some("closed"))
            .cloned()
            .collect();
        let blocks = dedup_vec(blocks_map.remove(&r.id).unwrap_or_default());
        let children_ids = dedup_vec(children_of.remove(&r.id).unwrap_or_default());

        let lane = derive_lane(&r.status, !open_blocked_by.is_empty());

        stats.total += 1;
        match lane {
            BoardLane::InProgress => stats.in_progress += 1,
            BoardLane::Ready => stats.ready += 1,
            BoardLane::Blocked => stats.blocked += 1,
            BoardLane::Closed => stats.closed += 1,
        }
        if r.issue_type == "epic" {
            stats.epics += 1;
        }

        let mut modules: Vec<String> = r
            .labels
            .iter()
            .filter(|l| l.starts_with(effective_prefix))
            .cloned()
            .collect();
        modules.sort();
        modules.dedup();
        for m in &modules {
            *mod_counts.entry(m.clone()).or_insert(0) += 1;
        }

        let parent_id = parent_of.get(&r.id).cloned();
        let parent_short_id = parent_id
            .as_ref()
            .and_then(|pid| short_map.get(pid).cloned());
        let parent_title = parent_id
            .as_ref()
            .and_then(|pid| title_map.get(pid).cloned());

        let short_id = short_map
            .get(&r.id)
            .cloned()
            .unwrap_or_else(|| r.id.clone());

        let ext = parse_beadline_ext(&r.metadata);

        let smart = extract_smart_meta(
            &r.external_ref,
            &r.description,
            &r.notes,
            &r.close_reason,
            github_repo.as_deref(),
            repo_worktrees,
            allowed_schemes,
        );

        issues.push(IssueDto {
            id: r.id,
            short_id,
            title: r.title,
            description: r.description,
            notes: r.notes,
            acceptance_criteria: r.acceptance_criteria,
            close_reason: r.close_reason,
            scope: ext.scope,
            proof: ext.proof,
            rev: ext.rev,
            status: r.status,
            lane,
            priority: r.priority,
            issue_type: r.issue_type,
            assignee: r.assignee,
            owner: r.owner,
            created_at: r.created_at,
            updated_at: r.updated_at,
            closed_at: r.closed_at,
            external_ref: r.external_ref,
            labels: r.labels,
            modules,
            parent_id,
            parent_short_id,
            parent_title,
            children_ids,
            blocked_by,
            open_blocked_by,
            blocks,
            comment_count: r.comment_count.max(r.comments.len()),
            smart,
        });
    }

    // Deterministic sort: priority asc, updated_at desc, id asc
    issues.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.id.cmp(&b.id))
    });

    let epics: Vec<IssueDto> = issues
        .iter()
        .filter(|i| i.issue_type == "epic")
        .cloned()
        .collect();

    let mut modules: Vec<ModuleCount> = mod_counts
        .into_iter()
        .map(|(key, count)| ModuleCount { key, count })
        .collect();
    modules.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));

    WorkspaceSnapshot {
        repo_id,
        repo_name,
        repo_path,
        store_path,
        store_kind,
        has_beads: true,
        storage_backend,
        prefix,
        github_repo,
        module_prefix: effective_prefix.to_string(),
        fingerprint,
        stats,
        modules,
        issues,
        epics,
        edges,
    }
}

pub fn read_workspace_snapshot_for_location(
    loc: &RepoLocation,
    repo_path_display: String,
    module_prefix: &str,
    github_repo: Option<String>,
    repo_worktrees: &[WorktreeRef],
    allowed_schemes: &[String],
) -> CoreResult<WorkspaceSnapshot> {
    let store_path = loc.store_dir.to_string_lossy().to_string();
    if !loc.has_store {
        return Ok(WorkspaceSnapshot {
            repo_id: loc.repo_id.clone(),
            repo_name: loc.repo_name.clone(),
            repo_path: repo_path_display,
            store_path,
            store_kind: loc.store_kind,
            has_beads: false,
            storage_backend: loc.storage_backend,
            prefix: None,
            github_repo,
            module_prefix: module_prefix.to_string(),
            fingerprint: "uninitialized".to_string(),
            stats: BoardStats::default(),
            modules: Vec::new(),
            issues: Vec::new(),
            epics: Vec::new(),
            edges: Vec::new(),
        });
    }

    let (prefix, cfg_gh) = read_beads_config_values(&loc.store_dir);
    let effective_gh = github_repo.or(cfg_gh);
    let fingerprint = compute_fingerprint(&loc.store_dir, module_prefix);
    let jsonl_path = loc.issues_jsonl_path();

    let raw = if jsonl_path.exists() {
        fs::read_to_string(&jsonl_path)
            .map_err(|e| CoreError::Io(format!("failed to read issues.jsonl: {e}")))?
    } else {
        String::new()
    };

    let records = parse_jsonl_str(&raw);

    Ok(build_snapshot_from_records(
        loc.repo_id.clone(),
        loc.repo_name.clone(),
        repo_path_display,
        store_path,
        loc.store_kind,
        loc.storage_backend,
        prefix,
        effective_gh,
        module_prefix,
        fingerprint,
        records,
        repo_worktrees,
        allowed_schemes,
    ))
}

pub fn read_workspace_snapshot(
    repo_root: &Path,
    repo_path_display: String,
    module_prefix: &str,
    github_repo: Option<String>,
    repo_worktrees: &[WorktreeRef],
    allowed_schemes: &[String],
) -> CoreResult<WorkspaceSnapshot> {
    let loc = RepoLocator::locate(repo_root)?;
    read_workspace_snapshot_for_location(
        &loc,
        repo_path_display,
        module_prefix,
        github_repo,
        repo_worktrees,
        allowed_schemes,
    )
}

pub fn derive_lane(status: &str, has_open_blockers: bool) -> BoardLane {
    match status {
        "closed" => BoardLane::Closed,
        "in_progress" => BoardLane::InProgress,
        "blocked" => BoardLane::Blocked,
        _ => {
            if has_open_blockers {
                BoardLane::Blocked
            } else {
                BoardLane::Ready
            }
        }
    }
}

pub fn make_short_id(id: &str, external_ref: &str) -> String {
    let ext = external_ref.trim();
    if let Some(num) = ext.strip_prefix("gh-").or_else(|| ext.strip_prefix("GH-")) {
        if num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
            return format!("GH-{num}");
        }
    }
    id.to_string()
}

fn dedup_vec(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        if seen.insert(it.clone()) {
            out.push(it);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_beadline_extensions_without_breaking_standard_fields() {
        let jsonl = r#"
{"_type":"issue","id":"bl-1","title":"Auth Epic","status":"in_progress","priority":0,"issue_type":"epic","external_ref":"gh-4","labels":["mod:auth"],"metadata":{"custom":"keep","beadline":{"schemaVersion":1,"rev":3,"scope":["src/auth/**"],"proof":{"summary":"all green","refs":["abc1234"],"submittedAt":"2026-11-26T00:00:00Z"}}}}
{"_type":"issue","id":"bl-1.1","title":"Ready task","status":"open","priority":1,"issue_type":"task","labels":["mod:auth"],"dependencies":[{"issue_id":"bl-1.1","depends_on_id":"bl-1","type":"parent-child"}]}
{"_type":"issue","id":"bl-1.2","title":"Blocked task","status":"open","priority":1,"issue_type":"task","dependencies":[{"issue_id":"bl-1.2","depends_on_id":"bl-1","type":"parent-child"},{"issue_id":"bl-1.2","depends_on_id":"bl-1.1","type":"blocks"}]}
"#;
        let records = parse_jsonl_str(jsonl);
        let snap = build_snapshot_from_records(
            "repo_1".into(),
            "beadline".into(),
            "Y:\\GitHub\\beadline".into(),
            "Y:\\GitHub\\beadline\\.git\\beadline".into(),
            StoreKind::SharedGitCommon,
            StorageBackend::NativeJsonl,
            Some("bl".into()),
            Some("example/beadline".into()),
            "mod:",
            "fp1".into(),
            records,
            &[],
            &["t3-thread".into()],
        );

        let epic = snap.issues.iter().find(|i| i.id == "bl-1").unwrap();
        assert_eq!(epic.rev, 3);
        assert_eq!(epic.scope, vec!["src/auth/**"]);
        assert_eq!(epic.proof.as_ref().unwrap().summary, "all green");
        assert_eq!(epic.short_id, "GH-4");

        let blocked = snap.issues.iter().find(|i| i.id == "bl-1.2").unwrap();
        assert_eq!(blocked.lane, BoardLane::Blocked);
    }

    #[test]
    fn fingerprint_ignores_identical_manifest_rewrite_and_detects_real_dolt_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let beads = dir.path().join(".beads");
        let noms = beads.join("embeddeddolt/bl/.dolt/noms");
        fs::create_dir_all(&noms).expect("mkdir");
        let manifest = noms.join("manifest");
        let jsonl = beads.join("issues.jsonl");

        let v1 = "5:__DOLT__:lock1:root_hash_aaa:0000:vvvv:100";
        fs::write(&manifest, v1).expect("write manifest");
        fs::write(&jsonl, "{\"id\":\"bl-1\",\"title\":\"A\"}\n").expect("write jsonl");

        let fp1 = compute_fingerprint(&beads, "mod:");
        fs::write(&manifest, v1).expect("rewrite identical manifest");
        let fp2 = compute_fingerprint(&beads, "mod:");
        assert_eq!(fp1, fp2);

        let v2 = "5:__DOLT__:lock2:root_hash_bbb:0000:vvvv:101";
        fs::write(&manifest, v2).expect("write updated manifest");
        let fp3 = compute_fingerprint(&beads, "mod:");
        assert_ne!(fp1, fp3);
    }
}
