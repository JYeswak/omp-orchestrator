//! Three algebras that were all named `Outcome` and must not be.
//!
//! asupersync's `Outcome<T, E>` (Ok/Err/Cancelled/Panicked) stays `Outcome` in
//! this crate. Child execution, journal emit, and transport attempts are
//! different types. AGENTS.md: do not reuse `Outcome` for child execution and
//! saga state.

use serde::{Deserialize, Serialize};

/// Subprocess result. A timeout is not a completed non-zero.
///
/// Was `tick-monitor::Outcome`. Routed here so LEG4 can refuse a third
/// `pub enum Outcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildOutcome {
    Completed {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
    TimedOut {
        after_ms: u64,
        group_killed: bool,
    },
    SpawnFailed {
        message: String,
    },
}

impl ChildOutcome {
    /// stdout only when the process genuinely completed.
    pub fn stdout_if_completed(&self) -> Option<&str> {
        match self {
            Self::Completed { stdout, .. } => Some(stdout),
            Self::TimedOut { .. } => None,
            Self::SpawnFailed { .. } => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Completed { .. } => "completed",
            Self::TimedOut { .. } => "timed_out",
            Self::SpawnFailed { .. } => "spawn_failed",
        }
    }
}

/// Journal emit result. Empty is unrepresentable at the reason-code layer.
///
/// Was `lifecycle-event::Outcome`. Emitted/Refused/Idle is not child execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitOutcome {
    Emitted,
    Refused,
    Idle,
}

impl EmitOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emitted => "emitted",
            Self::Refused => "refused",
            Self::Idle => "idle",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "emitted" => Some(Self::Emitted),
            "refused" => Some(Self::Refused),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}

/// Transport-attempt result. `Delivered` is not a receiver receipt.
///
/// Was `agent-mail-native::packet::Outcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttemptOutcome {
    Delivered,
    Refused,
    TimedOut,
    Unreachable,
    ToolError,
    NothingToDo,
}

impl AttemptOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Refused => "refused",
            Self::TimedOut => "timed-out",
            Self::Unreachable => "unreachable",
            Self::ToolError => "tool-error",
            Self::NothingToDo => "nothing-to-do",
        }
    }

    #[must_use]
    pub const fn is_restrictive(self) -> bool {
        matches!(
            self,
            Self::Refused | Self::TimedOut | Self::Unreachable | Self::ToolError
        )
    }
}
