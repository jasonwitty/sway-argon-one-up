#!/bin/sh
# greetd-wait-display — ExecStartPre guard for greetd.service
#
# Booting with the lapdock lid closed leaves the vc4 HDMI connector disconnected
# ("vc4-drm axi:gpu: [drm] Cannot find any crtc or sizes"). The greeter's sway then
# comes up with zero outputs, gtkgreet cannot map its layer-shell surface and exits,
# and greetd logs "check_children: greeter exited without creating a session".
#
# Packaged greetd.service has RestartSec=1 / StartLimitBurst=5 / StartLimitInterval=30,
# so five of those in ~11s trip 'start-limit-hit' and greetd stays dead for the whole
# boot — the machine sits with no login prompt until you reboot it. Worse, fbcon has no
# mode either, so the stale greeter stderr only paints when the lid is finally opened,
# which makes a week-old failure look like a fresh crash.
#
# So: don't let greetd spawn a greeter until some connector is actually connected.
# Always exits 0 — the caller (Restart=always with no start limit) simply tries again.

timeout=${GREETD_WAIT_TIMEOUT:-60}
waited=0

connector_connected() {
    for status in /sys/class/drm/card*-*/status; do
        [ -e "$status" ] || continue
        [ "$(cat "$status")" = "connected" ] && return 0
    done
    return 1
}

connector_connected && exit 0

echo "greetd-wait-display: no connected DRM connector, waiting up to ${timeout}s (lid closed?)"

while [ "$waited" -lt "$timeout" ]; do
    sleep 1
    waited=$((waited + 1))
    if connector_connected; then
        echo "greetd-wait-display: connector came up after ${waited}s"
        exit 0
    fi
done

echo "greetd-wait-display: still no connector after ${timeout}s, starting greeter anyway"
exit 0
