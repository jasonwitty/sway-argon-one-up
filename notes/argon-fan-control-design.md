# Argon Fan Control — Design Notes

## Current Hardware State

- **Fan type**: PWM fan on RPi 5 GPIO, managed by kernel `pwm-fan` driver
- **NOT I2C**: The Argon ONE UP does not use the I2C MCU at `0x1a` for fan control (that was the older Argon ONE non-UP). Address `0x1a` is unreachable on this system.
- **PWM sysfs path**: `/sys/class/hwmon/hwmon3/pwm1` (0–255 range)
  - Discover at runtime by scanning for `name == "pwmfan"` (hwmon number can change across reboots)
- **Fan RPM**: `fan1_input` reads 0 — no tachometer wired
- **Temp source**: `/sys/class/thermal/thermal_zone0/temp` (millidegrees C)
- **Cooling device**: `/sys/class/thermal/cooling_device0` — 5 states (0–4)

### Current Kernel Device Tree Curve

| Trip Point | Temp   | Hysteresis | PWM Value | Fan % |
|------------|--------|------------|-----------|-------|
| Off        | < 50°C | —          | 0         | 0%    |
| Tepid      | 50°C   | 5°C        | 75        | ~29%  |
| Warm       | 60°C   | 5°C        | 125       | ~49%  |
| Hot        | 67.5°C | 5°C        | 175       | ~69%  |
| Very Hot   | 75°C   | 5°C        | 250       | ~98%  |
| Critical   | 110°C  | —          | (shutdown) | —    |

Governor: `step_wise`. No custom dtoverlay — this is the Pi 5 default.

### Dead Code

`/etc/argon/argonregister.py` has `argonregister_setfanspeed()` and related functions that write to I2C `0x1a`. These are unused on the Argon ONE UP hardware.

The original `argononed.py` fan daemon (referenced in `/etc/argon/argon-uninstall.sh`) was never installed or has been removed.

---

## Proposed Architecture: `argon-fan`

New standalone Rust application in `sway-help/argon-fan/`.

```
waybar: custom/argon-fan
"icon Mode Temp"  [click -> popup, right-click -> cycle mode]
        |
        | reads
        v
/dev/shm/argon-fan.json  (shared state file)
        ^                         |
        | writes                  | reads
        |                         v
argon-fan daemon            argon-fan popup
(systemd service)           (GTK4 window, on-demand)
        |                         |
   sysfs PWM                socktop_connector (optional)
   hwmon/pwmfan/pwm1        ws://localhost:3000
```

### Subcommands

```
argon-fan daemon          # systemd service — poll temp, apply PWM curve, write state
argon-fan waybar          # output JSON for waybar module
argon-fan set <mode>      # set mode: silent | normal | turbo | full
argon-fan set next        # cycle to next mode
argon-fan status          # print current state JSON
argon-fan popup           # launch GTK4 popup window
```

### Fan Modes & Curves

| Mode       | Off Below | Ramp Range | Max At | Use Case                  |
|------------|-----------|------------|--------|---------------------------|
| **Silent** | 60°C      | 60–80°C    | 85°C   | Quiet work, battery       |
| **Normal** | 50°C      | 50–75°C    | 75°C   | Everyday use (≈ current)  |
| **Turbo**  | never     | 45–65°C    | 65°C   | Compiling, heavy loads    |
| **Full**   | never     | —          | always | 100% PWM, benchmarks      |

Curves should use smooth linear interpolation, not step jumps.

### Waybar Module

Config:
```json
"custom/argon-fan": {
  "exec": "argon-fan waybar",
  "return-type": "json",
  "interval": 3,
  "tooltip": true,
  "on-click": "argon-fan popup",
  "on-click-right": "argon-fan set next"
}
```

Output format:
```json
{
  "text": "󰈐 Normal 51°C",
  "tooltip": "Fan: Normal\nPWM: 75/255 (29%)\nTemp: 51°C",
  "class": "normal"
}
```

CSS classes: `silent`, `normal`, `turbo`, `full`

### GTK4 Popup (on-click)

- Animated spinning fan icon — rotation speed proportional to current PWM %
- Four mode buttons (radio/segmented control)
- Temperature history line graph (Cairo), last 60 seconds
- Temp logging starts when popup opens, stops on close (no background logging)
- Optionally pulls richer data (per-core CPU, etc.) via socktop_connector if agent is running; falls back to sysfs-only gracefully

### Daemon Details

- Polls temp every 2 seconds
- Reads mode from `/dev/shm/argon-fan.json`
- Computes PWM from mode curve via linear interpolation
- Writes PWM to sysfs
- Updates state file with: mode, current_pwm, current_temp, timestamp
- Must set `pwm1_enable = 1` (manual mode) on startup to take over from kernel governor
- Should restore `pwm1_enable = 2` (automatic/kernel) on clean shutdown
- Systemd service: `Type=simple`, `ExecStart=/usr/local/bin/argon-fan daemon`

### socktop Integration

socktop_connector provides `cpu_temp_c`, per-core CPU usage, and more via WebSocket (`ws://localhost:3000/ws`). The agent isn't always running locally, so:

- **Daemon**: always reads temp from sysfs (zero dependencies, always available)
- **Popup**: tries socktop_connector for enriched display data; falls back gracefully

Request format: `get_metrics` -> JSON with `cpu_temp_c: Option<f32>`, `cpu_per_core: Vec<f32>`, etc.

### Dependencies (Cargo.toml)

- `i2cdev` or raw sysfs file I/O (sysfs is simpler here)
- `serde`, `serde_json` — state file + waybar JSON
- `gtk4` (feature-gated or separate binary) — popup UI
- `cairo-rs` — graph rendering
- `socktop_connector` (optional) — enriched data
- `clap` — subcommand parsing

---

## Open Decisions

1. **Popup framework**: `gtk4-rs` fits Sway/Wayland + existing GTK theming. Could do a simpler v1 with wofi mode picker and add GTK popup later.

2. **Single binary vs split**: One binary with feature-gated GTK (keeps install simple) vs separate `argon-fan` + `argon-fan-popup` (avoids GTK dependency for the daemon).

3. **Battery-aware behavior**: Could auto-switch to Silent mode on battery (tie into argon-battery-rs state). Or keep them independent.

4. **hwmon path discovery**: Scan `/sys/class/hwmon/*/name` for `"pwmfan"` at daemon startup rather than hardcoding `hwmon3`.

5. **Config file vs state-only**: Could add `~/.config/argon-fan/config.toml` for custom curves, default mode, etc. Or keep it simple with just the four preset modes.

6. **Kernel governor handoff**: When daemon starts, it takes over PWM. If daemon crashes, fan stays at last-set speed. Consider a watchdog or reverting to kernel control on failure.
