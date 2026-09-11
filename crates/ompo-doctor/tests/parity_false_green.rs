//! `omp-orchestrator-ompo-parity-blind-current-72abf`: `ompo parity` must not return the
//! green verdict `CURRENT` from a comparison it cannot anchor.
//!
//! MEASURED 2026-09-10 at HEAD=08e9bc3, unpiped:
//!
//! ```text
//! ~/.local/bin/ompo parity --installed ~/.local/bin/ompo --json   rc=0
//! status=CURRENT reason_code=OMPO_VERB_PARITY_CURRENT
//! source_revision=unknown build_commit=unknown
//! installed_verbs=20 missing=[] unexpected=[]
//! ```
//!
//! Twenty verbs matched on both sides and the artifact was still one adapter behind source.
//! `ompo health` reading the same binary refused with `PROVENANCE_UNSTAMPED`.
//!
//! These legs run the REAL binary rather than the pure verdict function, because the defect
//! was reachable through the CLI: the operator's evidence was the process exit code and the
//! envelope on stdout, and a unit test over `parity_verdict` alone cannot see either. The
//! unit legs in `src/provenance.rs` pin the verdict table; these pin the wiring.

use ompo_doctor::provenance::{
    PARITY_EXIT_CURRENT, PARITY_EXIT_UNKNOWN, PARITY_REASON_CURRENT, PARITY_REASON_UNSTAMPED_BOTH,
    PARITY_REASON_UNSTAMPED_BUILD_COMMIT, PARITY_REASON_UNSTAMPED_SOURCE_REVISION,
};
use serde_json::Value;
use std::process::{Command, Output};

fn run_parity() -> Output {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
        .args([
            "parity",
            "--installed",
            env!("CARGO_BIN_EXE_ompo"),
            "--json",
        ])
        .output()
        .expect("ompo must launch")
}

/// The envelope goes to stdout only when the verdict is the green one; every other verdict
/// is written to stderr. Reading both is what keeps this test honest about which stream the
/// binary actually chose, instead of asserting against whichever one happens to be non-empty.
///
/// It also prints the raw envelope beside the real process exit code. `parity`'s evidence IS
/// the pair (`$?`, envelope) — the bead was filed off `rc=0` next to `status=CURRENT` — and a
/// test that asserts the pair without ever showing it makes the reader take the assertion's
/// word for the measurement. Visible under `--nocapture`.
fn envelope(output: &Output) -> Value {
    let stdout = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr.clone()).expect("utf8 stderr");
    let (stream, raw) = if stdout.trim().is_empty() {
        ("stderr", stderr.trim())
    } else {
        ("stdout", stdout.trim())
    };
    assert!(!raw.is_empty(), "parity emitted nothing on either stream");
    println!(
        "PARITY_EVIDENCE rc={:?} stream={stream} envelope={raw}",
        output.status.code()
    );
    serde_json::from_str(raw).unwrap_or_else(|error| panic!("envelope must parse: {error}: {raw}"))
}

/// THE INVARIANT, asserted unconditionally: `CURRENT` may only be spoken by a build that can
/// name one revision for itself. This is the leg that is RED on the specimen the bead was
/// filed from, and it is the leg that cannot be satisfied by a conditional.
#[test]
fn current_is_unreachable_without_a_revision_this_build_can_name() {
    let output = run_parity();
    let envelope = envelope(&output);
    let data = &envelope["data"];
    let status = data["status"].as_str().expect("status");
    let source_revision = data["source_revision"].as_str().expect("source_revision");
    let build_commit = data["build_commit"].as_str().expect("build_commit");

    if status == "CURRENT" {
        assert_ne!(
            source_revision, "unknown",
            "CURRENT from a build that cannot name its source revision is a false green"
        );
        assert_ne!(
            build_commit, "unknown",
            "CURRENT from a build that cannot name its build commit is a false green"
        );
        assert_eq!(
            source_revision, build_commit,
            "CURRENT from two different revisions names no single source"
        );
    }

    // The exit code is the axis a shell caller branches on, so it is asserted against the
    // verdict rather than merely alongside it: a code that drifts away from the status is the
    // same false green wearing the other half of the pair.
    let exit_code = u8::try_from(data["exit_code"].as_u64().expect("exit_code")).expect("u8");
    assert_eq!(
        output.status.code(),
        Some(i32::from(exit_code)),
        "the process exit code must be the one the payload declares"
    );
    if status == "CURRENT" {
        assert_eq!(exit_code, PARITY_EXIT_CURRENT);
    } else {
        assert_ne!(
            exit_code, PARITY_EXIT_CURRENT,
            "only CURRENT may exit 0; an UNKNOWN that exits 0 is the same false green"
        );
    }
}

/// The blind case, asserted exactly rather than by implication, including which stream the
/// binary wrote to. A build stamped by a checkout with readable git metadata takes the other
/// branch and is asserted just as exactly, so neither direction can go vacuous.
#[test]
fn blind_build_reports_unknown_and_names_the_missing_stamp() {
    let output = run_parity();
    let envelope = envelope(&output);
    let data = &envelope["data"];
    let status = data["status"].as_str().expect("status");
    let reason_code = data["reason_code"].as_str().expect("reason_code");
    let source_revision = data["source_revision"].as_str().expect("source_revision");
    let build_commit = data["build_commit"].as_str().expect("build_commit");
    let blind = source_revision == "unknown" || build_commit == "unknown";

    // The verb-set observation survives whatever the verdict is. This is the axis parity
    // really measures, and an UNKNOWN verdict must not delete it.
    assert_eq!(
        data["verb_set_status"].as_str(),
        Some("CURRENT"),
        "this binary is its own subject, so its verb sets are identical by construction"
    );
    // What parity is BLIND to, published in-band. The roster axis is measured by nobody here:
    // the specimen ran 87 adapters against a source roster of 88 and parity could not see it.
    let unmeasured: Vec<&str> = data["unmeasured_axes"]
        .as_array()
        .expect("unmeasured_axes")
        .iter()
        .map(|axis| axis.as_str().expect("axis"))
        .collect();
    assert!(
        unmeasured.contains(&"adapter_roster"),
        "parity must declare the roster axis unmeasured rather than let a reader assume it"
    );

    if blind {
        assert_eq!(status, "UNKNOWN", "payload: {data}");
        assert_eq!(
            envelope["status"].as_str(),
            Some("UNKNOWN"),
            "the envelope status must carry the verdict, not a second opinion"
        );
        assert_eq!(
            output.status.code(),
            Some(i32::from(PARITY_EXIT_UNKNOWN)),
            "UNKNOWN is instrument error (3), never success (0) and never upstream (4)"
        );
        let expected = match (build_commit, source_revision) {
            ("unknown", "unknown") => PARITY_REASON_UNSTAMPED_BOTH,
            ("unknown", _) => PARITY_REASON_UNSTAMPED_BUILD_COMMIT,
            _ => PARITY_REASON_UNSTAMPED_SOURCE_REVISION,
        };
        assert_eq!(reason_code, expected);
        let missing: Vec<&str> = data["missing_provenance"]
            .as_array()
            .expect("missing_provenance")
            .iter()
            .map(|field| field.as_str().expect("field"))
            .collect();
        assert!(
            !missing.is_empty(),
            "a blind verdict must name the input it lacked"
        );
        assert!(
            String::from_utf8(output.stdout)
                .expect("utf8")
                .trim()
                .is_empty(),
            "a non-green verdict must not be written to the green channel"
        );
    } else {
        assert_eq!(status, "CURRENT", "payload: {data}");
        assert_eq!(reason_code, PARITY_REASON_CURRENT);
        assert_eq!(output.status.code(), Some(i32::from(PARITY_EXIT_CURRENT)));
        assert_eq!(
            data["missing_provenance"].as_array().map(Vec::len),
            Some(0),
            "a stamped build has no missing provenance to report"
        );
    }
}
