//! Authored delivery and acknowledgement vocabulary for the OMP lifecycle.
//!
//! The names, ordering, and delivery-to-acknowledgement mapping mirror the
//! pinned asupersync source at `messaging/class.rs:17-29,83-94` (rev
//! `fa3c01aec`). They are authored here rather than re-exported because the
//! upstream `messaging-fabric` feature is not usable at that revision:
//! `messaging/consumer.rs:1299-1300` calls constructors gated behind
//! `test-internals`.
//!
//! **NO-CLAIM:** these enums define the shared vocabulary only. They do not
//! prove transport delivery, authority-plane commitment, or receiver receipt.
//! Those remain separate runtime claims.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The acknowledgement boundary a message or service flow has crossed.
///
/// This is the inhabited vocabulary from the pinned upstream messaging class,
/// not the uninhabited `obligation::graded::AckKind` marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AckKind {
    /// Packet-plane custody was accepted for forwarding.
    Accepted,
    /// The authority plane committed the control entry or obligation.
    Committed,
    /// The declared durability class was met.
    Recoverable,
    /// The callee completed the service obligation.
    Served,
    /// The configured delivery or receipt boundary was crossed.
    Received,
}

impl AckKind {
    /// All acknowledgement boundaries in their strength/order sequence.
    pub const ALL: [Self; 5] = [
        Self::Accepted,
        Self::Committed,
        Self::Recoverable,
        Self::Served,
        Self::Received,
    ];
}

impl fmt::Display for AckKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Accepted => "accepted",
            Self::Committed => "committed",
            Self::Recoverable => "recoverable",
            Self::Served => "served",
            Self::Received => "received",
        };
        f.write_str(name)
    }
}

/// The delivery guarantee class requested or offered by a flow.
///
/// The order mirrors the pinned upstream design from ephemeral interaction to
/// replayable forensic delivery. A stronger class is not evidence that a
/// particular runtime path actually delivered a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryClass {
    /// Hot ephemeral pub/sub with no durability or obligation tracking.
    #[default]
    EphemeralInteractive,
    /// Durable ordered stream semantics with authority-plane commit.
    DurableOrdered,
    /// Request/reply and service flows backed by explicit obligations.
    ObligationBacked,
    /// Safe for stewardship change and cut-certified mobility operations.
    MobilitySafe,
    /// Replay-heavy reasoning with explicit evidence retention.
    ForensicReplayable,
}

impl DeliveryClass {
    /// All delivery classes in ascending cost/strength order.
    pub const ALL: [Self; 5] = [
        Self::EphemeralInteractive,
        Self::DurableOrdered,
        Self::ObligationBacked,
        Self::MobilitySafe,
        Self::ForensicReplayable,
    ];

    /// The acknowledgement boundary associated with this delivery class in
    /// the pinned upstream messaging taxonomy.
    pub const fn ack_kind(self) -> AckKind {
        match self {
            Self::EphemeralInteractive => AckKind::Accepted,
            Self::DurableOrdered => AckKind::Recoverable,
            Self::ObligationBacked => AckKind::Served,
            Self::MobilitySafe | Self::ForensicReplayable => AckKind::Received,
        }
    }
}

impl fmt::Display for DeliveryClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::EphemeralInteractive => "ephemeral-interactive",
            Self::DurableOrdered => "durable-ordered",
            Self::ObligationBacked => "obligation-backed",
            Self::MobilitySafe => "mobility-safe",
            Self::ForensicReplayable => "forensic-replayable",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_kind_vocabulary_is_inhabited_and_ordered() {
        assert_eq!(AckKind::ALL.len(), 5);
        assert_eq!(AckKind::Accepted.to_string(), "accepted");
        assert!(AckKind::Accepted < AckKind::Committed);
        assert!(AckKind::Committed < AckKind::Recoverable);
        assert!(AckKind::Recoverable < AckKind::Served);
        assert!(AckKind::Served < AckKind::Received);
    }

    #[test]
    fn delivery_class_vocabulary_is_inhabited_and_mapped() {
        assert_eq!(DeliveryClass::ALL.len(), 5);
        assert_eq!(DeliveryClass::default(), DeliveryClass::EphemeralInteractive);
        assert_eq!(
            DeliveryClass::EphemeralInteractive.ack_kind(),
            AckKind::Accepted
        );
        assert_eq!(
            DeliveryClass::DurableOrdered.ack_kind(),
            AckKind::Recoverable
        );
        assert_eq!(
            DeliveryClass::ObligationBacked.ack_kind(),
            AckKind::Served
        );
        assert_eq!(DeliveryClass::MobilitySafe.ack_kind(), AckKind::Received);
        assert_eq!(
            DeliveryClass::ForensicReplayable.ack_kind(),
            AckKind::Received
        );
    }

    #[test]
    fn display_names_match_the_wire_vocabulary() {
        assert_eq!(DeliveryClass::MobilitySafe.to_string(), "mobility-safe");
        assert_eq!(
            DeliveryClass::ForensicReplayable.to_string(),
            "forensic-replayable"
        );
    }
}
