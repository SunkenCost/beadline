use crate::error::{CoreError, CoreResult};
use crate::fs_util::{atomic_write_bytes, fnv1a64, FileLockGuard};
use crate::models::{
    PendingHumanRequest, PendingRequestKind, SessionLease, SessionReachability, SessionState,
    TelemetrySource, TurnState,
};
use crate::scope::normalize_scope_list;
use crate::time::{format_unix_ms_rfc3339, now_rfc3339_utc, now_unix_ms, parse_rfc3339_to_unix_ms};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Active window before a running session without any hook/tool/pulse event transitions to `Idle`.
/// 15 minutes prevents false alarms during 3–10 minute test/build runs!
pub const ACTIVE_WINDOW_SECS: u64 = 15 * 60;
/// Window before an unconfirmed running session transitions to `Unconfirmed`.
/// NOTE: Never clears or hides `pending_request` (waiting_human)!
pub const UNCONFIRMED_WINDOW_SECS: u64 = 45 * 60;
/// Retention window before ended/unconfirmed sessions without pending requests are lazily pruned.
pub const RETENTION_WINDOW_SECS: u64 = 6 * 3600;
pub const KEEPALIVE_THROTTLE_SECS: u64 = 30;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct ClaimLeaseInput {
    pub session_id: Option<String>,
    pub issue_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub branch: Option<String>,
    pub harness: String,
    pub agent: String,
    pub summary: String,
    pub scope: Vec<String>,
    pub thread_url: Option<String>,
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PulseLeaseInput {
    pub session_id: String,
    pub issue_id: Option<String>,
    pub worktree_id: String,
    pub worktree_path: String,
    pub branch: Option<String>,
    pub harness: String,
    pub agent: String,
    pub state: SessionState,
    pub summary: Option<String>,
    pub waiting_reason: Option<String>,
    pub scope: Option<Vec<String>>,
    pub thread_url: Option<String>,
    pub ttl_secs: Option<u64>,
}

/// Normalized deterministic lifecycle event emitted by a host harness hook
/// (Claude Code, Codex CLI, Cursor, Antigravity, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEventKind {
    SessionStart,
    PreToolUse,
    PostToolUse,
    PermissionRequest,
    NotificationWait,
    TurnStop,
    SessionEnd,
}

#[derive(Debug, Clone)]
pub struct HookEventInput {
    pub session_id: String,
    pub worktree_id: String,
    pub worktree_path: String,
    pub branch: Option<String>,
    pub harness: String,
    pub agent: String,
    pub event: HookEventKind,
    pub tool_name: Option<String>,
    pub request_id: Option<String>,
    pub summary: Option<String>,
    pub waiting_reason: Option<String>,
}

pub fn generate_session_id() -> String {
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = format!("{}:{}:{seq}", std::process::id(), now_unix_ms());
    let h = fnv1a64(seed.as_bytes());
    format!("ses_{h:012x}")
}

pub fn sanitize_session_id(raw: &str) -> CoreResult<String> {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect();
    if clean.is_empty() {
        return Err(CoreError::InvalidOperation(
            "sessionId must contain alphanumeric, '-' or '_' characters".into(),
        ));
    }
    Ok(clean)
}

fn lease_file_path(runtime_dir: &Path, session_id: &str) -> PathBuf {
    runtime_dir.join(format!("{session_id}.json"))
}

fn lease_lock_path(runtime_dir: &Path, session_id: &str) -> PathBuf {
    runtime_dir.join(format!(".{session_id}.lock"))
}

/// Applies a deterministic host lifecycle hook event (`bl hook <harness>`).
/// Automatically discovers & registers sessions even if the user never copied a Beadline handoff
/// prompt (`issue_id: None` = unlinked session), tracks persistent approval/question requests by
/// host request ID, and clears them only on resolution/end.
pub fn apply_hook_event(
    runtime_dir: &Path,
    input: HookEventInput,
) -> CoreResult<Option<SessionLease>> {
    let sid = sanitize_session_id(&input.session_id)?;

    if input.event == HookEventKind::SessionEnd {
        let _ = release_session_lease(runtime_dir, &sid)?;
        return Ok(None);
    }

    fs::create_dir_all(runtime_dir)
        .map_err(|e| CoreError::Io(format!("failed to create runtime dir: {e}")))?;

    let _lock = FileLockGuard::acquire(&lease_lock_path(runtime_dir, &sid), Duration::from_secs(2))?;
    let path = lease_file_path(runtime_dir, &sid);
    let existing = read_lease_file(&path);

    let now_ms = now_unix_ms();
    let now_iso = format_unix_ms_rfc3339(now_ms);
    let expires_iso = format_unix_ms_rfc3339(now_ms + ACTIVE_WINDOW_SECS * 1000);

    let mut lease = existing.unwrap_or_else(|| SessionLease {
        session_id: sid.clone(),
        issue_id: None,
        worktree_id: input.worktree_id.clone(),
        worktree_path: input.worktree_path.clone(),
        branch: input.branch.clone(),
        harness: non_empty_or(input.harness.clone(), "hook"),
        agent: non_empty_or(input.agent.clone(), "agent"),
        source: TelemetrySource::Hook,
        state: SessionState::Working,
        turn_state: TurnState::Running,
        summary: String::new(),
        waiting_reason: None,
        pending_request: None,
        last_tool: None,
        scope: Vec::new(),
        thread_url: None,
        seq: 0,
        started_at: now_iso.clone(),
        last_seen_at: now_iso.clone(),
        last_activity_at: now_iso.clone(),
        last_progress_at: now_iso.clone(),
        expires_at: expires_iso.clone(),
        reachability: SessionReachability::Active,
        stale_heartbeat: false,
    });

    if !input.worktree_id.is_empty() {
        lease.worktree_id = input.worktree_id;
    }
    if !input.worktree_path.is_empty() {
        lease.worktree_path = input.worktree_path;
    }
    if input.branch.is_some() {
        lease.branch = input.branch;
    }
    if !input.harness.trim().is_empty() {
        lease.harness = input.harness.trim().to_string();
    }
    if !input.agent.trim().is_empty() {
        lease.agent = input.agent.trim().to_string();
    }
    lease.source = TelemetrySource::Hook;
    lease.seq += 1;
    lease.last_seen_at = now_iso.clone();
    lease.last_activity_at = now_iso.clone();
    lease.expires_at = expires_iso;
    lease.reachability = SessionReachability::Active;
    lease.stale_heartbeat = false;

    match input.event {
        HookEventKind::SessionStart => {
            lease.state = SessionState::Working;
            lease.turn_state = TurnState::Running;
            if lease.summary.is_empty() {
                lease.summary = input
                    .summary
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "session active".to_string());
            }
            lease.last_progress_at = now_iso;
        }
        HookEventKind::PreToolUse => {
            let tool = input.tool_name.clone();
            let is_question_tool = tool.as_deref().is_some_and(|t| {
                let lower = t.to_ascii_lowercase();
                lower.contains("ask_question") || lower.contains("askuserquestion")
            });

            if is_question_tool {
                let reason = input
                    .waiting_reason
                    .or(input.summary)
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "Agent asked a question and is awaiting your answer".into());
                let req_id = input
                    .request_id
                    .unwrap_or_else(|| format!("req_{:012x}", fnv1a64(reason.as_bytes())));
                lease.state = SessionState::WaitingHuman;
                lease.turn_state = TurnState::WaitingHuman;
                lease.waiting_reason = Some(reason.clone());
                lease.pending_request = Some(PendingHumanRequest {
                    request_id: req_id,
                    kind: PendingRequestKind::Question,
                    reason,
                    created_at: now_iso.clone(),
                    source: TelemetrySource::Hook,
                    tool_name: tool.clone(),
                });
            } else {
                // Tool execution actually started -> any previous approval prompt was resolved
                if lease
                    .pending_request
                    .as_ref()
                    .is_some_and(|r| r.kind == PendingRequestKind::Approval)
                {
                    lease.pending_request = None;
                    lease.waiting_reason = None;
                }
                lease.state = SessionState::Working;
                lease.turn_state = TurnState::Running;
                if let Some(s) = input.summary.filter(|s| !s.trim().is_empty()) {
                    lease.summary = s;
                } else if let Some(t) = &tool {
                    lease.summary = format!("Running {t}");
                }
            }
            lease.last_tool = tool;
            lease.last_progress_at = now_iso;
        }
        HookEventKind::PostToolUse => {
            // Tool completed -> clear any matching approval or question request for this tool
            let clear_pending = match (&lease.pending_request, &input.request_id) {
                (Some(req), Some(rid)) => req.request_id == *rid || req.kind == PendingRequestKind::Approval,
                (Some(req), None) => {
                    req.kind == PendingRequestKind::Approval
                        || req.tool_name.is_some() && req.tool_name == input.tool_name
                }
                (None, _) => false,
            };
            if clear_pending {
                lease.pending_request = None;
                lease.waiting_reason = None;
            }
            if lease.pending_request.is_none() {
                lease.state = SessionState::Working;
                lease.turn_state = TurnState::Running;
            }
            if let Some(s) = input.summary.filter(|s| !s.trim().is_empty()) {
                lease.summary = s;
            } else if let Some(t) = &input.tool_name {
                lease.summary = format!("Completed {t}");
            }
            if input.tool_name.is_some() {
                lease.last_tool = input.tool_name;
            }
            lease.last_progress_at = now_iso;
        }
        HookEventKind::PermissionRequest => {
            let tool = input.tool_name.clone();
            let reason = input
                .waiting_reason
                .or(input.summary)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| match &tool {
                    Some(t) => format!("Approval requested for {t}"),
                    None => "Approval requested".into(),
                });
            let req_id = input
                .request_id
                .unwrap_or_else(|| format!("perm_{:012x}", fnv1a64(reason.as_bytes())));
            lease.state = SessionState::WaitingHuman;
            lease.turn_state = TurnState::WaitingHuman;
            lease.waiting_reason = Some(reason.clone());
            lease.pending_request = Some(PendingHumanRequest {
                request_id: req_id,
                kind: PendingRequestKind::Approval,
                reason,
                created_at: now_iso.clone(),
                source: TelemetrySource::Hook,
                tool_name: tool.clone(),
            });
            if tool.is_some() {
                lease.last_tool = tool;
            }
            lease.last_progress_at = now_iso;
        }
        HookEventKind::NotificationWait => {
            let reason = input
                .waiting_reason
                .or(input.summary)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Waiting for human input".into());
            let req_id = input
                .request_id
                .unwrap_or_else(|| format!("notif_{:012x}", fnv1a64(reason.as_bytes())));
            lease.state = SessionState::WaitingHuman;
            lease.turn_state = TurnState::WaitingHuman;
            lease.waiting_reason = Some(reason.clone());
            lease.pending_request = Some(PendingHumanRequest {
                request_id: req_id,
                kind: PendingRequestKind::Question,
                reason,
                created_at: now_iso.clone(),
                source: TelemetrySource::Hook,
                tool_name: input.tool_name,
            });
            lease.last_progress_at = now_iso;
        }
        HookEventKind::TurnStop => {
            // Turn finished cleanly. Never auto-close the issue!
            // Clear transient approval prompts, but preserve explicit manual/question waits.
            if lease
                .pending_request
                .as_ref()
                .is_some_and(|r| r.kind == PendingRequestKind::Approval)
            {
                lease.pending_request = None;
                lease.waiting_reason = None;
            }
            if lease.pending_request.is_some() {
                lease.state = SessionState::WaitingHuman;
                lease.turn_state = TurnState::WaitingHuman;
            } else {
                lease.state = SessionState::Working;
                lease.turn_state = TurnState::IdleBetweenTurns;
                if let Some(s) = input.summary.filter(|s| !s.trim().is_empty()) {
                    lease.summary = s;
                }
            }
            lease.last_progress_at = now_iso;
        }
        HookEventKind::SessionEnd => unreachable!(),
    }

    write_lease_file(&path, &lease)?;
    Ok(Some(lease))
}

/// Upserts a per-session lease when an agent claims a task via MCP or CLI.
pub fn upsert_claim_lease(runtime_dir: &Path, input: ClaimLeaseInput) -> CoreResult<SessionLease> {
    let sid = match input.session_id {
        Some(raw) if !raw.trim().is_empty() => sanitize_session_id(&raw)?,
        _ => generate_session_id(),
    };
    let scope = normalize_scope_list(&input.scope)?;

    fs::create_dir_all(runtime_dir)
        .map_err(|e| CoreError::Io(format!("failed to create runtime dir: {e}")))?;

    let _lock = FileLockGuard::acquire(&lease_lock_path(runtime_dir, &sid), Duration::from_secs(2))?;
    let path = lease_file_path(runtime_dir, &sid);
    let existing = read_lease_file(&path);

    let now_ms = now_unix_ms();
    let now_iso = format_unix_ms_rfc3339(now_ms);
    let ttl = input.ttl_secs.unwrap_or(ACTIVE_WINDOW_SECS).max(60);
    let expires_iso = format_unix_ms_rfc3339(now_ms + ttl * 1000);

    let seq = existing.as_ref().map(|e| e.seq + 1).unwrap_or(1);
    let started_at = existing
        .as_ref()
        .filter(|e| e.issue_id.as_deref() == Some(input.issue_id.as_str()))
        .map(|e| e.started_at.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| now_iso.clone());

    let summary = if input.summary.trim().is_empty() {
        "claimed task".to_string()
    } else {
        input.summary.trim().to_string()
    };

    let lease = SessionLease {
        session_id: sid,
        issue_id: Some(input.issue_id),
        worktree_id: input.worktree_id,
        worktree_path: input.worktree_path,
        branch: input.branch,
        harness: non_empty_or(input.harness, "unknown"),
        agent: non_empty_or(input.agent, "agent"),
        source: existing
            .as_ref()
            .map(|e| e.source)
            .unwrap_or(TelemetrySource::Mcp),
        state: SessionState::Working,
        turn_state: TurnState::Running,
        summary,
        waiting_reason: None,
        pending_request: None,
        last_tool: existing.and_then(|e| e.last_tool),
        scope,
        thread_url: input
            .thread_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        seq,
        started_at,
        last_seen_at: now_iso.clone(),
        last_activity_at: now_iso.clone(),
        last_progress_at: now_iso,
        expires_at: expires_iso,
        reachability: SessionReachability::Active,
        stale_heartbeat: false,
    };

    write_lease_file(&path, &lease)?;
    Ok(lease)
}

/// Updates an existing or new per-session lease from MCP or CLI.
///
/// Key reliability rule: `WaitingHuman` creates a **persistent** `PendingHumanRequest` that NEVER
/// expires on a short heartbeat TTL — it stays visible until resolved by `Working`/`Released`,
/// a hook completion event, `bl_finish`, or human dismissal in the UI.
pub fn upsert_pulse_lease(runtime_dir: &Path, input: PulseLeaseInput) -> CoreResult<SessionLease> {
    let sid = sanitize_session_id(&input.session_id)?;

    if input.state == SessionState::WaitingHuman {
        let reason_ok = input
            .waiting_reason
            .as_deref()
            .is_some_and(|r| !r.trim().is_empty());
        if !reason_ok {
            return Err(CoreError::InvalidOperation(
                "waiting_human state requires a non-empty waitingReason".into(),
            ));
        }
    }

    let next_scope = match input.scope {
        Some(raw) => Some(normalize_scope_list(&raw)?),
        None => None,
    };

    fs::create_dir_all(runtime_dir)
        .map_err(|e| CoreError::Io(format!("failed to create runtime dir: {e}")))?;

    let _lock = FileLockGuard::acquire(&lease_lock_path(runtime_dir, &sid), Duration::from_secs(2))?;
    let path = lease_file_path(runtime_dir, &sid);
    let existing = read_lease_file(&path);

    let now_ms = now_unix_ms();
    let now_iso = format_unix_ms_rfc3339(now_ms);
    let ttl = input.ttl_secs.unwrap_or(ACTIVE_WINDOW_SECS).max(60);
    let expires_iso = format_unix_ms_rfc3339(now_ms + ttl * 1000);

    let prev = existing.unwrap_or_else(|| SessionLease {
        session_id: sid.clone(),
        issue_id: input.issue_id.clone(),
        worktree_id: input.worktree_id.clone(),
        worktree_path: input.worktree_path.clone(),
        branch: input.branch.clone(),
        harness: non_empty_or(input.harness.clone(), "unknown"),
        agent: non_empty_or(input.agent.clone(), "agent"),
        source: TelemetrySource::Mcp,
        state: input.state,
        turn_state: TurnState::Running,
        summary: String::new(),
        waiting_reason: None,
        pending_request: None,
        last_tool: None,
        scope: Vec::new(),
        thread_url: None,
        seq: 0,
        started_at: now_iso.clone(),
        last_seen_at: now_iso.clone(),
        last_activity_at: now_iso.clone(),
        last_progress_at: now_iso.clone(),
        expires_at: expires_iso.clone(),
        reachability: SessionReachability::Active,
        stale_heartbeat: false,
    });

    let new_issue_id = input.issue_id.or(prev.issue_id.clone());
    let new_summary = input
        .summary
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| prev.summary.clone());
    let new_waiting = if input.state == SessionState::WaitingHuman {
        input
            .waiting_reason
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    let new_pending = if let Some(ref reason) = new_waiting {
        Some(PendingHumanRequest {
            request_id: prev
                .pending_request
                .as_ref()
                .filter(|r| r.reason == *reason)
                .map(|r| r.request_id.clone())
                .unwrap_or_else(|| format!("wait_{:012x}", fnv1a64(reason.as_bytes()))),
            kind: PendingRequestKind::ManualWait,
            reason: reason.clone(),
            created_at: prev
                .pending_request
                .as_ref()
                .filter(|r| r.reason == *reason)
                .map(|r| r.created_at.clone())
                .unwrap_or_else(|| now_iso.clone()),
            source: TelemetrySource::Mcp,
            tool_name: None,
        })
    } else {
        None
    };

    let meaningful_change = prev.seq == 0
        || prev.state != input.state
        || prev.issue_id != new_issue_id
        || prev.summary != new_summary
        || prev.waiting_reason != new_waiting;

    if !meaningful_change {
        if let Some(prev_seen_ms) = parse_rfc3339_to_unix_ms(&prev.last_seen_at) {
            if now_ms.saturating_sub(prev_seen_ms) < KEEPALIVE_THROTTLE_SECS * 1000 {
                return Ok(prev);
            }
        }
    }

    let next_turn_state = match input.state {
        SessionState::WaitingHuman => TurnState::WaitingHuman,
        SessionState::Released => TurnState::Ended,
        SessionState::Working => TurnState::Running,
    };

    let lease = SessionLease {
        session_id: sid,
        issue_id: new_issue_id,
        worktree_id: if input.worktree_id.is_empty() {
            prev.worktree_id
        } else {
            input.worktree_id
        },
        worktree_path: if input.worktree_path.is_empty() {
            prev.worktree_path
        } else {
            input.worktree_path
        },
        branch: input.branch.or(prev.branch),
        harness: if input.harness.trim().is_empty() {
            prev.harness
        } else {
            input.harness.trim().to_string()
        },
        agent: if input.agent.trim().is_empty() {
            prev.agent
        } else {
            input.agent.trim().to_string()
        },
        source: prev.source,
        state: input.state,
        turn_state: next_turn_state,
        summary: new_summary,
        waiting_reason: new_waiting,
        pending_request: new_pending,
        last_tool: prev.last_tool,
        scope: next_scope.unwrap_or(prev.scope),
        thread_url: input
            .thread_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or(prev.thread_url),
        seq: prev.seq + 1,
        started_at: if prev.started_at.is_empty() {
            now_iso.clone()
        } else {
            prev.started_at
        },
        last_seen_at: now_iso.clone(),
        last_activity_at: now_iso.clone(),
        last_progress_at: if meaningful_change || prev.last_progress_at.is_empty() {
            now_iso
        } else {
            prev.last_progress_at
        },
        expires_at: expires_iso,
        reachability: SessionReachability::Active,
        stale_heartbeat: false,
    };

    write_lease_file(&path, &lease)?;
    Ok(lease)
}

pub fn release_session_lease(runtime_dir: &Path, session_id: &str) -> CoreResult<bool> {
    let sid = sanitize_session_id(session_id)?;
    let path = lease_file_path(runtime_dir, &sid);
    if !path.exists() {
        return Ok(false);
    }
    let _lock = FileLockGuard::acquire(&lease_lock_path(runtime_dir, &sid), Duration::from_secs(2))?;
    let _ = fs::remove_file(&path);
    Ok(true)
}

/// Explicitly dismisses a pending human wait from the GUI, restoring `Working` / `Active`.
pub fn clear_waiting_human(runtime_dir: &Path, session_id: &str) -> CoreResult<bool> {
    let sid = sanitize_session_id(session_id)?;
    let path = lease_file_path(runtime_dir, &sid);
    if !path.exists() {
        return Ok(false);
    }
    let _lock = FileLockGuard::acquire(&lease_lock_path(runtime_dir, &sid), Duration::from_secs(2))?;
    let Some(mut lease) = read_lease_file(&path) else {
        return Ok(false);
    };
    if lease.state == SessionState::WaitingHuman || lease.pending_request.is_some() {
        let now_iso = now_rfc3339_utc();
        lease.state = SessionState::Working;
        lease.turn_state = TurnState::Running;
        lease.waiting_reason = None;
        lease.pending_request = None;
        lease.seq += 1;
        lease.last_activity_at = now_iso.clone();
        lease.last_progress_at = now_iso;
        write_lease_file(&path, &lease)?;
        return Ok(true);
    }
    Ok(false)
}

/// Evaluates honest reachability for a persisted lease at `now_ms`.
///
/// Crucially, a 6-minute test/build run stays `Active` (<15m), `IdleBetweenTurns` stays `Idle`
/// (not an alarm), and `Unconfirmed` only triggers after 45m of silence — and NEVER suppresses
/// an unresolved `pending_request`.
pub fn evaluate_reachability(lease: &SessionLease, now_ms: u64) -> SessionReachability {
    let last_ts = if !lease.last_activity_at.is_empty() {
        parse_rfc3339_to_unix_ms(&lease.last_activity_at)
    } else {
        parse_rfc3339_to_unix_ms(&lease.last_seen_at)
    }
    .unwrap_or(0);

    let elapsed_secs = now_ms.saturating_sub(last_ts) / 1000;

    if lease.turn_state == TurnState::IdleBetweenTurns {
        return if elapsed_secs <= UNCONFIRMED_WINDOW_SECS {
            SessionReachability::Idle
        } else {
            SessionReachability::Unconfirmed
        };
    }

    if elapsed_secs <= ACTIVE_WINDOW_SECS {
        SessionReachability::Active
    } else if elapsed_secs <= UNCONFIRMED_WINDOW_SECS {
        SessionReachability::Idle
    } else {
        SessionReachability::Unconfirmed
    }
}

/// Reads all per-session lease files from `runtime_dir` at current wall clock.
pub fn list_leases(runtime_dir: &Path) -> CoreResult<Vec<SessionLease>> {
    list_leases_at(runtime_dir, now_unix_ms())
}

pub fn list_leases_at(runtime_dir: &Path, now_ms: u64) -> CoreResult<Vec<SessionLease>> {
    if !runtime_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut leases = Vec::new();
    let entries = fs::read_dir(runtime_dir)
        .map_err(|e| CoreError::Io(format!("failed to read runtime dir: {e}")))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        let Some(mut lease) = read_lease_file(&path) else {
            continue;
        };

        // Backfill pending_request for legacy v1 lease files written before PendingHumanRequest existed
        if lease.pending_request.is_none() && lease.state == SessionState::WaitingHuman {
            if let Some(reason) = lease.waiting_reason.clone().filter(|r| !r.trim().is_empty()) {
                lease.pending_request = Some(PendingHumanRequest {
                    request_id: format!("legacy_{}", lease.session_id),
                    kind: PendingRequestKind::ManualWait,
                    reason,
                    created_at: lease.last_progress_at.clone(),
                    source: lease.source,
                    tool_name: lease.last_tool.clone(),
                });
            }
        }
        if lease.last_activity_at.is_empty() {
            lease.last_activity_at = lease.last_seen_at.clone();
        }

        let reachability = evaluate_reachability(&lease, now_ms);
        lease.reachability = reachability;
        lease.stale_heartbeat = reachability == SessionReachability::Unconfirmed;

        let last_ts = parse_rfc3339_to_unix_ms(&lease.last_activity_at).unwrap_or(0);
        let age_secs = now_ms.saturating_sub(last_ts) / 1000;

        if lease.state == SessionState::Released || lease.turn_state == TurnState::Ended {
            if age_secs > RETENTION_WINDOW_SECS {
                let _ = fs::remove_file(&path);
            }
            continue;
        }

        // Never auto-prune a session that still has an unresolved pending human request!
        if lease.pending_request.is_none()
            && reachability == SessionReachability::Unconfirmed
            && age_secs > RETENTION_WINDOW_SECS
        {
            let _ = fs::remove_file(&path);
            continue;
        }

        leases.push(lease);
    }

    leases.sort_by(|a, b| {
        b.pending_request
            .is_some()
            .cmp(&a.pending_request.is_some())
            .then_with(|| a.stale_heartbeat.cmp(&b.stale_heartbeat))
            .then_with(|| b.last_activity_at.cmp(&a.last_activity_at))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    Ok(leases)
}

pub fn runtime_fingerprint(runtime_dir: &Path) -> String {
    if !runtime_dir.is_dir() {
        return "empty".to_string();
    }
    let mut items: Vec<(String, Vec<u8>)> = Vec::new();
    if let Ok(entries) = fs::read_dir(runtime_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !name.ends_with(".json") {
                continue;
            }
            if let Ok(bytes) = fs::read(&path) {
                items.push((name.to_string(), bytes));
            }
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash: u64 = 0xcbf29ce484222325;
    for (name, bytes) in items {
        for &b in name.as_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        for &b in &bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn read_lease_file(path: &Path) -> Option<SessionLease> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<SessionLease>(&raw).ok()
}

fn write_lease_file(path: &Path, lease: &SessionLease) -> CoreResult<()> {
    let bytes = serde_json::to_vec_pretty(lease)
        .map_err(|e| CoreError::Internal(format!("failed to serialize session lease: {e}")))?;
    atomic_write_bytes(path, &bytes)
}

fn non_empty_or(val: String, fallback: &str) -> String {
    let trimmed = val.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_minute_test_stays_active_and_ten_minute_approval_never_disappears() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rt = dir.path().join("pulses");

        let running = apply_hook_event(
            &rt,
            HookEventInput {
                session_id: "claude_run1".into(),
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: Some("feat/tests".into()),
                harness: "claude-code".into(),
                agent: "claude".into(),
                event: HookEventKind::PreToolUse,
                tool_name: Some("Bash".into()),
                request_id: Some("toolu_1".into()),
                summary: Some("cargo test --workspace".into()),
                waiting_reason: None,
            },
        )
        .unwrap()
        .unwrap();

        let started_ms = parse_rfc3339_to_unix_ms(&running.last_activity_at).unwrap();
        // 6 minutes later with zero heartbeats -> MUST remain Active!
        let at_6m = list_leases_at(&rt, started_ms + 6 * 60_000).unwrap();
        assert_eq!(at_6m[0].reachability, SessionReachability::Active);
        assert!(!at_6m[0].stale_heartbeat);

        // Now trigger PermissionRequest hook
        apply_hook_event(
            &rt,
            HookEventInput {
                session_id: "claude_run1".into(),
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: Some("feat/tests".into()),
                harness: "claude-code".into(),
                agent: "claude".into(),
                event: HookEventKind::PermissionRequest,
                tool_name: Some("Bash".into()),
                request_id: Some("perm_99".into()),
                summary: None,
                waiting_reason: Some("Allow running `git push`?".into()),
            },
        )
        .unwrap();

        // 10 minutes AND even 60 minutes later -> pending_request MUST still be preserved!
        let at_10m = list_leases_at(&rt, started_ms + 10 * 60_000).unwrap();
        assert!(at_10m[0].pending_request.is_some());
        assert_eq!(at_10m[0].reachability, SessionReachability::Active);

        let at_60m = list_leases_at(&rt, started_ms + 60 * 60_000).unwrap();
        assert!(at_60m[0].pending_request.is_some());
        assert_eq!(at_60m[0].reachability, SessionReachability::Unconfirmed);

        // PostToolUse resolves the approval cleanly
        apply_hook_event(
            &rt,
            HookEventInput {
                session_id: "claude_run1".into(),
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: Some("feat/tests".into()),
                harness: "claude-code".into(),
                agent: "claude".into(),
                event: HookEventKind::PostToolUse,
                tool_name: Some("Bash".into()),
                request_id: Some("perm_99".into()),
                summary: Some("Bash completed".into()),
                waiting_reason: None,
            },
        )
        .unwrap();
        let after = list_leases(&rt).unwrap();
        assert!(after[0].pending_request.is_none());
        assert_eq!(after[0].state, SessionState::Working);
    }

    #[test]
    fn per_session_leases_do_not_overwrite_each_other_and_require_waiting_reason() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rt = dir.path().join("pulses");

        let s1 = upsert_claim_lease(
            &rt,
            ClaimLeaseInput {
                session_id: Some("ses_a".into()),
                issue_id: "bl-1".into(),
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: Some("feat/a".into()),
                harness: "claude-code".into(),
                agent: "claude".into(),
                summary: "wiring auth".into(),
                scope: vec!["src/auth/**".into()],
                thread_url: None,
                ttl_secs: None,
            },
        )
        .unwrap();

        let s2 = upsert_claim_lease(
            &rt,
            ClaimLeaseInput {
                session_id: Some("ses_b".into()),
                issue_id: "bl-1".into(),
                worktree_id: "wt_2".into(),
                worktree_path: "/repo-wt2".into(),
                branch: Some("feat/b".into()),
                harness: "codex".into(),
                agent: "gpt".into(),
                summary: "writing tests".into(),
                scope: vec!["tests/**".into()],
                thread_url: None,
                ttl_secs: None,
            },
        )
        .unwrap();

        assert_eq!(s1.session_id, "ses_a");
        assert_eq!(s2.session_id, "ses_b");
        assert_eq!(list_leases(&rt).unwrap().len(), 2);

        let err = upsert_pulse_lease(
            &rt,
            PulseLeaseInput {
                session_id: "ses_a".into(),
                issue_id: None,
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: None,
                harness: "claude-code".into(),
                agent: "claude".into(),
                state: SessionState::WaitingHuman,
                summary: Some("blocked".into()),
                waiting_reason: None,
                scope: None,
                thread_url: None,
                ttl_secs: None,
            },
        )
        .unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");

        let waiting = upsert_pulse_lease(
            &rt,
            PulseLeaseInput {
                session_id: "ses_a".into(),
                issue_id: None,
                worktree_id: "wt_1".into(),
                worktree_path: "/repo".into(),
                branch: None,
                harness: "claude-code".into(),
                agent: "claude".into(),
                state: SessionState::WaitingHuman,
                summary: Some("need token decision".into()),
                waiting_reason: Some("Should refresh tokens rotate on every use?".into()),
                scope: None,
                thread_url: None,
                ttl_secs: None,
            },
        )
        .unwrap();
        assert!(waiting.pending_request.is_some());
        assert!(clear_waiting_human(&rt, "ses_a").unwrap());
        let after = list_leases(&rt).unwrap();
        let a = after.iter().find(|l| l.session_id == "ses_a").unwrap();
        assert!(a.pending_request.is_none());
        assert_eq!(a.state, SessionState::Working);
    }
}
