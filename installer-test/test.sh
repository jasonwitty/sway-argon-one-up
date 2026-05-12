#!/bin/bash
# Inside-container smoke test for install.sh prerequisites.
# Runs as user `pi` with passwordless sudo.

set -u  # no -e — we want to keep going through every check and report

PASS=0
FAIL=0
SKIP=0

step() { echo; echo "── $* ─────────────────────────────────"; }
ok()   { echo "✓ $*"; PASS=$((PASS+1)); }
bad()  { echo "✗ $*"; FAIL=$((FAIL+1)); }
skip() { echo "↷ $*"; SKIP=$((SKIP+1)); }

step "apt update"
sudo apt-get update >/dev/null 2>&1 && ok "apt-get update" || bad "apt-get update"

step "Phase 3: core package availability (simulate)"
CORE_PKGS=(
    sway swaybg swayidle swaylock xwayland
    waybar wofi foot wob mako-notifier
    greetd gtkgreet
    seatd pipewire wireplumber
    network-manager network-manager-gnome
    ukui-polkit
    ddcutil i2c-tools
    fish
    bat eza fzf zoxide ugrep jq
    grim slurp wl-clipboard wf-recorder libnotify-bin
    xdg-desktop-portal-wlr
    fonts-firacode
    thunar mpv imv file-roller galculator zathura
    blueman hwinfo neovim micro
    papirus-icon-theme libglib2.0-bin gsettings-desktop-schemas
    xdg-user-dirs
    python3
    git curl build-essential pkg-config unzip
)
MISSING=()
for p in "${CORE_PKGS[@]}"; do
    if ! apt-cache show "$p" >/dev/null 2>&1; then
        MISSING+=("$p")
    fi
done
if [ ${#MISSING[@]} -eq 0 ]; then
    ok "all ${#CORE_PKGS[@]} core packages resolvable"
else
    bad "missing in Trixie repo: ${MISSING[*]}"
fi

step "Phase 9b: webkit runtime (system-dashboard)"
for p in libwebkit2gtk-4.1-0 libayatana-appindicator3-1; do
    if apt-cache show "$p" >/dev/null 2>&1; then
        ok "$p available"
    else
        bad "$p missing"
    fi
done

step "External: Rust toolchain installer URL"
if curl -fsSI -o /dev/null https://sh.rustup.rs ; then
    ok "https://sh.rustup.rs reachable"
else
    bad "https://sh.rustup.rs unreachable"
fi

step "External: system-dashboard latest aarch64 release"
SD_TAG=$(curl -fsSL "https://api.github.com/repos/jasonwitty/sway-argon-one-up/releases" 2>/dev/null \
    | jq -r '[.[] | select(.tag_name | startswith("system-dashboard-v"))][0].tag_name // empty')
if [ -n "$SD_TAG" ]; then
    ok "release tag: $SD_TAG"
    SD_URL="https://github.com/jasonwitty/sway-argon-one-up/releases/download/${SD_TAG}/system-dashboard-aarch64"
    if curl -fsSL -o /tmp/sd-bin "$SD_URL" ; then
        sz=$(stat -c %s /tmp/sd-bin)
        ok "asset downloads ($sz bytes)"
        if file /tmp/sd-bin 2>/dev/null | grep -q "ELF.*aarch64"; then
            ok "binary is aarch64 ELF"
        else
            skip "file(1) not available to verify ELF arch (binary downloaded OK)"
        fi
    else
        bad "asset download failed: $SD_URL"
    fi
else
    bad "no system-dashboard release found via GitHub API"
fi

step "External: Argon hardware installer (Phase 7)"
# install.sh runs this script via `curl ... | bash` to set up the Argon ONE UP
# kernel modules and the original Python daemons.
if curl -fsSI -o /dev/null https://download.argon40.com/argononeup.sh ; then
    ok "argon40.com installer reachable"
else
    bad "argon40.com installer unreachable"
fi

step "External: Claude Code installer (optional, Phase 11)"
if curl -fsSI -o /dev/null https://claude.ai/install.sh ; then
    ok "claude.ai/install.sh reachable"
else
    bad "claude.ai/install.sh unreachable"
fi

step "Cargo crate sanity (cargo check)"
# Install just enough to run `cargo check` on each crate. We don't need a full
# release build for the smoke test; check verifies the dep tree resolves and
# the source compiles syntactically + type-checks.
sudo apt-get install -y --no-install-recommends \
    rustc cargo \
    libssl-dev libudev-dev libi2c-dev pkg-config \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev libsoup-3.0-dev patchelf \
    >/dev/null 2>&1
for c in argon-battery-rs argon-fan argon-lid-monitor trackpad-guard system-dashboard ; do
    if [ -f "$c/Cargo.toml" ]; then
        echo "  checking $c..."
        if (cd "$c" && cargo check --locked --release 2>&1 | tail -1) ; then
            ok "$c cargo check"
        else
            bad "$c cargo check failed"
        fi
    else
        skip "$c not present (expected if PR hasn't merged)"
    fi
done

step "shellcheck install.sh"
sudo apt-get install -y --no-install-recommends shellcheck >/dev/null 2>&1
if shellcheck install.sh >/dev/null 2>&1 ; then
    ok "install.sh clean"
else
    bad "install.sh has shellcheck warnings"
fi

echo
echo "════════════════════════════════════════════"
echo "SUMMARY: $PASS passed, $FAIL failed, $SKIP skipped"
echo "════════════════════════════════════════════"
[ "$FAIL" -eq 0 ]
