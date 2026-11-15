//! Touchpad disable-while-typing state — libinput's own DWT, as sway has it
//! applied to the touchpad (`swaymsg -t get_inputs`).
//!
//! The keyboard/touchpad pairing that makes native DWT work on the Argon ONE
//! UP comes from the quirks file and udev rule in `touchpad-native/`; those
//! stay installed. This tile only flips sway's `dwt` / `dwtp` setting, which
//! needs no root and no re-login. Toggling shells out to `touchpad-mode
//! toggle`, which also rewrites the sway config so the choice persists and
//! sends the notification. Mod+Shift+G runs the same command.
//!
//! If the older trackpad-guard daemon is running instead, the tile says so
//! and reports Off: libinput DWT is not what is protecting typing then.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

const GUARD_UNIT: &str = "trackpad-guard.service";
const MODE_BIN: &str = "touchpad-mode";

#[derive(Serialize, Clone, Debug)]
pub struct TrackpadState {
    /// libinput disable-while-typing is on for the touchpad.
    pub active: bool,
    /// The trackpad-guard daemon is running (native DWT not in use).
    pub guard: bool,
}

pub fn read() -> TrackpadState {
    let guard = Command::new("systemctl")
        .args(["is-active", "--quiet", GUARD_UNIT])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    TrackpadState {
        active: !guard && native_dwt_enabled(),
        guard,
    }
}

fn native_dwt_enabled() -> bool {
    let Ok(out) = Command::new("swaymsg")
        .args(["-t", "get_inputs", "--raw"])
        .output()
    else {
        return false;
    };
    let Ok(inputs) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return false;
    };
    inputs
        .as_array()
        .into_iter()
        .flatten()
        .filter(|i| i["type"] == "touchpad")
        .any(|i| i["libinput"]["dwt"] == "enabled")
}

/// The installer puts touchpad-mode in /usr/local/bin; a by-hand setup has it
/// in ~/.local/bin, which may not be on the PATH the dashboard was started
/// with. Fall back to a PATH lookup.
fn mode_bin() -> PathBuf {
    let mut candidates = vec![PathBuf::from("/usr/local/bin").join(MODE_BIN)];
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local/bin").join(MODE_BIN));
    }
    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(MODE_BIN))
}

pub fn toggle() -> Result<(), String> {
    let bin = mode_bin();
    let out = Command::new(&bin)
        .arg("toggle")
        .output()
        .map_err(|e| format!("{} failed to spawn: {e}", bin.display()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} toggle exited {:?}: {}",
            bin.display(),
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}
