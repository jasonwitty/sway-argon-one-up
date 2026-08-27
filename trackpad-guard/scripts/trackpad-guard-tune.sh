#!/usr/bin/env bash
# Read or set trackpad-guard's runtime tunables, then notify the user.
#
#   trackpad-guard-tune              # print the effective typing gate, in ms
#   trackpad-guard-tune 220          # set it to 220ms
#   trackpad-guard-tune 0            # disable the gate (control case for A/B tests)
#   trackpad-guard-tune --taps       # print whether taps are gated
#   trackpad-guard-tune --taps off   # let mid-typing taps through (old behavior)
#
# Used by both the system-dashboard trackpad tile and by hand in a terminal,
# so the value that lands is identical either way. No restart needed: the
# daemon stat()s this file every second and re-reads it when the mtime moves,
# so a new value is live within ~1s and can be compared by feel against the
# previous one in the same typing session.
#
# Every write emits the whole file including BOTH keys, read back from whatever
# is currently set. That matters because the dashboard slider only ever passes a
# millisecond value — if this script wrote just that key, moving the slider
# would silently reset gate_taps to its default.
#
# The write goes through `sudo -n /usr/bin/tee` because the config is root
# owned (the daemon is a system service) while every caller here is not.
# install.sh drops a NOPASSWD rule for this exact invocation — same approach
# argon-fan uses for /etc/argon-fan/config.json.
set -euo pipefail

CONFIG="/etc/trackpad-guard/config"
# Keep in sync with TYPING_GATE_MAX_MS in trackpad-guard/src/main.rs. The
# daemon clamps too — this check exists to tell the user, not to protect it.
MAX_MS=500
DEFAULT_MS=200
DEFAULT_TAPS=true

notify() {
    # notify-send may be absent on a minimal install; never fail the write
    # because the user couldn't be told about it.
    command -v notify-send >/dev/null 2>&1 || return 0
    notify-send -a "trackpad-guard" -i input-touchpad "$@" || true
}

# Last assignment wins, matching parse_config_value() in the daemon.
config_value() {
    local key="$1"
    [ -r "$CONFIG" ] || return 1
    sed -n "s/^[[:space:]]*${key}[[:space:]]*=[[:space:]]*\([^#[:space:]]*\).*/\1/p" \
        "$CONFIG" | tail -1 | grep . || return 1
}

current_ms() { config_value typing_gate_ms || printf '%s\n' "$DEFAULT_MS"; }
current_taps() { config_value gate_taps || printf '%s\n' "$DEFAULT_TAPS"; }

write_config() {
    local ms="$1" taps="$2"
    local body
    body="# trackpad-guard runtime tunables. Written by trackpad-guard-tune; the
# daemon re-reads this file within ~1s of any change (no restart needed).
#
# typing_gate_ms — touchpad events arriving within this many milliseconds of a
#   keystroke are dropped (palm protection while typing). 0 disables the gate.
#   Compiled default is ${DEFAULT_MS}ms; accepted range is 0-${MAX_MS}ms.
#
# gate_taps — whether the gate also hides a finger that LANDS while you are
#   typing. Taps are synthesized by libinput from a touch-down/touch-up pair,
#   so with this off a palm brushing the pad mid-sentence becomes a real click
#   at wherever the pointer happened to be. Off restores the pre-2026-08-20
#   behavior; only useful for A/B testing.
typing_gate_ms=${ms}
gate_taps=${taps}"

    # Absolute paths and literal args: sudoers matches the command line as
    # written, so these two invocations are exactly what install.sh whitelists.
    sudo -n /usr/bin/mkdir -p /etc/trackpad-guard
    printf '%s\n' "$body" | sudo -n /usr/bin/tee /etc/trackpad-guard/config >/dev/null
}

case "${1:-}" in
"")
    current_ms
    exit 0
    ;;
-h | --help | help)
    sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
--taps)
    if [ $# -eq 1 ]; then
        current_taps
        exit 0
    fi
    case "$2" in
    on | true | 1 | yes) taps=true ;;
    off | false | 0 | no) taps=false ;;
    *)
        echo "trackpad-guard-tune: --taps wants on or off, got '$2'" >&2
        exit 2
        ;;
    esac
    write_config "$(current_ms)" "$taps"
    if [ "$taps" = true ]; then
        notify "Tap gating on" "A finger landing while you type is hidden from libinput"
    else
        notify "Tap gating off" "Mid-typing taps can click again (old behavior)"
    fi
    printf '%s\n' "$taps"
    exit 0
    ;;
esac

ms="$1"
if ! [[ "$ms" =~ ^[0-9]+$ ]]; then
    echo "trackpad-guard-tune: not a whole number of milliseconds: '$ms'" >&2
    exit 2
fi
if [ "$ms" -gt "$MAX_MS" ]; then
    echo "trackpad-guard-tune: $ms ms is above the $MAX_MS ms ceiling" >&2
    exit 2
fi

write_config "$ms" "$(current_taps)"

if [ "$ms" -eq 0 ]; then
    notify "Typing gate off" "Touchpad no longer gated while typing (0 ms)"
else
    notify "Typing gate ${ms} ms" "Live within ~1s — no restart needed"
fi
printf '%s\n' "$ms"
