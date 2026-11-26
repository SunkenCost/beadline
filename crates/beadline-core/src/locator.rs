use crate::error::{CoreError, CoreResult};
use crate::fs_util::{
    canonical_path_key, configure_no_window, find_in_path, fnv1a64_hex, normalize_display_path,
};
use crate::models::{StorageBackend, StoreKind};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Canonical identity and storage paths for a local repository/worktree.
///
/// Shared across `bl` CLI, `bl mcp`, and `beadline.exe` GUI so all worktrees of the same
/// Git repository always agree on one authoritative task store and one shared runtime directory.
#[derive(Debug, Clone)]
pub struct RepoLocation {
    pub repo_id: String,
    pub repo_name: String,
    pub worktree_root: PathBuf,
    pub worktree_display: String,
    pub worktree_id: String,
    pub git_dir: Option<PathBuf>,
    pub git_common_dir: Option<PathBuf>,
    pub main_worktree_root: Option<PathBuf>,
    pub branch: Option<String>,
    pub head_oid: Option<String>,
    /// Authoritative task store directory (`<git-common-dir>/beadline` or legacy `.beads`).
    pub store_dir: PathBuf,
    /// Shared per-session pulse directory (`<git-common-dir>/beadline/runtime/pulses` or `.beads/runtime/pulses`).
    pub runtime_dir: PathBuf,
    pub store_kind: StoreKind,
    pub storage_backend: StorageBackend,
    pub has_store: bool,
}

impl RepoLocation {
    pub fn issues_jsonl_path(&self) -> PathBuf {
        self.store_dir.join("issues.jsonl")
    }

    pub fn issues_lock_path(&self) -> PathBuf {
        self.store_dir.join("issues.lock")
    }

    pub fn config_yaml_path(&self) -> PathBuf {
        self.store_dir.join("config.yaml")
    }

    pub fn metadata_json_path(&self) -> PathBuf {
        self.store_dir.join("metadata.json")
    }

    /// The canonical shared v2 directory for this repo (`<git-common-dir>/beadline`,
    /// or `<worktree_root>/.beads` if outside Git).
    pub fn shared_v2_dir(&self) -> PathBuf {
        match &self.git_common_dir {
            Some(common) => common.join("beadline"),
            None => self.worktree_root.join(".beads"),
        }
    }
}

pub struct RepoLocator;

impl RepoLocator {
    /// Resolves canonical Git + Beadline storage locations from any directory inside a repo
    /// (including linked worktrees, subdirectories, or plain non-Git folders).
    pub fn locate(start_path: &Path) -> CoreResult<RepoLocation> {
        if !start_path.exists() {
            return Err(CoreError::WorkspaceNotFound(
                start_path.to_string_lossy().to_string(),
            ));
        }

        let start_dir = if start_path.is_dir() {
            start_path.to_path_buf()
        } else {
            start_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| start_path.to_path_buf())
        };

        if let Some(git_info) = probe_git_context(&start_dir) {
            let worktree_display = normalize_display_path(&git_info.worktree_root.to_string_lossy());
            let worktree_root = PathBuf::from(&worktree_display);
            let common_key = canonical_path_key(&git_info.git_common_dir);
            let repo_id = format!("repo_{}", fnv1a64_hex(common_key.as_bytes()));
            let wt_key = canonical_path_key(&git_info.git_dir);
            let worktree_id = format!("wt_{}", fnv1a64_hex(wt_key.as_bytes()));

            let main_worktree_root = derive_main_worktree(&git_info.git_common_dir);
            let repo_name = main_worktree_root
                .as_ref()
                .unwrap_or(&worktree_root)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string();

            let shared_dir = git_info.git_common_dir.join("beadline");
            let runtime_dir = shared_dir.join("runtime").join("pulses");

            let (store_dir, store_kind, storage_backend, has_store) =
                resolve_git_store_dir(&shared_dir, main_worktree_root.as_deref(), &worktree_root);

            return Ok(RepoLocation {
                repo_id,
                repo_name,
                worktree_root,
                worktree_display,
                worktree_id,
                git_dir: Some(git_info.git_dir),
                git_common_dir: Some(git_info.git_common_dir),
                main_worktree_root,
                branch: git_info.branch,
                head_oid: git_info.head_oid,
                store_dir,
                runtime_dir,
                store_kind,
                storage_backend,
                has_store,
            });
        }

        // Non-Git directory fallback
        let worktree_display = normalize_display_path(&start_dir.to_string_lossy());
        let worktree_root = PathBuf::from(&worktree_display);
        let key = canonical_path_key(&worktree_root);
        let repo_id = format!("dir_{}", fnv1a64_hex(key.as_bytes()));
        let worktree_id = format!("wt_{}", fnv1a64_hex(key.as_bytes()));
        let repo_name = worktree_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string();

        let store_dir = worktree_root.join(".beads");
        let runtime_dir = store_dir.join("runtime").join("pulses");
        let has_store = store_dir.is_dir();
        let storage_backend = detect_storage_backend_in_dir(&store_dir);

        Ok(RepoLocation {
            repo_id,
            repo_name,
            worktree_root,
            worktree_display,
            worktree_id,
            git_dir: None,
            git_common_dir: None,
            main_worktree_root: None,
            branch: None,
            head_oid: None,
            store_dir,
            runtime_dir,
            store_kind: StoreKind::LocalBeadsDir,
            storage_backend,
            has_store,
        })
    }
}

pub fn detect_storage_backend_in_dir(store_dir: &Path) -> StorageBackend {
    if !store_dir.is_dir() {
        return StorageBackend::NativeJsonl;
    }

    if store_dir.join("embeddeddolt").is_dir() || store_dir.join("dolt").is_dir() {
        return StorageBackend::DoltCli;
    }

    let meta_path = store_dir.join("metadata.json");
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

/// Checks whether a workspace directory has either a shared `<git-common-dir>/beadline` store
/// or a `.beads` directory. Fast path avoids spawning git if `.beads` is already present.
pub fn has_any_store(root: &Path) -> bool {
    if root.join(".beads").is_dir() {
        return true;
    }
    if root.join(".git").exists() {
        if let Ok(loc) = RepoLocator::locate(root) {
            return loc.has_store;
        }
    }
    false
}

fn resolve_git_store_dir(
    shared_dir: &Path,
    main_worktree_root: Option<&Path>,
    worktree_root: &Path,
) -> (PathBuf, StoreKind, StorageBackend, bool) {
    // 1. Authoritative v2 shared store inside <git-common-dir>/beadline
    if shared_dir.join("issues.jsonl").is_file() || shared_dir.join("metadata.json").is_file() {
        return (
            shared_dir.to_path_buf(),
            StoreKind::SharedGitCommon,
            StorageBackend::NativeJsonl,
            true,
        );
    }

    // 2. Legacy `.beads/` in the current worktree or main worktree (readable out of the box)
    let wt_beads = worktree_root.join(".beads");
    if wt_beads.is_dir() {
        let backend = detect_storage_backend_in_dir(&wt_beads);
        return (wt_beads, StoreKind::LegacyBeadsDir, backend, true);
    }
    if let Some(main_root) = main_worktree_root {
        let main_beads = main_root.join(".beads");
        if main_beads.is_dir() {
            let backend = detect_storage_backend_in_dir(&main_beads);
            return (main_beads, StoreKind::LegacyBeadsDir, backend, true);
        }
    }

    // 3. Uninitialized Git repo -> defaults to shared v2 store when initialized
    (
        shared_dir.to_path_buf(),
        StoreKind::SharedGitCommon,
        StorageBackend::NativeJsonl,
        false,
    )
}

fn derive_main_worktree(git_common_dir: &Path) -> Option<PathBuf> {
    if git_common_dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(".git"))
    {
        return git_common_dir.parent().map(|p| {
            PathBuf::from(normalize_display_path(&p.to_string_lossy()))
        });
    }
    None
}

struct GitContextProbe {
    worktree_root: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
    branch: Option<String>,
    head_oid: Option<String>,
}

fn probe_git_context(start_dir: &Path) -> Option<GitContextProbe> {
    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;
    let mut cmd = Command::new(git_bin);
    cmd.args([
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-dir",
        "--git-common-dir",
        "--abbrev-ref",
        "HEAD",
    ])
    .current_dir(start_dir)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    configure_no_window(&mut cmd);

    let out = cmd.output().ok()?;
    if !out.status.success() {
        // Might be an unborn branch (fresh `git init` with no commits yet): retry without HEAD
        return probe_git_context_unborn(start_dir);
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let toplevel = lines.next()?.trim();
    let git_dir = lines.next()?.trim();
    let common_dir = lines.next()?.trim();
    let abbrev = lines.next().map(str::trim).unwrap_or("");

    let branch = if abbrev.is_empty() || abbrev == "HEAD" {
        None
    } else {
        Some(abbrev.to_string())
    };

    let head_oid = read_short_head(start_dir);

    Some(GitContextProbe {
        worktree_root: PathBuf::from(normalize_display_path(toplevel)),
        git_dir: PathBuf::from(normalize_display_path(git_dir)),
        git_common_dir: PathBuf::from(normalize_display_path(common_dir)),
        branch,
        head_oid,
    })
}

fn probe_git_context_unborn(start_dir: &Path) -> Option<GitContextProbe> {
    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;
    let mut cmd = Command::new(git_bin);
    cmd.args([
        "rev-parse",
        "--path-format=absolute",
        "--show-toplevel",
        "--git-dir",
        "--git-common-dir",
    ])
    .current_dir(start_dir)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    configure_no_window(&mut cmd);

    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let toplevel = lines.next()?.trim();
    let git_dir = lines.next()?.trim();
    let common_dir = lines.next()?.trim();

    Some(GitContextProbe {
        worktree_root: PathBuf::from(normalize_display_path(toplevel)),
        git_dir: PathBuf::from(normalize_display_path(git_dir)),
        git_common_dir: PathBuf::from(normalize_display_path(common_dir)),
        branch: read_symbolic_branch(start_dir),
        head_oid: None,
    })
}

fn read_symbolic_branch(dir: &Path) -> Option<String> {
    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;
    let mut cmd = Command::new(git_bin);
    cmd.args(["symbolic-ref", "--short", "HEAD"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_no_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn read_short_head(dir: &Path) -> Option<String> {
    let git_bin = find_in_path(if cfg!(windows) { "git.exe" } else { "git" })?;
    let mut cmd = Command::new(git_bin);
    cmd.args(["rev-parse", "--short=8", "HEAD"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_no_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_git(dir: &Path, args: &[&str]) {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com");
        configure_no_window(&mut cmd);
        let out = cmd.output().expect("git command");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn resolves_same_shared_store_across_multiple_git_worktrees() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main_repo = tmp.path().join("main-repo");
        let wt_a = tmp.path().join("wt-a");
        let wt_b = tmp.path().join("wt-b");
        fs::create_dir_all(&main_repo).unwrap();

        run_git(&main_repo, &["init", "-b", "main"]);
        fs::write(main_repo.join("README.md"), "# test\n").unwrap();
        run_git(&main_repo, &["add", "README.md"]);
        run_git(&main_repo, &["commit", "-m", "init"]);
        run_git(
            &main_repo,
            &["worktree", "add", "-b", "feat/a", wt_a.to_str().unwrap()],
        );
        run_git(
            &main_repo,
            &["worktree", "add", "-b", "feat/b", wt_b.to_str().unwrap()],
        );

        let loc_main = RepoLocator::locate(&main_repo).expect("main");
        let loc_a = RepoLocator::locate(&wt_a).expect("wt_a");
        let loc_b = RepoLocator::locate(&wt_b).expect("wt_b");

        assert_eq!(loc_main.repo_id, loc_a.repo_id);
        assert_eq!(loc_main.repo_id, loc_b.repo_id);
        assert_eq!(loc_main.store_dir, loc_a.store_dir);
        assert_eq!(loc_main.store_dir, loc_b.store_dir);
        assert_eq!(loc_main.runtime_dir, loc_a.runtime_dir);
        assert_eq!(loc_main.runtime_dir, loc_b.runtime_dir);
        assert_ne!(loc_main.worktree_id, loc_a.worktree_id);
        assert_ne!(loc_a.worktree_id, loc_b.worktree_id);
        assert_eq!(loc_a.branch.as_deref(), Some("feat/a"));
        assert_eq!(loc_b.branch.as_deref(), Some("feat/b"));
    }
}
