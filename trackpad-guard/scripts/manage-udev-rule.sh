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
# when the service stops, then USB-rebinds every AMIRA interface so
# libinput re-evaluates each device with the current ruleset. The
# rebind is the reliable trigger; udevadm trigger alone often doesn't
# cause libinput to detach/re-attach an already-open device.
#
# VOLATILE LOCATION (/run, not /etc): the removal half of this lifecycle
# only runs on a *clean* stop (systemd ExecStopPost). A power loss, OOM
# kill, or kernel panic takes the whole system down without running
# ExecStopPost, so the rule would survive into the next boot as an
# orphan — libinput keeps ignoring the real touchpad, and if the daemon
# then fails to start (e.g. a dropped enable symlink after the dirty
# boot) there is NO pointer at all. Catastrophic and hard to recover
# without a TTY. Installing into /run/udev/rules.d (tmpfs, wiped every
# boot) makes that impossible: any orphaned rule vanishes on reboot, so
# the worst case degrades to "real touchpad works, our DWT layer is off"
# instead of "no touchpad". udev reads /run/udev/rules.d natively.
#
# History: an earlier version only rebound product 8061 (the touchpad-
# carrying interface). The 8060 product also exposes a "Mouse" interface
# that matches our IGNORE rule by name. Without rebinding 8060, its
# evdev node enumerated at boot — before the rule existed — and
# libinput kept treating it as a live pointer indefinitely, competing
# with our virtual touchpad. Now we rebind every AMIRA interface
# (vendor 6080) so the rule applies uniformly.

set -eu

RULE_SRC=/usr/local/lib/trackpad-guard/60-trackpad-guard-amira-ignore.rules
RULE_DIR=/run/udev/rules.d
RULE_DST=$RULE_DIR/60-trackpad-guard-amira-ignore.rules
# Legacy persistent location (pre-2026-06-24). Always cleaned on remove
# so an upgrade doesn't leave a stale persistent copy shadowing us — a
# file of the same name in /etc has higher priority than /run.
RULE_LEGACY=/etc/udev/rules.d/60-trackpad-guard-amira-ignore.rules
AMIRA_VENDOR=6080

action=${1:-}
case "$action" in
    install)
        rm -f "$RULE_LEGACY"
        mkdir -p "$RULE_DIR"
        install -m 0644 "$RULE_SRC" "$RULE_DST"
        ;;
    remove)
        rm -f "$RULE_DST" "$RULE_LEGACY"
        ;;
    *)
        echo "usage: $0 install|remove" >&2
        exit 1
        ;;
esac

udevadm control --reload-rules

# Walk all top-level USB devices and rebind every AMIRA interface so
# libinput sees a fresh add event under the updated rules. We don't
# error out if a device isn't currently on the bus — it'll attach with
# the right rules whenever it next appears.
for entry in /sys/bus/usb/devices/*/; do
    name=$(basename "$entry")
    # Skip non-numeric entries like usb1, 1-0:1.0 (only top-level USB
    # devices have idVendor/idProduct files).
    vendor=$(cat "$entry/idVendor" 2>/dev/null) || continue
    if [ "$vendor" = "$AMIRA_VENDOR" ]; then
        echo "$name" > /sys/bus/usb/drivers/usb/unbind 2>/dev/null || true
        sleep 0.3
        echo "$name" > /sys/bus/usb/drivers/usb/bind 2>/dev/null || true
    fi
done
