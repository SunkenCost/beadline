use crate::engine::bd_cli::{configure_no_window, find_in_path};
use crate::error::{AppError, AppResult};
use crate::models::issue::ManagedBdStatus;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const BEADS_RELEASES_BASE: &str = "https://github.com/steveyegge/beads/releases/latest/download";

pub fn managed_bd_binary_path(config_dir: &Path) -> PathBuf {
    let name = if cfg!(windows) { "bd.exe" } else { "bd" };
    config_dir.join("bin").join(name)
}

fn expected_asset_suffix() -> AppResult<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("_windows_amd64.zip"),
        ("windows", "aarch64") => Ok("_windows_arm64.zip"),
        ("macos", "x86_64") => Ok("_darwin_amd64.tar.gz"),
        ("macos", "aarch64") => Ok("_darwin_arm64.tar.gz"),
        ("linux", "x86_64") => Ok("_linux_amd64.tar.gz"),
        ("linux", "aarch64") => Ok("_linux_arm64.tar.gz"),
        (os, arch) => Err(AppError::InstallFailed(format!(
            "unsupported platform for managed bd download: {os}/{arch}"
        ))),
    }
}

/// Downloads the latest official `steveyegge/beads` release binary into Beadline's
/// app config directory (`<config_dir>/bin/bd[.exe]`), verifies its SHA-256 digest
/// against `checksums.txt`, and extracts it without touching system PATH.
pub async fn install_managed_bd(config_dir: &Path) -> AppResult<ManagedBdStatus> {
    let suffix = expected_asset_suffix()?;
    let curl_bin = find_in_path(if cfg!(windows) { "curl.exe" } else { "curl" })
        .ok_or_else(|| AppError::InstallFailed("curl is not available on this system".into()))?;
    let tar_bin = find_in_path(if cfg!(windows) { "tar.exe" } else { "tar" })
        .ok_or_else(|| AppError::InstallFailed("tar is not available on this system".into()))?;

    let bin_dir = config_dir.join("bin");
    let stage_dir = bin_dir.join(format!(".bd-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&stage_dir);
    fs::create_dir_all(&stage_dir)
        .map_err(|e| AppError::InstallFailed(format!("failed to create staging dir: {e}")))?;

    let result = install_inner(&curl_bin, &tar_bin, &bin_dir, &stage_dir, suffix).await;
    let _ = fs::remove_dir_all(&stage_dir);
    result
}

async fn install_inner(
    curl_bin: &Path,
    tar_bin: &Path,
    bin_dir: &Path,
    stage_dir: &Path,
    suffix: &str,
) -> AppResult<ManagedBdStatus> {
    // 1. Fetch official checksums.txt to discover exact latest archive name + SHA-256
    let checksums_path = stage_dir.join("checksums.txt");
    let checksums_url = format!("{BEADS_RELEASES_BASE}/checksums.txt");
    run_tool(
        curl_bin,
        &[
            "-fsSL",
            "--retry",
            "2",
            "-o",
            &checksums_path.to_string_lossy(),
            &checksums_url,
        ],
        45,
    )
    .await?;

    let checksums_raw = fs::read_to_string(&checksums_path)
        .map_err(|e| AppError::InstallFailed(format!("failed to read checksums.txt: {e}")))?;
    let (expected_sha, asset_name) =
        parse_checksum_entry(&checksums_raw, suffix).ok_or_else(|| {
            AppError::InstallFailed(format!(
                "no matching release asset ending in {suffix} found in checksums.txt"
            ))
        })?;

    // 2. Download the archive
    let archive_path = stage_dir.join(&asset_name);
    let archive_url = format!("{BEADS_RELEASES_BASE}/{asset_name}");
    run_tool(
        curl_bin,
        &[
            "-fsSL",
            "--retry",
            "2",
            "-o",
            &archive_path.to_string_lossy(),
            &archive_url,
        ],
        240,
    )
    .await?;

    // 3. Verify SHA-256 digest in pure Rust before extracting
    let bytes = fs::read(&archive_path)
        .map_err(|e| AppError::InstallFailed(format!("failed to read downloaded archive: {e}")))?;
    let actual_sha = sha256_hex(&bytes);
    if !actual_sha.eq_ignore_ascii_case(&expected_sha) {
        return Err(AppError::InstallFailed(format!(
            "SHA-256 verification failed for {asset_name} (expected {expected_sha}, got {actual_sha})"
        )));
    }

    // 4. Extract archive and locate bd binary
    let extract_dir = stage_dir.join("extracted");
    fs::create_dir_all(&extract_dir)
        .map_err(|e| AppError::InstallFailed(format!("failed to create extract dir: {e}")))?;
    run_tool(
        tar_bin,
        &[
            "-xf",
            &archive_path.to_string_lossy(),
            "-C",
            &extract_dir.to_string_lossy(),
        ],
        60,
    )
    .await?;

    let exe_name = if cfg!(windows) { "bd.exe" } else { "bd" };
    let extracted_bin = find_file_recursive(&extract_dir, exe_name).ok_or_else(|| {
        AppError::InstallFailed(format!("{exe_name} not found inside {asset_name}"))
    })?;

    fs::create_dir_all(bin_dir)
        .map_err(|e| AppError::InstallFailed(format!("failed to create bin dir: {e}")))?;
    let target_path = bin_dir.join(exe_name);
    if target_path.exists() {
        let _ = fs::remove_file(&target_path);
    }
    fs::rename(&extracted_bin, &target_path)
        .or_else(|_| fs::copy(&extracted_bin, &target_path).map(|_| ()))
        .map_err(|e| AppError::InstallFailed(format!("failed to place {exe_name}: {e}")))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755));
    }

    let version = probe_bd_version(&target_path).await;
    Ok(ManagedBdStatus {
        installed: true,
        path: Some(target_path.to_string_lossy().to_string()),
        source: Some("managed".to_string()),
        version,
    })
}

pub async fn probe_bd_version(bd_bin: &Path) -> Option<String> {
    let mut cmd = Command::new(bd_bin);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    let out = timeout(Duration::from_secs(5), cmd.output())
        .await
        .ok()?
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    if line.is_empty() {
        None
    } else {
        Some(line)
    }
}

async fn run_tool(bin: &Path, args: &[&str], timeout_secs: u64) -> AppResult<()> {
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_no_window(&mut cmd);

    let out = timeout(Duration::from_secs(timeout_secs), cmd.output())
        .await
        .map_err(|_| AppError::InstallFailed(format!("download timed out after {timeout_secs}s")))?
        .map_err(|e| AppError::InstallFailed(format!("failed to execute helper: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(AppError::InstallFailed(if stderr.is_empty() {
            format!("tool exited with status {:?}", out.status.code())
        } else {
            stderr
        }));
    }
    Ok(())
}

fn parse_checksum_entry(raw: &str, suffix: &str) -> Option<(String, String)> {
    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let sha = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if sha.len() == 64 && name.starts_with("beads_") && name.ends_with(suffix) {
            return Some((sha.to_string(), name.to_string()));
        }
    }
    None
}

fn find_file_recursive(dir: &Path, target_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target_name) {
                return Some(found);
            }
        } else if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.eq_ignore_ascii_case(target_name))
        {
            return Some(path);
        }
    }
    None
}

/// Pure-Rust FIPS 180-4 SHA-256 (zero external crate bloat).
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity((data.len() + 9 + 63) & !63);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut hex = String::with_capacity(64);
    for word in h {
        hex.push_str(&format!("{word:08x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_nist_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parses_official_checksums_entry() {
        let sample = "\
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  beads_1.3.4_linux_amd64.tar.gz\n\
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  beads_1.3.4_windows_amd64.zip\n";
        let (sha, name) =
            parse_checksum_entry(sample, "_windows_amd64.zip").expect("windows entry");
        assert_eq!(
            sha,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(name, "beads_1.3.4_windows_amd64.zip");
    }
}
