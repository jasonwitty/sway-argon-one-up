#!/bin/sh
# sway-session — what gtkgreet launches on login (greetd/sway-config: gtkgreet -c)
#
# greetd runs the session command directly, so /usr/share/wayland-sessions/sway.desktop
# never applies and two things it provides go missing when the command is bare `sway`:
#
#   XDG_CURRENT_DESKTOP=sway  sway.desktop supplies this via DesktopNames=sway. Without
#                             it, sway's own environment has no XDG_CURRENT_DESKTOP, so
#                             /etc/sway/config.d/50-systemd-user.conf logs "Environment
#                             variable $XDG_CURRENT_DESKTOP not set, ignoring." and
#                             anything sway execs directly cannot see it. (The D-Bus
#                             activation environment still gets it from that same file's
#                             second exec line, which is why portals work regardless.)
#
#   systemd-cat -t sway       sway.desktop wraps sway so compositor output lands in
#                             `journalctl -t sway`. Launched bare, sway's stderr goes to
#                             the VT and is lost on the next mode set — which is exactly
#                             what happened to the 2026-08-18 greeter failure.
#
# $DISPLAY is deliberately not set here: it belongs to Xwayland, sway publishes it itself
# once the X server socket exists, and the warning from 50-systemd-user.conf is just that
# exec racing ahead of Xwayland startup.

export XDG_CURRENT_DESKTOP=sway
export XDG_SESSION_DESKTOP=sway

exec systemd-cat -t sway sway "$@"
