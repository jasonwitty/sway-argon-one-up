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

/// I2C bus for the display DDC/CI.
const DISPLAY_I2C_BUS: &str = "/dev/i2c-14";
/// DDC/CI address for the display.
const DISPLAY_DDC_ADDR: u16 = 0x37;

/// Brightness when on AC power.
const BRIGHTNESS_AC: u8 = 100;
/// Brightness when on battery.
const BRIGHTNESS_BATTERY: u8 = 60;

/// File to track last known charging state.
const STATE_FILE: &str = "/tmp/.argon-battery-charging";
/// Brightness cache file (shared with brightness script).
const BRIGHTNESS_CACHE: &str = "/tmp/.brightness_level";

/// Read battery percentage and charging status from I2C.
fn read_battery() -> Result<(u8, bool), Box<dyn core::error::Error>> {
    let mut dev = LinuxI2CDevice::new(I2C_BUS, ADDR_BATTERY)?;

    let soc = dev.smbus_read_byte_data(SOC_HIGH_REG)?;
    let percent = soc.min(100);

    let current_high = dev.smbus_read_byte_data(CURRENT_HIGH_REG)?;
    // Bit 7 set = discharging, clear = charging (Argon inverted logic)
    let charging = (current_high & 0x80) == 0;

    Ok((percent, charging))
}

/// Set display brightness via DDC/CI over I2C bus 14.
fn set_brightness(level: u8) {
    let Ok(file) = fs::OpenOptions::new().read(true).write(true).open(DISPLAY_I2C_BUS) else {
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
    let checksum = (ddc_addr_byte << 1)
        ^ source
        ^ length
        ^ opcode
        ^ vcp_code
        ^ value_high
        ^ level;

    let buf = [source, length, opcode, vcp_code, value_high, level, checksum];

    // Write may fail silently — that's fine for a best-effort brightness set
    let mut writer = std::io::BufWriter::new(&file);
    let _ = writer.write_all(&buf);
    let _ = writer.flush();

    // Update brightness cache so the brightness script stays in sync
    let _ = fs::write(BRIGHTNESS_CACHE, level.to_string());
}

/// Check if charging state changed and adjust brightness on transitions.
fn handle_power_transition(charging: bool) {
    let previous = fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|val| val.trim().parse::<u8>().ok());

    let current_state: u8 = u8::from(charging);

    if previous != Some(current_state) {
        let brightness = if charging { BRIGHTNESS_AC } else { BRIGHTNESS_BATTERY };
        set_brightness(brightness);
        let _ = fs::write(STATE_FILE, current_state.to_string());
    }
}

fn main() {
    let Ok((percent, charging)) = read_battery() else {
        println!(
            "{}",
            format_args!(
                r#"{{"text": "{} ?%", "tooltip": "Battery status unavailable", "class": "unknown"}}"#,
                "\u{f0079}"
            )
        );
        process::exit(0);
    };

    handle_power_transition(charging);

    let (icon, class) = if charging {
        let icon = match percent {
            80..=100 => "\u{f0085}",
            60..=79 => "\u{f0084}",
            40..=59 => "\u{f0088}",
            20..=39 => "\u{f0086}",
            _ => "\u{f089f}",
        };
        (icon, "charging")
    } else {
        match percent {
            80..=100 => ("\u{f0079}", "good"),
            60..=79 => ("\u{f007f}", "good"),
            40..=59 => ("\u{f007c}", "moderate"),
            20..=39 => ("\u{f007a}", "warning"),
            _ => ("\u{f0083}", "critical"),
        }
    };

    let status = if charging { "Charging" } else { "Discharging" };

    println!(
        r#"{{"text": "{icon} {percent}%", "tooltip": "Argon Battery: {status} {percent}%", "class": "{class}"}}"#
    );
}
