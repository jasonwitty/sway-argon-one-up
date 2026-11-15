//@ pragma UseQApplication
import QtQuick
import Quickshell
import qs.network
import qs.bluetooth

// Standalone host for the network and Bluetooth panels. One long-lived
// process: waybar's modules and the Sway keybinds toggle the popups over IPC
// (`network-panel toggle`, `bluetooth-panel toggle`), so they open instantly
// and NetworkManager / BlueZ state is already warm.
//
// Each panel is its own folder in the repo (network-panel/, bluetooth-panel/),
// installed here as network/ and bluetooth/ next to the shared Commons/ and Ui/.
ShellRoot {
  // Both panels take the whole output and the keyboard while open, so only
  // one is ever up: opening one closes the other, and Tab swaps them.
  NetworkPanel {
    id: network
    onOpenedChanged: if (opened) bluetooth.close()
    onSwitchRequested: bluetooth.open()
  }

  BluetoothPanel {
    id: bluetooth
    onOpenedChanged: if (opened) network.close()
    onSwitchRequested: network.open()
  }
}
