//@ pragma UseQApplication
import Quickshell

// Standalone host for the network panel. One long-lived process: waybar's
// network module and the Sway keybind toggle the popup over IPC
// (`network-panel toggle`), so it opens instantly and NetworkManager state is
// already warm.
ShellRoot {
  NetworkPanel {}
}
