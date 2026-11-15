# clock-panel

A port of the calendar popup from the [Omarchy](https://github.com/basecamp/omarchy)
clock to Sway + waybar: a month grid with ISO week numbers, today outlined,
and a year-progress rail.

Clicking waybar's clock or `Mod+Ctrl+C` toggles it. It runs in the shared
panel process; see `../quickshell-panels/` for the host, the shared UI kit,
the Quickshell build and the licence.

| Path | What it is |
|---|---|
| `ClockPanel.qml`, `Model.js`, `WallClock.qml` | Omarchy's panel, adapted (see the header comment for what changed) |
| `../bin/clock-panel` | Opens / closes the panel |

Keys: left / right step a month, up / down a year, `t` or Enter returns to
today, `w` (or clicking the `W` heading) switches weeks between starting on
Sunday and Monday, Esc closes.

## Differences from Omarchy

- **The bar label** stays waybar's own clock module; only the popup is ported.
- **Settings** (week start, and the optional birth year behind a double-click
  on the year bar) are kept in `~/.config/quickshell/clock-panel.json`
  instead of Omarchy's `shell.json`.
- **World clock and weather** panels are separate Omarchy plugins and are not
  ported.
