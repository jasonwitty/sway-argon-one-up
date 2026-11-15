#!/bin/bash
# Build a trimmed Quickshell (https://quickshell.org) for the network and Bluetooth panels.
#
#   build-quickshell.sh [--install-deps] [--prefix DIR] [--ref GIT_REF]
#
# Quickshell is not packaged for Debian trixie / Raspberry Pi OS 13, and it
# links against private Qt APIs, so the binary has to be built against the
# exact Qt the machine runs. This script is the one place the build recipe
# lives: install.sh runs it natively, and the Dockerfile next to it runs it in
# a clean trixie container to produce a release binary.
#
# Only what the panels need is compiled in: Wayland layer-shell, unix sockets
# (IPC), the NetworkManager module and the BlueZ module. That keeps the dependency list short
# and the build to a few minutes on a Pi 5.

set -euo pipefail

REF="v0.3.2"
PREFIX="/usr/local"
INSTALL_DEPS=false

while (($# > 0)); do
    case "$1" in
        --install-deps) INSTALL_DEPS=true ;;
        --prefix) PREFIX="$2"; shift ;;
        --ref) REF="$2"; shift ;;
        *) echo "Usage: build-quickshell.sh [--install-deps] [--prefix DIR] [--ref GIT_REF]" >&2; exit 2 ;;
    esac
    shift
done

SUDO=""
(( EUID == 0 )) || SUDO="sudo"

BUILD_DEPS=(
    ca-certificates git cmake ninja-build pkg-config g++
    qt6-base-dev qt6-base-private-dev
    qt6-declarative-dev qt6-declarative-private-dev
    qt6-wayland-dev qt6-wayland-private-dev
    qt6-shadertools-dev spirv-tools
    libcli11-dev libdrm-dev libjemalloc-dev
    libwayland-dev wayland-protocols
)

if [[ $INSTALL_DEPS == true ]]; then
    $SUDO apt-get update
    DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y --no-install-recommends "${BUILD_DEPS[@]}"
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# Quickshell generates bindings for protocols newer than Debian trixie's
# wayland-protocols 1.44 (ext-background-effect arrived in 1.45). The package
# is nothing but XML, so when the system copy is too old, point pkg-config at
# a newer checkout for this build only rather than touching /usr/share.
WP_NEEDED="1.45"
if ! pkg-config --atleast-version="$WP_NEEDED" wayland-protocols; then
    echo "wayland-protocols $(pkg-config --modversion wayland-protocols 2>/dev/null || echo missing) is older than $WP_NEEDED; using a newer copy for this build"
    git clone --quiet --depth 1 --branch 1.47 \
        https://gitlab.freedesktop.org/wayland/wayland-protocols.git "$WORK/wayland-protocols"
    mkdir -p "$WORK/pkgconfig"
    cat > "$WORK/pkgconfig/wayland-protocols.pc" <<EOF
pkgdatadir=$WORK/wayland-protocols

Name: Wayland Protocols
Description: Wayland protocol files
Version: 1.47
EOF
    export PKG_CONFIG_PATH="$WORK/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
fi

git clone --quiet --depth 1 --branch "$REF" \
    https://github.com/quickshell-mirror/quickshell.git "$WORK/quickshell"

cd "$WORK/quickshell"
cmake -GNinja -B build \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="$PREFIX" \
    -DDISTRIBUTOR="sway-argon-one-up (panels)" \
    -DCRASH_HANDLER=OFF \
    -DX11=OFF \
    -DHYPRLAND=OFF \
    -DSCREENCOPY=OFF \
    -DWAYLAND_SESSION_LOCK=OFF \
    -DWAYLAND_TOPLEVEL_MANAGEMENT=OFF \
    -DSERVICE_STATUS_NOTIFIER=OFF \
    -DSERVICE_PIPEWIRE=OFF \
    -DSERVICE_MPRIS=OFF \
    -DSERVICE_PAM=OFF \
    -DSERVICE_POLKIT=OFF \
    -DSERVICE_GREETD=OFF \
    -DSERVICE_UPOWER=OFF \
    -DSERVICE_NOTIFICATIONS=OFF
cmake --build build

# Only the binary is needed; the QML modules are linked into it.
strip build/src/quickshell
$SUDO install -D -m 0755 build/src/quickshell "$PREFIX/bin/quickshell"
echo "Installed $PREFIX/bin/quickshell ($REF)"
