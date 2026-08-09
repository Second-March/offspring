//! Spawn `offspring.exe` with the right CLI args when the user picks a
//! preset from the flyout.
//!
//! Explorer hands us an `IShellItemArray` of the selected items; we
//! convert those to filesystem paths, build the command-line, and
//! launch. Fire-and-forget — the child runs independently and we
//! return quickly so the menu animation doesn't hitch.

use std::path::PathBuf;
use std::process::Command;

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

use crate::presets::read_exe_path;

/// Pull every filesystem path out of an `IShellItemArray`. Items that
/// don't have a filesystem path — entries inside a .zip, an MTP camera,
/// an FTP site, a search-results view — can't be handed to ffmpeg and
/// are skipped.
///
/// Skipping used to be silent, and that quietly changed what the user
/// asked for: every tool gates its menu visibility on the RAW
/// `GetCount()` but then acts on this filtered list, so a five-item
/// selection with one unresolvable entry converted four files and said
/// nothing, and a two-item Merge could arrive at the app with one path
/// and fail with a message that doesn't match what the user selected.
/// One notice per invocation is cheap and only appears in a case that
/// genuinely can't be serviced.
pub fn items_to_paths(items: Option<&IShellItemArray>) -> Vec<PathBuf> {
    let Some(arr) = items else { return Vec::new() };
    let count = unsafe { arr.GetCount().unwrap_or(0) };
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        unsafe {
            if let Ok(item) = arr.GetItemAt(i) {
                if let Ok(pwstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                    if !pwstr.is_null() {
                        let s = pwstr.to_string().unwrap_or_default();
                        if !s.is_empty() {
                            out.push(PathBuf::from(s));
                        }
                        windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.0 as _));
                    }
                }
            }
        }
    }

    let skipped = count as usize - out.len();
    if skipped > 0 {
        let text = if out.is_empty() {
            format!(
                "Offspring can't work on {} of the selected item{}.\n\n\
                 They have no location on disk — items inside a .zip, on a \
                 camera or phone, or in a search-results view have to be \
                 copied to a folder first.",
                skipped,
                if skipped == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Skipping {} of the selected item{} — {} ha{} no location on \
                 disk (inside a .zip, on a camera or phone, or in a \
                 search-results view).\n\n\
                 Continuing with the other {}.",
                skipped,
                if skipped == 1 { "" } else { "s" },
                if skipped == 1 { "it" } else { "they" },
                if skipped == 1 { "s" } else { "ve" },
                out.len()
            )
        };
        message_box(&text);
    }
    out
}

/// Show a modal notice. The only channel available from inside
/// Explorer's process.
fn message_box(text: &str) {
    let mut text_w: Vec<u16> = text.encode_utf16().collect();
    text_w.push(0);
    let mut caption_w: Vec<u16> = "Offspring".encode_utf16().collect();
    caption_w.push(0);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(caption_w.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// Surface a failed `CreateProcess` instead of swallowing it.
///
/// Every launcher below used to end in `let _ = cmd.spawn();`. When the
/// spawn failed — offspring.exe uninstalled or moved while the stale
/// `ExePath` registry value still pointed at it, or blocked by policy —
/// the menu entry simply did nothing, forever, with no way for the user
/// to tell that anything was wrong.
///
/// A message box is the only channel available from inside Explorer's
/// process, and this only fires on a genuinely broken install, not on
/// any normal path.
fn report_spawn_failure(result: std::io::Result<std::process::Child>, exe: &str) {
    let Err(e) = result else { return };
    let text = format!(
        "Offspring couldn't start.\n\n{}\n\nTried to run:\n{}\n\n\
         Reinstalling Offspring will repair this.",
        e, exe
    );
    message_box(&text);
}

pub fn spawn_preset(preset_id: &str, files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("preset").arg("--id").arg(preset_id);
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_custom(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("custom");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_merge(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("merge");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_grayscale(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("grayscale");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_compare(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("compare");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_overlay(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("overlay");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_trim(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("trim");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_invert(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("invert");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_make_square(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("make-square");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

pub fn spawn_modify(files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("modify");
    for f in files {
        cmd.arg(f);
    }
    report_spawn_failure(cmd.spawn(), &exe);
}

/// Launch the main Offspring UI (the Settings window). No file args —
/// the CLI `settings` verb ignores any selection and always shows the
/// configuration surface.
pub fn spawn_settings() {
    let Some(exe) = read_exe_path() else { return };
    let mut cmd = Command::new(&exe);
    cmd.arg("settings");
    report_spawn_failure(cmd.spawn(), &exe);
}
