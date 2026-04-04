# argon-battery-rs

![argon-battery-rs in waybar](screenshot.png)

A lightweight Rust battery monitor for the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) laptop. Reads battery state-of-charge and charging status directly from the battery gauge IC over I2C, outputting JSON suitable for waybar, i3status-rs, or any bar that accepts JSON custom modules.

## Why this exists

The Argon ONE UP has an internal battery that isn't exposed through `/sys/class/power_supply/` — it's accessed via a fuel gauge IC on I2C bus 1 at address `0x64`. Argon's stock Python daemon (`argononeupd.py`) polls this in a tight loop with no sleep, burning 3-4% CPU constantly. This tool replaces that battery polling with a single-shot binary that reads the gauge, prints one line of JSON, and exits — designed to be called by waybar (or similar) on an interval.

**Comparison with the stock Argon daemon:**

| | Stock Python daemon | argon-battery-rs |
|---|---|---|
| CPU usage | ~3.5% constant | ~0% (runs for ~22ms per poll) |
| Poll interval | As fast as possible | Configurable (default: 1s via waybar) |
| Output | Writes to `/dev/shm/upslog.txt` | JSON to stdout |
| Brightness control | None | Auto-adjusts on AC/battery transitions |
| CPU governor | None | Auto-switches on AC/battery transitions |

## Features

- Battery percentage with appropriate icons per charge level
- Charging detection with distinct icons
- Automatic display brightness adjustment via DDC/CI on power state transitions (100% on AC, 40% on battery)
- Automatic CPU governor switching on power state transitions (ondemand on AC, powersave on battery)
- Single binary, no runtime dependencies

## Prerequisites

- Raspberry Pi Compute Module 5 in an Argon ONE UP case
- Rust toolchain (`rustup`)
- I2C access — your user must be in the `i2c` group:

```bash
sudo usermod -aG i2c "$USER"
# Log out and back in for group change to take effect
```

- For CPU governor switching, add a sudoers entry (or run waybar as a user with write access to the sysfs governor files):

```bash
sudo tee /etc/sudoers.d/cpu-governor > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
EOF
```

## Build and install

```bash
cd argon-battery-rs
cargo build --release
sudo cp target/release/argon-battery-rs /usr/local/bin/
```

## Disabling stock Argon battery polling

The stock Argon daemon handles battery monitoring, lid switch, fan control, and keyboard. You only need to disable the battery polling thread — the rest should continue to run.

In `/etc/argon/argononeupd.py`, find the SERVICE section near the bottom and comment out the battery thread:

```python
# Before
t1 = Thread(target = battery_check, args =(ipcq, ))
t2 = Thread(target = argonpowerbutton_monitorlid, args =(ipcq, ))
t1.start()
t2.start()

# After
# t1 = Thread(target = battery_check, args =(ipcq, ))
t2 = Thread(target = argonpowerbutton_monitorlid, args =(ipcq, ))
# t1.start()
t2.start()
```

Then restart the daemon:

```bash
sudo systemctl restart argononeupd.service
```

## Output format

Each invocation prints a single line of JSON:

```json
{"text": "󰁼 50%", "tooltip": "Argon Battery: Discharging 50%", "class": "moderate"}
```

**Classes** (for styling):
- `charging` — plugged in
- `good` — 60-100% on battery
- `moderate` — 40-59% on battery
- `warning` — 20-39% on battery
- `critical` — below 20% on battery

## Waybar integration

Add a custom module to your waybar `config`:

```json
"custom/argon-battery": {
    "exec": "/usr/local/bin/argon-battery-rs",
    "return-type": "json",
    "interval": 1,
    "tooltip": true
}
```

Add it to your modules list (e.g. `modules-right`):

```json
"modules-right": ["custom/argon-battery"]
```

Style it in `style.css`:

```css
#custom-argon-battery {
    color: #a6d189;
}
#custom-argon-battery.warning {
    color: #ef9f76;
}
#custom-argon-battery.critical {
    color: #e78284;
}
#custom-argon-battery.charging {
    color: #e5c890;
}
```

## Other desktop environments

### i3 / i3status-rs

Add a custom block to your i3status-rs config:

```toml
[[block]]
block = "custom"
command = "/usr/local/bin/argon-battery-rs"
json = true
interval = 1
```

### GNOME / other GTK desktops

For desktops without a JSON-based bar, you can use a simple wrapper script to extract the text:

```bash
#!/bin/bash
# argon-battery-text — outputs plain text battery status
/usr/local/bin/argon-battery-rs | python3 -c "import sys,json; print(json.load(sys.stdin)['text'])"
```

This can be used with GNOME extensions like [Executor](https://extensions.gnome.org/extension/2932/executor/) or similar custom command widgets.

### Generic polling script

For any environment, you can poll in a loop:

```bash
#!/bin/bash
while true; do
    /usr/local/bin/argon-battery-rs
    sleep 1
done
```

## Customization

The brightness levels and CPU governors are compile-time constants in `src/main.rs`:

```rust
const BRIGHTNESS_AC: u8 = 100;      // Brightness when plugged in (0-100)
const BRIGHTNESS_BATTERY: u8 = 40;  // Brightness when on battery (0-100)
```

The CPU governor is set to `ondemand` on AC and `powersave` on battery. To change these, edit the `handle_power_transition` function and rebuild.

## License

MIT
