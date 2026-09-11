#![forbid(unsafe_code)]

//! 7wn9.5: IrcDeliveryReceipt is consumed in the receipt path; pane transport
//! cannot map sender exit onto it. Empty crate-src scan is ERROR.

use receiver_receipt::{
    pane_transport_cannot_use_irc_receipt, record_hub_irc_receipt, IrcDeliveryOutcome,
    IrcDeliveryReceipt,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("receiver-receipt lives under crates/")
        .to_path_buf()
}

// Caller census routes through `text_structure::code_only` (bead -9ub39).
use text_structure::code_only;

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn scan_irc_delivery_callers(crates_src_roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    if crates_src_roots.is_empty() {
        return Err("SCAN_EMPTY: no crate src roots — empty scan is not a pass".to_owned());
    }
    let mut hits = Vec::new();
    for src in crates_src_roots {
        let mut files = Vec::new();
        rust_sources(src, &mut files);
        for file in files {
            let text = fs::read_to_string(&file).unwrap_or_default();
            if code_only(&text).contains("IrcDeliveryReceipt") {
                hits.push(file);
            }
        }
    }
    Ok(hits)
}

fn specimen_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/maps_sender_exit.rs")
}

fn run_mapping_gate() -> std::process::Output {
    let specimen = specimen_path();
    let bin = std::env::var_os("CARGO_BIN_EXE_receiver_receipt")
        .or_else(|| std::env::var_os("CARGO_BIN_EXE_receiver-receipt"))
        .expect("Cargo must set CARGO_BIN_EXE_receiver_receipt for the package bin");
    Command::new(bin)
        .args([
            "scan-sender-exit-mapping",
            specimen.to_str().expect("utf8"),
        ])
        .output()
        .expect("spawn receiver-receipt")
}


#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    let err = scan_irc_delivery_callers(&[]).expect_err("empty scan");
    assert!(err.contains("SCAN_EMPTY"), "{err}");
}

#[test]
fn production_src_has_a_real_irc_delivery_receipt_caller() {
    let src = repo_root().join("crates/receiver-receipt/src");
    let hits = scan_irc_delivery_callers(&[src]).expect("scan");
    assert!(
        !hits.is_empty(),
        "IrcDeliveryReceipt must appear in crates/*/src receipt path"
    );
    assert!(
        hits.iter()
            .any(|p| p.ends_with("irc_delivery.rs") || p.ends_with("lib.rs")),
        "caller must be the receipt module, got {hits:?}"
    );
}

#[test]
fn hub_send_records_upstream_receipt_as_receiver_evidence() {
    let receipt = IrcDeliveryReceipt {
        to: "peer".to_owned(),
        outcome: IrcDeliveryOutcome::Injected,
        error: None,
    };
    let evidence = record_hub_irc_receipt(receipt);
    assert_eq!(evidence.receipt.outcome, IrcDeliveryOutcome::Injected);
}

#[test]
fn known_bad_sender_exit_mapping_is_named_and_refused() {
    let output = run_mapping_gate();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_ne!(
        output.status.code(),
        Some(0),
        "gate must return nonzero, stderr={stderr}"
    );
    assert!(
        stderr.contains("maps_sender_exit.rs"),
        "refusal must name the specimen path, stderr={stderr}"
    );
    assert!(
        stderr.contains("SENDER_EXIT_MAPPED_TO_IRC_RECEIPT")
            || stderr.contains("PANE_TRANSPORT_HAS_NO_IRC_RECEIPT"),
        "refusal must name the contract, stderr={stderr}"
    );
    let production = fs::read_to_string(
        repo_root().join("crates/receiver-receipt/src/irc_delivery.rs"),
    )
    .expect("production");
    assert!(
        !receiver_receipt::maps_sender_exit_onto_irc_receipt(&production),
        "production must not map sender exit onto IrcDeliveryReceipt"
    );
}

#[test]
fn deleting_the_mapping_guard_turns_known_bad_red() {
    let output = run_mapping_gate();
    assert_ne!(
        output.status.code(),
        Some(0),
        "deleting refuse_sender_exit_mapping_file's mapping check makes this exit 0"
    );
}

#[test]
fn positive_control_clean_surface_records_typed_refusal_not_sender_exit() {
    let result = pane_transport_cannot_use_irc_receipt("ntm-robot-send", 0);
    assert!(result.is_err());
    let src = repo_root().join("crates/receiver-receipt/src");
    let hits = scan_irc_delivery_callers(&[src]).expect("scan");
    assert!(!hits.is_empty(), "artifact/receipt type is present in src");
}
