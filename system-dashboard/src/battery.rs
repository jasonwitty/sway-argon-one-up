//! CW2217 fuel gauge readouts. Mirrors argon-battery-rs's I2C access plus
//! a current-register read for the time-remaining estimate.

use i2cdev::core::I2CDevice as _;
use i2cdev::linux::LinuxI2CDevice;
use serde::Serialize;

const I2C_BUS: &str = "/dev/i2c-1";
const ADDR_BATTERY: u16 = 0x64;
const REG_SOC_HIGH: u8 = 0x04;
const REG_CURRENT_HIGH: u8 = 0x0e;

/// Nominal Argon ONE UP battery capacity in mAh. Used as a constant for
/// the time-remaining estimate — we don't read it back from the gauge.
const BATTERY_CAPACITY_MAH: u32 = 4500;

/// Approximate scale factor: each LSB of the signed current high byte
/// corresponds to ~20 mA on this board. Empirical, matches the order of
/// magnitude reported by `i2cget`.
const CURRENT_SCALE_MA_PER_LSB: i32 = 20;

#[derive(Serialize, Clone, Debug, Default)]
pub struct BatteryState {
    pub percent: u8,
    pub charging: bool,
    pub time_remaining_min: Option<u32>,
    pub current_ma: Option<i32>,
}

fn current_ma(high: u8) -> i32 {
    // Treat the high byte as a signed 8-bit integer; positive = discharging.
    (high as i8 as i32) * CURRENT_SCALE_MA_PER_LSB
}

pub fn read() -> BatteryState {
    let mut dev = match LinuxI2CDevice::new(I2C_BUS, ADDR_BATTERY) {
        Ok(d) => d,
        Err(_) => return BatteryState::default(),
    };

    let soc = dev.smbus_read_byte_data(REG_SOC_HIGH).unwrap_or(0).min(100);
    let cur_high = dev.smbus_read_byte_data(REG_CURRENT_HIGH).ok();

    // CW2217 sign convention on this hardware: bit 7 set = discharging,
    // clear = charging. At ~100% SOC on AC the charge current floats near
    // zero and the sign bit flips noisily, so treat 0xff as charging too.
    let charging = match cur_high {
        Some(h) => (h & 0x80) == 0 || h == 0xff,
        None => false,
    };

    let current = cur_high.map(current_ma);

    let time_remaining_min = if !charging {
        cur_high.and_then(|h| {
            let draw = current_ma(h).unsigned_abs();
            if draw < 50 {
                return None;
            }
            let remaining_mah = u32::from(soc) * BATTERY_CAPACITY_MAH / 100;
            Some(remaining_mah * 60 / draw)
        })
    } else {
        None
    };

    BatteryState {
        percent: soc,
        charging,
        time_remaining_min,
        current_ma: current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_ma_sign_convention() {
        assert_eq!(current_ma(0x00), 0);
        assert_eq!(current_ma(0x10), 16 * CURRENT_SCALE_MA_PER_LSB);
        assert_eq!(current_ma(0xff), -CURRENT_SCALE_MA_PER_LSB);
        assert_eq!(current_ma(0x80), -128 * CURRENT_SCALE_MA_PER_LSB);
    }
}
