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
