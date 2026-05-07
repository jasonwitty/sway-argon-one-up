# argon-fan

A small Rust daemon for controlling the PWM fan on the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) (and any other Raspberry Pi 5 with a kernel-managed PWM fan exposed via `hwmon`). Switch between fan modes — Silent, Normal, Turbo, Full — each with its own temperature/PWM curve. Includes a waybar custom-module mode and a wofi-based picker.

## Why this exists

The Pi 5's stock fan management is the kernel `pwm-fan` driver wired up through device-tree thermal trip points. It works fine, but you can't easily swap between "as quiet as possible" and "maximum airflow" without rebuilding the device tree.

`argon-fan` takes over `/sys/class/hwmon/<pwmfan>/pwm1_enable` (sets it to `1` for manual control) and applies a configurable temperature → PWM curve. On clean shutdown it restores `pwm1_enable=2` so the kernel governor takes back over.

## Hardware requirements

- Raspberry Pi 5, OR any system that exposes a fan as a hwmon device named `pwmfan` with `pwm1` and `pwm1_enable` attributes
- For full integration: an [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) case (the daemon itself works on any Pi 5 — the "Argon" in the name is just the case it was developed against)

To verify your hardware exposes a compatible fan:

```bash
for f in /sys/class/hwmon/*/name; do echo "$f: $(cat $f)"; done | grep pwmfan
```

You should see something like `hwmon3: pwmfan`. If you don't, this tool won't find the fan.

## Software requirements

- Rust toolchain (`rustup`) to build
- `systemd` to run the daemon as a service
- `wofi` for the mode picker (optional — only needed if you use `argon-fan picker`)
- `micro` and a terminal emulator (default: `foot`, override with `$TERMINAL`) for `argon-fan edit-config` (optional)
- A nerd font in your bar (e.g. JetBrainsMono Nerd Font) so the mode glyphs render

## Build and install

```bash
cd argon-fan
cargo build --release
sudo install -m 0755 target/release/argon-fan /usr/local/bin/argon-fan
```

### Sudoers entry (so non-root users can switch modes)

The daemon must run as root because writing to `pwm1` requires it. The CLI subcommands `set` and `edit-config` write to `/etc/argon-fan/config.json` via `sudo -n /usr/bin/tee`. Drop a sudoers rule allowing this single invocation to be passwordless:

```bash
sudo tee /etc/sudoers.d/argon-fan > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/argon-fan/config.json
EOF
sudo visudo -cf /etc/sudoers.d/argon-fan
```

Without this rule, mode switches will fail unless you have broader passwordless sudo configured.

### systemd unit

```bash
sudo tee /etc/systemd/system/argon-fan.service > /dev/null <<'EOF'
[Unit]
Description=Argon ONE UP fan control daemon
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/local/bin/argon-fan daemon
Restart=on-failure
RestartSec=2
KillMode=process
TimeoutStopSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now argon-fan
```

On first start the daemon will create `/etc/argon-fan/config.json` with default curves.

## Usage

```
argon-fan daemon          # the control loop (run by systemd)
argon-fan set <mode>      # switch active mode (silent | normal | turbo | full)
argon-fan status          # print current state JSON
argon-fan picker          # wofi-based mode picker
argon-fan edit-config     # open config in micro (validates JSON before saving)
argon-fan waybar          # emit one line of JSON for waybar
```

Mode switches propagate to the running daemon via config-file mtime polling — there's a ≤2s lag between `argon-fan set turbo` and the daemon applying the new curve.

## Configuration

`/etc/argon-fan/config.json` is the single source of truth. The four default modes look roughly like this:

```json
{
  "active_mode": "normal",
  "poll_interval_sec": 2,
  "modes": {
    "silent":  { "curve": [{"temp_c": 60, "pwm": 0}, {"temp_c": 85, "pwm": 217}] },
    "normal":  { "curve": [{"temp_c": 50, "pwm": 0}, {"temp_c": 75, "pwm": 255}] },
    "turbo":   { "curve": [{"temp_c": 45, "pwm": 0}, {"temp_c": 65, "pwm": 255}] },
    "full":    { "curve": [{"temp_c": 0,  "pwm": 255}] }
  }
}
```

Each curve is a list of `(temp_c, pwm)` points; the daemon linearly interpolates between them. Temps below the first point return its PWM; temps above the last point return its PWM. PWM range is `0`–`255` (8-bit duty cycle, the Linux hwmon convention).

You can add your own modes — they automatically become available to `argon-fan set` and the picker.

## Waybar integration

### Module config

```json
"custom/argon-fan": {
  "exec": "/usr/local/bin/argon-fan waybar",
  "return-type": "json",
  "interval": 3,
  "tooltip": true,
  "on-click": "/usr/local/bin/argon-fan picker",
  "on-click-right": "/usr/local/bin/argon-fan edit-config"
}
```

Add `"custom/argon-fan"` somewhere in `modules-left` / `modules-center` / `modules-right`.

### Output format

```json
{"text": "󰈐 51°C", "tooltip": "Mode: Normal\nPWM: 75/255 (29%)\nTemp: 51°C", "class": "normal"}
```

When the daemon isn't running:

```json
{"text": "󰈎 ?", "tooltip": "argon-fan daemon not running", "class": "offline"}
```

### CSS classes

Each mode is exposed as a class so you can colour it however you want:

```css
#custom-argon-fan          { color: #81c8be; }   /* fallback */
#custom-argon-fan.silent   { color: #babbf1; }
#custom-argon-fan.normal   { color: #81c8be; }
#custom-argon-fan.turbo    { color: #e5c890; }
#custom-argon-fan.full     { color: #ef9f76; }
#custom-argon-fan.offline  { color: #414559; }
```

### Mode glyphs

These are nerd-font Material Design Icons. Make sure a nerd font is set in your bar:

| Mode    | Glyph | Codepoint                                  |
|---------|-------|--------------------------------------------|
| silent  | 󰪟    | U+F0A9F (`nf-md-fan_off`)                  |
| normal  | 󰈐    | U+F0210 (`nf-md-fan`)                      |
| turbo   | 󰇻    | U+F01FB                                    |
| full    | 󱪈    | U+F1A88                                    |
| offline | 󰈎    | U+F020E (when daemon isn't running)        |

## Other bars / desktops

Anything that accepts JSON `{text, tooltip, class}` from a polled command works the same way — replace the waybar `exec` with the equivalent in your bar.

For plain-text bars:

```bash
argon-fan waybar | jq -r .text
```

## How it works

- On startup the daemon scans `/sys/class/hwmon/*/name` for `pwmfan` and pins to that hwmon path
- Reads CPU temp from `/sys/class/thermal/thermal_zone0/temp` every `poll_interval_sec` seconds
- Stats `/etc/argon-fan/config.json` each iteration; if mtime changed, reloads the config (this is how user-level mode switches reach the root daemon — non-root users can't signal a root process)
- Writes the computed PWM to `pwm1` and the live state (mode, pwm, temp) to `/dev/shm/argon-fan.json` for the waybar module to read
- On `SIGTERM` / `SIGINT`: restores `pwm1_enable=2` (kernel governor) and exits

## Testing

```bash
cargo test
```

Tests cover the curve-lerp logic and pure helpers. The hardware integration is verified manually.

## License

Apache-2.0
