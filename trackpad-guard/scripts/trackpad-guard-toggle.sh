#!/usr/bin/env bash
# Toggle trackpad-guard.service (system unit), then notify the user.
#
# Used by both the system-dashboard's trackpad tile and the sway
# Mod+Shift+G keybinding so the notification text is consistent across
# entry points. The sudoers carve-out for enable/disable --now is
# installed by install.sh.
set -euo pipefail

UNIT="trackpad-guard.service"
HOTKEY="Mod+Shift+G"

notify() {
    # notify-send may be absent on a minimal install; never fail the toggle
    # because the user couldn't be told about it.
    command -v notify-send >/dev/null 2>&1 || return 0
    notify-send -a "trackpad-guard" -i input-touchpad "$@" || true
}

if systemctl is-active --quiet "$UNIT"; then
    sudo -n systemctl disable --now "$UNIT" >/dev/null
    notify "Trackpad guard disabled" "Touchpad always-on (no disable-while-typing)"
else
    sudo -n systemctl enable --now "$UNIT" >/dev/null
    notify "Trackpad guard enabled (beta)" "Toggle: $HOTKEY  ·  USB rescue: Mod+Shift+T"
fi
