use crate::error::{CoreError, CoreResult};

/// Normalizes and validates a Phase 1 repo-relative write scope pattern.
///
/// Supported syntax:
/// - Exact repo-relative file path: `src/auth/jwt.rs`
/// - Recursive directory prefix: `src/auth/**`
///
/// Rejects absolute paths, drive letters, `..` traversal, empty segments, and unsupported wildcards.
pub fn normalize_scope_pattern(raw: &str) -> CoreResult<String> {
    let trimmed = raw.trim().replace('\\', "/");
    let stripped = trimmed.strip_prefix("./").unwrap_or(&trimmed);
    let cleaned = stripped.trim_end_matches('/');

    if cleaned.is_empty() {
        return Err(CoreError::InvalidOperation(
            "scope pattern cannot be empty".into(),
        ));
    }
    if cleaned.starts_with('/')
        || cleaned.starts_with('~')
        || (cleaned.len() >= 2 && cleaned.as_bytes()[1] == b':')
    {
        return Err(CoreError::InvalidOperation(format!(
            "scope pattern must be repo-relative: {raw}"
        )));
    }

    let is_glob_dir = cleaned.ends_with("/**");
    let base = if is_glob_dir {
        cleaned.strip_suffix("/**").unwrap_or("")
    } else {
        cleaned
    };

    if base.is_empty() {
        return Err(CoreError::InvalidOperation(format!(
            "scope pattern cannot target entire repository: {raw}"
        )));
    }

    let mut segments = Vec::new();
    for seg in base.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err(CoreError::InvalidOperation(format!(
                "scope pattern cannot contain '..': {raw}"
            )));
        }
        if seg.contains('*') || seg.contains('?') {
            return Err(CoreError::InvalidOperation(format!(
                "scope pattern only supports exact paths or trailing '/**': {raw}"
            )));
        }
        segments.push(seg);
    }

    if segments.is_empty() {
        return Err(CoreError::InvalidOperation(format!(
            "invalid scope pattern: {raw}"
        )));
    }

    let joined = segments.join("/");
    if is_glob_dir {
        Ok(format!("{joined}/**"))
    } else {
        Ok(joined)
    }
}

pub fn normalize_scope_list(raw: &[String]) -> CoreResult<Vec<String>> {
    let mut out = Vec::new();
    for item in raw {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        let norm = normalize_scope_pattern(trimmed)?;
        if !out.contains(&norm) {
            out.push(norm);
        }
    }
    Ok(out)
}

/// Normalizes a repo-relative file path (or collapsed untracked directory ending in `/`)
/// into POSIX slash format without changing its case.
pub fn normalize_rel_path(raw: &str) -> String {
    let replaced = raw.trim().replace('\\', "/");
    let stripped = replaced.strip_prefix("./").unwrap_or(&replaced);
    stripped.to_string()
}

/// Segment-aware match between a repo-relative dirty path (or collapsed `dir/` untracked prefix)
/// and a normalized scope pattern (`exact/file` or `dir/**`).
///
/// Guarantees `src/api/**` NEVER matches `src/api-old/foo.rs`.
pub fn path_matches_scope(rel_path: &str, scope_pattern: &str) -> bool {
    let path = normalize_rel_path(rel_path);
    let Ok(pattern) = normalize_scope_pattern(scope_pattern) else {
        return false;
    };

    let path_is_collapsed_dir = path.ends_with('/');
    let clean_path = path.trim_end_matches('/');

    if let Some(dir_prefix) = pattern.strip_suffix("/**") {
        // Exact directory match (e.g. collapsed `src/api/`) or child `src/api/...`
        if clean_path == dir_prefix || clean_path.starts_with(&format!("{dir_prefix}/")) {
            return true;
        }
        // Collapsed untracked ancestor dir (`src/` could contain `src/api/**`)
        if path_is_collapsed_dir && dir_prefix.starts_with(&format!("{clean_path}/")) {
            return true;
        }
        false
    } else {
        if clean_path == pattern {
            return true;
        }
        // Collapsed untracked dir `src/auth/` could contain exact target `src/auth/jwt.rs`
        if path_is_collapsed_dir && pattern.starts_with(&format!("{clean_path}/")) {
            return true;
        }
        false
    }
}

/// Returns true if two normalized scope patterns can match at least one common file.
pub fn scopes_overlap(a: &str, b: &str) -> bool {
    let Ok(na) = normalize_scope_pattern(a) else {
        return false;
    };
    let Ok(nb) = normalize_scope_pattern(b) else {
        return false;
    };

    match (na.strip_suffix("/**"), nb.strip_suffix("/**")) {
        (Some(da), Some(db)) => {
            da == db || da.starts_with(&format!("{db}/")) || db.starts_with(&format!("{da}/"))
        }
        (Some(da), None) => nb == da || nb.starts_with(&format!("{da}/")),
        (None, Some(db)) => na == db || na.starts_with(&format!("{db}/")),
        (None, None) => na == nb,
    }
}

/// Identifies common package-manager lockfiles so they remain reported as real overlaps
/// while allowing the UI to visually group or de-emphasize lockfile-only collisions.
pub fn is_lockfile_path(rel_path: &str) -> bool {
    let norm = normalize_rel_path(rel_path);
    let file_name = norm.rsplit('/').next().unwrap_or(&norm);
    matches!(
        file_name,
        "Cargo.lock"
            | "pnpm-lock.yaml"
            | "package-lock.json"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb"
            | "poetry.lock"
            | "uv.lock"
            | "go.sum"
            | "Gemfile.lock"
            | "composer.lock"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_rejects_unsafe_scopes() {
        assert_eq!(
            normalize_scope_pattern("src\\auth\\**").unwrap(),
            "src/auth/**"
        );
        assert_eq!(
            normalize_scope_pattern("./Cargo.toml").unwrap(),
            "Cargo.toml"
        );
        assert!(normalize_scope_pattern("/etc/passwd").is_err());
        assert!(normalize_scope_pattern("C:\\repo\\a.rs").is_err());
        assert!(normalize_scope_pattern("../outside.rs").is_err());
        assert!(normalize_scope_pattern("**").is_err());
        assert!(normalize_scope_pattern("src/*.rs").is_err());
    }

    #[test]
    fn segment_aware_scope_matching_prevents_prefix_false_positives() {
        assert!(path_matches_scope("src/api/routes.rs", "src/api/**"));
        assert!(path_matches_scope("src/api/v2/users.rs", "src/api/**"));
        assert!(!path_matches_scope("src/api-old/routes.rs", "src/api/**"));
        assert!(!path_matches_scope("src/api2/routes.rs", "src/api/**"));
        assert!(path_matches_scope("src/api/", "src/api/**"));
        assert!(path_matches_scope("Cargo.toml", "Cargo.toml"));
        assert!(!path_matches_scope("crates/foo/Cargo.toml", "Cargo.toml"));
    }

    #[test]
    fn detects_scope_overlaps_accurately() {
        assert!(scopes_overlap("src/auth/**", "src/auth/jwt.rs"));
        assert!(scopes_overlap("src/auth/**", "src/auth/oauth/**"));
        assert!(!scopes_overlap("src/api/**", "src/api-old/**"));
        assert!(!scopes_overlap("src/a.rs", "src/b.rs"));
    }
}
