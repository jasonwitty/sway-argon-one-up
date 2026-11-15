# bluetooth-panel

A port of the [Omarchy](https://github.com/basecamp/omarchy) Bluetooth panel
to Sway + waybar: power switch, paired and nearby devices, pair / connect /
disconnect / forget and battery level. It talks to BlueZ.

Waybar's Bluetooth module and `Mod+Ctrl+B` toggle it; right-click the waybar
icon turns Bluetooth on or off. It runs in the shared panel process; see
`../quickshell-panels/` for the host, the shared UI kit, the Quickshell build
and the licence.

| Path | What it is |
|---|---|
| `BluetoothPanel.qml`, `Model.js` | Omarchy's panel, adapted (see the header comment for what changed) |
| `../bin/bluetooth-panel` | Opens / closes the panel; `bluetooth-panel agent` runs the pairing agent (`bt-agent`, from `bluez-tools`) |
| `../bin/bluetooth-panel-power` | On/off through the rfkill soft block, so the choice survives a reboot |
| `../bin/bluetooth-panel-device` | Pair / connect / disconnect / forget through `bluetoothctl` |

## Differences from Omarchy

- **Default audio output** after a connect is set by `bluetooth-panel-device`
  with `wpctl` rather than in the panel through Quickshell's PipeWire module,
  which this build leaves out. It waits for the A2DP sink so it does not race
  `bt-a2dp-guard`.
- **Pairing agent** is started from the Sway config rather than a systemd
  user unit, because this session never activates `graphical-session.target`.
- **Devices that share a name** show their address, so two of the same model
  can be told apart.
