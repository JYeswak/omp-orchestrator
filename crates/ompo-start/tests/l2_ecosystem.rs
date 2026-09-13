#![forbid(unsafe_code)]

//! Named L2 target for the S1 layer gate.
//!
//! This target exercises the real inception writer and readback contract with an isolated
//! repository fixture. It is invoked directly as Cargo's `--test l2_ecosystem` target.

use ompo_start::inception::{
    initialize, list_backups, read_inception, restore_backup, InceptionError,
    PROJECT_AGENTS_OWNERSHIP_STAMP, SCHEMA_VERSION,
};
use std::path::Path;
use std::process::Command;
use serde_json::Value;
use std::time::Duration;
use tempfile::TempDir;
use lifecycle_event::{
    default_repo_journal, DurableJournal, EmitOutcome, Layer, LifecycleEvent, ReasonCode,
};
use lifecycle_monitor::{gate_claimed_write_readback, observe_layer, verify_artifact, LayerState};
fn run_git(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(10)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {}
        other => panic!("git fixture command failed: {other:?}"),
    }
}

fn write_project_agents_stamp(root: &Path) {
    let stamp = format!("fixture {}\n", PROJECT_AGENTS_OWNERSHIP_STAMP);
    assert!(!stamp.trim().is_empty(), "canonical cbl7 project-agent stamp");
    std::fs::write(root.join("AGENTS.md"), stamp).expect("stamped AGENTS.md");
}

fn repository_fixture() -> TempDir {
    let directory = tempfile::tempdir().expect("fixture directory");
    std::fs::create_dir(directory.path().join("docs")).expect("docs directory");
    for name in ["CLAUDE.md", "Cargo.toml", "README.md", "SCHEMAS.toml"] {
        std::fs::write(directory.path().join(name), b"fixture\n").expect("control file");
    }
    write_project_agents_stamp(directory.path());
    // zb2p companion: the gated trust entry requires a ready tracker
    // (bead zb2p). Initialize `.beads` here so gated legs measure their
    // own gate, not the tracker gate.
    std::fs::create_dir(directory.path().join(".beads")).expect("beads dir");
    std::fs::write(
        directory.path().join(".beads/issues.jsonl"),
        "{\"id\":\"fixture-0001\",\"title\":\"fixture\"}\n",
    )
    .expect("beads issues");
    std::fs::write(directory.path().join("docs/decisions.jsonl"), b"{}\n")
        .expect("decision ledger");
    run_git(directory.path(), &["init", "-q"]);
    run_git(directory.path(), &["add", "."]);
    run_git(
        directory.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    directory
}

#[test]
fn l2_named_target_initializes_and_reads_back_identity() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("first init");
    assert_eq!(first.actions, 1, "first init must write the artifact");
    assert_eq!(first.manifest.schema_version, SCHEMA_VERSION);
    assert_eq!(first.manifest.repo_identity.git_marker, "directory");
    assert!(!first.manifest.project_id.is_empty());
    assert!(!first.manifest.repo_identity.source_revision.is_empty());
    assert!(!first.manifest.repo_identity.host_identity.is_empty());
    assert_eq!(first.manifest.trust_status.reason_code, "TRUST_DECISION_REQUIRED");

    let readback = read_inception(&output).expect("inception readback");
    assert_eq!(readback.project_id, first.manifest.project_id);
    assert_eq!(readback.repo_identity, first.manifest.repo_identity);
    assert!(readback.control_files_complete);

    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "unchanged init must be idempotent");
}

#[cfg(unix)]
#[test]
fn equivalent_symlink_paths_share_identity() {
    let repository = repository_fixture();
    let alias_root = tempfile::tempdir().expect("alias parent");
    let alias = alias_root.path().join("repo-alias");
    std::os::unix::fs::symlink(repository.path(), &alias).expect("repo symlink");
    let first = initialize(
        repository.path(),
        &repository.path().join(".omp-orchestrator/inception.json"),
    )
    .expect("real path init");
    let second = initialize(
        &alias,
        &alias.join(".omp-orchestrator/inception.json"),
    )
    .expect("symlink path init");
    assert_eq!(first.manifest.project_id, second.manifest.project_id);
    assert_eq!(first.manifest.repo_identity, second.manifest.repo_identity);
}

#[test]
fn readback_refuses_empty_and_missing_identity_fields() {
    let fields = [
        ("project_id", false),
        ("canonical_path", true),
        ("source_revision", true),
        ("git_marker", true),
        ("host_identity", true),
    ];
    for (field, nested) in fields {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        initialize(repository.path(), &output).expect("write inception");
        let mut value: Value = serde_json::from_str(
            &std::fs::read_to_string(&output).expect("read inception"),
        )
        .expect("valid JSON");
        if nested {
            value
                .get_mut("repo_identity")
                .and_then(Value::as_object_mut)
                .expect("repo identity object")
                .insert(field.to_owned(), Value::String(String::new()));
        } else {
            value
                .as_object_mut()
                .expect("manifest object")
                .remove(field);
        }
        std::fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
            .expect("write mutation");
        let error = read_inception(&output).expect_err("identity omission must refuse");
        match (field, error) {
            ("project_id", InceptionError::ReadbackMissingKey { key, .. }) => {
                assert_eq!(key, "project_id");
            }
            (field, InceptionError::ReadbackEmpty { key, .. }) => {
                assert_eq!(key, format!("repo_identity.{field}"));
            }
            (field, error) => panic!("wrong typed refusal for {field}: {error}"),
        }
    }
}

#[test]
fn readback_refuses_empty_or_missing_manifest_objects() {
    for contents in ["", "{}"] {
        let repository = repository_fixture();
        let output = repository.path().join(".omp-orchestrator/inception.json");
        std::fs::create_dir_all(output.parent().expect("artifact parent")).expect("parent");
        std::fs::write(&output, contents).expect("write malformed artifact");
        let error = read_inception(&output).expect_err("empty manifest must refuse");
        if contents.is_empty() {
            assert!(matches!(error, InceptionError::ReadbackMalformed { .. }));
        } else {
            assert!(matches!(
                error,
                InceptionError::ReadbackMissingKey { key, .. } if key == "schema_version"
            ));
        }
    }

    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    initialize(repository.path(), &output).expect("write inception");
    let mut value: Value = serde_json::from_str(
        &std::fs::read_to_string(&output).expect("read inception"),
    )
    .expect("valid JSON");
    value
        .as_object_mut()
        .expect("manifest object")
        .insert("repo_identity".to_owned(), Value::Null);
    std::fs::write(&output, serde_json::to_vec_pretty(&value).expect("encode mutation"))
        .expect("write mutation");
    let error = read_inception(&output).expect_err("null repo identity must refuse");
    assert!(matches!(
        error,
        InceptionError::ReadbackWrongType { key, expected, found, .. }
            if key == "repo_identity" && expected == "object" && found == "null"
    ));
}

#[test]
fn nonexistent_root_refuses_canonicalization() {
    let root = tempfile::tempdir().expect("root parent").path().join("missing");
    let output = root.join(".omp-orchestrator/inception.json");
    let error = initialize(&root, &output).expect_err("nonexistent root must refuse");
    assert!(matches!(error, InceptionError::RepositoryUnreadable { .. }));
    assert!(error.to_string().contains("INCEPTION_REPOSITORY_UNREADABLE"));
}
#[test]
fn l2_named_target_refuses_missing_control_files() {
    let repository = repository_fixture();
    std::fs::remove_file(repository.path().join("SCHEMAS.toml")).expect("remove control file");
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let error = initialize(repository.path(), &output).expect_err("missing control file must refuse");
    assert!(matches!(error, InceptionError::MissingControlFiles(_)));
    assert!(!output.exists(), "refused init must not write the artifact");
    std::fs::write(repository.path().join("SCHEMAS.toml"), b"fixture\n")
        .expect("restore control file");
    std::fs::remove_dir_all(repository.path().join(".git")).expect("remove git");
    let identity_error = initialize(repository.path(), &output)
        .expect_err("non-git identity must refuse");
    assert!(matches!(
        identity_error,
        InceptionError::IdentityUnavailable {
            field: "source_revision",
            ..
        }
    ));
}

/// Typed scan for the one S1.L2 row the init write+reprobe chokepoint owes.
///
/// ANTI-VACUITY, and the reason this is a typed refusal rather than an
/// `Option`: zero rows is an ERROR, never a pass. The exit polarity is NOT
/// invented here -- it reuses lifecycle-monitor's existing authority
/// (`error_exit_code`, crates/lifecycle-monitor/src/main.rs:204): an absent or
/// empty scan exits 2, a real content violation exits 1. A second convention
/// would let the two disagree about what "nothing was written" means, which is
/// the collapse this layer exists to prevent.
#[derive(Debug, PartialEq, Eq)]
struct L2ScanRefusal {
    reason_code: &'static str,
    exit_code: u8,
    detail: String,
}

impl std::fmt::Display for L2ScanRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} exit={} {}",
            self.reason_code, self.exit_code, self.detail
        )
    }
}

const L2_EMIT_STAGE_TO: &str = "S1.L2";

/// Every S1.L2 row in `journal`, or a typed refusal naming the missing stage.
///
/// Blank lines are skipped rather than parsed: JSONL tolerates a trailing
/// newline, and treating one as an unparseable row would report a FORMAT fault
/// where the real answer is "the row is absent".
fn scan_s1_l2_rows(journal: &Path) -> Result<Vec<Value>, L2ScanRefusal> {
    let text = std::fs::read_to_string(journal).map_err(|error| L2ScanRefusal {
        reason_code: "L2_SCAN_UNREADABLE_JOURNAL",
        exit_code: 2,
        detail: format!(
            "journal={} stage_to={L2_EMIT_STAGE_TO} detail={error}",
            journal.display()
        ),
    })?;
    let mut rows = Vec::new();
    let mut scanned = 0_usize;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        scanned += 1;
        let value: Value = serde_json::from_str(line).map_err(|error| L2ScanRefusal {
            reason_code: "L2_SCAN_UNPARSEABLE_ROW",
            exit_code: 1,
            detail: format!("journal={} detail={error}", journal.display()),
        })?;
        if value.get("stage_to").and_then(Value::as_str) == Some(L2_EMIT_STAGE_TO) {
            rows.push(value);
        }
    }
    if rows.is_empty() {
        return Err(L2ScanRefusal {
            reason_code: "L2_SCAN_ZERO_S1_L2_ROWS",
            exit_code: 2,
            detail: format!(
                "journal={} stage_to={L2_EMIT_STAGE_TO} missing rows_scanned={scanned}",
                journal.display()
            ),
        });
    }
    Ok(rows)
}

/// The shape the chokepoint's row must carry.
///
/// A row that EXISTS with the wrong content is a real violation and exits 1 --
/// distinct from absence's 2. Without the distinction a grader cannot tell "the
/// writer was suppressed" from "the writer wrote the wrong thing", and those
/// need different repairs.
fn require_init_row_shape(row: &Value) -> Result<(), L2ScanRefusal> {
    for (key, expected) in [
        ("layer", "L2"),
        ("stage_from", "S1.L1"),
        ("stage_to", L2_EMIT_STAGE_TO),
        ("actor", "ompo-init"),
        ("outcome", "emitted"),
        ("reason_code", "INIT_REPROBE_OK"),
    ] {
        let found = row.get(key).and_then(Value::as_str).unwrap_or("<absent>");
        if found != expected {
            return Err(L2ScanRefusal {
                reason_code: "L2_SCAN_ROW_CONTENT_MISMATCH",
                exit_code: 1,
                detail: format!("key={key} expected={expected} found={found}"),
            });
        }
    }
    Ok(())
}

/// y7pq positive leg: the init write+reprobe chokepoint -- `initialize_inner`'s
/// `emit_init_event` call, which sits AFTER `write_atomic` and AFTER the
/// `read_inception` re-probe in crates/ompo-start/src/inception.rs -- emits
/// exactly ONE S1.L2 LifecycleEvent, with before/after evidence either side of
/// the single call.
#[test]
fn l2_init_chokepoint_emits_one_lifecycle_row_with_before_after_evidence() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());

    // BEFORE: no journal at all, pinned as a typed absence on BOTH message and
    // exit so the leg cannot pass by the journal being unreadable for some
    // unrelated reason.
    let before = scan_s1_l2_rows(&journal).expect_err("no journal before init");
    assert_eq!(
        before.reason_code, "L2_SCAN_UNREADABLE_JOURNAL",
        "before: {before}"
    );
    assert_eq!(before.exit_code, 2, "before: {before}");

    let report = initialize(repository.path(), &output).expect("accepted init");

    // AFTER: exactly one row, carrying the chokepoint's shape.
    let after = scan_s1_l2_rows(&journal).expect("one S1.L2 row after init");
    assert_eq!(after.len(), 1, "one row per chokepoint pass");
    require_init_row_shape(&after[0]).expect("chokepoint row shape");

    // RE-PROBE evidence: the writer's own readback count and the monitor's
    // INDEPENDENT re-read of the same journal agree. `journal_rows` comes from
    // the emit readback, `monitor_rows` from lifecycle_monitor::verify_artifact;
    // equal counts are what makes the re-probe an observation rather than a
    // restatement of the write's return code.
    assert_eq!(report.journal_rows, 1, "emit readback saw one row");
    assert_eq!(report.monitor_rows, 1, "monitor re-read saw one row");
}

/// y7pq KNOWN-BAD leg: suppress the event writer and the missing row is
/// DETECTED, naming the stage_to it owed.
///
/// Pins BOTH the message and the exit code: a message-only assertion survives
/// an exit-code collapse, and a code-only assertion survives an unrelated 101.
#[test]
fn l2_suppressed_event_writer_is_detected_naming_the_missing_stage_to() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());
    initialize(repository.path(), &output).expect("accepted init");

    let emitted = scan_s1_l2_rows(&journal).expect("row exists before suppression");
    let mut foreign = emitted[0].clone();
    foreign["layer"] = Value::String("L5".to_owned());
    foreign["stage_to"] = Value::String("S1.L5".to_owned());

    // Suppression modelled on disk: the L2 row is gone and a foreign-layer row
    // remains, so the refusal proves "the L2 row is missing" rather than "the
    // file is empty". Those are different failures and an empty-file check
    // would conflate them.
    std::fs::write(
        &journal,
        format!("{}\n", serde_json::to_string(&foreign).expect("encode foreign row")),
    )
    .expect("suppressed journal");

    let refusal = scan_s1_l2_rows(&journal).expect_err("suppressed writer must be detected");
    assert_eq!(refusal.reason_code, "L2_SCAN_ZERO_S1_L2_ROWS", "{refusal}");
    assert_eq!(refusal.exit_code, 2, "{refusal}");
    assert!(
        refusal.detail.contains("stage_to=S1.L2"),
        "refusal must name the missing stage_to: {refusal}"
    );
    assert!(
        refusal.detail.contains("rows_scanned=1"),
        "refusal must show the journal was non-empty: {refusal}"
    );
}

/// ANTI-VACUITY, measured rather than asserted in prose: absence and a real
/// content violation carry DIFFERENT exits, and the untouched row still passes
/// so the check is not over-strict.
#[test]
fn absent_row_and_content_violation_carry_distinct_exit_codes() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());
    initialize(repository.path(), &output).expect("accepted init");
    let row = scan_s1_l2_rows(&journal).expect("emitted row")[0].clone();

    // KNOWN-GOOD: the row the chokepoint actually wrote passes untouched.
    require_init_row_shape(&row).expect("known-good row must pass");

    let mut tampered = row.clone();
    tampered["reason_code"] = Value::String("INIT_REPROBE_LIED".to_owned());
    let violation = require_init_row_shape(&tampered).expect_err("content violation");
    assert_eq!(
        violation.reason_code, "L2_SCAN_ROW_CONTENT_MISMATCH",
        "{violation}"
    );
    assert_eq!(violation.exit_code, 1, "{violation}");
    assert!(
        violation.detail.contains("expected=INIT_REPROBE_OK"),
        "violation must name the reason_code it expected: {violation}"
    );

    std::fs::remove_file(&journal).expect("remove journal");
    let absent = scan_s1_l2_rows(&journal).expect_err("absent journal");
    assert_ne!(
        absent.exit_code, violation.exit_code,
        "absence must not share a real violation's exit: {absent} vs {violation}"
    );
}

/// The keys SCHEMAS.toml:158 declares required for the inception artifact.
///
/// Restated from the DECLARATION rather than imported from the writer's private
/// `REQUIRED_KEYS`: if the two ever disagree, this leg is the one that should
/// redden. A test that imports the writer's own constant proves only that the
/// writer agrees with itself.
const DECLARED_REQUIRED_KEYS: &[&str] = &[
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "trust_status",
];

/// 0pc9 MONITOR obligation: the re-probe happens after EVERY write, including
/// the write that turns out to be a no-op, and zero second-run actions are
/// earned ONLY by an identical hash.
///
/// The obligation is on the CALLER, so the leg watches the caller: it is not
/// enough that a probe function exists. Three passes are measured -- a real
/// write, an identical no-op, and a changed hash -- and the monitor's
/// INDEPENDENT re-read advances on all three. A re-probe that only ran when
/// bytes changed would leave the idempotent path unobserved, which is the path
/// that runs every time after the first.
#[test]
fn l2_reprobe_follows_every_write_and_only_identical_hashes_are_zero_action() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    let journal = default_repo_journal(repository.path());

    let first = initialize(repository.path(), &output).expect("first init");
    assert_eq!(first.actions, 1, "first pass must write the artifact");
    assert_eq!(
        first.monitor_rows,
        verify_artifact(&journal).expect("independent re-read"),
        "the monitor count must be re-derivable by a third party"
    );

    // Identical bytes: zero ARTIFACT actions, but the re-probe still ran.
    let second = initialize(repository.path(), &output).expect("second init");
    assert_eq!(second.actions, 0, "identical hash earns zero actions");
    assert_eq!(second.backup, None, "a no-op write must not snapshot");
    assert_eq!(
        second.monitor_rows,
        first.monitor_rows + 1,
        "the re-probe must run after the no-op write too"
    );

    // Changed hash: the second run is NOT zero-action, and the prior bytes are
    // preserved. Repair, never a silent overwrite.
    std::fs::write(&output, b"tampered\n").expect("change the artifact hash");
    let third = initialize(repository.path(), &output).expect("third init");
    assert_eq!(third.actions, 1, "a changed hash must not be zero-action");
    let backup = third.backup.expect("a changed hash must snapshot first");
    assert_eq!(
        std::fs::read(&backup).expect("backup bytes"),
        b"tampered\n",
        "the snapshot must hold the bytes that were replaced"
    );
    assert_eq!(
        third.monitor_rows,
        second.monitor_rows + 1,
        "the re-probe must run after the repairing write"
    );

    // The freshness-bearing read of the same journal: an observation with an
    // AGE, never a bare boolean.
    let verdict = observe_layer(&journal, Layer::L2, 600_000).expect("L2 observation");
    assert_eq!(verdict.state, LayerState::Progressing);
    assert_eq!(verdict.row_count, 3, "one row per chokepoint pass");
    assert_eq!(verdict.last_reason, "INIT_REPROBE_OK");
    assert!(verdict.fresh, "a just-written row cannot be stale");
}

/// 0pc9 KNOWN-BAD: skip the re-probe and success is REFUSED -- never granted by
/// the write's return code.
///
/// `gate_claimed_write_readback` is the production gate for exactly this: it
/// takes a write that CLAIMS to have happened and demands the artifact prove
/// it. The claimed row is never written here, so a caller that trusted its own
/// return code would report success over an empty journal.
#[test]
fn l2_write_claimed_without_a_reprobe_is_refused_not_believed() {
    let directory = tempfile::tempdir().expect("journal fixture");
    let journal_path = directory.path().join("claimed.jsonl");
    let journal = DurableJournal::open(&journal_path).expect("open journal");
    let claimed = LifecycleEvent::new(
        Layer::L2,
        "S1.L1",
        "S1.L2",
        "ompo-init",
        EmitOutcome::Emitted,
        ReasonCode::new("INIT_REPROBE_OK").expect("non-empty reason code"),
    );

    let refusal = gate_claimed_write_readback(&journal, std::slice::from_ref(&claimed))
        .expect_err("a claimed write with no artifact must refuse");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("READBACK"),
        "refusal must name the readback it could not make: {rendered}"
    );

    // KNOWN-GOOD, so the gate is not merely always-refusing: once the row is
    // really on disk the same claim passes.
    lifecycle_event::emit_one_host(&journal, claimed.clone()).expect("real emit");
    gate_claimed_write_readback(&journal, std::slice::from_ref(&claimed))
        .expect("a real write must pass the same gate");
}

/// up37 ARTIFACT-with-readback: an accepted init writes repo_identity,
/// control_files and trust_status, snapshots the bytes it replaces, and every
/// declared field survives a READ BACK from disk.
///
/// The readback is the acceptance. A write that returned success is not
/// evidence, so nothing here trusts `InitReport` alone -- each field is re-read
/// off the file, and the backup is verified by re-hashing its bytes against the
/// SHA in its own filename.
#[test]
fn l2_accepted_init_writes_backups_and_reads_back_every_declared_field() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");

    let first = initialize(repository.path(), &output).expect("accepted init");
    assert!(
        list_backups(&output).expect("backup listing").is_empty(),
        "a first write replaces nothing and must not invent a snapshot"
    );

    // Replace the artifact so the write path has something to snapshot.
    std::fs::write(&output, b"superseded\n").expect("supersede artifact");
    initialize(repository.path(), &output).expect("replacing init");

    let backups = list_backups(&output).expect("backup listing");
    assert_eq!(backups.len(), 1, "one replacement, one content-keyed backup");
    assert_eq!(
        std::fs::read(&backups[0].path).expect("backup bytes"),
        b"superseded\n"
    );
    restore_backup(&output, &backups[0]).expect("an intact backup must restore");

    // Re-run so the artifact on disk is the manifest again, then READ BACK.
    let restored = initialize(repository.path(), &output).expect("post-restore init");
    let readback = read_inception(&output).expect("readback from disk");
    assert_eq!(readback.repo_identity, restored.manifest.repo_identity);
    assert_eq!(readback.project_id, first.manifest.project_id);
    assert!(
        readback.control_files_complete,
        "control_files must read back complete"
    );
    assert_eq!(
        restored.manifest.trust_status.reason_code, "TRUST_DECISION_REQUIRED",
        "trust_status must carry an explicit decision, not a default"
    );

    let on_disk: Value =
        serde_json::from_str(&std::fs::read_to_string(&output).expect("artifact text"))
            .expect("artifact is JSON");
    for key in DECLARED_REQUIRED_KEYS {
        assert!(
            on_disk.get(*key).is_some(),
            "declared required key {key} missing from the artifact on disk"
        );
    }
}

/// up37 KNOWN-BAD: damage the BACKUP before it is read back and the restore is
/// refused, naming the integrity failure.
///
/// The field arm of this acceptance is already covered by
/// `readback_refuses_empty_and_missing_identity_fields`. This is the backup
/// arm, and it is the one that matters for a restore: a corrupted backup
/// restored silently would leave the operator believing the artifact had been
/// recovered when it had been destroyed.
#[test]
fn l2_damaged_backup_refuses_restore_instead_of_recovering_garbage() {
    let repository = repository_fixture();
    let output = repository.path().join(".omp-orchestrator/inception.json");
    initialize(repository.path(), &output).expect("accepted init");
    std::fs::write(&output, b"superseded\n").expect("supersede artifact");
    initialize(repository.path(), &output).expect("replacing init");

    let backups = list_backups(&output).expect("backup listing");
    assert_eq!(backups.len(), 1);
    let entry = backups[0].clone();

    // KNOWN-GOOD first: intact, it restores. Without this the refusal below
    // could be satisfied by a restore path that never works at all.
    let good = restore_backup(&output, &entry).expect("intact backup restores");
    assert_eq!(good.actions, 1);
    assert_eq!(good.restored_from, Some(entry.path.clone()));

    // Now damage the backup's CONTENT while leaving the SHA in its filename.
    std::fs::write(&entry.path, b"not the bytes the name claims\n").expect("damage backup");
    let refusal =
        restore_backup(&output, &entry).expect_err("a damaged backup must refuse to restore");
    let rendered = refusal.to_string();
    assert!(
        rendered.contains("backup integrity failed"),
        "refusal must name the integrity failure: {rendered}"
    );
    assert!(
        rendered.contains(&entry.content_sha),
        "refusal must name the SHA the filename claimed: {rendered}"
    );

    // And a missing backup is a DIFFERENT failure from a damaged one.
    std::fs::remove_file(&entry.path).expect("remove backup");
    let missing = restore_backup(&output, &entry).expect_err("a missing backup must refuse");
    assert!(
        missing.to_string().contains("backup read failed"),
        "missing and damaged must not collapse into one message: {missing}"
    );
}

/// L2-TEST-GIT-REPO (contract s1_l2_ecosystem.md `L2-BUILD-GIT-REPO`):
/// `git_repo_toplevel` yields the canonical top-level path for a real
/// repository and a typed halt with remediation outside one. Uses the
/// real production check -- never a copy of its arms: a copy would agree
/// with the subject by construction.
///
/// Upward-search note: git resolves parent checkouts, so the production
/// check ceilings the search at the argument's parent (per-spawn env,
/// thread-safe, no process-global games): a real repository carries its
/// own `.git`, found before any ascent, while a bare directory inside a
/// checkout then reads 128 deterministically on every lane.
///
/// KNOWN-BAD: invert the mapping (a refusal reads as identity) and the
/// halt arm fails: a bare tempdir would certify as a repository.
/// Message AND exit are pinned on the mutation run.
#[test]
fn non_repo_halts() {
    use ompo_start::inception::git_repo_toplevel;
    // Healthy: a real repository yields its canonical top level.
    let repository = repository_fixture();
    let top = git_repo_toplevel(repository.path()).expect("real repo resolves");
    assert_eq!(
        top,
        repository.path().canonicalize().expect("canonical"),
        "healthy branch yields the canonical top level"
    );
    // Halt: outside git, typed halt with remediation, never a guessed path.
    let bare = tempfile::tempdir().expect("bare fixture");
    let error = git_repo_toplevel(bare.path()).expect_err("non-repo must halt");
    let text = error.to_string();
    assert!(
        text.starts_with("INCEPTION_IDENTITY_UNAVAILABLE"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
}

/// L2-TEST-REMOTE-PERSONA-A (bead nqac): Persona A with no git remote
/// records an explicit `remote_optional=true` allowance and continues;
/// absence is never a silent universal success. Uses the real
/// `persona_remote_policy` over live `git remote -v` observations --
/// never a copy of its arms. (No Persona B/C variants exist in this
/// tree; non-Persona-A cells cover that population by construction.)
///
/// KNOWN-BAD: drop the Persona A allowance (never optional) and the
/// no-remote Persona A cell fails: a local-only subject would halt
/// where the contract grants continuation. Message AND exit are pinned
/// on the mutation run.
#[test]
fn persona_a_local_only_remote_rule() {
    use ompo_start::inception::{persona_remote_policy, PersonaRemote};
    // No remote anywhere here: repository_fixture never adds one, so the
    // no-remote cells observe a real absence, not an injected boolean.
    let repository = repository_fixture();
    // Persona A, no remote: explicit allowance with continuation.
    let allowed = persona_remote_policy(repository.path(), true).expect("policy answers");
    assert_eq!(
        allowed,
        PersonaRemote {
            remote_optional: true,
            remote_present: false,
            reason_code: "PERSONA_A_LOCAL_ONLY",
        },
        "Persona A with no remote must carry the explicit allowance"
    );
    // Non-Persona-A, no remote: restrictive, unchanged by this rule.
    let required = persona_remote_policy(repository.path(), false).expect("policy answers");
    assert_eq!(
        required.remote_optional, false,
        "a missing remote stays required off Persona A"
    );
    assert_eq!(
        required.reason_code, "REMOTE_REQUIRED",
        "the restrictive branch names its reason, got {}",
        required.reason_code
    );
    // Persona A WITH a remote: nothing to allow, still restrictive-shaped
    // (adding a remote needs no network: `remote -v` only reads config).
    run_git(
        repository.path(),
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    let present = persona_remote_policy(repository.path(), true).expect("policy answers");
    assert_eq!(
        present.remote_optional, false,
        "a present remote needs no allowance"
    );
}

/// L2-TEST-REMOTE-PERSONA-BC (bead mxro): a non-optional policy without
/// an observed remote halts shared dispatch with a named remediation;
/// every other record passes through. Reuses the nqac `PersonaRemote`
/// policy -- no second remote detector, no new persona variants (none
/// exist in this tree; non-Persona-A covers that population).
///
/// KNOWN-BAD: drop the halt (always continue) and the restrictive cell
/// below passes silently: a fleet subject with no remote would advance
/// with no evidence anything was required. Message AND exit are pinned
/// on the mutation run.
#[test]
fn persona_bc_missing_remote_halts_dispatch() {
    use ompo_start::inception::{persona_remote_policy, require_remote_for_dispatch};
    let repository = repository_fixture();
    // Allowance passes through: Persona A local-only continues.
    let allowed = persona_remote_policy(repository.path(), true).expect("policy answers");
    require_remote_for_dispatch(&allowed).expect("allowance continues");
    // Present remote passes through on every persona.
    run_git(
        repository.path(),
        &["remote", "add", "origin", "https://example.invalid/x.git"],
    );
    for persona_a in [true, false] {
        let present = persona_remote_policy(repository.path(), persona_a)
            .expect("policy answers");
        require_remote_for_dispatch(&present).expect("a present remote continues");
    }
    // Restrictive branch with nothing behind it: non-Persona-A, no
    // remote. Remove the remote again and require the named halt.
    run_git(repository.path(), &["remote", "remove", "origin"]);
    let required = persona_remote_policy(repository.path(), false).expect("policy answers");
    assert_eq!(
        required.remote_optional, false,
        "non-Persona-A without remote stays restrictive"
    );
    let error =
        require_remote_for_dispatch(&required).expect_err("missing remote must halt dispatch");
    let text = error.to_string();
    assert!(
        text.contains("REMOTE_REQUIRED"),
        "halt must name the required branch, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
}

/// L2-ENTRY-GIT-REPO (bead e0li rework): the L2 operator entry gates on
/// the repository check before downstream state continues. A real
/// repository proceeds through `initialize` (canonical root); a bare
/// directory halts typed with remediation and writes nothing -- no
/// artifact, no journal side effects from a flow that never started.
/// Uses the real `initialize_gated` entry, never a copy of its arms.
///
/// KNOWN-BAD: bypassing the gate (delegating straight to `initialize`)
/// greens the bare directory below and this leg fails: downstream state
/// would continue without a repository, which is the unwired adoption
/// this row exists to prevent. Message AND exit are pinned on the
/// mutation run.
#[test]
fn gated_entry_requires_git_repo() {
    use ompo_start::inception::initialize_gated;
    // Healthy: a real repository proceeds with actions recorded.
    let repository = repository_fixture();
    // qruz companion: the gated entry now also requires a stamped
    // CLAUDE.md. Stamp it so this leg keeps measuring the git gate,
    // not the stamp gate.
    std::fs::write(repository.path().join("CLAUDE.md"), b"fixture omp-orchestrator\n")
        .expect("stamped claude");
    let output = repository
        .path()
        .join(".omp-orchestrator/inception.json");
    let report = initialize_gated(repository.path(), &output).expect("real repo proceeds");
    assert!(
        output.is_file(),
        "a gated real repo must produce its artifact"
    );
    let _ = report;
    // Halt: a bare directory halts typed with remediation BEFORE any
    // downstream write -- the artifact must not exist afterwards.
    let bare = tempfile::tempdir().expect("bare fixture");
    let bare_output = bare.path().join(".omp-orchestrator/inception.json");
    let error = initialize_gated(bare.path(), &bare_output).expect_err("non-repo must halt");
    let text = error.to_string();
    assert!(
        text.starts_with("INCEPTION_IDENTITY_UNAVAILABLE"),
        "halt must be typed, got: {text}"
    );
    assert!(
        text.contains("remedy:"),
        "halt must carry remediation, got: {text}"
    );
    assert!(
        !bare_output.exists(),
        "halted entry must write nothing, found {}",
        bare_output.display()
    );
}

/// L2-TEST-AGENTS-STAMP (bead 43x7): the control-file probe reports
/// AGENTS.md stamp identity, live source revision, and a typed status.
/// Uses the real `agents_stamp_report` over real fixtures -- stamp
/// token referenced, never copied; no line count pinned anywhere.
///
/// KNOWN-BAD: blind the detector (never see the token) and the stamped
/// arms fail while the unstamped arms stay green: a missing stamp
/// would certify. Message AND exit are pinned on the mutation run.
#[test]
fn agents_stamp_reports_identity_revision_and_status() {
    use ompo_start::inception::{agents_stamp_report, AgentsStampStatus};
    fn live_head(repo: &std::path::Path) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse runs");
        assert!(
            output.status.success(),
            "fixture must be a live repository"
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
    // Healthy: stamped file in a live repo reports identity, live
    // revision, and Stamped status together.
    let repository = repository_fixture();
    let report = agents_stamp_report(repository.path());
    assert!(report.stamp_present, "the fixture token must be seen");
    assert_eq!(
        report.source_revision.as_deref(),
        Some(live_head(repository.path()).as_str()),
        "revision must track live HEAD, not a constant"
    );
    assert_eq!(
        report.status,
        AgentsStampStatus::Stamped,
        "stamped plus revision is Stamped"
    );
    // Revision disagreement: a new commit moves HEAD and the report
    // must track it, never the recorded value.
    run_git(
        repository.path(),
        &[
            "-c",
            "user.name=ompo-start-test",
            "-c",
            "user.email=ompo-start-test@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "second",
        ],
    );
    let moved = agents_stamp_report(repository.path());
    assert_ne!(
        moved.source_revision.as_deref(),
        report.source_revision.as_deref(),
        "the report must track HEAD across commits"
    );
    assert!(moved.stamp_present, "stamp survives unrelated commits");
    // Corrupt the stamp (token gone, file otherwise intact): identity
    // lost, revision still observed, status Unstamped.
    std::fs::write(repository.path().join("AGENTS.md"), b"foreign stuff\n")
        .expect("corruption lands");
    let corrupt = agents_stamp_report(repository.path());
    assert!(
        !corrupt.stamp_present,
        "a tokenless file must not certify"
    );
    assert!(
        corrupt.source_revision.is_some(),
        "revision observes independently of the stamp"
    );
    assert_eq!(
        corrupt.status,
        AgentsStampStatus::Unstamped,
        "tokenless is Unstamped"
    );
    // Stamped file with no usable git: a `.git` FILE pointing nowhere is
    // fatal locally, so no upward search can rescue it -- revision is
    // deterministically unknown on every lane, unlike a bare tempdir
    // (which resolves parent checkouts on some workers).
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(nogit.path().join("AGENTS.md"), b"fixture omp-orchestrator\n")
        .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/e0li\n")
        .expect("broken gitdir");
    let report = agents_stamp_report(nogit.path());
    assert!(
        report.stamp_present && report.source_revision.is_none(),
        "non-git stamp keeps identity without revision, got {report:?}"
    );
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-CLAUDE-STAMP (bead qruz): the reachable L2 trust-flow entry
/// admits a stamped CLAUDE.md and refuses every other stamp state
/// before trust-dependent continuation. Uses the real
/// `initialize_gated` entry end to end -- never a copy of its arms:
/// a copy would agree with the subject by construction.
///
/// KNOWN-BAD: bypass the stamp gate at the entry (proceed regardless)
/// and the restrictive arms below pass silently: unstamped trust would
/// advance with no evidence anything was required. Message AND exit
/// are pinned on the mutation run.
#[test]
fn claude_stamp_gates_trust_entry() {
    use ompo_start::inception::{claude_stamp_report, initialize_gated, AgentsStampStatus};
    // Healthy: stamped CLAUDE.md in a live repo reaches the trust
    // branch -- initialize runs and writes the artifact.
    let repository = repository_fixture();
    std::fs::write(repository.path().join("CLAUDE.md"), b"fixture omp-orchestrator\n")
        .expect("stamped claude");
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated.json");
    initialize_gated(repository.path(), &output).expect("stamped entry proceeds");
    assert!(
        output.exists(),
        "a trusted entry writes its artifact"
    );
    // Restrictive matrix: every non-Stamped state refuses typed before
    // initialize runs, with the file and the remedy named.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        ("empty", Box::new(|root| {
            std::fs::write(root.join("CLAUDE.md"), b"").expect("empty file");
        })),
        ("foreign", Box::new(|root| {
            std::fs::write(root.join("CLAUDE.md"), b"foreign stuff\n").expect("foreign file");
        })),
        ("unreadable", Box::new(|root| {
            // Hermetic: a previous arm may have left a directory here.
            let path = root.join("CLAUDE.md");
            if path.is_dir() {
                std::fs::remove_dir_all(&path).expect("clear dir");
            } else {
                let _ = std::fs::remove_file(&path);
            }
            std::fs::create_dir(&path).expect("directory mask");
        })),
        ("missing", Box::new(|root| {
            let path = root.join("CLAUDE.md");
            if path.is_dir() {
                std::fs::remove_dir_all(&path).expect("clear dir");
            } else {
                std::fs::remove_file(&path).expect("remove file");
            }
        })),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-{name}.json"));
        let error =
            initialize_gated(repository.path(), &output).expect_err("unstamped must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains("CLAUDE.md"),
            "{name} refusal must be typed and name the file, got: {text}"
        );
        assert!(
            text.contains("remedy:") || text.contains("stamp it"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
    // GitUnavailable is a report-level state: a stamped file with no
    // usable git observes identity without revision.
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(nogit.path().join("CLAUDE.md"), b"fixture omp-orchestrator\n")
        .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/qruz\n")
        .expect("broken gitdir");
    let report = claude_stamp_report(nogit.path());
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-AGENTS-STAMP (bead 43x7): the reachable L2 trust-flow entry
/// admits a stamped AGENTS.md and refuses every other AGENTS stamp state
/// before trust-dependent continuation. Reuses the shared stamp core and
/// the `initialize_gated` seam beside the CLAUDE report -- no duplicate
/// core, no second stamp vocabulary.
///
/// KNOWN-BAD: bypass the AGENTS report at the entry (proceed regardless)
/// and the restrictive arms below pass silently: unstamped trust would
/// advance with no evidence anything was required. Message AND exit
/// are pinned on the mutation run.
#[test]
fn agents_stamp_gates_trust_entry() {
    use ompo_start::inception::{agents_stamp_report, initialize_gated, AgentsStampStatus};
    // Healthy: stamped AGENTS.md and stamped CLAUDE.md in a live repo
    // reach the trust branch -- initialize runs and writes the artifact.
    // (The fixture stamps AGENTS.md; CLAUDE.md is stamped here so the
    // CLAUDE gate passes and this leg measures the AGENTS gate alone.)
    let repository = repository_fixture();
    std::fs::write(repository.path().join("CLAUDE.md"), b"fixture omp-orchestrator\n")
        .expect("stamped claude");
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated-agents.json");
    initialize_gated(repository.path(), &output).expect("stamped entry proceeds");
    assert!(
        output.exists(),
        "a trusted entry writes its artifact"
    );
    // Restrictive matrix: every non-Stamped AGENTS.md state refuses typed
    // before initialize runs, with the file and the remedy named. CLAUDE.md
    // stays stamped throughout, so each refusal is the AGENTS gate firing.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        ("empty", Box::new(|root| {
            std::fs::write(root.join("AGENTS.md"), b"").expect("empty file");
        })),
        ("foreign", Box::new(|root| {
            std::fs::write(root.join("AGENTS.md"), b"foreign stuff\n").expect("foreign file");
        })),
        ("corrupt", Box::new(|root| {
            std::fs::write(root.join("AGENTS.md"), b"\xff\xfe invalid \x00 bytes\n")
                .expect("corrupt file");
        })),
        ("unreadable", Box::new(|root| {
            // Hermetic: a previous arm may have left a directory here.
            let path = root.join("AGENTS.md");
            if path.is_dir() {
                std::fs::remove_dir_all(&path).expect("clear dir");
            } else {
                let _ = std::fs::remove_file(&path);
            }
            std::fs::create_dir(&path).expect("directory mask");
        })),
        ("missing", Box::new(|root| {
            let path = root.join("AGENTS.md");
            if path.is_dir() {
                std::fs::remove_dir_all(&path).expect("clear dir");
            } else {
                std::fs::remove_file(&path).expect("remove file");
            }
        })),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-agents-{name}.json"));
        let error =
            initialize_gated(repository.path(), &output).expect_err("unstamped must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains("AGENTS.md"),
            "{name} refusal must be typed and name the file, got: {text}"
        );
        assert!(
            text.contains("remedy:") || text.contains("stamp it"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
    // GitUnavailable is a report-level state: a stamped file with no
    // usable git observes identity without revision.
    let nogit = tempfile::tempdir().expect("no-git fixture");
    std::fs::write(nogit.path().join("AGENTS.md"), b"fixture omp-orchestrator\n")
        .expect("stamped non-repo file");
    std::fs::write(nogit.path().join(".git"), b"gitdir: /nonexistent/43x7r\n")
        .expect("broken gitdir");
    let report = agents_stamp_report(nogit.path());
    assert_eq!(
        report.status,
        AgentsStampStatus::GitUnavailable,
        "stamp without git is GitUnavailable"
    );
}

/// L2-TEST-BEADS (bead zb2p): the reachable L2 trust-flow entry admits a
/// ready tracker and refuses every other tracker state before
/// initialization or dispatch can continue. Consumes the read-only
/// [`beads_init_report`] probe at `initialize_gated` -- never a second
/// tracker client, and fixtures are isolated tempdirs, never the live
/// tracker. Project identity is the first row's id prefix (see the
/// reporter's NO-CLAIM); writability is permission-bit evidence.
///
/// KNOWN-BAD: bypass the beads report at the entry (proceed regardless)
/// and the restrictive arms below pass silently: uninitialized trust
/// would advance with no evidence anything was required. Message AND
/// exit are pinned on the mutation run.
#[test]
fn beads_init_gates_trust_entry() {
    use ompo_start::inception::{beads_init_report, initialize_gated, BeadsInitStatus};
    use std::os::unix::fs::PermissionsExt;
    // Healthy: stamped control files plus an initialized isolated
    // tracker reach the trust branch -- initialize runs, the artifact
    // lands, and the report names the fixture project identity.
    // (The fixture stamps AGENTS.md and initializes `.beads`; CLAUDE.md
    // is stamped here so the earlier gates pass and this leg measures
    // the tracker gate alone.)
    let repository = repository_fixture();
    std::fs::write(repository.path().join("CLAUDE.md"), b"fixture omp-orchestrator\n")
        .expect("stamped claude");
    let report = beads_init_report(repository.path());
    assert_eq!(report.status, BeadsInitStatus::Ready, "fixture tracker is ready");
    assert_eq!(
        report.project_identity.as_deref(),
        Some("fixture"),
        "report names the fixture project identity"
    );
    assert!(report.readable && report.writable, "ready means readable and writable");
    let output = repository
        .path()
        .join(".omp-orchestrator/init-gated-beads.json");
    initialize_gated(repository.path(), &output).expect("stamped entry proceeds");
    assert!(
        output.exists(),
        "a trusted entry writes its artifact"
    );
    // Restrictive matrix: every non-Ready tracker state refuses typed
    // before initialize runs, with the tracker path and the remedy
    // named. Control files stay stamped throughout, so each refusal is
    // the tracker gate firing.
    let cases: Vec<(&str, Box<dyn Fn(&std::path::Path)>)> = vec![
        ("missing", Box::new(|root| {
            let dir = root.join(".beads");
            if dir.is_dir() {
                std::fs::remove_dir_all(&dir).expect("remove beads dir");
            }
        })),
        ("uninitialized-empty", Box::new(|root| {
            // Hermetic: a previous arm may have removed the dir or left a
            // directory mask at the file path.
            let dir = root.join(".beads");
            if !dir.is_dir() {
                std::fs::create_dir(&dir).expect("recreate beads dir");
            }
            let file = dir.join("issues.jsonl");
            if file.is_dir() {
                std::fs::remove_dir_all(&file).expect("clear mask");
            }
            std::fs::write(&file, b"").expect("empty issues");
        })),
        ("uninitialized-absent", Box::new(|root| {
            let file = root.join(".beads/issues.jsonl");
            if file.is_dir() {
                std::fs::remove_dir_all(&file).expect("clear mask");
            } else {
                let _ = std::fs::remove_file(&file);
            }
        })),
        ("unreadable", Box::new(|root| {
            // (no chmod hazard): identity can never be established.
            let file = root.join(".beads/issues.jsonl");
            if !file.is_dir() {
                let _ = std::fs::remove_file(&file);
                std::fs::create_dir(&file).expect("directory mask");
            }
        })),
        ("unreadable-garbage", Box::new(|root| {
            // Valid UTF-8 with no usable id: initialized bytes, missing
            // project identity. Treated as unreadable -- identity cannot
            // be established either way.
            let file = root.join(".beads/issues.jsonl");
            if file.is_dir() {
                std::fs::remove_dir_all(&file).expect("clear mask");
            }
            std::fs::write(&file, b"not json at all\n").expect("garbage issues");
        })),
        ("unwritable", Box::new(|root| {
            // Permission-bit evidence, read -- never an access proof, so
            // this holds for every uid including root (see reporter).
            let file = root.join(".beads/issues.jsonl");
            if file.is_dir() {
                std::fs::remove_dir_all(&file).expect("clear mask");
            }
            std::fs::write(&file, "{\"id\":\"fixture-0001\"}\n").expect("restore issues");
            let mut permissions = std::fs::metadata(&file).expect("metadata").permissions();
            permissions.set_mode(0o444);
            std::fs::set_permissions(&file, permissions).expect("deny write bits");
        })),
    ];
    for (name, arrange) in cases {
        arrange(repository.path());
        let output = repository
            .path()
            .join(format!(".omp-orchestrator/init-gated-beads-{name}.json"));
        let error =
            initialize_gated(repository.path(), &output).expect_err("unready must refuse");
        let text = error.to_string();
        assert!(
            text.contains("HUMAN_HALT") && text.contains(".beads"),
            "{name} refusal must be typed and name the tracker, got: {text}"
        );
        assert!(
            text.contains("br init") || text.contains("readable and writable"),
            "{name} refusal must carry remediation, got: {text}"
        );
        assert!(
            !output.exists(),
            "a refused entry must write nothing, found {}",
            output.display()
        );
    }
}
