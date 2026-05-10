//! Fan state from /dev/shm/argon-fan.json — the daemon writes this every
//! poll_interval_sec. Absence of the file means the daemon isn't running.

use serde::{Deserialize, Serialize};
use std::fs;

const STATE_FILE: &str = "/dev/shm/argon-fan.json";

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
}

pub fn read() -> FanState {
    match fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<DaemonState>(&s).ok())
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
