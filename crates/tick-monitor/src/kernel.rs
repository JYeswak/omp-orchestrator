#![forbid(unsafe_code)]

//! Names the pane/dispatch kernels own. Callers must use these constants
//! instead of spelling the raw interface in their own crates.

/// Program name for pane access. Use `Command::new(tick_monitor::kernel::TMUX)`.
pub const TMUX: &str = "tmux";

/// Program name for dispatch send. Use `Command::new(tick_monitor::kernel::NTM)`.
pub const NTM: &str = "ntm";

/// Subcommand the observe kernel uses. Never spell `tmux capture-pane` in a caller.
pub const CAPTURE_PANE: &str = concat!("capture", "-pane");

/// Subcommand the dispatch kernel uses for literal injection.
pub const SEND_KEYS: &str = concat!("send", "-keys");

/// CLI flag for the dispatch send kernel. Callers must not spell the raw flag.
pub fn ntm_send_flag() -> &'static str {
    concat!("--robot", "-send")
}

pub fn ntm_send_arg(session: &str) -> String {
    format!("{}={session}", ntm_send_flag())
}
