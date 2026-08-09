use anyhow::{Context, Result};
use mslnk::ShellLink;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::paths;
use crate::presets::{Preset, Settings};

/// Legacy filename prefix. Any .lnk in the user's SendTo folder whose stem
/// starts with "Offspring - " is treated as ours and cleaned up on sync.
/// This lets pre-existing installs upgrade cleanly to the unadorned naming
/// scheme ("GIF 720p.lnk" instead of "Offspring - GIF 720p.lnk").
const LEGACY_PREFIXES: &[&str] = &["Offspring"];

/// On-disk record of which SendTo shortcut filenames belong to us. Without
/// a filename prefix we have no other way to identify our .lnks vs the user's
/// other SendTo entries (e.g. Bluetooth, 7-Zip, Desktop).
#[derive(Serialize, Deserialize, Default)]
struct Manifest {
    shortcuts: Vec<String>,
}

impl Manifest {
    fn load() -> Manifest {
        let Ok(path) = paths::sendto_manifest_path() else { return Manifest::default() };
        if !path.exists() {
            return Manifest::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<()> {
        let path = paths::sendto_manifest_path()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json).context("writing sendto manifest")?;
        Ok(())
    }

    fn clear() -> Result<()> {
        let path = paths::sendto_manifest_path()?;
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        Ok(())
    }
}

/// Turn a preset name into a filename Windows will actually accept.
///
/// Preset names are free text. `shortcut_path` used to interpolate them
/// straight into a path, so a perfectly reasonable name like
/// `GIF 16:9` — a colon is illegal in a Windows filename — made
/// `create_lnk` fail, which aborted the whole sync mid-loop and left
/// the already-written shortcuts orphaned (the manifest that records
/// them is only saved at the end). A name containing `..\` was worse:
/// it resolved outside the SendTo folder entirely.
///
/// Replaces every reserved character, flattens path separators, trims
/// the trailing dots and spaces Windows silently strips, side-steps the
/// reserved device names, and caps the length so the whole path stays
/// under MAX_PATH. Falls back to a fixed stem when nothing usable is
/// left (a name made entirely of illegal characters).
fn sanitize_shortcut_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let mut out: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if (c as u32) < 0x20 => '-',
            c => c,
        })
        .collect();

    // Windows drops trailing dots and spaces from filenames, so a name
    // ending in one would produce a .lnk we then fail to find again.
    out = out.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();

    // Keep the whole component well inside MAX_PATH once ".lnk" and the
    // SendTo directory are added.
    const MAX_STEM: usize = 96;
    if out.chars().count() > MAX_STEM {
        out = out.chars().take(MAX_STEM).collect::<String>().trim_end().to_string();
    }

    if out.is_empty() {
        return "Offspring preset".to_string();
    }

    // Reserved device names are illegal with or without an extension.
    let stem_upper = out
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if RESERVED.contains(&stem_upper.as_str()) {
        out.insert(0, '_');
    }
    out
}

fn shortcut_path(name: &str) -> Result<PathBuf> {
    Ok(paths::sendto_dir()?.join(format!("{}.lnk", sanitize_shortcut_name(name))))
}

pub fn current_exe() -> Result<PathBuf> {
    std::env::current_exe().context("getting current exe path")
}

pub fn write_preset_shortcut(preset: &Preset) -> Result<PathBuf> {
    let path = shortcut_path(&preset.name)?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some(format!("preset --id {}", preset.id)));
    link.set_working_dir(Some(String::new())); // empty so output goes next to source
    if let Some(ref icon) = preset.icon {
        link.set_icon_location(Some(icon.clone()));
    }
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

pub fn write_custom_shortcut() -> Result<PathBuf> {
    let path = shortcut_path("Custom...")?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some("custom".to_string()));
    link.set_working_dir(Some(String::new()));
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

/// Single "Offspring Merge" shortcut. Distinct from the per-preset
/// shortcuts because Merge doesn't take a preset — the Rust side
/// derives format + settings from the first selected file. Windows
/// forwards the full multi-selection to `offspring.exe merge` as argv,
/// so one shortcut handles any N≥2 selection.
pub fn write_merge_shortcut() -> Result<PathBuf> {
    let path = shortcut_path("Offspring Merge")?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some("merge".to_string()));
    link.set_working_dir(Some(String::new()));
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

/// Single "Offspring Greyscale" shortcut. Same pattern as Merge — one
/// entry handles any-sized multi-selection (each file is encoded to a
/// greyscale copy alongside it).
pub fn write_grayscale_shortcut() -> Result<PathBuf> {
    let path = shortcut_path("Offspring Greyscale")?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some("grayscale".to_string()));
    link.set_working_dir(Some(String::new()));
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

pub fn write_compare_shortcut() -> Result<PathBuf> {
    let path = shortcut_path("Offspring Compare")?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some("compare".to_string()));
    link.set_working_dir(Some(String::new()));
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

pub fn write_overlay_shortcut() -> Result<PathBuf> {
    let path = shortcut_path("Offspring Overlay")?;
    let exe = current_exe()?;
    let mut link = ShellLink::new(exe.to_string_lossy().as_ref())
        .context("creating shell link")?;
    link.set_arguments(Some("overlay".to_string()));
    link.set_working_dir(Some(String::new()));
    link.create_lnk(&path).context("writing .lnk")?;
    Ok(path)
}

/// Remove any leftover pre-manifest shortcuts from the user's SendTo folder.
/// These are the old "Offspring - *.lnk" naming we used before switching to
/// unadorned preset names. Safe to run on every sync.
fn remove_legacy_shortcuts() -> Result<()> {
    let dir = paths::sendto_dir()?;
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let p = entry.path();
        if p.extension().map(|e| e != "lnk").unwrap_or(true) {
            continue;
        }
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        let looks_legacy = LEGACY_PREFIXES
            .iter()
            .any(|pre| stem.starts_with(&format!("{pre} - ")));
        if looks_legacy {
            let _ = std::fs::remove_file(&p);
        }
    }
    Ok(())
}

/// Remove every shortcut listed in the current manifest. Missing files are
/// silently skipped — the user may have deleted them manually, which is fine.
fn remove_manifest_shortcuts(manifest: &Manifest) -> Result<()> {
    let dir = paths::sendto_dir()?;
    for name in &manifest.shortcuts {
        let path = dir.join(name);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

pub fn sync(presets: &[Preset], settings: &Settings) -> Result<()> {
    // Remove legacy prefix-style shortcuts from any previous version, then
    // remove everything our current manifest claims. This catches renames:
    // the preset "Fast GIF" → "Fast.gif" means the old "Fast GIF.lnk" is in
    // the manifest and gets cleaned up before we write the new name.
    remove_legacy_shortcuts()?;
    let old = Manifest::load();
    remove_manifest_shortcuts(&old)?;

    // Write new shortcuts, collecting their basenames for the manifest.
    // De-dup by name (case-insensitive on Windows) so two presets with the
    // same name don't double-write and fight over the same .lnk.
    let mut written: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let push_if_new = |p: &Path, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            let key = name.to_lowercase();
            if seen.insert(key) {
                out.push(name.to_string());
            }
        }
    };

    // One preset that can't be written must not abort the sync. This
    // loop runs AFTER the old shortcuts have been deleted and BEFORE the
    // manifest is saved, so bailing out mid-way used to leave the
    // already-written .lnk files unrecorded — invisible to every later
    // sync and to `cleanup()`, i.e. orphaned on the user's machine for
    // good. Skip the failure, keep the rest, record what actually landed.
    for preset in presets.iter().filter(|p| p.enabled) {
        match write_preset_shortcut(preset) {
            Ok(p) => push_if_new(&p, &mut written, &mut seen),
            Err(e) => {
                crate::dlog!(
                    "sendto: skipping preset {:?} — shortcut write failed: {e:#}",
                    preset.name
                );
            }
        }
    }
    let cp = write_custom_shortcut()?;
    push_if_new(&cp, &mut written, &mut seen);

    // Single "Offspring Merge" shortcut, gated on the Merge tool toggle.
    // One entry serves any multi-selection — no per-preset duplication.
    if settings.tools.merge.enabled {
        let mp = write_merge_shortcut()?;
        push_if_new(&mp, &mut written, &mut seen);
    }
    if settings.tools.grayscale.enabled {
        let gp = write_grayscale_shortcut()?;
        push_if_new(&gp, &mut written, &mut seen);
    }
    if settings.tools.compare.enabled {
        let cp = write_compare_shortcut()?;
        push_if_new(&cp, &mut written, &mut seen);
    }
    if settings.tools.overlay.enabled {
        let op = write_overlay_shortcut()?;
        push_if_new(&op, &mut written, &mut seen);
    }

    Manifest { shortcuts: written }.save()?;
    Ok(())
}

pub fn cleanup() -> Result<()> {
    remove_legacy_shortcuts()?;
    let m = Manifest::load();
    remove_manifest_shortcuts(&m)?;
    Manifest::clear()?;
    Ok(())
}
