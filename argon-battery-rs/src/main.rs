//! Argon ONE UP battery monitor for waybar.
//! Reads battery SOC and charging status directly via I2C.
//! Adjusts display brightness on power state transitions.

use i2cdev::core::I2CDevice as _;
use i2cdev::linux::LinuxI2CDevice;
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::process;

/// I2C bus for the Argon battery gauge.
const I2C_BUS: &str = "/dev/i2c-1";
/// I2C address of the battery gauge IC.
const ADDR_BATTERY: u16 = 0x64;
/// State-of-charge high byte register.
const SOC_HIGH_REG: u8 = 0x04;
/// Current measurement high byte register (bit 7 = discharging).
const CURRENT_HIGH_REG: u8 = 0x0E;
/// Control register — 0x00 active, 0x30 restart, 0xF0 sleep.
const REG_CONTROL: u8 = 0x08;
/// GPIO/interrupt config — 0x00 disables interrupts.
const REG_GPIOCONFIG: u8 = 0x0A;
/// SOC alert / profile-loaded flag register (bit 7 = profile loaded).
const REG_SOCALERT: u8 = 0x0B;
/// Start of 80-byte battery profile (OCV curve) at 0x10–0x5F.
const REG_PROFILE: u8 = 0x10;
/// IC ready state register — bits [3:2] non-zero = ready.
const REG_ICSTATE: u8 = 0xA7;

/// 80-byte OCV curve for the Argon ONE UP battery — required for correct SOC.
/// Sourced from Argon's argononeupd.py `battery_checkupdateprofile`, verified
/// byte-for-byte against the IC after a successful reset. See
/// `cw2217-reference.md` for details.
const PROFILE_DATA: [u8; 80] = [
    0x32, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA8, 0xAA, 0xBE, 0xC6, 0xB8, 0xAE, 0xC2, 0x98,
    0x82, 0xFF, 0xFF, 0xCA, 0x98, 0x75, 0x63, 0x55, 0x4E, 0x4C, 0x49, 0x98, 0x88, 0xDC, 0x34, 0xDB,
    0xD3, 0xD4, 0xD3, 0xD0, 0xCE, 0xCB, 0xBB, 0xE7, 0xA2, 0xC2, 0xC4, 0xAE, 0x96, 0x89, 0x80, 0x74,
    0x67, 0x63, 0x71, 0x8E, 0x9F, 0x85, 0x6F, 0x3B, 0x20, 0x00, 0xAB, 0x10, 0xFF, 0xB0, 0x73, 0x00,
    0x00, 0x00, 0x64, 0x08, 0xD3, 0x77, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFA,
];

/// Rate-limit file for self-heal attempts.
const HEAL_TS_FILE: &str = "/tmp/.argon-battery-heal";
/// Minimum seconds between self-heal attempts.
const HEAL_MIN_INTERVAL_SECS: u64 = 300;

/// I2C bus for the display DDC/CI.
const DISPLAY_I2C_BUS: &str = "/dev/i2c-14";
/// DDC/CI address for the display.
const DISPLAY_DDC_ADDR: u16 = 0x37;

/// Brightness when on AC power.
const BRIGHTNESS_AC: u8 = 100;
/// Brightness when on battery.
const BRIGHTNESS_BATTERY: u8 = 40;

/// File to track last known charging state.
const STATE_FILE: &str = "/tmp/.argon-battery-charging";
/// Consecutive new-state reads required before triggering a transition.
const PENDING_FILE: &str = "/tmp/.argon-battery-pending";
/// Number of consecutive agreeing reads required to confirm a state change.
const CONFIRM_COUNT: u8 = 3;
/// Brightness cache file (shared with brightness script).
const BRIGHTNESS_CACHE: &str = "/tmp/.brightness_level";

/// Snapshot of a single battery read including IC health.
struct BatteryReading {
    percent: u8,
    charging: bool,
    /// False when the IC is in sleep or the profile-loaded flag is clear —
    /// measurements in that state are garbage (voltage/SOC/current all 0).
    healthy: bool,
}

/// Read battery percentage, charging status, and IC health from I2C.
fn read_battery() -> Result<BatteryReading, Box<dyn core::error::Error>> {
    let mut dev = LinuxI2CDevice::new(I2C_BUS, ADDR_BATTERY)?;

    let control = dev.smbus_read_byte_data(REG_CONTROL)?;
    let soc_alert = dev.smbus_read_byte_data(REG_SOCALERT)?;
    let healthy = control == 0x00 && (soc_alert & 0x80) != 0;

    let soc = dev.smbus_read_byte_data(SOC_HIGH_REG)?;
    let percent = soc.min(100);

    let current_high = dev.smbus_read_byte_data(CURRENT_HIGH_REG)?;
    // Bit 7 set = discharging, clear = charging (Argon inverted logic).
    // At 100% SOC on AC the charge current is zero and noise tips the sign
    // bit negative (0xFF), so treat near-zero discharge as "charging".
    let charging = (current_high & 0x80) == 0 || current_high == 0xFF;

    Ok(BatteryReading {
        percent,
        charging,
        healthy,
    })
}

/// Check the rate-limit file and update it if a heal attempt is allowed.
fn heal_allowed() -> bool {
    if let Ok(meta) = fs::metadata(HEAL_TS_FILE) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed < std::time::Duration::from_secs(HEAL_MIN_INTERVAL_SECS) {
                    return false;
                }
            }
        }
    }
    let _ = fs::write(HEAL_TS_FILE, "");
    true
}

/// Full IC recovery: restart, reprogram the 80-byte OCV profile, set flags,
/// activate. Matches the sequence in Argon's `battery_checkupdateprofile`.
/// Takes ~3 seconds; callers should invoke after printing output so waybar
/// isn't blocked.
fn heal_ic() -> Result<(), Box<dyn core::error::Error>> {
    use std::thread::sleep;
    use std::time::Duration;

    let mut dev = LinuxI2CDevice::new(I2C_BUS, ADDR_BATTERY)?;

    dev.smbus_write_byte_data(REG_CONTROL, 0x30)?; // restart
    sleep(Duration::from_millis(500));
    dev.smbus_write_byte_data(REG_CONTROL, 0xF0)?; // sleep for profile write
    sleep(Duration::from_millis(500));

    for (i, byte) in PROFILE_DATA.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        dev.smbus_write_byte_data(REG_PROFILE + i as u8, *byte)?;
    }

    dev.smbus_write_byte_data(REG_SOCALERT, 0x80)?; // profile-loaded flag
    sleep(Duration::from_millis(500));
    dev.smbus_write_byte_data(REG_GPIOCONFIG, 0x00)?; // disable interrupts
    sleep(Duration::from_millis(500));

    dev.smbus_write_byte_data(REG_CONTROL, 0x30)?; // restart
    sleep(Duration::from_millis(500));
    dev.smbus_write_byte_data(REG_CONTROL, 0x00)?; // active

    for _ in 0..20 {
        sleep(Duration::from_millis(100));
        if dev.smbus_read_byte_data(REG_ICSTATE)? & 0x0C != 0 {
            return Ok(());
        }
    }
    Err("CW2217 did not become ready after reprogram".into())
}

/// Set display brightness via DDC/CI over I2C bus 14.
fn set_brightness(level: u8) {
    let Ok(file) = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(DISPLAY_I2C_BUS)
    else {
        return;
    };

    // ioctl I2C_SLAVE = 0x0703
    #[allow(clippy::cast_lossless)]
    unsafe {
        libc::ioctl(file.as_raw_fd(), 0x0703, DISPLAY_DDC_ADDR as libc::c_ulong);
    }

    let source: u8 = 0x51;
    let length: u8 = 0x84;
    let opcode: u8 = 0x03;
    let vcp_code: u8 = 0x10;
    let value_high: u8 = 0x00;
    #[allow(clippy::cast_possible_truncation)]
    let ddc_addr_byte = DISPLAY_DDC_ADDR as u8;
    let checksum = (ddc_addr_byte << 1) ^ source ^ length ^ opcode ^ vcp_code ^ value_high ^ level;

    let buf = [
        source, length, opcode, vcp_code, value_high, level, checksum,
    ];

    // Write may fail silently — that's fine for a best-effort brightness set
    let mut writer = std::io::BufWriter::new(&file);
    let _ = writer.write_all(&buf);
    let _ = writer.flush();

    // Update brightness cache so the brightness script stays in sync
    let _ = fs::write(BRIGHTNESS_CACHE, level.to_string());
}

/// Set CPU governor on all cores via sudo tee.
fn set_governor(governor: &str) {
    for entry in fs::read_dir("/sys/devices/system/cpu/")
        .into_iter()
        .flatten()
        .flatten()
    {
        let path = entry.path().join("cpufreq/scaling_governor");
        if path.exists() {
            let _ = process::Command::new("sudo")
                .args(["tee", &path.to_string_lossy()])
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::null())
                .stderr(process::Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    if let Some(ref mut stdin) = child.stdin {
                        let _ = stdin.write_all(governor.as_bytes());
                    }
                    child.wait()
                });
        }
    }
}

/// Check if charging state changed and adjust brightness/governor on transitions.
/// If the state file doesn't exist yet, power-startup hasn't finished initialising
/// — skip transition handling so we don't race against it.
/// Requires CONFIRM_COUNT consecutive reads in the new state before acting,
/// which filters out transient I2C misreads.
fn handle_power_transition(charging: bool) {
    let previous = match fs::read_to_string(STATE_FILE) {
        Ok(val) => val.trim().parse::<u8>().ok(),
        Err(_) => return, // state file missing — power-startup hasn't run yet
    };

    let current_state: u8 = u8::from(charging);

    if previous == Some(current_state) {
        // State unchanged — clear any pending transition counter
        let _ = fs::remove_file(PENDING_FILE);
        return;
    }

    // State differs — increment the pending counter
    let pending_count = fs::read_to_string(PENDING_FILE)
        .ok()
        .and_then(|v| v.trim().parse::<u8>().ok())
        .unwrap_or(0)
        + 1;

    if pending_count >= CONFIRM_COUNT {
        // Confirmed transition — apply brightness and governor
        let brightness = if charging {
            BRIGHTNESS_AC
        } else {
            BRIGHTNESS_BATTERY
        };
        set_brightness(brightness);
        set_governor(if charging { "ondemand" } else { "powersave" });
        let _ = fs::write(STATE_FILE, current_state.to_string());
        let _ = fs::remove_file(PENDING_FILE);
    } else {
        // Not yet confirmed — record the count and wait for next poll
        let _ = fs::write(PENDING_FILE, pending_count.to_string());
    }
}

fn main() {
    let Ok(reading) = read_battery() else {
        println!(
            r#"{{"text": "{} ?%", "tooltip": "Battery status unavailable", "class": "unknown"}}"#,
            "\u{f0079}"
        );
        process::exit(0);
    };

    if !reading.healthy {
        // IC is asleep or profile flag clear — measurements are garbage.
        // Show unknown, then attempt a rate-limited heal.
        println!(
            r#"{{"text": "{} ?%", "tooltip": "Fuel gauge recovering…", "class": "unknown"}}"#,
            "\u{f0079}"
        );
        if heal_allowed() {
            let _ = heal_ic();
        }
        return;
    }

    handle_power_transition(reading.charging);

    let (icon, class) = if reading.charging {
        let icon = match reading.percent {
            80..=100 => "\u{f0085}",
            60..=79 => "\u{f0084}",
            40..=59 => "\u{f0088}",
            20..=39 => "\u{f0086}",
            _ => "\u{f089f}",
        };
        (icon, "charging")
    } else {
        match reading.percent {
            80..=100 => ("\u{f0079}", "good"),
            60..=79 => ("\u{f007f}", "good"),
            40..=59 => ("\u{f007c}", "moderate"),
            20..=39 => ("\u{f007a}", "warning"),
            _ => ("\u{f0083}", "critical"),
        }
    };

    let status = if reading.charging {
        "Charging"
    } else {
        "Discharging"
    };
    let governor = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .unwrap_or_default();
    let governor = governor.trim();
    let mode = match governor {
        "powersave" => "powersave",
        "performance" => "performance",
        _ => "balanced",
    };
    let percent = reading.percent;

    println!(
        r#"{{"text": "{icon} {percent}%", "tooltip": "Argon Battery: {status} {percent}%\nCPU: {mode}", "class": "{class}"}}"#
    );
}
