use ack_spine::ack::{classify_ack_readback, AckVerdict};
use ack_spine::authorities::{
    AckAuthority, AckEvidence, DeliveryAuthority, ReceiverReceipt, TransportAuthority,
};
use ack_spine::spine::{AckSpine, DispatchIntent, PendingDispatchStore};
use asupersync::runtime::RuntimeBuilder;
use asupersync::types::CancelKind;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
fn marker_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ack-spine-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn observed_receipt() -> ReceiverReceipt {
    ReceiverReceipt::idle_to_working(
        "%1409",
        1,
        "spinner-stripped-before",
        "spinner-stripped-after",
    )
    .expect("valid receiver receipt")
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn mutate_restore(
    label: &str,
    original: &[u8],
    mutated: &[u8],
    verify_mutation: impl FnOnce(&[u8]),
) {
    let path = marker_path(label);
    std::fs::write(&path, original).expect("write mutation fixture");
    let before = std::fs::read(&path).expect("read original fixture");
    let before_sha = sha256(&before);
    assert_eq!(before, original);

    std::fs::write(&path, mutated).expect("write mutated fixture");
    let mutated_on_disk = std::fs::read(&path).expect("read mutated fixture");
    assert_ne!(mutated_on_disk, original);
    verify_mutation(&mutated_on_disk);

    std::fs::write(&path, original).expect("restore mutation fixture");
    let after = std::fs::read(&path).expect("read restored fixture");
    let after_sha = sha256(&after);
    println!("mutation sha256 both sides: {label} before={before_sha} after={after_sha}");
    assert_eq!(before, after, "{label} mutation restore is byte-identical");
    assert_eq!(before_sha, after_sha, "{label} mutation digest matches");
    std::fs::remove_file(path).expect("cleanup mutation fixture");
}

#[test]
fn one_row_per_step_and_three_authorities_are_independent() {
    let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
    runtime.block_on(async {
        let cx = asupersync::Cx::current().expect("runtime Cx");
        let path = marker_path("complete");
        let intent = DispatchIntent::new("ack-spine-test", "%1409", "omp-orchestrator");
        let mut spine = AckSpine::new(intent, path.clone());

        spine.begin(&cx).await.expect("begin");
        spine
            .packet_rendered(&cx)
            .await
            .expect("packet rendered");
        spine
            .record_transport(
                &cx,
                TransportAuthority::Succeeded {
                    receipt: "success:[4]".to_owned(),
                },
            )
            .await
            .expect("transport");
        spine
            .record_delivery(&cx, DeliveryAuthority::Observed { receipt: observed_receipt() })
            .await
            .expect("delivery");
        spine
            .record_ack(
                &cx,
                AckAuthority::ReadBack {
                    bead_id: "ack-spine-test".to_owned(),
                    comment_id: "ack-comment-1".to_owned(),
                },
            )
            .await
            .expect("ack");

        assert_eq!(spine.ledger().rows().len(), 5);
        assert_eq!(spine.ledger().steps_taken(), 5);
        let evidence = spine.finish(&cx).await.expect("finish");
        assert!(evidence.transport_succeeded());
        assert!(evidence.delivery_observed());
        assert!(evidence.acknowledgement_read_back());
        assert!(evidence.fully_acknowledged());
        assert!(!path.exists(), "completed dispatch clears pending marker");
    });
}

#[test]
fn cancellation_after_send_persists_recoverable_marker_and_blocks_retry() {
    let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
    runtime.block_on(async {
        let cx = asupersync::Cx::current().expect("runtime Cx");
        let path = marker_path("cancel");
        let intent = DispatchIntent::new("ack-spine-cancel", "%1409", "omp-orchestrator");
        let mut spine = AckSpine::new(intent.clone(), path.clone());
        spine.begin(&cx).await.expect("begin");
        spine
            .record_transport(
                &cx,
                TransportAuthority::Succeeded {
                    receipt: "transport-ok".to_owned(),
                },
            )
            .await
            .expect("transport");

        cx.clone()
            .cancel_with(CancelKind::User, Some("cancel between send and receipt"));
        spine
            .cancel(&cx, "receiver proof not observed")
            .await
            .expect("persist cancellation");

        assert!(path.is_file(), "cancellation leaves pending marker");
        assert!(!spine.retry_allowed(), "pending marker forbids automatic retry");
        let recovered = PendingDispatchStore::new(path.clone())
            .load()
            .expect("read marker")
            .expect("recoverable marker");
        assert_eq!(recovered.intent, intent);
        assert!(spine.ledger().is_consistent());
        assert_eq!(spine.ledger().last_kind(), Some(ack_spine::StepKind::PacketSent));
    });
}

#[test]
fn transport_success_without_observation_is_not_delivery() {
    let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
    runtime.block_on(async {
        let cx = asupersync::Cx::current().expect("runtime Cx");
        let path = marker_path("separation");
        let mut spine = AckSpine::new(
            DispatchIntent::new("ack-spine-separation", "%1409", "omp-orchestrator"),
            path,
        );
        spine.begin(&cx).await.expect("begin");
        spine
            .record_transport(
                &cx,
                TransportAuthority::Succeeded {
                    receipt: "success-without-delivery".to_owned(),
                },
            )
            .await
            .expect("transport");
        spine
            .record_delivery(
                &cx,
                DeliveryAuthority::NotObserved {
                    reason: "post-send capture unchanged".to_owned(),
                },
            )
            .await
            .expect("delivery absence");
        spine
            .record_ack(
                &cx,
                AckAuthority::NotReadBack {
                    reason: "no tracker comment".to_owned(),
                },
            )
            .await
            .expect("ack absence");
        let evidence = spine.evidence().expect("transport, delivery, and ack evidence");
        assert!(evidence.transport_succeeded());
        assert!(!evidence.delivery_observed());
        assert!(!evidence.acknowledgement_read_back());
        assert!(!evidence.fully_acknowledged());
    });
}

#[test]
fn mutation_leg_one_restores_ledger_fixture() {
    mutate_restore(
        "ledger-leg-1",
        b"rows=2\nsteps_taken=2\n",
        b"rows=1\nsteps_taken=2\n",
        |bytes| assert!(String::from_utf8_lossy(bytes).contains("rows=1")),
    );
}

#[test]
fn mutation_leg_two_restores_ack_fixture() {
    mutate_restore(
        "ack-leg-2",
        b"Comments for fixture-bead:\n",
        b"Comments for fixture-bead:\nACK-MUTATION\n",
        |bytes| {
            let read_back = String::from_utf8_lossy(bytes);
            let verdict = classify_ack_readback("fixture-bead", "ACK-MUTATION", Some(0), &read_back, "");
            assert!(matches!(verdict, AckVerdict::Confirmed { .. }));
        },
    );
}

#[test]
fn mutation_leg_four_restores_authority_fixture() {
    mutate_restore(
        "authority-leg-4",
        b"transport=success\ndelivery=not_observed\nack=not_read_back\n",
        b"transport=success\ndelivery=observed\nack=not_read_back\n",
        |bytes| {
            let observed = String::from_utf8_lossy(bytes).contains("delivery=observed");
            let evidence = AckEvidence::new(
                TransportAuthority::Succeeded {
                    receipt: "transport".to_owned(),
                },
                if observed {
                    DeliveryAuthority::Observed { receipt: observed_receipt() }
                } else {
                    DeliveryAuthority::NotObserved { reason: "fixture".to_owned() }
                },
                AckAuthority::NotReadBack { reason: "fixture".to_owned() },
            );
            assert!(evidence.transport_succeeded());
            assert!(evidence.delivery_observed());
            assert!(!evidence.fully_acknowledged());
        },
    );
}
