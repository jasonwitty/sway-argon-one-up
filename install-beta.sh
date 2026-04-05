#!/bin/bash
# sway-argon-one-up installer
# Sets up a complete Sway desktop on the Argon ONE UP CM5 laptop
# from a fresh Raspberry Pi OS Lite (Trixie) install.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jasonwitty/sway-argon-one-up/main/install.sh | bash
#
# Prerequisites (do these before running):
#   1. Flash Raspberry Pi OS Lite (Trixie, 64-bit) and boot
#   2. Connect to WiFi
#   3. sudo apt update && sudo apt full-upgrade -y && sudo reboot
#   4. Set Wi-Fi country: sudo raspi-config nonint do_wifi_country US

set -euo pipefail

# ---------------------------------------------------------------------------
# Colors and formatting
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

info()    { echo -e "${BLUE}::${NC} $*"; }
success() { echo -e "${GREEN}OK${NC} $*"; }
warn()    { echo -e "${YELLOW}!!${NC} $*"; }
error()   { echo -e "${RED}ERROR${NC} $*" >&2; }

phase() {
    echo ""
    echo -e "${BOLD}${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}  $*${NC}"
    echo -e "${BOLD}${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# Prompt with description. Usage: prompt_yn "VARNAME" "Title" "description"
prompt_yn() {
    local varname="$1"
    local title="$2"
    local description="$3"

    echo ""
    echo -e "${BOLD}${title}${NC}"
    echo -e "${DIM}${description}${NC}"
    echo ""
    read -rp "Install ${title}? [y/N] " response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        eval "$varname=y"
    else
        eval "$varname=n"
    fi
}

REPO_URL="https://github.com/jasonwitty/sway-argon-one-up.git"
REPO_DIR="/tmp/sway-argon-one-up"

# ---------------------------------------------------------------------------
# Phase 1: Preflight checks
# ---------------------------------------------------------------------------
phase "Phase 1: Preflight checks"

# Must not be root
if [ "$(id -u)" -eq 0 ]; then
    error "Do not run this script as root. Run as your normal user — it will sudo when needed."
    exit 1
fi

# Architecture check
ARCH=$(uname -m)
if [ "$ARCH" != "aarch64" ]; then
    warn "Expected aarch64 architecture, got ${ARCH}. This installer is designed for Raspberry Pi."
    read -rp "Continue anyway? [y/N] " response
    [[ "$response" =~ ^[Yy]$ ]] || exit 1
fi

# Debian version check
if [ -f /etc/os-release ]; then
    . /etc/os-release
    if [[ "${VERSION_CODENAME:-}" != "trixie" ]]; then
        warn "Expected Debian Trixie, got ${VERSION_CODENAME:-unknown}."
        read -rp "Continue anyway? [y/N] " response
        [[ "$response" =~ ^[Yy]$ ]] || exit 1
    fi
else
    warn "Cannot determine OS version (/etc/os-release not found)."
fi

# Ensure this is RPi OS Lite (no pre-installed desktop)
if dpkg -l lightdm &>/dev/null 2>&1; then
    error "lightdm is installed — this looks like the full Raspberry Pi OS (Desktop)."
    error "This installer requires Raspberry Pi OS Lite (Trixie, 64-bit)."
    error "Please re-image with the Lite variant and try again."
    exit 1
fi
if [ -d /usr/share/raspi-ui-overrides ] || dpkg -l rpd-plymouth-splash &>/dev/null 2>&1; then
    error "A desktop environment appears to be pre-installed."
    error "This installer requires Raspberry Pi OS Lite (Trixie, 64-bit)."
    error "Please re-image with the Lite variant and try again."
    exit 1
fi
success "Raspberry Pi OS Lite detected"

# Internet connectivity
if ! ping -c 1 -W 3 github.com &>/dev/null; then
    error "No internet connectivity. Please connect to the network first."
    exit 1
fi
success "Internet connectivity"


# Disk space check (~1GB minimum)
AVAIL_KB=$(df --output=avail /home | tail -1 | tr -d ' ')
if [ "$AVAIL_KB" -lt 1048576 ]; then
    warn "Less than 1GB free disk space. Installation may fail."
    read -rp "Continue anyway? [y/N] " response
    [[ "$response" =~ ^[Yy]$ ]] || exit 1
else
    success "Disk space: $(( AVAIL_KB / 1024 ))MB available"
fi

success "Preflight checks passed"

# ---------------------------------------------------------------------------
# Phase 2: Optional package prompts
# ---------------------------------------------------------------------------
phase "Phase 2: Optional packages"

info "The following packages are optional. You will be asked about each one."
info "All prompts happen now — nothing is installed until you confirm."

INSTALL_BRAVE="n"
INSTALL_CHROMIUM="n"
INSTALL_WEBAPPS="n"
INSTALL_CLAUDE="n"

prompt_yn "INSTALL_BRAVE" "Brave Browser" \
    "Brave is a privacy-focused open-source browser with built-in ad and tracker
blocking enabled by default. It supports vertical tabs for a clean, minimal
look. This is the default browser for this setup and is fully integrated with
the theme switcher — title bar and color scheme update live on every theme change."

prompt_yn "INSTALL_CHROMIUM" "Chromium" \
    "Chromium carries the latest patches for Wayland screen sharing. If you need to
run web apps like Slack or Microsoft Teams and want to share your screen,
Chromium is the way to go. It is also fully integrated with the theme switcher,
so it works great as a daily driver too."

prompt_yn "INSTALL_WEBAPPS" "WebApps (Linux Mint)" \
    "Running on ARM, you will quickly find that not every app is packaged for your
architecture. WebApps lets you pin sites like Slack or Teams as standalone
windows with their own icons — no browser tabs needed."

prompt_yn "INSTALL_CLAUDE" "Claude Code" \
    "Claude Code is an AI coding assistant that lives in your terminal. It can read
your project, write and edit code, run commands, and help you think through
problems — all from the command line. This setup includes Mod+C to launch
Claude and Mod+Shift+C for a quick-prompt popup via wofi."

echo ""
info "Selections saved. Starting installation..."
echo ""

# ---------------------------------------------------------------------------
# Phase 3: System packages
# ---------------------------------------------------------------------------
phase "Phase 3: System packages"

info "Updating package lists..."
sudo apt update

info "Installing core packages..."
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

# Optional: Chromium
if [ "$INSTALL_CHROMIUM" = "y" ]; then
    info "Installing Chromium..."
    sudo apt install -y chromium
    success "Chromium installed"
fi

# Optional: Brave
if [ "$INSTALL_BRAVE" = "y" ]; then
    info "Installing Brave browser..."
    curl -fsS https://dl.brave.com/install.sh | sh
    success "Brave installed"
fi

# Optional: WebApps
if [ "$INSTALL_WEBAPPS" = "y" ]; then
    info "Installing WebApps (webapp-manager)..."
    WEBAPPS_DEB="/tmp/webapp-manager.deb"
    curl -fLo "$WEBAPPS_DEB" \
        "http://packages.linuxmint.com/pool/main/w/webapp-manager/webapp-manager_1.4.6_all.deb"
    sudo apt install -y "$WEBAPPS_DEB"
    rm -f "$WEBAPPS_DEB"
    success "WebApps installed"
fi

success "System packages installed"

# ---------------------------------------------------------------------------
# Phase 4: Rust toolchain
# ---------------------------------------------------------------------------
phase "Phase 4: Rust toolchain"

if command -v cargo &>/dev/null; then
    success "Rust toolchain already installed"
else
    info "Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    success "Rust toolchain installed"
fi

# Ensure cargo is in PATH for this session
source "$HOME/.cargo/env" 2>/dev/null || true

if command -v pfetch &>/dev/null; then
    success "pfetch already installed"
else
    info "Installing pfetch (system info display)..."
    cargo install --locked pfetch
    success "pfetch installed"
fi

# ---------------------------------------------------------------------------
# Phase 5: Fonts
# ---------------------------------------------------------------------------
phase "Phase 5: Fonts"

FONT_DIR="$HOME/.local/share/fonts"
if ls "$FONT_DIR"/JetBrainsMonoNerdFont*.ttf &>/dev/null; then
    success "JetBrainsMono Nerd Font already installed"
else
    info "Installing JetBrainsMono Nerd Font..."
    mkdir -p "$FONT_DIR"
    curl -fLo /tmp/JetBrainsMono.zip \
        https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.zip
    unzip -o /tmp/JetBrainsMono.zip -d "$FONT_DIR/"
    fc-cache -fv
    rm -f /tmp/JetBrainsMono.zip
    success "JetBrainsMono Nerd Font installed"
fi

# ---------------------------------------------------------------------------
# Phase 6: Shell tools (non-apt)
# ---------------------------------------------------------------------------
phase "Phase 6: Shell tools"

# Starship prompt
if command -v starship &>/dev/null; then
    success "Starship already installed"
else
    info "Installing Starship prompt..."
    curl -sS https://starship.rs/install.sh | sh -s -- -y
    success "Starship installed"
fi

# Atuin shell history
if command -v atuin &>/dev/null; then
    success "Atuin already installed"
else
    info "Installing Atuin shell history..."
    curl --proto '=https' --tlsv1.2 -LsSf https://setup.atuin.sh | sh -s -- --non-interactive
    success "Atuin installed"
fi

# papirus-folders
if [ -x "$HOME/.local/bin/papirus-folders" ]; then
    success "papirus-folders already installed"
else
    info "Installing papirus-folders..."
    mkdir -p "$HOME/.local/bin"
    curl -fLo "$HOME/.local/bin/papirus-folders" \
        https://raw.githubusercontent.com/PapirusDevelopmentTeam/papirus-folders/master/papirus-folders
    chmod +x "$HOME/.local/bin/papirus-folders"
    success "papirus-folders installed"
fi

# ---------------------------------------------------------------------------
# Phase 7: Argon ONE UP hardware
# ---------------------------------------------------------------------------
phase "Phase 7: Argon ONE UP hardware"

if [ -d /etc/argon ]; then
    success "Argon daemon already installed"
else
    info "Installing Argon ONE UP daemon..."
    curl https://download.argon40.com/argononeup.sh | bash
    success "Argon daemon installed"
fi

# Patch out the stock battery polling thread (replaced by argon-battery-rs)
if [ -f /etc/argon/argononeupd.py ]; then
    if grep -q '^[[:space:]]*t1\.start()' /etc/argon/argononeupd.py; then
        info "Patching Argon daemon: disabling stock battery polling thread..."
        sudo sed -i 's/^[[:space:]]*t1\.start()/#&/' /etc/argon/argononeupd.py
        success "Battery polling thread disabled"
    else
        success "Battery polling thread already patched"
    fi
else
    warn "Argon daemon script not found at /etc/argon/argononeupd.py — skipping patch"
fi

# Configure lid action
info "Configuring Argon lid action..."
sudo tee /etc/argononeupd.conf > /dev/null <<EOF
lidshutdownsecs=0
lidaction=suspend
EOF

# Restart daemon to pick up changes
if systemctl is-active argononed &>/dev/null; then
    sudo systemctl restart argononed
    success "Argon daemon restarted"
else
    warn "Argon daemon service not running — will start on next boot"
fi

# ---------------------------------------------------------------------------
# Phase 8: Clone repo and copy configs
# ---------------------------------------------------------------------------
phase "Phase 8: Desktop configuration"

if [ -d "$REPO_DIR" ]; then
    info "Repo already cloned, pulling latest..."
    cd "$REPO_DIR" && git pull
else
    info "Cloning sway-argon-one-up..."
    git clone "$REPO_URL" "$REPO_DIR"
fi

cd "$REPO_DIR"

info "Copying config files..."

# Sway and desktop configs
mkdir -p ~/.config
cp -r sway waybar wob wofi foot mako swaylock gtk-3.0 sway-themes fish ~/.config/
cp starship.toml ~/.config/

# Wallpapers
cp -r wallpapers ~/.wallpapers

# Scripts
mkdir -p ~/.local/bin
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/*

# GTK themes
mkdir -p ~/.themes
cp -r gtk-themes/* ~/.themes/

# Login screen
sudo cp greetd/config.toml greetd/sway-config greetd/gtkgreet.css greetd/wallpaper.png /etc/greetd/

success "Config files copied"

# ---------------------------------------------------------------------------
# Phase 9: Build argon-battery-rs
# ---------------------------------------------------------------------------
phase "Phase 9: Build argon-battery-rs"

if [ -x /usr/local/bin/argon-battery-rs ]; then
    success "argon-battery-rs already installed"
else
    info "Building argon-battery-rs (this may take a few minutes)..."
    cd "$REPO_DIR/argon-battery-rs"
    cargo build --release
    sudo cp target/release/argon-battery-rs /usr/local/bin/
    success "argon-battery-rs built and installed"
fi

# ---------------------------------------------------------------------------
# Phase 10: System configuration
# ---------------------------------------------------------------------------
phase "Phase 10: System configuration"

# Enable services
info "Enabling services..."
sudo systemctl enable --now seatd
sudo systemctl enable greetd
sudo systemctl disable --now power-profiles-daemon 2>/dev/null || true
success "Services configured"

# User groups
info "Adding user to required groups..."
sudo usermod -aG seat,video,audio,input,render,i2c "$USER"
success "User groups updated"

# Sudoers for lid-suspend, CPU governor, USB bind/unbind
info "Configuring sudoers for power management..."
sudo tee /etc/sudoers.d/lid-power > /dev/null <<SUDOEOF
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block bluetooth
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock bluetooth
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/unbind
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/bind
SUDOEOF
sudo visudo -cf /etc/sudoers.d/lid-power
success "Power management sudoers configured"

# Browser theme sudoers (only if Brave or Chromium selected)
if [ "$INSTALL_BRAVE" = "y" ] || [ "$INSTALL_CHROMIUM" = "y" ]; then
    info "Configuring sudoers for browser theme integration..."
    sudo mkdir -p /etc/brave/policies/managed /etc/chromium/policies/managed
    sudo tee /etc/sudoers.d/browser-theme > /dev/null <<SUDOEOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/brave/policies/managed/color.json
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/chromium/policies/managed/color.json
SUDOEOF
    sudo visudo -cf /etc/sudoers.d/browser-theme
    success "Browser theme sudoers configured"
fi

# Prevent logind from handling lid switch (Pi5 has no suspend support)
info "Configuring logind lid switch override..."
sudo mkdir -p /etc/systemd/logind.conf.d
sudo tee /etc/systemd/logind.conf.d/lid-ignore.conf > /dev/null <<EOF
[Login]
HandleLidSwitch=ignore
HandleLidSwitchExternalPower=ignore
HandleLidSwitchDocked=ignore
EOF
success "Logind lid switch set to ignore"

# Set fish as default shell
info "Setting fish as default shell..."
sudo chsh -s /usr/bin/fish "$USER"
success "Default shell set to fish"

# Install socktop
if command -v socktop &>/dev/null; then
    success "socktop already installed"
else
    info "Installing socktop..."
    curl -fsSL https://jasonwitty.github.io/socktop/KEY.gpg | \
        sudo gpg --dearmor -o /usr/share/keyrings/socktop-archive-keyring.gpg
    echo "deb [signed-by=/usr/share/keyrings/socktop-archive-keyring.gpg] https://jasonwitty.github.io/socktop stable main" | \
        sudo tee /etc/apt/sources.list.d/socktop.list > /dev/null
    sudo apt update
    sudo apt install -y socktop socktop-agent
    sudo systemctl enable --now socktop-agent
    success "socktop installed"
fi

# ---------------------------------------------------------------------------
# Phase 11: Claude Code (if selected)
# ---------------------------------------------------------------------------
if [ "$INSTALL_CLAUDE" = "y" ]; then
    phase "Phase 11: Claude Code"

    if command -v claude &>/dev/null; then
        success "Claude Code already installed"
    else
        # Install Node.js if not present
        if ! command -v node &>/dev/null; then
            info "Installing Node.js..."
            curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
            sudo apt install -y nodejs
            success "Node.js installed"
        fi

        info "Installing Claude Code..."
        sudo npm install -g @anthropic-ai/claude-code
        success "Claude Code installed"
        echo ""
        info "Run 'claude' after rebooting to authenticate."
    fi
fi

# ---------------------------------------------------------------------------
# Phase 12: Cleanup and finish
# ---------------------------------------------------------------------------
phase "Installation complete!"

rm -rf "$REPO_DIR"

echo -e "${GREEN}${BOLD}Your Sway desktop is ready.${NC}"
echo ""
echo "Next steps:"
echo "  1. Reboot:  sudo reboot"
echo "  2. Log in at the gtkgreet login screen"
echo "  3. Press Mod+T to pick a theme"
echo ""
echo "Keybindings:"
echo "  Mod+Enter     Terminal"
echo "  Mod+D         App launcher"
echo "  Mod+T         Theme picker"
echo "  Mod+B         Brave browser"
echo "  Mod+C         Claude Code"
echo "  Mod+=         Calculator"
echo "  Mod+Shift+H   Keybinding help"
echo "  Mod+Shift+R   Screen record"
echo ""
