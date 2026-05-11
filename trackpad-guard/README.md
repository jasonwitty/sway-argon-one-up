# trackpad-guard

A lightweight Rust daemon that disables the sway touchpad while typing on USB combo keyboard/touchpad devices like the AMIRA keyboard in the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) case.

## Why this exists

sway and libinput support DWT (disable-while-typing), but it doesn't work reliably on devices that present the keyboard and touchpad as separate USB interfaces with different product IDs — libinput can't tell they're the same physical device. The AMIRA combo in the Argon ONE UP is exactly that: the keyboard and trackpad show up as unrelated USB devices. This daemon closes the gap by watching keyboard events directly and toggling `swaymsg input type:touchpad events disabled|enabled`.

## How it differs from the earlier Python version

This replaces an earlier shell-script-style Python implementation. The key functional fix is how the daemon tracks typing state.

The Python version treated `value=1` (press) and `value=2` (autorepeat) the same — both reset a "time since last keypress" timer. It never looked at `value=0` (release). If a release event got dropped (USB transient, kernel quirk), the kernel kept emitting autorepeats forever and the timer never elapsed — leaving the touchpad stuck disabled until any new key was pressed.

This version tracks a `HashSet<KeyCode>` of currently-pressed keys:

| Event | Action |
|---|---|
| value=1 (press) | Insert keycode into set, disable touchpad if not already |
| value=0 (release) | Remove from set. If set empty, start 150ms grace timer |
| value=2 (autorepeat) | Ignored — key is already in the set |

Plus a 2-second "stuck state" safety net: if any key is marked pressed but no events at all arrived for 2 seconds, force-clear the set and re-enable the touchpad. Catches missed-release events without needing any new input from the user.

## What it does

- Enumerates evdev devices matching vendor `0x6080` (AMIRA) with the exact name `AMIRA-KEYBOAR USB KEYBOARD` — the AMIRA exposes this on two USB interfaces with different product IDs, and both are watched. Name equality excludes the Mouse/Touchpad/System Control/Wireless Radio Control/Consumer Control subsystem nodes on the same device.
- Spawns a reader thread per matched device; events flow into a `mpsc` channel.
- Main loop tracks pressed-key state and calls `swaymsg input type:touchpad events disabled|enabled` on transitions.
- `SIGTERM`/`SIGINT`/`SIGHUP` handler guarantees the touchpad is re-enabled on exit so a crash can't leave it permanently disabled.
- Re-enables the touchpad on startup in case a previous run died mid-disabled.

## Prerequisites

- A Linux system with sway, running on hardware where keyboard and touchpad are separate USB interfaces (so libinput's native DWT can't associate them). The match logic is AMIRA-specific; see Customization below for other hardware.
- Rust toolchain (`rustup`).
- `input` group membership to read `/dev/input/event*`:

```bash
sudo usermod -aG input "$USER"
# Log out and back in for group change to take effect
```

- `swaymsg` on `$PATH` (comes with sway).

## Build and install

```bash
cd trackpad-guard
cargo build --release
sudo install -m 755 target/release/trackpad-guard /usr/local/bin/trackpad-guard
```

## Running as a systemd user service

A ready-made unit is included at [`systemd/trackpad-guard.service`](systemd/trackpad-guard.service):

```ini
[Unit]
Description=Disable sway touchpad while typing (AMIRA combo keyboard workaround)
After=graphical-session.target
PartOf=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/trackpad-guard
Restart=on-failure
RestartSec=2s

[Install]
WantedBy=default.target
```

Install and enable:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/trackpad-guard.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now trackpad-guard.service
```

Verify:

```bash
systemctl --user status trackpad-guard.service
journalctl --user -u trackpad-guard -f
```

## Customization

All tunables are compile-time constants at the top of `src/main.rs`:

```rust
const VENDOR_ID: u16 = 0x6080;
const KEYBOARD_NAME: &str = "AMIRA-KEYBOAR USB KEYBOARD";
const GRACE: Duration = Duration::from_millis(150);
const STUCK_TIMEOUT: Duration = Duration::from_secs(2);
```

To adapt this for a different keyboard, find your device's vendor ID and the exact name string for its bare keyboard event node:

```bash
for ev in /sys/class/input/event*; do
    echo "$(basename "$ev"): vid=$(cat "$ev/device/id/vendor") name='$(cat "$ev/device/name")'"
done
```

Then update `VENDOR_ID` and `KEYBOARD_NAME` and rebuild.

## License

Apache-2.0 — see [LICENSE](../LICENSE) at the repo root.
