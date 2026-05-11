//! argon-fan — PWM fan control daemon for the Argon ONE UP.
//!
//! Subcommands: daemon | set <mode> | status | picker | edit-config | waybar.
//! Single binary; the daemon long-runs, the others are short-lived CLI calls.

use serde::{Deserialize, Serialize};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::flag;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type AnyResult<T> = std::result::Result<T, Box<dyn Error>>;

const STATE_FILE: &str = "/dev/shm/argon-fan.json";
const TEMP_PATH: &str = "/sys/class/thermal/thermal_zone0/temp";
const HWMON_DIR: &str = "/sys/class/hwmon";
const PWMFAN_NAME: &str = "pwmfan";
const CONFIG_PATH: &str = "/etc/argon-fan/config.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CurvePoint {
    temp_c: f32,
    pwm: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModeCurve {
    curve: Vec<CurvePoint>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Config {
    active_mode: String,
    poll_interval_sec: u64,
    modes: BTreeMap<String, ModeCurve>,
}

impl Default for Config {
    fn default() -> Self {
        let pt = |temp_c: f32, pwm: u32| CurvePoint { temp_c, pwm };
        let curve = |points: Vec<CurvePoint>| ModeCurve { curve: points };

        let mut modes = BTreeMap::new();

        // silent: prioritize quietness; fan stays off until the chip is
        // genuinely hot, then ramps confidently past the fan-stall floor.
        // Pi 5 self-throttles at 80°C — silent mode trusts that ceiling.
        modes.insert(
            "silent".into(),
            curve(vec![pt(65.0, 0), pt(68.0, 80), pt(85.0, 230)]),
        );

        // normal: balanced daily-use curve targeting <60°C under typical
        // load. Fan reaches max at 58°C, two degrees of buffer before the
        // 60°C target.
        modes.insert(
            "normal".into(),
            curve(vec![
                pt(45.0, 0),
                pt(48.0, 100),
                pt(55.0, 200),
                pt(58.0, 250),
            ]),
        );

        // turbo: aggressive cooling for sustained heavy load, targeting
        // <50°C. Fan reaches max at 48°C — same two-degree buffer.
        modes.insert(
            "turbo".into(),
            curve(vec![
                pt(38.0, 0),
                pt(40.0, 120),
                pt(45.0, 220),
                pt(48.0, 250),
            ]),
        );

        // full: pinned at 250 (the kernel device-tree's chosen cap).
        // Going to 255 doesn't increase airflow on this fan, only noise.
        modes.insert("full".into(), curve(vec![pt(0.0, 250)]));

        Self {
            active_mode: "normal".into(),
            poll_interval_sec: 2,
            modes,
        }
    }
}

impl Config {
    fn pwm_for(&self, temp_c: f32) -> u32 {
        let curve = match self.modes.get(&self.active_mode) {
            Some(m) => &m.curve,
            None => return 0,
        };
        if curve.is_empty() {
            return 0;
        }
        if temp_c <= curve[0].temp_c {
            return curve[0].pwm;
        }
        let last = curve.last().unwrap();
        if temp_c >= last.temp_c {
            return last.pwm;
        }
        for w in curve.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if temp_c >= a.temp_c && temp_c <= b.temp_c {
                let span = b.temp_c - a.temp_c;
                if span <= 0.0 {
                    return a.pwm;
                }
                let t = (temp_c - a.temp_c) / span;
                let lerp = a.pwm as f32 + t * (b.pwm as f32 - a.pwm as f32);
                return lerp.round().clamp(0.0, 255.0) as u32;
            }
        }
        0
    }
}

#[derive(Serialize, Deserialize)]
struct State {
    active_mode: String,
    pwm: u32,
    pwm_pct: u32,
    temp_c: f32,
    timestamp: u64,
}

fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_PATH)
}

// Direct write — works as root (the daemon).
fn write_config_direct(p: &Path, body: &str) -> AnyResult<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, body)?;
    fs::rename(&tmp, p)?;
    Ok(())
}

// Fallback for non-root callers: pipe through `sudo -n /usr/bin/tee`. The
// installer drops a sudoers rule that allows this exact invocation NOPASSWD.
fn write_config_via_sudo(p: &Path, body: &str) -> AnyResult<()> {
    let path_str = p.to_str().ok_or("config path not utf8")?;
    let mut child = Command::new("sudo")
        .args(["-n", "/usr/bin/tee", path_str])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or("failed to open sudo stdin")?
        .write_all(body.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(format!("sudo tee {path_str} failed (exit {status})").into());
    }
    Ok(())
}

fn load_or_seed_config() -> AnyResult<Config> {
    let p = config_path();
    if p.exists() {
        let body = fs::read_to_string(&p)?;
        let cfg: Config = serde_json::from_str(&body)?;
        return Ok(cfg);
    }
    let cfg = Config::default();
    save_config(&cfg)?;
    Ok(cfg)
}

fn save_config(cfg: &Config) -> AnyResult<()> {
    let p = config_path();
    let body = serde_json::to_string_pretty(cfg)?;
    match write_config_direct(&p, &body) {
        Ok(()) => Ok(()),
        Err(_) => write_config_via_sudo(&p, &body),
    }
}

fn find_pwmfan_hwmon() -> AnyResult<PathBuf> {
    for entry in fs::read_dir(HWMON_DIR)? {
        let p = entry?.path();
        if let Ok(name) = fs::read_to_string(p.join("name")) {
            if name.trim() == PWMFAN_NAME {
                return Ok(p);
            }
        }
    }
    Err(format!("no hwmon entry with name '{PWMFAN_NAME}'").into())
}

fn read_temp() -> AnyResult<f32> {
    let s = fs::read_to_string(TEMP_PATH)?;
    let m: i64 = s.trim().parse()?;
    Ok(m as f32 / 1000.0)
}

fn write_pwm(hwmon: &Path, pwm: u32) -> AnyResult<()> {
    fs::write(hwmon.join("pwm1"), pwm.to_string())?;
    Ok(())
}

fn set_pwm_enable(hwmon: &Path, mode: u8) -> AnyResult<()> {
    fs::write(hwmon.join("pwm1_enable"), mode.to_string())?;
    Ok(())
}

fn write_state(s: &State) -> AnyResult<()> {
    let body = serde_json::to_string(s)?;
    let tmp = format!("{STATE_FILE}.tmp");
    fs::write(&tmp, body)?;
    fs::rename(&tmp, STATE_FILE)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cmd_daemon() -> AnyResult<()> {
    let mut cfg = load_or_seed_config()?;
    let cfg_path = config_path();
    // Track mtime so we reload after a `set` or `edit-config` from the user CLI.
    // We can't rely on signals — non-root callers can't signal the root daemon.
    let mut last_mtime = fs::metadata(&cfg_path).and_then(|m| m.modified()).ok();
    let hwmon = find_pwmfan_hwmon()?;
    eprintln!("argon-fan: hwmon at {}", hwmon.display());

    // Take over from the kernel governor; the loop now owns pwm1.
    set_pwm_enable(&hwmon, 1)?;

    let term = Arc::new(AtomicBool::new(false));
    flag::register(SIGTERM, Arc::clone(&term))?;
    flag::register(SIGINT, Arc::clone(&term))?;

    while !term.load(Ordering::Relaxed) {
        let cur_mtime = fs::metadata(&cfg_path).and_then(|m| m.modified()).ok();
        if cur_mtime != last_mtime {
            match load_or_seed_config() {
                Ok(c) => {
                    cfg = c;
                    last_mtime = cur_mtime;
                    eprintln!(
                        "argon-fan: config reloaded; active_mode={}",
                        cfg.active_mode
                    );
                }
                Err(e) => eprintln!("argon-fan: config reload failed: {e}"),
            }
        }
        let temp = read_temp()?;
        let pwm = cfg.pwm_for(temp);
        write_pwm(&hwmon, pwm)?;
        let _ = write_state(&State {
            active_mode: cfg.active_mode.clone(),
            pwm,
            pwm_pct: ((pwm as u64 * 100) / 255) as u32,
            temp_c: temp,
            timestamp: now_secs(),
        });
        sleep(Duration::from_secs(cfg.poll_interval_sec.max(1)));
    }

    // Hand the fan back to the kernel governor on clean shutdown.
    let _ = set_pwm_enable(&hwmon, 2);
    eprintln!("argon-fan: daemon exiting; pwm1_enable restored to 2");
    Ok(())
}

fn cmd_set(mode: &str) -> AnyResult<()> {
    let mut cfg = load_or_seed_config()?;
    if !cfg.modes.contains_key(mode) {
        let known: Vec<&String> = cfg.modes.keys().collect();
        eprintln!("argon-fan: unknown mode '{mode}'; known: {known:?}");
        exit(2);
    }
    cfg.active_mode = mode.to_string();
    save_config(&cfg)?;
    println!("active_mode = {mode}");
    Ok(())
}

fn cmd_status() -> AnyResult<()> {
    if let Ok(body) = fs::read_to_string(STATE_FILE) {
        print!("{body}");
        if !body.ends_with('\n') {
            println!();
        }
    } else {
        let cfg = load_or_seed_config()?;
        println!("active_mode = {} (daemon not running)", cfg.active_mode);
    }
    Ok(())
}

fn cmd_picker() -> AnyResult<()> {
    let cfg = load_or_seed_config()?;
    let mut input = String::new();
    for k in cfg.modes.keys() {
        if k == &cfg.active_mode {
            input.push_str(&format!("{k} (current)\n"));
        } else {
            input.push_str(k);
            input.push('\n');
        }
    }
    let mut child = Command::new("wofi")
        .args([
            "--dmenu", "--prompt", "Fan mode", "--width", "300", "--height", "250",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    {
        let mut stdin = child.stdin.take().ok_or("failed to open wofi stdin")?;
        stdin.write_all(input.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let chosen = String::from_utf8_lossy(&out.stdout)
        .trim()
        .replace(" (current)", "");
    if !chosen.is_empty() {
        cmd_set(&chosen)?;
    }
    Ok(())
}

// Best-effort desktop notification. We use this when the CLI is invoked
// from waybar (right-click → edit-config) and there's no TTY for the
// caller to see stderr — without this, a JSON validation failure is
// silent.
fn notify(summary: &str, body: &str) {
    let _ = Command::new("notify-send")
        .args(["-t", "8000", summary, body])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn cmd_edit_config() -> AnyResult<()> {
    let p = config_path();
    if !p.exists() {
        load_or_seed_config()?;
    }
    let body = fs::read_to_string(&p)?;
    let scratch = std::env::temp_dir().join("argon-fan-edit.json");
    fs::write(&scratch, &body)?;

    let term = std::env::var("TERMINAL").unwrap_or_else(|_| "foot".into());
    let scratch_str = scratch.to_str().ok_or("scratch path not utf8")?;
    let status = Command::new(&term)
        .args(["-e", "micro", scratch_str])
        .status()?;
    if !status.success() {
        let _ = fs::remove_file(&scratch);
        return Err(format!("editor exited with {status}").into());
    }

    let new_body = fs::read_to_string(&scratch)?;
    if new_body == body {
        let _ = fs::remove_file(&scratch);
        eprintln!("argon-fan: config unchanged");
        return Ok(());
    }
    if let Err(e) = serde_json::from_str::<Config>(&new_body) {
        let _ = fs::remove_file(&scratch);
        let msg = format!("invalid config (not saved): {e}");
        notify("argon-fan: config not saved", &e.to_string());
        return Err(msg.into());
    }

    match write_config_direct(&p, &new_body) {
        Ok(()) => {}
        Err(_) => write_config_via_sudo(&p, &new_body)?,
    }
    let _ = fs::remove_file(&scratch);
    println!("argon-fan: config saved");
    Ok(())
}

fn mode_icon(mode: &str) -> &'static str {
    match mode {
        "silent" => "\u{F0A9F}",
        "normal" => "\u{F0210}",
        "turbo" => "\u{F01FB}",
        "full" => "\u{F1A88}",
        _ => "\u{F020E}",
    }
}

fn mode_label(mode: &str) -> String {
    let mut chars = mode.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn cmd_waybar() -> AnyResult<()> {
    let body = match fs::read_to_string(STATE_FILE) {
        Ok(b) => b,
        Err(_) => {
            let out = serde_json::json!({
                "text": format!("{} ?", "\u{F020E}"),
                "tooltip": "argon-fan daemon not running",
                "class": "offline",
            });
            println!("{out}");
            return Ok(());
        }
    };
    let state: State = serde_json::from_str(&body)?;
    let icon = mode_icon(&state.active_mode);
    let out = serde_json::json!({
        "text": format!("{icon} {:.0}°C", state.temp_c),
        "tooltip": format!(
            "Mode: {}\nPWM: {}/255 ({}%)\nTemp: {:.0}°C",
            mode_label(&state.active_mode),
            state.pwm,
            state.pwm_pct,
            state.temp_c
        ),
        "class": state.active_mode,
    });
    println!("{out}");
    Ok(())
}

fn usage() {
    eprintln!(
        "argon-fan — PWM fan control for Argon ONE UP\n\
         \n\
         USAGE:\n\
         \x20  argon-fan daemon         run the control loop\n\
         \x20  argon-fan set <mode>     change active mode\n\
         \x20  argon-fan status         print current state\n\
         \x20  argon-fan picker         wofi mode picker\n\
         \x20  argon-fan edit-config    open config in micro\n\
         \x20  argon-fan waybar         emit waybar JSON"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result: AnyResult<()> = match args.get(1).map(String::as_str) {
        Some("daemon") => cmd_daemon(),
        Some("set") => match args.get(2) {
            Some(m) => cmd_set(m),
            None => {
                usage();
                exit(2);
            }
        },
        Some("status") => cmd_status(),
        Some("picker") => cmd_picker(),
        Some("edit-config") => cmd_edit_config(),
        Some("waybar") => cmd_waybar(),
        _ => {
            usage();
            exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("argon-fan: {e}");
        exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with(mode: &str, curve: Vec<CurvePoint>) -> Config {
        let mut modes = BTreeMap::new();
        modes.insert(mode.into(), ModeCurve { curve });
        Config {
            active_mode: mode.into(),
            poll_interval_sec: 2,
            modes,
        }
    }

    #[test]
    fn pwm_for_temp_below_first_point_returns_first_pwm() {
        let c = cfg_with(
            "m",
            vec![
                CurvePoint {
                    temp_c: 50.0,
                    pwm: 0,
                },
                CurvePoint {
                    temp_c: 75.0,
                    pwm: 255,
                },
            ],
        );
        assert_eq!(c.pwm_for(20.0), 0);
        assert_eq!(c.pwm_for(50.0), 0);
    }

    #[test]
    fn pwm_for_temp_above_last_point_returns_last_pwm() {
        let c = cfg_with(
            "m",
            vec![
                CurvePoint {
                    temp_c: 50.0,
                    pwm: 0,
                },
                CurvePoint {
                    temp_c: 75.0,
                    pwm: 255,
                },
            ],
        );
        assert_eq!(c.pwm_for(75.0), 255);
        assert_eq!(c.pwm_for(150.0), 255);
    }

    #[test]
    fn pwm_for_lerp_midpoint() {
        let c = cfg_with(
            "m",
            vec![
                CurvePoint {
                    temp_c: 50.0,
                    pwm: 0,
                },
                CurvePoint {
                    temp_c: 75.0,
                    pwm: 250,
                },
            ],
        );
        // At 62.5°C (midpoint), pwm should be 125.
        assert_eq!(c.pwm_for(62.5), 125);
        // At 60°C, ((60-50)/25)*250 = 100.
        assert_eq!(c.pwm_for(60.0), 100);
    }

    #[test]
    fn pwm_for_unknown_active_mode_returns_zero() {
        let c = Config {
            active_mode: "bogus".into(),
            ..Config::default()
        };
        assert_eq!(c.pwm_for(60.0), 0);
    }

    #[test]
    fn pwm_for_empty_curve_returns_zero() {
        let c = cfg_with("m", vec![]);
        assert_eq!(c.pwm_for(60.0), 0);
    }

    #[test]
    fn pwm_for_single_point_curve_returns_that_pwm() {
        let c = cfg_with(
            "m",
            vec![CurvePoint {
                temp_c: 0.0,
                pwm: 255,
            }],
        );
        assert_eq!(c.pwm_for(20.0), 255);
        assert_eq!(c.pwm_for(80.0), 255);
    }

    #[test]
    fn pwm_for_clamps_to_255_at_max() {
        // The max curve point is 255 itself; ensure no overflow / wraparound.
        let c = cfg_with(
            "m",
            vec![
                CurvePoint {
                    temp_c: 50.0,
                    pwm: 0,
                },
                CurvePoint {
                    temp_c: 75.0,
                    pwm: 255,
                },
            ],
        );
        assert_eq!(c.pwm_for(74.99), 255);
    }

    #[test]
    fn default_config_has_four_modes() {
        let c = Config::default();
        for m in ["silent", "normal", "turbo", "full"] {
            assert!(c.modes.contains_key(m), "missing mode {m}");
        }
        assert_eq!(c.active_mode, "normal");
        assert!(c.poll_interval_sec >= 1);
    }

    #[test]
    fn default_full_mode_is_always_max_pwm() {
        // `full` is pinned at 250 (the device-tree's chosen cap — going
        // higher doesn't increase airflow, only noise).
        let c = Config {
            active_mode: "full".into(),
            ..Config::default()
        };
        assert_eq!(c.pwm_for(0.0), 250);
        assert_eq!(c.pwm_for(40.0), 250);
        assert_eq!(c.pwm_for(80.0), 250);
    }

    #[test]
    fn default_silent_mode_is_off_below_threshold() {
        let c = Config {
            active_mode: "silent".into(),
            ..Config::default()
        };
        // silent stays off until 65°C — generous quiet zone.
        assert_eq!(c.pwm_for(40.0), 0);
        assert_eq!(c.pwm_for(60.0), 0);
        assert_eq!(c.pwm_for(65.0), 0);
        assert!(c.pwm_for(85.0) > 0);
    }

    #[test]
    fn mode_icon_each_known_mode_is_unique() {
        let icons = [
            mode_icon("silent"),
            mode_icon("normal"),
            mode_icon("turbo"),
            mode_icon("full"),
        ];
        for (i, a) in icons.iter().enumerate() {
            for b in icons.iter().skip(i + 1) {
                assert_ne!(a, b, "duplicate icon: {a}");
            }
        }
    }

    #[test]
    fn mode_icon_unknown_returns_offline_glyph() {
        assert_eq!(mode_icon("bogus"), "\u{F020E}");
        assert_eq!(mode_icon(""), "\u{F020E}");
    }

    #[test]
    fn mode_label_capitalizes_first_char() {
        assert_eq!(mode_label("turbo"), "Turbo");
        assert_eq!(mode_label("silent"), "Silent");
        assert_eq!(mode_label("a"), "A");
    }

    #[test]
    fn mode_label_empty_string_returns_empty() {
        assert_eq!(mode_label(""), "");
    }

    #[test]
    fn config_round_trips_through_json() {
        let c = Config::default();
        let s = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(back.active_mode, c.active_mode);
        assert_eq!(back.poll_interval_sec, c.poll_interval_sec);
        assert_eq!(back.modes.len(), c.modes.len());
    }
}
