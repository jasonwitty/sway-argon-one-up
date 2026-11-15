//@ pragma UseQApplication
import QtQuick
import Quickshell
import qs.network
import qs.bluetooth
import qs.clock

// Standalone host for the network, Bluetooth and calendar panels. One
// long-lived process: waybar's modules and the Sway keybinds toggle the popups
// over IPC (`network-panel toggle`, `bluetooth-panel toggle`,
// `clock-panel toggle`), so they open instantly and NetworkManager / BlueZ
// state is already warm.
//
// Each panel is its own folder in the repo (network-panel/, bluetooth-panel/,
// clock-panel/), installed here as network/, bluetooth/ and clock/ next to the
// shared Commons/ and Ui/.
ShellRoot {
  id: shell

  // Every panel takes the whole output and the keyboard while open, so only
  // one is ever up: opening one closes the others, and Tab walks the ring.
  readonly property var ring: [clock, network, bluetooth]

  function opening(panel) {
    for (var i = 0; i < ring.length; i++) if (ring[i] !== panel) ring[i].close()
  }

  function step(panel, direction) {
    var next = (ring.indexOf(panel) + direction + ring.length) % ring.length
    ring[next].open()
  }

  ClockPanel {
    id: clock
    onOpenedChanged: if (opened) shell.opening(clock)
    onSwitchRequested: function(direction) { shell.step(clock, direction) }
  }

  NetworkPanel {
    id: network
    onOpenedChanged: if (opened) shell.opening(network)
    onSwitchRequested: function(direction) { shell.step(network, direction) }
  }

  BluetoothPanel {
    id: bluetooth
    onOpenedChanged: if (opened) shell.opening(bluetooth)
    onSwitchRequested: function(direction) { shell.step(bluetooth, direction) }
  }
}
