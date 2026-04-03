# sway-argon-one-up

Sway window manager configuration for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system), a 14-inch laptop powered by the Raspberry Pi Compute Module 5. Includes Catppuccin Frappe theming, a dynamic keybinding help overlay, instant brightness control via direct I2C, and Claude Code integration.

![screenshot](screenshot.png)

## Hardware

This config is built for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) which uses a Raspberry Pi Compute Module 5. The display is connected via HDMI internally, so standard backlight controls don't apply — brightness is controlled by writing DDC/CI commands directly to the display over I2C bus 14, achieving ~30ms response time. This approach was inspired by [esvertit's display calibration guide](https://forum.argon40.com/t/guide-professional-display-calibration-on-argon-one-up/9188) on the Argon40 forum, which documented the display's I2C interface and DDC/CI capabilities. The Argon case also has its own battery, monitored via a custom script.

## What's included

| Directory | Description |
|-----------|-------------|
| `sway/` | Sway config with Catppuccin Frappe window colors, idle lock, touchpad, media keys |
| `waybar/` | Top bar with workspaces, clock, CPU, volume, backlight, Argon battery, tray, Claude + help + power buttons |
| `wob/` | Wayland Overlay Bar config for brightness/volume indicators |
| `wofi/` | App launcher and help overlay styles |
| `foot/` | Terminal emulator with Frappe 16-color palette |
| `mako/` | Notification daemon themed to match |
| `swaylock/` | Lock screen with Frappe colored ring indicator |
| `gtk-3.0/` | GTK dark theme settings |
| `bin/` | `sway-help`, `claude-prompt`, `brightness`, `start-wob`, `argon-battery`, `lid-suspend`, `powermenu` scripts |

## Media keys

| Key | Action |
|-----|--------|
| **Fn+F2** | Brightness down (direct I2C, ~30ms) |
| **Fn+F3** | Brightness up |
| **Fn+F6** | Mute/unmute |
| **Fn+F7** | Volume down |
| **Fn+F8** | Volume up |
| **Battery key** | Open Argon battery dashboard |

All media keys show a visual indicator via wob (Wayland Overlay Bar).

## sway-help

The help overlay (`bin/sway-help`) parses your sway config every time it runs, so it always reflects your current keybindings. Access it via:

- **Mod+Shift+H** (keyboard shortcut)
- Click the keyboard icon in waybar

Type to filter, Escape to dismiss.

## Claude Code integration

Launch Claude Code directly from Sway:

| Binding | Action |
|---------|--------|
| **Mod+C** | Open Claude in a foot terminal |
| **Mod+Shift+C** | Quick prompt — wofi popup, type a question, Claude opens with it |
| **Waybar icon** | Left-click opens Claude, right-click opens quick prompt |

`claude-prompt` opens a minimal wofi input, takes your question, and launches Claude in foot with that prompt. The terminal stays open after Claude responds so you can continue the conversation.

---

## Standalone guides

These sections explain specific features in detail so you can add them to your own setup without cloning the full config.

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

The power button in waybar opens a wofi menu (`bin/powermenu`) with Lock, Reboot, Shutdown, and Logout options. Suspend is intentionally excluded — the Pi 5 / CM5 does not support system suspend and attempting it will black-screen the system.

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

The Argon ONE UP has its own battery that isn't visible in `/sys/class/power_supply/` — it's accessed through the Argon daemon. To show battery percentage in waybar:

**1. Install the Argon config tool** (if not already):

```bash
curl https://download.argon40.com/argononeup.sh | bash
```

**2. Create the battery script** at `~/.local/bin/argon-battery`:

```bash
#!/bin/bash
output=$(sudo /usr/bin/python3 /etc/argon/argononeupd.py GETBATTERY 2>/dev/null)
percent=$(echo "$output" | grep -oP '\d+')

if [ -z "$percent" ]; then
    echo '{"text": "?%", "tooltip": "Battery status unavailable", "class": "unknown"}'
    exit 0
fi

if [ "$percent" -ge 80 ]; then class="good"
elif [ "$percent" -ge 40 ]; then class="moderate"
elif [ "$percent" -ge 20 ]; then class="warning"
else class="critical"
fi

echo "{\"text\": \"$percent%\", \"tooltip\": \"Argon Battery: $percent%\", \"class\": \"$class\"}"
```

```bash
chmod +x ~/.local/bin/argon-battery
```

**3. Add the module to waybar config:**

```json
{
  "modules-right": ["custom/argon-battery"],
  "custom/argon-battery": {
    "exec": "~/.local/bin/argon-battery",
    "return-type": "json",
    "interval": 60,
    "tooltip": true,
    "on-click": "foot -e sudo /usr/bin/python3 /etc/argon/argondashboard.py"
  }
}
```

**4. Style it in waybar `style.css`:**

```css
#custom-argon-battery { color: #a6d189; }
#custom-argon-battery.warning { color: #ef9f76; }
#custom-argon-battery.critical { color: #e78284; }
```

Note: the script uses `sudo` to query the battery. This requires passwordless sudo for your user, or a targeted sudoers entry for the argon script.

### Battery key binding

The Argon ONE UP has a battery key between F12 and Print Screen. It registers as `Pause` in Sway. To bind it to the Argon battery dashboard:

```
bindsym Pause exec foot -e sudo /usr/bin/python3 /etc/argon/argondashboard.py
```

---

## Initial setup from scratch

This section covers setting up a fresh Argon ONE UP CM5 laptop from a clean Raspberry Pi OS install to a working Sway desktop with this config.

### 1. Flash and boot Raspberry Pi OS

Flash **Raspberry Pi OS Lite** (minimal, no desktop, Debian Trixie-based, 64-bit) to your NVMe or SD card using [Raspberry Pi Imager](https://www.raspberrypi.com/software/). The Lite image gives you a clean base without a pre-installed desktop environment. Boot and log in.

### 2. Update the system

```bash
sudo apt update
sudo apt full-upgrade -y
sudo reboot
```

### 3. Fix NVMe power management (critical)

The Argon ONE UP's NVMe drive can become extremely sluggish or cause I/O timeouts without these kernel parameters. **This was the single biggest stability issue during initial setup — the system was nearly unusable without this fix.**

Edit the kernel command line (**keep everything on one line**):

```bash
sudo nano /boot/firmware/cmdline.txt
```

Append to the existing line:

```
nvme_core.default_ps_max_latency_us=0 pcie_aspm=off
```

This disables NVMe power state transitions and PCIe Active State Power Management, both of which cause latency spikes on the CM5's PCIe bus.

Reboot, then verify no NVMe errors:

```bash
sudo reboot
dmesg -T | grep -i nvme
```

### 4. Install the Argon config tool

This provides battery monitoring, fan control, and power button configuration for the Argon ONE UP case:

```bash
curl https://download.argon40.com/argononeup.sh | bash
```

### 5. Set Wi-Fi regulatory domain

Incorrect settings can cause poor Wi-Fi performance and channel restrictions:

```bash
sudo raspi-config
```

Navigate to **Localisation Options > WLAN Country** and set your country code (e.g. US).

Verify with:

```bash
iw reg get
```

### 6. Enable seat management

Sway needs proper seat access to manage the display and input devices:

```bash
sudo apt install -y seatd
sudo systemctl enable --now seatd
sudo usermod -aG seat,video,audio,input,render "$USER"
sudo reboot
```

Verify after reboot:

```bash
groups  # should include seat, video, audio, input, render
systemctl status seatd  # should be active
```

### 7. Install a login manager

Install GDM (GNOME Display Manager) so you can select Sway as your session at the login screen:

```bash
sudo apt install -y gdm3
sudo systemctl enable gdm
```

After installing Sway (next step), you'll be able to choose **Sway** from the session dropdown on the GDM login screen.

### 8. Install dependencies

Core packages:

```bash
sudo apt install -y \
  sway swaybg swayidle swaylock xwayland \
  waybar wofi foot wob mako-notifier \
  grim slurp wl-clipboard \
  ddcutil i2c-tools pipewire wireplumber \
  network-manager network-manager-gnome \
  ukui-polkit papirus-icon-theme \
  fish
```

Install JetBrainsMono Nerd Font (required for waybar icons):

```bash
mkdir -p ~/.local/share/fonts
curl -fLo /tmp/JetBrainsMono.zip https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
unzip -o /tmp/JetBrainsMono.zip -d ~/.local/share/fonts/
fc-cache -fv
```

Optional but recommended:

```bash
sudo apt install -y \
  firefox-esr thunar mpv imv file-roller
```

### 9. Install socktop

[socktop](https://socktop.io) is a TUI-first remote system monitor. It's used by the waybar CPU module (click to open). Install from the apt repo:

```bash
curl -fsSL https://jasonwitty.github.io/socktop/KEY.gpg | \
    sudo gpg --dearmor -o /usr/share/keyrings/socktop-archive-keyring.gpg

echo "deb [signed-by=/usr/share/keyrings/socktop-archive-keyring.gpg] https://jasonwitty.github.io/socktop stable main" | \
    sudo tee /etc/apt/sources.list.d/socktop.list

sudo apt update
sudo apt install -y socktop socktop-agent
sudo systemctl enable --now socktop-agent
```

### 10. Copy this config

```bash
# Clone the repo
git clone https://github.com/jasonwitty/sway-argon-one-up.git
cd sway-argon-one-up

# Copy configs
cp -r sway waybar wob wofi foot mako swaylock gtk-3.0 ~/.config/
mkdir -p ~/.local/bin
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/*
```

### 11. Set up lid close power management

The lid-suspend script and sudoers config are required for the lid to properly lock, blank the display, and save power when closed. See the [Lid close power management](#lid-close-power-management) standalone guide for full details, but the short version:

```bash
# Sudoers for passwordless power management
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

Configure the Argon daemon in `/etc/argononeupd.conf`:

```
lidshutdownsecs=0
lidaction=suspend
```

### 12. Log in to Sway

Reboot, and at the GDM login screen select **Sway** from the session menu (gear icon). If everything is set up correctly, you should see the Catppuccin-themed desktop with waybar at the top.

If Sway fails to start, check:

```bash
journalctl -b --no-pager | tail -200
systemctl status seatd
groups  # make sure seat group is present
```

### 13. Install Claude Code (optional)

For the Mod+C integration:

```bash
# See https://claude.ai/claude-code for installation
claude  # run once to authenticate
```

---

## Troubleshooting

### System is extremely slow / NVMe timeouts

The most common issue. Check that the kernel parameters are set:

```bash
cat /proc/cmdline | grep nvme_core
```

If `nvme_core.default_ps_max_latency_us=0` and `pcie_aspm=off` are not present, see step 3 above.

### Brightness keys don't work

Check that the display is accessible over I2C bus 14:

```bash
sudo i2cdetect -y 14  # should show device at 0x37
ddcutil --bus 14 getvcp 10  # should return current brightness
```

The brightness script writes DDC/CI commands directly to `/dev/i2c-14`. Make sure your user has access (should be in the `i2c` or `render` group).

### No sound / volume keys don't work

This config uses PipeWire with WirePlumber (`wpctl`), not PulseAudio (`pactl`):

```bash
wpctl status
wpctl get-volume @DEFAULT_AUDIO_SINK@
```

### Lid close doesn't do anything / no power savings

The Argon ONE UP uses a GPIO-based lid switch, not a standard ACPI lid. Sway `bindswitch` and `logind.conf` `HandleLidSwitch` will not work. The lid is handled entirely by the Argon daemon.

Check that the Argon daemon is running and configured:

```bash
systemctl status argononed  # daemon should be active
cat /etc/argononeupd.conf   # should contain lidaction=suspend
```

Check the lid event log to see if the script is being called:

```bash
cat ~/.local/state/lid-events.log
```

A successful close/open cycle looks like:

```
=== LID CLOSE 2026-04-03 10:01:06 ===
  display: off
  cpu governor: powersave
  wifi: Soft blocked: yes
  bluetooth: Soft blocked: yes
  webcam (1-1.4): unbound
=== LID OPEN 2026-04-03 10:04:45 ===
  display: on
  cpu governor: ondemand
  wifi: Soft blocked: no
  bluetooth: Soft blocked: no
  webcam (1-1.4): bound
```

If the log file doesn't exist, the script isn't being called. Verify:

- `/etc/argononeupd.conf` has `lidaction=suspend`
- `~/.local/bin/lid-suspend` exists and is executable
- The Argon daemon's `argonpowerbutton.py` has the suspend case that calls `lid-suspend`

If the log shows a subsystem failed (e.g. `webcam: not found`), the webcam vendor ID (`11cc:2812`) may not match your hardware. Check with `lsusb` and update the IDs in the script.

### Black screen after lid close (hard reboot required)

This means something attempted a real system suspend. The Pi 5 / CM5 has no suspend support — it will freeze. Check:

```bash
# Make sure logind is NOT trying to handle the lid
grep -r HandleLidSwitch /etc/systemd/logind.conf /etc/systemd/logind.conf.d/ 2>/dev/null
```

All `HandleLidSwitch` values should be commented out or set to `ignore`. The safest option is to not have any overrides — the Argon GPIO daemon handles the lid, not logind.

Also make sure the powermenu does not include a Suspend option (`systemctl suspend` will black-screen).

### Waybar or wob missing after sway reload

These use `exec_always` and should survive reloads. If they don't appear, check the processes:

```bash
pgrep waybar
pgrep wob
```

And restart manually if needed: `swaymsg reload`
