//! system-dashboard — Tauri app that surfaces battery, power profile,
//! idle/screen-lock mode, current theme, and fan state. Read-side modules
//! poll the same sysfs/I2C/state files the existing waybar modules use;
//! click handlers shell out to the existing scripts (idle-toggle,
//! power-mode, switch-theme, argon-fan) so semantics stay consistent.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod brightness;
mod fan;
mod idle;
mod power;
mod theme;
mod trackpad;
mod volume;

use serde::Serialize;
use std::process::Command;
use tauri::{Emitter, Manager};

#[derive(Serialize, Clone, Debug)]
struct Snapshot {
    battery: battery::BatteryState,
    power: power::PowerState,
    idle: idle::IdleState,
    theme: theme::ThemeState,
    fan: fan::FanState,
    brightness: brightness::BrightnessState,
    volume: volume::VolumeState,
    trackpad: trackpad::TrackpadGuardState,
}

#[tauri::command]
fn snapshot() -> Snapshot {
    let battery = battery::read();
    let power = power::read(&battery);
    Snapshot {
        battery,
        power,
        idle: idle::read(),
        theme: theme::read(),
        fan: fan::read(),
        brightness: brightness::read(),
        volume: volume::read(),
        trackpad: trackpad::read(),
    }
}

#[tauri::command]
fn set_brightness(level: u8) -> Result<(), String> {
    brightness::set(level)
}

#[tauri::command]
fn set_volume(level: u8) -> Result<(), String> {
    volume::set(level)
}

#[tauri::command]
fn cycle_power() -> Result<(), String> {
    run_detached("power-mode", &["toggle"])
}

#[tauri::command]
fn cycle_idle() -> Result<(), String> {
    run_detached("idle-toggle", &["cycle"])
}

#[tauri::command]
fn open_theme_picker() -> Result<(), String> {
    run_detached("switch-theme", &["--picker"])
}

#[tauri::command]
fn open_fan_picker() -> Result<(), String> {
    run_detached("argon-fan", &["picker"])
}

#[tauri::command]
fn toggle_trackpad_guard() -> Result<(), String> {
    trackpad::toggle()
}

/// Detect whether we're running under a tiling window manager. Used to
/// suppress our CSD titlebar — tiling WMs already manage window placement
/// and the buttons just take up space.
///
/// We check each WM's signature IPC-socket env var first because
/// XDG_CURRENT_DESKTOP isn't reliably set on minimal sessions (e.g. greetd
/// → sway doesn't export it on some installs).
fn is_tiling_wm() -> bool {
    let env = |k: &str| std::env::var(k).is_ok();
    if env("SWAYSOCK") || env("HYPRLAND_INSTANCE_SIGNATURE") || env("NIRI_SOCKET") || env("I3SOCK")
    {
        return true;
    }
    match std::env::var("XDG_CURRENT_DESKTOP") {
        Ok(s) => {
            let s = s.to_lowercase();
            ["sway", "hyprland", "i3", "river", "niri", "wayfire"]
                .iter()
                .any(|w| s.contains(w))
        }
        Err(_) => false,
    }
}

fn run_detached(cmd: &str, args: &[&str]) -> Result<(), String> {
    Command::new(cmd)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{cmd} failed: {e}"))
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Hide decorations on tiling WMs — they handle window management
            // themselves, and a CSD titlebar with min/max buttons is just
            // visual noise. Stacking environments (GNOME/KDE/Cinnamon) keep
            // the decorations so users get familiar affordances.
            if is_tiling_wm() {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_decorations(false);
                }
            }

            let handle = app.handle().clone();
            std::thread::spawn(move || {
                use notify::{RecursiveMode, Watcher};
                let path = theme::stylesheet_watch_path();
                let (tx, rx) = std::sync::mpsc::channel();
                let mut watcher = match notify::recommended_watcher(tx) {
                    Ok(w) => w,
                    Err(_) => return,
                };
                if watcher
                    .watch(std::path::Path::new(&path), RecursiveMode::NonRecursive)
                    .is_err()
                {
                    return;
                }
                while rx.recv().is_ok() {
                    let _ = handle.emit("theme-stylesheet-changed", ());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            cycle_power,
            cycle_idle,
            open_theme_picker,
            open_fan_picker,
            toggle_trackpad_guard,
            set_brightness,
            set_volume
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
