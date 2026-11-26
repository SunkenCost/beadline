use crate::error::{CoreError, CoreResult};
use crate::fs_util::{atomic_write_bytes, fnv1a64, FileLockGuard};
use crate::models::{SessionLease, SessionState};
use crate::scope::normalize_scope_list;
use crate::time::{format_unix_ms_rfc3339, now_rfc3339_utc, now_unix_ms, parse_rfc3339_to_unix_ms};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub const DEFAULT_LEASE_TTL_SECS: u64 = 120;
pub const KEEPALIVE_THROTTLE_SECS: u64 = 30;
pub const EXPIRED_RETENTION_SECS: u64 = 3600;

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

/// Upserts a per-session lease when an agent claims a task.
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
    let ttl = input.ttl_secs.unwrap_or(DEFAULT_LEASE_TTL_SECS).max(30);
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
        state: SessionState::Working,
        summary,
        waiting_reason: None,
        scope,
        thread_url: input
            .thread_url
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        seq,
        started_at,
        last_seen_at: now_iso.clone(),
        last_progress_at: now_iso,
        expires_at: expires_iso,
        stale_heartbeat: false,
    };

    write_lease_file(&path, &lease)?;
    Ok(lease)
}

/// Updates an existing or new per-session lease.
///
/// Rules:
/// - `SessionState::WaitingHuman` MUST include a non-empty `waiting_reason`.
/// - `last_progress_at` only advances when state, issue_id, summary, or waiting_reason actually changed.
/// - Identical keepalive pulses within 30s still extend `expires_at` only when needed to avoid churn.
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
    let ttl = input.ttl_secs.unwrap_or(DEFAULT_LEASE_TTL_SECS).max(30);
    let expires_iso = format_unix_ms_rfc3339(now_ms + ttl * 1000);

    let prev = existing.unwrap_or_else(|| SessionLease {
        session_id: sid.clone(),
        issue_id: input.issue_id.clone(),
        worktree_id: input.worktree_id.clone(),
        worktree_path: input.worktree_path.clone(),
        branch: input.branch.clone(),
        harness: non_empty_or(input.harness.clone(), "unknown"),
        agent: non_empty_or(input.agent.clone(), "agent"),
        state: input.state,
        summary: String::new(),
        waiting_reason: None,
        scope: Vec::new(),
        thread_url: None,
        seq: 0,
        started_at: now_iso.clone(),
        last_seen_at: now_iso.clone(),
        last_progress_at: now_iso.clone(),
        expires_at: expires_iso.clone(),
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

    let meaningful_change = prev.seq == 0
        || prev.state != input.state
        || prev.issue_id != new_issue_id
        || prev.summary != new_summary
        || prev.waiting_reason != new_waiting;

    // Throttle pure identical keepalive writes within KEEPALIVE_THROTTLE_SECS
    if !meaningful_change {
        if let Some(prev_seen_ms) = parse_rfc3339_to_unix_ms(&prev.last_seen_at) {
            if now_ms.saturating_sub(prev_seen_ms) < KEEPALIVE_THROTTLE_SECS * 1000 {
                return Ok(prev);
            }
        }
    }

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
        state: input.state,
        summary: new_summary,
        waiting_reason: new_waiting,
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
        last_progress_at: if meaningful_change || prev.last_progress_at.is_empty() {
            now_iso
        } else {
            prev.last_progress_at
        },
        expires_at: expires_iso,
        stale_heartbeat: false,
    };

    write_lease_file(&path, &lease)?;
    Ok(lease)
}

/// Marks a session as `Released` (or removes its lease file if requested).
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

/// Clears `waiting_human` on a session (returning it to `working`) from the GUI.
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
    if lease.state == SessionState::WaitingHuman {
        lease.state = SessionState::Working;
        lease.waiting_reason = None;
        lease.seq += 1;
        lease.last_progress_at = now_rfc3339_utc();
        write_lease_file(&path, &lease)?;
        return Ok(true);
    }
    Ok(false)
}

/// Reads all per-session lease files from `runtime_dir`, computing `stale_heartbeat` at read time.
/// Old expired/released files beyond `EXPIRED_RETENTION_SECS` are lazily pruned.
pub fn list_leases(runtime_dir: &Path) -> CoreResult<Vec<SessionLease>> {
    if !runtime_dir.is_dir() {
        return Ok(Vec::new());
    }

    let now_ms = now_unix_ms();
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

        let expires_ms = parse_rfc3339_to_unix_ms(&lease.expires_at).unwrap_or(0);
        let expired = expires_ms > 0 && now_ms > expires_ms;
        lease.stale_heartbeat = expired;

        if lease.state == SessionState::Released {
            if expires_ms == 0 || now_ms.saturating_sub(expires_ms) > EXPIRED_RETENTION_SECS * 1000 {
                let _ = fs::remove_file(&path);
            }
            continue;
        }

        if expired && now_ms.saturating_sub(expires_ms) > EXPIRED_RETENTION_SECS * 1000 {
            let _ = fs::remove_file(&path);
            continue;
        }

        leases.push(lease);
    }

    leases.sort_by(|a, b| {
        a.stale_heartbeat
            .cmp(&b.stale_heartbeat)
            .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    Ok(leases)
}

/// Cheap content fingerprint across all per-session lease files so watcher/IPC can skip
/// redundant updates.
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
                ttl_secs: Some(120),
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
                ttl_secs: Some(120),
            },
        )
        .unwrap();

        assert_eq!(s1.session_id, "ses_a");
        assert_eq!(s2.session_id, "ses_b");
        assert_eq!(list_leases(&rt).unwrap().len(), 2);

        // waiting_human without reason must fail
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

        // waiting_human with reason succeeds
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
        assert_eq!(waiting.state, SessionState::WaitingHuman);
        assert_eq!(waiting.seq, 2);

        assert!(clear_waiting_human(&rt, "ses_a").unwrap());
        let after = list_leases(&rt).unwrap();
        let a = after.iter().find(|l| l.session_id == "ses_a").unwrap();
        assert_eq!(a.state, SessionState::Working);
        assert!(a.waiting_reason.is_none());
    }
}
