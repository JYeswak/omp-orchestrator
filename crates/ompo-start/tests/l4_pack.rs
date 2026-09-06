//! L4-SPAWN pack receipt. Send success without a receipt object fails (DP-6).

use ompo_start::{retain_pack_receipt, PackError, PackReceipt, SendAttempt, TICK_ZERO_TARGET};

#[test]
fn spawn_retains_pack_receipt_object() {
    let pack = b"context pack for tick_zero";
    let receipt = PackReceipt::new("%1408", pack);
    assert_eq!(receipt.target(), TICK_ZERO_TARGET);
    let retained = retain_pack_receipt(SendAttempt {
        exit_code: 0,
        success: true,
        receipt: Some(receipt.clone()),
    })
    .expect("send with receipt must retain the object");
    assert_eq!(retained, receipt);
    assert_eq!(retained.pane_id(), "%1408");
    assert_eq!(retained.pack_sha256().len(), 64);
    println!("pack_sha256 {}", retained.pack_sha256());
}

#[test]
fn send_success_without_receipt_fails() {
    match retain_pack_receipt(SendAttempt {
        exit_code: 0,
        success: true,
        receipt: None,
    }) {
        Err(PackError::SendWithoutReceipt {
            exit_code: 0,
            success: true,
        }) => {
            println!("DP-6: exit 0 success true without receipt refused");
        }
        other => panic!("send success without receipt must fail, got {other:?}"),
    }
}
