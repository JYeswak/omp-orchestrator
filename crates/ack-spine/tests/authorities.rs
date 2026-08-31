use ack_spine::authorities::{
    AckAuthority, AckEvidence, AckSummary, DeliveryAuthority, TransportAuthority,
};

fn success() -> TransportAuthority {
    TransportAuthority::Succeeded {
        receipt: "ntm-success:true".to_owned(),
    }
}

fn observed() -> DeliveryAuthority {
    DeliveryAuthority::Observed {
        pane_id: "%1408".to_owned(),
        evidence: "idle_to_working timer_reset_plus_hash_change".to_owned(),
    }
}

fn read_back() -> AckAuthority {
    AckAuthority::ReadBack {
        bead_id: "omp-orchestrator-ack-spine-oj6.3".to_owned(),
        comment_id: "live-comment-read-back".to_owned(),
    }
}

#[test]
fn transport_success_without_observation_is_no_delivery() {
    let evidence = AckEvidence::new(
        success(),
        DeliveryAuthority::NotObserved {
            reason: "post-send capture unchanged".to_owned(),
        },
        AckAuthority::NotReadBack {
            reason: "no tracker comment".to_owned(),
        },
    );
    assert!(evidence.transport_succeeded());
    assert!(!evidence.delivery_observed());
    assert!(!evidence.acknowledgement_read_back());
    assert!(!evidence.fully_acknowledged());
}

#[test]
fn observation_without_bead_comment_is_no_ack() {
    let evidence = AckEvidence::new(
        success(),
        observed(),
        AckAuthority::NotReadBack {
            reason: "br comments list has no matching author".to_owned(),
        },
    );
    assert!(evidence.transport_succeeded());
    assert!(evidence.delivery_observed());
    assert!(!evidence.acknowledgement_read_back());
    assert!(!evidence.fully_acknowledged());
}

#[test]
fn live_shape_requires_all_three_citations_separately() {
    let evidence = AckEvidence::new(success(), observed(), read_back());
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
fn mutation_collapsing_any_authority_goes_red() {
    let transport_only = AckEvidence::new(
        success(),
        DeliveryAuthority::NotObserved {
            reason: "missing observation".to_owned(),
        },
        AckAuthority::NotReadBack {
            reason: "missing comment".to_owned(),
        },
    );
    assert!(!transport_only.fully_acknowledged());

    let delivery_without_transport = AckEvidence::new(
        TransportAuthority::Failed {
            detail: "transport failure".to_owned(),
        },
        observed(),
        read_back(),
    );
    assert!(!delivery_without_transport.fully_acknowledged());

    let delivery_only = AckEvidence::new(
        TransportAuthority::Failed {
            detail: "transport failure".to_owned(),
        },
        observed(),
        AckAuthority::NotReadBack {
            reason: "missing comment".to_owned(),
        },
    );
    assert!(!delivery_only.fully_acknowledged());
}
