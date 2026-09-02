//! The OMP pane lifecycle and its bounded subprocess bridge.
//!
//! This is intentionally not the RPC session machine in the control-plane sibling. It is the
//! pane-facing state algebra consumed by a future orchestrator boundary.

use std::time::Duration;
use subprocess_contract::BoundedOutcome;

/// The pane lifecycle. `Stopped`, `Failed`, and `TimedOut` are terminal by construction of
/// [`Lifecycle::transition`]; no caller receives a mutable state slot to bypass that rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// Child started; readiness has not been observed.
    Spawned,
    /// Readiness observed and the required version advertised.
    Ready,
    /// Protocol negotiation completed.
    Negotiated,
    /// Handshake complete and issued requests are answered.
    Active,
    /// Shutdown requested with a bounded deadline.
    Stopping,
    /// Clean terminal completion.
    Stopped,
    /// Restrictive terminal: the subject failed independently or could not spawn.
    Failed,
    /// Restrictive terminal: a bounded wait elapsed and the subject was killed/cancelled.
    TimedOut,
}

/// A finite deadline carried by shutdown input. There is no `Default`, zero-argument constructor,
/// or unbounded wait representation in this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitDeadline(Duration);

impl WaitDeadline {
    /// Construct a deadline from a finite standard duration.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// Return the duration owned by this deadline.
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.0
    }
}

/// Events accepted by the pane lifecycle machine. Shutdown cannot be requested without carrying
/// a [`WaitDeadline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleInput {
    Ready,
    Negotiated,
    Activated,
    StopRequested(WaitDeadline),
    Stopped,
    Failed,
    TimedOut,
}

impl Lifecycle {
    /// Whether this state admits no further transitions.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed | Self::TimedOut)
    }

    /// Whether this terminal is restrictive and must not be read as success.
    #[must_use]
    pub const fn is_restrictive_terminal(self) -> bool {
        matches!(self, Self::Failed | Self::TimedOut)
    }

    /// Apply one input. Terminal closure belongs here, not to callers: every input leaves every
    /// terminal unchanged, including an input that would otherwise be valid from another state.
    #[must_use]
    pub const fn transition(self, input: LifecycleInput) -> Self {
        if self.is_terminal() {
            return self;
        }

        match (self, input) {
            (Self::Spawned, LifecycleInput::Ready) => Self::Ready,
            (Self::Ready, LifecycleInput::Negotiated) => Self::Negotiated,
            (Self::Negotiated, LifecycleInput::Activated) => Self::Active,
            (Self::Active, LifecycleInput::StopRequested(_)) => Self::Stopping,
            (Self::Stopping, LifecycleInput::Stopped) => Self::Stopped,
            (_, LifecycleInput::Failed) => Self::Failed,
            (_, LifecycleInput::TimedOut) => Self::TimedOut,
            (current, _) => current,
        }
    }

    /// Map the complete bounded subprocess outcome vocabulary into this lifecycle.
    ///
    /// - `Completed(success)` → `Stopped`.
    /// - `Completed(non-success)` → `Failed`.
    /// - `TimedOut` → `TimedOut`.
    /// - `Unspawned` → `Failed`.
    ///
    /// The exhaustive match is the bridge law: adding a new bounded outcome forces this mapping
    /// to be revisited at compile time.
    #[must_use]
    pub fn from_bounded_outcome(outcome: BoundedOutcome) -> Self {
        match outcome {
            BoundedOutcome::Completed(output) if output.status.success() => Self::Stopped,
            BoundedOutcome::Completed(_) => Self::Failed,
            BoundedOutcome::TimedOut => Self::TimedOut,
            BoundedOutcome::Unspawned(_) => Self::Failed,
        }
    }
}
