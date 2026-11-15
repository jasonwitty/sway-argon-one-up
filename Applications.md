# Applications

Every package below is installed by the Sway Argon ONE UP installer (`install.sh`). Packages marked **Required** are installed unconditionally; packages marked **Optional** are only installed when the user selects them at the interactive prompt.

## Core Desktop

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| sway | Tiling Wayland compositor. The window manager and primary desktop session. | Required |
| swaybg | Wallpaper setter for Sway. Displays the active theme wallpaper. | Required |
| swayidle | Idle management daemon for Wayland. Handles screen lock and suspend after inactivity. | Required |
| swaylock | Screen locker for Sway. Activated by swayidle and the lock keybinding. | Required |
| xwayland | X11 compatibility layer for Wayland. Runs legacy X11 apps inside the Sway session. | Required |
| waybar | Status bar for Wayland compositors. Displays workspaces, clock, battery, volume, brightness, and system info. | Required |
| wofi | Application launcher for Wayland. Used for app launching (Mod+D), theme picker (Mod+T), and quick-prompt popups. | Required |
| foot | Fast, lightweight Wayland terminal emulator. The default terminal, with live theme recoloring on theme switch. | Required |
| wob | Wayland overlay bar. Shows on-screen volume and brightness indicators. | Required |
| mako-notifier | Lightweight notification daemon for Wayland. Displays desktop notifications from libnotify. | Required |
| xdg-desktop-portal-wlr | XDG desktop portal backend for wlroots. Enables screen sharing in browsers and Flatpak apps. | Required |

## System Services

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| greetd | Minimal login manager daemon. Manages the graphical login session. | Required |
| gtkgreet | GTK-based greeter for greetd. Provides the Catppuccin-themed login screen. | Required |
| seatd | Minimal seat management daemon. Grants Sway access to input and display devices without logind. | Required |
| pipewire | Multimedia server for audio and video. Handles all audio playback and recording. | Required |
| wireplumber | Session and policy manager for PipeWire. Manages audio routing and device selection. | Required |
| network-manager | Network connection manager. Handles Wi-Fi and wired network connections. | Required |
| quickshell | QML shell toolkit that runs the network and Bluetooth panels. Not packaged for trixie, so the installer uses a prebuilt release or builds it from source (`quickshell-panels/quickshell-build/`). | Required |
| network panel | Port of the Omarchy network manager (`network-panel/`): Wi-Fi list and connect, connection details, band pinning, DNS provider, Wi-Fi QR share. Opens from the waybar network icon or **Mod+Ctrl+N**. | Required |
| iw, qrencode | Used by the network panel for link details and the Wi-Fi share QR code. | Required |
| ukui-polkit | Polkit authentication agent. Handles privilege-escalation prompts for GUI apps. | Required |

## Display / Brightness

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| ddcutil | DDC/CI monitor control utility. Adjusts external display brightness over HDMI. | Required |
| i2c-tools | I2C bus utilities. Required by ddcutil for DDC communication with the display. | Required |

## Shell & CLI Tools

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| fish | Friendly interactive shell. The default login shell, with custom config and theme integration. | Required |
| starship | Cross-shell prompt. Provides the customized terminal prompt (installed via curl). | Required |
| atuin | Shell history manager with sync and search. Replaces default shell history with fuzzy-searchable history (installed via curl). | Required |
| bat | Cat clone with syntax highlighting and git integration. Used as the default pager (`batcat` symlinked to `bat`). | Required |
| eza | Modern replacement for `ls`. Provides colorized, icon-rich file listings. | Required |
| fzf | Command-line fuzzy finder. Powers interactive file and history search in fish. | Required |
| zoxide | Smarter `cd` command that learns your habits. Provides fast directory jumping. | Required |
| ugrep | Ultra-fast grep with interactive TUI. Used for file content searching. | Required |
| neovim | Hyperextensible terminal text editor. Available as a power-user editor. | Required |
| micro | Modern, intuitive terminal text editor. Available as an easy-to-use editor. | Required |
| git | Distributed version control system. Used during installation and general development. | Required |
| curl | Command-line HTTP client. Used during installation and general use. | Required |
| build-essential | C/C++ compiler and make tools. Required for building argon-battery-rs and native extensions. | Required |
| pkg-config | Compiler helper for finding libraries. Required for building Rust crates with native dependencies. | Required |
| unzip | Archive extraction utility. Used to unpack the Nerd Font zip during installation. | Required |
| libnotify-bin | Desktop notification command-line tools (`notify-send`). Used by scripts to send notifications to mako. | Required |
| hwinfo | Hardware information tool. Used for system diagnostics and hardware detection. | Required |
| python3 | Python interpreter. Required by the Argon ONE UP daemon. | Required |
| xdg-user-dirs | Tool to manage well-known user directories. Creates standard Desktop, Documents, Downloads, etc. folders. | Required |

## Screenshot / Recording

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| grim | Screenshot utility for Wayland. Captures full-screen or region screenshots. | Required |
| slurp | Region selector for Wayland. Lets you draw a rectangle to select a screen region for grim. | Required |
| wl-clipboard | Wayland clipboard utilities (`wl-copy`, `wl-paste`). Copies screenshots and text to the clipboard. | Required |
| wf-recorder | Screen recorder for Wayland. Records screen video (Mod+Shift+R). | Required |

## Fonts

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| fonts-firacode | Fira Code programming font with ligatures. Available as a UI and coding font. | Required |
| JetBrainsMono Nerd Font | JetBrains Mono patched with Nerd Font icons. The primary terminal and waybar font, providing powerline and icon glyphs (installed via curl/zip). | Required |

## Desktop Apps

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| thunar | Lightweight GTK file manager. The default file manager (Mod+N). | Required |
| mpv | Minimalist media player. The default video and audio player. | Required |
| imv | Lightweight Wayland image viewer. The default image viewer. | Required |
| file-roller | Archive manager with GUI. Handles zip, tar, and other archive formats. | Required |
| galculator | GTK calculator. Launched with Mod+=. | Required |
| zathura | Minimalist document viewer. The default PDF viewer. | Required |
| bluetooth panel | Port of the Omarchy Bluetooth panel (`bluetooth-panel/`): power switch, paired and nearby devices, pair / connect / disconnect / forget, battery level. Opens from the waybar Bluetooth icon or **Mod+Ctrl+B**; right-click the icon to turn Bluetooth on or off. | Required |
| bluez-tools | Provides `bt-agent`, the pairing agent the Bluetooth panel relies on. | Required |

## Theming

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| papirus-icon-theme | Icon theme for Linux desktops. Provides consistent folder and app icons across all themes. | Required |
| papirus-folders | Script to recolor Papirus folder icons. Changes folder icon color to match the active theme (installed via curl). | Required |
| libglib2.0-bin | GLib utility binaries including `gsettings`. Used to apply GTK theme, icon theme, and font settings. | Required |
| gsettings-desktop-schemas | GSettings schema collection for desktop settings. Required by gsettings for GTK theming. | Required |
| GTK themes | Bundled GTK3 themes (Catppuccin, Dracula, Nordic, Gruvbox, Monokai, etc.). Copied to ~/.themes and activated by the theme switcher. | Required |

## Hardware (Argon ONE UP)

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| Argon ONE UP daemon | Vendor daemon for Argon ONE UP case hardware. Controls the fan, power button, and lid switch (installed via curl from argon40.com). | Required |
| argon-battery-rs | Rust-based battery monitor for the Argon ONE UP. Polls battery state and reports to waybar; replaces the stock Python battery thread (built from repo source). | Required |
| socktop | SoC performance monitor. Provides real-time system-on-chip metrics (installed via apt from a custom repository). | Required |
| socktop-agent | Background agent for socktop. Collects SoC telemetry for the socktop dashboard. | Required |

## Rust Toolchain

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| rustup | Rust toolchain installer and manager. Installs the Rust compiler and cargo (installed via curl from rustup.rs). | Required |
| pfetch | Minimal system information display written in Rust. Shows a brief system summary in new terminal sessions (installed via `cargo install`). | Required |

## Optional

| Name | Description | Required/Optional |
|------|-------------|-------------------|
| Brave Browser | Privacy-focused browser with built-in ad and tracker blocking. The default browser for this setup, fully integrated with the theme switcher for live color scheme updates (installed via curl from brave.com). | Optional |
| Chromium | Open-source web browser with up-to-date Wayland screen sharing patches. Fully integrated with the theme switcher; recommended if you need web-app screen sharing for Slack or Teams. | Optional |
| webapp-manager | Linux Mint WebApps manager. Lets you pin websites as standalone desktop windows with their own icons -- useful on ARM where native apps may not be available (installed via .deb from linuxmint.com). | Optional |
| Flatpak | Application sandboxing and distribution framework. Enables installing apps from Flathub when native ARM packages are unavailable. | Optional |
| Bazaar | Modern graphical app store for Flatpak. Provides a GUI for browsing and installing Flatpak apps from Flathub (installed as a Flatpak). | Optional |
| Claude Code | AI coding assistant for the terminal. Reads projects, writes code, and runs commands from the command line; bound to Mod+C with a wofi quick-prompt on Mod+Shift+C (installed via curl from claude.ai). | Optional |
