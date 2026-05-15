//! Trackpad-guard state — wraps `systemctl is-active trackpad-guard.service`.
//! trackpad-guard runs as a SYSTEM service (not user) so it can also cover
//! the greeter's sway session, where no user is logged in yet. Reading
//! state doesn't need root; toggling does, so we shell out via `sudo -n`
//! against a sudoers entry installed by install.sh.
//! Toggling uses `enable --now` / `disable --now` so the choice persists.

use serde::Serialize;
use std::process::Command;

const UNIT: &str = "trackpad-guard.service";

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
    let action = if read().active { "disable" } else { "enable" };
    let status = Command::new("sudo")
        .args(["-n", "systemctl", action, "--now", UNIT])
        .status()
        .map_err(|e| format!("sudo systemctl {action} failed to spawn: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "sudo systemctl {action} --now {UNIT} exited {:?}",
            status.code()
        ))
    }
}
