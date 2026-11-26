use crate::engine::bd_cli::{configure_no_window, find_in_path};
use crate::models::issue::WorktreeRef;
use beadline_core::git_radar::list_worktrees_porcelain_z;
use beadline_core::jsonl_reader::read_beads_config_values as read_config_in_store;
use beadline_core::locator::RepoLocator;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub fn read_beads_config_values(repo_root: &Path) -> (Option<String>, Option<String>) {
    if let Ok(loc) = RepoLocator::locate(repo_root) {
        let vals = read_config_in_store(&loc.store_dir);
        if vals.0.is_some() || vals.1.is_some() {
            return vals;
        }
    }
    read_config_in_store(&repo_root.join(".beads"))
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
    let root = repo_root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        list_worktrees_porcelain_z(&root)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|wt| {
                let branch = wt.branch?;
                Some(WorktreeRef {
                    path: wt.path,
                    branch,
                })
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
