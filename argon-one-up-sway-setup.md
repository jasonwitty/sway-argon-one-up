# Argon ONE UP / Raspberry Pi CM5 Sway Setup Guide

This is a step-by-step reference for rebuilding my **Argon ONE UP laptop** setup from scratch using a **Debian-based Raspberry Pi OS** environment with a **Wayland + Sway** desktop.

It is written as a practical recovery guide, not as a perfect generic Linux manual. Some package names can vary slightly between Raspberry Pi OS releases, so if one package is unavailable, search for the equivalent with `apt search <name>`.

---

## Goals

- Debian-based Raspberry Pi OS
- Lightweight Wayland desktop using **Sway**
- Launcher/menu with **wofi**
- Top bar with **waybar**
- VNC access using **wayvnc**
- Stable seat/session handling with **seatd**
- Good enough laptop UX for daily use
- Optional fullscreen VNC scaling with **gamescope**

---

## Hardware / OS assumptions

This guide assumes:

- **Argon ONE UP** chassis
- **Raspberry Pi Compute Module 5** or Raspberry Pi 5-class hardware
- Debian-based **Raspberry Pi OS**
- A normal user account already exists
- You can log in locally on the console/TTY

---

## 1. Update the system

```bash
sudo apt update
sudo apt full-upgrade -y
sudo reboot
```

After reboot:

```bash
sudo apt update
```

---

## 2. Install the desktop stack

Install the core packages:

```bash
sudo apt install -y \
  sway swaybg swayidle swaylock xwayland \
  waybar wofi seatd \
  foot alacritty \
  grim slurp wl-clipboard \
  pavucontrol brightnessctl playerctl \
  policykit-1 lxpolkit \
  network-manager network-manager-gnome \
  wayvnc
```

Useful optional packages:

```bash
sudo apt install -y \
  thunar thunar-volman tumbler \
  firefox-esr \
  fonts-noto fonts-noto-color-emoji
```

If you want fullscreen/scaled VNC sessions later:

```bash
sudo apt install -y gamescope
```

Notes:

- `foot` is a great lightweight terminal on Pi hardware.
- `alacritty` is usable too, but it is heavier.
- `network-manager-gnome` provides `nm-applet` for tray/network management.
- `lxpolkit` helps with privilege prompts in a lightweight environment.

---

## 3. Enable seat management correctly

One of the early problems in this setup was seat/session access. The fix was to use **seatd** properly and make sure the user could access the seat.

Enable and start the service:

```bash
sudo systemctl enable --now seatd
```

Make sure your user is in the `seat` group:

```bash
sudo usermod -aG seat "$USER"
```

Also add the user to common desktop-related groups if needed:

```bash
sudo usermod -aG video,audio,input,render "$USER"
```

Then **log all the way out and back in**, or reboot:

```bash
sudo reboot
```

To verify:

```bash
groups
systemctl status seatd
```

You should see `seat` in your user’s groups.

---

## 4. Create a basic Sway config

Create the config directory:

```bash
mkdir -p ~/.config/sway
```

If the default config exists, copy it as a starting point:

```bash
cp /etc/sway/config ~/.config/sway/config
```

Then edit it:

```bash
micro ~/.config/sway/config
```

A solid starting config is below.

```ini
### ~/.config/sway/config

set $mod Mod4
set $term foot
set $menu wofi --show drun

font pango:Noto Sans 10

output * bg #303446 solid_color

default_border pixel 2
default_floating_border pixel 2
titlebar_border_thickness 0
hide_edge_borders smart

floating_modifier $mod normal

# Terminal / launcher
bindsym $mod+Return exec $term
bindsym $mod+d exec $menu
bindsym $mod+Shift+q kill
bindsym $mod+Shift+r reload
bindsym $mod+Shift+e exec swaynag -t warning -m 'Exit Sway?' -B 'Yes' 'swaymsg exit'

# Focus
bindsym $mod+h focus left
bindsym $mod+j focus down
bindsym $mod+k focus up
bindsym $mod+l focus right

# Move windows
bindsym $mod+Shift+h move left
bindsym $mod+Shift+j move down
bindsym $mod+Shift+k move up
bindsym $mod+Shift+l move right

# Layout
bindsym $mod+b splith
bindsym $mod+v splitv
bindsym $mod+f fullscreen toggle
bindsym $mod+s layout stacking
bindsym $mod+w layout tabbed
bindsym $mod+e layout toggle split

# Workspaces
bindsym $mod+1 workspace 1
bindsym $mod+2 workspace 2
bindsym $mod+3 workspace 3
bindsym $mod+4 workspace 4
bindsym $mod+5 workspace 5
bindsym $mod+6 workspace 6
bindsym $mod+7 workspace 7
bindsym $mod+8 workspace 8
bindsym $mod+9 workspace 9
bindsym $mod+0 workspace 10

bindsym $mod+Shift+1 move container to workspace 1
bindsym $mod+Shift+2 move container to workspace 2
bindsym $mod+Shift+3 move container to workspace 3
bindsym $mod+Shift+4 move container to workspace 4
bindsym $mod+Shift+5 move container to workspace 5
bindsym $mod+Shift+6 move container to workspace 6
bindsym $mod+Shift+7 move container to workspace 7
bindsym $mod+Shift+8 move container to workspace 8
bindsym $mod+Shift+9 move container to workspace 9
bindsym $mod+Shift+0 move container to workspace 10

# Screenshot
bindsym Print exec grim ~/Pictures/screenshot-$(date +%F-%T).png
bindsym Shift+Print exec grim -g "$(slurp)" ~/Pictures/screenshot-$(date +%F-%T).png

# Brightness keys (adjust if needed)
bindsym XF86MonBrightnessUp exec brightnessctl set +10%
bindsym XF86MonBrightnessDown exec brightnessctl set 10%-

# Volume keys
bindsym XF86AudioRaiseVolume exec pactl set-sink-volume @DEFAULT_SINK@ +5%
bindsym XF86AudioLowerVolume exec pactl set-sink-volume @DEFAULT_SINK@ -5%
bindsym XF86AudioMute exec pactl set-sink-mute @DEFAULT_SINK@ toggle

# Autostart
exec_always waybar
exec_always nm-applet
exec_always lxpolkit
```

Notes:

- I preferred a **clean, minimal look** with reduced chrome.
- Removing visible title bar clutter was part of the goal.
- If borders or title bars do not look right, fully log out and back in instead of only reloading Sway.

---

## 5. Create a Waybar config

Make the directory:

```bash
mkdir -p ~/.config/waybar
```

Create the config:

```bash
micro ~/.config/waybar/config
```

Example:

```json
{
  "layer": "top",
  "position": "top",
  "height": 30,
  "modules-left": ["sway/workspaces", "sway/window"],
  "modules-center": ["clock"],
  "modules-right": ["pulseaudio", "network", "battery", "tray"],

  "clock": {
    "format": "{:%a %Y-%m-%d %H:%M}"
  },

  "battery": {
    "format": "{capacity}% {icon}",
    "format-icons": ["", "", "", "", ""]
  },

  "network": {
    "format-wifi": "  {essid}",
    "format-ethernet": "󰈀  {ipaddr}",
    "format-disconnected": "󰖪"
  },

  "pulseaudio": {
    "format": "  {volume}%",
    "format-muted": " muted"
  },

  "tray": {
    "spacing": 8
  }
}
```

Create the style:

```bash
micro ~/.config/waybar/style.css
```

Example Catppuccin-ish starting point:

```css
* {
  border: none;
  border-radius: 0;
  font-family: "Noto Sans", sans-serif;
  font-size: 13px;
  min-height: 0;
}

window#waybar {
  background: #303446;
  color: #c6d0f5;
}

#workspaces button {
  padding: 0 8px;
  color: #c6d0f5;
  background: transparent;
}

#workspaces button.focused {
  background: #51576d;
}

#clock,
#battery,
#network,
#pulseaudio,
#tray,
#window {
  padding: 0 10px;
}
```

If Waybar comes up looking like the wrong/default config, make sure your files are actually in `~/.config/waybar/` and restart Sway.

---

## 6. Create a Wofi launcher config

```bash
mkdir -p ~/.config/wofi
micro ~/.config/wofi/config
```

Example:

```ini
width=700
height=400
show=drun
prompt=Run...
allow_images=true
term=foot
```

Style file:

```bash
micro ~/.config/wofi/style.css
```

Example:

```css
window {
  margin: 0px;
  border: 2px solid #8caaee;
  background-color: #303446;
}

#input {
  margin: 8px;
  border: none;
  color: #c6d0f5;
  background-color: #414559;
}

#entry {
  padding: 6px;
  color: #c6d0f5;
}

#entry:selected {
  background-color: #51576d;
}
```

---

## 7. Start Sway manually first

Before attempting any autologin or service automation, verify that a normal manual launch works.

From TTY:

```bash
sway
```

If that works reliably, continue.

If it fails, check:

```bash
journalctl -b --no-pager | tail -n 200
systemctl status seatd
```

---

## 8. Optional: make Sway start from TTY login

I recommend **starting simple first**. A clean manual login is easier to debug than jumping straight into autologin.

You can add this to `~/.bash_profile` if you want Sway to start automatically on TTY1 login:

```bash
if [ -z "$DISPLAY" ] && [ "$(tty)" = /dev/tty1 ]; then
  exec sway
fi
```

Then log in on TTY1 and it will launch Sway automatically.

If you are not using bash, adapt this to your shell/session flow. For fish users, I still recommend keeping the TTY auto-start logic simple and conservative.

---

## 9. Optional: autologin on a TTY

This was one of the more fragile parts of the setup. It can work, but it is also the part most likely to cause confusing startup issues.

Only do this after the manual setup is stable.

Create an override for the getty service. Example for `tty2`:

```bash
sudo systemctl edit getty@tty2
```

Add:

```ini
[Service]
ExecStart=
ExecStart=-/sbin/agetty --autologin YOURUSER --noclear %I $TERM
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl restart getty@tty2
```

Replace `YOURUSER` with your actual username.

In practice, autologin plus user services plus Wayland/VNC sometimes became unstable. If you hit weird behavior, go back to manual login first.

---

## 10. Set up WayVNC

Create a config directory:

```bash
mkdir -p ~/.config/wayvnc
```

Basic config:

```bash
micro ~/.config/wayvnc/config
```

Example:

```ini
address=0.0.0.0
port=5901
```

Manual launch test from inside Sway:

```bash
wayvnc 0.0.0.0 5901
```

If you want it as a **systemd user service**:

```bash
mkdir -p ~/.config/systemd/user
micro ~/.config/systemd/user/wayvnc.service
```

Example service:

```ini
[Unit]
Description=WayVNC server
After=graphical-session.target

[Service]
ExecStart=/usr/bin/wayvnc 0.0.0.0 5901
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
```

Enable it:

```bash
systemctl --user daemon-reload
systemctl --user enable --now wayvnc.service
```

If you want the service to survive without an active user login session, you can also enable lingering:

```bash
loginctl enable-linger "$USER"
```

However, I would only do that after everything works normally.

### More robust WayVNC start logic

At one point, a more careful startup flow helped avoid launching too early. A wrapper script can wait for Wayland and an output to exist.

Create script:

```bash
mkdir -p ~/.local/bin
micro ~/.local/bin/start-wayvnc.sh
chmod +x ~/.local/bin/start-wayvnc.sh
```

Example:

```bash
#!/usr/bin/env bash
set -e

for i in $(seq 1 30); do
  if [ -n "$WAYLAND_DISPLAY" ] && swaymsg -t get_outputs >/dev/null 2>&1; then
    exec /usr/bin/wayvnc 0.0.0.0 5901
  fi
  sleep 1
done

exit 1
```

Then point the systemd user service at that script instead of calling `wayvnc` directly.

---

## 11. Use Gamescope to improve VNC fullscreen scaling

One of the nicer tricks was using **gamescope** to make VNC sessions look better on mismatched resolutions.

Example:

```bash
gamescope -b -W 2304 -H 1296 -w 2560 -h 1440 -S stretch -F linear --force-windows-fullscreen -- vncviewer localhost:5901
```

This is useful when:

- the VNC client resolution does not match the display nicely
- scaling looks ugly
- you want a more appliance-like fullscreen experience

Adjust the `-W/-H` and `-w/-h` values to fit your panel and preferred render resolution.

---

## 12. NVMe stability tuning (important if using NVMe storage)

At one point, NVMe behavior improved by adding these kernel parameters:

- `nvme_core.default_ps_max_latency_us=0`
- `pcie_aspm=off`

Edit the kernel command line:

```bash
sudo micro /boot/firmware/cmdline.txt
```

Append the parameters to the **single existing line**:

```text
nvme_core.default_ps_max_latency_us=0 pcie_aspm=off
```

Important:

- Do **not** add line breaks.
- Keep everything on one line.

Then reboot:

```bash
sudo reboot
```

After reboot, check dmesg for NVMe errors:

```bash
dmesg -T | grep -i nvme
```

This was specifically useful when dealing with NVMe I/O timeout-style problems.

---

## 13. Wi-Fi region / regulatory domain (optional)

If Wi-Fi behaves oddly, set your regulatory domain correctly.

For the US:

```bash
sudo raspi-config
```

Then set WLAN country to **US**.

Or verify using:

```bash
iw reg get
```

Incorrect regdom settings can cause annoying Wi-Fi behavior and channel restrictions.

---

## 14. Recommended apps / quality-of-life tools

A few things that fit this setup well:

```bash
sudo apt install -y \
  firefox-esr \
  file-roller \
  mpv \
  imv \
  pcmanfm
```

Notes:

- `mpv` is a great lightweight media player.
- `imv` is a simple Wayland-friendly image viewer.
- A lightweight file manager is nice to have even if you mostly live in the terminal.

---

## 15. Lid close power management

The Pi 5 / CM5 **does not support system suspend** — there is no `/sys/power/state` or `mem_sleep` kernel interface. Calling `systemctl suspend` will black-screen the system and require a hard reboot.

The Argon ONE UP case has its own lid switch detected via GPIO, monitored by the Argon daemon (`argononeupd.py`). Standard ACPI/libinput lid detection and sway `bindswitch` do not work on this hardware.

The solution is a "soft suspend" script that powers down individual subsystems on lid close and restores them on lid open.

### What the script does

| Lid close | Lid open |
|-----------|----------|
| Lock screen (swaylock) | Display on |
| Display off | CPU governor → ondemand |
| CPU governor → powersave | WiFi unblocked |
| WiFi blocked (rfkill) | Bluetooth unblocked |
| Bluetooth blocked (rfkill) | Webcam rebound (USB) |
| Webcam unbound (USB) | |

All events are logged to `~/.local/state/lid-events.log`.

### Setup

**1. Create the lid-suspend script** at `~/.local/bin/lid-suspend`:

```bash
#!/bin/bash
# Called by argon daemon on lid close/open
# Usage: lid-suspend close | lid-suspend open
export XDG_RUNTIME_DIR=/run/user/1000
export SWAYSOCK=/run/user/1000/sway-ipc.1000.$(pgrep -x sway).sock
export WAYLAND_DISPLAY=wayland-1

LOG="$HOME/.local/state/lid-events.log"
mkdir -p "$(dirname "$LOG")"

# Find webcam USB device path by vendor:product ID
find_webcam() {
    for dev in /sys/bus/usb/devices/*/idVendor; do
        devdir="$(dirname "$dev")"
        if [ "$(cat "$devdir/idVendor" 2>/dev/null)" = "11cc" ] &&
           [ "$(cat "$devdir/idProduct" 2>/dev/null)" = "2812" ]; then
            basename "$devdir"
            return
        fi
    done
}

log() { echo "  $1" >> "$LOG"; }

case "$1" in
    close)
        echo "=== LID CLOSE $(date '+%Y-%m-%d %H:%M:%S') ===" >> "$LOG"
        swaylock -f &
        sleep 0.5
        swaymsg "output * power off"
        log "display: off"
        echo powersave | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor > /dev/null
        log "cpu governor: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
        sudo rfkill block wifi
        sudo rfkill block bluetooth
        log "wifi: $(rfkill list wifi | grep -o 'Soft blocked: .*')"
        log "bluetooth: $(rfkill list bluetooth | grep -o 'Soft blocked: .*')"
        WEBCAM=$(find_webcam)
        if [ -n "$WEBCAM" ]; then
            echo "$WEBCAM" | sudo tee /sys/bus/usb/drivers/usb/unbind > /dev/null 2>&1
            log "webcam ($WEBCAM): unbound"
        else
            log "webcam: not found (already unbound or disconnected)"
        fi
        ;;
    open)
        echo "=== LID OPEN $(date '+%Y-%m-%d %H:%M:%S') ===" >> "$LOG"
        swaymsg "output * power on"
        log "display: on"
        echo ondemand | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor > /dev/null
        log "cpu governor: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
        sudo rfkill unblock wifi
        sudo rfkill unblock bluetooth
        log "wifi: $(rfkill list wifi | grep -o 'Soft blocked: .*')"
        log "bluetooth: $(rfkill list bluetooth | grep -o 'Soft blocked: .*')"
        WEBCAM=$(find_webcam)
        if [ -z "$WEBCAM" ]; then
            for dev in /sys/bus/usb/devices/*/idVendor; do
                devdir="$(dirname "$dev")"
                if [ "$(cat "$devdir/idVendor" 2>/dev/null)" = "11cc" ] &&
                   [ "$(cat "$devdir/idProduct" 2>/dev/null)" = "2812" ]; then
                    WEBCAM=$(basename "$devdir")
                    break
                fi
            done
        fi
        if [ -n "$WEBCAM" ]; then
            echo "$WEBCAM" | sudo tee /sys/bus/usb/drivers/usb/bind > /dev/null 2>&1
            log "webcam ($WEBCAM): bound"
        else
            log "webcam: not found"
        fi
        ;;
esac
```

```bash
chmod +x ~/.local/bin/lid-suspend
```

**2. Configure the Argon daemon** to use the suspend action. Edit `/etc/argononeupd.conf`:

```
lidshutdownsecs=0
lidaction=suspend
```

The Argon daemon's power button monitor (`argonpowerbutton.py`) checks `lidaction` and calls `sudo -u jason /home/jason/.local/bin/lid-suspend close` on lid close and `open` on lid open.

**3. Add passwordless sudo** for the specific operations the script needs:

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

### Important notes

- **Do not configure logind** (`HandleLidSwitch` etc.) — there is no standard lid switch device on this hardware. If logind somehow intercepts a suspend request, it will black-screen.
- **The webcam is found by vendor:product ID** (`11cc:2812` for the SunplusIT PC Camera), not by USB path. This survives port changes.
- **WiFi reconnects automatically** after `rfkill unblock` — NetworkManager handles reassociation.
- **Verify with the log** after testing: `cat ~/.local/state/lid-events.log`

---

## 16. Backup the working config

Once the system is working, back up your config immediately.

Good things to save:

- `~/.config/sway/`
- `~/.config/waybar/`
- `~/.config/wofi/`
- `~/.config/wayvnc/`
- `~/.config/systemd/user/`
- `~/.local/bin/`
- `/boot/firmware/cmdline.txt`
- any custom fonts/themes/backgrounds

A quick archive example:

```bash
mkdir -p ~/backups

tar -czf ~/backups/pi-sway-config-$(date +%F).tar.gz \
  ~/.config/sway \
  ~/.config/waybar \
  ~/.config/wofi \
  ~/.config/wayvnc \
  ~/.config/systemd/user \
  ~/.local/bin
```

---

## 17. Troubleshooting

### Sway will not start

Check:

```bash
systemctl status seatd
journalctl -b --no-pager | tail -n 200
```

Common causes:

- user not in `seat` group
- not fully logged out after group changes
- missing GPU/session/permission access

### Waybar starts with the wrong config or default look

Check:

```bash
ls -la ~/.config/waybar/
```

Make sure `config` and `style.css` are in the right location.

### WayVNC does not connect

Check whether Sway is running and whether the Wayland session exists first. Then test WayVNC manually inside the active Sway session:

```bash
wayvnc 0.0.0.0 5901
```

If that works manually but not as a service, the service is probably starting too early.

### Title bars / borders do not change after editing config

Do a full logout/login instead of only reloading Sway.

### System feels unstable after adding autologin or user services

Back out the automation and return to the simplest known-good state:

1. manual login
2. manual `sway`
3. manual `wayvnc`

Then reintroduce automation one piece at a time.

---

## 18. Minimal rebuild checklist

If rebuilding from scratch, this is the short version:

1. Install Raspberry Pi OS
2. Update system
3. Install Sway/Waybar/Wofi/seatd/wayvnc
4. Enable `seatd`
5. Add user to `seat`, `video`, `audio`, `input`, `render`
6. Reboot
7. Create Sway config
8. Create Waybar config
9. Create Wofi config
10. Test manual `sway`
11. Test manual `wayvnc`
12. Add optional autostart/user services only after manual flow is stable
13. Add optional NVMe kernel tuning if needed
14. Set up lid close power management (lid-suspend script + sudoers + argon config)
15. Back up configs once working

---

## 19. Final notes

The big lesson from this build was: **do not automate too early**.

The most reliable path is:

- get Sway working manually
- get WayVNC working manually
- confirm the display/output/session is stable
- only then add autologin, systemd user services, and other convenience layers

That makes troubleshooting much easier when something breaks.

