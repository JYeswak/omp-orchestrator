#![forbid(unsafe_code)]

use contabo_reclaim::{
    basename_is_whitelisted, decide_guards, parse_listing, validate_candidate, ActiveBuild,
    Candidate, CandidateSet, ControlSnapshot, EntryKind, GuardDecision, ReclaimMode, ReclaimReport,
    RemoteProcessObservation, RunOutcome, WHITELIST,
};
use contabo_reclaim::{probe, WorkerSpec};
use std::path::{Path, PathBuf};
use std::process::Command;

fn worker() -> WorkerSpec {
    WorkerSpec {
        id: "contabo-2",
        host: "94.72.121.46",
    }
}

fn candidate(path: &str, kind: EntryKind) -> Candidate {
    Candidate {
        path: PathBuf::from(path),
        kind,
        bytes: 4096,
    }
}

fn clear_control() -> ControlSnapshot {
    ControlSnapshot {
        worker_id: worker().id.to_owned(),
        worker_host: worker().host.to_owned(),
        worker_status: "healthy".to_owned(),
        used_slots: 0,
        active_builds: Vec::new(),
    }
}

#[test]
fn known_good_whitelist_and_containment_are_explicit() {
    let validated = validate_candidate(
        Path::new("/var/lib/rch"),
        candidate(
            "/var/lib/rch/.rch-target-contabo-2-pool-a",
            EntryKind::Directory,
        ),
        None,
    )
    .expect("known-good pool directory");
    assert_eq!(validated.rule.as_str(), ".rch-target-*");
    assert!(basename_is_whitelisted(&validated.candidate.path));
    assert_eq!(WHITELIST.len(), 6);
    println!(
        "KNOWN_GOOD rule={} path={}",
        validated.rule.as_str(),
        validated.candidate.path.display()
    );
}

#[test]
fn whitelist_and_containment_refusals_name_message_and_code() {
    let cases = [
        (
            candidate("/Volumes/other/.rch-target-contabo-2", EntryKind::Directory),
            None,
            "OUTSIDE_BASE",
        ),
        (
            candidate("/var/lib/rch/project", EntryKind::Directory),
            None,
            "NOT_WHITELISTED",
        ),
        (
            candidate(
                "/var/lib/rch/../escape/.rch-target",
                EntryKind::Directory,
            ),
            None,
            "PARENT_COMPONENT",
        ),
        (
            candidate(
                "/var/lib/rch/.rch-target-escape",
                EntryKind::Symlink,
            ),
            Some(Path::new("/Volumes/other")),
            "SYMLINK_ESCAPE",
        ),
    ];
    for (candidate, resolved, reason) in cases {
        let refusal = validate_candidate(Path::new("/var/lib/rch"), candidate, resolved)
            .expect_err("known-bad candidate must refuse");
        assert_eq!(refusal.exit_code(), 1);
        let message = refusal.to_string();
        assert!(message.contains("RECLAIM_REFUSED"), "{message}");
        assert!(message.contains(reason), "{message}");
        println!(
            "KNOWN_BAD exit={} reason={} message={message}",
            refusal.exit_code(),
            reason
        );
    }
}

#[test]
fn dual_authority_matrix_requires_agreement_before_deletion() {
    let clear = clear_control();
    assert!(clear.matches_worker(worker()));
    let wrong_host = ControlSnapshot {
        worker_host: "wrong-host".to_owned(),
        ..clear.clone()
    };
    assert!(!wrong_host.matches_worker(worker()));
    assert_eq!(
        decide_guards(&clear, &RemoteProcessObservation::Empty),
        GuardDecision::Authorized
    );
    let busy = ControlSnapshot {
        used_slots: 2,
        active_builds: vec![ActiveBuild {
            id: "build-1".to_owned(),
            worker_id: worker().id.to_owned(),
        }],
        ..clear.clone()
    };
    assert!(matches!(
        decide_guards(
            &busy,
            &RemoteProcessObservation::Present {
                lines: vec!["123 cargo".to_owned()]
            }
        ),
        GuardDecision::SkippedLiveBuild { .. }
    ));
    assert!(matches!(
        decide_guards(
            &clear,
            &RemoteProcessObservation::Present {
                lines: vec!["123 rustc".to_owned()]
            }
        ),
        GuardDecision::Unknown { .. }
    ));
    assert!(matches!(
        decide_guards(&busy, &RemoteProcessObservation::Empty),
        GuardDecision::Unknown { .. }
    ));
}

#[test]
fn control_plane_status_requires_worker_and_both_live_fields() {
    let json = br#"{
        "success": true,
        "data": {
            "daemon": {
                "workers": [{"id":"contabo-2","host":"94.72.121.46","status":"healthy","used_slots":0}],
                "active_builds": []
            }
        }
    }"#;
    let parsed = probe::parse_control_snapshot(json, worker()).expect("status");
    assert!(parsed.is_clear());
    let missing = br#"{"success":true,"data":{"daemon":{"workers":[]}}}"#;
    let error = probe::parse_control_snapshot(missing, worker()).expect_err("missing worker");
    assert!(error.to_string().contains("worker contabo-2 absent"));
    let malformed = br#"{"success":true,"data":{"daemon":{"workers":[{"id":"contabo-2","host":"94.72.121.46","status":"healthy"}],"active_builds":[]}}}"#;
    let error = probe::parse_control_snapshot(malformed, worker()).expect_err("missing slots");
    assert!(error.to_string().contains("missing used_slots"));
}

#[test]
fn candidate_listing_distinguishes_empty_from_malformed() {
    assert!(matches!(parse_listing("\n  \n"), Ok(CandidateSet::Empty)));
    let parsed = parse_listing("/var/lib/rch/.rch-tmp\td\t17\n").expect("valid listing");
    assert!(matches!(parsed, CandidateSet::NonEmpty(rows) if rows.len() == 1));
    let error =
        parse_listing("/var/lib/rch/.rch-tmp\tq\t17\n").expect_err("unknown entry type");
    assert!(error.to_string().contains("unsupported entry type"));
    let error = parse_listing("/var/lib/rch/.rch-tmp\td\n").expect_err("truncated row");
    assert!(error.to_string().contains("expected path"));
}

#[test]
fn report_serializes_mode_outcome_and_empty_candidate_detail() {
    let mut report = ReclaimReport::new(worker(), ReclaimMode::DryRun);
    report.outcome = RunOutcome::AlreadyClean;
    report.detail = "EMPTY_CANDIDATE_SET: no whitelisted artifact entries under base".to_owned();
    let json = serde_json::to_string(&report).expect("report JSON");
    assert!(json.contains("contabo-reclaim/report-v1"));
    assert!(json.contains("dry-run"));
    assert!(json.contains("ALREADY_CLEAN"));
    assert!(json.contains("EMPTY_CANDIDATE_SET"));
}

#[test]
fn operator_binary_is_a_reachable_dry_run_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_contabo-reclaim"))
        .arg("--help")
        .output()
        .expect("contabo-reclaim binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "help failed: {output:?}");
    assert!(stdout.contains("contabo-reclaim --worker"), "{stdout}");
    let duplicate = Command::new(env!("CARGO_BIN_EXE_contabo-reclaim"))
        .args([
            "--worker",
            "contabo-2",
            "--worker",
            "contabo-3",
            "--base",
            "/var/lib/rch",
        ])
        .output()
        .expect("duplicate worker invocation");
    let stderr = String::from_utf8_lossy(&duplicate.stderr);
    assert_eq!(
        duplicate.status.code(),
        Some(2),
        "duplicate worker status: {duplicate:?}"
    );
    assert!(stderr.contains("ONE_WORKER_REQUIRED"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn symlink_inside_base_is_distinguished_from_escape() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().expect("temporary base");
    let target = root.path().join("target");
    std::fs::create_dir(&target).expect("target directory");
    let link = root.path().join(".rch-target-link");
    symlink(&target, &link).expect("symlink");
    let resolved = std::fs::canonicalize(&link).expect("resolve link");
    let accepted = validate_candidate(
        root.path(),
        candidate(link.to_str().expect("link utf8"), EntryKind::Symlink),
        Some(&resolved),
    )
    .expect("in-base symlink");
    assert_eq!(accepted.rule, contabo_reclaim::WhitelistRule::RchTargetPool);
    let outside_root = tempfile::tempdir().expect("temporary outside root");
    let outside = outside_root.path().join("outside");
    std::fs::create_dir(&outside).expect("outside directory");
    let escaping = root.path().join(".rch-target-escape");
    symlink(&outside, &escaping).expect("escaping symlink");
    let refusal = validate_candidate(
        root.path(),
        candidate(
            escaping.to_str().expect("escaping utf8"),
            EntryKind::Symlink,
        ),
        Some(&outside),
    )
    .expect_err("escaping symlink");
    assert_eq!(
        refusal.reason,
        contabo_reclaim::RefusalReason::SymlinkEscape
    );
}
