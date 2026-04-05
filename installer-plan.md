# Installer Script Plan

## Overview

A single `curl | bash` installer that takes a fresh Raspberry Pi OS Lite (Trixie, 64-bit, headless) system — after the user has created their account, expanded the filesystem, and connected to WiFi — and turns it into a fully configured Sway desktop.

```bash
curl -fsSL https://raw.githubusercontent.com/jasonwitty/sway-argon-one-up/main/install.sh | bash
```

## Prerequisites (user does manually before running installer)

The README will guide users through these steps before running the installer:

1. Flash **Raspberry Pi OS Lite** (minimal, no desktop, Debian Trixie, 64-bit) via Raspberry Pi Imager
2. Boot, create user account (Imager pre-configures this)
3. Connect to WiFi (`nmtui` or Imager pre-config)
4. Run `sudo apt update && sudo apt full-upgrade -y && sudo reboot`
5. Fix NVMe power management — append to `/boot/firmware/cmdline.txt`:
   ```
   nvme_core.default_ps_max_latency_us=0 pcie_aspm=off
   ```
   Then `sudo reboot`.
6. Set Wi-Fi regulatory domain:
   ```bash
   sudo raspi-config nonint do_wifi_country US
   ```

## Installer Flow

### Phase 1: Preflight checks

- Verify running on aarch64 (Raspberry Pi)
- Verify Debian Trixie (or compatible) via `/etc/os-release`
- Verify running as the logged-in user (not root) — script uses `sudo` where needed
- Verify internet connectivity (ping test)
- Check NVMe kernel params are set (`/proc/cmdline`), warn if missing
- Check sufficient disk space (~500MB for packages + ~200MB for Rust toolchain)

### Phase 2: Optional package prompts

Prompt the user **up front** for all optional packages, storing choices in variables. Each prompt includes a description and `[y/N]` default. All prompts happen before any installation begins.

1. **Brave Browser**
   > Brave is a privacy-focused open-source browser with built-in ad and tracker
   > blocking enabled by default. It supports vertical tabs for a clean, minimal
   > look. This is the default browser for this setup and is fully integrated with
   > the theme switcher — title bar and color scheme update live on every theme change.

2. **Chromium**
   > Chromium carries the latest patches for Wayland screen sharing. If you need to
   > run web apps like Slack or Microsoft Teams and want to share your screen,
   > Chromium is the way to go. It is also fully integrated with the theme switcher,
   > so it works great as a daily driver too.

3. **WebApps** (Linux Mint webapp-manager)
   > Running on ARM, you will quickly find that not every app is packaged for your
   > architecture. WebApps lets you pin sites like Slack or Teams as standalone
   > windows with their own icons — no browser tabs needed.

4. **Claude Code**
   > Claude Code is an AI coding assistant that lives in your terminal. It can read
   > your project, write and edit code, run commands, and help you think through
   > problems — all from the command line. This setup includes Mod+C to launch
   > Claude and Mod+Shift+C for a quick-prompt popup via wofi.

### Phase 3: System packages

```bash
sudo apt install -y \
  sway swaybg swayidle swaylock xwayland \
  waybar wofi foot wob mako-notifier \
  greetd gtkgreet \
  seatd pipewire wireplumber \
  network-manager network-manager-gnome \
  ukui-polkit \
  ddcutil i2c-tools \
  fish \
  bat eza fzf zoxide ugrep \
  grim slurp wl-clipboard wf-recorder \
  fonts-firacode \
  thunar mpv imv file-roller galculator zathura \
  blueman hwinfo neovim micro \
  papirus-icon-theme libglib2.0-bin \
  python3 \
  git curl build-essential pkg-config unzip
```

Install optional apt packages based on Phase 2 choices:
- **Chromium:** `sudo apt install -y chromium`
- **Brave:** add Brave apt repo + signing key, then `sudo apt install -y brave-browser-stable`
- **WebApps:** install `webapp-manager` from Linux Mint repo (or from source if PPA doesn't support Trixie aarch64 — needs testing)

### Phase 4: Rust toolchain and cargo packages

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cargo install --locked pfetch-rs
```

Note: `bat` and `eza` are installed via apt in Phase 3 (faster than compiling from source). Only `pfetch-rs` needs cargo.

### Phase 5: Fonts

```bash
mkdir -p ~/.local/share/fonts
curl -fLo /tmp/JetBrainsMono.zip \
  https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
unzip -o /tmp/JetBrainsMono.zip -d ~/.local/share/fonts/
fc-cache -fv
rm -f /tmp/JetBrainsMono.zip
```

### Phase 6: Shell tools (non-apt)

```bash
# Starship prompt
curl -sS https://starship.rs/install.sh | sh -s -- -y

# Atuin shell history
curl --proto '=https' --tlsv1.2 -LsSf https://setup.atuin.sh | sh

# papirus-folders (for per-theme folder icon colors)
mkdir -p ~/.local/bin
curl -fLo ~/.local/bin/papirus-folders \
  https://raw.githubusercontent.com/PapirusDevelopmentTeam/papirus-folders/master/papirus-folders
chmod +x ~/.local/bin/papirus-folders
```

### Phase 7: Argon ONE UP hardware

```bash
# Install Argon case daemon (fan control, power button, lid switch)
curl https://download.argon40.com/argononeup.sh | bash

# Patch the daemon to disable its built-in battery polling thread.
# argon-battery-rs replaces this with a more efficient Rust implementation.
# Comment out the t1 (battery_check) thread start in argononeupd.py.
sudo sed -i 's/^[[:space:]]*t1\.start()/#&/' /etc/argon/argononeupd.py

# Configure lid action
sudo tee /etc/argononeupd.conf > /dev/null <<EOF
lidshutdownsecs=0
lidaction=suspend
EOF

# Restart the daemon to pick up changes
sudo systemctl restart argononed
```

### Phase 8: Clone repo and copy configs

```bash
git clone https://github.com/jasonwitty/sway-argon-one-up.git /tmp/sway-argon-one-up
cd /tmp/sway-argon-one-up

# Sway and desktop configs
cp -r sway waybar wob wofi foot mako swaylock gtk-3.0 sway-themes fish ~/.config/
cp starship.toml ~/.config/

# Wallpapers
cp -r wallpapers ~/.wallpapers

# Scripts
mkdir -p ~/.local/bin
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/*

# GTK themes (bundled in repo with upstream licenses)
cp -r gtk-themes/* ~/.themes/

# Login screen
sudo cp greetd/config.toml greetd/sway-config greetd/gtkgreet.css greetd/wallpaper.png /etc/greetd/
```

### Phase 9: Build and install argon-battery-rs

```bash
cd /tmp/sway-argon-one-up/argon-battery-rs
cargo build --release
sudo cp target/release/argon-battery-rs /usr/local/bin/
```

### Phase 10: System configuration

```bash
# Enable services
sudo systemctl enable --now seatd
sudo systemctl enable greetd
sudo systemctl disable --now power-profiles-daemon 2>/dev/null || true

# User groups
sudo usermod -aG seat,video,audio,input,render,i2c "$USER"

# Sudoers for lid-suspend, CPU governor, USB bind/unbind
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

# Browser theme sudoers (only if Brave or Chromium selected)
if [ "$INSTALL_BRAVE" = "y" ] || [ "$INSTALL_CHROMIUM" = "y" ]; then
    sudo mkdir -p /etc/brave/policies/managed /etc/chromium/policies/managed
    sudo tee /etc/sudoers.d/browser-theme > /dev/null <<EOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/brave/policies/managed/color.json
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/chromium/policies/managed/color.json
EOF
    sudo visudo -cf /etc/sudoers.d/browser-theme
fi

# Prevent logind from handling lid switch (Pi5 has no suspend support)
sudo mkdir -p /etc/systemd/logind.conf.d
sudo tee /etc/systemd/logind.conf.d/lid-ignore.conf > /dev/null <<EOF
[Login]
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore
EOF

# Set fish as default shell
chsh -s /usr/bin/fish

# Install socktop (system monitor)
curl -fsSL https://jasonwitty.github.io/socktop/KEY.gpg | \
  sudo gpg --dearmor -o /usr/share/keyrings/socktop-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/socktop-archive-keyring.gpg] https://jasonwitty.github.io/socktop stable main" | \
  sudo tee /etc/apt/sources.list.d/socktop.list
sudo apt update
sudo apt install -y socktop socktop-agent
sudo systemctl enable --now socktop-agent
```

### Phase 11: Claude Code (if selected)

```bash
# Install via npm (Node.js required)
if ! command -v node &>/dev/null; then
    curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
    sudo apt install -y nodejs
fi
sudo npm install -g @anthropic-ai/claude-code

echo ""
echo "Claude Code installed. Run 'claude' to authenticate."
```

### Phase 12: Cleanup and finish

```bash
rm -rf /tmp/sway-argon-one-up

echo ""
echo "=========================================="
echo "  Installation complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo "  1. Reboot:  sudo reboot"
echo "  2. Log in at the gtkgreet login screen"
echo "  3. Press Mod+T to pick a theme"
echo ""
echo "Keybindings:"
echo "  Mod+Enter    Terminal"
echo "  Mod+D        App launcher"
echo "  Mod+T        Theme picker"
echo "  Mod+B        Brave browser"
echo "  Mod+C        Claude Code"
echo "  Mod+Shift+H  Keybinding help"
echo ""
```

## Script Structure

The installer script (`install.sh`) should be structured as follows:

```
install.sh
├── Color/formatting variables (green checkmarks, red errors, etc.)
├── Helper functions
│   ├── info(), success(), warn(), error() — colored output
│   ├── prompt_yn() — yes/no prompt with description
│   └── check_command() — verify a command exists after install
├── Phase 1: Preflight
├── Phase 2: Optional prompts
├── Phase 3–11: Installation (each phase wrapped in a function)
│   └── Each function checks if its work is already done (idempotent)
└── Phase 12: Cleanup and summary
```

**Idempotency:** The script should be safe to re-run. Each phase should check whether its work is already done before acting:
- Don't re-install packages that are already installed
- Don't re-clone if the repo already exists in /tmp
- Don't duplicate sudoers entries
- Don't re-add groups the user is already in

**Error handling:** Use `set -euo pipefail` at the top. Wrap critical sections in functions with clear error messages. If a phase fails, print what failed and suggest how to resume.

## Decisions

1. **WebApps** — Confirmed working on Trixie aarch64, installed manually. Installer will replicate.
2. **Claude Code** — Install via npm + Node.js. Adds ~100MB but straightforward.
3. **Fish theme** — Hardcoded Dracula theme line commented out in config. Theme switcher handles terminal colors.
4. **Argon daemon patch** — Use `sed` to comment out `t1.start()`. Verify the line exists before patching and warn if not found. Licensing question to be filed with Argon40 later.
