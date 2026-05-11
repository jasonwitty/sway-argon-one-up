# sway-argon-one-up

A complete Sway desktop environment for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system), a 14-inch laptop powered by the Raspberry Pi Compute Module 5. Includes a one-command installer, a 10-theme switcher with matching wallpapers and live terminal recoloring, a dynamic keybinding help overlay, instant brightness control via direct I2C, display scaling controls, custom Rust daemons for battery / fan / lid / touchpad / audio, a Tauri-based system dashboard, automatic Bluetooth audio routing, and Claude Code integration.

![screenshot](screenshot.png)

## Quick Install

Starting from a fresh [Raspberry Pi OS Lite](https://www.raspberrypi.com/software/) (Trixie, 64-bit) installation:

```bash
curl -fsSL https://raw.githubusercontent.com/jasonwitty/sway-argon-one-up/main/install.sh | bash
```

See the [Installation Guide](Installation.md) for detailed step-by-step instructions including how to flash the NVMe drive and configure initial settings.

## Hardware

This setup is built for the [Argon ONE UP CM5 Laptop](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) which uses a Raspberry Pi Compute Module 5. The display is connected via HDMI internally, so standard backlight controls don't apply — brightness is controlled by writing DDC/CI commands directly to the display over I2C bus 14, achieving ~1ms response time. This approach was inspired by [esvertit's display calibration guide](https://forum.argon40.com/t/guide-professional-display-calibration-on-argon-one-up/9188) on the Argon40 forum. The Argon case also has its own battery, monitored by a purpose-built Rust binary ([argon-battery-rs](argon-battery-rs/)).

## What's Included

| Component | Description |
|-----------|-------------|
| `sway/` | Sway config with themed window colors, idle lock, touchpad, media keys, system-dashboard bindings |
| `waybar/` | Top bar with workspaces, clock, CPU, volume, backlight, fan, Argon battery, idle/lock toggle, audio sink picker, tray, theme/scale/Claude/help/power buttons |
| `sway-themes/` | 10 theme definitions + templates for all themed apps (sway, waybar, foot, mako, swaylock, wofi, wob, Brave, Chromium, Thunar/GTK, system-dashboard) |
| `wallpapers/` | Matching wallpaper for each theme (auto-applied on theme switch) |
| `bin/` | `switch-theme`, `sway-scale`, `sway-help`, `claude-prompt`, `brightness`, `start-wob`, `lid-suspend`, `power-mode`, `power-startup`, `powermenu`, `screen-record`, `argon-battery`, `idle-toggle`, `audio-sink-picker` |
| `foot/` | Terminal emulator config with live-recolored theme support |
| `wofi/` | App launcher and help overlay styles |
| `wob/` | Wayland Overlay Bar config for brightness/volume indicators |
| `mako/` | Notification daemon themed to match |
| `swaylock/` | Lock screen with themed ring indicator |
| `gtk-3.0/` | GTK theme settings (switched automatically per theme) |
| `gtk-themes/` | 8 bundled GTK themes with upstream licenses |
| `fish/` | Fish shell config |
| `starship.toml` | Starship prompt config |
| `greetd/` | Login screen: greetd + gtkgreet with Catppuccin Frappe theme |
| `wireplumber/` | WirePlumber policy: auto-route audio to Bluetooth devices on connect, prefer A2DP |
| `mpv/` | mpv config tuned for Pi 5 (hardware-friendly defaults) |
| `argon-battery-rs/` | Rust battery monitor; CW2217 fuel-gauge over I2C, auto brightness + governor switching on AC/battery transitions |
| `argon-fan/` | Rust PWM fan-control daemon with mode-based curves (silent / normal / turbo / full); waybar tile + wofi picker |
| `argon-lid-monitor/` | Rust GPIO 27 lid-event monitor; shells to `lid-suspend` to lock + sleep displays and switch governor on close |
| `trackpad-guard/` | Rust DWT (disable-while-typing) daemon for the AMIRA combo keyboard/touchpad; `Mod+Shift+T` triggers a USB rebind rescue |
| `system-dashboard/` | Tauri 2 desktop app bound to the hardware battery key (`Pause`): battery, power profile, idle mode, theme, fan, brightness slider, volume slider — all live |

## Themes

Switch between 10 themes with **Mod+T** or the waybar palette icon. Every themed app updates simultaneously — sway window borders, waybar, foot terminals, mako notifications, swaylock, wofi, wob, GTK apps (including Thunar folder colors), Brave, Chromium, and the wallpaper.

<p>
<img src="screenshots/theme-frappe.png" width="19%" alt="Catppuccin Frappe">
<img src="screenshots/theme-mocha.png" width="19%" alt="Catppuccin Mocha">
<img src="screenshots/theme-latte.png" width="19%" alt="Catppuccin Latte">
<img src="screenshots/theme-macchiato.png" width="19%" alt="Catppuccin Macchiato">
<img src="screenshots/theme-dracula.png" width="19%" alt="Dracula">
</p>
<p>
<img src="screenshots/theme-nord.png" width="19%" alt="Nord">
<img src="screenshots/theme-gruvbox.png" width="19%" alt="Gruvbox Dark">
<img src="screenshots/theme-monokai-dark.png" width="19%" alt="Monokai Dark">
<img src="screenshots/theme-monokai-light.png" width="19%" alt="Monokai Light">
<img src="screenshots/theme-arc-raiders.png" width="19%" alt="Arc Raiders">
</p>

*Frappe, Mocha, Latte, Macchiato, Dracula, Nord, Gruvbox Dark, Monokai Dark, Monokai Light, Arc Raiders*

## Documentation

| Document | Description |
|----------|-------------|
| [Installation Guide](Installation.md) | Step-by-step setup from a blank NVMe drive to a working desktop |
| [Applications](Applications.md) | Complete list of installed software with descriptions |
| [Usage Manual](Usage_Manual.md) | Keyboard shortcuts, theme switching, power management, configuration reference |
| [Standalone Guides](Stand_Alone_Guides.md) | Self-contained guides for specific features you can add to your own setup |
| [Troubleshooting](Troubleshooting.md) | Common problems and solutions |

## License

[Apache License 2.0](LICENSE)

Bundled GTK themes retain their original licenses (GPL-3.0, MIT) — see individual theme directories under `gtk-themes/` for details.
