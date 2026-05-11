//! Audio volume — reads + writes via `wpctl`. The pipewire-pulse daemon
//! reports volume on a 0.0–1.0 scale (or higher, with boost). We clamp to
//! 0–100 percent for the UI.

use serde::Serialize;
use std::process::Command;

#[derive(Serialize, Clone, Debug)]
pub struct VolumeState {
    pub percent: u8,
    pub muted: bool,
}

pub fn read() -> VolumeState {
    let out = match Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            return VolumeState {
                percent: 0,
                muted: false,
            }
        }
    };
    let s = String::from_utf8_lossy(&out.stdout);
    // wpctl prints e.g. "Volume: 0.70" or "Volume: 0.70 [MUTED]".
    let muted = s.contains("[MUTED]");
    let percent = s
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| (v * 100.0).round().clamp(0.0, 100.0) as u8)
        .unwrap_or(0);
    VolumeState { percent, muted }
}

pub fn set(level: u8) -> Result<(), String> {
    let level = level.min(100);
    let value = format!("{:.2}", level as f32 / 100.0);
    let status = Command::new("wpctl")
        .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &value])
        .status()
        .map_err(|e| format!("wpctl exec failed: {e}"))?;
    if !status.success() {
        return Err(format!("wpctl exited with {status}"));
    }
    Ok(())
}
