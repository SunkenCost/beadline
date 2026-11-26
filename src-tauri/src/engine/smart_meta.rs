use crate::models::issue::{SmartLink, SmartMeta, WorktreeRef};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

fn gh_ref_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:github\.com/[\w.-]+/[\w.-]+/issues/|gh-)(\d+)\b").expect("gh regex")
    })
}

fn branch_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:branch\s*:\s*|(?:origin/)?)((?:feat|fix|release|integration|chore|hotfix|refactor|docs)/[a-z0-9._/-]{2,80})",
        )
        .expect("branch regex")
    })
}

fn commit_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:(?:commit|base|sha)\s*[:=]\s*|\(([0-9a-f]{7,40})\)|→\s*)([0-9a-f]{7,40})?",
        )
        .expect("commit regex")
    })
}

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"([a-z][a-z0-9+.-]{1,20}://[^\s)\]>"'`]+)"#).expect("link regex")
    })
}

pub fn extract_smart_meta(
    external_ref: &str,
    description: &str,
    notes: &str,
    close_reason: &str,
    github_repo: Option<&str>,
    repo_worktrees: &[WorktreeRef],
    allowed_schemes: &[String],
) -> SmartMeta {
    let mut meta = SmartMeta::default();
    let combined = format!("{external_ref}\n{notes}\n{close_reason}\n{description}");

    // 1. GitHub Issue number & URL
    if let Some(cap) = gh_ref_re()
        .captures(external_ref)
        .or_else(|| gh_ref_re().captures(&combined))
    {
        if let Some(num_str) = cap.get(1) {
            if let Ok(num) = num_str.as_str().parse::<u32>() {
                meta.github_number = Some(num);
                if let Some(slug) = github_repo {
                    meta.github_url = Some(format!("https://github.com/{slug}/issues/{num}"));
                }
            }
        }
    }

    // 2. Git branches (deduplicated, order-preserving)
    let mut seen_branches = HashSet::new();
    for cap in branch_re().captures_iter(&combined) {
        if let Some(m) = cap.get(1) {
            let clean = m
                .as_str()
                .trim_start_matches("origin/")
                .trim_end_matches(&['.', ',', ';', ':', ')', ']', '`'][..])
                .to_string();
            if !clean.is_empty() && seen_branches.insert(clean.clone()) {
                meta.branches.push(clean);
            }
        }
    }

    // 3. Git commit SHAs (7-char short form, deduplicated)
    let mut seen_commits = HashSet::new();
    for cap in commit_re().captures_iter(&combined) {
        let sha_raw = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        if sha_raw.len() >= 7 && sha_raw.chars().any(|c| c.is_ascii_alphabetic()) {
            let short = sha_raw[..7].to_lowercase();
            if seen_commits.insert(short.clone()) {
                meta.commits.push(short);
            }
        }
    }

    // 4. Match local Git worktrees corresponding to detected branches
    let mut seen_wt = HashSet::new();
    for br in &meta.branches {
        for wt in repo_worktrees {
            if wt.branch == *br && seen_wt.insert(wt.path.clone()) {
                meta.worktrees.push(wt.clone());
            }
        }
    }

    // 5. Custom deep links (only schemes in user's configured allowlist)
    let scheme_set: HashSet<String> = allowed_schemes
        .iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    let mut seen_urls = HashSet::new();
    for cap in link_re().captures_iter(&combined) {
        if let Some(m) = cap.get(1) {
            let url = m
                .as_str()
                .trim_end_matches(&['.', ',', ';', ')', ']', '`'][..])
                .to_string();
            if let Some((scheme_raw, _)) = url.split_once("://") {
                let scheme = scheme_raw.to_lowercase();
                if scheme_set.contains(&scheme) && seen_urls.insert(url.clone()) {
                    let label = match scheme.as_str() {
                        "t3-thread" => "T3 Thread".to_string(),
                        "vscode" => "VS Code".to_string(),
                        "cursor" => "Cursor".to_string(),
                        other => other.to_string(),
                    };
                    meta.links.push(SmartLink { scheme, url, label });
                }
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_github_branches_commits_worktrees_and_deeplinks() {
        let worktrees = vec![WorktreeRef {
            path: "Y:\\worktrees\\rc".into(),
            branch: "release/v371p5-unified-rc".into(),
        }];
        let schemes = vec!["t3-thread".into(), "vscode".into()];
        let meta = extract_smart_meta(
            "gh-7",
            "Description text",
            "Branch: origin/release/v371p5-unified-rc | Commit: 7318baf9ae0d | Thread: [Review](t3-thread://v2/env/123) | bad: javascript://evil",
            "Merged (309afe3)",
            Some("example/demo-repo"),
            &worktrees,
            &schemes,
        );

        assert_eq!(meta.github_number, Some(7));
        assert_eq!(
            meta.github_url.as_deref(),
            Some("https://github.com/example/demo-repo/issues/7")
        );
        assert_eq!(meta.branches, vec!["release/v371p5-unified-rc"]);
        assert_eq!(meta.commits, vec!["7318baf", "309afe3"]);
        assert_eq!(meta.worktrees.len(), 1);
        assert_eq!(meta.worktrees[0].path, "Y:\\worktrees\\rc");
        assert_eq!(meta.links.len(), 1);
        assert_eq!(meta.links[0].scheme, "t3-thread");
        assert_eq!(meta.links[0].url, "t3-thread://v2/env/123");
    }
}
