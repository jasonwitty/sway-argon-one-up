//! Disable sway touchpad while typing — evdev intercept (v4).
//!
//! Architecture: trackpad-guard sits between the real AMIRA touchpad
//! and libinput. We do NOT use `swaymsg ... events disabled` anymore,
//! because that flow killed in-progress touches in libinput's view and
//! forced the user to lift+retouch to recover.
//!
//! Instead:
//! 1. A udev rule tags the AMIRA touchpad evdev nodes with
//!    LIBINPUT_IGNORE_DEVICE=1 so libinput refuses to open them.
//! 2. We open + EVIOCGRAB the real touchpad nodes ourselves.
//! 3. We create a virtual uinput device that mirrors the real touchpad's
//!    capabilities (vid:pid, keys, relative axes, properties).
//! 4. Each event batch from the real touchpad is forwarded byte-for-byte
//!    to the virtual device — UNLESS a keyboard event has been seen in
//!    the last KEY_TYPING_WINDOW (150 ms), in which case the batch is
//!    silently dropped.
//!
//! From sway/libinput's perspective there's only the virtual device,
//! and it never changes state — no send_events toggles, no
//! "touch cancelled" events. So mid-typing brushes on the trackpad
//! simply do nothing; the moment typing stops, the next motion event
//! flows through naturally.
//!
//! SIGUSR1 still triggers a manual USB unbind/rebind on the AMIRA
//! touchpad interface (vid:pid 6080:8061), bound to Mod+Shift+T.

use evdev::{uinput::VirtualDevice, Device, EventSummary, InputEvent, UinputAbsSetup};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const VENDOR_ID: u16 = 0x6080;
const KEYBOARD_NAME: &str = "AMIRA-KEYBOAR USB KEYBOARD";
/// The AMIRA's mouse-class evdev node has one of these names depending on
/// which kernel HID driver is bound to the interface (hid-generic vs
/// hid-multitouch). We open whichever appears.
const TOUCHPAD_NAMES: &[&str] = &[
    "AMIRA-KEYBOAR USB KEYBOARD Touchpad",
    "AMIRA-KEYBOAR USB KEYBOARD Mouse",
];
/// Name we give the virtual replacement device. Deliberately distinct
/// from the real names so the LIBINPUT_IGNORE_DEVICE udev rule (which
/// matches the real names) doesn't apply to our virtual.
const VIRTUAL_NAME: &str = "trackpad-guard AMIRA touchpad";

/// USB ids used by the SIGUSR1-driven manual rebind path.
const TOUCHPAD_USB_PRODUCT: &str = "8061";
const TOUCHPAD_USB_VENDOR_STR: &str = "6080";

/// Touchpad events arriving within this window of the most recent
/// keyboard event are dropped (typing → palm protection).
const KEY_TYPING_WINDOW: Duration = Duration::from_millis(150);

const DISCOVER_INTERVAL: Duration = Duration::from_secs(2);

enum Msg {
    /// A keyboard event batch arrived. The timestamp is captured in the
    /// reader thread the moment fetch_events() returns, so it reflects
    /// when the kernel emitted the event — NOT when the main loop gets
    /// around to processing the message. Using processing time would
    /// drift under load: a backlog can make us think a stale keystroke
    /// happened "now" and incorrectly gate touchpad events that are
    /// actually well past the typing window.
    KeyActivity { at: Instant },
    TouchpadEvents {
        events: Vec<InputEvent>,
        at: Instant,
    },
    KeyboardReaderDied,
    /// A touchpad reader thread exited (typically ENODEV from fetch_events
    /// when the underlying USB device drops or its evdev node is
    /// invalidated). Carries the device path so the main loop can drop
    /// just that entry from the active set and schedule a rescan to
    /// re-grab whatever node reappears.
    TouchpadReaderDied { path: PathBuf },
    ManualRebind,
    Shutdown,
}

fn matches_keyboard(device: &Device) -> bool {
    device.input_id().vendor() == VENDOR_ID && device.name() == Some(KEYBOARD_NAME)
}

fn matches_touchpad(device: &Device) -> bool {
    if device.input_id().vendor() != VENDOR_ID {
        return false;
    }
    let Some(name) = device.name() else { return false };
    TOUCHPAD_NAMES.contains(&name)
}

fn find_keyboards() -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| matches_keyboard(d))
        .collect()
}

fn find_touchpads() -> Vec<(PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| matches_touchpad(d))
        .collect()
}

fn spawn_keyboard_reader(mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                let at = Instant::now();
                let mut had_transition = false;
                for ev in events {
                    if let EventSummary::Key(_, _, value) = ev.destructure() {
                        // Only real presses (value=1) and releases
                        // (value=0) count as "typing." Autorepeat
                        // (value=2) is *deliberately* ignored: the
                        // AMIRA drops release events often enough that
                        // counting autorepeats as activity would let a
                        // single missed release lock the touchpad off
                        // forever — the kernel keeps autorepeating
                        // until we get a fresh release. This matches
                        // the original Rust port's design.
                        if value == 0 || value == 1 {
                            had_transition = true;
                        }
                    }
                }
                if had_transition && tx.send(Msg::KeyActivity { at }).is_err() {
                    return;
                }
            }
            Err(_) => {
                let _ = tx.send(Msg::KeyboardReaderDied);
                return;
            }
        }
    });
}

fn spawn_touchpad_reader(path: PathBuf, mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                let at = Instant::now();
                let batch: Vec<InputEvent> = events.collect();
                if !batch.is_empty()
                    && tx
                        .send(Msg::TouchpadEvents { events: batch, at })
                        .is_err()
                {
                    return;
                }
            }
            Err(_) => {
                let _ = tx.send(Msg::TouchpadReaderDied { path });
                return;
            }
        }
    });
}

/// Dedupe candidate touchpads the same way initial discovery does: for
/// each USB product id, prefer the "Touchpad"-named device over the
/// "Mouse"-named one. Used by both first-time setup and the rescan path.
fn dedupe_touchpads(touchpads: Vec<(PathBuf, Device)>) -> Vec<(PathBuf, Device)> {
    let mut chosen: HashMap<u16, (PathBuf, Device)> = HashMap::new();
    for (path, dev) in touchpads {
        let product = dev.input_id().product();
        let is_touchpad = dev.name() == Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad");
        let prefer_new = match chosen.get(&product) {
            None => true,
            Some(existing) => {
                is_touchpad
                    && existing.1.name() != Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad")
            }
        };
        if prefer_new {
            chosen.insert(product, (path, dev));
        }
    }
    chosen.into_values().collect()
}

/// Re-enumerate /dev/input, grab any AMIRA touchpad we don't already
/// hold, and spawn a reader thread for it. Returns the number of newly
/// acquired devices. Safe to call repeatedly — devices already in
/// `active` are skipped, so this is the same primitive used both at
/// startup and on recovery.
fn try_acquire_missing(
    active: &mut HashSet<PathBuf>,
    tx: &mpsc::Sender<Msg>,
) -> usize {
    let mut acquired = 0;
    for (path, mut dev) in dedupe_touchpads(find_touchpads()) {
        if active.contains(&path) {
            continue;
        }
        match dev.grab() {
            Ok(()) => {
                eprintln!("trackpad-guard: (re)grabbed {}", path.display());
                spawn_touchpad_reader(path.clone(), dev, tx.clone());
                active.insert(path);
                acquired += 1;
            }
            Err(e) => {
                eprintln!(
                    "trackpad-guard: grab {} failed during rescan: {e}",
                    path.display()
                );
            }
        }
    }
    acquired
}

/// Build a uinput virtual device that mirrors the capabilities of the
/// given real device. Sway/libinput will discover this and treat it as
/// the actual touchpad (the real one is hidden via LIBINPUT_IGNORE_DEVICE).
fn create_virtual_touchpad(real: &Device) -> std::io::Result<VirtualDevice> {
    let mut builder = VirtualDevice::builder()?
        .name(VIRTUAL_NAME)
        .input_id(real.input_id());

    if let Some(keys) = real.supported_keys() {
        builder = builder.with_keys(keys)?;
    }
    if let Some(rel) = real.supported_relative_axes() {
        builder = builder.with_relative_axes(rel)?;
    }
    // Mirror absolute axes one-by-one — libinput needs the ABS_X/ABS_Y +
    // resolution info to classify the device as a touchpad rather than a
    // plain mouse, and to compute pointer acceleration correctly.
    if let Ok(absinfo_iter) = real.get_absinfo() {
        for (code, info) in absinfo_iter {
            builder = builder.with_absolute_axis(&UinputAbsSetup::new(code, info))?;
        }
    }
    if let Some(msc) = real.misc_properties() {
        builder = builder.with_msc(msc)?;
    }
    let props = real.properties();
    if props.iter().next().is_some() {
        builder = builder.with_properties(props)?;
    }
    builder.build()
}

/// True if the AMIRA touchpad's USB device is currently bound to the
/// `usb` driver — i.e., the kernel sees it on the bus and the port
/// directory has an `idProduct` of 0x8061 with our vendor. Used as the
/// authoritative "device is really gone" signal for the wedge watchdog,
/// since plain event silence can just mean the user isn't touching the
/// trackpad. Reads a few small sysfs files, no syscalls into hotpath.
fn touchpad_is_bound_to_bus() -> bool {
    let drivers = match std::fs::read_dir("/sys/bus/usb/drivers/usb/") {
        Ok(d) => d,
        Err(_) => return true, // be conservative — don't false-positive a rebind
    };
    for entry in drivers.flatten() {
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let product = std::fs::read_to_string(path.join("idProduct"))
            .ok()
            .map(|s| s.trim().to_string());
        if vendor.as_deref() == Some(TOUCHPAD_USB_VENDOR_STR)
            && product.as_deref() == Some(TOUCHPAD_USB_PRODUCT)
        {
            return true;
        }
    }
    false
}

fn find_touchpad_usb_id() -> Option<String> {
    for entry in std::fs::read_dir("/sys/bus/usb/devices/").ok()?.flatten() {
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let product = std::fs::read_to_string(path.join("idProduct"))
            .ok()
            .map(|s| s.trim().to_string());
        if vendor.as_deref() == Some(TOUCHPAD_USB_VENDOR_STR)
            && product.as_deref() == Some(TOUCHPAD_USB_PRODUCT)
        {
            return Some(entry.file_name().to_string_lossy().into_owned());
        }
    }
    None
}

fn write_sysfs_with_fallback(path: &str, value: &str) -> std::io::Result<()> {
    // Fast path: try a direct write. Succeeds when the daemon runs as
    // root (the system-service deployment path). If the open fails with
    // PermissionDenied, fall back to `sudo -n tee` so the binary still
    // works when launched by hand as a regular user with the appropriate
    // sudoers entry.
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(mut f) => {
            f.write_all(value.as_bytes())?;
            f.write_all(b"\n")?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let mut child = Command::new("sudo")
                .arg("-n")
                .arg("tee")
                .arg(path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(value.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            let status = child.wait()?;
            if !status.success() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("sudo tee {path} exited with {status}"),
                ));
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn rebind_touchpad_usb() {
    // We try to find the device's USB path under /sys/bus/usb/devices/. If
    // the touchpad has already self-disconnected from the bus, that lookup
    // returns None — but we can still recover by binding any AMIRA-shaped
    // device ID back to the usb driver, since the kernel still has the
    // port number even after the device dropped. Fall back to the known
    // hub path "1-1.6" which is where the AMIRA touchpad interface lives.
    let id_for_bind = find_touchpad_usb_id().unwrap_or_else(|| "1-1.6".to_string());
    let id_for_unbind = find_touchpad_usb_id();

    eprintln!(
        "trackpad-guard: USB rebind on {} (unbind={} + bind)",
        id_for_bind,
        id_for_unbind.as_deref().unwrap_or("<absent>"),
    );

    // Step 1: attempt unbind only if the device is currently bound.
    // The recurring wedge mode we see in practice is that the AMIRA
    // touchpad drops itself off the bus before our watchdog notices —
    // unbind then fails with ENODEV and we used to abort before getting
    // to the bind step, leaving the device stuck off the bus indefinitely.
    if let Some(unbind_id) = id_for_unbind {
        match write_sysfs_with_fallback("/sys/bus/usb/drivers/usb/unbind", &unbind_id) {
            Ok(()) => {
                thread::sleep(Duration::from_millis(500));
            }
            Err(e) if e.raw_os_error() == Some(19) => {
                // ENODEV — device already gone from the bus. That's the
                // state we want anyway; proceed to bind.
                eprintln!(
                    "trackpad-guard: USB rebind: unbind reported ENODEV; \
                     device already off the bus, proceeding to bind"
                );
            }
            Err(e) => {
                eprintln!("trackpad-guard: USB rebind: unbind failed: {e} (continuing to bind)");
            }
        }
    } else {
        eprintln!("trackpad-guard: USB rebind: device not currently bound, skipping unbind");
    }

    // Step 2: bind. This is the step that actually brings the device
    // back. If it fails, log loudly — the rescan won't have anything to
    // grab.
    if let Err(e) = write_sysfs_with_fallback("/sys/bus/usb/drivers/usb/bind", &id_for_bind) {
        eprintln!("trackpad-guard: USB rebind: bind failed: {e}");
        return;
    }
    eprintln!("trackpad-guard: USB rebind complete");
}

fn main() {
    let (tx, rx) = mpsc::channel::<Msg>();

    {
        let tx = tx.clone();
        let mut signals =
            Signals::new([SIGUSR1, SIGTERM, SIGINT, SIGHUP]).expect("install signal handler");
        thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGUSR1 => {
                        let _ = tx.send(Msg::ManualRebind);
                    }
                    _ => {
                        let _ = tx.send(Msg::Shutdown);
                        return;
                    }
                }
            }
        });
    }

    // Initial discovery.
    let keyboards = loop {
        let found = find_keyboards();
        if !found.is_empty() {
            break found;
        }
        eprintln!("trackpad-guard: no matching keyboards found, retrying in 2s");
        thread::sleep(DISCOVER_INTERVAL);
    };

    let mut touchpads = loop {
        let found = find_touchpads();
        if !found.is_empty() {
            break found;
        }
        eprintln!("trackpad-guard: no matching touchpads found, retrying in 2s");
        thread::sleep(DISCOVER_INTERVAL);
    };

    // On the AMIRA, both hid-generic and hid-multitouch can bind to the
    // same physical USB interface and each creates its own input device
    // ("Mouse" and "Touchpad" respectively) carrying the same underlying
    // HID reports. Forwarding from both would double every event. Dedupe
    // by USB product id, preferring the Touchpad-named variant.
    touchpads = dedupe_touchpads(touchpads);

    // Build the virtual replacement device. Prefer mirroring from a
    // "Touchpad"-named real device if present — hid-multitouch exposes
    // it with INPUT_PROP_POINTER + INPUT_PROP_BUTTONPAD set, which is
    // what makes libinput classify the result as a touchpad (not a
    // plain mouse). Fall back to the first found otherwise.
    let mirror_idx = touchpads
        .iter()
        .position(|(_, d)| d.name() == Some("AMIRA-KEYBOAR USB KEYBOARD Touchpad"))
        .unwrap_or(0);
    let virt = match create_virtual_touchpad(&touchpads[mirror_idx].1) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "trackpad-guard: failed to create virtual touchpad: {e}. \
                 Check that /dev/uinput is writable by the input group \
                 (see udev/60-trackpad-guard-amira.rules) and that the \
                 uinput kernel module is loaded."
            );
            std::process::exit(1);
        }
    };

    // EVIOCGRAB each real touchpad so nothing else reads them.
    let mut active_touchpads: HashSet<PathBuf> = HashSet::new();
    for (path, dev) in touchpads.iter_mut() {
        if let Err(e) = dev.grab() {
            eprintln!(
                "trackpad-guard: failed to EVIOCGRAB {}: {e}. \
                 If libinput is still holding it, you may need to restart \
                 sway or unbind/rebind the USB device once for the \
                 LIBINPUT_IGNORE_DEVICE tag to take effect.",
                path.display()
            );
            std::process::exit(1);
        }
        active_touchpads.insert(path.clone());
    }

    eprintln!(
        "trackpad-guard: watching {} keyboard(s), {} real touchpad(s), \
         virtual device created (mirroring AMIRA capabilities)",
        keyboards.len(),
        touchpads.len()
    );

    let mut keyboards_alive = keyboards.len();
    // Remember how many touchpads we had at the cleanest moment we ever
    // observed. The opportunistic rescan path uses this as the target
    // and stops enumerating /dev/input every tick once we're back to it
    // — enumeration is non-trivial work on the main thread and starves
    // touchpad event forwarding when run too often.
    let expected_touchpads = touchpads.len();
    for (_, device) in keyboards {
        spawn_keyboard_reader(device, tx.clone());
    }
    for (path, device) in touchpads {
        spawn_touchpad_reader(path, device, tx.clone());
    }

    let mut last_key_ts: Option<Instant> = None;
    let mut last_rebind = Instant::now() - Duration::from_secs(60);
    let debug = std::env::var_os("TRACKPAD_GUARD_DEBUG").is_some();
    let mut virt = virt;
    let mut dropped_batches: u64 = 0;
    let mut forwarded_batches: u64 = 0;
    // Initial state is "forwarding" — first batch with no recent key
    // stays in that state silently; first batch during typing logs the
    // transition.
    let mut was_forwarding = true;
    // Suppress unused-when-debug-off warning.
    let _ = debug;

    // Heartbeat counters: rolling 5-second window of activity so that
    // when the user reports a stuck moment we can see exactly what the
    // daemon was seeing in the surrounding period.
    let mut hb_last = Instant::now();
    let mut hb_tp: u64 = 0;
    let mut hb_tp_fwd: u64 = 0;
    let mut hb_tp_drop: u64 = 0;
    let mut hb_key: u64 = 0;
    const HEARTBEAT: Duration = Duration::from_secs(5);

    // Recovery state: when a touchpad reader dies (ENODEV from the USB
    // device dropping or its evdev node being invalidated), we schedule a
    // rescan instead of exiting. The main loop wakes on the deadline,
    // tries to (re)grab any missing touchpads, and reschedules with
    // exponential backoff if the node hasn't reappeared yet.
    let mut next_rescan: Option<Instant> = None;
    let mut rescan_backoff = Duration::from_millis(200);
    const RESCAN_INITIAL: Duration = Duration::from_millis(200);
    const RESCAN_BACKOFF_MAX: Duration = Duration::from_secs(2);
    // recv_timeout when no rescan is pending — long enough to effectively
    // block until a real message arrives.
    const IDLE_TIMEOUT: Duration = Duration::from_secs(3600);

    // Wedge-watchdog state: catches the "events were flowing, then stopped
    // without the evdev node going away" failure mode. The rescan path
    // above only fires when fetch_events() returns Err; in the wedge case
    // the reader stays blocked on a healthy node that has simply gone
    // silent, so we need an active check. We force a USB rebind if:
    //   (1) the touchpad produced enough events recently to count as
    //       "active" since the last rebind (WEDGE_BURST_MIN), and
    //   (2) it's been silent for at least WEDGE_SILENCE for the last
    //       observed event, and
    //   (3) we haven't already rebound in the last WEDGE_COOLDOWN.
    //
    // Note: an earlier version of this added "user present at the
    // keyboard within WEDGE_KEY_PRESENCE" as a fourth condition, to
    // avoid spurious rebinds when the machine was idle. That turned out
    // to lock the watchdog out when the user was actively trying to use
    // a stuck trackpad without typing — exactly the case it most needed
    // to catch. The burst-since-rebind counter already proves we had
    // real activity, and a stray rebind on an idle machine costs nothing
    // visible, so the presence check is gone.
    let mut last_tp_event: Option<Instant> = None;
    let mut tp_burst_since_rebind: u32 = 0;
    let mut next_watchdog = Instant::now() + Duration::from_secs(1);
    const WATCHDOG_TICK: Duration = Duration::from_secs(1);
    // Silence threshold is intentionally long. A short threshold (we
    // started with 3s) is technically faster to react to a wedge, but
    // false-positives constantly during normal "user pauses typing /
    // pauses touchpad" patterns — every pause looked like a wedge, the
    // daemon rebound, and the user perceived the resulting blackouts as
    // jitter. The USB-bus presence check below is the primary signal;
    // this threshold just ensures we don't fire instantly on a
    // sub-second drop that the bus might recover from on its own.
    const WEDGE_SILENCE: Duration = Duration::from_secs(5);
    const WEDGE_COOLDOWN: Duration = Duration::from_secs(30);
    const WEDGE_BURST_MIN: u32 = 30;

    'main: loop {
        // Run periodic timers (watchdog + rescan) BEFORE reading the next
        // message, every iteration. Doing this only on recv_timeout's
        // Timeout branch was wrong: when keyboard events stream in
        // continuously the channel never goes quiet, so we'd never check
        // these deadlines and the wedge would sit unnoticed indefinitely.

        let now = Instant::now();

        // Rescan deadline?
        if next_rescan.map_or(false, |t| t <= now) {
            let new = try_acquire_missing(&mut active_touchpads, &tx);
            if new > 0 {
                eprintln!(
                    "trackpad-guard: rescan recovered {new} touchpad(s) \
                     (now holding {})",
                    active_touchpads.len()
                );
                next_rescan = None;
                rescan_backoff = RESCAN_INITIAL;
            } else {
                rescan_backoff = (rescan_backoff * 2).min(RESCAN_BACKOFF_MAX);
                next_rescan = Some(now + rescan_backoff);
            }
        }

        // Watchdog deadline?
        if next_watchdog <= now {
            next_watchdog = now + WATCHDOG_TICK;

            // Opportunistic rescan: pick up any AMIRA touchpad that
            // wasn't there at startup but is on the bus now. ONLY run
            // when we're short of the expected count — enumerating
            // /dev/input on every watchdog tick caused noticeable
            // stalls in cursor motion because the main thread was busy
            // opening/probing devices instead of forwarding events.
            if active_touchpads.len() < expected_touchpads {
                let _ = try_acquire_missing(&mut active_touchpads, &tx);
            }

            let silent_for = last_tp_event.map(|t| t.elapsed());
            let cooldown_ok = last_rebind.elapsed() > WEDGE_COOLDOWN;
            let active_recently = tp_burst_since_rebind >= WEDGE_BURST_MIN;
            let silent = silent_for.map_or(false, |d| d > WEDGE_SILENCE);
            // Primary truth signal: is the AMIRA touchpad currently
            // bound to the usb driver? If it's still on the bus, the
            // "silence" is the user just not touching the trackpad —
            // firing a rebind here causes a visible cursor blackout,
            // which the user perceives as jitter. If it's off the bus,
            // events truly cannot reach us and a rebind is the right
            // recovery.
            let bus_lost = !touchpad_is_bound_to_bus();
            if active_recently && silent && cooldown_ok && bus_lost {
                eprintln!(
                    "trackpad-guard: wedge watchdog — touchpad silent for \
                     {:?} after {} batches AND USB device off the bus, \
                     forcing rebind",
                    silent_for,
                    tp_burst_since_rebind,
                );
                rebind_touchpad_usb();
                last_rebind = Instant::now();
                tp_burst_since_rebind = 0;
            }
        }

        let now = Instant::now();
        let until_rescan = next_rescan
            .map(|t| t.saturating_duration_since(now))
            .unwrap_or(IDLE_TIMEOUT);
        let until_watchdog = next_watchdog.saturating_duration_since(now);
        let wait = until_rescan.min(until_watchdog);
        let msg = match rx.recv_timeout(wait) {
            Ok(m) => m,
            Err(mpsc::RecvTimeoutError::Timeout) => continue 'main,
            Err(mpsc::RecvTimeoutError::Disconnected) => break 'main,
        };
        match msg {
            Msg::KeyActivity { at } => {
                last_key_ts = Some(at);
                hb_key += 1;
            }
            Msg::TouchpadEvents { events, at } => {
                hb_tp += 1;
                last_tp_event = Some(at);
                tp_burst_since_rebind = tp_burst_since_rebind.saturating_add(1);
                // Compare the touchpad event's arrival time against the
                // last keyboard event's arrival time, NOT against now.
                // This stays correct even if the main loop is processing
                // a backlog — what matters is whether the kernel emitted
                // the touchpad event close in time to a real keystroke.
                let gap = last_key_ts.map(|t| at.saturating_duration_since(t));
                let typing = gap
                    .map(|g| g < KEY_TYPING_WINDOW)
                    .unwrap_or(false);

                // Log every gate-state transition with the gap in ms so
                // we can correlate user-reported "stuck" periods to what
                // the daemon was actually seeing.
                let became_dropping = typing && was_forwarding;
                let became_forwarding = !typing && !was_forwarding;
                if became_dropping {
                    eprintln!(
                        "trackpad-guard: gate ON (dropping) — gap-since-last-key={:?}",
                        gap
                    );
                    was_forwarding = false;
                } else if became_forwarding {
                    eprintln!(
                        "trackpad-guard: gate OFF (forwarding) — gap-since-last-key={:?}",
                        gap
                    );
                    was_forwarding = true;
                }

                if typing {
                    dropped_batches += 1;
                    hb_tp_drop += 1;
                } else if let Err(e) = virt.emit(&events) {
                    eprintln!("trackpad-guard: virtual emit failed: {e}");
                } else {
                    forwarded_batches += 1;
                    hb_tp_fwd += 1;
                }
            }
            Msg::ManualRebind => {
                if last_rebind.elapsed() < Duration::from_secs(5) {
                    eprintln!("trackpad-guard: SIGUSR1 ignored (rate-limited)");
                } else {
                    eprintln!("trackpad-guard: SIGUSR1 — manual USB rebind requested");
                    rebind_touchpad_usb();
                    last_rebind = Instant::now();
                    tp_burst_since_rebind = 0;
                }
            }
            Msg::KeyboardReaderDied => {
                keyboards_alive = keyboards_alive.saturating_sub(1);
                if keyboards_alive == 0 {
                    eprintln!("trackpad-guard: all keyboards disconnected, exiting");
                    break 'main;
                }
            }
            Msg::TouchpadReaderDied { path } => {
                // The reader thread saw fetch_events() return Err — almost
                // always ENODEV after the USB device dropped or its evdev
                // node got invalidated by a renumeration. Drop just this
                // path and schedule a rescan; the new node (often with the
                // same path) will be picked up within a few hundred ms.
                let removed = active_touchpads.remove(&path);
                eprintln!(
                    "trackpad-guard: reader for {} died (was tracked: {}), \
                     scheduling rescan",
                    path.display(),
                    removed
                );
                if next_rescan.is_none() {
                    rescan_backoff = RESCAN_INITIAL;
                    next_rescan = Some(Instant::now() + RESCAN_INITIAL);
                }
            }
            Msg::Shutdown => break 'main,
        }

        // Heartbeat: once HEARTBEAT has elapsed AND there's been any
        // activity, emit a one-liner summarizing the window. We only log
        // when something happened so an idle daemon doesn't spam logs.
        let elapsed = hb_last.elapsed();
        if elapsed >= HEARTBEAT && (hb_tp != 0 || hb_key != 0) {
            let last_key_ago = last_key_ts.map(|t| t.elapsed());
            eprintln!(
                "trackpad-guard: hb {}s — tp_batches={} (fwd={} drop={}) key_batches={} last_key_ago={:?}",
                elapsed.as_secs(),
                hb_tp,
                hb_tp_fwd,
                hb_tp_drop,
                hb_key,
                last_key_ago,
            );
            hb_last = Instant::now();
            hb_tp = 0;
            hb_tp_fwd = 0;
            hb_tp_drop = 0;
            hb_key = 0;
        }
    }

    eprintln!(
        "trackpad-guard: exiting (forwarded {} batches, dropped {})",
        forwarded_batches, dropped_batches
    );
}
