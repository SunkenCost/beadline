use beadline_core::fs_util::fnv1a64_hex;
use beadline_core::locator::RepoLocator;
use beadline_core::runtime_store::{apply_hook_event, HookEventInput, HookEventKind};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_HOOK_STDIN_BYTES: u64 = 256 * 1024;

/// Processes a native harness lifecycle hook payload from `stdin`.
///
/// Design contract:
/// - **Strictly fail-open**: NEVER returns non-zero or writes to `stdout`, so Beadline
///   telemetry can never block, hang, or interfere with Claude Code / Codex / Cursor / Antigravity.
/// - **Zero-noise on non-repo directories**: if `cwd` is outside any Git repo or Beadline store,
///   exits immediately without creating any files.
pub fn handle_hook_from_reader<R: Read>(
    harness: &str,
    event_override: Option<&str>,
    fallback_repo: &Path,
    reader: R,
) {
    let mut raw = String::new();
    let _ = reader.take(MAX_HOOK_STDIN_BYTES).read_to_string(&mut raw);
    let payload: Value = serde_json::from_str(raw.trim()).unwrap_or(Value::Null);
    let _ = process_hook_payload(harness, event_override, fallback_repo, &payload);
}

pub fn process_hook_payload(
    harness_raw: &str,
    event_override: Option<&str>,
    fallback_repo: &Path,
    payload: &Value,
) -> Option<()> {
    let harness = normalize_harness(harness_raw);
    let event_name = event_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| payload.get("hook_event_name").and_then(Value::as_str))
        .or_else(|| payload.get("event").and_then(Value::as_str))
        .or_else(|| payload.get("hookEventName").and_then(Value::as_str))?;

    let event = map_event_kind(event_name, payload)?;

    let cwd = extract_cwd(payload).unwrap_or_else(|| fallback_repo.to_path_buf());
    let loc = RepoLocator::locate(&cwd).ok()?;
    // Only record hook telemetry for Git repositories or initialized Beadline workspaces
    if loc.git_dir.is_none() && !loc.has_store {
        return None;
    }

    let raw_sid = payload
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("conversationId").and_then(Value::as_str))
        .or_else(|| payload.get("conversation_id").and_then(Value::as_str))
        .or_else(|| payload.get("thread_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let session_id = match raw_sid {
        Some(sid) => format!("{}_{}", harness_prefix(&harness), sanitize_id_part(sid)),
        None => {
            // Deterministic fallback per harness + worktree when host omits session_id
            format!(
                "{}_{}",
                harness_prefix(&harness),
                fnv1a64_hex(loc.worktree_id.as_bytes())
            )
        }
    };

    let tool_name = extract_tool_name(event_name, payload);
    let request_id = payload
        .get("tool_use_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("request_id").and_then(Value::as_str))
        .or_else(|| payload.get("requestId").and_then(Value::as_str))
        .map(String::from);

    let agent = payload
        .get("model")
        .and_then(Value::as_str)
        .or_else(|| payload.get("agent_type").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&harness)
        .to_string();

    let (summary, waiting_reason) = extract_descriptions(event, tool_name.as_deref(), payload);

    let _ = apply_hook_event(
        &loc.runtime_dir,
        HookEventInput {
            session_id,
            worktree_id: loc.worktree_id,
            worktree_path: loc.worktree_display,
            branch: loc.branch,
            harness,
            agent,
            event,
            tool_name,
            request_id,
            summary,
            waiting_reason,
        },
    );
    Some(())
}

fn normalize_harness(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude-code" | "claudecode" => "claude-code".into(),
        "codex" | "codex-cli" => "codex".into(),
        "cursor" => "cursor".into(),
        "antigravity" | "agy" => "antigravity".into(),
        "t3" | "t3-code" | "t3code" => "t3-code".into(),
        other if !other.is_empty() && other != "auto" => other.to_string(),
        _ => "hook".into(),
    }
}

fn harness_prefix(harness: &str) -> &str {
    match harness {
        "claude-code" => "claude",
        "codex" => "codex",
        "cursor" => "cursor",
        "antigravity" => "agy",
        "t3-code" => "t3",
        _ => "hk",
    }
}

fn sanitize_id_part(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(44)
        .collect();
    if clean.is_empty() {
        fnv1a64_hex(raw.as_bytes())
    } else {
        clean
    }
}

fn map_event_kind(event_name: &str, payload: &Value) -> Option<HookEventKind> {
    let norm = event_name.trim().to_ascii_lowercase();
    match norm.as_str() {
        "sessionstart" | "session_start" | "workspaceopen" => Some(HookEventKind::SessionStart),
        "pretooluse" | "pre_tool_use" | "beforeshellexecution" | "beforemcpexecution"
        | "beforesubmitprompt" | "preinvocation" => Some(HookEventKind::PreToolUse),
        "posttooluse" | "post_tool_use" | "posttoolusefailure" | "afterfileedit"
        | "aftershellexecution" | "aftermcpexecution" | "postinvocation" => {
            Some(HookEventKind::PostToolUse)
        }
        "permissionrequest" | "permission_request" => Some(HookEventKind::PermissionRequest),
        "notification" => {
            // Only treat explicit permission/elicitation/input prompts as waiting_human,
            // ignoring passive idle notifications so we never spam false alerts.
            let ntype = payload
                .get("notification_type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_ascii_lowercase();
            if ntype.contains("permission")
                || ntype.contains("elicitation")
                || ntype.contains("input")
                || ntype.contains("question")
            {
                Some(HookEventKind::NotificationWait)
            } else {
                None
            }
        }
        "stop" | "afteragentresponse" | "subagentstop" => Some(HookEventKind::TurnStop),
        "sessionend" | "session_end" => Some(HookEventKind::SessionEnd),
        _ => None,
    }
}

fn extract_cwd(payload: &Value) -> Option<PathBuf> {
    if let Some(wt) = payload.get("worktree").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        return Some(PathBuf::from(wt));
    }
    if let Some(cwd) = payload.get("cwd").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        return Some(PathBuf::from(cwd));
    }
    for arr_key in ["workspacePaths", "workspace_roots", "workspace_paths"] {
        if let Some(first) = payload
            .get(arr_key)
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return Some(PathBuf::from(first));
        }
    }
    None
}

fn extract_tool_name(event_name: &str, payload: &Value) -> Option<String> {
    if let Some(t) = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("toolName").and_then(Value::as_str))
        .or_else(|| {
            payload
                .get("toolCall")
                .and_then(|tc| tc.get("name"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(t.to_string());
    }

    let lower = event_name.trim().to_ascii_lowercase();
    if lower.contains("shell") {
        Some("Shell".into())
    } else if lower.contains("fileedit") {
        Some("Edit".into())
    } else if lower.contains("mcp") {
        Some("MCP".into())
    } else {
        None
    }
}

fn extract_descriptions(
    event: HookEventKind,
    tool_name: Option<&str>,
    payload: &Value,
) -> (Option<String>, Option<String>) {
    let tool_hint = extract_safe_tool_hint(payload);

    match event {
        HookEventKind::PermissionRequest => {
            let reason = match (tool_name, tool_hint) {
                (Some(t), Some(h)) => format!("Approval needed: {t} ({h})"),
                (Some(t), None) => format!("Approval needed: {t}"),
                (None, Some(h)) => format!("Approval needed: {h}"),
                (None, None) => "Approval needed".into(),
            };
            (Some(reason.clone()), Some(reason))
        }
        HookEventKind::NotificationWait => {
            let title = payload
                .get("title")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let msg = payload
                .get("message")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            let combined = match (title.is_empty(), msg.is_empty()) {
                (false, false) => format!("{title}: {msg}"),
                (true, false) => msg.to_string(),
                (false, true) => title.to_string(),
                (true, true) => "Waiting for your input".into(),
            };
            let clipped = clip_chars(&combined, 140);
            (Some(clipped.clone()), Some(clipped))
        }
        HookEventKind::PreToolUse => {
            let sum = match (tool_name, tool_hint) {
                (Some(t), Some(h)) => Some(format!("{t}: {h}")),
                (Some(t), None) => Some(format!("Running {t}")),
                _ => None,
            };
            (sum, None)
        }
        HookEventKind::PostToolUse => {
            let sum = match (tool_name, tool_hint) {
                (Some(t), Some(h)) => Some(format!("Done {t}: {h}")),
                (Some(t), None) => Some(format!("Done {t}")),
                _ => None,
            };
            (sum, None)
        }
        HookEventKind::TurnStop => (Some("Turn complete — idle".into()), None),
        HookEventKind::SessionStart => (Some("Session active".into()), None),
        HookEventKind::SessionEnd => (None, None),
    }
}

fn extract_safe_tool_hint(payload: &Value) -> Option<String> {
    let input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolCall").and_then(|tc| tc.get("args")))
        .unwrap_or(payload);

    for path_key in ["file_path", "path", "Path", "target_file"] {
        if let Some(p) = input.get(path_key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
            let base = Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p);
            return Some(clip_chars(base, 64));
        }
    }

    for cmd_key in ["command", "CommandLine", "cmd"] {
        if let Some(c) = input.get(cmd_key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
            let first_line = c.lines().next().unwrap_or(c).trim();
            return Some(clip_chars(first_line, 64));
        }
    }

    None
}

fn clip_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in s.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use beadline_core::fs_util::configure_no_window;
    use beadline_core::models::{PendingRequestKind, SessionState, TurnState};
    use beadline_core::runtime_store::list_leases;
    use serde_json::json;
    use std::process::Command;

    fn init_git_repo(dir: &Path) {
        let mut cmd = Command::new("git");
        cmd.args(["init", "-b", "main"]).current_dir(dir);
        configure_no_window(&mut cmd);
        assert!(cmd.output().unwrap().status.success());
    }

    #[test]
    fn claude_and_codex_hooks_auto_register_sessions_and_track_approvals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_git_repo(tmp.path());
        let loc = RepoLocator::locate(tmp.path()).unwrap();

        // 1. Claude Code SessionStart (zero bl_claim!)
        let start = json!({
            "session_id": "abc-123",
            "cwd": tmp.path().to_string_lossy(),
            "hook_event_name": "SessionStart",
            "model": "claude-opus"
        });
        assert!(process_hook_payload("claude-code", None, tmp.path(), &start).is_some());

        let leases = list_leases(&loc.runtime_dir).unwrap();
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].session_id, "claude_abc-123");
        assert!(leases[0].issue_id.is_none()); // unlinked session auto-discovered!

        // 2. Claude Code PermissionRequest -> persistent Approval pending_request
        let perm = json!({
            "session_id": "abc-123",
            "cwd": tmp.path().to_string_lossy(),
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_use_id": "toolu_99",
            "tool_input": { "command": "cargo test --workspace" }
        });
        assert!(process_hook_payload("claude-code", None, tmp.path(), &perm).is_some());
        let leases = list_leases(&loc.runtime_dir).unwrap();
        let req = leases[0].pending_request.as_ref().expect("pending approval");
        assert_eq!(req.kind, PendingRequestKind::Approval);
        assert!(req.reason.contains("Bash"));
        assert!(req.reason.contains("cargo test"));

        // 3. PostToolUse resolves approval cleanly
        let post = json!({
            "session_id": "abc-123",
            "cwd": tmp.path().to_string_lossy(),
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_use_id": "toolu_99"
        });
        assert!(process_hook_payload("claude-code", None, tmp.path(), &post).is_some());
        let leases = list_leases(&loc.runtime_dir).unwrap();
        assert!(leases[0].pending_request.is_none());
        assert_eq!(leases[0].state, SessionState::Working);

        // 4. Codex CLI Stop event sets IdleBetweenTurns without closing anything
        let codex_stop = json!({
            "session_id": "sess_codex_1",
            "event": "Stop",
            "cwd": tmp.path().to_string_lossy()
        });
        assert!(process_hook_payload("codex", None, tmp.path(), &codex_stop).is_some());
        let leases = list_leases(&loc.runtime_dir).unwrap();
        let codex_lease = leases
            .iter()
            .find(|l| l.session_id == "codex_sess_codex_1")
            .unwrap();
        assert_eq!(codex_lease.turn_state, TurnState::IdleBetweenTurns);
    }
}
