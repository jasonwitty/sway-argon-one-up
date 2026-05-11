//! Idle / screen-lock state — reads the same `/tmp/.idle-toggle-mode` file
//! that `bin/idle-toggle` writes, plus the `/tmp/.idle-toggle-claude-active`
//! sub-state file for the `claude` mode.

use serde::Serialize;
use std::fs;

const MODE_FILE: &str = "/tmp/.idle-toggle-mode";
const CLAUDE_STATE_FILE: &str = "/tmp/.idle-toggle-claude-active";

#[derive(Serialize, Clone, Debug)]
pub struct IdleState {
    /// "on" | "off" | "claude"
    pub mode: String,
    /// Only set when mode == "claude": "active" or "idle".
    pub claude_state: Option<String>,
}

pub fn read() -> IdleState {
    let mode = fs::read_to_string(MODE_FILE)
        .unwrap_or_else(|_| "on".into())
        .trim()
        .to_string();

    let claude_state = if mode == "claude" {
        fs::read_to_string(CLAUDE_STATE_FILE)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        None
    };

    IdleState { mode, claude_state }
}
