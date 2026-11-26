use crate::config::store::normalize_display_path;
use crate::engine::bd_cli::{configure_no_window, find_in_path};
use crate::models::issue::WorktreeRef;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub fn read_beads_config_values(repo_root: &Path) -> (Option<String>, Option<String>) {
    let cfg_path = repo_root.join(".beads").join("config.yaml");
    let Ok(text) = std::fs::read_to_string(cfg_path) else {
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

pub async fn detect_github_repo(repo_root: &Path) -> Option<String> {
    let (_, from_cfg) = read_beads_config_values(repo_root);
    if from_cfg.is_some() {
        return from_cfg;
    }

    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;
    let mut cmd = Command::new(git_bin);
    cmd.args(["remote", "get-url", "origin"])
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    let out = timeout(Duration::from_secs(3), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    parse_github_slug(&url)
}

pub fn parse_github_slug(remote_url: &str) -> Option<String> {
    let trimmed = remote_url.trim().trim_end_matches(".git");
    if let Some(rest) = trimmed.strip_prefix("https://github.com/") {
        return validate_slug(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("http://github.com/") {
        return validate_slug(rest);
    }
    if let Some(rest) = trimmed.strip_prefix("git@github.com:") {
        return validate_slug(rest);
    }
    None
}

fn validate_slug(rest: &str) -> Option<String> {
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() == 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

pub async fn list_git_worktrees(repo_root: &Path) -> Vec<WorktreeRef> {
    let Some(git_bin) = find_in_path(if cfg!(windows) { "git.exe" } else { "git" }) else {
        return Vec::new();
    };
    let mut cmd = Command::new(git_bin);
    cmd.args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    let Ok(Ok(out)) = timeout(Duration::from_secs(3), cmd.output()).await else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut results = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_branch: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let (Some(p), Some(b)) = (cur_path.take(), cur_branch.take()) {
                results.push(WorktreeRef {
                    path: normalize_display_path(&p),
                    branch: b,
                });
            }
            cur_path = None;
            cur_branch = None;
            continue;
        }
        if let Some(p) = trimmed.strip_prefix("worktree ") {
            cur_path = Some(p.trim().to_string());
        } else if let Some(b) = trimmed.strip_prefix("branch refs/heads/") {
            cur_branch = Some(b.trim().to_string());
        }
    }
    if let (Some(p), Some(b)) = (cur_path, cur_branch) {
        results.push(WorktreeRef {
            path: normalize_display_path(&p),
            branch: b,
        });
    }
    results
}
