#![forbid(unsafe_code)]

//! 7wn9.5: IrcDeliveryReceipt is consumed in the receipt path; pane transport
//! cannot map sender exit onto it. Empty crate-src scan is ERROR.

use receiver_receipt::{
    pane_transport_cannot_use_irc_receipt, record_hub_irc_receipt, IrcDeliveryOutcome,
    IrcDeliveryReceipt,
};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("receiver-receipt lives under crates/")
        .to_path_buf()
}

fn code_only_lines(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

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
            if code_only_lines(&text).contains("IrcDeliveryReceipt") {
                hits.push(file);
            }
        }
    }
    Ok(hits)
}

fn maps_sender_exit_onto_irc_receipt(text: &str) -> bool {
    let code = code_only_lines(text);
    code.contains("IrcDeliveryReceipt")
        && (code.contains("sender_exit") || code.contains("sender.exit") || code.contains("status.success()"))
        && code.contains("IrcDeliveryOutcome")
        && !code.contains("PANE_TRANSPORT_HAS_NO_IRC_RECEIPT")
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
    let dir = std::env::temp_dir().join(format!(
        "irc-delivery-known-bad-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("specimen dir");
    let specimen = dir.join("maps_sender_exit.rs");
    fs::write(
        &specimen,
        r#"
        fn from_exit(sender_exit: i32) -> IrcDeliveryReceipt {
            IrcDeliveryReceipt {
                to: "x".into(),
                outcome: if sender_exit == 0 { IrcDeliveryOutcome::Injected } else { IrcDeliveryOutcome::Failed },
                error: None,
            }
        }
        "#,
    )
    .expect("write specimen");
    let text = fs::read_to_string(&specimen).expect("read specimen");
    assert!(
        maps_sender_exit_onto_irc_receipt(&text),
        "detector must name the specimen {}",
        specimen.display()
    );
    let pane = pane_transport_cannot_use_irc_receipt("tmux-send-keys", 0);
    let err = pane.expect_err("pane transport");
    assert!(
        err.to_string().contains(specimen.file_name().unwrap().to_str().unwrap())
            || err.to_string().contains("PANE_TRANSPORT_HAS_NO_IRC_RECEIPT"),
        "refusal must name the contract: {err}"
    );
    let production = fs::read_to_string(repo_root().join("crates/receiver-receipt/src/irc_delivery.rs"))
        .expect("production");
    assert!(
        !maps_sender_exit_onto_irc_receipt(&production),
        "production must not map sender exit onto IrcDeliveryReceipt"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deleting_the_mapping_guard_turns_known_bad_red() {
    let production = fs::read_to_string(
        repo_root().join("crates/receiver-receipt/src/irc_delivery.rs"),
    )
    .expect("production");
    assert!(
        production.contains("PANE_TRANSPORT_HAS_NO_IRC_RECEIPT"),
        "deleting PANE_TRANSPORT_HAS_NO_IRC_RECEIPT from irc_delivery.rs turns this RED"
    );
    let specimen = r#"
        fn from_exit(sender_exit: i32) -> IrcDeliveryReceipt {
            IrcDeliveryReceipt { to: "x".into(), outcome: IrcDeliveryOutcome::Injected, error: None }
        }
    "#;
    assert!(
        maps_sender_exit_onto_irc_receipt(specimen),
        "guard must still classify a sender-exit mapping specimen as known-bad"
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
