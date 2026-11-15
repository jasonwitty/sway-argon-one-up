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
    read -rp "Install ${title}? [y/N] " response </dev/tty
    if [[ "$response" =~ ^[Yy]$ ]]; then
        eval "$varname=y"
    else
        eval "$varname=n"
    fi
}

REPO_URL="https://github.com/jasonwitty/sway-argon-one-up.git"
# Branch to install from. Override to test a release candidate:
#   curl -fsSL https://raw.githubusercontent.com/jasonwitty/sway-argon-one-up/<branch>/install.sh | REPO_BRANCH=<branch> bash
REPO_BRANCH="${REPO_BRANCH:-main}"
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
    read -rp "Continue anyway? [y/N] " response </dev/tty
    [[ "$response" =~ ^[Yy]$ ]] || exit 1
fi

# Debian version check
if [ -f /etc/os-release ]; then
    # shellcheck source=/dev/null
    . /etc/os-release
    if [[ "${VERSION_CODENAME:-}" != "trixie" ]]; then
        warn "Expected Debian Trixie, got ${VERSION_CODENAME:-unknown}."
        read -rp "Continue anyway? [y/N] " response </dev/tty
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
    read -rp "Continue anyway? [y/N] " response </dev/tty
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
INSTALL_WIDEVINE="n"
INSTALL_CHROMIUM="n"
INSTALL_WEBAPPS="n"
INSTALL_FLATPAK="n"
INSTALL_CLAUDE="n"

# BRAVE_MODE: origin = install Brave Origin, keep = stay on the regular Brave
# that is already installed, none = no Brave.
BRAVE_MODE="none"
pkg_installed() { dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q "install ok installed"; }

if pkg_installed brave-origin; then
    # Already on Brave Origin (a re-run): nothing to ask.
    BRAVE_MODE="origin"
    info "Brave Origin is already installed — keeping it."
elif pkg_installed brave-browser; then
    # Installed by an earlier release, when this setup shipped regular Brave.
    echo ""
    echo -e "${BOLD}Brave: this setup has moved to Brave Origin${NC}"
    echo -e "${DIM}You have the regular Brave browser from an earlier version of this installer.
The default browser here is now Brave Origin, the debloated build of the same
browser: Rewards, Wallet, VPN, Leo AI, News, Talk and Tor are stripped out.

  1) Migrate to Brave Origin. Installs it, copies your Brave profile (bookmarks,
     history, extensions, settings) across, and points Mod+B at it. Regular
     Brave and its profile are left in place, untouched, until you remove them
     yourself (sudo apt remove brave-browser).
  2) Keep your current Brave. Nothing is installed; Mod+B keeps opening it.${NC}"
    echo ""
    read -rp "Choose [1/2] (default 2, keep current Brave): " response </dev/tty
    if [ "$response" = "1" ]; then
        BRAVE_MODE="origin"
    else
        BRAVE_MODE="keep"
    fi
else
    prompt_yn "INSTALL_BRAVE" "Brave Origin" \
    "Brave Origin is the debloated build of Brave: the same privacy-focused engine
with built-in ad and tracker blocking, but with Rewards, Wallet, VPN, Leo AI, News,
Talk, Tor and other extras stripped out (free on Linux, arm64 supported). It supports
vertical tabs for a clean, minimal look. This is the default browser for this setup
and is fully integrated with the theme switcher — title bar and color scheme update
live on every theme change."
    [ "$INSTALL_BRAVE" = "y" ] && BRAVE_MODE="origin"
fi
# The rest of the script asks two things: is Brave Origin to be installed
# (INSTALL_BRAVE), and is there a Brave of either kind to integrate (HAS_BRAVE).
INSTALL_BRAVE="n"; HAS_BRAVE="n"
[ "$BRAVE_MODE" = "origin" ] && INSTALL_BRAVE="y"
[ "$BRAVE_MODE" != "none" ] && HAS_BRAVE="y"

prompt_yn "INSTALL_WIDEVINE" "Widevine DRM (Netflix, etc.) for Brave" \
    "Widevine is the DRM module streaming services like Netflix, Amazon Prime, and
Spotify require. Brave on ARM ships DRM support but does not fetch the module on
its own, so this installs the Raspberry Pi Widevine package and enables it in
Brave. Requires Brave (either kind). Playback tops out around 720p on Linux
(a platform limitation, not a bug)."

prompt_yn "INSTALL_CHROMIUM" "Chromium" \
    "Chromium carries the latest patches for Wayland screen sharing. If you need to
run web apps like Slack or Microsoft Teams and want to share your screen,
Chromium is the way to go. It is also fully integrated with the theme switcher,
so it works great as a daily driver too."

prompt_yn "INSTALL_WEBAPPS" "WebApps (Linux Mint)" \
    "Running on ARM, you will quickly find that not every app is packaged for your
architecture. WebApps lets you pin sites like Slack or Teams as standalone
windows with their own icons — no browser tabs needed."

prompt_yn "INSTALL_FLATPAK" "Flatpak + Bazaar App Store" \
    "Flatpak lets you install sandboxed desktop apps from Flathub — useful when
native ARM packages are not available. Bazaar is a fast, modern app store for
browsing and managing Flatpak apps. This installs Flatpak, adds the Flathub
repository, and installs Bazaar as your graphical app store."

prompt_yn "INSTALL_CLAUDE" "Claude Code" \
    "Claude Code is an AI coding assistant that lives in your terminal. It can read
your project, write and edit code, run commands, and help you think through
problems — all from the command line. This setup includes Mod+C to launch
Claude and Mod+Shift+C for a quick-prompt popup via wofi."

echo ""
info "Selections saved. Starting installation..."
echo ""

# Prompt for sudo password once now, then keep it alive for the entire install
# so the user is never interrupted by a password prompt mid-run.
info "Requesting sudo access..."
sudo -v
while true; do sudo -n true; sleep 120; done 2>/dev/null &
SUDO_KEEPALIVE_PID=$!
trap 'kill $SUDO_KEEPALIVE_PID 2>/dev/null' EXIT

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
    seatd pipewire wireplumber libspa-0.2-bluetooth libspa-0.2-libcamera \
    network-manager \
    ukui-polkit \
    ddcutil i2c-tools \
    fish \
    bat eza fzf zoxide ugrep jq \
    grim slurp wl-clipboard wf-recorder libnotify-bin \
    xdg-desktop-portal-wlr \
    fonts-firacode fonts-noto-color-emoji fonts-noto-core \
    thunar mpv imv file-roller galculator zathura \
    bluez bluez-tools hwinfo neovim micro \
    papirus-icon-theme libglib2.0-bin gsettings-desktop-schemas \
    xdg-user-dirs \
    python3 \
    git curl build-essential pkg-config unzip

# Optional: Chromium
if [ "$INSTALL_CHROMIUM" = "y" ]; then
    info "Installing Chromium..."
    sudo apt install -y chromium
    success "Chromium installed"
fi

# Optional: Brave Origin (debloated Brave; same apt repo, arm64 supported)
if [ "$INSTALL_BRAVE" = "y" ]; then
    info "Installing Brave Origin..."
    # Regular Brave and Brave Origin share one apt repository. If an earlier
    # install already configured it (as a .list or a .sources file), reuse
    # that rather than add a second entry for the same repository.
    if ! compgen -G "/etc/apt/sources.list.d/brave-browser-release.*" > /dev/null; then
        sudo curl -fsSLo /usr/share/keyrings/brave-browser-archive-keyring.gpg \
            https://brave-browser-apt-release.s3.brave.com/brave-browser-archive-keyring.gpg
        echo "deb [signed-by=/usr/share/keyrings/brave-browser-archive-keyring.gpg] https://brave-browser-apt-release.s3.brave.com/ stable main" \
            | sudo tee /etc/apt/sources.list.d/brave-browser-release.list >/dev/null
    fi
    sudo apt update
    sudo apt install -y brave-origin
    success "Brave Origin installed"

    # Migrating from regular Brave: give Brave Origin a copy of the profile.
    # Only when Origin has no profile yet, and never touching the original.
    BRAVE_CFG="$HOME/.config/BraveSoftware"
    if [ -d "$BRAVE_CFG/Brave-Browser" ] && [ ! -e "$BRAVE_CFG/Brave-Origin" ]; then
        if pgrep -f "/opt/brave.com/" >/dev/null 2>&1; then
            warn "Brave is running, so its profile was not copied (a live profile copies inconsistently)."
            warn "To bring it across later: close Brave, then run"
            warn "  cp -a ~/.config/BraveSoftware/Brave-Browser ~/.config/BraveSoftware/Brave-Origin"
        else
            info "Copying your Brave profile to Brave Origin..."
            cp -a "$BRAVE_CFG/Brave-Browser" "$BRAVE_CFG/Brave-Origin"
            # Lock files name the old instance; a copy of them blocks startup.
            rm -f "$BRAVE_CFG/Brave-Origin"/Singleton*
            success "Brave profile copied (the original is untouched)"
        fi
    fi
fi

# Optional: WebApps
if [ "$INSTALL_WEBAPPS" = "y" ]; then
    info "Installing WebApps (webapp-manager)..."
    WEBAPPS_DEB="/tmp/webapp-manager.deb"
    # The Mint mirror is plain HTTP (no HTTPS endpoint) and this file is
    # installed as root, so pin its SHA-256 (as published in Mint's own package
    # index). Update the hash together with the version in the URL.
    WEBAPPS_SHA256="75f8c5d5edb0fc440880b801810b2a0231a772f901162e0c824c0bfa5727bcff"
    curl -fLo "$WEBAPPS_DEB" \
        "http://packages.linuxmint.com/pool/main/w/webapp-manager/webapp-manager_1.4.6_all.deb"
    if ! echo "$WEBAPPS_SHA256  $WEBAPPS_DEB" | sha256sum -c --status; then
        rm -f "$WEBAPPS_DEB"
        error "webapp-manager download does not match its expected checksum — not installing it."
        exit 1
    fi
    sudo apt install -y "$WEBAPPS_DEB"
    rm -f "$WEBAPPS_DEB"
    success "WebApps installed"
fi

# Optional: Flatpak + Bazaar
if [ "$INSTALL_FLATPAK" = "y" ]; then
    info "Installing Flatpak..."
    sudo apt install -y flatpak
    sudo flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo </dev/null >/dev/null 2>&1
    info "Installing Bazaar app store..."
    sudo flatpak install --noninteractive -y flathub io.github.kolunmi.Bazaar </dev/null >/dev/null 2>&1
    success "Flatpak + Bazaar installed"
fi

# Debian ships bat as batcat — create a symlink so scripts can use 'bat'
if [ -x /usr/bin/batcat ] && [ ! -e /usr/local/bin/bat ]; then
    sudo ln -s /usr/bin/batcat /usr/local/bin/bat
    success "Created bat symlink (batcat → bat)"
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
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
    success "Rust toolchain installed"
fi

# Ensure cargo is in PATH for this session
# shellcheck source=/dev/null
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
# and the stock lid monitor thread (replaced by argon-lid-monitor).
if [ -f /etc/argon/argononeupd.py ]; then
    if grep -q '^[[:space:]]*t1\.start()' /etc/argon/argononeupd.py; then
        info "Patching Argon daemon: disabling stock battery polling thread..."
        sudo sed -i 's/^[[:space:]]*t1\.start()/#&/' /etc/argon/argononeupd.py
        success "Battery polling thread disabled"
    else
        success "Battery polling thread already patched"
    fi

    if grep -q '^[[:space:]]*t2\.start()' /etc/argon/argononeupd.py; then
        info "Patching Argon daemon: disabling stock lid monitor thread..."
        sudo sed -i 's/^[[:space:]]*t2 = Thread(target = argonpowerbutton_monitorlid.*$/#&/' /etc/argon/argononeupd.py
        sudo sed -i 's/^[[:space:]]*t2\.start()/#&/' /etc/argon/argononeupd.py
        success "Lid monitor thread disabled"
    else
        success "Lid monitor thread already patched"
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

# Select the external Wi-Fi antenna.
# The ONE UP shell wires its antenna to the CM5 U.FL connector, but a CM5
# defaults to its onboard PCB antenna (ANT1) unless dtparam=ant2 is set.
# Argon's argononeup.sh defines set_external_antenna() for this but never
# calls it, so without this the laptop runs on the buried PCB antenna:
# weak signal, ~1/3 of association attempts time out (status_code=16), and
# far-away or guest networks are visible but never join. Takes effect on the
# reboot at the end of the install.
BOOT_CONFIG=/boot/firmware/config.txt
# (tr strips the NUL terminator: some grep builds treat the file as binary and miss)
if [ -f "$BOOT_CONFIG" ] && tr -d "\\0" < /proc/device-tree/model 2>/dev/null | grep -q "Compute Module 5"; then
    if grep -qE "^[[:space:]]*dtparam=ant2([[:space:]]|$)" "$BOOT_CONFIG"; then
        success "External Wi-Fi antenna already selected (dtparam=ant2)"
    else
        info "Selecting external Wi-Fi antenna (dtparam=ant2)..."
        # Repeat the [all] header so the line applies regardless of which
        # conditional section the file happens to end in.
        printf "\n[all]\n# Argon ONE UP: use the shell antenna on the CM5 U.FL connector\ndtparam=ant2\n" \
            | sudo tee -a "$BOOT_CONFIG" > /dev/null
        success "External Wi-Fi antenna selected (active after reboot)"
    fi
else
    warn "Not a CM5 or no $BOOT_CONFIG — skipping external antenna selection"
fi

# ---------------------------------------------------------------------------
# Phase 8: Clone repo and copy configs
# ---------------------------------------------------------------------------
phase "Phase 8: Desktop configuration"

if [ -d "$REPO_DIR" ]; then
    info "Removing stale clone..."
    rm -rf "$REPO_DIR"
fi
info "Cloning sway-argon-one-up ($REPO_BRANCH)..."
git clone --depth=1 --branch "$REPO_BRANCH" "$REPO_URL" "$REPO_DIR"

cd "$REPO_DIR"

# Re-running the installer (to upgrade) overwrites these configs. Keep a copy
# of whatever is there first so local edits are never lost without a trace.
CONFIG_ITEMS=(sway waybar wob wofi foot mako swaylock gtk-3.0 sway-themes fish wireplumber starship.toml mimeapps.list)
BACKUP_DIR="$HOME/.config/sway-argon-backup-$(date +%Y%m%d-%H%M%S)"
for item in "${CONFIG_ITEMS[@]}"; do
    if [ -e "$HOME/.config/$item" ]; then
        mkdir -p "$BACKUP_DIR"
        cp -a "$HOME/.config/$item" "$BACKUP_DIR/"
    fi
done
if [ -d "$BACKUP_DIR" ]; then
    info "Existing configs backed up to $BACKUP_DIR"
fi

info "Copying config files..."

# Sway and desktop configs
mkdir -p ~/.config
cp -r sway waybar wob wofi foot mako swaylock gtk-3.0 sway-themes fish wireplumber ~/.config/
cp starship.toml ~/.config/
cp mimeapps.list ~/.config/

# Staying on regular Brave: the shipped sway config launches Brave Origin on
# Mod+B, so point that binding at the browser that is actually installed.
if [ "$BRAVE_MODE" = "keep" ]; then
    sed -i 's|exec /usr/bin/brave-origin |exec /usr/bin/brave-browser |' ~/.config/sway/config
fi

# Wallpapers. Copy the directory's contents: `cp -r wallpapers ~/.wallpapers`
# nests them in ~/.wallpapers/wallpapers once the target exists (any re-run).
mkdir -p ~/.wallpapers
cp -r wallpapers/. ~/.wallpapers/

# The Sakurazuki wallpaper has no licence in its upstream repo, so it is not
# bundled here; fetch it from its author (pinned commit) instead. Without it the
# theme still applies and simply keeps the previous wallpaper.
if [ ! -f "$HOME/.wallpapers/sakurazuki.png" ]; then
    if curl -fsSL -o "$HOME/.wallpapers/sakurazuki.png" \
        "https://raw.githubusercontent.com/ahmed-z0/omarchy-sakurazuki-theme/488946b79e6ba9a0f93c84c8c9bfacd0add6f395/backgrounds/sakura.png"; then
        # It is 5744x2944 and 18 MB; shrink it when a converter is around.
        if command -v magick >/dev/null; then
            magick "$HOME/.wallpapers/sakurazuki.png" -resize 2560x "$HOME/.wallpapers/sakurazuki.png" || true
        fi
    else
        rm -f "$HOME/.wallpapers/sakurazuki.png"
        warn "Could not download the Sakurazuki wallpaper; the theme will keep the previous wallpaper"
    fi
fi

# Scripts
mkdir -p ~/.local/bin
cp bin/* ~/.local/bin/
chmod +x ~/.local/bin/*

# Optional: Widevine DRM for Brave (installs the CDM + seeds the Brave profile)
if [ "$INSTALL_WIDEVINE" = "y" ]; then
    if [ "$HAS_BRAVE" != "y" ]; then
        warn "Widevine was selected but Brave was not — skipping (Widevine needs Brave)."
    else
        info "Enabling Widevine DRM for Brave..."
        "$HOME/.local/bin/enable-widevine"
        success "Widevine DRM enabled for Brave"
    fi
fi

# Systemd user units that aren't tied to a specific Rust crate
mkdir -p "$HOME/.config/systemd/user"
if compgen -G "systemd/*.service" > /dev/null; then
    cp systemd/*.service "$HOME/.config/systemd/user/"
    systemctl --user daemon-reload 2>/dev/null || true
    # Keeps Bluetooth headsets in A2DP stereo. BlueZ intermittently fails its
    # a2dp-sink connect ("Device or resource busy"), leaving the device HFP-only
    # with no stereo profile enumerated at all — i.e. 16kHz mono with nothing for
    # WirePlumber policy to select. This watcher forces A2DP back on connect.
    systemctl --user enable bt-a2dp-guard.service 2>/dev/null || true
    if systemctl --user is-active --quiet graphical-session.target 2>/dev/null; then
        systemctl --user start bt-a2dp-guard.service 2>/dev/null || true
    fi
fi

# mpv config — Pi 5 needs OpenGL via wayland-egl and the fast profile;
# default Vulkan path crashes on fullscreen toggle.
mkdir -p "$HOME/.config/mpv"
cp mpv/mpv.conf "$HOME/.config/mpv/mpv.conf"

# GTK themes
mkdir -p ~/.themes
cp -r gtk-themes/* ~/.themes/

# Login screen
sudo cp greetd/config.toml greetd/sway-config greetd/gtkgreet.css greetd/wallpaper.png /etc/greetd/

# Session wrapper (gtkgreet -c) and the display guard used by greetd's ExecStartPre.
# Both live in /usr/local/bin, not ~/.local/bin: greetd needs them as root and the paths
# are baked into config files, so they must not depend on the username.
sudo install -m 0755 greetd/sway-session.sh /usr/local/bin/sway-session
sudo install -m 0755 greetd/greetd-wait-display.sh /usr/local/bin/greetd-wait-display

# Drop-in that stops a fast-failing greeter (lid closed at boot -> no output) from
# tripping greetd's start limit and killing the login screen for the whole boot.
sudo mkdir -p /etc/systemd/system/greetd.service.d
sudo install -m 0644 greetd/greetd-override.conf \
    /etc/systemd/system/greetd.service.d/override.conf
sudo systemctl daemon-reload

success "Config files copied"

# ---------------------------------------------------------------------------
# Phase 9: Build Rust daemons
# ---------------------------------------------------------------------------
phase "Phase 9: Build Rust daemons"

# Always build and install, even when a binary is already there: a re-run of
# the installer is the upgrade path, and it replaces the configs and scripts
# above, so skipping the binaries would leave new scripts driving old daemons.
build_and_install() {
    local crate="$1"
    info "Building $crate (this may take a few minutes)..."
    (cd "$REPO_DIR/$crate" && cargo build --release)
    sudo install -m 0755 "$REPO_DIR/$crate/target/release/$crate" "/usr/local/bin/$crate"
    success "$crate built and installed"
}

for crate in argon-battery-rs argon-lid-monitor argon-fan power-button-guard; do
    build_and_install "$crate"
done

# Root helper that sets the CPU governor (used by power-mode, lid-suspend and
# argon-battery-rs through sudo). Must be root-owned and outside $HOME: it is
# what the sudoers rule in Phase 10 allows.
sudo install -o root -g root -m 0755 "$REPO_DIR/sbin/set-cpu-governor" /usr/local/sbin/set-cpu-governor

# User must be in the gpio group to access /dev/gpiochip0 from argon-lid-monitor
if ! groups | grep -qw gpio; then
    info "Adding $USER to the gpio group (needed by argon-lid-monitor)..."
    sudo usermod -aG gpio "$USER"
    warn "gpio group membership takes effect after next login"
fi

# Install and enable the argon-lid-monitor user service
info "Installing argon-lid-monitor systemd user unit..."
mkdir -p "$HOME/.config/systemd/user"
cp "$REPO_DIR/argon-lid-monitor/systemd/argon-lid-monitor.service" "$HOME/.config/systemd/user/"
systemctl --user daemon-reload
systemctl --user enable argon-lid-monitor.service
# On an upgrade the old binary is still running; pick up the new one.
systemctl --user try-restart argon-lid-monitor.service 2>/dev/null || true
# Only start if sway session is up; otherwise it will start at next login
if systemctl --user is-active --quiet graphical-session.target 2>/dev/null; then
    systemctl --user start argon-lid-monitor.service
fi
success "argon-lid-monitor user service installed and enabled"

# trackpad-guard is retired: typing protection is libinput's own
# disable-while-typing now (set up just below). Earlier releases installed the
# daemon as a system service, and before that as a user unit and a Python
# script, so on an upgrade take out everything any of them put down. The crate
# stays in the repository (trackpad-guard/README.md) for anyone who wants to
# build it by hand; nothing here installs it.
info "Removing trackpad-guard (replaced by native disable-while-typing)..."
# Stop it first, while its helper script still exists: the unit's ExecStopPost
# removes the udev rule that hides the real touchpad from libinput.
sudo systemctl disable --now trackpad-guard.service 2>/dev/null || true
systemctl --user disable --now trackpad-guard.service 2>/dev/null || true
rm -f "$HOME/.config/systemd/user/trackpad-guard.service"
if [ -f "$HOME/.local/bin/trackpad-guard" ] && head -1 "$HOME/.local/bin/trackpad-guard" | grep -q python; then
    rm -f "$HOME/.local/bin/trackpad-guard"
fi
sudo rm -f \
    /etc/systemd/system/trackpad-guard.service \
    /usr/local/bin/trackpad-guard \
    /usr/local/bin/trackpad-guard-toggle \
    /usr/local/bin/trackpad-guard-tune \
    /etc/modules-load.d/trackpad-guard.conf \
    /etc/udev/rules.d/60-trackpad-guard-uinput.rules \
    /etc/udev/rules.d/60-trackpad-guard-amira.rules \
    /etc/udev/rules.d/60-trackpad-guard-amira-ignore.rules \
    /run/udev/rules.d/60-trackpad-guard-amira-ignore.rules
sudo rm -rf /usr/local/lib/trackpad-guard /etc/trackpad-guard
sudo systemctl daemon-reload
systemctl --user daemon-reload 2>/dev/null || true
sudo udevadm control --reload-rules
sudo udevadm trigger --action=change --subsystem-match=input
success "trackpad-guard removed"

# Native libinput disable-while-typing (squeeby's approach): a quirks file and
# a udev rule tag the AMIRA keyboard and touchpad as internal so libinput
# pairs them. On by default; the dashboard tile and Mod+Shift+G turn it off
# and on with `touchpad-mode toggle`. Takes effect at the next login.
info "Installing native touchpad disable-while-typing..."
sudo install -d -m 0755 /usr/local/share/touchpad-native
sudo install -m 0644 "$REPO_DIR/touchpad-native/local-overrides.quirks" \
    "$REPO_DIR/touchpad-native/99-argon-one-touchpad.rules" \
    /usr/local/share/touchpad-native/
sudo install -m 0755 "$REPO_DIR/touchpad-native/touchpad-mode" \
    /usr/local/share/touchpad-native/touchpad-mode
sudo ln -sf /usr/local/share/touchpad-native/touchpad-mode /usr/local/bin/touchpad-mode
if /usr/local/bin/touchpad-mode native; then
    success "Native disable-while-typing installed (toggle via dashboard or Mod+Shift+G)"
else
    warn "Could not set up native disable-while-typing — run 'touchpad-mode native' after install"
fi

# With argon-battery-rs owning battery polling + CW2217 self-heal, and
# argon-lid-monitor owning lid events, Argon's Python daemons have nothing
# left to do. Sway handles media/brightness/power keys natively, so the
# user daemon (argonkeyboard.py) is also redundant. Disable both.
info "Disabling Argon's Python daemons (replaced by Rust + sway)..."
sudo systemctl disable --now argononeupd.service 2>/dev/null || true
systemctl --user disable --now argononeupduser.service 2>/dev/null || true
success "Argon Python daemons disabled"

# ---------------------------------------------------------------------------
# Phase 9b: Install system-dashboard (prebuilt aarch64 from GitHub Releases)
# ---------------------------------------------------------------------------
phase "Phase 9b: Install system-dashboard"

# Runtime libraries for the Tauri webview. The -dev packages are NOT needed
# at install time because we download a prebuilt binary instead of compiling.
info "Installing webkit2gtk runtime..."
sudo apt install -y libwebkit2gtk-4.1-0 libayatana-appindicator3-1
success "Webkit runtime installed"

# Always fetch, so a re-run of the installer upgrades the dashboard too.
info "Downloading prebuilt system-dashboard for aarch64..."
# SD_TAG can be set in the environment to install a specific release
# (e.g. a release-candidate build); otherwise take the newest one.
SD_TAG="${SD_TAG:-}"
[ -n "$SD_TAG" ] || SD_TAG=$(curl -fsSL "https://api.github.com/repos/jasonwitty/sway-argon-one-up/releases" 2>/dev/null \
    | jq -r '[.[] | select(.tag_name | startswith("rc-system-dashboard-v"))][0].tag_name // empty' || true)  # `|| true`: a failed lookup must reach the error below, not trip `set -e`. RC-ONLY: revert the prefix to "system-dashboard-v" before merging to main
if [ -z "$SD_TAG" ]; then
    error "Could not find a published system-dashboard release on GitHub."
    error "Check https://github.com/jasonwitty/sway-argon-one-up/releases or tag a release."
    exit 1
fi
info "Found release: $SD_TAG"
if ! curl -fsSL -o /tmp/system-dashboard \
    "https://github.com/jasonwitty/sway-argon-one-up/releases/download/${SD_TAG}/system-dashboard-aarch64"; then
    error "Failed to download system-dashboard binary from GitHub Releases."
    error "Check internet connectivity and try again."
    exit 1
fi
sudo install -m 0755 /tmp/system-dashboard /usr/local/bin/system-dashboard
rm -f /tmp/system-dashboard
success "system-dashboard installed"

# .desktop file so any DE / launcher can list and start it
info "Installing system-dashboard .desktop entry..."
sudo mkdir -p /usr/local/share/applications
sudo install -m 0644 "$REPO_DIR/system-dashboard/system-dashboard.desktop" \
    /usr/local/share/applications/system-dashboard.desktop
success ".desktop entry installed"

# ---------------------------------------------------------------------------
# Phase 9c: Network + Bluetooth panels (Quickshell ports of the Omarchy ones)
# ---------------------------------------------------------------------------
phase "Phase 9c: Network, Bluetooth and calendar panels"

# Qt runtime the panels need, plus the tools the network helper scripts call:
# iw (link details, band), ping (latency), qrencode (Wi-Fi share card) and
# uuidgen (profile ids for WPA-Enterprise networks).
info "Installing panel runtime..."
sudo apt install -y \
    libqt6svg6 qt6-wayland libjemalloc2 \
    qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts \
    qml6-module-qtquick-templates qml6-module-qtquick-window qml6-module-qtqml-workerscript \
    iw iputils-ping qrencode uuid-runtime
success "Panel runtime installed"

mkdir -p "$HOME/.config/quickshell"
rm -rf "$HOME/.config/quickshell/panels" "$HOME/.config/quickshell/network-panel"
cp -r "$REPO_DIR/quickshell-panels/config" "$HOME/.config/quickshell/panels"
# Each panel is its own folder in the repo; Quickshell loads them as the
# qs.network, qs.bluetooth and qs.clock modules next to the shared Commons/ and Ui/.
mkdir -p "$HOME/.config/quickshell/panels/network" "$HOME/.config/quickshell/panels/bluetooth" "$HOME/.config/quickshell/panels/clock"
cp "$REPO_DIR"/network-panel/*.qml "$REPO_DIR"/network-panel/*.js "$HOME/.config/quickshell/panels/network/"
cp "$REPO_DIR"/bluetooth-panel/*.qml "$REPO_DIR"/bluetooth-panel/*.js "$HOME/.config/quickshell/panels/bluetooth/"
cp "$REPO_DIR"/clock-panel/*.qml "$REPO_DIR"/clock-panel/*.js "$HOME/.config/quickshell/panels/clock/"

# Quickshell is not packaged for trixie and links against private Qt APIs, so
# a binary only works with the Qt it was built for. Prefer a prebuilt release
# when one is published and actually starts on this system's Qt; otherwise
# build it here (a few minutes on a Pi 5).
quickshell_works() {
    "$1" --version >/dev/null 2>&1
}

if [ -x /usr/local/bin/quickshell ] && quickshell_works /usr/local/bin/quickshell; then
    success "quickshell already installed"
else
    QS_TAG=$(curl -fsSL "https://api.github.com/repos/jasonwitty/sway-argon-one-up/releases" 2>/dev/null \
        | jq -r '[.[] | select(.tag_name | startswith("quickshell-v"))][0].tag_name // empty' || true)  # `|| true`: on a failed lookup fall through to the source build
    if [ -n "$QS_TAG" ] && curl -fsSL -o /tmp/quickshell \
        "https://github.com/jasonwitty/sway-argon-one-up/releases/download/${QS_TAG}/quickshell-aarch64" \
        && chmod +x /tmp/quickshell && quickshell_works /tmp/quickshell; then
        info "Using prebuilt quickshell from release $QS_TAG"
        sudo install -m 0755 /tmp/quickshell /usr/local/bin/quickshell
    else
        info "No usable prebuilt quickshell; building from source..."
        "$REPO_DIR/quickshell-panels/quickshell-build/build-quickshell.sh" --install-deps --prefix /usr/local
    fi
    rm -f /tmp/quickshell
    if quickshell_works /usr/local/bin/quickshell; then
        success "quickshell installed"
    else
        # Not fatal: waybar's right-click still opens nmtui.
        warn "quickshell did not install; the network panel will not open (right-click the waybar network icon for nmtui)"
    fi
fi

# ---------------------------------------------------------------------------
# Phase 10: System configuration
# ---------------------------------------------------------------------------
phase "Phase 10: System configuration"

# Enable services
info "Enabling services..."
sudo systemctl enable --now seatd
sudo systemctl enable greetd
sudo systemctl set-default graphical.target
sudo systemctl mask --now power-profiles-daemon 2>/dev/null || true
success "Services configured"

# User groups
info "Adding user to required groups..."
# 'pipewire' matters more than it looks: /etc/security/limits.d/25-pw-rlimits.conf
# grants rtprio 95 / memlock / nice to @pipewire ONLY. Without it PipeWire gets no
# realtime scheduling at all (every thread SCHED_OTHER), and on a loaded Pi at
# 1.5GHz that means audible clipping and dropouts — diagnosed 2026-08-03 on
# Bluetooth audio. Being in 'audio' is NOT sufficient on Debian.
sudo usermod -aG video,audio,input,render,i2c,pipewire "$USER"
success "User groups updated"

# Create standard XDG user directories (Desktop, Documents, Downloads, etc.)
info "Creating user directories..."
xdg-user-dirs-update
success "User directories created"

# Sudoers for lid-suspend, CPU governor, USB bind/unbind.
# Every rule names an exact command line. Do not add one with a wildcard in
# its arguments: sudoers lets `*` match spaces and slashes, so a rule such as
# `tee /sys/.../cpu*/...` allows writing any file as root.
info "Configuring sudoers for power management..."
sudo tee /etc/sudoers.d/lid-power > /dev/null <<SUDOEOF
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock wifi
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill block bluetooth
$USER ALL=(ALL) NOPASSWD: /usr/sbin/rfkill unblock bluetooth
$USER ALL=(root) NOPASSWD: /usr/local/sbin/set-cpu-governor
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/unbind
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /sys/bus/usb/drivers/usb/bind
SUDOEOF
sudo visudo -cf /etc/sudoers.d/lid-power
success "Power management sudoers configured"

# Browser theme sudoers (only if Brave or Chromium selected)
if [ "$HAS_BRAVE" = "y" ] || [ "$INSTALL_CHROMIUM" = "y" ]; then
    info "Configuring sudoers for browser theme integration..."
    sudo mkdir -p /etc/brave/policies/managed /etc/chromium/policies/managed
    sudo tee /etc/sudoers.d/browser-theme > /dev/null <<SUDOEOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/brave/policies/managed/color.json
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/chromium/policies/managed/color.json
SUDOEOF
    sudo visudo -cf /etc/sudoers.d/browser-theme
    success "Browser theme sudoers configured"
fi

# argon-fan sudoers + systemd unit
info "Configuring argon-fan service..."
sudo tee /etc/sudoers.d/argon-fan > /dev/null <<SUDOEOF
$USER ALL=(ALL) NOPASSWD: /usr/bin/tee /etc/argon-fan/config.json
SUDOEOF
sudo visudo -cf /etc/sudoers.d/argon-fan
sudo tee /etc/systemd/system/argon-fan.service > /dev/null <<'EOF'
[Unit]
Description=Argon ONE UP fan control daemon
After=multi-user.target

[Service]
Type=simple
ExecStart=/usr/local/bin/argon-fan daemon
Restart=on-failure
RestartSec=2
KillMode=process
TimeoutStopSec=5

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable argon-fan
# restart, not `enable --now`: on an upgrade the old binary is still running.
sudo systemctl restart argon-fan
success "argon-fan service installed and started"

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

# Hand the power key to power-button-guard instead of powering off on a bare tap.
# Shipped as a file rather than a heredoc so the comments explaining why
# HandlePowerKeyLongPress stays as a backstop travel with it.
info "Configuring logind power key override..."
sudo install -m 0644 "$REPO_DIR/power-button-guard/logind.conf.d/10-power-button-guard.conf" \
    /etc/systemd/logind.conf.d/10-power-button-guard.conf
mkdir -p "$HOME/.config/systemd/user"
cp "$REPO_DIR/power-button-guard/systemd/power-button-guard.service" \
    "$HOME/.config/systemd/user/"
systemctl --user daemon-reload 2>/dev/null || true
systemctl --user enable power-button-guard.service 2>/dev/null || true
systemctl --user try-restart power-button-guard.service 2>/dev/null || true
success "Power button confirm prompt installed"

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
        info "Installing Claude Code..."
        curl -fsSL https://claude.ai/install.sh | bash
        success "Claude Code installed"
        echo ""
        info "Run 'claude' after rebooting to authenticate."
    fi
fi

# ---------------------------------------------------------------------------
# Phase 12: Cleanup and finish
# ---------------------------------------------------------------------------
phase "Phase 12: Set default theme"

# Keep the theme already in use on a re-run; Catppuccin Frappe on a new install.
THEME="frappe"
if [ -f "$HOME/.config/sway-themes/current" ]; then
    SAVED_THEME=$(cat "$HOME/.config/sway-themes/current")
    if [ -n "$SAVED_THEME" ] && [ -f "$HOME/.config/sway-themes/$SAVED_THEME" ]; then
        THEME="$SAVED_THEME"
    fi
fi
info "Applying theme ($THEME)..."
echo "$THEME" > "$HOME/.config/sway-themes/current"
"$HOME/.local/bin/switch-theme" "$THEME" &>/dev/null || true
success "Theme applied"

# ---------------------------------------------------------------------------
# Phase 13: Cleanup and finish
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
echo "  Mod+N         File manager"
echo "  Mod+B         Brave (Origin)"
echo "  Mod+C         Claude Code"
echo "  Mod+=         Calculator"
echo "  Mod+Shift+H   Keybinding help"
echo "  Mod+Shift+R   Screen record"
echo ""
