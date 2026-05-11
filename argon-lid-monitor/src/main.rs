//! Argon ONE UP lid monitor.
//!
//! Watches GPIO line 27 on `/dev/gpiochip0` for both edges and invokes
//! `~/.local/bin/lid-suspend close` / `open` to drive our power-savings
//! drop-ins. Replaces the `argonpowerbutton_monitorlid` thread in
//! Argon's `argononeupd.py` — the lid GPIO isn't exposed via standard
//! Linux ACPI, so this is the only path to react to lid motion.

use gpiocdev::line::{Bias, EdgeDetection, EdgeKind};
use gpiocdev::Request;
use std::process::Command;
use std::time::Duration;

const GPIO_CHIP: &str = "/dev/gpiochip0";
const LID_LINE: u32 = 27;
/// Ignore subsequent edges for this window after firing — cheap debounce
/// in case the hall-effect sensor bounces across the threshold.
const DEBOUNCE: Duration = Duration::from_millis(100);

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

    loop {
        match req.read_edge_event() {
            Ok(event) => {
                let action = match event.kind {
                    EdgeKind::Falling => "close",
                    EdgeKind::Rising => "open",
                };
                run_lid_suspend(action);
                std::thread::sleep(DEBOUNCE);
            }
            Err(e) => {
                eprintln!("argon-lid-monitor: edge read error: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}
