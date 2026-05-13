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
use std::collections::HashMap;
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
    TouchpadReaderDied,
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
                let mut had_key = false;
                for ev in events {
                    if matches!(ev.destructure(), EventSummary::Key(..)) {
                        had_key = true;
                    }
                }
                if had_key && tx.send(Msg::KeyActivity { at }).is_err() {
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

fn spawn_touchpad_reader(mut device: Device, tx: mpsc::Sender<Msg>) {
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
                let _ = tx.send(Msg::TouchpadReaderDied);
                return;
            }
        }
    });
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

fn rebind_touchpad_usb() {
    let id = match find_touchpad_usb_id() {
        Some(s) => s,
        None => {
            eprintln!("trackpad-guard: USB rebind aborted — AMIRA touchpad device not found");
            return;
        }
    };
    eprintln!("trackpad-guard: USB rebind on {id} (unbind + bind)");

    for path in &[
        "/sys/bus/usb/drivers/usb/unbind",
        "/sys/bus/usb/drivers/usb/bind",
    ] {
        let mut child = match Command::new("sudo")
            .arg("-n")
            .arg("tee")
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("trackpad-guard: USB rebind: failed to spawn sudo tee {path}: {e}");
                return;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(id.as_bytes());
            let _ = stdin.write_all(b"\n");
        }
        let _ = child.wait();
        if path.ends_with("unbind") {
            thread::sleep(Duration::from_millis(500));
        }
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
    touchpads = {
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
    };

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
    }

    eprintln!(
        "trackpad-guard: watching {} keyboard(s), {} real touchpad(s), \
         virtual device created (mirroring AMIRA capabilities)",
        keyboards.len(),
        touchpads.len()
    );

    let mut keyboards_alive = keyboards.len();
    let mut touchpads_alive = touchpads.len();
    for (_, device) in keyboards {
        spawn_keyboard_reader(device, tx.clone());
    }
    for (_, device) in touchpads {
        spawn_touchpad_reader(device, tx.clone());
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

    'main: loop {
        let msg = match rx.recv() {
            Ok(m) => m,
            Err(_) => break 'main,
        };
        match msg {
            Msg::KeyActivity { at } => {
                last_key_ts = Some(at);
                hb_key += 1;
            }
            Msg::TouchpadEvents { events, at } => {
                hb_tp += 1;
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
                }
            }
            Msg::KeyboardReaderDied => {
                keyboards_alive = keyboards_alive.saturating_sub(1);
                if keyboards_alive == 0 {
                    eprintln!("trackpad-guard: all keyboards disconnected, exiting");
                    break 'main;
                }
            }
            Msg::TouchpadReaderDied => {
                touchpads_alive = touchpads_alive.saturating_sub(1);
                if touchpads_alive == 0 {
                    eprintln!("trackpad-guard: all touchpads disconnected, exiting");
                    break 'main;
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
