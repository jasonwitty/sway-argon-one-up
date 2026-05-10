# system-dashboard

A small, slick Tauri-based dashboard that surfaces five live system metrics on the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system) (and any Linux machine with the matching hardware paths). Replaces the stock Argon dashboard that the hardware battery key used to open.

| Tile | Source | Click action |
|---|---|---|
| Battery | CW2217 fuel gauge over I2C (percent + current draw → time-remaining estimate) | — |
| Power | `/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor` + AC inferred from CW2217 | `power-mode toggle` (cycles ondemand → powersave → performance) |
| Idle | `/tmp/.idle-toggle-mode` (written by `bin/idle-toggle`) | `idle-toggle cycle` (on → off → claude) |
| Theme | `~/.config/sway-themes/current` | `switch-theme --picker` |
| Fan | `/dev/shm/argon-fan.json` (written by the [argon-fan](../argon-fan/) daemon) | `argon-fan picker` |

## Why this exists

The Argon ONE UP keyboard has a hardware battery key (`XF86Battery`) that used to launch Argon's own dashboard. Since we replaced their battery polling with [argon-battery-rs](../argon-battery-rs/), the stock dashboard no longer has live data and the key effectively did nothing.

This app fills the same role, but with the data we actually have (and a little more — fan mode, idle/lock state, current theme), and is theme-aware so it follows the rest of the desktop palette.

## Architecture

Built with **Tauri 2** (Rust backend + a WebView frontend). The frontend is vanilla HTML/CSS/JS — no Node toolchain. Backend modules:

```
src/
├── main.rs       # Tauri builder + IPC commands
├── battery.rs    # I2C reads from CW2217
├── power.rs      # governor + AC presence
├── idle.rs       # idle-toggle state file reader
├── theme.rs      # current theme name + rendered CSS path
└── fan.rs        # argon-fan state file reader
```

Frontend (`dist/`) calls a single `snapshot` IPC command every 2 seconds while focused (10 seconds while blurred — easier on battery). Click handlers invoke separate IPC commands that shell out to the existing user scripts (`power-mode`, `idle-toggle`, `switch-theme`, `argon-fan`).

## Theming

`sway-themes/templates/dashboard.css` defines CSS custom properties (`--bg`, `--text`, `--accent`, per-tile accents, etc.) using Catppuccin palette tokens (`@@BASE@@`, `@@MAUVE@@`, …). When you switch theme with `Mod+T`, `bin/switch-theme` renders the template to `~/.config/system-dashboard/dashboard.css`. The frontend loads that file and reacts to changes via a file-watcher → the dashboard recolors live without restarting.

## Build

This crate **does not build during `install.sh`**. The installer downloads a prebuilt `aarch64` binary from the latest `system-dashboard-v*` GitHub release. For local dev:

```bash
sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    libssl-dev librsvg2-dev libsoup-3.0-dev patchelf

cd system-dashboard
cargo build --release
./target/release/system-dashboard
```

First Tauri build on a Pi 5 takes 5–15 minutes (`tauri` pulls a large transitive dep tree). Incremental builds are fast.

## Release flow

A push of a `system-dashboard-v*` tag triggers `.github/workflows/release-system-dashboard.yml`. The workflow runs on `ubuntu-24.04-arm`, builds the release binary, and uploads `system-dashboard-aarch64` to the GitHub Release for that tag. The installer queries the GitHub API for the latest such tag and downloads the asset.

To cut a release:

```bash
git tag system-dashboard-v0.1.0
git push origin system-dashboard-v0.1.0
```

You can also run the workflow manually via `workflow_dispatch` — that path uploads to workflow artifacts only (no release).

## Sway keybindings

Bound in `sway/config`:

```
bindsym $mod+Shift+d exec system-dashboard
bindsym XF86Battery exec system-dashboard
```

`Mod+D` is taken by the wofi app launcher (`$menu`), hence `Mod+Shift+d` for the dashboard.

## Use on other window managers / desktops

The `.desktop` file shipped at `/usr/local/share/applications/system-dashboard.desktop` exposes the app to any launcher (wofi-drun, gnome-shell activities, KDE krunner, etc.).

To bind a key on a different WM, run the binary directly:

- **i3**: `bindsym $mod+Shift+d exec system-dashboard`
- **Hyprland**: `bind = SUPER SHIFT, D, exec, system-dashboard`
- **GNOME**: Settings → Keyboard Shortcuts → custom shortcut

The four backend scripts (`power-mode`, `idle-toggle`, `switch-theme`, `argon-fan`) are sway-help-specific. If you're using this app outside that desktop, the read-side will still work (battery, fan, theme name from a file you point it at) but the click actions will no-op unless those scripts are on `$PATH`.

## License

Apache-2.0
