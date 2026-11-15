//! Argon ONE UP lid monitor.
//!
//! Watches GPIO line 27 on `/dev/gpiochip0` for both edges and invokes
//! `~/.local/bin/lid-suspend close` / `open` to drive our power-savings
//! drop-ins. Replaces the `argonpowerbutton_monitorlid` thread in
//! Argon's `argononeupd.py` — the lid GPIO isn't exposed via standard
//! Linux ACPI, so this is the only path to react to lid motion.

use gpiocdev::line::{Bias, EdgeDetection, EdgeKind, Value};
use gpiocdev::Request;
use std::process::Command;
use std::time::Duration;

const GPIO_CHIP: &str = "/dev/gpiochip0";
const LID_LINE: u32 = 27;
/// After an edge, wait this long for the hall-effect sensor to stop bouncing
/// across its threshold, then act on where the line actually settled.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// The lid-suspend action for a line level: the sensor pulls the line low
/// when the lid is closed (a falling edge is a close).
fn action_for_level(level: Value) -> &'static str {
    match level {
        Value::Inactive => "close",
        Value::Active => "open",
    }
}

/// Whether to run lid-suspend for a settled lid position. Skips a repeat of
/// the action that ran last, so a bounce that ends where it started (or edges
/// that queued up while lid-suspend was running) does nothing.
fn should_run(last: Option<&'static str>, settled: &'static str) -> bool {
    last != Some(settled)
}

fn run_lid_suspend(action: &str) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            eprintln!("argon-lid-monitor: HOME not set");
            return;
        }
    };
    let path = format!("{home}/.local/bin/lid-suspend");
    eprintln!("argon-lid-monitor: lid {action} → {path}");
    match Command::new(&path).arg(action).status() {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("argon-lid-monitor: lid-suspend {action} exited {s}"),
        Err(e) => eprintln!("argon-lid-monitor: failed to run lid-suspend: {e}"),
    }
}

fn main() {
    let req = match Request::builder()
        .on_chip(GPIO_CHIP)
        .with_line(LID_LINE)
        .as_input()
        .with_bias(Bias::PullUp)
        .with_edge_detection(EdgeDetection::BothEdges)
        .with_consumer("argon-lid-monitor")
        .request()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("argon-lid-monitor: GPIO request failed: {e}");
            std::process::exit(1);
        }
    };

    eprintln!("argon-lid-monitor: watching {GPIO_CHIP} line {LID_LINE}");

    // The last action handed to lid-suspend. None until the first edge, which
    // therefore always runs.
    let mut last_action: Option<&'static str> = None;

    loop {
        match req.read_edge_event() {
            Ok(event) => {
                // Let the sensor settle, then throw away every edge that
                // arrived meanwhile. The kernel queues them; without the drain
                // a chattering close replayed as close/open/close, each one a
                // full lid-suspend run.
                std::thread::sleep(DEBOUNCE);
                while matches!(req.has_edge_event(), Ok(true)) {
                    if req.read_edge_event().is_err() {
                        break;
                    }
                }
                // Act on the level the line settled at, not on the edge that
                // woke us: after a bounce the two can differ. Fall back to the
                // edge if the level cannot be read.
                let action = match req.value(LID_LINE) {
                    Ok(level) => action_for_level(level),
                    Err(_) => match event.kind {
                        EdgeKind::Falling => "close",
                        EdgeKind::Rising => "open",
                    },
                };
                if should_run(last_action, action) {
                    run_lid_suspend(action);
                    last_action = Some(action);
                }
            }
            Err(e) => {
                eprintln!("argon-lid-monitor: edge read error: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_maps_to_action() {
        assert_eq!(action_for_level(Value::Inactive), "close");
        assert_eq!(action_for_level(Value::Active), "open");
    }

    #[test]
    fn a_repeat_of_the_last_action_is_skipped() {
        assert!(should_run(None, "close"));
        assert!(should_run(Some("open"), "close"));
        assert!(!should_run(Some("close"), "close"));
    }
}
