use beadline_core::error::{CoreError, CoreResult};
use beadline_core::git_radar::compute_radar_snapshot;
use beadline_core::jsonl_reader::read_workspace_snapshot_for_location;
use beadline_core::locator::{RepoLocation, RepoLocator};
use beadline_core::models::{BoardLane, IssueDto, SessionState};
use beadline_core::native_store::{finish_issue_in, update_issue_in};
use beadline_core::runtime_store::{
    generate_session_id, list_leases, release_session_lease, upsert_claim_lease,
    upsert_pulse_lease, ClaimLeaseInput, PulseLeaseInput,
};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

const SERVER_NAME: &str = "beadline";
const PROTOCOL_VERSION: &str = "2025-03-26";

pub struct McpServer {
    repo_root: PathBuf,
    harness: String,
    agent: String,
    session_id: String,
    current_issue_id: Option<String>,
}

impl McpServer {
    pub fn new(
        repo_root: PathBuf,
        harness: Option<String>,
        agent: Option<String>,
        session_id: Option<String>,
    ) -> CoreResult<Self> {
        let loc = RepoLocator::locate(&repo_root)?;
        Ok(Self {
            repo_root: loc.worktree_root,
            harness: harness.unwrap_or_else(detect_harness_from_env),
            agent: agent.unwrap_or_else(detect_agent_from_env),
            session_id: session_id
                .filter(|s| !s.trim().is_empty())
                .or_else(|| std::env::var("BL_SESSION_ID").ok())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(generate_session_id),
            current_issue_id: None,
        })
    }

    pub fn serve_stdio<R: BufRead, W: Write>(&mut self, reader: R, mut writer: W) -> CoreResult<()> {
        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("[beadline-mcp] stdin read error: {e}");
                    break;
                }
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let req: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(e) => {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": { "code": -32700, "message": format!("Parse error: {e}") }
                    });
                    write_json_line(&mut writer, &err_resp)?;
                    continue;
                }
            };

            if let Some(resp) = self.handle_jsonrpc(&req) {
                write_json_line(&mut writer, &resp)?;
            }
        }
        Ok(())
    }

    pub fn handle_jsonrpc(&mut self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned();
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        // Notifications have no `id` and expect no reply
        if id.is_none() || method.starts_with("notifications/") {
            return None;
        }
        let id = id.unwrap_or(Value::Null);

        let result = match method {
            "initialize" => {
                let client_proto = params
                    .get("protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION);
                Ok(json!({
                    "protocolVersion": client_proto,
                    "capabilities": {
                        "tools": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": "Use bl_ready to find unblocked tasks, bl_show to inspect acceptance criteria & scope, bl_claim when starting work, bl_pulse when changing steps or waiting on the human (with waitingReason), and bl_finish with structured proof when done."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                Ok(self.dispatch_tool(name, &args))
            }
            _ => Err((-32601, format!("Method not found: {method}"))),
        };

        match result {
            Ok(val) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": val
            })),
            Err((code, message)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message }
            })),
        }
    }

    fn dispatch_tool(&mut self, name: &str, args: &Value) -> Value {
        let res = match name {
            "bl_ready" => self.tool_ready(args),
            "bl_show" => self.tool_show(args),
            "bl_claim" => self.tool_claim(args),
            "bl_pulse" => self.tool_pulse(args),
            "bl_finish" => self.tool_finish(args),
            _ => Err(CoreError::InvalidOperation(format!("unknown tool: {name}"))),
        };

        match res {
            Ok((text, structured)) => json!({
                "content": [{ "type": "text", "text": text }],
                "structuredContent": structured,
                "isError": false
            }),
            Err(err) => {
                let hint = error_hint(&err);
                let payload = json!({
                    "code": err.code(),
                    "message": err.to_string(),
                    "hint": hint
                });
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("[{}] {} — {}", err.code(), err, hint)
                    }],
                    "structuredContent": payload,
                    "isError": true
                })
            }
        }
    }

    fn locate(&self) -> CoreResult<RepoLocation> {
        RepoLocator::locate(&self.repo_root)
    }

    fn tool_ready(&self, args: &Value) -> CoreResult<(String, Value)> {
        let loc = self.locate()?;
        if !loc.has_store {
            return Err(CoreError::NotInitialized);
        }
        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(10)
            .clamp(1, 50) as usize;
        let module_filter = args
            .get("module")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let snap = read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )?;

        let ready_all: Vec<&IssueDto> = snap
            .issues
            .iter()
            .filter(|i| i.lane == BoardLane::Ready && i.issue_type != "epic")
            .filter(|i| {
                if let Some(m) = module_filter {
                    i.modules.iter().any(|x| x == m || x.ends_with(&format!(":{m}")))
                } else {
                    true
                }
            })
            .collect();

        let has_more = ready_all.len() > limit;
        let items: Vec<Value> = ready_all
            .iter()
            .take(limit)
            .map(|i| {
                json!({
                    "id": i.id,
                    "shortId": i.short_id,
                    "title": i.title,
                    "priority": i.priority,
                    "scopeSummary": i.scope,
                    "rev": i.rev
                })
            })
            .collect();

        let text = if items.is_empty() {
            "No ready tasks found.".to_string()
        } else {
            let lines: Vec<String> = ready_all
                .iter()
                .take(limit)
                .map(|i| format!("- {} [P{}] {}", i.id, i.priority, i.title))
                .collect();
            format!("Ready tasks ({}):\n{}", items.len(), lines.join("\n"))
        };

        Ok((text, json!({ "items": items, "hasMore": has_more })))
    }

    fn tool_show(&self, args: &Value) -> CoreResult<(String, Value)> {
        let issue_id = required_str(args, "issueId")?;
        let loc = self.locate()?;
        if !loc.has_store {
            return Err(CoreError::NotInitialized);
        }
        let snap = read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )?;
        let issue = snap
            .issues
            .iter()
            .find(|i| i.id == issue_id || i.short_id == issue_id)
            .ok_or_else(|| CoreError::NotFound(format!("issue not found: {issue_id}")))?;

        let leases = list_leases(&loc.runtime_dir).unwrap_or_default();
        let active_sessions: Vec<Value> = leases
            .iter()
            .filter(|l| !l.stale_heartbeat && l.issue_id.as_deref() == Some(issue.id.as_str()))
            .map(|l| {
                json!({
                    "sessionId": l.session_id,
                    "harness": l.harness,
                    "agent": l.agent,
                    "state": l.state,
                    "summary": l.summary,
                    "branch": l.branch
                })
            })
            .collect();

        let task_json = serialize_task_brief(issue, &active_sessions);
        let text = format!(
            "{} ({}, P{}, rev {}): {}\nScope: {}\nBlockers: {}",
            issue.id,
            issue.status,
            issue.priority,
            issue.rev,
            issue.title,
            if issue.scope.is_empty() {
                "unspecified".into()
            } else {
                issue.scope.join(", ")
            },
            if issue.open_blocked_by.is_empty() {
                "none".into()
            } else {
                issue.open_blocked_by.join(", ")
            }
        );

        Ok((text, task_json))
    }

    fn tool_claim(&mut self, args: &Value) -> CoreResult<(String, Value)> {
        let issue_id = required_str(args, "issueId")?;
        let loc = self.locate()?;
        if !loc.has_store {
            return Err(CoreError::NotInitialized);
        }

        let snap = read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )?;
        let issue = snap
            .issues
            .iter()
            .find(|i| i.id == issue_id || i.short_id == issue_id)
            .ok_or_else(|| CoreError::NotFound(format!("issue not found: {issue_id}")))?;
        let canonical_id = issue.id.clone();

        let scope = optional_string_list(args, "scope").unwrap_or_else(|| issue.scope.clone());
        let summary = args
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("starting work")
            .to_string();
        let thread_url = args
            .get("threadUrl")
            .and_then(Value::as_str)
            .map(String::from);

        let lease = upsert_claim_lease(
            &loc.runtime_dir,
            ClaimLeaseInput {
                session_id: Some(self.session_id.clone()),
                issue_id: canonical_id.clone(),
                worktree_id: loc.worktree_id.clone(),
                worktree_path: loc.worktree_display.clone(),
                branch: loc.branch.clone(),
                harness: self.harness.clone(),
                agent: self.agent.clone(),
                summary,
                scope: scope.clone(),
                thread_url,
                ttl_secs: None,
            },
        )?;
        self.current_issue_id = Some(canonical_id.clone());

        // Move open task into in_progress best-effort (never hard-fails claim on legacy Dolt)
        if issue.status == "open" && loc.storage_backend == beadline_core::models::StorageBackend::NativeJsonl {
            let _ = update_issue_in(
                &loc.store_dir,
                &beadline_core::models::UpdateIssueRequest {
                    workspace_path: loc.worktree_display.clone(),
                    issue_id: canonical_id.clone(),
                    title: None,
                    status: Some("in_progress".into()),
                    priority: None,
                    assignee: None,
                    labels: None,
                    description: None,
                    acceptance_criteria: None,
                    notes: None,
                    scope: if !scope.is_empty() && issue.scope.is_empty() {
                        Some(scope)
                    } else {
                        None
                    },
                    expected_rev: None,
                },
            );
        }

        // Re-read fresh snapshot + gather concurrency warnings (soft coordination, never hard lock)
        let fresh_snap = read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )?;
        let fresh_issue = fresh_snap
            .issues
            .iter()
            .find(|i| i.id == canonical_id)
            .unwrap_or(issue);

        let warnings = collect_session_warnings(&loc, &fresh_snap.issues, &lease.session_id, Some(&canonical_id));
        let active_sessions: Vec<Value> = list_leases(&loc.runtime_dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|l| !l.stale_heartbeat && l.issue_id.as_deref() == Some(canonical_id.as_str()))
            .map(|l| {
                json!({
                    "sessionId": l.session_id,
                    "harness": l.harness,
                    "agent": l.agent,
                    "state": l.state,
                    "summary": l.summary,
                    "branch": l.branch
                })
            })
            .collect();

        let payload = json!({
            "sessionId": lease.session_id,
            "task": serialize_task_brief(fresh_issue, &active_sessions),
            "worktreeId": lease.worktree_id,
            "worktreePath": lease.worktree_path,
            "branch": lease.branch,
            "expiresAt": lease.expires_at,
            "warnings": warnings
        });

        let text = format!(
            "Claimed {} (rev {}) as session {}. Expires at {}. Warnings: {}",
            fresh_issue.id,
            fresh_issue.rev,
            lease.session_id,
            lease.expires_at,
            if warnings.is_empty() {
                "none".into()
            } else {
                warnings.join("; ")
            }
        );

        Ok((text, payload))
    }

    fn tool_pulse(&mut self, args: &Value) -> CoreResult<(String, Value)> {
        let loc = self.locate()?;
        let state_str = required_str(args, "state")?;
        let state = parse_session_state(&state_str)?;

        let summary = args.get("summary").and_then(Value::as_str).map(String::from);
        let waiting_reason = args
            .get("waitingReason")
            .and_then(Value::as_str)
            .map(String::from);
        let issue_id = args
            .get("issueId")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| self.current_issue_id.clone());
        let scope = optional_string_list(args, "scope");
        let thread_url = args
            .get("threadUrl")
            .and_then(Value::as_str)
            .map(String::from);

        if state == SessionState::Released {
            let _ = release_session_lease(&loc.runtime_dir, &self.session_id)?;
            self.current_issue_id = None;
            let payload = json!({
                "sessionId": self.session_id,
                "state": "released",
                "expiresAt": Value::Null,
                "warnings": []
            });
            return Ok((format!("Released session {}.", self.session_id), payload));
        }

        let lease = upsert_pulse_lease(
            &loc.runtime_dir,
            PulseLeaseInput {
                session_id: self.session_id.clone(),
                issue_id: issue_id.clone(),
                worktree_id: loc.worktree_id.clone(),
                worktree_path: loc.worktree_display.clone(),
                branch: loc.branch.clone(),
                harness: self.harness.clone(),
                agent: self.agent.clone(),
                state,
                summary,
                waiting_reason,
                scope,
                thread_url,
                ttl_secs: None,
            },
        )?;
        self.current_issue_id = lease.issue_id.clone();

        let snap = read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )
        .ok();
        let warnings = match snap {
            Some(s) => collect_session_warnings(
                &loc,
                &s.issues,
                &lease.session_id,
                lease.issue_id.as_deref(),
            ),
            None => Vec::new(),
        };

        let payload = json!({
            "sessionId": lease.session_id,
            "state": lease.state,
            "expiresAt": lease.expires_at,
            "warnings": warnings
        });

        let text = format!(
            "Pulse recorded for {} (state: {:?}, expires: {}).",
            lease.session_id, lease.state, lease.expires_at
        );
        Ok((text, payload))
    }

    fn tool_finish(&mut self, args: &Value) -> CoreResult<(String, Value)> {
        let issue_id = required_str(args, "issueId")?;
        let loc = self.locate()?;
        if !loc.has_store {
            return Err(CoreError::NotInitialized);
        }

        let expected_rev = args.get("expectedRev").and_then(Value::as_u64);
        let operation_id = args.get("operationId").and_then(Value::as_str);

        let proof_obj = args
            .get("proof")
            .and_then(Value::as_object)
            .ok_or_else(|| CoreError::InvalidOperation("proof object with summary is required".into()))?;
        let proof_summary = proof_obj
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| CoreError::InvalidOperation("proof.summary cannot be empty".into()))?;
        let proof_refs: Vec<String> = proof_obj
            .get("refs")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        let res = finish_issue_in(
            &loc.store_dir,
            &issue_id,
            expected_rev,
            operation_id,
            proof_summary,
            &proof_refs,
            Some(&self.session_id),
        )?;

        // Best-effort release of this session's lease when finishing its active issue
        if self.current_issue_id.as_deref() == Some(issue_id.as_str()) {
            let _ = release_session_lease(&loc.runtime_dir, &self.session_id);
            self.current_issue_id = None;
        }

        let unblocked_text = if res.unblocked.is_empty() {
            "none".to_string()
        } else {
            res.unblocked
                .iter()
                .map(|u| format!("{} ({})", u.short_id, u.title))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let text = format!(
            "Finished {} (status: {}, rev: {}, replay: {}). Newly unblocked: {}",
            res.id, res.status, res.rev, res.idempotent_replay, unblocked_text
        );
        let structured = serde_json::to_value(&res)
            .map_err(|e| CoreError::Internal(format!("serialize finish result: {e}")))?;
        Ok((text, structured))
    }
}

fn serialize_task_brief(issue: &IssueDto, active_sessions: &[Value]) -> Value {
    json!({
        "id": issue.id,
        "shortId": issue.short_id,
        "title": issue.title,
        "description": issue.description,
        "acceptanceCriteria": issue.acceptance_criteria,
        "notes": issue.notes,
        "scope": issue.scope,
        "blockers": issue.open_blocked_by,
        "blocks": issue.blocks,
        "status": issue.status,
        "lane": issue.lane,
        "priority": issue.priority,
        "rev": issue.rev,
        "proof": issue.proof,
        "activeSessions": active_sessions
    })
}

fn collect_session_warnings(
    loc: &RepoLocation,
    issues: &[IssueDto],
    my_session_id: &str,
    my_issue_id: Option<&str>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let (radar, _) = compute_radar_snapshot(loc, issues, 0, false, None);

    // Warn if another live session is already working on the exact same issue
    if let Some(iid) = my_issue_id {
        for other in &radar.sessions {
            if other.session_id != my_session_id
                && !other.stale_heartbeat
                && other.issue_id.as_deref() == Some(iid)
            {
                warnings.push(format!(
                    "Another active session ({}, {}) is already claiming {}",
                    other.harness,
                    other.branch.as_deref().unwrap_or("unknown branch"),
                    iid
                ));
            }
        }
    }

    for c in &radar.conflicts {
        if c.participants.iter().any(|p| p.id == my_session_id) {
            warnings.push(format!(
                "Overlap ({:?}): {}",
                c.kind,
                if !c.evidence.paths.is_empty() {
                    c.evidence.paths.join(", ")
                } else {
                    c.evidence.matched_scopes.join(", ")
                }
            ));
        }
    }

    warnings
}

pub fn parse_session_state(raw: &str) -> CoreResult<SessionState> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "working" => Ok(SessionState::Working),
        "waiting_human" | "waiting-human" => Ok(SessionState::WaitingHuman),
        "released" => Ok(SessionState::Released),
        other => Err(CoreError::InvalidOperation(format!(
            "invalid session state '{other}'; expected working | waiting_human | released"
        ))),
    }
}

fn required_str(args: &Value, key: &str) -> CoreResult<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| CoreError::InvalidOperation(format!("missing required field '{key}'")))
}

fn optional_string_list(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|arr| {
        arr.iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect()
    })
}

fn error_hint(err: &CoreError) -> &'static str {
    match err {
        CoreError::StaleRevision { .. } => {
            "Call bl_show to inspect the latest rev before retrying."
        }
        CoreError::Busy(_) => "Another writer holds the lock; retry after a short backoff.",
        CoreError::NotInitialized => "Initialize the repository first with `bl init`.",
        CoreError::NotFound(_) => "Call bl_ready to inspect valid issue IDs.",
        CoreError::InvalidOperation(_) => "Check required arguments and scope syntax (`path` or `dir/**`).",
        _ => "Run `bl doctor --json` to inspect repository health.",
    }
}

pub fn detect_harness_from_env() -> String {
    if std::env::var_os("CLAUDECODE").is_some() || std::env::var_os("CLAUDE_CODE").is_some() {
        "claude-code".into()
    } else if std::env::var_os("CODEX_THREAD_ID").is_some() || std::env::var_os("CODEX_CI").is_some() {
        "codex".into()
    } else if std::env::var_os("CURSOR_TRACE_ID").is_some() || std::env::var_os("CURSOR_AGENT").is_some() {
        "cursor".into()
    } else if std::env::var_os("ANTIGRAVITY_SESSION").is_some() {
        "antigravity".into()
    } else if std::env::var_os("T3_THREAD_ID").is_some() {
        "t3-code".into()
    } else {
        "mcp".into()
    }
}

pub fn detect_agent_from_env() -> String {
    std::env::var("BL_AGENT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(detect_harness_from_env)
}

fn write_json_line<W: Write>(writer: &mut W, val: &Value) -> CoreResult<()> {
    let serialized = serde_json::to_string(val)
        .map_err(|e| CoreError::Internal(format!("json serialize failed: {e}")))?;
    writeln!(writer, "{serialized}")
        .and_then(|()| writer.flush())
        .map_err(|e| CoreError::Io(format!("stdout write failed: {e}")))
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "bl_ready",
            "description": "List ready, unblocked tasks in this repository sorted by priority.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
                    "module": { "type": "string", "description": "Optional module filter (e.g. 'mod:auth' or 'auth')." }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "bl_show",
            "description": "Read full task context including why it exists (description), acceptance criteria, declared write scope, active blockers, current rev, and active agent sessions.",
            "inputSchema": {
                "type": "object",
                "required": ["issueId"],
                "properties": {
                    "issueId": { "type": "string" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "bl_claim",
            "description": "Register this agent session as working on a task. Returns full task details, lease expiry, and concurrency overlap warnings without hard-locking other agents.",
            "inputSchema": {
                "type": "object",
                "required": ["issueId"],
                "properties": {
                    "issueId": { "type": "string" },
                    "scope": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Repo-relative write scope patterns ('exact/file.rs' or 'dir/**')."
                    },
                    "summary": { "type": "string", "description": "Brief 1-line summary of what you are doing right now." },
                    "threadUrl": { "type": "string", "description": "Optional deep link back to the current agent thread." }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "bl_pulse",
            "description": "Update live session state, renew lease, or explicitly flag waiting_human (waitingReason is required for waiting_human).",
            "inputSchema": {
                "type": "object",
                "required": ["state"],
                "properties": {
                    "state": { "type": "string", "enum": ["working", "waiting_human", "released"] },
                    "summary": { "type": "string" },
                    "waitingReason": { "type": "string", "description": "Required when state is 'waiting_human'." },
                    "issueId": { "type": "string" },
                    "scope": { "type": "array", "items": { "type": "string" } },
                    "threadUrl": { "type": "string" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "bl_finish",
            "description": "Atomically close a task with structured delivery proof and return newly unblocked downstream tasks. Safe to retry with the same operationId.",
            "inputSchema": {
                "type": "object",
                "required": ["issueId", "proof"],
                "properties": {
                    "issueId": { "type": "string" },
                    "expectedRev": { "type": "integer", "minimum": 1 },
                    "operationId": { "type": "string", "description": "Idempotency key for safe retries." },
                    "proof": {
                        "type": "object",
                        "required": ["summary"],
                        "properties": {
                            "summary": { "type": "string", "description": "Concise verification/delivery summary." },
                            "refs": { "type": "array", "items": { "type": "string" }, "description": "Optional commits, PR URLs, or test commands." }
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use beadline_core::models::CreateIssueRequest;
    use beadline_core::native_store::{create_issue_native, init_native_workspace};
    use std::io::Cursor;

    #[test]
    fn mcp_stdio_end_to_end_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        init_native_workspace(dir.path(), "mcp").unwrap();
        let a = create_issue_native(
            dir.path(),
            &CreateIssueRequest {
                workspace_path: dir.path().to_string_lossy().into(),
                title: "Implement token refresh".into(),
                issue_type: "task".into(),
                priority: 1,
                status: "open".into(),
                assignee: None,
                parent_id: None,
                labels: vec!["mod:auth".into()],
                description: Some("Users get logged out after 15m".into()),
                acceptance_criteria: Some("Refresh flow passes tests".into()),
                notes: None,
                scope: vec!["src/auth/**".into()],
                blocked_by: vec![],
            },
            "mcp",
        )
        .unwrap();

        let requests = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bl_ready","arguments":{"limit":5}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bl_claim","arguments":{"issueId":a,"summary":"editing refresh handler"}}}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"bl_pulse","arguments":{"state":"waiting_human","waitingReason":"Confirm token rotation policy"}}}),
            json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"bl_finish","arguments":{"issueId":a,"operationId":"op-42","proof":{"summary":"cargo test green","refs":["commit:abc123"]}}}}),
        );

        let mut server = McpServer::new(
            dir.path().to_path_buf(),
            Some("codex".into()),
            Some("gpt".into()),
            Some("ses_test1".into()),
        )
        .unwrap();

        let mut out_buf = Vec::new();
        server
            .serve_stdio(Cursor::new(requests.as_bytes()), &mut out_buf)
            .unwrap();

        let output = String::from_utf8(out_buf).unwrap();
        let responses: Vec<Value> = output
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .collect();

        assert_eq!(responses.len(), 6);
        assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 5);
        assert_eq!(responses[2]["result"]["structuredContent"]["items"].as_array().unwrap().len(), 1);
        assert_eq!(responses[3]["result"]["isError"], false);
        assert_eq!(responses[3]["result"]["structuredContent"]["sessionId"], "ses_test1");
        assert_eq!(responses[4]["result"]["isError"], false);
        assert_eq!(responses[5]["result"]["isError"], false);
        assert_eq!(responses[5]["result"]["structuredContent"]["status"], "closed");
    }
}
