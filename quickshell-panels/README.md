# quickshell-panels

What the [Omarchy](https://github.com/basecamp/omarchy) panel ports share.
In Omarchy the panels are plugins inside its Quickshell desktop shell; here
they run in one small standalone [Quickshell](https://quickshell.org) process
that waybar and the Sway keybinds toggle over IPC.

| Path | What it is |
|---|---|
| `config/shell.qml` | Host: loads both panels, keeps only one open, Tab swaps them |
| `config/Commons/`, `config/Ui/` | The subset of Omarchy's shared style and UI kit the panels need |
| `quickshell-build/` | Build recipe for Quickshell, which trixie does not package |
| `../network-panel/` | The network panel |
| `../bluetooth-panel/` | The Bluetooth panel |
| `../bin/panels` | Starts / stops the process and forwards IPC calls |
| `../sway-themes/templates/panels-colors.toml` | Theme template; `switch-theme` renders it and the panels recolour live |

Installed layout, under `~/.config/quickshell/panels/`:

```
shell.qml  Commons/  Ui/     from config/
network/                     from ../network-panel/
bluetooth/                   from ../bluetooth-panel/
theme/colors.toml            rendered by switch-theme
```

Strings are English only.

## Licence

The files that come from Omarchy, and the helper scripts ported from its
`bin/`, are MIT licensed, copyright David Heinemeier Hansson; see
`LICENSE-omarchy`.
