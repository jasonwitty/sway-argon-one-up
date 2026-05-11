//! CPU governor + AC presence. Reads the same sysfs file `power-mode get`
//! does, and infers AC presence from the CW2217 current-register sign bit
//! the way `lid-suspend open` does.

use serde::Serialize;
use std::fs;

const GOVERNOR_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";

#[derive(Serialize, Clone, Debug)]
pub struct PowerState {
    pub governor: String,
    pub on_ac: bool,
}

pub fn read(battery: &crate::battery::BatteryState) -> PowerState {
    let governor = fs::read_to_string(GOVERNOR_PATH)
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();
    // `charging == true` here means the bit-7-clear branch of the CW2217
    // current register, which is the same condition lid-suspend uses to
    // detect AC. Reusing the BatteryState saves an extra I2C read.
    PowerState {
        governor,
        on_ac: battery.charging,
    }
}
