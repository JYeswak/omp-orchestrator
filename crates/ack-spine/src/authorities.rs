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

/// Evidence from an independent post-send receiver observation only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryAuthority {
    Observed { pane_id: String, evidence: String },
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

    fn delivery_observed() -> DeliveryAuthority {
        DeliveryAuthority::Observed {
            pane_id: "%1408".to_owned(),
            evidence: "idle_to_working timer_reset hash_changed".to_owned(),
        }
    }

    fn delivery_missing() -> DeliveryAuthority {
        DeliveryAuthority::NotObserved {
            reason: "post-send capture unchanged".to_owned(),
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
            reason: "br comments list has no matching author".to_owned(),
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
