# sway-argon-one-up

Sway window manager configuration for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system), a 14-inch laptop powered by the Raspberry Pi Compute Module 5. Includes a 9-theme switcher with matching wallpapers and live terminal recoloring, a dynamic keybinding help overlay, instant brightness control via direct I2C, display scaling controls, and Claude Code integration.

![screenshot](screenshot.png)

## Hardware

This config is built for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) which uses a Raspberry Pi Compute Module 5. The display is connected via HDMI internally, so standard backlight controls don't apply — brightness is controlled by writing DDC/CI commands directly to the display over I2C bus 14, achieving ~30ms response time. This approach was inspired by [esvertit's display calibration guide](https://forum.argon40.com/t/guide-professional-display-calibration-on-argon-one-up/9188) on the Argon40 forum, which documented the display's I2C interface and DDC/CI capabilities. The Argon case also has its own battery, monitored via a custom script.

## What's included

| Directory | Description |
|-----------|-------------|
| `sway/` | Sway config with themed window colors, idle lock, touchpad, media keys |
| `waybar/` | Top bar with workspaces, clock, CPU, volume, backlight, Argon battery, tray, theme/scale/Claude/help/power buttons |
| `sway-themes/` | 9 theme definitions + templates for all themed apps (sway, waybar, foot, mako, swaylock, wofi, wob, Brave, Chromium, Thunar/GTK) |
| `wallpapers/` | Matching wallpaper for each theme (auto-applied on theme switch) |
| `wob/` | Wayland Overlay Bar config for brightness/volume indicators |
| `wofi/` | App launcher and help overlay styles |
| `foot/` | Terminal emulator config + color include (live-recolored on theme switch via OSC sequences) |
| `mako/` | Notification daemon themed to match |
| `swaylock/` | Lock screen with themed ring indicator |
| `gtk-3.0/` | GTK theme settings (switched automatically per theme) |
| `fish/` | Fish shell config |
| `starship.toml` | Starship prompt config |
| `greetd/` | Login screen config: greetd + gtkgreet with Catppuccin Frappe theme and wallpaper |
| `bin/` | `switch-theme`, `sway-scale`, `sway-help`, `claude-prompt`, `brightness`, `start-wob`, `argon-battery`, `lid-suspend`, `powermenu` scripts |

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

## Browser integration

| Binding | Action |
|---------|--------|
| **Mod+B** | Launch Brave browser |

Brave is launched with performance flags optimized for the Pi's hardware (`--enable-low-end-device-mode`, `--process-per-site`, etc.). Both Brave and Chromium are themed automatically by the theme switcher via managed policy files — the browser title bar and color scheme update live on theme change, no restart needed.

**Prerequisites:** Install [Brave](https://brave.com/linux/) and/or [Chromium](https://www.chromium.org/). Both browsers must be installed for full theme integration. Set up the managed policy directories:

```bash
sudo mkdir -p /etc/brave/policies/managed
sudo mkdir -p /etc/chromium/policies/managed
```

The `switch-theme` script needs passwordless sudo to write the policy files. Add to your sudoers:

```bash
sudo tee /etc/sudoers.d/browser-theme > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/brave/policies/managed/color.json
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/chromium/policies/managed/color.json
EOF
```

## Theme switcher

Switch between 9 themes with a single click or command. Every themed app updates simultaneously — sway window borders, waybar, foot terminals, mako notifications, swaylock, wofi, wob, GTK apps (including Thunar folder colors), Brave, Chromium, and the wallpaper.

**Available themes:**

<p>
<img src="screenshots/theme-frappe.png" width="30%" alt="Catppuccin Frappe">
<img src="screenshots/theme-mocha.png" width="30%" alt="Catppuccin Mocha">
<img src="screenshots/theme-latte.png" width="30%" alt="Catppuccin Latte">
</p>
<p>
<img src="screenshots/theme-macchiato.png" width="30%" alt="Catppuccin Macchiato">
<img src="screenshots/theme-dracula.png" width="30%" alt="Dracula">
<img src="screenshots/theme-nord.png" width="30%" alt="Nord">
</p>
<p>
<img src="screenshots/theme-gruvbox.png" width="30%" alt="Gruvbox Dark">
<img src="screenshots/theme-monokai-dark.png" width="30%" alt="Monokai Dark">
<img src="screenshots/theme-monokai-light.png" width="30%" alt="Monokai Light">
</p>

*Frappe, Mocha, Latte, Macchiato, Dracula, Nord, Gruvbox Dark, Monokai Dark, Monokai Light*

| Method | Action |
|--------|--------|
| **Mod+T** | Open theme picker (wofi) |
| **Waybar palette icon** | Open theme picker |
| `switch-theme <name>` | Switch directly (e.g. `switch-theme dracula`) |
| `switch-theme --wallpaper-picker` | Choose a wallpaper (overrides theme default) |
| `switch-theme --wallpaper <path>` | Set a specific wallpaper |
| `switch-theme --wallpaper-reset` | Revert to the current theme's default wallpaper |

**How it works:**

- Theme definitions in `sway-themes/` set 35 color variables + wallpaper path + GTK theme + Papirus folder color
- Templates in `sway-themes/templates/` use `@@VARIABLE@@` placeholders
- `switch-theme` sources a theme, renders all templates, and applies colors at runtime
- Foot terminals are live-recolored via OSC 4/10/11 escape sequences sent directly to each terminal's pts device — no restart needed
- Brave and Chromium are themed via managed policy files (`/etc/brave/policies/managed/color.json` and `/etc/chromium/policies/managed/color.json`) — the browser reads these live, no restart needed
- GTK apps (including Thunar) update live via gsettings — folder icons change color per theme using `papirus-folders`
- Wallpapers are applied via `swaybg`, with an optional override that persists across theme switches
- Waybar is restarted (via `swaymsg exec` to survive the restart), mako is reloaded, wob is restarted

**Adding a new theme:** Create a file in `sway-themes/` following the existing format (see `sway-themes/dracula` for reference), add a matching wallpaper to `wallpapers/`, and add the name to the `available_flavors()` list in `bin/switch-theme`.

## Display scaling

A waybar module for adjusting Sway's output scale on the fly — useful for finding the right balance between screen real estate and readability.

| Method | Action |
|--------|--------|
| **Waybar magnifier icon** | Open scale picker (wofi) |
| Available steps | 1x, 1.25x, 1.5x, 1.6x, 1.75x, 2x |

The default is 1.6x (1200x750 effective on the 1920x1200 panel). Click to reset to default.

<!--
## WiFi stability

The BCM43455 WiFi chip in the CM5 has aggressive power management that causes intermittent disconnects. This is fixed by disabling WiFi power save:

```bash
sudo tee /etc/NetworkManager/conf.d/wifi-powersave-off.conf > /dev/null <<EOF
[connection]
wifi.powersave = 2
EOF
sudo nmcli general reload
```

Verify with `iw dev wlan0 get power_save` — should show `Power save: off`.
-->

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

### 7. Install a login manager (greetd + gtkgreet)

Install [greetd](https://sr.ht/~kennylevinsen/greetd/) with [gtkgreet](https://man.sr.ht/~kennylevinsen/greetd/#gtkgreet) for a lightweight Wayland-native login screen that runs inside its own minimal Sway session:

```bash
sudo apt install -y greetd gtkgreet
sudo systemctl enable greetd
```

Copy the greeter configs from this repo:

```bash
sudo cp greetd/config.toml greetd/sway-config greetd/gtkgreet.css /etc/greetd/
```

Set a wallpaper for the login screen (any image you like):

```bash
sudo cp wallpapers/frappe.png /etc/greetd/wallpaper.png
```

**How it works:** greetd launches a minimal Sway session (no bar, no keybindings) that displays gtkgreet over a wallpaper. After login, gtkgreet starts your real Sway session and the greeter's Sway exits. The CSS uses Catppuccin Frappe colors to match the default theme.

**Fallback:** If you prefer a more traditional login manager, you can use GDM instead:

```bash
sudo apt install -y gdm3
sudo systemctl enable gdm
# (disable greetd if switching: sudo systemctl disable greetd)
```

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
  fish fonts-firacode zoxide fzf ugrep
```

Install JetBrainsMono Nerd Font (required for waybar icons):

```bash
mkdir -p ~/.local/share/fonts
curl -fLo /tmp/JetBrainsMono.zip https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
unzip -o /tmp/JetBrainsMono.zip -d ~/.local/share/fonts/
fc-cache -fv
```

Install Rust toolchain and CLI tools used by the fish config:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
cargo install --locked bat eza pfetch-rs
```

Install [Starship](https://starship.rs/) prompt and [Atuin](https://atuin.sh/) shell history:

```bash
curl -sS https://starship.rs/install.sh | sh
curl --proto '=https' --tlsv1.2 -LsSf https://setup.atuin.sh | sh
```

Optional but recommended:

```bash
sudo apt install -y \
  firefox-esr thunar mpv imv zathura file-roller chromium
```

### 8b. Install GTK themes and Papirus folder colors

The theme switcher sets a matching GTK theme and Papirus folder icon color for each theme. Install the GTK themes to `~/.themes/`:

- **Catppuccin GTK** — download from [catppuccin/gtk](https://github.com/catppuccin/gtk/releases) (mocha, frappe, latte, macchiato — blue, standard+default variants)
- **Dracula GTK** — download from [dracula/gtk](https://github.com/dracula/gtk/releases)
- **Nordic GTK** — download from [EliverLara/Nordic](https://github.com/EliverLara/Nordic/releases)
- **Gruvbox GTK** — download from [Fausto-Korpsvansen/Gruvbox-GTK-Theme](https://github.com/Fausto-Korpsvansen/Gruvbox-GTK-Theme) (Gruvbox-Dark)
- **Monokai GTK** — download from [Maikuolan/Monokai](https://github.com/Maikuolan/Monokai)

Extract each into `~/.themes/` so they appear as e.g. `~/.themes/Dracula/`, `~/.themes/Nordic/`, etc. Monokai Light uses the built-in Adwaita theme (no download needed).

Install the `papirus-folders` script for per-theme folder icon colors:

```bash
curl -fLo ~/.local/bin/papirus-folders \
  https://raw.githubusercontent.com/PapirusDevelopmentTeam/papirus-folders/master/papirus-folders
chmod +x ~/.local/bin/papirus-folders
```

For Brave browser, follow the [official install guide](https://brave.com/linux/). Both Brave and Chromium are themed automatically by the theme switcher. Set up the policy directories for browser theming:

```bash
sudo mkdir -p /etc/brave/policies/managed /etc/chromium/policies/managed
sudo tee /etc/sudoers.d/browser-theme > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/brave/policies/managed/color.json
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/chromium/policies/managed/color.json
EOF
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
cp -r sway waybar wob wofi foot mako swaylock gtk-3.0 sway-themes fish ~/.config/
cp starship.toml ~/.config/
cp -r wallpapers ~/.wallpapers
mkdir -p ~/.local/bin
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/*
```

### 11. Build and install argon-battery-rs

The battery monitor is a Rust binary that reads the Argon battery gauge directly over I2C. It powers the waybar battery indicator, auto-adjusts brightness, and switches CPU governor on AC/battery transitions. See the [argon-battery-rs README](argon-battery-rs/README.md) for full details including stock daemon changes.

```bash
cd argon-battery-rs
cargo build --release
sudo cp target/release/argon-battery-rs /usr/local/bin/
```

### 12. Set up lid close power management

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

### 13. Log in to Sway

Reboot, and you'll see the gtkgreet login screen with your wallpaper. Enter your username and password to launch Sway. If everything is set up correctly, you should see the Catppuccin-themed desktop with waybar at the top.

If Sway fails to start, check:

```bash
journalctl -b --no-pager | tail -200
systemctl status seatd
groups  # make sure seat group is present
```

### 14. Install Claude Code (optional)

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
