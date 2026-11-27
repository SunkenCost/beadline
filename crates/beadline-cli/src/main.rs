mod hooks;
mod mcp;

use beadline_core::error::{CoreError, CoreResult};
use beadline_core::git_radar::compute_radar_snapshot;
use beadline_core::jsonl_reader::{read_beads_config_values, read_workspace_snapshot_for_location};
use beadline_core::locator::RepoLocator;
use beadline_core::models::{BoardLane, CreateIssueRequest, SessionState, UpdateIssueRequest};
use beadline_core::native_store::{
    add_comment_in, create_issue_in, derive_prefix, export_jsonl_from_store, finish_issue_in,
    import_jsonl_into_shared, init_native_workspace, mutate_dependency_in, reopen_issue_in,
    run_doctor, update_issue_in,
};
use beadline_core::runtime_store::{
    generate_session_id, list_leases, release_session_lease, upsert_claim_lease,
    upsert_pulse_lease, ClaimLeaseInput, PulseLeaseInput,
};
use mcp::{detect_agent_from_env, detect_harness_from_env, parse_session_state, McpServer};
use serde_json::{json, Value};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

struct GlobalOpts {
    repo: PathBuf,
    json: bool,
    session: Option<String>,
    harness: Option<String>,
    agent: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        print_help();
        return ExitCode::SUCCESS;
    }
    if matches!(args[0].as_str(), "-V" | "--version" | "version") {
        println!("bl {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let (opts, rest) = match parse_global_opts(&args) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error [{}]: {err}", err.code());
            return ExitCode::from(2);
        }
    };

    if rest.is_empty() {
        print_help();
        return ExitCode::SUCCESS;
    }

    match dispatch(&opts, &rest) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if opts.json {
                let payload = json!({
                    "ok": false,
                    "error": {
                        "code": err.code(),
                        "message": err.to_string()
                    }
                });
                println!("{payload}");
            } else {
                eprintln!("error [{}]: {err}", err.code());
            }
            ExitCode::from(1)
        }
    }
}

fn parse_global_opts(args: &[String]) -> CoreResult<(GlobalOpts, Vec<String>)> {
    let mut repo: Option<PathBuf> = None;
    let mut json = false;
    let mut session = std::env::var("BL_SESSION_ID")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let mut harness: Option<String> = None;
    let mut agent: Option<String> = None;
    let mut rest = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| CoreError::InvalidOperation("--repo requires a path".into()))?;
                repo = Some(PathBuf::from(val));
            }
            "--json" => json = true,
            "--session" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| CoreError::InvalidOperation("--session requires a value".into()))?;
                session = Some(val.clone());
            }
            "--harness" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| CoreError::InvalidOperation("--harness requires a value".into()))?;
                harness = Some(val.clone());
            }
            "--agent" => {
                i += 1;
                let val = args
                    .get(i)
                    .ok_or_else(|| CoreError::InvalidOperation("--agent requires a value".into()))?;
                agent = Some(val.clone());
            }
            _ => rest.push(args[i].clone()),
        }
        i += 1;
    }

    let cwd = repo.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    Ok((
        GlobalOpts {
            repo: cwd,
            json,
            session,
            harness,
            agent,
        },
        rest,
    ))
}

fn dispatch(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let cmd = args[0].as_str();
    let tail = &args[1..];

    match cmd {
        "init" => cmd_init(opts, tail),
        "ready" => cmd_ready(opts, tail),
        "list" => cmd_list(opts, tail),
        "show" => cmd_show(opts, tail),
        "create" => cmd_create(opts, tail),
        "update" => cmd_update(opts, tail),
        "dep" => cmd_dep(opts, tail),
        "reopen" => cmd_reopen(opts, tail),
        "comment" => cmd_comment(opts, tail),
        "claim" => cmd_claim(opts, tail),
        "pulse" => cmd_pulse(opts, tail),
        "finish" | "close" => cmd_finish(opts, tail),
        "radar" => cmd_radar(opts, tail),
        "import" => cmd_import(opts, tail),
        "export" => cmd_export(opts, tail),
        "doctor" => cmd_doctor(opts),
        "hook" => cmd_hook(opts, tail),
        "mcp" => cmd_mcp(opts),
        other => Err(CoreError::InvalidOperation(format!(
            "unknown subcommand '{other}' (run `bl --help`)"
        ))),
    }
}

fn cmd_init(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let prefix_flag = take_flag_value(args, &["--prefix", "-p"]);
    let loc_before = RepoLocator::locate(&opts.repo)?;
    let prefix = prefix_flag.unwrap_or_else(|| derive_prefix(&loc_before.worktree_root));
    let loc = init_native_workspace(&opts.repo, &prefix)?;

    if opts.json {
        println!(
            "{}",
            json!({
                "ok": true,
                "repoId": loc.repo_id,
                "storeDir": loc.store_dir.to_string_lossy(),
                "storeKind": loc.store_kind,
                "runtimeDir": loc.runtime_dir.to_string_lossy(),
                "prefix": prefix
            })
        );
    } else {
        println!(
            "Initialized Beadline store at {} (prefix: {})",
            loc.store_dir.display(),
            prefix
        );
    }
    Ok(())
}

fn cmd_ready(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let limit = take_flag_value(args, &["--limit", "-n"])
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20);
    let module_filter = take_flag_value(args, &["--module", "-m"]);

    let snap = read_workspace_snapshot_for_location(
        &loc,
        loc.worktree_display.clone(),
        "mod:",
        None,
        &[],
        &[],
    )?;
    let ready: Vec<_> = snap
        .issues
        .iter()
        .filter(|i| i.lane == BoardLane::Ready && i.issue_type != "epic")
        .filter(|i| {
            if let Some(m) = &module_filter {
                i.modules.iter().any(|x| x == m || x.ends_with(&format!(":{m}")))
            } else {
                true
            }
        })
        .take(limit)
        .collect();

    if opts.json {
        let items: Vec<Value> = ready
            .iter()
            .map(|i| {
                json!({
                    "id": i.id,
                    "shortId": i.short_id,
                    "title": i.title,
                    "priority": i.priority,
                    "scope": i.scope,
                    "rev": i.rev
                })
            })
            .collect();
        println!("{}", json!({ "items": items }));
    } else if ready.is_empty() {
        println!("No ready tasks.");
    } else {
        for i in ready {
            println!("{} [P{}] {}", i.id, i.priority, i.title);
        }
    }
    Ok(())
}

fn cmd_list(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let status_filter = take_flag_value(args, &["--status", "-s"]);
    let show_all = args.iter().any(|a| a == "--all" || a == "-a");
    let snap = read_workspace_snapshot_for_location(
        &loc,
        loc.worktree_display.clone(),
        "mod:",
        None,
        &[],
        &[],
    )?;
    let items: Vec<_> = snap
        .issues
        .iter()
        .filter(|i| {
            if let Some(st) = &status_filter {
                i.status == *st
            } else if show_all {
                true
            } else {
                i.status != "closed"
            }
        })
        .collect();

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&items)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else if items.is_empty() {
        println!("No matching issues.");
    } else {
        for i in items {
            println!(
                "{:<14} [{:<11}] P{} {}",
                i.id, i.status, i.priority, i.title
            );
        }
    }
    Ok(())
}

fn cmd_show(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let id = args
        .first()
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl show <issue-id>".into()))?;
    let loc = RepoLocator::locate(&opts.repo)?;
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
        .find(|i| i.id == *id || i.short_id == *id)
        .ok_or_else(|| CoreError::NotFound(format!("issue not found: {id}")))?;
    let sessions: Vec<_> = list_leases(&loc.runtime_dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|l| !l.stale_heartbeat && l.issue_id.as_deref() == Some(issue.id.as_str()))
        .collect();

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "issue": issue, "activeSessions": sessions }))
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!("{} — {} [{} · P{} · rev {}]", issue.id, issue.title, issue.status, issue.priority, issue.rev);
        if !issue.description.is_empty() {
            println!("\nWhy:\n{}", issue.description);
        }
        if !issue.acceptance_criteria.is_empty() {
            println!("\nAcceptance Criteria:\n{}", issue.acceptance_criteria);
        }
        if !issue.scope.is_empty() {
            println!("\nScope: {}", issue.scope.join(", "));
        }
        if !issue.open_blocked_by.is_empty() {
            println!("\nBlocked by: {}", issue.open_blocked_by.join(", "));
        }
        if let Some(proof) = &issue.proof {
            println!("\nProof: {}", proof.summary);
        }
        if !sessions.is_empty() {
            println!("\nActive Sessions:");
            for s in sessions {
                println!("  - {} ({}, {:?}): {}", s.session_id, s.harness, s.state, s.summary);
            }
        }
    }
    Ok(())
}

fn cmd_create(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let title = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl create <title> [flags]".into()))?
        .clone();

    let issue_type = take_flag_value(args, &["-t", "--type"]).unwrap_or_else(|| "task".into());
    let priority = take_flag_value(args, &["-p", "--priority"])
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(2);
    let status = take_flag_value(args, &["--status"]).unwrap_or_else(|| "open".into());
    let assignee = take_flag_value(args, &["--assignee"]);
    let parent_id = take_flag_value(args, &["--parent"]);
    let description = take_flag_value(args, &["-d", "--description"]);
    let acceptance_criteria = take_flag_value(args, &["--acceptance"]);
    let notes = take_flag_value(args, &["--notes"]);
    let labels = take_multi_flag_values(args, &["-l", "--label"]);
    let scope = take_multi_flag_values(args, &["--scope"]);
    let blocked_by = take_multi_flag_values(args, &["--blocked-by"]);

    let (cfg_prefix, _) = read_beads_config_values(&loc.store_dir);
    let prefix = cfg_prefix.unwrap_or_else(|| derive_prefix(&loc.worktree_root));

    let id = create_issue_in(
        &loc.store_dir,
        &CreateIssueRequest {
            workspace_path: loc.worktree_display,
            title,
            issue_type,
            priority,
            status,
            assignee,
            parent_id,
            labels,
            description,
            acceptance_criteria,
            notes,
            scope,
            blocked_by,
        },
        &prefix,
    )?;

    if opts.json {
        println!("{}", json!({ "id": id }));
    } else {
        println!("Created {id}");
    }
    Ok(())
}

fn cmd_update(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let issue_id = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl update <issue-id> [flags]".into()))?
        .clone();

    let scope_vals = take_multi_flag_values(args, &["--scope"]);
    let labels_vals = take_multi_flag_values(args, &["-l", "--label"]);
    let rev = update_issue_in(
        &loc.store_dir,
        &UpdateIssueRequest {
            workspace_path: loc.worktree_display,
            issue_id: issue_id.clone(),
            title: take_flag_value(args, &["--title"]),
            status: take_flag_value(args, &["--status", "-s"]),
            priority: take_flag_value(args, &["-p", "--priority"]).and_then(|s| s.parse().ok()),
            assignee: take_flag_value(args, &["--assignee"]),
            labels: if labels_vals.is_empty() {
                None
            } else {
                Some(labels_vals)
            },
            description: take_flag_value(args, &["-d", "--description"]),
            acceptance_criteria: take_flag_value(args, &["--acceptance"]),
            notes: take_flag_value(args, &["--notes"]),
            scope: if scope_vals.is_empty() {
                None
            } else {
                Some(scope_vals)
            },
            expected_rev: take_flag_value(args, &["--expected-rev"]).and_then(|s| s.parse().ok()),
        },
    )?;

    if opts.json {
        println!("{}", json!({ "id": issue_id, "rev": rev }));
    } else {
        println!("Updated {issue_id} (rev {rev})");
    }
    Ok(())
}

fn cmd_dep(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    if args.len() < 3 {
        return Err(CoreError::InvalidOperation(
            "usage: bl dep add|rm <blocked-id> <blocker-id>".into(),
        ));
    }
    let add = match args[0].as_str() {
        "add" => true,
        "rm" | "remove" => false,
        other => {
            return Err(CoreError::InvalidOperation(format!(
                "expected 'add' or 'rm', got '{other}'"
            )));
        }
    };
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    mutate_dependency_in(&loc.store_dir, &args[1], &args[2], add)?;
    if opts.json {
        println!("{}", json!({ "ok": true, "blocked": args[1], "blocker": args[2], "added": add }));
    } else {
        println!("Dependency updated: {} blocked by {} (added={add})", args[1], args[2]);
    }
    Ok(())
}

fn cmd_reopen(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let id = args
        .first()
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl reopen <issue-id>".into()))?;
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let rev = reopen_issue_in(&loc.store_dir, id)?;
    if opts.json {
        println!("{}", json!({ "id": id, "status": "open", "rev": rev }));
    } else {
        println!("Reopened {id} (rev {rev})");
    }
    Ok(())
}

fn cmd_comment(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    if args.len() < 2 {
        return Err(CoreError::InvalidOperation(
            "usage: bl comment <issue-id> <text>".into(),
        ));
    }
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    add_comment_in(&loc.store_dir, &args[0], &args[1..].join(" "))?;
    if opts.json {
        println!("{}", json!({ "ok": true, "issueId": args[0] }));
    } else {
        println!("Comment added to {}", args[0]);
    }
    Ok(())
}

fn cmd_claim(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let issue_id = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl claim <issue-id> [--scope <pat>] [--summary <text>]".into()))?;
    let loc = RepoLocator::locate(&opts.repo)?;
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
        .find(|i| i.id == *issue_id || i.short_id == *issue_id)
        .ok_or_else(|| CoreError::NotFound(format!("issue not found: {issue_id}")))?;

    let scope = {
        let s = take_multi_flag_values(args, &["--scope"]);
        if s.is_empty() {
            issue.scope.clone()
        } else {
            s
        }
    };
    let summary = take_flag_value(args, &["--summary", "-m"]).unwrap_or_else(|| "starting work".into());
    let thread_url = take_flag_value(args, &["--thread-url"]);

    let lease = upsert_claim_lease(
        &loc.runtime_dir,
        ClaimLeaseInput {
            session_id: opts.session.clone(),
            issue_id: issue.id.clone(),
            worktree_id: loc.worktree_id.clone(),
            worktree_path: loc.worktree_display.clone(),
            branch: loc.branch.clone(),
            harness: opts.harness.clone().unwrap_or_else(detect_harness_from_env),
            agent: opts.agent.clone().unwrap_or_else(detect_agent_from_env),
            summary,
            scope,
            thread_url,
            ttl_secs: None,
        },
    )?;

    if issue.status == "open" {
        let _ = update_issue_in(
            &loc.store_dir,
            &UpdateIssueRequest {
                workspace_path: loc.worktree_display,
                issue_id: issue.id.clone(),
                title: None,
                status: Some("in_progress".into()),
                priority: None,
                assignee: None,
                labels: None,
                description: None,
                acceptance_criteria: None,
                notes: None,
                scope: None,
                expected_rev: None,
            },
        );
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "lease": lease, "task": issue }))
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "Claimed {} (session: {}, expires: {})",
            issue.id, lease.session_id, lease.expires_at
        );
    }
    Ok(())
}

fn cmd_pulse(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    let state_raw = take_flag_value(args, &["--state", "-s"]).unwrap_or_else(|| "working".into());
    let state = parse_session_state(&state_raw)?;
    let sid = opts.session.clone().unwrap_or_else(generate_session_id);

    if state == SessionState::Released {
        release_session_lease(&loc.runtime_dir, &sid)?;
        if opts.json {
            println!("{}", json!({ "sessionId": sid, "state": "released" }));
        } else {
            println!("Released session {sid}");
        }
        return Ok(());
    }

    let scope_vals = take_multi_flag_values(args, &["--scope"]);
    let lease = upsert_pulse_lease(
        &loc.runtime_dir,
        PulseLeaseInput {
            session_id: sid,
            issue_id: take_flag_value(args, &["--issue"]),
            worktree_id: loc.worktree_id,
            worktree_path: loc.worktree_display,
            branch: loc.branch,
            harness: opts.harness.clone().unwrap_or_else(detect_harness_from_env),
            agent: opts.agent.clone().unwrap_or_else(detect_agent_from_env),
            state,
            summary: take_flag_value(args, &["--summary", "-m"]),
            waiting_reason: take_flag_value(args, &["--waiting-reason"]),
            scope: if scope_vals.is_empty() {
                None
            } else {
                Some(scope_vals)
            },
            thread_url: take_flag_value(args, &["--thread-url"]),
            ttl_secs: None,
        },
    )?;

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&lease)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "Pulse saved for {} (state={:?}, expires={})",
            lease.session_id, lease.state, lease.expires_at
        );
    }
    Ok(())
}

fn cmd_finish(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let issue_id = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .ok_or_else(|| CoreError::InvalidOperation("usage: bl finish <issue-id> --summary <text>".into()))?;
    let loc = RepoLocator::locate(&opts.repo)?;
    if !loc.has_store {
        return Err(CoreError::NotInitialized);
    }
    let summary = take_flag_value(args, &["--summary", "--reason", "-m"]).ok_or_else(|| {
        CoreError::InvalidOperation("bl finish requires --summary <text>".into())
    })?;
    let refs = take_multi_flag_values(args, &["--ref"]);
    let expected_rev = take_flag_value(args, &["--expected-rev"]).and_then(|s| s.parse().ok());
    let op_id = take_flag_value(args, &["--op", "--operation-id"]);

    let res = finish_issue_in(
        &loc.store_dir,
        issue_id,
        expected_rev,
        op_id.as_deref(),
        &summary,
        &refs,
        opts.session.as_deref(),
    )?;

    if let Some(sid) = &opts.session {
        let _ = release_session_lease(&loc.runtime_dir, sid);
    }

    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&res)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!("Finished {} (rev {})", res.id, res.rev);
        if !res.unblocked.is_empty() {
            println!("Newly unblocked:");
            for u in &res.unblocked {
                println!("  - {} {}", u.short_id, u.title);
            }
        }
    }
    Ok(())
}

fn cmd_radar(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let loc = RepoLocator::locate(&opts.repo)?;
    let scan_git = !args.iter().any(|a| a == "--no-git");
    let issues = if loc.has_store {
        read_workspace_snapshot_for_location(
            &loc,
            loc.worktree_display.clone(),
            "mod:",
            None,
            &[],
            &[],
        )?
        .issues
    } else {
        Vec::new()
    };

    let (radar, _) = compute_radar_snapshot(&loc, &issues, 1, scan_git, None);
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&radar)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "Radar [{}] — sessions: {}, worktrees: {}, conflicts: {}, attention: {}",
            loc.repo_name,
            radar.sessions.len(),
            radar.worktrees.len(),
            radar.conflicts.len(),
            radar.attention.len()
        );
        for a in &radar.attention {
            println!("  ! [{:?}] {}", a.kind, a.reason);
        }
        for c in &radar.conflicts {
            println!("  ~ [{:?}] {:?}", c.kind, c.evidence.paths);
        }
    }
    Ok(())
}

fn cmd_import(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let from = take_flag_value(args, &["--from", "-f"]).ok_or_else(|| {
        CoreError::InvalidOperation("usage: bl import --from <path-to-issues.jsonl> [--replace]".into())
    })?;
    let replace = args.iter().any(|a| a == "--replace");
    let report = import_jsonl_into_shared(&opts.repo, PathBuf::from(from).as_path(), replace)?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "Imported {} new / {} updated issues into {}",
            report.imported_count, report.updated_count, report.store_path
        );
        if !report.skipped_malformed_lines.is_empty() {
            println!("Skipped malformed lines: {:?}", report.skipped_malformed_lines);
        }
        if !report.dangling_dependencies.is_empty() {
            println!("Dangling deps: {:?}", report.dangling_dependencies);
        }
    }
    Ok(())
}

fn cmd_export(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let to = take_flag_value(args, &["--to", "-o"]).ok_or_else(|| {
        CoreError::InvalidOperation(
            "usage: bl export --to <path> [--format bd-v1|native] [--force]".into(),
        )
    })?;
    let format = take_flag_value(args, &["--format"]).unwrap_or_else(|| "bd-v1".into());
    let force = args.iter().any(|a| a == "--force");
    let report = export_jsonl_from_store(&opts.repo, PathBuf::from(to).as_path(), &format, force)?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!(
            "Exported {} issues ({}) to {}",
            report.exported_count, report.format, report.dest_path
        );
        for w in &report.warnings {
            println!("warning: {w}");
        }
    }
    Ok(())
}

fn cmd_doctor(opts: &GlobalOpts) -> CoreResult<()> {
    let report = run_doctor(&opts.repo, env!("CARGO_PKG_VERSION"))?;
    if opts.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| CoreError::Internal(e.to_string()))?
        );
    } else {
        println!("Beadline Doctor (bl v{})", report.version);
        println!("  Repo:        {} ({})", report.repo_name, report.repo_id);
        println!("  Worktree:    {} ({})", report.worktree_root, report.worktree_id);
        println!("  Branch:      {}", report.branch.as_deref().unwrap_or("(detached)"));
        println!("  Store:       {} ({:?}, {:?})", report.store_dir, report.store_kind, report.storage_backend);
        println!("  Runtime:     {}", report.runtime_dir);
        println!("  Issues:      {} | Active sessions: {}", report.issue_count, report.active_sessions);
        for w in &report.warnings {
            println!("  Warning:     {w}");
        }
    }
    Ok(())
}

fn cmd_hook(opts: &GlobalOpts, args: &[String]) -> CoreResult<()> {
    let harness = args
        .first()
        .filter(|s| !s.starts_with('-'))
        .map(String::as_str)
        .or(opts.harness.as_deref())
        .unwrap_or("auto");
    let event_override = take_flag_value(args, &["--event", "-e"]);
    hooks::handle_hook_from_reader(harness, event_override.as_deref(), &opts.repo, io::stdin().lock());
    Ok(())
}

fn cmd_mcp(opts: &GlobalOpts) -> CoreResult<()> {
    let mut server = McpServer::new(
        opts.repo.clone(),
        opts.harness.clone(),
        opts.agent.clone(),
        opts.session.clone(),
    )?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    server.serve_stdio(stdin.lock(), stdout.lock())
}

fn take_flag_value(args: &[String], names: &[&str]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if names.contains(&args[i].as_str()) {
            return args.get(i + 1).cloned();
        }
        i += 1;
    }
    None
}

fn take_multi_flag_values(args: &[String], names: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if names.contains(&args[i].as_str()) {
            if let Some(val) = args.get(i + 1) {
                for piece in val.split(',') {
                    let trimmed = piece.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                }
            }
            i += 2;
            continue;
        }
        i += 1;
    }
    out
}

fn print_help() {
    println!(
        "bl {} — Beadline ultra-fast task CLI & native MCP server\n\n\
         USAGE:\n  bl [--repo <path>] [--json] [--session <id>] <command> [args]\n\n\
         AGENT WORKFLOW:\n  \
           ready [--limit N] [--module M]      List unblocked ready tasks\n  \
           show <id>                           Show full task context, scope, blockers, rev\n  \
           claim <id> [--scope P] [--summary]  Claim task for current agent session\n  \
           pulse --state working|waiting_human|released [--summary S] [--waiting-reason R]\n  \
           finish <id> --summary S [--ref R]   Close task with delivery proof & show unblocked\n  \
           hook <harness> [--event E]          Ingest native lifecycle hook JSON from stdin (fail-open)\n  \
           mcp                                 Start native stdio MCP server (5 core tools)\n\n\
         MANAGEMENT & EXCHANGE:\n  \
           init [--prefix P]                   Initialize shared native store (<git-common-dir>/beadline)\n  \
           list [--status S] [--all]           List tasks\n  \
           create <title> [flags]              Create a task\n  \
           update <id> [flags]                 Update fields / scope / status\n  \
           dep add|rm <blocked> <blocker>      Manage dependency edges (cycle-checked)\n  \
           reopen <id>                         Reopen a closed task\n  \
           comment <id> <text>                 Append a comment\n  \
           radar [--no-git]                    Inspect live sessions, worktrees & conflicts\n  \
           import --from <file> [--replace]    Import .beads/issues.jsonl into shared store\n  \
           export --to <file> [--format bd-v1] Export tasks with drift check\n  \
           doctor                              Check repo locator, store & runtime health",
        env!("CARGO_PKG_VERSION")
    );
}
