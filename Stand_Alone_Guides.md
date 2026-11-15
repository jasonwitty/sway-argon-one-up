# Standalone Guides

These are self-contained guides for specific features from the [sway-argon-one-up](https://github.com/jasonwitty/sway-argon-one-up) config. Each can be added to your own Sway setup independently without cloning or installing the full config.

---

### Lid close power management

The Raspberry Pi 5 / CM5 does not support system suspend — there is no `/sys/power/state` or `mem_sleep` interface. Attempting `systemctl suspend` will result in a black screen requiring a hard reboot.

Instead, the lid close script (`bin/lid-suspend`) performs a "soft suspend" by powering down subsystems individually. The Argon ONE UP case detects the lid switch via a GPIO line monitored by its own daemon (`argononeupd.py`), not through the standard ACPI/libinput lid switch — so sway `bindswitch` does not work here.

**What happens on lid close:**

| Action | Savings | Detail |
|--------|---------|--------|
| Lock screen | — | `swaylock -f` |
| Display off | ~1-2W | `swaymsg "output * power off"` |
| CPU governor → powersave | ~200-400mW | Scales frequency down |
| WiFi off | ~150-300mW | `rfkill block wifi` |
| Bluetooth off | ~100-200mW | `rfkill block bluetooth` |
| Webcam unbound | ~100-200mW | USB unbind by vendor ID (`11cc:2812`) |

All actions are reversed on lid open. WiFi reconnects automatically. Events are logged to `~/.local/state/lid-events.log`.

**Setup:**

**1. Configure the Argon daemon** — set `lidaction=suspend` in `/etc/argononeupd.conf`:

```
# /etc/argononeupd.conf
lidshutdownsecs=0
lidaction=suspend
```

The Argon daemon's `argonpowerbutton.py` checks this value and calls `lid-suspend close` / `lid-suspend open` accordingly.

**2. Install the script:**

```bash
cp bin/lid-suspend ~/.local/bin/
chmod +x ~/.local/bin/lid-suspend
```

**3. Add passwordless sudo** for the power management operations:

```bash
sudo tee /etc/sudoers.d/lid-power > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block bluetooth
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock bluetooth
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/unbind
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/bind
EOF
sudo visudo -cf /etc/sudoers.d/lid-power
```

**4. Idle timeout (swayidle)** — separate from the lid, this locks after 5 minutes idle and turns off the display after 10:

```
exec swayidle -w \
    timeout 300 'swaylock -f' \
    timeout 600 'swaymsg "output * power off"' resume 'swaymsg "output * power on"' \
    before-sleep 'swaylock -f'
```

**Important:** Do not configure `logind.conf` to handle the lid switch — there is no standard lid switch device on this hardware, and if logind attempts to suspend it will black-screen the system.

### Power menu

The power button in waybar opens a themed wofi menu (`bin/power-prompt`) with Power off, Restart, Suspend, Lock, Logout, and Cancel options. The **physical** power button opens the same menu via the `power-button-guard` daemon, so there is one power UI regardless of how you reach it. (`bin/powermenu` is now just a compatibility wrapper around `power-prompt`.)

**Suspend is not `systemctl suspend`** — that black-screens the Pi 5 / CM5, which cannot do real system suspend. The menu entry runs `bin/soft-suspend`, which performs the identical soft suspend a lid close does (lock, displays off, powersave governor, wifi/bluetooth rfkill'd, webcam unbound).

Because a lid close is resumed by the lid GPIO's rising edge, a menu-initiated suspend has no such edge coming and would otherwise never wake. So `soft-suspend` drops a flag in `$XDG_RUNTIME_DIR/soft-suspended`, and `power-button-guard` treats the next short press of the physical power button as "resume" rather than "show the menu". `lid-suspend open` clears the flag, so waking by lid works too and both paths leave consistent state. The flag is on tmpfs, so a hard power-off can't strand the daemon in resume-only mode.

```bash
#!/bin/bash
choice=$(printf "Lock\nReboot\nShutdown\nLogout" | wofi --dmenu --prompt "Power")

case "$choice" in
  Lock) swaylock ;;
  Reboot) systemctl reboot ;;
  Shutdown) systemctl poweroff ;;
  Logout) swaymsg exit ;;
esac
```

The waybar power button is configured to call this script on click.

### Argon battery in waybar

The Argon ONE UP has its own battery that isn't visible in `/sys/class/power_supply/` — it's accessed via a fuel gauge IC on I2C bus 1 at address `0x64`. A purpose-built Rust binary ([argon-battery-rs](argon-battery-rs/)) replaces the stock Argon Python daemon's battery polling, reducing CPU usage from ~3.5% constant to near zero.

**Features:**
- Battery percentage with level-appropriate icons
- Charging detection with distinct charging icons
- Automatic display brightness adjustment on power state transitions (100% on AC, 40% on battery)
- Automatic CPU governor switching (ondemand on AC, powersave on battery)

See the [argon-battery-rs README](argon-battery-rs/README.md) for build instructions, stock daemon changes, waybar/i3/GNOME integration guides, and customization options.

### Battery key binding

The Argon ONE UP has a battery key between F12 and Print Screen. It registers as `Pause` in Sway:

```
bindsym Pause exec foot -e sudo /usr/bin/python3 /etc/argon/argondashboard.py
```
