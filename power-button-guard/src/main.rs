//! power-button-guard — make the power button ask before it kills the machine.
//!
//! Out of the box, systemd-logind's default `HandlePowerKey=poweroff` shuts
//! the machine down the instant the power button is tapped. On the Argon ONE
//! UP that is easy to hit by accident, and pressing it to *wake* from suspend
//! shuts the machine down instead of resuming it.
//!
//! This daemon takes the key over from logind (see the companion
//! `logind.conf.d` drop-in that sets `HandlePowerKey=ignore`) and instead:
//!
//!   * short press          -> themed power menu (bin/power-prompt)
//!   * short press while soft-suspended -> resume instead of showing the menu
//!   * held >= HOLD_FORCE   -> immediate power off, menu or no menu
//!   * press just after resume -> ignored (the wake-the-machine case)
//!
//! ## Hardware facts this is built on (measured 2026-08-04, RPi5 + AMIRA)
//!
//! The button is the gpio-keys device named `pwr_button`. The AMIRA keyboard
//! also exposes a "System Control" interface that *advertises* KEY_POWER, but
//! it never fires it -- captures across both devices showed every press on
//! `pwr_button` and nothing at all on the AMIRA node.
//!
//! The device is resolved BY NAME, never by event number: numbering on this
//! machine demonstrably shifts (trackpad-guard injects a virtual device, and
//! the AMIRA interfaces have renumbered across kernel updates).
//!
//! gpio-keys emits a clean press(1)/release(0) pair with real hold timing --
//! measured 122 ms for a tap and 4.98 s for a deliberate hold -- but it emits
//! NO autorepeat (value 2) while the button is down. That is why the hold path
//! below is a `recv_timeout` against a deadline rather than a blocking read:
//! nothing arrives to wake us mid-hold, so we must time out on purpose. A
//! plain blocking read loop would only ever notice the hold on release, which
//! from the user's side looks like the force-off doing nothing.
//!
//! logind's own long-press threshold is hardcoded at 5 s and cannot be
//! configured, which is why the 4 s spec needs this daemon at all. Acting at
//! 4 s also means we always fire before logind would.

use evdev::{Device, EventSummary, KeyCode};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

/// gpio-keys device name for the power button.
const BUTTON_NAME: &str = "pwr_button";

/// Hold this long and the machine powers off regardless of the prompt.
const HOLD_FORCE: Duration = Duration::from_millis(4000);

/// Presses this soon after a resume are swallowed. Pressing the power button
/// is how you wake the machine, and that same press gets delivered to
/// userspace once we are back -- without this, waking the machine would pop a
/// shutdown prompt (or worse, a hold-to-wake would force it off).
const RESUME_GRACE: Duration = Duration::from_millis(1500);

/// How often the resume watcher compares its clocks.
const RESUME_TICK: Duration = Duration::from_secs(1);

/// Wall-clock vs monotonic skew that counts as "we were suspended" rather
/// than NTP nudging the clock.
const SUSPEND_SKEW: Duration = Duration::from_secs(5);

enum Msg {
    Press(Instant),
    Release(Instant),
}

fn main() {
    let hold_force = env_duration("POWER_BUTTON_HOLD_MS", HOLD_FORCE);
    let resume_grace = env_duration("POWER_BUTTON_RESUME_GRACE_MS", RESUME_GRACE);

    let (path, device) = match find_power_button() {
        Some(found) => found,
        None => {
            eprintln!(
                "power-button-guard: no evdev device named {BUTTON_NAME:?}; \
                 candidates were: {:?}",
                evdev::enumerate()
                    .filter_map(|(_, d)| d.name().map(str::to_owned))
                    .collect::<Vec<_>>()
            );
            std::process::exit(1);
        }
    };
    eprintln!(
        "power-button-guard: watching {} ({BUTTON_NAME}), force-off at {:?}, resume grace {:?}",
        path.display(),
        hold_force,
        resume_grace
    );

    let (tx, rx) = mpsc::channel();
    spawn_button_reader(device, tx);

    let last_resume = Arc::new(Mutex::new(Instant::now()));
    spawn_resume_watcher(Arc::clone(&last_resume));

    // The currently-open prompt, if any. Kept so a second press does not
    // stack a second prompt on top of the first.
    let mut prompt: Option<Child> = None;

    while let Ok(msg) = rx.recv() {
        let Msg::Press(pressed_at) = msg else {
            // A release with no press we care about -- e.g. the release half
            // of a press we swallowed during the resume grace window.
            continue;
        };

        if within_resume_grace(&last_resume, pressed_at, resume_grace) {
            eprintln!("power-button-guard: press ignored (woke from suspend)");
            continue;
        }

        // Checked before the hold loop so the decision reflects the state at
        // press time, not after a possible 4s wait.
        let was_soft_suspended = soft_suspended();

        match wait_for_release_or_deadline(&rx, pressed_at, hold_force) {
            Outcome::Released(held) if was_soft_suspended => {
                eprintln!("power-button-guard: short press ({held:?}) -> resuming soft suspend");
                resume_soft_suspend();
            }
            Outcome::Released(held) => {
                eprintln!("power-button-guard: short press ({held:?}) -> prompt");
                show_prompt(&mut prompt);
            }
            Outcome::HeldPastDeadline => {
                eprintln!("power-button-guard: held >= {hold_force:?} -> forcing power off");
                if let Some(mut open) = prompt.take() {
                    kill_prompt(&mut open);
                }
                force_power_off();
                return;
            }
            Outcome::ReaderGone => return,
        }
    }
}

enum Outcome {
    Released(Duration),
    HeldPastDeadline,
    ReaderGone,
}

/// Wait for the button to come back up, or for the force-off deadline to pass.
///
/// gpio-keys is silent while the button is held, so the deadline has to be
/// enforced by timing out rather than by waiting for an event.
fn wait_for_release_or_deadline(
    rx: &mpsc::Receiver<Msg>,
    pressed_at: Instant,
    hold: Duration,
) -> Outcome {
    let deadline = pressed_at + hold;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Outcome::HeldPastDeadline;
        }
        match rx.recv_timeout(remaining) {
            Ok(Msg::Release(at)) => {
                return Outcome::Released(at.saturating_duration_since(pressed_at))
            }
            // Duplicate press without an intervening release: keep waiting on
            // the original deadline rather than restarting the clock, so a
            // stuttering contact cannot postpone the force-off forever.
            Ok(Msg::Press(_)) => continue,
            Err(RecvTimeoutError::Timeout) => return Outcome::HeldPastDeadline,
            Err(RecvTimeoutError::Disconnected) => return Outcome::ReaderGone,
        }
    }
}

/// Show the confirm prompt, unless one is already on screen.
///
/// The prompt is a separate script rather than an inlined `swaynag` call:
/// swaynag can only take flat colors as arguments -- no CSS, no border radius,
/// no Nerd Font -- so it could not be made to match the rest of the desktop.
/// `power-prompt` uses wofi, which shares its stylesheet pipeline with waybar
/// through switch-theme and therefore re-themes automatically.
fn show_prompt(prompt: &mut Option<Child>) {
    if let Some(open) = prompt {
        match open.try_wait() {
            // Still up -- leave it alone rather than stacking another.
            Ok(None) => return,
            _ => *prompt = None,
        }
    }

    // New process group so the force-off path can take down the whole prompt,
    // script plus the wofi it spawned. Killing just the script would orphan
    // wofi on screen over a machine that is powering off.
    match Command::new(prompt_command()).process_group(0).spawn() {
        Ok(child) => *prompt = Some(child),
        Err(e) => eprintln!("power-button-guard: could not launch the power prompt: {e}"),
    }
}

/// Is the desktop in the menu-initiated "pretend suspend" state?
///
/// The Pi 5 cannot really suspend, so `bin/soft-suspend` powers subsystems down
/// individually. A lid close is woken by the lid GPIO's rising edge, but a
/// menu-initiated suspend has no such edge coming -- so it drops this flag and
/// the physical power button becomes the wake trigger, which is what a power
/// button should do on a sleeping machine.
///
/// The flag lives in XDG_RUNTIME_DIR (tmpfs) so a crash or hard power-off
/// cannot strand us in resume-only mode across a boot.
fn soft_suspended() -> bool {
    soft_suspend_flag().is_some_and(|p| p.exists())
}

fn soft_suspend_flag() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("soft-suspended"))
}

/// Wake from soft suspend by running the same script a lid open runs. That
/// script also clears the flag, so lid-resume and button-resume agree.
fn resume_soft_suspend() {
    let script = match std::env::var("HOME") {
        Ok(home) => format!("{home}/.local/bin/lid-suspend"),
        Err(_) => "lid-suspend".to_string(),
    };
    if let Err(e) = Command::new(script).arg("open").spawn() {
        eprintln!("power-button-guard: could not run lid-suspend open: {e}");
        // Leave the flag in place: without a successful resume the desktop is
        // still down, and clearing it would turn the next press into a menu
        // nobody can see.
    }
}

/// Path to the prompt script. Overridable for testing; defaults to the copy
/// `install.sh` drops in `~/.local/bin`.
fn prompt_command() -> String {
    if let Ok(cmd) = std::env::var("POWER_BUTTON_PROMPT_CMD") {
        return cmd;
    }
    match std::env::var("HOME") {
        Ok(home) => format!("{home}/.local/bin/power-prompt"),
        Err(_) => "power-prompt".to_string(),
    }
}

/// Tear down an open prompt and everything it spawned.
fn kill_prompt(child: &mut Child) {
    // Negative PID targets the process group created in show_prompt, so wofi
    // dies with its parent script rather than being left on screen.
    let group = format!("-{}", child.id());
    let killed_group = Command::new("kill")
        .args(["-TERM", &group])
        .status()
        .is_ok_and(|s| s.success());
    if !killed_group {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// Power off now. `-i` so a stray inhibitor cannot veto an explicit 4-second
/// hold; falls back to the plain call if policy refuses the ignore-inhibitors
/// variant.
fn force_power_off() {
    let forced = Command::new("systemctl").args(["poweroff", "-i"]).status();
    match forced {
        Ok(status) if status.success() => {}
        other => {
            eprintln!("power-button-guard: `systemctl poweroff -i` failed ({other:?}), retrying without -i");
            if let Err(e) = Command::new("systemctl").arg("poweroff").status() {
                eprintln!("power-button-guard: power off failed: {e}");
            }
        }
    }
}

fn within_resume_grace(
    last_resume: &Arc<Mutex<Instant>>,
    pressed_at: Instant,
    grace: Duration,
) -> bool {
    let resumed_at = *last_resume.lock().expect("resume clock poisoned");
    pressed_at.saturating_duration_since(resumed_at) < grace
}

fn spawn_button_reader(mut device: Device, tx: mpsc::Sender<Msg>) {
    thread::spawn(move || loop {
        match device.fetch_events() {
            Ok(events) => {
                let at = Instant::now();
                for ev in events {
                    let EventSummary::Key(_, KeyCode::KEY_POWER, value) = ev.destructure() else {
                        continue;
                    };
                    // value 2 is autorepeat; gpio-keys does not send it for
                    // this button, but ignore it rather than treat it as a
                    // fresh press if a future kernel starts to.
                    let msg = match value {
                        1 => Msg::Press(at),
                        0 => Msg::Release(at),
                        _ => continue,
                    };
                    if tx.send(msg).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("power-button-guard: read error on power button: {e}");
                return;
            }
        }
    });
}

/// Detect resume from suspend without pulling in a D-Bus dependency.
///
/// CLOCK_MONOTONIC (what `Instant` uses on Linux) stops while the machine is
/// suspended; the wall clock does not. So a tick where wall time advanced far
/// more than monotonic time means we just came back from suspend.
fn spawn_resume_watcher(last_resume: Arc<Mutex<Instant>>) {
    thread::spawn(move || {
        let mut prev_mono = Instant::now();
        let mut prev_wall = SystemTime::now();
        loop {
            thread::sleep(RESUME_TICK);
            let mono = Instant::now();
            let wall = SystemTime::now();
            let mono_delta = mono.saturating_duration_since(prev_mono);
            let wall_delta = wall.duration_since(prev_wall).unwrap_or(mono_delta);

            if wall_delta > mono_delta + SUSPEND_SKEW {
                eprintln!(
                    "power-button-guard: resumed (slept ~{}s), ignoring the wake press",
                    (wall_delta - mono_delta).as_secs()
                );
                *last_resume.lock().expect("resume clock poisoned") = mono;
            }
            prev_mono = mono;
            prev_wall = wall;
        }
    });
}

fn find_power_button() -> Option<(PathBuf, Device)> {
    evdev::enumerate().find(|(_, d)| {
        d.name() == Some(BUTTON_NAME)
            && d.supported_keys()
                .is_some_and(|keys| keys.contains(KeyCode::KEY_POWER))
    })
}

fn env_duration(var: &str, default: Duration) -> Duration {
    match std::env::var(var) {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(ms) => Duration::from_millis(ms),
            Err(_) => {
                eprintln!("power-button-guard: ignoring bad {var}={raw:?}");
                default
            }
        },
        Err(_) => default,
    }
}
