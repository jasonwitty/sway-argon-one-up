# Usage Manual

A reference guide for using the Sway desktop on Argon ONE UP. Covers keyboard shortcuts, the system dashboard, theme switching, fan control, power management, and configuration.

## Table of Contents

- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Help Menu](#help-menu)
- [System Dashboard](#system-dashboard)
- [Theme Switching](#theme-switching)
- [Fan Control](#fan-control)
- [Battery and Power Management](#battery-and-power-management)
- [Touchpad (disable while typing)](#touchpad-disable-while-typing)
- [Bluetooth Audio](#bluetooth-audio)
- [Display Scaling](#display-scaling)
- [Login Screen](#login-screen)
- [Bazaar Flatpak Store](#bazaar-flatpak-store)
- [Web Apps](#web-apps)
- [Screen Recording](#screen-recording)
- [Configuration Files](#configuration-files)

---

## Keyboard Shortcuts

`Mod` is the Super/Windows key.

### General

| Shortcut | Action |
|----------|--------|
| **Mod+Enter** | Open terminal (foot) |
| **Mod+D** | App launcher (wofi) |
| **Ctrl+Shift+Enter** | App launcher (alternate) |
| **Mod+Q** | Close focused window |
| **Mod+Shift+R** | Reload sway config |
| **Mod+Shift+E** | Exit sway (logout prompt) |

### Navigation

| Shortcut | Action |
|----------|--------|
| **Mod+H/J/K/L** | Focus left / down / up / right |
| **Mod+Arrow keys** | Focus left / down / up / right |
| **Mod+1–0** | Switch to workspace 1–10 |
| **Mod+Shift+1–0** | Move window to workspace 1–10 |

### Window Management

| Shortcut | Action |
|----------|--------|
| **Mod+Shift+H/J/K/L** | Move window left / down / up / right |
| **Mod+Shift+Arrow keys** | Move window left / down / up / right |
| **Mod+F** | Toggle fullscreen |
| **Mod+Shift+V** | Split horizontal |
| **Mod+V** | Split vertical |
| **Mod+S** | Stacking layout |
| **Mod+W** | Tabbed layout |
| **Mod+E** | Toggle split layout |
| **Mod+Shift+Space** | Toggle floating mode |
| **Mod+Space** | Focus tiling / floating |
| **Mod+A** | Focus parent container |
| **Mod+R** | Enter resize mode (H/J/K/L to resize, Esc to exit) |

### Scratchpad

| Shortcut | Action |
|----------|--------|
| **Mod+Shift+Minus** | Send window to scratchpad |
| **Mod+Minus** | Show/cycle scratchpad windows |

### Applications

| Shortcut | Action |
|----------|--------|
| **Mod+B** | Launch Brave Origin (or regular Brave, if you chose to keep it when upgrading) |
| **Mod+N** | Open file manager (Thunar) |
| **Mod+Ctrl+N** | Open the network panel (Wi-Fi, connection details, DNS). Navigate with arrows or h/j/k/l, Enter to act, Esc to close |
| **Mod+Ctrl+B** | Open the Bluetooth panel (pair, connect, forget, power). Same keys as the network panel; **B** toggles Bluetooth, **X** forgets a device, **Tab** switches between the panels |
| **Mod+Ctrl+C** | Open the calendar (also: click the clock in the bar; right-click the clock to change its date/time format). Left/right step a month, up/down a year, **T** returns to today, **W** switches the week start |
| **Mod+C** | Open Claude Code |
| **Mod+Shift+C** | Quick Claude prompt (wofi popup) |
| **Mod+T** | Theme picker |
| **Mod+Shift+D** | Open system dashboard |
| **Pause** / **Battery key** | Open system dashboard |
| **Mod+Shift+H** | Keybinding help overlay |
| **Mod+=** | Calculator (galculator) |

### Touchpad

| Shortcut | Action |
|----------|--------|
| **Mod+Shift+G** | Toggle disable-while-typing on/off |

### Media Keys

| Key | Action |
|-----|--------|
| **Fn+F2** | Brightness down |
| **Fn+F3** | Brightness up |
| **Fn+F6** | Mute / unmute |
| **Fn+F7** | Volume down (5%) |
| **Fn+F8** | Volume up (5%) |

### Screenshots & Recording

| Shortcut | Action |
|----------|--------|
| **Print** | Full screenshot (saved to ~/Pictures) |
| **Mod+Print** | Area screenshot to clipboard |
| **Mod+X** | Toggle screen recording (full screen, MP4 to ~/Videos) |

---

## Help Menu

Press **Mod+Shift+H** or click the keyboard icon in the waybar to open the help overlay. It parses your live sway config every time it runs, so it always reflects your current keybindings. Type to filter, press Escape to dismiss.

---

## System Dashboard

A Tauri-based desktop dashboard (`system-dashboard`) replaces the stock Argon dashboard that used to be bound to the hardware battery key. Open it with **Pause** (the hardware battery key), **Mod+Shift+D**, or the dashboard icon in waybar.

It surfaces seven live tiles, recoloring with the active theme:

| Tile | Click action |
|------|--------------|
| **Battery** | Charge % + current draw → time-remaining estimate. Turns green at 100%. |
| **Power** | Cycle CPU governor (ondemand → powersave → performance) |
| **Idle** | Cycle idle mode (on → off → claude) |
| **Theme** | Open theme picker |
| **Fan** | Open fan-mode picker |
| **Brightness** | Drag slider to set 5–100% |
| **Volume** | Drag slider to set 0–100% |

Polls at 2s while focused, 10s while blurred. On tiling WMs it hides client-side decorations; on stacking WMs (GNOME, KDE) the titlebar stays.

---

## Theme Switching

Twelve themes are available, each applying a coordinated color scheme across all apps simultaneously — sway window borders, waybar, foot terminals, mako notifications, swaylock, wofi, wob, GTK apps (including Thunar folder colors), Brave, Chromium, and the system dashboard.

**Available themes:** Catppuccin Frappe, Mocha, Latte, Macchiato, Dracula, Nord, Gruvbox Dark, Monokai Dark, Monokai Light, Arc Raiders, Osaka Jade, Sakurazuki.

| Method | Action |
|--------|--------|
| **Mod+T** | Open theme picker (wofi) |
| Waybar palette icon | Open theme picker |
| Dashboard theme tile | Open theme picker |
| `switch-theme <name>` | Switch directly (e.g., `switch-theme dracula`) |
| `switch-theme --wallpaper-picker` | Choose a wallpaper (overrides theme default) |
| `switch-theme --wallpaper <path>` | Set a specific wallpaper |
| `switch-theme --wallpaper-reset` | Revert to the current theme's default wallpaper |

Foot terminals are recolored live via OSC escape sequences — no restart needed. Browsers update live via managed policy files — no restart needed. The system dashboard file-watches its stylesheet and reloads colors automatically.

---

## Fan Control

The `argon-fan` daemon takes over PWM fan control (`/sys/class/hwmon/<pwmfan>/pwm1`) and applies a configurable temperature → PWM curve. Four modes are available, each with its own curve:

| Mode | Target |
|------|--------|
| **Silent** | Quietest — only ramps under sustained heat |
| **Normal** | Balanced (default; target <60°C) |
| **Turbo** | Aggressive cooling (target <50°C) |
| **Full** | Fan at 100% always |

| Method | Action |
|--------|--------|
| Waybar fan icon (click) | Open mode picker (wofi) |
| Dashboard fan tile | Open mode picker |
| `argon-fan picker` | Open mode picker from terminal |
| `argon-fan set <mode>` | Switch directly (e.g., `argon-fan set turbo`) |
| `argon-fan edit-config` | Edit `/etc/argon-fan/config.json` in `$EDITOR` |

On clean shutdown the daemon restores the kernel's automatic governor.

---

## Battery and Power Management

### Battery Status

The Argon ONE UP has its own battery, separate from the Pi's power supply. The battery icon in waybar shows the current charge level and whether it's charging. The battery is monitored by `argon-battery-rs`, a purpose-built Rust binary that reads the CW2217 fuel gauge IC directly over I2C.

### Automatic Power Behavior

On login, the `power-startup` script detects whether the laptop is on AC or battery and sets:

| State | Brightness | CPU Governor |
|-------|-----------|-------------|
| **AC power** | 100% | ondemand (scales with demand) |
| **Battery** | 40% | powersave (minimum frequency) |

When you plug/unplug the charger, `argon-battery-rs` detects the transition and automatically adjusts brightness and governor to match.

### Lid Close

Lid open/close events are watched by `argon-lid-monitor`, a small Rust binary that reads GPIO line 27 on `/dev/gpiochip0` directly (the Argon's lid sensor isn't exposed through ACPI). On close it invokes `lid-suspend close`, which:

- Locks the screen (swaylock)
- Turns the display off
- Switches CPU governor to powersave
- Blocks WiFi and Bluetooth
- Unbinds the webcam from USB

All are reversed on open. WiFi reconnects automatically.

### Changing Power Profile

Click the battery icon in waybar, click the dashboard power tile, or run `power-mode toggle` to cycle through CPU governors:

| Profile | Governor | Behavior |
|---------|----------|----------|
| **Balanced** | ondemand | CPU scales frequency with demand |
| **Powersave** | powersave | CPU stays at minimum frequency |
| **Performance** | performance | CPU stays at maximum frequency |

You can also set a specific profile: `power-mode powersave`, `power-mode performance`, `power-mode ondemand`.

### Brightness Controls

Brightness is controlled via DDC/CI over I2C bus 14, achieving ~1ms response time (much faster than standard backlight controls).

| Method | Action |
|--------|--------|
| **Fn+F2 / Fn+F3** | Brightness down / up (5% steps) |
| Dashboard brightness slider | Drag to set level |
| `brightness up` / `brightness down` | Manual adjustment from terminal |
| `brightness set <N>` | Set explicit level |

Brightness range: 5% – 100%. Current level is cached in `/tmp/.brightness_level`.

---

## Touchpad (disable while typing)

The AMIRA keyboard in the Argon ONE UP exposes its keyboard and touchpad as separate USB devices, so out of the box libinput never pairs them and its disable-while-typing does nothing. The installer tags both as internal (a libinput quirks file and a udev rule, from `touchpad-native/` — squeeby's approach), after which libinput's own disable-while-typing works with no daemon.

| Shortcut | Action |
|----------|--------|
| **Mod+Shift+G** | Toggle disable-while-typing on/off (the dashboard's Touchpad tile does the same) |

From a terminal: `touchpad-mode on`, `touchpad-mode off`, `touchpad-mode toggle`, `touchpad-mode status`. The choice is written to the sway config, so it survives a reboot.

The earlier `trackpad-guard` daemon is retired: the installer no longer sets it up, and removes it when upgrading an older install. Its source is still in `trackpad-guard/` for anyone who wants to build it by hand.

---

## Bluetooth Audio

WirePlumber is configured to auto-route audio to a Bluetooth device the moment it connects, preferring the A2DP profile. No manual sink switching needed for headphones or speakers. To pick a sink manually:

| Method | Action |
|--------|--------|
| Right-click waybar volume icon | Open audio sink picker (wofi) |
| `audio-sink-picker` | Open sink picker from terminal |

---

## Display Scaling

Adjust Sway's output scale to balance screen real estate vs readability on the 1920x1200 panel.

| Method | Action |
|--------|--------|
| Waybar magnifier icon | Open scale picker (wofi) |
| `sway-scale picker` | Open scale picker from terminal |

Available steps: 1x, 1.25x, 1.5x, 1.6x (default), 1.75x, 2x. The default is 1.6x, giving an effective resolution of 1200x750.

---

## Login Screen

The display manager is `greetd` with `gtkgreet` as the UI, themed to match Catppuccin Frappe. It runs under a minimal Sway session — no Plymouth, no GDM. If greetd fails to start, the install leaves GDM as a fallback.

---

## Bazaar Flatpak Store

If you installed Bazaar (Flatpak app store) during setup, you can browse and install Flatpak apps through its graphical interface. Launch it from the app launcher (Mod+D, search for "Bazaar").

Flatpak apps run sandboxed and are architecture-independent, which is useful on ARM where not every app has a native .deb package.

---

## Web Apps

If you installed WebApps (Linux Mint webapp-manager) during setup, you can pin websites as standalone windows with their own app icons — no browser tabs needed.

To create a web app:

1. Launch **Web Apps** from the app launcher (Mod+D)
2. Enter the URL (e.g., `https://slack.com`)
3. Give it a name and optionally choose an icon
4. Select which browser to use (Brave or Chromium)
5. Click **Install**

The web app will appear in your app launcher like any other application. This is especially useful for services like Slack, Teams, or other web-based tools that don't have native ARM packages.

---

## Screen Recording

A toggle-based screen recorder using `wf-recorder`:

- First run starts recording, second run stops it
- Recordings are saved to `~/Videos/` as MP4
- A notification confirms when recording starts and stops

| Method | Action |
|--------|--------|
| **Mod+X** | Toggle full-screen recording |
| `screen-record` | Toggle full-screen recording from terminal |
| `screen-record area` | Select a region first, then record |

---

## Configuration Files

These are the files you're most likely to want to customize:

| File | What it controls |
|------|-----------------|
| `~/.config/sway/config` | Keyboard shortcuts, autostart apps, gaps, borders, output scale |
| `~/.config/waybar/config` | Waybar modules, layout, click actions |
| `~/.config/foot/foot.ini` | Terminal font, size, padding |
| `~/.config/wofi/config` | App launcher dimensions, behavior |
| `~/.config/sway-themes/current` | Active theme name (text file) |
| `~/.config/sway-themes/<theme>` | Theme color definitions (35 color variables + wallpaper path) |
| `~/.config/sway-themes/templates/*` | Templates with `@@VARIABLE@@` placeholders rendered by switch-theme |
| `~/.config/mako/config` | Notification style, timeout, position |
| `~/.config/swaylock/config` | Lock screen appearance |
| `~/.config/gtk-3.0/settings.ini` | GTK theme, icon theme, font |
| `~/.config/fish/config.fish` | Shell aliases, environment variables, prompt behavior |
| `~/.config/starship.toml` | Starship prompt appearance |
| `~/.wallpapers/` | Wallpaper images (add your own here) |
| `~/.local/bin/` | All custom scripts (brightness, lid-suspend, power-mode, etc.) |
| `/etc/argon-fan/config.json` | Fan mode definitions and temperature/PWM curves |
| `/etc/argononeupd.conf` | Legacy Argon daemon settings (lid/fan now handled by Rust daemons) |
