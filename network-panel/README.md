# network-panel

A port of the [Omarchy](https://github.com/basecamp/omarchy) network manager
to Sway + waybar: Wi-Fi list with connect / forget / passphrase and 802.1X
prompts, live connection details, Wi-Fi band pinning, DNS provider switch,
captive-portal sign-in and a Wi-Fi share QR code. It talks to NetworkManager.

Waybar's network module and `Mod+Ctrl+N` toggle it. It runs in the shared
panel process; see `../quickshell-panels/` for the host, the shared UI kit,
the Quickshell build and the licence.

| Path | What it is |
|---|---|
| `NetworkPanel.qml`, `Model.js` | Omarchy's panel, adapted (see the header comment for what changed) |
| `WifiQrCard.qml` | Share card, a leaner take on Omarchy's `wifiqr` panel |
| `../bin/network-panel` | Opens / closes the panel |
| `../bin/network-panel-status`, `-band`, `-dns`, `-qr` | Helper scripts the panel calls |

## Differences from Omarchy

- **DNS** is set on each saved connection through `nmcli` instead of a global
  root-owned drop-in, so it needs no sudoers rule. A network joined after a
  provider is picked starts on DHCP until the provider is picked again.
- **Speed test** is not ported: it depends on an Omarchy-hosted service.
- **Captive portal** sign-in opens the browser directly; there is no
  Tailscale exit-node handling.
