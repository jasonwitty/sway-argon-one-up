//! system-dashboard — Tauri app that surfaces battery, power profile,
//! idle/screen-lock mode, current theme, and fan state. Read-side modules
//! poll the same sysfs/I2C/state files the existing waybar modules use;
//! click handlers shell out to the existing scripts (idle-toggle,
//! power-mode, switch-theme, argon-fan) so semantics stay consistent.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod battery;
mod fan;
mod idle;
mod power;
mod theme;

use serde::Serialize;
use std::process::Command;
use tauri::Emitter;

#[derive(Serialize, Clone, Debug)]
struct Snapshot {
    battery: battery::BatteryState,
    power: power::PowerState,
    idle: idle::IdleState,
    theme: theme::ThemeState,
    fan: fan::FanState,
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
    }
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

fn run_detached(cmd: &str, args: &[&str]) -> Result<(), String> {
    Command::new(cmd)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{cmd} failed: {e}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
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
            open_fan_picker
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
