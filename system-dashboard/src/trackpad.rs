//! Trackpad-guard state — wraps `systemctl --user is-active trackpad-guard.service`.
//! Toggling uses `enable --now` / `disable --now` so the choice persists across reboots.

use serde::Serialize;
use std::process::Command;

const UNIT: &str = "trackpad-guard.service";

#[derive(Serialize, Clone, Debug)]
pub struct TrackpadGuardState {
    pub active: bool,
}

pub fn read() -> TrackpadGuardState {
    let active = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", UNIT])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    TrackpadGuardState { active }
}

pub fn toggle() -> Result<(), String> {
    let action = if read().active { "disable" } else { "enable" };
    let status = Command::new("systemctl")
        .args(["--user", action, "--now", UNIT])
        .status()
        .map_err(|e| format!("systemctl {action} failed to spawn: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user {action} --now {UNIT} exited {:?}",
            status.code()
        ))
    }
}
