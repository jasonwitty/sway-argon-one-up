# touchpad-native

Typing protection for the Argon ONE UP touchpad using libinput's own
disable-while-typing, with no daemon. This is what the installer sets up;
`trackpad-guard/` is the earlier daemon. It is retired: the installer no longer sets it up and removes it when upgrading.

The approach is **squeeby's**: the AMIRA keyboard and touchpad are separate
USB devices, so libinput treats them as unrelated and never pairs them. Tag
both as internal and it pairs them itself.

| File | Installed to | What it does |
|---|---|---|
| `local-overrides.quirks` | `/etc/libinput/local-overrides.quirks` | Tags the keyboard internal; makes libinput ignore the touchpad's unreliable palm label |
| `99-argon-one-touchpad.rules` | `/etc/udev/rules.d/` | Tags the touchpad internal |
| `touchpad-mode` | `/usr/local/bin/` (symlink) | `on` / `off` / `toggle`: disable-while-typing on or off, live and in the sway config; what the dashboard tile and `Mod+Shift+G` run. `native` / `guard` / `status`: switches between this and trackpad-guard |

```
touchpad-mode toggle   # disable-while-typing on/off, no root, no re-login
touchpad-mode native   # set this up (the installer does); then log out and back in
touchpad-mode guard    # back to trackpad-guard (only if you installed it by hand)
```

Two things beyond squeeby's original steps:

- The keyboard quirk matches the vendor only. Typing arrives on product
  `0x8062` on some units and `0x8060` on others.
- `AttrEventCode=-ABS_MT_TOOL_TYPE;` on the touchpad. Its firmware
  intermittently labels a normal finger as a palm, and libinput then drops
  that contact entirely (a cursor frozen for seconds to minutes).
  trackpad-guard works around this by rewriting the label; in native mode
  libinput is told not to read it.

The old `/etc/udev/rules.d/99-amira-dwt.rules` set
`LIBINPUT_ATTR_KEYBOARD_INTEGRATION`, a udev property libinput no longer
reads, so it never had any effect. `touchpad-mode native` removes it.
