//! Display brightness — reads the cache file that `bin/brightness` keeps
//! and writes via the same script. Falls back to a one-shot ddcutil query
//! if the cache is missing.

use serde::Serialize;
use std::fs;
use std::process::Command;

const CACHE_FILE: &str = "/tmp/.brightness_level";

#[derive(Serialize, Clone, Debug)]
pub struct BrightnessState {
    pub percent: u8,
}

pub fn read() -> BrightnessState {
    let percent = fs::read_to_string(CACHE_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .unwrap_or(50);
    BrightnessState { percent }
}

pub fn set(level: u8) -> Result<(), String> {
    let level = level.min(100);
    let status = Command::new("brightness")
        .args(["set", &level.to_string()])
        .status()
        .map_err(|e| format!("brightness exec failed: {e}"))?;
    if !status.success() {
        return Err(format!("brightness exited with {status}"));
    }
    Ok(())
}
