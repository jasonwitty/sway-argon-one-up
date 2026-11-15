//! Fan state from /dev/shm/argon-fan.json — the daemon writes this every
//! poll_interval_sec. A missing file, or one the daemon stopped refreshing,
//! means the daemon isn't running: a crashed or killed daemon leaves its last
//! state behind, and that must not be shown as live.

use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const STATE_FILE: &str = "/dev/shm/argon-fan.json";
/// Matches argon-fan's STATE_STALE_SEC (it polls at most 30s apart).
const STATE_STALE_SEC: u64 = 90;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FanState {
    pub mode: String,
    pub pwm: u32,
    pub pwm_pct: u32,
    pub temp_c: f32,
    pub online: bool,
}

#[derive(Deserialize)]
struct DaemonState {
    active_mode: String,
    pwm: u32,
    pwm_pct: u32,
    temp_c: f32,
    timestamp: u64,
}

fn is_fresh(timestamp: u64, now: u64) -> bool {
    now.saturating_sub(timestamp) <= STATE_STALE_SEC
}

pub fn read() -> FanState {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<DaemonState>(&s).ok())
        .filter(|d| is_fresh(d.timestamp, now))
    {
        Some(d) => FanState {
            mode: d.active_mode,
            pwm: d.pwm,
            pwm_pct: d.pwm_pct,
            temp_c: d.temp_c,
            online: true,
        },
        None => FanState {
            mode: "offline".into(),
            pwm: 0,
            pwm_pct: 0,
            temp_c: 0.0,
            online: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_state_is_not_live() {
        assert!(is_fresh(1000, 1000));
        assert!(is_fresh(1000, 1000 + STATE_STALE_SEC));
        assert!(!is_fresh(1000, 1001 + STATE_STALE_SEC));
        // A clock that stepped backwards must not make the state look stale.
        assert!(is_fresh(1000, 900));
    }
}
