# Troubleshooting

Common issues and fixes for the Sway Argon ONE UP configuration.

## System is extremely slow / NVMe timeouts

Some NVMe drives (observed with certain TeamGroup models) can become extremely sluggish or cause I/O timeouts due to power state transitions on the CM5's PCIe bus. This may not affect all drives.

Check whether the kernel parameters are already set:

```bash
cat /proc/cmdline | grep nvme_core
```

If `nvme_core.default_ps_max_latency_us=0` and `pcie_aspm=off` are not present, edit the kernel command line (**keep everything on one line**):

```bash
sudo nano /boot/firmware/cmdline.txt
```

Append to the existing line:

```
nvme_core.default_ps_max_latency_us=0 pcie_aspm=off
```

This disables NVMe power state transitions and PCIe Active State Power Management. Reboot and verify:

```bash
sudo reboot
dmesg -T | grep -i nvme
```

## Brightness keys don't work

Check that the display is accessible over I2C bus 14:

```bash
sudo i2cdetect -y 14  # should show device at 0x37
ddcutil --bus 14 getvcp 10  # should return current brightness
```

The brightness script writes DDC/CI commands directly to `/dev/i2c-14`. Make sure your user has access (should be in the `i2c` or `render` group).

## No sound / volume keys don't work

This config uses PipeWire with WirePlumber (`wpctl`), not PulseAudio (`pactl`):

```bash
wpctl status
wpctl get-volume @DEFAULT_AUDIO_SINK@
```

## Lid close doesn't do anything / no power savings

The Argon ONE UP uses a GPIO-based lid switch, not a standard ACPI lid. Sway `bindswitch` and `logind.conf` `HandleLidSwitch` will not work. The lid is handled entirely by the Argon daemon.

Check that the Argon daemon is running and configured:

```bash
systemctl status argononed  # daemon should be active
cat /etc/argononeupd.conf   # should contain lidaction=suspend
```

Check the lid event log to see if the script is being called:

```bash
cat ~/.local/state/lid-events.log
```

A successful close/open cycle looks like:

```
=== LID CLOSE 2026-04-03 10:01:06 ===
  display: off
  cpu governor: powersave
  wifi: Soft blocked: yes
  bluetooth: Soft blocked: yes
  webcam (1-1.4): unbound
=== LID OPEN 2026-04-03 10:04:45 ===
  display: on
  cpu governor: ondemand
  wifi: Soft blocked: no
  bluetooth: Soft blocked: no
  webcam (1-1.4): bound
```

If the log file doesn't exist, the script isn't being called. Verify:

- `/etc/argononeupd.conf` has `lidaction=suspend`
- `~/.local/bin/lid-suspend` exists and is executable
- The Argon daemon's `argonpowerbutton.py` has the suspend case that calls `lid-suspend`

If the log shows a subsystem failed (e.g. `webcam: not found`), the webcam vendor ID (`11cc:2812`) may not match your hardware. Check with `lsusb` and update the IDs in the script.

## Black screen after lid close (hard reboot required)

This means something attempted a real system suspend. The Pi 5 / CM5 has no suspend support -- it will freeze. Check:

```bash
# Make sure logind is NOT trying to handle the lid
grep -r HandleLidSwitch /etc/systemd/logind.conf /etc/systemd/logind.conf.d/ 2>/dev/null
```

All `HandleLidSwitch` values should be commented out or set to `ignore`. The safest option is to not have any overrides -- the Argon GPIO daemon handles the lid, not logind.

Also make sure the powermenu does not include a Suspend option (`systemctl suspend` will black-screen).

If a resume leaves the CPU stuck on `powersave` while you're plugged in, check `~/.local/state/lid-events.log` for the `battery_class=` field. `lid-suspend` asks `argon-battery-rs` for the power state because a raw `i2cget 0x0e` returns `0xff` when the fuel gauge is asleep, and `0xff` has bit 7 set — indistinguishable from "discharging". A `battery_class=unknown` means the gauge is recovering and the governor correctly defaults to `ondemand`.

## Waybar or wob missing after sway reload

These use `exec_always` and should survive reloads. If they don't appear, check the processes:

```bash
pgrep waybar
pgrep wob
```

And restart manually if needed: `swaymsg reload`
