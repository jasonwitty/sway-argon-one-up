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

#[derive(Serialize, Clone, Debug)]
pub struct TrackpadGuardState {
    pub active: bool,
}

pub fn read() -> TrackpadGuardState {
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", UNIT])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    TrackpadGuardState { active }
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
