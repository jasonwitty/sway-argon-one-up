//! Reads the active theme name from `~/.config/sway-themes/current` and
//! the rendered dashboard stylesheet from
//! `~/.config/system-dashboard/dashboard.css`. switch-theme renders the
//! stylesheet on every theme switch, so the frontend can re-load it and
//! pick up the new palette without restarting.

use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug)]
pub struct ThemeState {
    pub name: String,
    pub stylesheet: String,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn current_theme_path() -> PathBuf {
    home().join(".config/sway-themes/current")
}

fn rendered_css_path() -> PathBuf {
    home().join(".config/system-dashboard/dashboard.css")
}

pub fn read() -> ThemeState {
    let name = fs::read_to_string(current_theme_path())
        .unwrap_or_else(|_| "frappe".into())
        .trim()
        .to_string();
    let stylesheet = fs::read_to_string(rendered_css_path()).unwrap_or_default();
    ThemeState { name, stylesheet }
}

/// File-watch target so the frontend can be told to reload its stylesheet
/// when switch-theme rewrites it. Returned as a path string for the watcher.
pub fn stylesheet_watch_path() -> String {
    rendered_css_path().to_string_lossy().into_owned()
}
