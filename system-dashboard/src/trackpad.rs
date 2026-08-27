//! Trackpad-guard state — wraps `systemctl is-active trackpad-guard.service`.
//! trackpad-guard runs as a SYSTEM service (not user) so it can also cover
//! the greeter's sway session, where no user is logged in yet. Reading
//! state doesn't need root; toggling does, so we shell out to the
//! `trackpad-guard-toggle` wrapper which handles sudo + notify-send.
//! The same wrapper is bound to Mod+Shift+G so the on/off notification is
//! identical regardless of which path the user took.

use serde::Serialize;
use std::process::Command;

const UNIT: &str = "trackpad-guard.service";
const TOGGLE_BIN: &str = "/usr/local/bin/trackpad-guard-toggle";
const TUNE_BIN: &str = "/usr/local/bin/trackpad-guard-tune";
/// Read directly rather than shelling out to `trackpad-guard-tune` on every
/// poll — this runs on the snapshot path a few times a second, and spawning a
/// bash script that often to read one integer is not worth it. Writes still go
/// through the wrapper, so the sudo/notify semantics stay in one place.
const CONFIG: &str = "/etc/trackpad-guard/config";
/// Keep in sync with KEY_TYPING_WINDOW in trackpad-guard/src/main.rs.
const DEFAULT_GATE_MS: u16 = 200;

#[derive(Serialize, Clone, Debug)]
pub struct TrackpadGuardState {
    pub active: bool,
    /// Effective typing gate in ms. Falls back to the daemon's compiled
    /// default when the config file is absent, which is also what the daemon
    /// itself does, so the two can't disagree while the file is missing.
    pub typing_gate_ms: u16,
}

pub fn read() -> TrackpadGuardState {
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", UNIT])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    TrackpadGuardState {
        active,
        typing_gate_ms: read_typing_gate().unwrap_or(DEFAULT_GATE_MS),
    }
}

fn read_typing_gate() -> Option<u16> {
    let body = std::fs::read_to_string(CONFIG).ok()?;
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .filter(|(k, _)| k.trim() == "typing_gate_ms")
        .filter_map(|(_, v)| v.trim().parse::<u16>().ok())
        .next_back()
}

/// Set the typing gate. Takes effect within ~1s without a restart — the
/// daemon watches the config file's mtime.
pub fn set_typing_gate(ms: u16) -> Result<(), String> {
    let status = Command::new(TUNE_BIN)
        .arg(ms.to_string())
        .status()
        .map_err(|e| format!("{TUNE_BIN} failed to spawn: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{TUNE_BIN} exited {:?}", status.code()))
    }
}

pub fn toggle() -> Result<(), String> {
    let status = Command::new(TOGGLE_BIN)
        .status()
        .map_err(|e| format!("{TOGGLE_BIN} failed to spawn: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{TOGGLE_BIN} exited {:?}", status.code()))
    }
}
