#![forbid(unsafe_code)]
//! K6 invariant suite — `omp-orchestrator-yv5k`.
//!
//! One leg per acceptance clause, plus the known-bad and mutation legs this repo
//! requires of every gate. Nothing here asserts an exit code: every leg reads the
//! emitted MESSAGE or the emitted BYTES, because an exit-code-only leg goes green
//! on any unrelated breakage.

use agent_mail_native::identity::{BindingStatus, PaneIdentity};
use agent_mail_native::journey::{AgentName, ProjectKey, ResumePoint};
use agent_mail_native::packet::{
    assert_field_order, parse_line, sha256_hex, ActorIdentity, Authority, CursorPoint, CursorTouch,
    EffectResult, Outcome, PacketError, PacketJournal, PacketRow, RowSpec, Stage, Timestamp,
    ACTOR_FIELD_ORDER,
    FIELD_ORDER, SCHEMA, SCHEMA_VERSION,
};
use agent_mail_native::DeliveryCursor;
use std::fs;
use std::path::PathBuf;

fn actor() -> ActorIdentity {
    ActorIdentity::new("AmberGate", "%1408", 4, BindingStatus::VerifiedLive)
        .expect("a fully-populated actor is valid")
}

fn resume_point(recipient: &str, cursor: u64) -> ResumePoint {
    ResumePoint::restored(
        ProjectKey::new("/Users/josh/Developer/omp-orchestrator"),
        AgentName::new(recipient),
        DeliveryCursor::new(cursor),
    )
}

/// A spec with every required field populated and both digests well-formed.
fn valid_spec() -> RowSpec {
    RowSpec {
        authority: Authority::Daemon,
        stage: Stage::Dispatched,
        actor: actor(),
        bead: "omp-orchestrator-yv5k".to_owned(),
        attempt: 1,
        cursor: CursorTouch::Untouched,
        input_sha256: sha256_hex(b"input payload"),
        result: EffectResult::completed(sha256_hex(b"output payload"))
            .expect("a well-formed digest is a valid completed effect"),
        ts: Timestamp::from_unix(1_767_331_200),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k6-packet-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

// ── ACCEPTANCE 1: field order is FIXED and asserted ────────────────────────

/// The emitted key order is read back off the LINE with an independent scanner,
/// not trusted from the struct. A test that asserts the order by reading the
/// same declaration the emitter used proves only that the file is internally
/// consistent.
#[test]
fn emitted_field_order_matches_the_declared_contract() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid spec")
        .emit()
        .expect("emit");
    assert_field_order(&emitted.line).expect("emitted order must match FIELD_ORDER");

    let keys = agent_mail_native::packet::top_level_keys(&emitted.line);
    assert_eq!(
        keys,
        FIELD_ORDER.to_vec(),
        "the emitted key sequence IS the byte contract: {}",
        emitted.line
    );
}

/// POSITIVE CONTROL for the scanner. A scanner that returned nested keys, or
/// that returned nothing, would make the leg above vacuously green. The row has
/// a nested `actor` object with four keys of its own; none may appear at top
/// level, and the nested order is itself part of the contract.
#[test]
fn the_key_scanner_ignores_nested_objects_and_still_sees_them_nested() {
    let spec = RowSpec {
        cursor: CursorTouch::Touched {
            before: CursorPoint::from_resume_point(&resume_point("AmberGate", 5140)),
            after: CursorPoint::from_resume_point(&resume_point("AmberGate", 5162)),
        },
        ..valid_spec()
    };
    let emitted = PacketRow::new(spec).expect("valid spec").emit().expect("emit");
    let keys = agent_mail_native::packet::top_level_keys(&emitted.line);

    assert_eq!(keys.len(), 14, "exactly the top-level keys: {keys:?}");
    for nested in ACTOR_FIELD_ORDER {
        assert!(
            !keys.contains(&nested.to_owned()),
            "`{nested}` is an actor field and must not surface at top level: {keys:?}"
        );
        assert!(
            emitted.line.contains(&format!("\"{nested}\"")),
            "`{nested}` must still be present nested: {}",
            emitted.line
        );
    }
    // The nested object's own order, in the emitted bytes.
    let actor_at = emitted.line.find("\"actor\"").expect("actor key present");
    let tail = &emitted.line[actor_at..];
    let mut cursor = 0usize;
    for expected in ACTOR_FIELD_ORDER {
        let needle = format!("\"{expected}\"");
        let at = tail[cursor..]
            .find(&needle)
            .unwrap_or_else(|| panic!("actor field `{expected}` missing or out of order: {tail}"));
        cursor += at + needle.len();
    }
}

/// Optional fields are emitted as explicit `null`, never omitted. Skipping would
/// break the fixed order AND make "absent" indistinguishable from "present and
/// empty" — the same indistinguishability defect as folding a working refusal
/// into a crash's exit code.
#[test]
fn absent_optional_fields_are_explicit_nulls_not_omitted_keys() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid spec")
        .emit()
        .expect("emit");
    for key in ["cursor_before", "cursor_after", "error"] {
        assert!(
            emitted.line.contains(&format!("\"{key}\":null")),
            "`{key}` must be an explicit null: {}",
            emitted.line
        );
    }
    assert_eq!(
        agent_mail_native::packet::top_level_keys(&emitted.line).len(),
        FIELD_ORDER.len(),
        "a row with three absent optionals still carries all 14 keys"
    );
}

// ── ACCEPTANCE 2: the hash covers THE EMITTED BYTES ────────────────────────

/// The round trip the bead names: emit -> hash -> re-read from disk -> re-hash ->
/// equal. This is the only leg that distinguishes a digest of the emitted bytes
/// from a digest of the struct they came from.
#[test]
fn the_digest_survives_a_write_and_reread_round_trip() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid spec")
        .emit()
        .expect("emit");
    let path = scratch("roundtrip").join("packet.jsonl");
    fs::write(&path, format!("{}\n", emitted.line)).expect("write");

    let read_back = fs::read_to_string(&path).expect("read");
    let line = read_back
        .lines()
        .next()
        .expect("one line was written, so one line reads back");
    assert_eq!(line, emitted.line, "the bytes on disk are the bytes emitted");
    assert_eq!(
        sha256_hex(line.as_bytes()),
        emitted.sha256,
        "re-hashing the line read from disk reproduces the emitted digest"
    );
}

/// MUTATION LEG. Change one byte of the emitted line and the digest must move.
/// A digest that does not track its bytes certifies nothing, and this is the leg
/// that proves the hash is attributable to the content rather than to the run.
#[test]
fn a_single_byte_change_moves_the_digest() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid spec")
        .emit()
        .expect("emit");
    let mutated = emitted.line.replacen("\"attempt\":1", "\"attempt\":2", 1);
    assert_ne!(mutated, emitted.line, "the mutation must actually apply");
    assert_ne!(
        sha256_hex(mutated.as_bytes()),
        emitted.sha256,
        "one changed byte must produce a different digest"
    );
    // Restore-equivalence: the original bytes still hash to the original digest.
    assert_eq!(
        sha256_hex(emitted.line.as_bytes()),
        emitted.sha256,
        "the unmutated line is unchanged by the mutation above"
    );
}

/// Two rows differing only in field ORDER would hash differently, which is
/// precisely why the order is fixed. Demonstrated on the bytes rather than
/// argued: the same values reordered are a different artifact.
#[test]
fn reordered_bytes_are_a_different_artifact() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid spec")
        .emit()
        .expect("emit");
    let reordered = format!(
        "{{\"version\":{SCHEMA_VERSION},\"schema\":\"{SCHEMA}\"}}"
    );
    assert_ne!(
        sha256_hex(reordered.as_bytes()),
        emitted.sha256,
        "order is part of the byte contract"
    );
    assert!(
        assert_field_order(&reordered).is_err(),
        "a reordered row must be refused by the order gate"
    );
}

// ── ACCEPTANCE 3: a cursor is meaningless without its recipient ────────────

/// A bare integer must be unrepresentable as a resume point. There is no
/// `CursorPoint` constructor taking a `u64`; the only path is from a
/// `ResumePoint`, which already carries the project and recipient. This leg
/// proves the pairing reaches the BYTES.
#[test]
fn a_cursor_on_the_wire_always_carries_its_recipient_and_project() {
    let spec = RowSpec {
        stage: Stage::Observed,
        cursor: CursorTouch::Touched {
            before: CursorPoint::from_resume_point(&resume_point("GreenFrog", 2108)),
            after: CursorPoint::from_resume_point(&resume_point("GreenFrog", 5161)),
        },
        ..valid_spec()
    };
    let emitted = PacketRow::new(spec).expect("valid spec").emit().expect("emit");
    let parsed = parse_line(&emitted.line).expect("parse");

    let before = parsed.cursor_before.expect("touched rows carry both ends");
    let after = parsed.cursor_after.expect("touched rows carry both ends");
    assert_eq!(before.recipient, "GreenFrog");
    assert_eq!(after.recipient, "GreenFrog");
    assert_eq!(before.cursor, 2108);
    assert_eq!(after.cursor, 5161);
    assert!(
        before.project.ends_with("omp-orchestrator"),
        "the project is part of the pairing: {}",
        before.project
    );
}

/// KNOWN-BAD: a bare integer must not deserialise into a cursor position. This
/// is the wire-side half of the unrepresentability claim — the type system covers
/// construction, this covers a hand-written row.
#[test]
fn a_bare_integer_cannot_be_read_as_a_cursor_position() {
    let hand_written = serde_json::from_str::<CursorPoint>("5105");
    assert!(
        hand_written.is_err(),
        "5105 was circulated fleet-wide as `the cursor` and was unusable for two \
         of four recipients; a bare integer must not parse as a position"
    );
}

/// `CursorTouch` makes "touched a cursor and recorded only one end"
/// unrepresentable rather than validated: the touched variant carries both.
#[test]
fn an_untouched_cursor_emits_both_ends_as_null_and_a_touched_one_emits_neither_as_null() {
    let untouched = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");
    assert!(untouched.line.contains("\"cursor_before\":null"));
    assert!(untouched.line.contains("\"cursor_after\":null"));

    let touched_spec = RowSpec {
        cursor: CursorTouch::Touched {
            before: CursorPoint::from_resume_point(&resume_point("AmberGate", 1)),
            after: CursorPoint::from_resume_point(&resume_point("AmberGate", 2)),
        },
        ..valid_spec()
    };
    let touched = PacketRow::new(touched_spec)
        .expect("valid")
        .emit()
        .expect("emit");
    assert!(!touched.line.contains("\"cursor_before\":null"));
    assert!(!touched.line.contains("\"cursor_after\":null"));
}

/// KNOWN-BAD for a defect that was IN MY OWN FIRST DRAFT of this module.
///
/// `parse_line` originally read the version with
/// `u32::try_from(version).unwrap_or(u32::MAX)`. A row claiming version
/// 5_000_000_000 would then have been REFUSED while REPORTING 4294967295 — a
/// diagnostic naming a version the row never carried. Found by auditing my own
/// module for lenient defaults after planting one deliberately in a mutation
/// leg, which is the only reason I looked.
///
/// The refusal must now echo the wire value byte-for-byte.
#[test]
fn an_over_wide_version_is_reported_verbatim_not_clamped() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");
    let over_wide = emitted.line.replacen(
        &format!("\"version\":{SCHEMA_VERSION}"),
        "\"version\":5000000000",
        1,
    );
    assert_ne!(over_wide, emitted.line, "the mutation must actually apply");

    match parse_line(&over_wide) {
        Err(PacketError::UnsupportedVersion { found, expected }) => {
            assert_eq!(
                found, 5_000_000_000,
                "the refusal must name the version the row carried, not a clamp"
            );
            assert_eq!(expected, SCHEMA_VERSION);
        }
        other => panic!("an over-wide version must refuse by name, got {other:?}"),
    }
    let refusal = parse_line(&over_wide).unwrap_err().to_string();
    assert!(
        refusal.contains("5000000000"),
        "the MESSAGE a reader greps must carry the raw value: {refusal}"
    );
    assert!(
        !refusal.contains("4294967295"),
        "a clamped value must never appear in the refusal: {refusal}"
    );
}

// ── ACCEPTANCE 4: a version bump is DETECTABLE by a reader ─────────────────

#[test]
fn a_reader_refuses_an_unsupported_version_by_name() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");
    let bumped = emitted.line.replacen(
        &format!("\"version\":{SCHEMA_VERSION}"),
        "\"version\":2",
        1,
    );
    assert_ne!(bumped, emitted.line, "the bump must actually apply");

    match parse_line(&bumped) {
        Err(PacketError::UnsupportedVersion { found, expected }) => {
            assert_eq!(found, 2);
            assert_eq!(expected, SCHEMA_VERSION);
        }
        other => panic!("a version bump must be a typed refusal naming both, got {other:?}"),
    }
    // The MESSAGE, not just the variant: a reader greps this.
    let refusal = parse_line(&bumped).unwrap_err().to_string();
    assert!(
        refusal.contains("PACKET_UNSUPPORTED_VERSION") && refusal.contains("version 2"),
        "the refusal must name the condition and the version: {refusal}"
    );
}

#[test]
fn a_reader_refuses_a_foreign_schema_by_name() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");
    let foreign = emitted
        .line
        .replacen(SCHEMA, "someone.elses.packet", 1);
    match parse_line(&foreign) {
        Err(PacketError::UnsupportedSchema { found }) => {
            assert_eq!(found, "someone.elses.packet");
        }
        other => panic!("a foreign schema must be refused by name, got {other:?}"),
    }
}

/// KNOWN-GOOD leg, mandatory: an over-strict reader that refuses everything is
/// indistinguishable from a working one on attack input alone.
#[test]
fn a_current_version_row_parses_cleanly() {
    let emitted = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");
    let parsed = parse_line(&emitted.line).expect("a current row must parse");
    assert_eq!(parsed.schema, SCHEMA);
    assert_eq!(parsed.version, SCHEMA_VERSION);
    assert_eq!(parsed.bead, "omp-orchestrator-yv5k");
    assert_eq!(parsed.actor.pane_id, "%1408");
    assert_eq!(parsed.actor.pane_index, 4);
    assert_eq!(parsed.authority, Authority::Daemon);
    assert_eq!(parsed.outcome, Outcome::Delivered);
    assert_eq!(parsed.ts_unix, 1_767_331_200);
}

// ── ACCEPTANCE 5: a missing required field is a WRITE-time typed failure ───

/// THE CENTRAL LEG. K0's `PaneIdentity` carries `agent_name` and `pane_index` as
/// `Option`, because the resolver may not return them. K6 requires both, so an
/// absent value is refused HERE rather than defaulted. `unwrap_or_default` on a
/// required field is the defect this bead exists to prevent, and a zero pane
/// index or an empty agent name is exactly the unresolvable row it produces.
#[test]
fn an_absent_agent_name_or_pane_index_refuses_at_write_time() {
    let no_name = PaneIdentity {
        pane_id: "%1408".to_owned(),
        binding: BindingStatus::VerifiedLive,
        agent_name: None,
        session: Some("omp-orchestrator".to_owned()),
        pane_index: Some(4),
    };
    match ActorIdentity::from_pane_identity(&no_name) {
        Err(PacketError::MissingField { field }) => assert_eq!(field, "actor.agent_name"),
        other => panic!("an absent agent name must refuse, got {other:?}"),
    }

    let no_index = PaneIdentity {
        pane_id: "%1408".to_owned(),
        binding: BindingStatus::VerifiedLive,
        agent_name: Some(AgentName::new("AmberGate")),
        session: Some("omp-orchestrator".to_owned()),
        pane_index: None,
    };
    match ActorIdentity::from_pane_identity(&no_index) {
        Err(PacketError::MissingField { field }) => assert_eq!(field, "actor.pane_index"),
        other => panic!("an absent pane index must refuse, got {other:?}"),
    }

    // KNOWN-GOOD: a fully-populated identity converts.
    let complete = PaneIdentity {
        pane_id: "%1408".to_owned(),
        binding: BindingStatus::VerifiedLive,
        agent_name: Some(AgentName::new("AmberGate")),
        session: Some("omp-orchestrator".to_owned()),
        pane_index: Some(4),
    };
    let built = ActorIdentity::from_pane_identity(&complete).expect("complete identity converts");
    assert_eq!(built.pane_id, "%1408");
    assert_eq!(built.pane_index, 4);
    assert_eq!(built.agent_name, "AmberGate");
    assert_eq!(built.pane_binding, BindingStatus::VerifiedLive);
}

#[test]
fn a_blank_required_field_refuses_by_name() {
    let blank_bead = RowSpec {
        bead: "   ".to_owned(),
        ..valid_spec()
    };
    match PacketRow::new(blank_bead) {
        Err(PacketError::EmptyField { field }) => assert_eq!(field, "bead"),
        other => panic!("a blank bead must refuse, got {other:?}"),
    }

    match ActorIdentity::new("", "%1408", 4, BindingStatus::VerifiedLive) {
        Err(PacketError::EmptyField { field }) => assert_eq!(field, "actor.agent_name"),
        other => panic!("a blank agent name must refuse, got {other:?}"),
    }
    match ActorIdentity::new("AmberGate", "", 4, BindingStatus::VerifiedLive) {
        Err(PacketError::EmptyField { field }) => assert_eq!(field, "actor.pane_id"),
        other => panic!("a blank pane id must refuse, got {other:?}"),
    }
}

#[test]
fn a_malformed_digest_refuses_with_the_observed_length() {
    let truncated = RowSpec {
        input_sha256: "abc123".to_owned(),
        ..valid_spec()
    };
    match PacketRow::new(truncated) {
        Err(PacketError::MalformedSha256 {
            field,
            observed_len,
        }) => {
            assert_eq!(field, "input_sha256");
            assert_eq!(observed_len, 6, "the length tells truncation from garbage");
        }
        other => panic!("a truncated digest must refuse, got {other:?}"),
    }

    // Uppercase hex is refused too: two spellings of one digest would make an
    // equality comparison a false mismatch.
    assert!(
        matches!(
            EffectResult::completed(sha256_hex(b"x").to_uppercase()),
            Err(PacketError::MalformedSha256 { .. })
        ),
        "uppercase hex must be refused so one digest has one spelling"
    );
}

#[test]
fn a_zeroth_attempt_refuses_because_it_did_not_happen() {
    let zeroth = RowSpec {
        attempt: 0,
        ..valid_spec()
    };
    let refusal = PacketRow::new(zeroth).unwrap_err();
    assert_eq!(refusal, PacketError::ZeroAttempt);
    assert!(
        refusal.to_string().contains("1-based"),
        "the message must say why: {refusal}"
    );
}

// ── ANTI-VACUITY ───────────────────────────────────────────────────────────

#[test]
fn an_empty_journal_is_an_error_not_a_clean_run() {
    let journal = PacketJournal::new();
    let refusal = journal.assert_non_empty().unwrap_err();
    assert_eq!(refusal, PacketError::EmptyJournal);
    assert!(
        refusal.to_string().contains("never a clean run"),
        "the message must refuse the reading, not just fail: {refusal}"
    );

    // KNOWN-GOOD: one appended row clears it.
    let mut journal = PacketJournal::new();
    journal.append(valid_spec()).expect("append");
    journal
        .assert_non_empty()
        .expect("a journal with a row is not vacuous");
}

// ── JSONL INTEGRITY: the defect measured next door, refused here ───────────

/// KNOWN-BAD, and it is a real defect in this workspace rather than a synthetic
/// one. `crates/ack-spine/src/ledger.rs:203-219` hand-rolls JSON with `format!`
/// and escapes only `"`. A value containing a newline emits a literal newline
/// INSIDE a JSON string: invalid JSON, and it breaks the one-object-per-line
/// invariant that makes the format JSONL at all. K6 goes through `serde_json`,
/// so this leg asserts the hostile value survives as ONE line that still parses.
#[test]
fn a_value_containing_newlines_quotes_and_backslashes_stays_one_parseable_line() {
    let hostile = "line one\nline two\ttabbed \"quoted\" back\\slash \u{1b}[31mansi\u{1b}[0m";
    let spec = RowSpec {
        result: EffectResult::denied(Outcome::ToolError, hostile)
            .expect("ToolError is restrictive"),
        ..valid_spec()
    };
    let emitted = PacketRow::new(spec).expect("valid").emit().expect("emit");

    assert_eq!(
        emitted.line.lines().count(),
        1,
        "a hostile value must not split the row across lines: {}",
        emitted.line
    );
    assert!(
        !emitted.line.contains('\n'),
        "the emitted line must contain no raw newline"
    );
    let parsed = parse_line(&emitted.line).expect("the row must still parse");
    assert_eq!(
        parsed.error.as_deref(),
        Some(hostile),
        "the value must round-trip byte-for-byte through escaping"
    );
    assert_field_order(&emitted.line).expect("escaping must not disturb field order");
}

#[test]
fn the_journal_writes_one_row_per_line_with_a_trailing_newline() {
    let mut journal = PacketJournal::new();
    for attempt in 1..=3 {
        journal
            .append(RowSpec {
                attempt,
                ..valid_spec()
            })
            .expect("append");
    }
    let jsonl = journal.to_jsonl();
    assert_eq!(jsonl.lines().count(), 3, "three rows, three lines");
    assert!(
        jsonl.ends_with('\n'),
        "a trailing newline is what stops the next append joining two rows"
    );
    for line in jsonl.lines() {
        parse_line(line).expect("every line is an independently parseable row");
        assert_field_order(line).expect("every line carries the fixed order");
    }

    // The journal digest covers the emitted bytes and survives a disk round trip.
    let path = scratch("journal").join("journal.jsonl");
    fs::write(&path, &jsonl).expect("write");
    let read_back = fs::read_to_string(&path).expect("read");
    assert_eq!(read_back, jsonl, "bytes on disk are bytes emitted");
    assert_eq!(
        sha256_hex(read_back.as_bytes()),
        journal.jsonl_sha256(),
        "the journal digest re-derives from disk"
    );
}

// ── OUTCOME SEMANTICS: a restrictive outcome is never success ──────────────

#[test]
fn restrictive_outcomes_are_named_and_delivered_is_not_a_receipt() {
    for outcome in [
        Outcome::Refused,
        Outcome::TimedOut,
        Outcome::Unreachable,
        Outcome::ToolError,
    ] {
        assert!(
            outcome.is_restrictive(),
            "{} must not be readable as the work having happened",
            outcome.as_str()
        );
    }
    assert!(
        !Outcome::Delivered.is_restrictive(),
        "delivered is not restrictive"
    );
    assert!(
        !Outcome::NothingToDo.is_restrictive(),
        "nothing-to-do is a real observation, not a fault"
    );
    // And it is a DISTINCT token from delivered: folding them would make the one
    // actionable outcome indistinguishable from an empty run.
    assert_ne!(Outcome::NothingToDo.as_str(), Outcome::Delivered.as_str());
}

/// Every wire token is distinct across the three enums. Two states sharing a
/// token is the exit-code overload defect in a different alphabet.
#[test]
fn every_wire_token_is_distinct_within_its_enum() {
    let authorities = [
        Authority::Daemon,
        Authority::Cli,
        Authority::PaneObservation,
        Authority::Tracker,
    ]
    .map(Authority::as_str);
    let stages = [
        Stage::Filed,
        Stage::Claimed,
        Stage::Dispatched,
        Stage::Observed,
        Stage::Verified,
        Stage::Closed,
    ]
    .map(Stage::as_str);
    let outcomes = [
        Outcome::Delivered,
        Outcome::Refused,
        Outcome::TimedOut,
        Outcome::Unreachable,
        Outcome::ToolError,
        Outcome::NothingToDo,
    ]
    .map(Outcome::as_str);

    for set in [authorities.to_vec(), stages.to_vec(), outcomes.to_vec()] {
        let mut sorted = set.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), set.len(), "duplicate wire token in {set:?}");
    }
}

/// The `authority` field must survive to the bytes for every variant: a row that
/// does not say which of the two authorities over one store produced it cannot be
/// reconciled against the other later.
#[test]
fn every_authority_round_trips_through_the_wire() {
    for authority in [
        Authority::Daemon,
        Authority::Cli,
        Authority::PaneObservation,
        Authority::Tracker,
    ] {
        let emitted = PacketRow::new(RowSpec {
            authority,
            ..valid_spec()
        })
        .expect("valid")
        .emit()
        .expect("emit");
        assert!(
            emitted
                .line
                .contains(&format!("\"authority\":\"{}\"", authority.as_str())),
            "{} must appear on the wire: {}",
            authority.as_str(),
            emitted.line
        );
        assert_eq!(
            parse_line(&emitted.line).expect("parse").authority,
            authority
        );
    }
}

// ─────────── ABSENCE AS PROOF (rigor-atlas: admit-then-act-with-absence-as-proof)
//             kinds: audit-kernel, runtime-kernel, sandbox-kernel
//             exemplar: franken_node/.../runtime/effect_receipt.rs:21

/// THE DECISIVE LEG. A denied effect emits `"output_sha256":null`, and the proof
/// that nothing ran is that ABSENCE — not a settable flag beside a digest.
///
/// The first version of this contract had `outcome: Outcome` next to a REQUIRED
/// `output_sha256: String`, so a row could say `refused` while carrying a 64-hex
/// digest of bytes that were never produced. The card's anti-pattern, verbatim:
/// *"a boolean can be set wrongly; a missing field cannot be forged into
/// presence."*
#[test]
fn a_denied_effect_emits_no_output_digest() {
    for outcome in [
        Outcome::Refused,
        Outcome::TimedOut,
        Outcome::Unreachable,
        Outcome::ToolError,
    ] {
        let spec = RowSpec {
            result: EffectResult::denied(outcome, "the fence refused this pane")
                .expect("every restrictive outcome is a valid denial"),
            ..valid_spec()
        };
        let emitted = PacketRow::new(spec).expect("valid").emit().expect("emit");

        assert!(
            emitted.line.contains("\"output_sha256\":null"),
            "{} must emit a null output digest: {}",
            outcome.as_str(),
            emitted.line
        );
        assert!(
            !emitted.line.contains(r#""error":null"#),
            "a denial must carry its reason: {}",
            emitted.line
        );
        // The field order is untouched by the change: still exactly 14 keys.
        assert_field_order(&emitted.line).expect("order holds for a denied row");

        let parsed = parse_line(&emitted.line).expect("a coherent denial parses");
        assert_eq!(
            parsed.output_sha256, None,
            "the absence must survive the round trip, not become an empty string"
        );
        assert_eq!(parsed.outcome, outcome);
    }
}

/// The type has no field to hold a digest on the denied side, so the forgery is
/// unconstructible in this crate. This leg asserts the accessor agrees for every
/// restrictive outcome — the structural claim, checked rather than asserted in
/// prose.
#[test]
fn no_restrictive_outcome_can_carry_an_output_digest() {
    for outcome in [
        Outcome::Refused,
        Outcome::TimedOut,
        Outcome::Unreachable,
        Outcome::ToolError,
    ] {
        let denied = EffectResult::denied(outcome, "reason").expect("restrictive");
        assert_eq!(denied.output_sha256(), None, "{}", outcome.as_str());
        assert_eq!(denied.outcome(), outcome);
        assert!(denied.error().is_some());
    }
    let completed = EffectResult::completed(sha256_hex(b"out")).expect("valid digest");
    assert!(completed.output_sha256().is_some());
    assert_eq!(completed.error(), None, "a completed effect has no error text");
}

/// KNOWN-BAD: a NON-restrictive outcome cannot be paired with a denial.
/// `Delivered` and `NothingToDo` describe effects that ran, so allowing either
/// would reintroduce the incoherence the type removes.
#[test]
fn a_non_restrictive_outcome_cannot_be_denied() {
    for outcome in [Outcome::Delivered, Outcome::NothingToDo] {
        match EffectResult::denied(outcome, "reason") {
            Err(PacketError::NonRestrictiveDenial { outcome: named }) => {
                assert_eq!(named, outcome.as_str());
            }
            other => panic!("{} must not be deniable, got {other:?}", outcome.as_str()),
        }
    }
    let refusal = EffectResult::denied(Outcome::Delivered, "x").unwrap_err().to_string();
    assert!(
        refusal.contains("PACKET_NON_RESTRICTIVE_DENIAL") && refusal.contains("describes an effect that RAN"),
        "the refusal must say why: {refusal}"
    );
}

/// A denial with no reason records nothing.
#[test]
fn a_denial_without_a_reason_is_refused() {
    match EffectResult::denied(Outcome::Refused, "   ") {
        Err(PacketError::EmptyField { field }) => assert_eq!(field, "error"),
        other => panic!("a blank denial reason must refuse, got {other:?}"),
    }
}

/// ABSENCE AS PROOF ENFORCED ON READ, both directions. The emitter cannot build
/// the forgery; a HAND-WRITTEN row can attempt it, and a reader that accepted it
/// would let it in through the back door.
#[test]
fn a_hand_written_row_cannot_forge_either_direction() {
    let completed = PacketRow::new(valid_spec())
        .expect("valid")
        .emit()
        .expect("emit");

    // FORGERY 1: a restrictive outcome carrying an output digest.
    let forged_denial = completed
        .line
        .replacen("\"outcome\":\"delivered\"", "\"outcome\":\"refused\"", 1);
    assert_ne!(forged_denial, completed.line, "the forgery must apply");
    match parse_line(&forged_denial) {
        Err(PacketError::IncoherentResult {
            outcome,
            output_present,
        }) => {
            assert_eq!(outcome, "refused");
            assert!(output_present, "the forgery is a denial WITH a digest");
        }
        other => panic!("a denial carrying a digest must be refused, got {other:?}"),
    }

    // FORGERY 2: the inverse — a completed outcome with the digest stripped.
    let stripped = completed.line.replacen(
        &format!("\"output_sha256\":\"{}\"", sha256_hex(b"output payload")),
        "\"output_sha256\":null",
        1,
    );
    assert_ne!(stripped, completed.line, "the strip must apply");
    match parse_line(&stripped) {
        Err(PacketError::IncoherentResult {
            outcome,
            output_present,
        }) => {
            assert_eq!(outcome, "delivered");
            assert!(!output_present, "the forgery is a completion WITHOUT a digest");
        }
        other => panic!("a completion missing its digest must be refused, got {other:?}"),
    }

    // KNOWN-GOOD: the unforged row still parses. Without this the check is
    // attack-only, and an over-strict gate gets routed around.
    parse_line(&completed.line).expect("the coherent row must still parse");
}
