use crate::error::{CoreError, CoreResult};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Cross-process advisory/mandatory OS file lock backed by a persistent `.lock` file.
/// The lock file itself is never deleted or replaced; OS releases the lock automatically
/// if a process terminates unexpectedly.
pub struct FileLockGuard {
    _file: File,
}

impl FileLockGuard {
    pub fn acquire(lock_path: &Path, timeout: Duration) -> CoreResult<Self> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CoreError::Io(format!("failed to create lock dir: {e}")))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|e| {
                CoreError::Io(format!(
                    "failed to open lock file {}: {e}",
                    lock_path.display()
                ))
            })?;

        let deadline = Instant::now() + timeout;
        let mut sleep_ms = 5_u64;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) => {
                    if Instant::now() >= deadline {
                        return Err(CoreError::Busy(format!(
                            "timed out acquiring lock {}",
                            lock_path.display()
                        )));
                    }
                    thread::sleep(Duration::from_millis(sleep_ms));
                    sleep_ms = (sleep_ms * 2).min(50);
                }
                Err(TryLockError::Error(err)) => {
                    return Err(CoreError::Io(format!(
                        "failed to acquire OS lock {}: {err}",
                        lock_path.display()
                    )));
                }
            }
        }
    }
}

/// Safely writes `bytes` to `target` using a same-directory unique temporary file,
/// `sync_all()`, and atomic same-volume replacement.
///
/// On Windows, uses `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`
/// with bounded backoff on sharing violations, NEVER deleting `target` prior to commit.
pub fn atomic_write_bytes(target: &Path, bytes: &[u8]) -> CoreResult<()> {
    let parent = target
        .parent()
        .ok_or_else(|| CoreError::Internal("missing parent directory for atomic write".into()))?;
    fs::create_dir_all(parent)
        .map_err(|e| CoreError::Io(format!("failed to create parent directory: {e}")))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let base_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("beadline");
    let tmp_path = parent.join(format!(
        ".{base_name}.{}.{stamp}.{seq}.tmp",
        std::process::id()
    ));

    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .map_err(|e| {
                CoreError::Io(format!(
                    "failed to create temp file {}: {e}",
                    tmp_path.display()
                ))
            })?;
        file.write_all(bytes)
            .map_err(|e| CoreError::Io(format!("failed to write temp file: {e}")))?;
        file.sync_all()
            .map_err(|e| CoreError::Io(format!("failed to flush temp file: {e}")))?;
    }

    match replace_file_atomic(&tmp_path, target) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&tmp_path);
            Err(err)
        }
    }
}

#[cfg(windows)]
fn replace_file_atomic(tmp_path: &Path, target: &Path) -> CoreResult<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(
            lp_existing_file_name: *const u16,
            lp_new_file_name: *const u16,
            dw_flags: u32,
        ) -> i32;
    }

    let from_wide: Vec<u16> = tmp_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let to_wide: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let backoffs_ms = [10_u64, 25, 50, 100, 200];
    let mut last_err = None;
    for attempt in 0..=backoffs_ms.len() {
        let ok = unsafe {
            MoveFileExW(
                from_wide.as_ptr(),
                to_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok != 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        let code = err.raw_os_error().unwrap_or(0);
        let retryable = matches!(
            code,
            ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION
        );
        last_err = Some(err);
        if retryable && attempt < backoffs_ms.len() {
            thread::sleep(Duration::from_millis(backoffs_ms[attempt]));
            continue;
        }
        break;
    }

    let err = last_err.unwrap_or_else(|| std::io::Error::other("MoveFileExW failed"));
    let code = err.raw_os_error().unwrap_or(0);
    if matches!(
        code,
        ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION
    ) {
        Err(CoreError::Busy(format!(
            "target file {} is locked by another process: {err}",
            target.display()
        )))
    } else {
        Err(CoreError::Io(format!(
            "failed to replace {}: {err}",
            target.display()
        )))
    }
}

#[cfg(not(windows))]
fn replace_file_atomic(tmp_path: &Path, target: &Path) -> CoreResult<()> {
    fs::rename(tmp_path, target).map_err(|e| {
        CoreError::Io(format!(
            "failed to atomically replace {}: {e}",
            target.display()
        ))
    })
}

/// Strips Windows `\\?\` verbatim prefixes while preserving filesystem case semantics.
pub fn normalize_display_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let cleaned = if let Some(rest) = trimmed.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = trimmed.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        trimmed.to_string()
    };

    let pb = PathBuf::from(&cleaned);
    let canonical = fs::canonicalize(&pb).unwrap_or(pb);
    let text = canonical.to_string_lossy().to_string();
    let stripped = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    };

    if stripped.len() > 3 {
        stripped.trim_end_matches(['/', '\\']).to_string()
    } else {
        stripped
    }
}

/// Canonical comparison key that respects platform case-sensitivity.
pub fn canonical_path_key(path: &Path) -> String {
    let display = normalize_display_path(&path.to_string_lossy());
    #[cfg(windows)]
    {
        display.replace('\\', "/").to_lowercase()
    }
    #[cfg(not(windows))]
    {
        display
    }
}

pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn fnv1a64_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

pub fn configure_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

pub fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn atomic_write_replaces_existing_file_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("issues.jsonl");
        atomic_write_bytes(&target, b"v1\n").expect("write v1");
        assert_eq!(fs::read_to_string(&target).unwrap(), "v1\n");

        atomic_write_bytes(&target, b"v2-longer\n").expect("write v2");
        assert_eq!(fs::read_to_string(&target).unwrap(), "v2-longer\n");

        let leftover_tmps: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftover_tmps.is_empty(), "no temp files should remain");
    }

    #[test]
    fn file_lock_serializes_concurrent_writers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock_path = Arc::new(dir.path().join("issues.lock"));
        let counter_path = Arc::new(dir.path().join("counter.txt"));
        fs::write(&*counter_path, "0").unwrap();

        let mut handles = Vec::new();
        for _ in 0..8 {
            let lp = Arc::clone(&lock_path);
            let cp = Arc::clone(&counter_path);
            handles.push(thread::spawn(move || {
                for _ in 0..5 {
                    let _guard = FileLockGuard::acquire(&lp, Duration::from_secs(5)).expect("lock");
                    let cur: u32 = fs::read_to_string(&*cp).unwrap().trim().parse().unwrap();
                    atomic_write_bytes(&cp, format!("{}", cur + 1).as_bytes()).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(fs::read_to_string(&*counter_path).unwrap(), "40");
    }
}
