# trackpad-guard

A Rust daemon that sits between the AMIRA combo keyboard/touchpad (the input
device in the [Argon ONE UP](https://argon40.com/products/argon-one-up-cm5-laptop-core-system)
laptop) and libinput, forwarding touchpad events through a virtual device so it
can filter out the things that make the stock experience miserable: palm
brushes while typing, and a set of firmware quirks that freeze the cursor.

## Why this exists

Two separate problems, one device:

1. **libinput's disable-while-typing can't see this hardware.** The AMIRA
   presents its keyboard and touchpad as unrelated USB devices with different
   product IDs, so libinput never associates them and DWT does nothing.
   Typing with palms near the pad moves the cursor and clicks mid-sentence.

2. **The firmware misbehaves in ways nothing downstream can fix.** Documented
   across months of captures: it drops finger-up/finger-down reports
   (libinput's contact count desyncs and the cursor locks into a phantom
   scroll/gesture mode), it occasionally goes mute mid-motion, and — the big
   one, captured 2026-08-27 — it randomly marks real fingers as **palms**
   (`ABS_MT_TOOL_TYPE=MT_TOOL_PALM`). libinput silently discards a
   device-flagged palm for the contact's entire life, which looks like a
   dead/frozen trackpad for seconds at a time and explains the long-standing
   intermittent "trackpad stuck" reports.

## How it works (v4 architecture)

1. A udev rule tags the real AMIRA touchpad nodes `LIBINPUT_IGNORE_DEVICE=1`
   so libinput won't open them. The rule is **service-managed**: installed to
   `/run/udev/rules.d/` when the service starts, removed when it stops. It
   lives in `/run` (tmpfs) deliberately — after a power loss the orphaned rule
   evaporates on reboot, so the worst case is "raw touchpad, no guard", never
   "no pointer at all".
2. The daemon opens and `EVIOCGRAB`s the real touchpad, so it is the only
   reader.
3. It creates a virtual uinput device (`trackpad-guard AMIRA touchpad`) that
   mirrors the real pad's capabilities. sway/libinput bind to that.
4. Every event batch from the real pad is forwarded to the virtual device,
   subject to the protections below. From libinput's side there is one
   ordinary touchpad that never toggles or disappears.

### Protections and recoveries

| Mechanism | What it stops |
|---|---|
| **Typing gate** | Touch *motion* within `typing_gate_ms` (default 200ms) of a real key press/release is dropped — palm protection. Contact-count changes (a finger landing or lifting) are always forwarded so libinput's finger count can't desync. Key autorepeat doesn't count as typing. |
| **Tap quarantine** (`gate_taps`) | A contact that *lands* inside the typing gate is withheld from libinput until it proves it's a finger by moving. A palm that touches down and lifts without moving is never seen at all — so it can't become a tap-to-click at wherever the pointer happened to be. |
| **Palm-verdict rewrite** (`trust_tool_palm=false`) | The firmware's bogus `MT_TOOL_PALM` verdicts are rewritten to `MT_TOOL_FINGER` before forwarding. Without this, libinput silently discards the contact and the cursor freezes until you lift. This pad reports no size/pressure axes, so there is no libinput quirks-file alternative. |
| **Ghost-slot release** | If a finger-up report was lost while another finger keeps moving, the stale MT slot is force-released after 1.5s so the cursor doesn't stay locked in two-finger-scroll mode. |
| **Lifted-ghost release** | A slot left open after everything lifted (the finger-up's tracking-id release was lost) is synthesized closed within ~1s — previously these froze the pointer for minutes. |
| **Phantom guard** | A contact that streams at high rate while pinned to one position for 10s is treated as a stuck firmware contact: synthetic release + USB rebind. |
| **Wedge watchdog + rescan** | If the device drops off the USB bus, or goes silent with a finger down, the daemon rebinds and re-grabs the node automatically (~1s blackout). |

## Build and install (testers)

```bash
git clone -b trackpad-tool-palm https://github.com/jasonwitty/sway-argon-one-up.git
cd sway-argon-one-up/trackpad-guard
cargo build --release
./target/release/trackpad-guard --version   # expect: trackpad-guard 0.3.0 (git <hash>)
sudo install -m 755 target/release/trackpad-guard /usr/local/bin/trackpad-guard
sudo systemctl restart trackpad-guard.service
```

Notes:

- A `-dirty` suffix on the hash means your checkout has local modifications.
- The systemd unit, udev rules, and helper scripts are installed by the repo's
  `install.sh`; if you've run that before, replacing the binary and restarting
  is all an upgrade needs. **The installer does not rebuild an existing
  binary** — build and install by hand as above.
- The service ships **disabled** (beta opt-in). Enable with `Mod+Shift+G`, the
  system-dashboard trackpad tile, or
  `sudo systemctl enable --now trackpad-guard.service`.
- It is a **system** service (it must exist before login, or the greeter has
  no pointer), so status and logs use plain `systemctl` / `journalctl`, not
  `--user`.

## Runtime tuning — no rebuild, no restart

The daemon re-reads `/etc/trackpad-guard/config` within ~1s of any change.
Use the helper (also wired to the dashboard's slider):

```bash
trackpad-guard-tune            # print the effective typing gate in ms
trackpad-guard-tune 250        # set it (0–500; 0 disables the gate)
trackpad-guard-tune --taps off # let mid-typing taps through (old behavior)
```

| Key | Default | Meaning |
|---|---|---|
| `typing_gate_ms` | `200` | Drop touch motion within this many ms of a keystroke. `0` disables the gate — useful to test whether the gate is implicated in a problem at all. |
| `gate_taps` | `true` | Withhold contacts that land mid-gate until they move (the tap quarantine). `false` restores pre-2026-08-20 behavior. |
| `trust_tool_palm` | `false` | `true` passes the firmware's palm verdicts through untouched — an A/B switch for the freeze fix. |

## Rescues and keybindings (from this repo's sway config)

| Keys | Action |
|---|---|
| `Mod+Shift+G` | Toggle the service on/off (with a notification). |
| `Mod+Shift+U` | **First thing to try when stuck.** SIGUSR2 contact resync: releases every contact libinput might believe in and re-lands one fresh one. Safe — never touches USB, can't hang, can't lose the pointer. |
| `Mod+Shift+T` | SIGUSR1 USB rebind — the heavier rescue (~1s blackout while the device re-enumerates). |

## Reading the logs

```bash
journalctl -u trackpad-guard -b        # this boot
journalctl -fu trackpad-guard          # follow live
```

The **first line of every run names the build**:
`trackpad-guard: version 0.3.0 (git <commit-hash>)` — the hash matches the
commit you built from (`git rev-parse --short=12 HEAD`).

While in use, a heartbeat line summarizes each ~5s window:

```
hb 5s — tp_batches=242 (fwd=35 drop=207 phantom=0 slotrel=0 tapsupp=1 palmfix=3) key_batches=37 last_key_ago=... | slots=[0:12ms] tool=F touch=Some(1)
```

- `fwd`/`drop` — batches forwarded vs dropped by the typing gate. High `drop`
  alongside high `key_batches` just means the gate is doing its job.
- `palmfix` — firmware palm verdicts rewritten. **This is the number to watch
  on this branch**: nonzero means the freeze fix is actively saving you.
- `tapsupp` — palm taps swallowed by the quarantine.
- `phantom` / `slotrel` — phantom-contact suppressions and ghost-slot releases.
- `slots=[...] tool=... touch=...` — the contact state libinput currently
  believes (open MT slots + ms since update, BTN_TOOL finger count, BTN_TOUCH).

## Reporting a problem

Please include:

1. The **version line** from the journal (or `trackpad-guard --version`).
2. `journalctl -u trackpad-guard -b` output covering the incident — note the
   wall-clock time it happened as precisely as you can.
3. What it felt like (cursor frozen? only 2-finger worked? clicks while
   typing?) and what cleared it (waiting? lifting the finger? `Mod+Shift+U`?
   a restart?).
4. If you can, whether toggling the relevant config key changes it
   (`trust_tool_palm=true`, `gate_taps=false`, `typing_gate_ms=0`) — each is
   live within ~1s, so A/B is cheap.

## Known trade-offs (by design)

- A deliberate **tap that starts within the gate window** of your last
  keystroke and lifts without moving is swallowed with the quarantine on.
  If that bothers you, `trackpad-guard-tune --taps off` or lower the gate.
- With the palm rewrite active, the firmware's palm rejection is overridden
  **everywhere**, not just while typing — a resting palm can move the cursor
  in situations the firmware might previously have (correctly) suppressed.
  The typing gate covers the common case; `trust_tool_palm=true` reverts.
- Phantom/wedge recoveries issue a USB rebind, which costs ~1s of pointer
  blackout while the device re-enumerates.

## Adapting to other hardware

The match logic is AMIRA-specific, in constants at the top of `src/main.rs`:
`VENDOR_ID` (`0x6080`), `KEYBOARD_NAME`, and `TOUCHPAD_NAMES`. To find your
device's values:

```bash
for ev in /sys/class/input/event*; do
    echo "$(basename "$ev"): vid=$(cat "$ev/device/id/vendor") name='$(cat "$ev/device/name")'"
done
```

The USB-rebind recovery paths also assume the AMIRA's vendor/product IDs
(`6080:8061`); other hardware would need those constants updated too.

## License

Apache-2.0 — see [LICENSE](../LICENSE) at the repo root.
