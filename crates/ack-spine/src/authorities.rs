#![forbid(unsafe_code)]

//! Independent ACK authorities for one dispatch attempt.
//!
//! Transport success, receiver observation, and tracker comment read-back are
//! intentionally distinct claims. None can manufacture either of the others.

/// Evidence from the sending transport only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportAuthority {
    Succeeded { receipt: String },
    Failed { detail: String },
}

/// The two receiver-receipt observations that can establish delivery.
///
/// This shape mirrors receiver-receipt: delivery is observational only and
/// requires both a receiver transition and a changed spinner-stripped hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverReceiptBasis {
    IdleToWorking,
    TimerResetWithHashChange,
}

/// A validated receiver-side receipt. It contains no sender result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverReceipt {
    pane_id: String,
    basis: ReceiverReceiptBasis,
    timer_before_secs: Option<u64>,
    timer_after_secs: u64,
    spinner_stripped_hash_before: String,
    spinner_stripped_hash_after: String,
}

impl ReceiverReceipt {
    /// Construct the IDLE -> WORKING receiver-receipt shape.
    ///
    /// The receiver-receipt classifier caps the new timer at 30 seconds and
    /// requires a changed spinner-stripped content hash.
    pub fn idle_to_working(
        pane_id: impl Into<String>,
        timer_after_secs: u64,
        spinner_stripped_hash_before: impl Into<String>,
        spinner_stripped_hash_after: impl Into<String>,
    ) -> Option<Self> {
        let pane_id = pane_id.into();
        let before = spinner_stripped_hash_before.into();
        let after = spinner_stripped_hash_after.into();
        if pane_id.is_empty()
            || timer_after_secs > 30
            || before.is_empty()
            || after.is_empty()
            || before == after
        {
            return None;
        }
        Some(Self {
            pane_id,
            basis: ReceiverReceiptBasis::IdleToWorking,
            timer_before_secs: None,
            timer_after_secs,
            spinner_stripped_hash_before: before,
            spinner_stripped_hash_after: after,
        })
    }

    /// Construct the WORKING -> WORKING timer-reset receiver-receipt shape.
    pub fn timer_reset_with_hash_change(
        pane_id: impl Into<String>,
        timer_before_secs: u64,
        timer_after_secs: u64,
        spinner_stripped_hash_before: impl Into<String>,
        spinner_stripped_hash_after: impl Into<String>,
    ) -> Option<Self> {
        let pane_id = pane_id.into();
        let before = spinner_stripped_hash_before.into();
        let after = spinner_stripped_hash_after.into();
        if pane_id.is_empty()
            || timer_after_secs >= timer_before_secs
            || before.is_empty()
            || after.is_empty()
            || before == after
        {
            return None;
        }
        Some(Self {
            pane_id,
            basis: ReceiverReceiptBasis::TimerResetWithHashChange,
            timer_before_secs: Some(timer_before_secs),
            timer_after_secs,
            spinner_stripped_hash_before: before,
            spinner_stripped_hash_after: after,
        })
    }

    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    pub const fn basis(&self) -> ReceiverReceiptBasis {
        self.basis
    }

    pub const fn timer_before_secs(&self) -> Option<u64> {
        self.timer_before_secs
    }

    pub const fn timer_after_secs(&self) -> u64 {
        self.timer_after_secs
    }

    pub fn spinner_stripped_hash_before(&self) -> &str {
        &self.spinner_stripped_hash_before
    }

    pub fn spinner_stripped_hash_after(&self) -> &str {
        &self.spinner_stripped_hash_after
    }
}

/// Evidence from an independent post-send receiver observation only.
///
/// Observed is deliberately coupled to ReceiverReceipt. A transport success,
/// pane capture, timer, or arbitrary text cannot be called delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryAuthority {
    Observed { receipt: ReceiverReceipt },
    NotObserved { reason: String },
}

/// Evidence from a durable tracker comment read-back only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckAuthority {
    ReadBack { bead_id: String, comment_id: String },
    NotReadBack { reason: String },
}

/// One independently obtained fact from each authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckEvidence {
    pub transport: TransportAuthority,
    pub delivery: DeliveryAuthority,
    pub acknowledgement: AckAuthority,
}

impl AckEvidence {
    /// Build an evidence row without deriving one authority from another.
    pub const fn new(
        transport: TransportAuthority,
        delivery: DeliveryAuthority,
        acknowledgement: AckAuthority,
    ) -> Self {
        Self {
            transport,
            delivery,
            acknowledgement,
        }
    }

    /// Ask only the transport authority.
    pub const fn transport_succeeded(&self) -> bool {
        matches!(self.transport, TransportAuthority::Succeeded { .. })
    }

    /// Ask only the receiver-observation authority.
    pub const fn delivery_observed(&self) -> bool {
        matches!(self.delivery, DeliveryAuthority::Observed { .. })
    }

    /// Ask only the tracker read-back authority.
    pub const fn acknowledgement_read_back(&self) -> bool {
        matches!(self.acknowledgement, AckAuthority::ReadBack { .. })
    }

    /// The final claim requires all three authorities independently.
    pub const fn fully_acknowledged(&self) -> bool {
        self.transport_succeeded() && self.delivery_observed() && self.acknowledgement_read_back()
    }

    pub const fn summary(&self) -> AckSummary {
        AckSummary {
            transport_succeeded: self.transport_succeeded(),
            delivery_observed: self.delivery_observed(),
            acknowledgement_read_back: self.acknowledgement_read_back(),
            fully_acknowledged: self.fully_acknowledged(),
        }
    }
}

/// Stable, separately named result for a ledger or external oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckSummary {
    pub transport_succeeded: bool,
    pub delivery_observed: bool,
    pub acknowledgement_read_back: bool,
    pub fully_acknowledged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transport_success() -> TransportAuthority {
        TransportAuthority::Succeeded {
            receipt: "successful:[%1408]".to_owned(),
        }
    }

    fn receipt() -> ReceiverReceipt {
        ReceiverReceipt::idle_to_working(
            "%1408",
            1,
            "spinner-stripped-before",
            "spinner-stripped-after",
        )
        .expect("fixture must satisfy receiver-receipt shape")
    }

    fn delivery_observed() -> DeliveryAuthority {
        DeliveryAuthority::Observed { receipt: receipt() }
    }

    fn delivery_missing() -> DeliveryAuthority {
        DeliveryAuthority::NotObserved {
            reason: "receiver receipt absent: post-send capture unchanged".to_owned(),
        }
    }

    fn ack_read_back() -> AckAuthority {
        AckAuthority::ReadBack {
            bead_id: "omp-orchestrator-ack-spine-oj6.3".to_owned(),
            comment_id: "live-read-back".to_owned(),
        }
    }

    fn ack_missing() -> AckAuthority {
        AckAuthority::NotReadBack {
            reason: "br comments list has no matching marker".to_owned(),
        }
    }

    #[test]
    fn transport_success_without_observation_is_not_delivery() {
        let evidence = AckEvidence::new(transport_success(), delivery_missing(), ack_missing());
        let summary = evidence.summary();
        assert!(summary.transport_succeeded);
        assert!(!summary.delivery_observed);
        assert!(!summary.acknowledgement_read_back);
        assert!(!summary.fully_acknowledged);
    }

    #[test]
    fn observation_without_bead_comment_is_not_ack() {
        let evidence = AckEvidence::new(transport_success(), delivery_observed(), ack_missing());
        let summary = evidence.summary();
        assert!(summary.transport_succeeded);
        assert!(summary.delivery_observed);
        assert!(!summary.acknowledgement_read_back);
        assert!(!summary.fully_acknowledged);
    }

    #[test]
    fn known_good_has_three_separately_cited_authorities() {
        let evidence = AckEvidence::new(transport_success(), delivery_observed(), ack_read_back());
        assert_eq!(
            evidence.summary(),
            AckSummary {
                transport_succeeded: true,
                delivery_observed: true,
                acknowledgement_read_back: true,
                fully_acknowledged: true,
            }
        );
    }

    #[test]
    fn invalid_receiver_receipt_shapes_are_rejected() {
        assert!(ReceiverReceipt::idle_to_working("%1408", 31, "a", "b").is_none());
        assert!(ReceiverReceipt::idle_to_working("%1408", 1, "same", "same").is_none());
        assert!(
            ReceiverReceipt::timer_reset_with_hash_change("%1408", 1, 1, "a", "b").is_none()
        );
        assert!(
            ReceiverReceipt::timer_reset_with_hash_change("%1408", 58, 1, "same", "same")
                .is_none()
        );
    }

    #[test]
    fn collapsing_any_two_authorities_cannot_make_a_full_ack() {
        let evidence = AckEvidence::new(
            TransportAuthority::Failed {
                detail: "transport failed".to_owned(),
            },
            delivery_observed(),
            ack_read_back(),
        );
        assert!(!evidence.fully_acknowledged());

        let evidence = AckEvidence::new(transport_success(), delivery_missing(), ack_read_back());
        assert!(!evidence.fully_acknowledged());

        let evidence = AckEvidence::new(transport_success(), delivery_observed(), ack_missing());
        assert!(!evidence.fully_acknowledged());
    }
}
