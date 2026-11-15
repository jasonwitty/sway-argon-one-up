# network-panel

A port of the [Omarchy](https://github.com/basecamp/omarchy) network manager
to Sway + waybar: Wi-Fi list with connect / forget / passphrase and 802.1X
prompts, live connection details, Wi-Fi band pinning, DNS provider switch,
captive-portal sign-in and a Wi-Fi share QR code. It talks to NetworkManager.

In Omarchy the panel is a plugin inside its Quickshell desktop shell. Here it
runs as one small standalone [Quickshell](https://quickshell.org) process
that waybar's network module and `Mod+Ctrl+N` toggle over IPC.

| Path | What it is |
|---|---|
| `config/` | The Quickshell config, installed to `~/.config/quickshell/network-panel/` |
| `config/NetworkPanel.qml`, `config/Model.js` | Omarchy's panel, adapted (see the header comment for what changed) |
| `config/Commons/`, `config/Ui/` | The subset of Omarchy's shared style and UI kit the panel needs |
| `config/WifiQrCard.qml` | Share card, a leaner take on Omarchy's `wifiqr` panel |
| `quickshell-build/` | Build recipe for Quickshell, which trixie does not package |
| `../bin/network-panel*` | Launcher and helper scripts (status, band, DNS, QR) |
| `../sway-themes/templates/network-panel-colors.toml` | Theme template; `switch-theme` renders it and the panel recolours live |

## Differences from Omarchy

- **DNS** is set on each saved connection through `nmcli` instead of a global
  root-owned drop-in, so it needs no sudoers rule. A network joined after a
  provider is picked starts on DHCP until the provider is picked again.
- **Speed test** is not ported: it depends on an Omarchy-hosted service.
- **Captive portal** sign-in opens the browser directly; there is no
  Tailscale exit-node handling.
- English strings only.

## Licence

The files under `config/` that come from Omarchy, and the helper scripts
ported from its `bin/`, are MIT licensed, copyright David Heinemeier Hansson;
see `LICENSE-omarchy`.
