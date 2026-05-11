//! Disable sway touchpad while typing.
//!
//! Works around DWT (disable-while-typing) not functioning on USB combo
//! keyboard/touchpad devices like the AMIRA in the Argon ONE UP case.
//! Watches all matching keyboard evdev nodes (the AMIRA exposes the same
//! vid:pid on two USB interfaces) and disables `type:touchpad` via swaymsg
//! while any non-modifier keys are pressed, re-enabling 100ms after they
//! are all released.
//!
//! Modifier-only holds (Super/Shift/Ctrl/Alt) do not disable the touchpad.
//! Workspace-switch chords like Mod+digit involve a non-modifier press, so
//! they still disable briefly, then re-enable cleanly when the digit is
//! released regardless of whether Mod-release is delivered.
//!
//! Autorepeat events (value=2) do not update timers, so a dropped release
//! event will be detected via the 2s stuck-state safety net and the
//! touchpad will be re-enabled.
//!
//! SIGUSR1 triggers a manual USB unbind/rebind on the AMIRA touchpad
//! interface (vid:pid 6080:8061). This is the user-explicit recovery if
//! the device ever appears stuck at the kernel level (sway shows it as
//! enabled but the cursor doesn't move). Sway is wired to send SIGUSR1 on
//! Mod+Shift+T.
//!
//! There is intentionally no auto-rebind heuristic — earlier versions
//! tried to detect stuck touchpads by watching for "keyboard active but
//! touchpad evdev silent for N seconds." That fired on false positives
//! (user just typing without using the trackpad), and the unnecessary
//! USB rebinds occasionally left the device in a bad state, *creating*
//! the stuck-trackpad symptom they were meant to recover from.

use evdev::{Device, EventSummary, KeyCode};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::collections::HashSet;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

const VENDOR_ID: u16 = 0x6080;
const KEYBOARD_NAME: &str = "AMIRA-KEYBOAR USB KEYBOARD";
// USB product ID of the AMIRA interface that the touchpad lives behind —
// used by the SIGUSR1-driven manual rebind path.
const TOUCHPAD_USB_PRODUCT: &str = "8061";
const TOUCHPAD_USB_VENDOR: &str = "6080";
// Note: the AMIRA exposes two USB interfaces with different PIDs (0x8060
// and 0x8061). We match keyboards on vendor + exact name so we catch
// both — name equality still excludes the Mouse/Touchpad/System Control/
// Consumer Control/Wireless Radio Control subsystem nodes that share the
// vid.

/// Grace window after the last key release before re-enabling the touchpad.
const GRACE: Duration = Duration::from_millis(100);

/// Keys we track in the pressed set but which never on their own disable
/// the touchpad. Holding Super to switch workspaces, Shift to select, Ctrl
/// for a shortcut, etc. isn't "typing" — no palm rest to guard against —
/// and the AMIRA drops release events on modifiers frequently enough that
/// treating modifier-only holds as typing locks the touchpad out for the
/// full STUCK_TIMEOUT every time a modifier press is mishandled.
fn is_modifier(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
    )
}

/// If any key is marked pressed but no events at all have arrived for this
/// long, assume a release was dropped and force-clear state.
const STUCK_TIMEOUT: Duration = Duration::from_secs(2);

/// Retry interval when no matching keyboards are found at startup.
const DISCOVER_INTERVAL: Duration = Duration::from_secs(2);

enum Msg {
    KeyEvent { key: KeyCode, value: i32 },
    KeyboardReaderDied,
    ManualRebind,
    Shutdown,
}

fn matches_keyboard(device: &Device) -> bool {
    device.input_id().vendor() == VENDOR_ID && device.name() == Some(KEYBOARD_NAME)
}

fn find_keyboards() -> Vec<(std::path::PathBuf, Device)> {
    evdev::enumerate()
        .filter(|(_, d)| matches_keyboard(d))
        .collect()
}

fn spawn_keyboard_reader(mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                for ev in events {
                    if let EventSummary::Key(_, key, value) = ev.destructure() {
                        if tx.send(Msg::KeyEvent { key, value }).is_err() {
                            return;
                        }
                    }
                }
            }
            Err(_) => {
                let _ = tx.send(Msg::KeyboardReaderDied);
                return;
            }
        }
    });
}

/// Find the AMIRA touchpad's USB device id (e.g. "1-1.6") by vid:pid.
fn find_touchpad_usb_id() -> Option<String> {
    for entry in std::fs::read_dir("/sys/bus/usb/devices/").ok()?.flatten() {
        let path = entry.path();
        let vendor = std::fs::read_to_string(path.join("idVendor"))
            .ok()
            .map(|s| s.trim().to_string());
        let product = std::fs::read_to_string(path.join("idProduct"))
            .ok()
            .map(|s| s.trim().to_string());
        if vendor.as_deref() == Some(TOUCHPAD_USB_VENDOR)
            && product.as_deref() == Some(TOUCHPAD_USB_PRODUCT)
        {
            return Some(entry.file_name().to_string_lossy().into_owned());
        }
    }
    None
}

/// USB unbind+rebind on the AMIRA touchpad-paired interface. Recovers the
/// "kernel evdev silent while sysfs reports active" state we keep hitting.
/// Requires a sudoers entry for `tee` on the unbind/bind sysfs files (the
/// installer already grants this).
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
            // Give the device a moment to fully detach before binding back.
            thread::sleep(Duration::from_millis(500));
        }
    }
    eprintln!("trackpad-guard: USB rebind complete");
}

/// Synchronously tell sway to enable/disable the touchpad. We *must* wait
/// for the child to exit, otherwise back-to-back disable/enable calls race
/// each other to the sway IPC socket — two parallel `swaymsg` processes
/// don't preserve issue order, and the `enabled` command can land at sway
/// before the `disabled` one. Result: sway sees `disabled` last and the
/// touchpad stays off, while our in-process state thinks we re-enabled it.
/// That was the "stuck → type → unstuck" symptom that's been plaguing us.
fn swaymsg(state: &'static str) {
    let status = Command::new("swaymsg")
        .args(["input", "type:touchpad", "events", state])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Err(e) = status {
        eprintln!("trackpad-guard: swaymsg {state} failed to run: {e}");
    } else if let Ok(s) = status {
        if !s.success() {
            eprintln!("trackpad-guard: swaymsg {state} exited {s}");
        }
    }
}

fn main() {
    let (tx, rx) = mpsc::channel::<Msg>();

    // Signal handler thread: SIGUSR1 → manual USB rebind request.
    // SIGTERM/SIGINT/SIGHUP → clean shutdown.
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

    // Initial keyboard discovery (retry until something shows up).
    let keyboards = loop {
        let found = find_keyboards();
        if !found.is_empty() {
            break found;
        }
        eprintln!("trackpad-guard: no matching keyboards found, retrying in 2s");
        thread::sleep(DISCOVER_INTERVAL);
    };

    eprintln!("trackpad-guard: watching {} keyboard(s)", keyboards.len());
    let mut keyboards_alive = keyboards.len();
    for (_, device) in keyboards {
        spawn_keyboard_reader(device, tx.clone());
    }

    // Make sure we start from a known-enabled state in case a previous run
    // crashed while the touchpad was disabled.
    swaymsg("enabled");

    let mut pressed: HashSet<KeyCode> = HashSet::new();
    let mut disabled = false;
    let mut last_release: Option<Instant> = None;
    // Time of the most recent press or release. Autorepeat (value=2) does
    // NOT update this — that's the whole point. When a release is dropped,
    // the kernel keeps firing autorepeats at ~33Hz, so we can't detect a
    // stuck key by "no events for a while"; we have to detect "only
    // autorepeats for a while."
    let mut last_transition = Instant::now();
    // Rate-limit floor for the SIGUSR1 manual rebind path.
    let mut last_rebind = Instant::now() - Duration::from_secs(60);
    let debug = std::env::var_os("TRACKPAD_GUARD_DEBUG").is_some();

    'main: loop {
        let timeout = if !disabled {
            // Block forever until a keyboard event or signal arrives.
            Duration::from_secs(3600)
        } else if !pressed.is_empty() {
            STUCK_TIMEOUT.saturating_sub(last_transition.elapsed()) + Duration::from_millis(1)
        } else if let Some(t) = last_release {
            GRACE.saturating_sub(t.elapsed()) + Duration::from_millis(1)
        } else {
            Duration::from_millis(50)
        };

        match rx.recv_timeout(timeout) {
            Ok(Msg::KeyEvent { key, value }) => match value {
                1 => {
                    pressed.insert(key);
                    last_transition = Instant::now();
                    last_release = None;
                    // Only non-modifiers trigger the disable. Modifiers
                    // stay in the `pressed` set (so "still typing"
                    // stays accurate while they're held) but never on
                    // their own flip the touchpad off.
                    if !disabled && !is_modifier(key) {
                        swaymsg("disabled");
                        disabled = true;
                        if debug {
                            eprintln!("trackpad-guard: disabled (press {key:?})");
                        }
                    }
                }
                0 => {
                    pressed.remove(&key);
                    last_transition = Instant::now();
                    // Start the grace timer when no non-modifier keys
                    // remain (typing stopped). Dangling modifier holds
                    // don't keep the touchpad off.
                    if pressed.iter().all(|k| is_modifier(*k)) {
                        last_release = Some(Instant::now());
                    }
                }
                // value=2 is autorepeat — the key is still logically
                // held. Deliberately NOT updating last_transition.
                _ => {}
            },
            Ok(Msg::KeyboardReaderDied) => {
                keyboards_alive = keyboards_alive.saturating_sub(1);
                if keyboards_alive == 0 {
                    eprintln!("trackpad-guard: all keyboards disconnected, exiting");
                    break 'main;
                }
            }
            Ok(Msg::ManualRebind) => {
                if last_rebind.elapsed() < Duration::from_secs(5) {
                    eprintln!("trackpad-guard: SIGUSR1 ignored (rate-limited)");
                } else {
                    eprintln!("trackpad-guard: SIGUSR1 — manual USB rebind requested");
                    rebind_touchpad_usb();
                    last_rebind = Instant::now();
                }
            }
            Ok(Msg::Shutdown) => break 'main,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break 'main,
        }

        if disabled {
            // Stuck-key safety net: if any key is marked pressed but we've
            // seen only autorepeats (no real press or release) for
            // STUCK_TIMEOUT, a release was dropped by the kernel/USB layer.
            // Force-clear and let the grace timer re-enable.
            if !pressed.is_empty() && last_transition.elapsed() >= STUCK_TIMEOUT {
                eprintln!(
                    "trackpad-guard: autorepeat-only for {:?}, clearing stuck state ({} key(s): {:?})",
                    STUCK_TIMEOUT,
                    pressed.len(),
                    pressed,
                );
                pressed.clear();
                last_release = Some(Instant::now());
            }

            // Grace period expired and no non-modifiers remain pressed —
            // re-enable. Held modifiers alone don't count as typing.
            if pressed.iter().all(|k| is_modifier(*k)) {
                if let Some(t) = last_release {
                    if t.elapsed() >= GRACE {
                        swaymsg("enabled");
                        disabled = false;
                        last_release = None;
                        if debug {
                            eprintln!("trackpad-guard: enabled (grace elapsed)");
                        }
                    }
                }
            }
        }
    }

    // Always re-enable on exit.
    swaymsg("enabled");
}
