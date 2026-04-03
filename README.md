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
| `bin/` | `sway-help`, `claude-prompt`, `brightness`, `start-wob`, `argon-battery` scripts |

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
  fish \
  fonts-jetbrains-mono
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

### 11. Log in to Sway

Reboot, and at the GDM login screen select **Sway** from the session menu (gear icon). If everything is set up correctly, you should see the Catppuccin-themed desktop with waybar at the top.

If Sway fails to start, check:

```bash
journalctl -b --no-pager | tail -200
systemctl status seatd
groups  # make sure seat group is present
```

### 12. Install Claude Code (optional)

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

### Waybar or wob missing after sway reload

These use `exec_always` and should survive reloads. If they don't appear, check the processes:

```bash
pgrep waybar
pgrep wob
```

And restart manually if needed: `swaymsg reload`
