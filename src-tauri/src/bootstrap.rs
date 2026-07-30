//! FFmpeg bootstrap.
//!
//! Downloads a static FFmpeg build from BtbN/FFmpeg-Builds and extracts
//! it under `%LOCALAPPDATA%\Offspring\ffmpeg\`. Runs on a background
//! thread, emitting `ffmpeg-download` progress events.
//!
//! Sole path for the FFmpeg fetch — the installer used to do this via a
//! `download_ffmpeg.ps1` script at install time, but that was removed in
//! 0.4.4 to keep all outbound network calls explicitly user-initiated
//! (per the SECURITY.md "no automatic outbound" promise) and to drop one
//! more PowerShell-script-drop signature from sandbox scanners.

// HTTP path is the entire module; only the studio stub at the bottom
// stays compiled in the studio build. Imports gated accordingly so
// `cargo build --features studio` doesn't trip "unused import"
// warnings.
#[cfg(not(feature = "studio"))]
use anyhow::{anyhow, bail, Context, Result};
#[cfg(not(feature = "studio"))]
use serde::Serialize;
#[cfg(not(feature = "studio"))]
use sha2::{Digest, Sha256};
#[cfg(not(feature = "studio"))]
use std::io::{Read, Write};
#[cfg(not(feature = "studio"))]
use std::path::PathBuf;
#[cfg(not(feature = "studio"))]
use std::time::Duration;
use tauri::AppHandle;
#[cfg(not(feature = "studio"))]
use tauri::Emitter;

#[cfg(not(feature = "studio"))]
use crate::paths;

// Source URLs are split per-platform because the canonical "static
// build" provider is different on each OS. On Windows we use
// BtbN/FFmpeg-Builds (bundles ffmpeg.exe + ffprobe.exe + presets in one
// zip). On macOS we use evermeet.cx, which ships each binary as a
// standalone zip and exposes a JSON `info/{tool}/release` endpoint we
// can hit to grab the official SHA-256 — same verification strength as
// the Windows side, just a different sidecar shape.
//
// The Windows build must be the `gpl` variant, not `lgpl`: libx264 and
// libx265 are GPL-only, and every video preset in this app encodes with
// one of them. The `lgpl` variant simply has no H.264/HEVC encoder.
//
// Why not gyan.dev's build, which we used through 0.5.x: its
// "essentials" archive ships without libdav1d, leaving libaom-av1 as
// the only AV1 decoder. libaom rejects any sequence header carrying a
// reserved `seq_level_idx` — which real-world encoders do emit (Houdini
// 21 writes level 7.3) — and then fails every subsequent frame with
// "No sequence header" until the decode error rate aborts the job.
// dav1d ignores the level field and decodes those files fine. gyan's
// `full` build does include dav1d, but it's published as 7z only and
// we'd need a whole 7z decoder to read it.
///
/// NOTHING HERE PINS A FILENAME. BtbN's `latest` release is a rolling
/// tag: when upstream moves from n8.1 to n8.2, the old
/// `ffmpeg-n8.1-latest-win64-gpl-8.1.zip` asset stops being published
/// and a hardcoded URL starts 404ing for every user at once — an app
/// that silently loses its ability to install FFmpeg until someone
/// notices and ships a new build. So we read the release's
/// `checksums.sha256` manifest (a stable filename) and *derive* both
/// the asset name and its hash from it. New upstream version, same
/// code path, no release required.
#[cfg(all(windows, not(feature = "studio")))]
const FFMPEG_RELEASE_BASE: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";
/// BtbN publishes one combined `checksums.sha256` manifest per release
/// covering every asset. We fetch it BEFORE downloading anything, pick
/// our asset out of it, then verify the downloaded ZIP against the hash
/// on that same line and refuse to extract on mismatch. Doesn't fully
/// neutralise an attacker who can MITM GitHub's TLS (they could swap
/// both the manifest and the ZIP), but it does defeat
/// partial-compromise / cache-poisoning scenarios where one URL gets
/// tampered with and not the other, and it catches transport- or
/// storage-level corruption.
#[cfg(all(windows, not(feature = "studio")))]
const FFMPEG_SHA256_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/checksums.sha256";

/// evermeet.cx info JSON endpoints. The response includes a versioned
/// download URL and an official SHA-256 we can verify against. We hit
/// these for ffmpeg + ffprobe separately (evermeet ships one binary
/// per zip, not a bundle).
#[cfg(all(target_os = "macos", not(feature = "studio")))]
const EVERMEET_FFMPEG_INFO_URL: &str = "https://evermeet.cx/ffmpeg/info/ffmpeg/release";
#[cfg(all(target_os = "macos", not(feature = "studio")))]
const EVERMEET_FFPROBE_INFO_URL: &str = "https://evermeet.cx/ffmpeg/info/ffprobe/release";

#[cfg(not(feature = "studio"))]
#[derive(Serialize, Clone, Debug)]
pub struct DownloadEvent {
    /// "downloading" | "extracting" | "done" | "error"
    pub phase: String,
    /// 0.0..=100.0 for downloading; None otherwise
    pub percent: Option<f32>,
    /// Optional human-readable status
    pub message: Option<String>,
}

#[cfg(not(feature = "studio"))]
fn emit(app: &AppHandle, ev: DownloadEvent) {
    let _ = app.emit("ffmpeg-download", ev);
}

/// Run the full download → extract → install pipeline on a background
/// thread. Returns immediately; observe progress via `ffmpeg-download`
/// events.
#[cfg(not(feature = "studio"))]
pub fn spawn_download(app: AppHandle) {
    std::thread::spawn(move || {
        let result = download_and_install(&app);
        match result {
            Ok(path) => emit(
                &app,
                DownloadEvent {
                    phase: "done".into(),
                    percent: Some(100.0),
                    message: Some(path.display().to_string()),
                },
            ),
            Err(e) => emit(
                &app,
                DownloadEvent {
                    phase: "error".into(),
                    percent: None,
                    message: Some(e.to_string()),
                },
            ),
        }
    });
}

/// Platform dispatcher. Each OS picks up its source binaries from a
/// different canonical provider, and the extraction logic is shaped
/// differently enough (one bundled archive on Windows vs. two
/// per-binary archives on macOS) that splitting the implementations
/// is cleaner than threading branches through one big function.
#[cfg(not(feature = "studio"))]
fn download_and_install(app: &AppHandle) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        return download_and_install_windows(app);
    }
    #[cfg(target_os = "macos")]
    {
        return download_and_install_macos(app);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = app;
        bail!("Automatic FFmpeg download isn't supported on this platform yet. Install ffmpeg manually and point Offspring at it via Settings.")
    }
}

#[cfg(all(windows, not(feature = "studio")))]
fn download_and_install_windows(app: &AppHandle) -> Result<PathBuf> {
    let target = paths::local_data_dir()?.join("ffmpeg");
    let bin_exe = target.join("bin").join("ffmpeg.exe");

    // Unique temp paths so parallel installs (unlikely, but possible) don't clash
    let uid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_zip = std::env::temp_dir().join(format!("ffmpeg-offspring-{uid}-{ts}.zip"));
    let tmp_extract = std::env::temp_dir().join(format!("ffmpeg-offspring-extract-{uid}-{ts}"));

    // --- 1. Resolve which build to fetch ----------------------------------
    // The manifest comes first, before a single byte of the ~160 MB ZIP:
    // it tells us both which asset the current release actually
    // publishes and what that asset's hash is. Failing here costs the
    // user a few hundred milliseconds instead of a long download that
    // turns out to 404 or mismatch at the end.
    emit(
        app,
        DownloadEvent {
            phase: "downloading".into(),
            percent: Some(0.0),
            message: Some("Connecting to github.com…".into()),
        },
    );
    let (asset, expected_hash) =
        fetch_win_asset().context("looking up the current FFmpeg build on github.com")?;
    let url = format!("{FFMPEG_RELEASE_BASE}/{asset}");

    // --- 1a. Download -----------------------------------------------------
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(120))
        .build();
    let resp = agent
        .get(&url)
        .call()
        .with_context(|| format!("downloading {asset}"))?;

    let total_len: Option<u64> = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok());

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&tmp_zip)
        .with_context(|| format!("creating temp file {}", tmp_zip.display()))?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    // Hash the bytes as they stream past so we don't have to re-read
    // the file from disk after the download finishes. SHA-256 is fast
    // enough (~500 MB/s on modern x86) that this never gates progress.
    let mut hasher = Sha256::new();

    loop {
        let n = reader.read(&mut buf).context("reading zip stream")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).context("writing temp zip")?;
        hasher.update(&buf[..n]);
        downloaded += n as u64;

        // Throttle progress emits so we don't flood the event bus.
        if last_emit.elapsed() >= Duration::from_millis(150) {
            last_emit = std::time::Instant::now();
            let (pct, msg) = match total_len {
                Some(total) if total > 0 => {
                    let p = (downloaded as f64 / total as f64 * 100.0).min(100.0) as f32;
                    (
                        Some(p),
                        Some(format!(
                            "{:.1} of {:.1} MB",
                            downloaded as f64 / 1_048_576.0,
                            total as f64 / 1_048_576.0
                        )),
                    )
                }
                _ => (
                    None,
                    Some(format!("{:.1} MB", downloaded as f64 / 1_048_576.0)),
                ),
            };
            emit(
                app,
                DownloadEvent {
                    phase: "downloading".into(),
                    percent: pct,
                    message: msg,
                },
            );
        }
    }
    drop(file);
    let computed_hash = hex_lower(&hasher.finalize());

    // --- 1b. Verify against the hash the manifest gave us in step 1 ---
    // Failing here aborts the bootstrap before we touch the extract
    // path, so a tampered or corrupted ZIP never reaches the user's
    // filesystem as a real ffmpeg.exe.
    emit(
        app,
        DownloadEvent {
            phase: "downloading".into(),
            percent: None,
            message: Some("Verifying checksum…".into()),
        },
    );
    if !constant_time_eq(computed_hash.as_bytes(), expected_hash.as_bytes()) {
        let _ = std::fs::remove_file(&tmp_zip);
        bail!(
            "FFmpeg ZIP integrity check failed: expected sha256 {expected_hash}, got {computed_hash}. \
             Refusing to install. Try again later, or set a manual ffmpeg.exe path in Settings."
        );
    }

    // --- 2. Extract -------------------------------------------------------
    emit(
        app,
        DownloadEvent {
            phase: "extracting".into(),
            percent: None,
            message: Some("Unpacking archive…".into()),
        },
    );

    std::fs::create_dir_all(&tmp_extract).context("creating extract dir")?;
    let zip_file = std::fs::File::open(&tmp_zip).context("reopening downloaded zip")?;
    let mut archive = zip::ZipArchive::new(zip_file).context("opening zip archive")?;
    // `extract` handles nested dirs for us.
    archive.extract(&tmp_extract).context("extracting zip")?;
    drop(archive);

    // BtbN ships a single top-level folder like "ffmpeg-nN.N-latest-win64-gpl-N.N/"
    let nested = std::fs::read_dir(&tmp_extract)
        .context("listing extracted files")?
        .filter_map(|e| e.ok())
        .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .ok_or_else(|| anyhow!("unexpected archive layout (no top-level folder)"))?;

    // --- 3. Install -------------------------------------------------------
    std::fs::create_dir_all(&target).context("creating install dir")?;
    for sub in ["bin", "presets", "doc"] {
        let src = nested.path().join(sub);
        let dst = target.join(sub);
        if src.exists() {
            if dst.exists() {
                let _ = std::fs::remove_dir_all(&dst);
            }
            // rename across drives can fail; fall back to recursive copy
            if std::fs::rename(&src, &dst).is_err() {
                copy_dir_recursive(&src, &dst)?;
            }
        }
    }
    // BtbN names it LICENSE.txt; gyan named it LICENSE. Copy whichever
    // is present so an install carried over from an older version isn't
    // left with a stale license file from the previous vendor.
    let _ = std::fs::remove_file(target.join("LICENSE"));
    let _ = std::fs::remove_file(target.join("LICENSE.txt"));
    for name in ["LICENSE.txt", "LICENSE"] {
        let license = nested.path().join(name);
        if license.exists() {
            let _ = std::fs::copy(&license, target.join(name));
            break;
        }
    }

    // Cleanup best-effort
    let _ = std::fs::remove_file(&tmp_zip);
    let _ = std::fs::remove_dir_all(&tmp_extract);

    if !bin_exe.exists() {
        bail!(
            "ffmpeg.exe missing after extraction: expected {}",
            bin_exe.display()
        );
    }
    Ok(bin_exe)
}

#[cfg(all(windows, not(feature = "studio")))]
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

/// Lowercase hex of a digest. Standalone instead of pulling in the
/// `hex` crate just for this — the loop is straightforward and the
/// result is only used for a constant-time string comparison.
#[cfg(not(feature = "studio"))]
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(*b >> 4) as usize] as char);
        s.push(HEX[(*b & 0x0f) as usize] as char);
    }
    s
}

/// Constant-time byte-slice equality. Avoids leaking timing information
/// during hash comparison — almost certainly overkill against a remote
/// attacker, but cheap enough that we may as well not skip it.
#[cfg(not(feature = "studio"))]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Pull BtbN's combined `checksums.sha256` manifest and pick the asset
/// we should install, returning `(filename, sha256)`.
#[cfg(all(windows, not(feature = "studio")))]
fn fetch_win_asset() -> Result<(String, String)> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(30))
        .build();
    let body: String = agent
        .get(FFMPEG_SHA256_URL)
        .call()?
        .into_string()?;
    select_win_asset(&body)
}

/// Pick the FFmpeg build to install out of a sha256sum-style manifest,
/// returning `(filename, hash)`.
///
/// Lines are `<hash>` then the filename, GNU style (two spaces) or BSD
/// style (` *`); we split on whitespace and strip a leading `*` so
/// either shape works.
///
/// What we're looking for, and why:
///   * `win64` — excludes the linux, win32 and winarm64 assets in the
///     same manifest. ("winarm64" doesn't contain "win64", so the plain
///     substring test is safe.)
///   * `gpl`, not `lgpl` — libx264 and libx265 are GPL-only and every
///     video preset in this app encodes with one of them; the `lgpl`
///     variant has no H.264/HEVC encoder at all. ("win64-lgpl" doesn't
///     contain "win64-gpl", so again the substring test does the right
///     thing.)
///   * not `-shared` — the shared build puts the codecs in separate
///     DLLs, which survives extraction but not the way we copy only
///     `bin/` into place.
///
/// Among the survivors we prefer the highest numbered release branch
/// (`ffmpeg-n8.1-latest-win64-gpl-8.1.zip` over `…-n7.1-…`), falling
/// back to the `master` build if BtbN ever publishes no release-branch
/// asset. Comparing on the parsed (major, minor) rather than
/// alphabetically matters as soon as a two-digit minor appears — "n8.9"
/// sorts above "n8.10" as a string, and below it as a number.
///
/// An empty result is a hard error, never a silent fallback: a manifest
/// with no matching asset means an HTML error page, a redirect, or an
/// upstream reorganisation we need to look at, and installing "whatever
/// else was in the list" would be worse than failing.
#[cfg(all(windows, not(feature = "studio")))]
fn select_win_asset(body: &str) -> Result<(String, String)> {
    // (rank, name, hash) — rank orders release branches above master.
    let mut best: Option<((u32, u32, u32), String, String)> = None;

    for line in body.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(raw_name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = raw_name.trim_start_matches('*');
        // Anchored to ASCII so multi-byte surprises in a tampered
        // response can't slide a "valid" hash past us.
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if !name.starts_with("ffmpeg-")
            || !name.ends_with(".zip")
            || !name.contains("win64-gpl")
            || name.contains("-shared")
        {
            continue;
        }
        let rank = branch_rank(name);
        if best.as_ref().is_none_or(|(best_rank, _, _)| rank > *best_rank) {
            best = Some((rank, name.to_string(), hash.to_ascii_lowercase()));
        }
    }

    match best {
        Some((_, name, hash)) => Ok((name, hash)),
        None => Err(anyhow!(
            "no win64-gpl FFmpeg build found in the release manifest: {:?}",
            body.trim().chars().take(120).collect::<String>()
        )),
    }
}

/// Rank an asset name for `select_win_asset`. `ffmpeg-nX.Y-…` yields
/// `(1, X, Y)`; anything else (i.e. the `master` build) yields
/// `(0, 0, 0)` so it only wins when no release branch is published.
#[cfg(all(windows, not(feature = "studio")))]
fn branch_rank(name: &str) -> (u32, u32, u32) {
    let Some(rest) = name.strip_prefix("ffmpeg-n") else {
        return (0, 0, 0);
    };
    // "8.1-latest-win64-gpl-8.1.zip" -> "8.1"
    let branch = rest.split('-').next().unwrap_or("");
    let mut nums = branch.split('.');
    let major = nums.next().and_then(|s| s.parse().ok());
    let minor = nums.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    match major {
        Some(major) => (1, major, minor),
        None => (0, 0, 0),
    }
}

#[cfg(all(test, not(feature = "studio")))]
mod tests {
    use super::*;

    #[test]
    fn hex_lower_basic() {
        assert_eq!(hex_lower(&[0x00, 0xff, 0xab]), "00ffab");
        assert_eq!(hex_lower(&[]), "");
    }

    #[test]
    fn constant_time_eq_basic() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(constant_time_eq(b"", b""));
    }

    #[cfg(windows)]
    const A_HASH: &str = "8394b52cb5dedb8c472200685edcdf2d882163d0c6bf550e9d3db77995255afb";
    #[cfg(windows)]
    const B_HASH: &str = "aef0c1aaeb9bb804d3187aed95e78ff9b912dcc02199fd0af85be20575f923ae";

    /// A trimmed-down copy of the real `checksums.sha256`, keeping every
    /// asset shape that could plausibly be mistaken for the one we
    /// want: the shared build, the LGPL build, the other architectures,
    /// an older release branch, and the master build.
    #[cfg(windows)]
    fn sample_manifest() -> String {
        format!(
            "{B_HASH}  ffmpeg-master-latest-linux64-gpl.tar.xz\n\
             {B_HASH}  ffmpeg-master-latest-win64-gpl.zip\n\
             {B_HASH}  ffmpeg-master-latest-win64-lgpl.zip\n\
             {B_HASH}  ffmpeg-master-latest-winarm64-gpl.zip\n\
             {B_HASH}  ffmpeg-n7.1-latest-win64-gpl-7.1.zip\n\
             {B_HASH}  ffmpeg-n8.1-latest-win64-gpl-shared-8.1.zip\n\
             {B_HASH}  ffmpeg-n8.1-latest-win64-lgpl-8.1.zip\n\
             {A_HASH}  ffmpeg-n8.1-latest-win64-gpl-8.1.zip\n"
        )
    }

    #[test]
    #[cfg(windows)]
    fn selects_the_newest_release_branch_gpl_build() {
        let (name, hash) = select_win_asset(&sample_manifest()).unwrap();
        assert_eq!(name, "ffmpeg-n8.1-latest-win64-gpl-8.1.zip");
        assert_eq!(hash, A_HASH);
    }

    /// The whole point of deriving the name instead of pinning it: when
    /// upstream rolls the release branch forward, an unchanged build of
    /// the app has to follow it without a code change.
    #[test]
    #[cfg(windows)]
    fn follows_upstream_when_the_branch_rolls_forward() {
        let body = format!(
            "{B_HASH}  ffmpeg-n8.1-latest-win64-gpl-8.1.zip\n\
             {A_HASH}  ffmpeg-n9.0-latest-win64-gpl-9.0.zip\n"
        );
        let (name, hash) = select_win_asset(&body).unwrap();
        assert_eq!(name, "ffmpeg-n9.0-latest-win64-gpl-9.0.zip");
        assert_eq!(hash, A_HASH);
    }

    /// Branch ordering is numeric, not lexical — "n8.9" must not beat
    /// "n8.10".
    #[test]
    #[cfg(windows)]
    fn orders_branches_numerically() {
        let body = format!(
            "{B_HASH}  ffmpeg-n8.9-latest-win64-gpl-8.9.zip\n\
             {A_HASH}  ffmpeg-n8.10-latest-win64-gpl-8.10.zip\n"
        );
        let (name, _) = select_win_asset(&body).unwrap();
        assert_eq!(name, "ffmpeg-n8.10-latest-win64-gpl-8.10.zip");
    }

    /// If upstream ever ships only the master build, take it rather
    /// than leaving the user with no FFmpeg at all.
    #[test]
    #[cfg(windows)]
    fn falls_back_to_master_when_no_release_branch_exists() {
        let body = format!("{A_HASH}  ffmpeg-master-latest-win64-gpl.zip\n");
        let (name, hash) = select_win_asset(&body).unwrap();
        assert_eq!(name, "ffmpeg-master-latest-win64-gpl.zip");
        assert_eq!(hash, A_HASH);
    }

    #[test]
    #[cfg(windows)]
    fn accepts_bsd_star_prefix() {
        let body = format!("{A_HASH} *ffmpeg-n8.1-latest-win64-gpl-8.1.zip\n");
        let (name, hash) = select_win_asset(&body).unwrap();
        assert_eq!(name, "ffmpeg-n8.1-latest-win64-gpl-8.1.zip");
        assert_eq!(hash, A_HASH);
    }

    /// Nothing usable must fail loudly. Installing "whatever else was
    /// in the list" would mean an LGPL build with no H.264 encoder, or
    /// an ARM archive on an x64 machine.
    #[test]
    #[cfg(windows)]
    fn rejects_manifests_with_nothing_usable() {
        // Only variants we must never pick.
        let body = format!(
            "{B_HASH}  ffmpeg-n8.1-latest-win64-lgpl-8.1.zip\n\
             {B_HASH}  ffmpeg-n8.1-latest-win64-gpl-shared-8.1.zip\n\
             {B_HASH}  ffmpeg-master-latest-winarm64-gpl.zip\n\
             {B_HASH}  ffmpeg-master-latest-linux64-gpl.tar.xz\n"
        );
        assert!(select_win_asset(&body).is_err());
        // Right name, malformed hash.
        assert!(select_win_asset("nothexnothex  ffmpeg-n8.1-latest-win64-gpl-8.1.zip\n").is_err());
        // An HTML error page served with a 200, and an empty body.
        assert!(select_win_asset("<html>404</html>").is_err());
        assert!(select_win_asset("").is_err());
    }
}

// =====================================================================
// macOS bootstrap. evermeet.cx ships ffmpeg and ffprobe as individual
// zips (no bundled archive), so we run the download-extract loop twice
// and drop both binaries into the same bin/ subdir for symmetry with
// the Windows install layout.
// =====================================================================

/// Metadata pulled from evermeet's info JSON. We only consume the fields
/// we need — the response carries more (version, size, signature URLs)
/// that future-us might want.
#[cfg(all(target_os = "macos", not(feature = "studio")))]
struct EvermeetRelease {
    download_url: String,
    /// Optional because evermeet's JSON schema has shifted over time.
    /// When None we skip SHA verification with a warning rather than
    /// hard-failing — TLS is still doing the heavy lifting and the
    /// download stays usable. Phase 2 of the macOS port can promote
    /// this back to required once we've confirmed the stable field path.
    sha256: Option<String>,
}

#[cfg(all(target_os = "macos", not(feature = "studio")))]
fn fetch_evermeet_info(info_url: &str) -> Result<EvermeetRelease> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(30))
        .build();
    let body: String = agent
        .get(info_url)
        .call()
        .context("fetching evermeet info JSON")?
        .into_string()?;
    let json: serde_json::Value = serde_json::from_str(&body)
        .context("parsing evermeet info JSON")?;

    let download_url = json
        .get("download")
        .and_then(|d| d.get("zip"))
        .and_then(|z| z.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow!("evermeet info missing download.zip.url"))?
        .to_string();

    // Probe several plausible locations for the SHA. evermeet's JSON
    // structure isn't formally documented and has changed across
    // versions; covering multiple paths keeps verification working
    // through future schema drift.
    let sha256 = json
        .pointer("/checksums/sha256")
        .and_then(|v| v.as_str())
        .or_else(|| {
            json.pointer("/download/zip/sha256")
                .and_then(|v| v.as_str())
        })
        .or_else(|| json.get("sha256").and_then(|v| v.as_str()))
        .map(|s| s.to_ascii_lowercase());

    Ok(EvermeetRelease { download_url, sha256 })
}

#[cfg(all(target_os = "macos", not(feature = "studio")))]
fn download_and_install_macos(app: &AppHandle) -> Result<PathBuf> {
    let target = paths::local_data_dir()?.join("ffmpeg");
    let bin_dir = target.join("bin");
    std::fs::create_dir_all(&bin_dir).context("creating bin dir")?;
    let bin_exe = bin_dir.join("ffmpeg");

    // Each tuple: (label shown in progress events, info URL, name we
    // expect to find inside the extracted zip, final destination path).
    let jobs = [
        (
            "FFmpeg",
            EVERMEET_FFMPEG_INFO_URL,
            "ffmpeg",
            bin_dir.join("ffmpeg"),
        ),
        (
            "FFprobe",
            EVERMEET_FFPROBE_INFO_URL,
            "ffprobe",
            bin_dir.join("ffprobe"),
        ),
    ];

    for (label, info_url, archive_name, dst) in jobs {
        // --- 1. Resolve the actual download URL + expected hash --------
        emit(
            app,
            DownloadEvent {
                phase: "downloading".into(),
                percent: Some(0.0),
                message: Some(format!("Resolving {label} release…")),
            },
        );
        let release = fetch_evermeet_info(info_url)
            .with_context(|| format!("looking up {label} release from evermeet.cx"))?;

        // --- 2. Download the zip with progress -------------------------
        let uid = std::process::id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_zip = std::env::temp_dir()
            .join(format!("{archive_name}-offspring-{uid}-{ts}.zip"));
        let tmp_extract = std::env::temp_dir()
            .join(format!("{archive_name}-offspring-extract-{uid}-{ts}"));

        emit(
            app,
            DownloadEvent {
                phase: "downloading".into(),
                percent: Some(0.0),
                message: Some(format!("Connecting to evermeet.cx for {label}…")),
            },
        );

        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(120))
            .build();
        let resp = agent
            .get(&release.download_url)
            .call()
            .with_context(|| format!("downloading {label}"))?;

        let total_len: Option<u64> = resp
            .header("Content-Length")
            .and_then(|s| s.parse::<u64>().ok());

        let mut reader = resp.into_reader();
        let mut file = std::fs::File::create(&tmp_zip)
            .with_context(|| format!("creating temp file {}", tmp_zip.display()))?;
        let mut buf = [0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        let mut last_emit = std::time::Instant::now();
        let mut hasher = Sha256::new();

        loop {
            let n = reader.read(&mut buf).context("reading zip stream")?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).context("writing temp zip")?;
            hasher.update(&buf[..n]);
            downloaded += n as u64;

            if last_emit.elapsed() >= Duration::from_millis(150) {
                last_emit = std::time::Instant::now();
                let (pct, msg) = match total_len {
                    Some(total) if total > 0 => {
                        let p = (downloaded as f64 / total as f64 * 100.0).min(100.0) as f32;
                        (
                            Some(p),
                            Some(format!(
                                "{label}: {:.1} of {:.1} MB",
                                downloaded as f64 / 1_048_576.0,
                                total as f64 / 1_048_576.0
                            )),
                        )
                    }
                    _ => (
                        None,
                        Some(format!(
                            "{label}: {:.1} MB",
                            downloaded as f64 / 1_048_576.0
                        )),
                    ),
                };
                emit(
                    app,
                    DownloadEvent {
                        phase: "downloading".into(),
                        percent: pct,
                        message: msg,
                    },
                );
            }
        }
        drop(file);
        let computed_hash = hex_lower(&hasher.finalize());

        // --- 3. Verify SHA-256 when evermeet exposed one --------------
        match release.sha256 {
            Some(expected) => {
                emit(
                    app,
                    DownloadEvent {
                        phase: "downloading".into(),
                        percent: None,
                        message: Some(format!("Verifying {label} checksum…")),
                    },
                );
                if !constant_time_eq(computed_hash.as_bytes(), expected.as_bytes()) {
                    let _ = std::fs::remove_file(&tmp_zip);
                    bail!(
                        "{label} ZIP integrity check failed: expected sha256 {expected}, got {computed_hash}. \
                         Refusing to install. Try again later, or set a manual {archive_name} path in Settings."
                    );
                }
            }
            None => {
                // evermeet's info JSON didn't include a hash this run.
                // We log via the progress event so the user can see the
                // gap rather than have it disappear silently.
                emit(
                    app,
                    DownloadEvent {
                        phase: "downloading".into(),
                        percent: None,
                        message: Some(format!(
                            "{label}: no SHA-256 in evermeet info — relying on TLS only."
                        )),
                    },
                );
            }
        }

        // --- 4. Extract the zip and move the binary into bin/ ---------
        emit(
            app,
            DownloadEvent {
                phase: "extracting".into(),
                percent: None,
                message: Some(format!("Unpacking {label}…")),
            },
        );

        std::fs::create_dir_all(&tmp_extract).context("creating extract dir")?;
        let zip_file = std::fs::File::open(&tmp_zip).context("reopening downloaded zip")?;
        let mut archive = zip::ZipArchive::new(zip_file).context("opening zip archive")?;
        archive.extract(&tmp_extract).context("extracting zip")?;
        drop(archive);

        // evermeet ships a flat archive with the binary at the root —
        // no nested folder. Walk the extracted tree and grab the first
        // file whose name matches what we expect (tolerant of zips that
        // sneak in a __MACOSX/ metadata dir).
        let found = find_binary_in(&tmp_extract, archive_name)
            .with_context(|| format!("locating {archive_name} inside extracted archive"))?;

        if dst.exists() {
            let _ = std::fs::remove_file(&dst);
        }
        // Try rename first (cheap when temp dir is on the same volume),
        // fall back to copy across volumes.
        if std::fs::rename(&found, &dst).is_err() {
            std::fs::copy(&found, &dst)
                .with_context(|| format!("copying {archive_name} into bin/"))?;
        }

        // --- 5. chmod +x. Rust's zip extractor doesn't preserve Unix
        // mode bits from arbitrary archives, so the extracted binary
        // lands without the executable bit set. Without this, every
        // subprocess spawn would fail with EACCES.
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dst)
            .with_context(|| format!("statting {}", dst.display()))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dst, perms)
            .with_context(|| format!("chmod +x on {}", dst.display()))?;

        // Cleanup best-effort
        let _ = std::fs::remove_file(&tmp_zip);
        let _ = std::fs::remove_dir_all(&tmp_extract);
    }

    if !bin_exe.exists() {
        bail!(
            "ffmpeg binary missing after extraction: expected {}",
            bin_exe.display()
        );
    }
    Ok(bin_exe)
}

/// Walk a directory tree looking for the first entry whose filename
/// matches `name` exactly. Used by the macOS bootstrap to find the
/// binary inside evermeet's zip without assuming a specific layout.
#[cfg(all(target_os = "macos", not(feature = "studio")))]
fn find_binary_in(root: &std::path::Path, name: &str) -> Result<PathBuf> {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).context("reading dir during binary search")? {
            let entry = entry?;
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                // Skip macOS metadata noise that some zip tools embed.
                if path.file_name().map(|n| n == "__MACOSX").unwrap_or(false) {
                    continue;
                }
                stack.push(path);
            } else if ft.is_file() {
                if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                    return Ok(path);
                }
            }
        }
    }
    Err(anyhow!(
        "no file named {name} inside {}",
        root.display()
    ))
}

// ----- studio build: spawn_download is a no-op error path ----------
//
// Keeping the function signature lets `commands::download_ffmpeg`
// stay variant-agnostic. The studio build's frontend won't show the
// "Download FFmpeg" button (the variant is exposed via
// `commands::get_build_variant`), so this stub is defense-in-depth.
#[cfg(feature = "studio")]
pub fn spawn_download(app: AppHandle) {
    use tauri::Emitter;
    let _ = app.emit(
        "ffmpeg-download",
        serde_json::json!({
            "phase": "error",
            "percent": null,
            "message": "Offspring Studio does not include the FFmpeg downloader. Install ffmpeg.exe manually and set its path in Settings."
        }),
    );
}
