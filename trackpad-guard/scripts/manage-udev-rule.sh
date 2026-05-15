#!/bin/sh
# Service-managed udev rule for trackpad-guard.
#
# When trackpad-guard.service is running, libinput must ignore the real
# AMIRA touchpad evdev nodes — otherwise libinput and the daemon would
# fight over the device. We do this with a udev rule. But that same rule
# stuck around while the service was *disabled*, which left the touchpad
# unusable: libinput still skipped the real device and there was no
# virtual replacement.
#
# This script installs the rule when the service starts and removes it
# when the service stops, then USB-rebinds the AMIRA touchpad interface
# so libinput re-evaluates the device with the current ruleset. The
# rebind is the reliable trigger; udevadm trigger alone often doesn't
# cause libinput to detach/re-attach an already-open device.

set -eu

RULE_SRC=/usr/local/lib/trackpad-guard/60-trackpad-guard-amira-ignore.rules
RULE_DST=/etc/udev/rules.d/60-trackpad-guard-amira-ignore.rules
TOUCHPAD_VENDOR=6080
TOUCHPAD_PRODUCT=8061

action=${1:-}
case "$action" in
    install)
        install -m 0644 "$RULE_SRC" "$RULE_DST"
        ;;
    remove)
        rm -f "$RULE_DST"
        ;;
    *)
        echo "usage: $0 install|remove" >&2
        exit 1
        ;;
esac

udevadm control --reload-rules

# Find the AMIRA touchpad's USB bus path (e.g. "1-1.6") and rebind it so
# libinput sees a fresh add event under the updated rules. We don't
# error out if the device isn't currently on the bus — it'll attach with
# the right rules whenever it next appears.
for entry in /sys/bus/usb/devices/*/; do
    name=$(basename "$entry")
    # Skip non-numeric entries like usb1, 1-0:1.0 (only top-level USB
    # devices have idVendor/idProduct files).
    vendor=$(cat "$entry/idVendor" 2>/dev/null) || continue
    product=$(cat "$entry/idProduct" 2>/dev/null) || continue
    if [ "$vendor" = "$TOUCHPAD_VENDOR" ] && [ "$product" = "$TOUCHPAD_PRODUCT" ]; then
        echo "$name" > /sys/bus/usb/drivers/usb/unbind 2>/dev/null || true
        sleep 0.3
        echo "$name" > /sys/bus/usb/drivers/usb/bind 2>/dev/null || true
        break
    fi
done
