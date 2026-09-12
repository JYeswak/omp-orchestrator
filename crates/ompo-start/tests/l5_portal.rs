#![forbid(unsafe_code)]

use ompo_start::inception::{read_inception, write_atomic_observed, AtomicWriteEffect};
use ompo_start::liveness::SourceVerdict;
use ompo_start::portal::{
    data_hash_without_self, gates_verdict, observability, parse_gates_aggregate, queue_depth,
    require_envelope, seal, SCHEMA_ID, SCHEMA_VERSION, ENVELOPE_FIELDS,
};
use ompo_start::portal_contract::{
    alerts_are_complete, validate_one_next_action, AlertDefect, NextActionDefect,
};
use serde_json::json;

#[test]
fn portal_hash_excludes_its_own_field() {
    let row = json!({
        "schema_id": SCHEMA_ID,
        "schema_version": "1",
        "sources": {},
        "_alerts": [],
        "one_next_action": {"command": "halt", "reason_code": "HD-0009"},
        "data_hash": "placeholder",
    });
    let sealed = seal(row.clone()).expect("object row");
    let hash = sealed["data_hash"].as_str().expect("hash string");
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, data_hash_without_self(&row));

    let mut changed = sealed.clone();
    changed["data_hash"] = json!("different");
    assert_eq!(hash, data_hash_without_self(&changed));
    changed["one_next_action"]["command"] = json!("ompo start --json");
    assert_ne!(hash, data_hash_without_self(&changed));
}

#[test]
fn portal_seal_refuses_non_object_rows() {
    let error = seal(json!(["not", "a", "row"]))
        .expect_err("portal rows must be objects");
    assert_eq!(error, "L5_PORTAL_ROW_NOT_OBJECT");
}

fn mirror(dir: &std::path::Path, body: &str) {
    let beads = dir.join(".beads");
    std::fs::create_dir_all(&beads).unwrap();
    std::fs::write(beads.join("issues.jsonl"), body).unwrap();
}

/// KNOWN-GOOD: exact depth + distribution over a known mirror. Terminal is
/// closed + tombstone; everything else counts.
#[test]
fn queue_depth_counts_non_terminal() {
    let dir = std::env::temp_dir().join(format!("omp-queue-full-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    mirror(
        &dir,
        "{\"id\":\"a\",\"status\":\"open\"}\n{\"id\":\"b\",\"status\":\"in_progress\"}\n{\"id\":\"c\",\"status\":\"closed\"}\n{\"id\":\"d\",\"status\":\"tombstone\"}\n{\"id\":\"e\",\"status\":\"blocked\"}\n",
    );
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "FULL");
    assert_eq!(queue["depth"], 3);
    assert_eq!(queue["by_status"]["open"], 1);
    assert_eq!(queue["by_status"]["closed"], 1);
    assert_eq!(queue["source"], ".beads/issues.jsonl");
    let _ = std::fs::remove_dir_all(&dir);
}

/// KNOWN-BAD (mutation target): a missing mirror is TYPED UNOBSERVABLE, never
/// an empty depth and never a panic. Break the source -> typed refusal.
#[test]
fn queue_depth_missing_mirror_is_unobservable() {
    let dir = std::env::temp_dir().join(format!("omp-queue-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "UNOBSERVABLE");
    assert!(queue.get("reason").is_some(), "refusal carries why: {queue}");
    assert!(
        queue.get("depth").is_none(),
        "no depth when nothing was read: {queue}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// PARTIAL: corrupt lines bound the result instead of failing it or
/// silently dropping rows. Break some lines -> typed PARTIAL with bounds.
#[test]
fn queue_depth_corrupt_lines_are_partial() {
    let dir = std::env::temp_dir().join(format!("omp-queue-partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    mirror(
        &dir,
        "{\"id\":\"a\",\"status\":\"open\"}\nNOT-JSON\n{\"id\":\"c\",\"status\":\"closed\"}\n",
    );
    let queue = queue_depth(&dir);
    assert_eq!(queue["state"], "PARTIAL");
    assert_eq!(queue["bound_kind"], "unparseable_lines");
    assert_eq!(queue["bound_value"], 2);
    assert_eq!(queue["lines_total"], 3);
    assert_eq!(queue["depth"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// KNOWN-GOOD: a well-formed aggregate line parses to its six keys with the
/// invariant holding. Grammar mirrors ci_citation::parse_aggregate.
#[test]
fn gates_aggregate_parses_valid_line() {
    let values =
        parse_gates_aggregate("GATE_RUNNER crates=88 pass=80 fail=5 unmeasurable=3 short=0 no_tests=0\n")
            .expect("valid line parses");
    assert_eq!(values["crates"], 88);
    assert_eq!(values["pass"], 80);
    assert_eq!(values["fail"], 5);
}

/// KNOWN-BAD (mutation targets): each refusal shape is pinned so the grammar
/// cannot drift -- missing line, missing key, broken invariant, non-numeric.
#[test]
fn gates_aggregate_refusals_are_typed() {
    assert!(parse_gates_aggregate("nothing here\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=2 fail=0 unmeasurable=0 short=0\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=1 fail=0 unmeasurable=0 short=0 no_tests=0\n").is_err());
    assert!(parse_gates_aggregate("GATE_RUNNER crates=2 pass=x fail=0 unmeasurable=0 short=0 no_tests=0\n").is_err());
}

/// KNOWN-BAD end to end: no aggregate file is TYPED UNOBSERVABLE (never a
/// zero, never a panic). Break the source -> typed refusal.
#[test]
fn gates_verdict_missing_file_is_unobservable() {
    let dir = std::env::temp_dir().join(format!("omp-gates-absent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let gates = gates_verdict(&dir);
    assert_eq!(gates["state"], "UNOBSERVABLE");
    assert!(gates.get("reason").is_some(), "refusal carries why: {gates}");
    assert!(
        gates.get("crates").is_none() && gates.get("fail").is_none(),
        "no verdict fields when nothing was read: {gates}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// KNOWN-GOOD end to end: a written aggregate file reads FULL with its keys.
#[test]
fn gates_verdict_present_file_is_full() {
    let dir = std::env::temp_dir().join(format!("omp-gates-present-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let target = dir.join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("gate-runner-aggregate.line"),
        "GATE_RUNNER crates=4 pass=3 fail=1 unmeasurable=0 short=0 no_tests=0\n",
    )
    .unwrap();
    let gates = gates_verdict(&dir);
    assert_eq!(gates["state"], "FULL");
    assert_eq!(gates["crates"], 4);
    assert_eq!(gates["fail"], 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// ciay, THE ROW-LEVEL SCHEMA IDENTITY: both halves of the portal row's schema
/// contract are pinned to the LITERAL tokens the contract names, not to each
/// other. Asserting `SCHEMA_ID == SCHEMA_ID` would be a tautology that survives
/// any rename; these compare against the spelled-out strings, so changing
/// either constant reddens this leg and nothing else.
///
/// `schema_version` is covered here because until now it was an inline literal
/// in `ompo-doctor`'s `run_portal` -- the row emitted a version that no test in
/// `ompo-start` could see, so deleting or changing it was undetectable.
#[test]
fn portal_row_schema_identity_is_contract() {
    assert_eq!(SCHEMA_ID, "ompo:portal:v1");
    assert_eq!(SCHEMA_VERSION, "1");
}

// ---------------------------------------------------------------------------
// L5-OBS: the `observability` block. Four acceptances read four adjacent keys
// of ONE object, so the legs below are grouped rather than scattered.
// ---------------------------------------------------------------------------

/// A source verdict with an explicit age. Built here rather than probed so the
/// legs below are deterministic -- a real probe's age changes every run, which
/// would make the data_hash legs untestable.
fn source(name: &str, age_ms: Option<u64>, available: bool, fresh: bool) -> SourceVerdict {
    SourceVerdict {
        name: name.to_owned(),
        available,
        fresh,
        reason_code: if fresh { "FRESH" } else { "STALE" }.to_owned(),
        age_ms,
        panes: Vec::new(),
    }
}

/// KNOWN-GOOD (bbc8): the observability block carries the portal schema id, so
/// `.observability.schema_id` is readable without consulting the outer row.
#[test]
fn obs_schema_id_is_portal_v1() {
    let sources = [source("ntm", Some(12), true, true)];
    let obs = observability(&sources, true, false).expect("non-empty sources are admissible");
    assert_eq!(obs["schema_id"], "ompo:portal:v1");
    assert_eq!(obs["schema_id"], SCHEMA_ID);
}

/// KNOWN-GOOD (1o28): per-source `age_ms` is emitted verbatim per source, and
/// `min_age_ms` is the FRESHEST observed age -- the "min age" half of the
/// contract.
#[test]
fn obs_sources_carry_per_source_age_ms() {
    let sources = [
        source("ntm", Some(900), true, true),
        source("tick-monitor", Some(120), true, true),
        source("agent-mail", Some(4_000), true, false),
    ];
    let obs = observability(&sources, true, false).unwrap();
    assert_eq!(obs["sources"]["ntm"]["age_ms"], 900);
    assert_eq!(obs["sources"]["tick-monitor"]["age_ms"], 120);
    assert_eq!(obs["sources"]["agent-mail"]["age_ms"], 4_000);
    assert_eq!(obs["sources"]["agent-mail"]["fresh"], false);
    assert_eq!(obs["source_count"], 3);
    assert_eq!(obs["min_age_ms"], 120, "min age is the freshest observed age");
    assert!(
        obs["sources"].as_object().is_some_and(|rows| !rows.is_empty()),
        "non-empty sources is the readable half of the acceptance"
    );
}

/// KNOWN-BAD (1o28, the named known-bad): EMPTY sources while claiming live is
/// an ERROR, never an empty map reading as fine. This is the leg that makes the
/// acceptance's "non-empty OR live=false" a real disjunction instead of a
/// sentence -- the third combination is refused outright.
#[test]
fn obs_empty_sources_while_live_is_error() {
    let error = observability(&[], true, false)
        .expect_err("zero sources while live must not produce a row");
    assert!(
        error.starts_with("L5_OBS_EMPTY_SOURCES_WHILE_LIVE"),
        "typed refusal, got {error:?}"
    );
}

/// KNOWN-GOOD (1o28, the other side of the disjunction): empty sources are
/// admissible precisely when the verdict does NOT claim live, and then every
/// age is silent rather than a synthesised zero.
#[test]
fn obs_empty_sources_not_live_is_silent_not_zero() {
    let obs = observability(&[], false, false).expect("empty + not live is admissible");
    assert_eq!(obs["live"], false);
    assert_eq!(obs["source_count"], 0);
    assert!(
        obs["sources"].as_object().is_some_and(|rows| rows.is_empty()),
        "sources is an empty object, not absent"
    );
    assert!(
        obs["min_age_ms"].is_null(),
        "silent is null, NEVER 0 -- a 0 would read as perfectly fresh"
    );
}

/// KNOWN-GOOD (1o28): a present-but-unread source contributes no age, so one
/// silent source cannot drag `min_age_ms` to zero.
#[test]
fn obs_min_age_ignores_silent_sources() {
    let sources = [
        source("ntm", None, true, false),
        source("tick-monitor", Some(77), true, true),
    ];
    let obs = observability(&sources, true, false).unwrap();
    assert!(obs["sources"]["ntm"]["age_ms"].is_null());
    assert_eq!(obs["min_age_ms"], 77);
}

/// KNOWN-BAD (zi3x): `readback_ok` is 0 while the inception readback fails --
/// today's production state -- and it is the NUMBER 0, not `false` and not the
/// string "false". The acceptance reads it numerically.
#[test]
fn obs_readback_ok_is_zero_when_readback_fails() {
    let sources = [source("ntm", Some(5), true, true)];
    let obs = observability(&sources, true, false).unwrap();
    assert_eq!(obs["readback_ok"], 0);
    assert!(
        obs["readback_ok"].is_number(),
        "0/1 number, not a bool: {:?}",
        obs["readback_ok"]
    );
}

/// KNOWN-GOOD (zi3x): and 1 once the readback passes, so the field tracks the
/// verdict instead of being pinned to today's answer.
#[test]
fn obs_readback_ok_is_one_when_readback_passes() {
    let sources = [source("ntm", Some(5), true, true)];
    let obs = observability(&sources, true, true).unwrap();
    assert_eq!(obs["readback_ok"], 1);
}

/// ub2l, THE NAMED ACCEPTANCE: the observability block's `data_hash` is taken
/// over the block EXCLUDING itself, and it is a real content hash -- it MOVES
/// when any covered field moves. A constant hash, or a hash of the object
/// including its own hash field, passes a naive presence test; neither passes
/// this one.
#[test]
fn hash_excludes_self_obs() {
    let sources = [source("ntm", Some(900), true, true)];
    let obs = observability(&sources, true, false).unwrap();

    // Present, and a hex sha256.
    let hash = obs["data_hash"].as_str().expect("data_hash is a string");
    assert_eq!(hash.len(), 64, "sha256 hex, got {hash:?}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

    // EXCLUDES ITSELF: recomputing over the block with the field removed
    // reproduces it exactly. A hash that covered itself could not.
    assert_eq!(
        data_hash_without_self(&obs),
        hash,
        "data_hash must be the hash of the block WITHOUT data_hash"
    );

    // And self-exclusion is not vacuous: overwriting the hash field alone does
    // not change the recomputed value, which is only true if it is excluded.
    let mut tampered = obs.clone();
    tampered["data_hash"] = json!("0".repeat(64));
    assert_eq!(
        data_hash_without_self(&tampered),
        hash,
        "tampering with data_hash alone cannot change the covered payload"
    );

    // KNOWN-BAD, the leg a constant hash fails: the hash MOVES when covered
    // data moves. One changed age, one changed readback verdict, one changed
    // source set -- each yields a different hash.
    let age_changed = observability(&[source("ntm", Some(901), true, true)], true, false).unwrap();
    assert_ne!(
        age_changed["data_hash"], obs["data_hash"],
        "a changed age_ms must change the hash"
    );
    let readback_changed = observability(&sources, true, true).unwrap();
    assert_ne!(
        readback_changed["data_hash"], obs["data_hash"],
        "a changed readback_ok must change the hash"
    );
    let source_added = observability(
        &[
            source("ntm", Some(900), true, true),
            source("tick-monitor", Some(4), true, true),
        ],
        true,
        false,
    )
    .unwrap();
    assert_ne!(
        source_added["data_hash"], obs["data_hash"],
        "an added source must change the hash"
    );

    // Deterministic: identical input, identical hash. Otherwise the two legs
    // above would pass on noise.
    let repeated = observability(&sources, true, false).unwrap();
    assert_eq!(repeated["data_hash"], obs["data_hash"]);
}

// ---------------------------------------------------------------------------
// The two acceptance-named legs, cqwo and qcev. They live HERE, in `l5_portal`,
// because that is the test target their acceptances name:
//   cargo test -p ompo-start --test l5_portal alert_requires_action
//   cargo test -p ompo-start --test l5_portal one_next_action_is_object
// A leg in a target the acceptance does not name reports `0 passed; N filtered
// out` at exit 0, which is silent green. The exhaustive defect tables for both
// laws are in `tests/l5_portal_contract.rs`; these two are the named entry
// points, and each still carries its own known-bad so neither is a redirect.
// ---------------------------------------------------------------------------

/// cqwo, THE NAMED ACCEPTANCE: every alert carries severity, summary AND a
/// non-empty action -- refusals as `am robot` triples, because an empty action
/// on an error is incomplete.
#[test]
fn alert_requires_action() {
    let sources = json!({
        "agent-mail": {"age_ms": 12, "available": true, "fresh": true, "panes": [],
                       "reason_code": "L4_MAIL_SOURCE"},
        "ntm": {"age_ms": null, "available": false, "fresh": false, "panes": [],
                "reason_code": "L4_NTM_UNAVAILABLE detail=ntm timed out after 10s"},
        "tick-monitor": {"age_ms": 5_035_000u64, "available": true, "fresh": false,
                         "panes": ["%19"], "reason_code": "L4_TICK_SOURCE"},
    });
    let complete = json!([
        {"severity": "error", "summary": "ntm source is not fresh and available",
         "action": "ompo portal --json --session omp-orchestrator"},
        {"severity": "warn", "summary": "tick-monitor source is not fresh and available",
         "action": "ompo portal --json --session omp-orchestrator"},
    ]);

    // KNOWN-GOOD: the complete triples the shipped emitter produces.
    assert_eq!(
        alerts_are_complete(&complete, &sources),
        Ok(()),
        "complete triples must pass or the law refuses its own emitter"
    );

    // KNOWN-BAD: blank the action on the FIRST alert and nothing else.
    let mut actionless = complete.clone();
    actionless[0]["action"] = json!("");
    assert_eq!(
        alerts_are_complete(&actionless, &sources),
        Err(AlertDefect::Empty {
            index: 0,
            field: "action"
        }),
        "an alert with no action must refuse"
    );

    // KNOWN-BAD, the one an entry-only check cannot see: NO alerts at all
    // while two sources are degraded.
    assert_eq!(
        alerts_are_complete(&json!([]), &sources),
        Err(AlertDefect::SilentDegradation {
            source: "ntm".to_owned()
        }),
        "an emitter hard-wired to [] must not pass a degraded population"
    );
}

/// qcev, THE NAMED ACCEPTANCE: `one_next_action` is an object, never an array.
/// EXACTLY ONE -- zero refuses, two refuses, one passes -- and the singleton
/// array refuses too, because a list is a protocol bug regardless of length.
#[test]
fn one_next_action_is_object() {
    let action = json!({
        "command": "ompo start --repo /repo --session omp-orchestrator --json",
        "reason_code": "L4_SILENT_SOURCE source=ntm"
    });

    // ONE: passes.
    assert_eq!(validate_one_next_action(&action), Ok(()));

    // ZERO: refuses.
    assert_eq!(
        validate_one_next_action(&serde_json::Value::Null),
        Err(NextActionDefect::Absent)
    );

    // TWO: refuses.
    assert_eq!(
        validate_one_next_action(&json!([action.clone(), action.clone()])),
        Err(NextActionDefect::NotAnObject { len: 2 })
    );

    // ONE, BUT IN A LIST: still refuses. This is the leg a cardinality-only
    // predicate passes, and it is the whole point of the bead's title.
    assert_eq!(
        validate_one_next_action(&json!([action])),
        Err(NextActionDefect::NotAnObject { len: 1 })
    );
}

// ---------------------------------------------------------------------------
// q7jz and u7qq -- the two inception durability ORDER laws.
//
// These legs live in `l5_portal` because that is the target both acceptances
// name. THE INSTRUMENT IS NOT AN INJECTION. Nothing can fail between the write
// and the rename through the public API, and the remote lane runs as ROOT, so
// chmod, read-only files and unwritable directories all silently succeed there
// and every permission-based known-bad is INEXPRESSIBLE. So the order is read
// out of production instead: `write_atomic_observed` is the only write path,
// and each effect it returns is the RETURN VALUE OF THE SYSCALL THAT PERFORMED
// IT. An effect cannot be reported without its syscall having returned Ok, and
// deleting the fsync deletes its record. Real bytes land on a real filesystem
// in every leg below; nothing here is a fake.
// ---------------------------------------------------------------------------

/// The ordered effects of one real atomic replace into a fresh directory, plus
/// the destination path and its parent.
fn observed_replace(payload: &[u8]) -> (tempfile::TempDir, std::path::PathBuf, Vec<AtomicWriteEffect>) {
    let dir = tempfile::tempdir().expect("fixture directory");
    let destination = dir.path().join("inception.json");
    let effects =
        write_atomic_observed(&destination, payload).expect("the atomic replace must succeed");
    (dir, destination, effects)
}

fn index_of(effects: &[AtomicWriteEffect], wanted: fn(&AtomicWriteEffect) -> bool) -> usize {
    effects
        .iter()
        .position(wanted)
        .unwrap_or_else(|| panic!("effect absent from the observed sequence: {effects:?}"))
}

/// q7jz, THE NAMED ACCEPTANCE: the staging file's bytes are fsynced BEFORE the
/// rename publishes them. Renaming an unsynced file is a torn write.
#[test]
fn inception_fsyncs_file() {
    let payload = b"{\"schema_version\":\"inception.v1\"}\n";
    let (_dir, destination, effects) = observed_replace(payload);

    // KNOWN-GOOD, and the proof this is production and not a simulation: the
    // bytes are actually on disk afterwards.
    assert_eq!(
        std::fs::read(&destination).expect("the destination exists"),
        payload,
        "the observed replace must actually publish the bytes"
    );

    let written = index_of(&effects, |effect| {
        matches!(effect, AtomicWriteEffect::BytesWritten(_))
    });
    let fsynced = index_of(&effects, |effect| {
        matches!(effect, AtomicWriteEffect::FileFsynced(_))
    });
    let renamed = index_of(&effects, |effect| {
        matches!(effect, AtomicWriteEffect::Renamed { .. })
    });

    // THE LAW, stated as an order and not as a presence: an fsync that happens
    // AFTER the rename is not this law, and a present-but-late fsync is
    // exactly what a presence-only assertion would accept.
    assert!(
        written < fsynced,
        "the bytes must be written before they are synced: {effects:?}"
    );
    assert!(
        fsynced < renamed,
        "q7jz: fsync must precede the rename, got {effects:?}"
    );

    // And the fsync is on THE STAGING FILE's fd -- the same path the rename
    // publishes FROM -- not on some other file that happens to be syncable.
    let AtomicWriteEffect::FileFsynced(synced_path) = &effects[fsynced] else {
        unreachable!("selected by matches!")
    };
    let AtomicWriteEffect::Renamed { from, to } = &effects[renamed] else {
        unreachable!("selected by matches!")
    };
    assert_eq!(
        synced_path, from,
        "the synced fd must be the staging file the rename publishes"
    );
    assert_eq!(to, &destination);
    assert_eq!(
        effects[written],
        AtomicWriteEffect::BytesWritten(payload.len()),
        "the byte count is the write's own return, not a re-measurement"
    );

    println!("Q7JZ_ORDER {effects:?}");
}

/// u7qq, THE NAMED ACCEPTANCE: the PARENT DIRECTORY is fsynced AFTER the
/// rename. The directory entry is not durable until the parent is synced
/// (beads_rust sync/mod.rs:507-509), so a parent fsync taken BEFORE the rename
/// syncs a directory that does not yet contain the entry.
#[test]
fn inception_fsyncs_parent_dir() {
    #[cfg(not(unix))]
    panic!("u7qq is a unix durability law and this lane is not unix: UNMEASURED, never green");

    let (dir, destination, effects) = observed_replace(b"payload\n");
    assert!(destination.exists());

    let renamed = index_of(&effects, |effect| {
        matches!(effect, AtomicWriteEffect::Renamed { .. })
    });
    let parent = index_of(&effects, |effect| {
        matches!(effect, AtomicWriteEffect::ParentFsynced(_))
    });
    assert!(
        renamed < parent,
        "u7qq: the parent fsync must follow the rename, got {effects:?}"
    );
    assert_eq!(
        parent,
        effects.len() - 1,
        "the parent fsync is the last thing a durable publish does: {effects:?}"
    );

    let AtomicWriteEffect::ParentFsynced(synced) = &effects[parent] else {
        unreachable!("selected by matches!")
    };
    assert_eq!(
        synced.canonicalize().expect("the parent exists"),
        dir.path().canonicalize().expect("the fixture exists"),
        "the synced directory must be the destination's own parent"
    );
    assert_eq!(
        synced,
        &destination
            .parent()
            .expect("a file has a parent")
            .to_path_buf()
    );

    println!("U7QQ_ORDER {effects:?}");
}

/// KNOWN-GOOD over both laws at once, and the over-strictness control: the
/// WHOLE sequence is pinned, so neither leg above can be satisfied by an
/// implementation that also does something extra or something out of order.
/// This is also the leg that fails if a sixth effect is ever introduced
/// without a reader deciding where it belongs.
#[test]
fn inception_durability_sequence_is_exactly_five_ordered_effects() {
    let payload = b"five\n";
    let (dir, destination, effects) = observed_replace(payload);
    let AtomicWriteEffect::Renamed { from, .. } = &effects[3] else {
        panic!("the fourth effect must be the rename: {effects:?}")
    };
    assert_eq!(
        effects,
        vec![
            AtomicWriteEffect::StageCreated(from.clone()),
            AtomicWriteEffect::BytesWritten(payload.len()),
            AtomicWriteEffect::FileFsynced(from.clone()),
            AtomicWriteEffect::Renamed {
                from: from.clone(),
                to: destination.clone(),
            },
            AtomicWriteEffect::ParentFsynced(dir.path().to_path_buf()),
        ],
        "create, write, fsync, rename, fsync-parent -- in that order and nothing else"
    );
    // v809's law, re-read here rather than trusted: staging is in the SAME
    // directory as the destination, so the publish is a same-filesystem rename.
    assert_eq!(from.parent(), destination.parent());
}

/// L5-READBACK (3tek): after a write, required keys must be present on
/// readback. A missing key is REFUSE, never a success token (not S2_OK).
#[test]
fn write_zero_readback_fail_refuses() {
    let dir = tempfile::tempdir().expect("fixture directory");
    let destination = dir.path().join("inception.json");
    let incomplete = br#"{"schema_version":"inception.v1"}"#;
    write_atomic_observed(&destination, incomplete).expect("bytes land");
    let error = read_inception(&destination).expect_err(
        "missing required keys after write must REFUSE, not succeed",
    );
    let detail = error.to_string();
    let detail_upper = detail.to_ascii_uppercase();
    assert!(
        detail_upper.contains("READBACK")
            || detail.contains("project_id")
            || detail.contains("schema_version"),
        "refusal must name the missing key or readback class, got {detail}"
    );
    assert!(
        !detail.contains("S2_OK"),
        "a readback miss must not render as S2_OK: {detail}"
    );
}

/// L5-HASH (jbmy), THE NAMED ACCEPTANCE: alias for `hash_excludes_self_obs`
/// under the acceptance name. A command matching no test exits 0 and proves
/// nothing, so the property ships under both names; the body lives in exactly
/// one place. Same property: the hash is unchanged when only the `data_hash`
/// field changes, and it MOVES when covered data moves.
#[test]
fn hash_stable_under_hash_field() {
    hash_excludes_self_obs();
}

/// L5-ENVELOPE (wryz): a portal JSON missing a required envelope field is
/// NOT a row. The refusal is typed and names the field — never a partial
/// object treated as success, and never a default standing in for absence.
#[test]
fn missing_envelope_field_is_not_a_row() {
    // KNOWN-GOOD control first: a sealed observability block passes — the
    // gate does not refuse everything, so the refusals below are load-bearing.
    let sources = [source("ntm", Some(900), true, true)];
    let block = observability(&sources, true, false).unwrap();
    assert!(
        require_envelope(&block).is_ok(),
        "a sealed block must satisfy its own envelope"
    );

    // TARGET: each required field removed in turn refuses, naming the field.
    for field in ENVELOPE_FIELDS {
        let mut partial = block.clone();
        partial.as_object_mut().expect("block is an object").remove(*field);
        let error = require_envelope(&partial).expect_err(
            "a block missing an envelope field must REFUSE, not succeed",
        );
        println!("L5_ENVELOPE_MISSING field={field} refusal={error}");
        assert!(
            error.contains(*field),
            "the refusal must name the missing field, got {error}"
        );
    }

    // NOT AN OBJECT is not a row either, with its own typed refusal.
    let scalar = json!(42);
    let error = require_envelope(&scalar)
        .expect_err("a non-object portal JSON must REFUSE, not succeed");
    assert!(
        error.contains("L5_ENVELOPE_NOT_OBJECT"),
        "non-object refusal must be typed, got {error}"
    );
}

/// L5-CURSOR (enjg): cursor fields are a number when known, null WITH a
/// reason when not, and never a fabricated 0. `None` must not become `0` on
/// any path — a 0 reads as a real position, not as ignorance.
#[test]
fn cursor_field_number_or_null_with_reason() {
    use ompo_start::portal::cursor_envelope;

    // KNOWN-GOOD control: a known position renders as a number.
    let known = cursor_envelope(Some(42), Some((7, 42)), None).expect("known cursor renders");
    assert_eq!(known["latest_cursor"], json!(42));
    assert_eq!(known["replay_window"], json!([7, 42]));
    println!("L5_CURSOR_KNOWN row={known}");

    // TARGET: unknown renders null WITH the reason — and the null is null,
    // not a fabricated 0.
    let unknown =
        cursor_envelope(None, None, Some("snapshot predates cursor tracking"))
            .expect("null with reason renders");
    assert!(
        unknown["latest_cursor"].is_null(),
        "unknown cursor must be null, got {}",
        unknown["latest_cursor"]
    );
    assert!(
        unknown["replay_window"].is_null(),
        "unknown window must be null, got {}",
        unknown["replay_window"]
    );
    assert_eq!(
        unknown["cursor_reason"],
        json!("snapshot predates cursor tracking")
    );
    assert_ne!(
        unknown["latest_cursor"], json!(0),
        "a null cursor must never render as 0"
    );
    println!("L5_CURSOR_NULL row={unknown}");

    // A null WITHOUT a reason is refused, not rendered: bare nulls explain
    // nothing about why the position is unknown.
    let refused = cursor_envelope(None, Some((0, 9)), None).expect_err(
        "a null cursor field without a reason must REFUSE, not render",
    );
    assert!(
        refused.contains("L5_CURSOR_NULL_WITHOUT_REASON"),
        "refusal must be typed, got {refused}"
    );
}

/// L5-ARTIFACT (vocw): after the write lands, the artifact exists and
/// readback of the required keys succeeds. A zero-byte write and a missing
/// file both FAIL readback — neither is an empty success.
#[test]
fn artifact_readback_required_keys() {
    use ompo_start::inception::read_inception;

    // KNOWN-GOOD control: a complete artifact reads back with its keys.
    let dir = tempfile::tempdir().expect("fixture directory");
    let destination = dir.path().join("inception.json");
    let complete = br#"{
        "schema_version": "inception.v1",
        "project_id": "vocw-fixture",
        "repo_identity": {
            "canonical_path": "/tmp/vocw",
            "git_marker": ".git",
            "source_revision": "vocw0",
            "host_identity": "vocw-host"
        },
        "control_files": {
            "AGENTS.md": true, "CLAUDE.md": true, "README.md": true,
            "Cargo.toml": true, "SCHEMAS.toml": true, "docs/decisions.jsonl": true
        },
        "host_capabilities": {"os": "linux", "arch": "x86_64", "filesystem": "ext4"},
        "required_tools": ["git", "cargo", "br", "bv", "ntm", "am", "jq"],
        "trust_status": {
            "status": "trusted", "reason_code": "vocw",
            "control_files_complete": true
        }
    }"#;
    write_atomic_observed(&destination, complete).expect("bytes land");
    assert!(
        destination.exists(),
        "the artifact must exist after the write"
    );
    let readback = read_inception(&destination).expect("complete artifact reads back");
    assert_eq!(readback.project_id, "vocw-fixture");
    assert!(
        readback.control_files_complete,
        "control_files_complete must be true"
    );
    println!(
        "L5_ARTIFACT_OK project={} complete={}",
        readback.project_id, readback.control_files_complete
    );

    // ZERO-BYTE write FAILS readback: empty bytes are not an empty success.
    let zero_path = dir.path().join("zero.json");
    write_atomic_observed(&zero_path, b"").expect("zero bytes land");
    let error = read_inception(&zero_path).expect_err(
        "a zero-byte artifact must FAIL readback, not succeed",
    );
    println!("L5_ARTIFACT_ZERO refusal={error}");

    // MISSING file FAILS readback: absence is not a row either.
    let missing = dir.path().join("never-written.json");
    assert!(!missing.exists());
    let error = read_inception(&missing).expect_err(
        "a missing artifact must FAIL readback, not succeed",
    );
    let detail = error.to_string();
    println!("L5_ARTIFACT_MISSING refusal={detail}");
    assert!(
        detail.contains("never-written.json"),
        "the refusal must name the missing path, got {detail}"
    );
}

/// L5-DECISIONS (4tq2): `decisions_owed` is always an array of `{id,age_s}`,
/// never omitted. Empty array is the honest zero. An unanswered HD row with
/// a numeric `ts` is owed; an `answers` pointer with a non-empty decision
/// settles it. A fabricated 0-age is refused by using a known `now_s`.
#[test]
fn decisions_owed_never_omitted() {
    use ompo_start::portal::decisions_owed_from_repo;

    let missing = tempfile::tempdir().expect("repo");
    let empty = decisions_owed_from_repo(missing.path(), 1_000);
    assert!(empty.is_array(), "missing ledger must still emit an array");
    assert_eq!(empty, serde_json::json!([]));

    let repo = tempfile::tempdir().expect("repo");
    let docs = repo.path().join("docs");
    std::fs::create_dir_all(&docs).expect("docs");
    std::fs::write(
        docs.join("decisions.jsonl"),
        concat!(
            r#"{"id":"HD-0099","ts":100,"decision":"","question":"owed?"}"#,
            "\n",
            r#"{"id":"HD-0100","ts":50,"decision":"done","question":"settled in-row"}"#,
            "\n",
            r#"{"id":"HD-0101","ts":10,"decision":"","question":"has answer pointer"}"#,
            "\n",
            r#"{"answers":"HD-0101","decision":"parked","ts":20}"#,
            "\n",
        ),
    )
    .expect("ledger");
    let owed = decisions_owed_from_repo(repo.path(), 150);
    assert_eq!(
        owed,
        serde_json::json!([{"id":"HD-0099","age_s":50}]),
        "only the unanswered unpointed row is owed; age is now-ts not 0"
    );
    assert_ne!(owed[0]["age_s"], serde_json::json!(0));

}


/// L5-METRIC-HOME (sheg): the `decisions_owed` age metric carries an
/// UNMEASURABLE delta verdict, never a numeric green without a home. There
/// is no xkr6 expectation row for this metric, so no threshold exists to
/// compute a delta against; a number here would green forever.
#[test]
fn decisions_owed_delta_unmeasurable_without_home() {
    use ompo_start::portal::decisions_owed_delta;

    let verdict = decisions_owed_delta();
    println!("L5_METRIC_HOME verdict={verdict}");
    assert_eq!(
        verdict["metric"], serde_json::json!("decisions_owed_age_s"),
        "the label must name the metric it homes, got {verdict}"
    );
    assert_eq!(verdict["verdict"], serde_json::json!("UNMEASURABLE"));
    assert_eq!(verdict["delta"], serde_json::json!("UNMEASURABLE"));
    assert!(
        verdict["expected"].is_null() && verdict["threshold"].is_null(),
        "no fabricated threshold may stand in for the missing xkr6 row, got {verdict}"
    );
    assert!(
        verdict["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("xkr6")),
        "the label must name the missing home, got {verdict}"
    );
}

/// L4-METRIC-HOME (87iq): the L4 sources agreement metric carries an
/// UNMEASURABLE delta verdict, never a numeric green without a home. There
/// is no xkr6 expectation row for this metric, so no threshold exists to
/// compute a delta against; a number here would green forever. Mirrors the
/// sheg leg for `decisions_owed`, applied to L4 sources instead.
#[test]
fn sources_agreement_delta_unmeasurable_without_home() {
    use ompo_start::portal::sources_agreement_delta;

    let verdict = sources_agreement_delta();
    println!("L4_METRIC_HOME verdict={verdict}");
    assert_eq!(
        verdict["metric"], serde_json::json!("sources_agreement_rate"),
        "the label must name the metric it homes, got {verdict}"
    );
    assert_eq!(verdict["verdict"], serde_json::json!("UNMEASURABLE"));
    assert_eq!(verdict["delta"], serde_json::json!("UNMEASURABLE"));
    assert!(
        verdict["expected"].is_null() && verdict["threshold"].is_null(),
        "no fabricated threshold may stand in for the missing xkr6 row, got {verdict}"
    );
    assert!(
        verdict["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("xkr6")),
        "the label must name the missing home, got {verdict}"
    );
}
