#![forbid(unsafe_code)]
//! THE FLEET LOOP'S ON/OFF SWITCH.
//!
//! Joshua, 2026-08-27: *"it should stay on until i say so or session resets"* -- the loop must be
//! OFF only when a human turned it off, and that intent must survive a crash, a reboot, and a
//! context reset. So the switch is a FILE, following the house pattern from
//! `mcp_agent_mail_rust/crates/mcp-agent-mail-server/src/atc.rs` (activate/deactivate/refresh +
//! status): the file's EXISTENCE is the state and its CONTENTS are the reason.
//!
//! Why a file and not a flag, an env var, or a daemon:
//!   * it survives process death -- an env var dies with the shell that set it;
//!   * every lane can read it without linking this crate or running a binary;
//!   * `ls` answers "is the loop off?" with no daemon to interrogate;
//!   * there is no writer to crash, so the switch cannot fail INTO the off state.
//!
//! THE DEFAULT IS ON. A missing file, an unreadable file, an unset path -- all mean RUNNING. This
//! is the deliberate inverse of the dispatch gates, and the asymmetry is the point: a dispatch
//! gate fails CLOSED because dispatching on a stale verdict does damage, whereas a switch that
//! failed closed would let a permissions error or a full disk silently stop the fleet -- which is
//! precisely the "the loop keeps turning itself off" failure this exists to end.

use std::path::{Path, PathBuf};

/// Ledger location relative to `$HOME`. Never a home literal: this repo is
/// installable on any fleet machine, and a hardcoded home compiled fine after
/// a move and then silently read the WRONG user's state (bead 7ai class).
pub const DEFAULT_SWITCH_PATH_HOME_RELATIVE: &str = ".local/state/flywheel/loop-switch.off";

/// Resolved state of the loop switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchState {
    /// The loop runs. This is the default and the fail-safe.
    On,
    /// A human turned the loop off. Carries the reason recorded at the time.
    Off { reason: String },
}

impl SwitchState {
    pub fn is_on(&self) -> bool {
        matches!(self, SwitchState::On)
    }
    pub fn label(&self) -> &'static str {
        match self {
            SwitchState::On => "ON",
            SwitchState::Off { .. } => "OFF",
        }
    }
}

pub fn switch_path() -> PathBuf {
    std::env::var_os("FLEET_LOOP_SWITCH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(PathBuf::new);
            if home.as_os_str().is_empty() {
                // FAIL-CLOSED: with no HOME the loop switch resolves nowhere.
                // Reading it reports unambiguously ON (file absent), so a
                // missing HOME degrades to "loop runs", the documented
                // fail-safe, and never to another user's state.
                eprintln!("loop-switch: HOME unset; switch file unresolved, state ON");
            }
            home.join(DEFAULT_SWITCH_PATH_HOME_RELATIVE)
        })
}

/// Read the switch. FAIL-SAFE ON: only a file that exists AND is readable turns the loop off.
///
/// An unreadable file is treated as OFF with a stated reason rather than silently ON -- if a human
/// created the file, their intent to stop was expressed, and we honour it even when we cannot read
/// why. A file that does not exist is unambiguously ON.
pub fn read_state(path: &Path) -> SwitchState {
    if !path.exists() {
        return SwitchState::On;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let reason = text.trim();
            SwitchState::Off {
                reason: if reason.is_empty() {
                    "no reason recorded".to_string()
                } else {
                    reason.to_string()
                },
            }
        }
        Err(e) => SwitchState::Off {
            reason: format!("switch file present but unreadable ({e}); honouring the stop"),
        },
    }
}

/// Turn the loop OFF, recording who/why. Creating the parent directory is part of the operation:
/// a switch that cannot be written is a switch that does not exist.
pub fn turn_off(path: &Path, reason: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stamp = chrono::Utc::now().to_rfc3339();
    let body = if reason.trim().is_empty() {
        format!("{stamp} stopped by hand; no reason given")
    } else {
        format!("{stamp} {}", reason.trim())
    };
    std::fs::write(path, body)
}

/// Turn the loop ON by removing the switch file. Idempotent: already-on is a success, not an
/// error, so `on` is always safe to run and a stuck loop is never one failed command away.
pub fn turn_on(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn status_json(path: &Path) -> serde_json::Value {
    let state = read_state(path);
    serde_json::json!({
        "schema": "zs.loop-switch.v1",
        "state": state.label(),
        "running": state.is_on(),
        "path": path.display().to_string(),
        "reason": match &state {
            SwitchState::Off { reason } => Some(reason.clone()),
            SwitchState::On => None,
        },
    })
}
