# argon-lid-monitor

A lightweight Rust lid-open/close monitor for the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) laptop. Watches the lid-sensor GPIO directly via the kernel's `/dev/gpiochip0` character-device interface and invokes a user-provided hook script on each edge.

## Why this exists

The Argon ONE UP's lid hall-effect sensor isn't exposed through standard Linux ACPI — there is no `/proc/acpi/button/lid/*` for logind to watch. The only way to react to lid motion is to poll GPIO line 27 on `/dev/gpiochip0` directly. Argon's stock Python daemon (`argononeupd.py`) does this via a monitor thread (`argonpowerbutton_monitorlid`) that requires the whole daemon to be running. This tool replaces just that thread with a standalone ~70-line Rust binary that runs as its own systemd service.

**Comparison with the stock Argon daemon's lid thread:**

| | Stock Python thread | argon-lid-monitor |
|---|---|---|
| Runtime | Part of `argononeupd.py` | Standalone binary |
| Dependency | Whole Argon daemon + Python | `gpiocdev` crate only |
| Startup | Pulls in I2C/battery/etc. | Single GPIO request, one syscall loop |
| Debounce | None | 100ms after each edge |
| Licensing | Argon's (unlicensed) | Apache-2.0 |

## What it does

- Opens `/dev/gpiochip0` line 27 as an input with pull-up bias and both-edge detection.
- Blocks on edge events. On `Falling` (lid closing, magnet approaches sensor) runs `$HOME/.local/bin/lid-suspend close`; on `Rising` (lid opening) runs `$HOME/.local/bin/lid-suspend open`.
- Sleeps 100ms after each event as debounce in case the hall-effect sensor chatters near the threshold.
- Logs to stderr — read via `journalctl --user -u argon-lid-monitor`.

The binary doesn't implement any power-saving logic itself — all of that lives in your `lid-suspend` script. That separation lets you change close/open behavior (screen off, rfkill, USB unbind, etc.) without recompiling.

## Prerequisites

- Raspberry Pi Compute Module 5 in an Argon ONE UP case (or any board where the lid sensor is wired to `/dev/gpiochip0` line 27 — adjust `GPIO_CHIP` and `LID_LINE` in `src/main.rs` for other hardware).
- Rust toolchain (`rustup`).
- GPIO access — your user must be in the `gpio` group:

```bash
sudo usermod -aG gpio "$USER"
# Log out and back in for group change to take effect
```

- A `lid-suspend` handler script at `$HOME/.local/bin/lid-suspend` that accepts `open` and `close` subcommands. A minimal version:

```bash
#!/bin/bash
case "$1" in
    close) swaylock -f & swaymsg "output * power off" ;;
    open)  swaymsg "output * power on" ;;
esac
```

See the [parent repo's `bin/lid-suspend`](../bin/lid-suspend) for a fuller reference that also handles the CPU governor, rfkill, and USB webcam unbind.

## Build and install

```bash
cd argon-lid-monitor
cargo build --release
sudo cp target/release/argon-lid-monitor /usr/local/bin/
```

## Running as a systemd user service

A ready-made unit is included at [`systemd/argon-lid-monitor.service`](systemd/argon-lid-monitor.service):

```ini
[Unit]
Description=Argon ONE UP lid monitor (Rust)
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/local/bin/argon-lid-monitor
Restart=on-failure
RestartSec=2s

[Install]
WantedBy=default.target
```

Install and enable it:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/argon-lid-monitor.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now argon-lid-monitor.service
```

Verify:

```bash
systemctl --user status argon-lid-monitor.service
journalctl --user -u argon-lid-monitor -f
```

## Disabling Argon's stock lid thread

Argon's daemon will keep its own lid monitor running alongside this one and both will fire — duplicate `lid-suspend` invocations and edge races. Disable the Argon lid thread by commenting out `t2` and `t2.start()` in `/etc/argon/argononeupd.py`'s SERVICE section:

```python
# t2 = Thread(target = argonpowerbutton_monitorlid, args =(ipcq, ))
# t2.start()
```

Then restart: `sudo systemctl restart argononeupd.service`. If you've also disabled the battery thread (per [argon-battery-rs](../argon-battery-rs/)), the SERVICE path has no threads left to start — you can disable `argononeupd.service` entirely.

## Customization

All tunables are compile-time constants at the top of `src/main.rs`:

```rust
const GPIO_CHIP: &str = "/dev/gpiochip0";
const LID_LINE: u32 = 27;
const DEBOUNCE: Duration = Duration::from_millis(100);
```

To change the hook script path, edit `run_lid_suspend()` and rebuild.

## License

Apache-2.0 — see [LICENSE](../LICENSE) at the repo root.
