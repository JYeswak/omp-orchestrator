//! Integration legs for `ompo messages` — the verb that drives ONE bounded OMP `--mode=rpc`
//! session, issues OMP's native `get_messages`, and reports OMP's own answer.
//!
//! Every leg here is HOST-INDEPENDENT by construction. `ompo messages` reaches a real `omp`
//! binary, so a machine without one is a legitimate UNMEASURED (exit 4) rather than a
//! failure; each assertion below is written against the exit-code DICTIONARY, never against
//! one host's outcome. The dictionary:
//!
//! ```text
//! 0  OMP answered and the messages array is present -- INCLUDING an EMPTY array
//! 1  OMP answered and REFUSED, or answered with no `messages` array at all
//! 2  invocation error   -- an unknown flag
//! 3  instrument error   -- this binary could not build its async runtime
//! 4  UNMEASURED         -- `omp` is absent from PATH, or the session timed out
//! ```
//!
//! Note the first row: an empty message list is exit 0. A verb that spent a non-zero code on
//! "ran fine, no results" would train every caller to ignore its exit code, and a real
//! refusal would then be silent — so the leg below checks that a zero exit carries a real
//! count rather than that a count implies a zero exit.
//!
//! CARGO IS NEVER INVOKED HERE. The binary under test is addressed through
//! `env!("CARGO_BIN_EXE_ompo")`, which is the path cargo already built for this test target;
//! a bare `cargo` in this repo goes through an `rch` wrapper that refuses local fallback, and
//! that refusal is an UNRUN rather than a result.

use std::process::Command;

fn ompo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
}

/// The verb is REACHABLE: it is present in the umbrella's dispatch table.
///
/// A verb absent from the dispatch table does not fail interestingly — it answers
/// `UAD_UNKNOWN_VERB` and exits 2, which is the same shape as a typo. So the property is
/// asserted in both directions: the exit code must come from the verb's OWN dictionary
/// (0, 1 or 4) and NOT be the caller-mistake code, AND the typed unknown-verb reason must be
/// absent from the output. Exit 4 is admissible precisely so this leg stays honest on a host
/// with no `omp` installed; it proves the ARM exists without pretending OMP was measured.
#[test]
fn the_messages_verb_is_reachable_and_never_reports_an_unknown_verb() {
    let out = ompo().arg("messages").output().expect("ompo runs");
    let code = out.status.code();
    assert!(
        matches!(code, Some(0) | Some(1) | Some(4)),
        "`ompo messages` must answer from its own exit dictionary (0, 1 or 4) and never 2, \
         which is the umbrella's unknown-verb code; got {code:?}"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        !combined.contains("UAD_UNKNOWN_VERB"),
        "a dispatched verb must never report itself unknown; got {combined:?}"
    );
}

/// The DECLARED surface and the IMPLEMENTED surface agree on this verb.
///
/// This is the parity half of the leg above. An entry in `capabilities` with no dispatch arm
/// is a lie told to an agent reading the manifest; a dispatch arm with no entry is invisible
/// to that same agent. Neither half detects the other alone, so both are asserted.
#[test]
fn the_messages_verb_appears_in_capabilities_and_the_two_halves_agree() {
    let out = ompo()
        .args(["capabilities", "--json"])
        .output()
        .expect("ompo runs");
    assert!(
        out.status.success(),
        "capabilities must succeed; got {:?}",
        out.status.code()
    );
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("capabilities emits JSON");
    let verbs = value["data"]["verbs"]
        .as_array()
        .unwrap_or_else(|| panic!("capabilities must declare a verbs array; got {value:?}"));
    let declared: Vec<&str> = verbs.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        declared.contains(&"messages"),
        "the dispatched verb `messages` MUST be declared in capabilities; got {declared:?}"
    );
}

/// A caller mistake is exit 2, and it is NOT dressed up as an unreachable OMP.
///
/// The two causes send a reader to opposite remedies: exit 2 means fix the command line,
/// while `OMP_MESSAGES_ABSENT` / `OMP_MESSAGES_TRANSPORT_FAILED` mean go look at the machine.
/// A binary that answers an unknown flag by trying to open a session, failing, and reporting a
/// transport fault has converted the caller's typo into a false infrastructure incident.
#[test]
fn an_unknown_flag_on_messages_is_an_invocation_error_not_a_transport_failure() {
    let out = ompo()
        .args(["messages", "--zzz-not-a-flag"])
        .output()
        .expect("ompo runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown flag is a caller mistake and must exit exactly 2; got {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    for forbidden in ["OMP_MESSAGES_ABSENT", "OMP_MESSAGES_TRANSPORT_FAILED"] {
        assert!(
            !stderr.contains(forbidden),
            "a typo must not be reported as {forbidden}, which points the reader at the host \
             instead of at their own command line; got {stderr:?}"
        );
    }
}

/// Every non-zero exit NAMES its cause with a typed reason code.
///
/// The failure this guards is a SILENT non-zero exit: a status the caller can see but cannot
/// act on. Which specific code appears is host-dependent — `OMP_MESSAGES_ABSENT` on a machine
/// with no `omp`, `OMP_MESSAGES_REFUSED` or `OMP_MESSAGES_NO_PAYLOAD` on one where OMP
/// answered — so only the FAMILY is asserted, which is the strongest host-independent claim
/// available.
#[test]
fn every_non_zero_messages_path_names_a_typed_reason_code() {
    let out = ompo().arg("messages").output().expect("ompo runs");
    if out.status.success() {
        return;
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("OMP_MESSAGES_"),
        "a non-zero `ompo messages` (exit {:?}) must emit a typed OMP_MESSAGES_* reason, never \
         a bare status; got {stderr:?}",
        out.status.code()
    );
}

/// ANTI-VACUITY: the JSON envelope is emitted on success and WITHHELD on failure.
///
/// A refusal that still prints a success-shaped envelope is indistinguishable from a success
/// to any consumer that parses stdout and ignores the exit code — which is what makes a
/// shape-only schema check vacuous. So the leg is two-sided: on exit 0 the envelope must
/// carry its documented keys AND the adopted method must be OMP's native `get_messages` (not
/// a substituted or invented call); on any non-zero exit stdout must NOT parse as an object.
#[test]
fn the_json_shape_is_stable_whenever_the_verb_succeeds() {
    let out = ompo()
        .args(["messages", "--json"])
        .output()
        .expect("ompo runs");
    let stdout = String::from_utf8_lossy(&out.stdout);

    if out.status.code() == Some(0) {
        let value: serde_json::Value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|e| panic!("a successful --json run must emit JSON ({e}); got {stdout:?}"));
        for key in ["schema_version", "command", "status", "data"] {
            assert!(
                !value[key].is_null(),
                "the envelope must carry a top-level `{key}`; got {value:?}"
            );
        }
        assert_eq!(
            value["command"], "messages",
            "the envelope must name the verb that produced it; got {value:?}"
        );
        assert_eq!(
            value["data"]["adopted_method"], "get_messages",
            "the adopted method must be OMP's native `get_messages`, not a substitute; got \
             {value:?}"
        );
        return;
    }

    let parsed: Option<serde_json::Value> = serde_json::from_slice(&out.stdout).ok();
    let looks_like_an_envelope = parsed.as_ref().is_some_and(|v| v.is_object());
    assert!(
        stdout.trim().is_empty() || !looks_like_an_envelope,
        "a failed run (exit {:?}) must NOT print a success-shaped envelope on stdout, or a \
         consumer that parses stdout cannot tell refusal from success; got {stdout:?}",
        out.status.code()
    );
}

/// A ZERO exit carries a REAL count, and never the no-payload reason code.
///
/// This is the integration half of the module's central semantic leg. An empty message list
/// is a success at exit 0 with `count = 0`; a missing `messages` array is exit 1. The failure
/// being prevented is a verb that uses a non-zero exit to mean "ran fine, no results", so the
/// assertion is written the only host-independent way available: when the run DID succeed,
/// the count must be present as a number (a null count would mean the success path printed a
/// refusal's payload) and the reason code must be the OK one rather than `NO_PAYLOAD`.
#[test]
fn a_successful_messages_run_reports_a_count_rather_than_an_empty_success() {
    let out = ompo()
        .args(["messages", "--json"])
        .output()
        .expect("ompo runs");
    if out.status.code() != Some(0) {
        return;
    }
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("a successful --json run must emit JSON");
    assert!(
        value["data"]["count"].is_u64(),
        "a zero exit must carry a real message count -- zero is an answer, not an absence; got \
         {value:?}"
    );
    assert_eq!(
        value["data"]["reason_code"], "OMP_MESSAGES_OK",
        "exit 0 must never carry a refusal's reason code; got {value:?}"
    );
    assert_eq!(
        value["status"], "OK",
        "exit 0 and envelope status must agree; got {value:?}"
    );
}

/// The verdict is DETERMINISTIC across repeated runs on one host.
///
/// What is at stake is that the refusal CLASS is stable — a verb that reports UNMEASURED once
/// and REFUSED the next time on an unchanged host is reporting noise, and no operator can act
/// on a code that moves. Two identical invocations must land in the same bucket of the
/// dictionary.
#[test]
fn two_identical_messages_runs_agree_on_their_exit_code() {
    let first = ompo()
        .args(["messages", "--json"])
        .output()
        .expect("ompo runs");
    let second = ompo()
        .args(["messages", "--json"])
        .output()
        .expect("ompo runs");
    assert_eq!(
        first.status.code(),
        second.status.code(),
        "two identical `ompo messages --json` runs must agree on their exit code; got {:?} then \
         {:?} (stderr {:?} / {:?})",
        first.status.code(),
        second.status.code(),
        String::from_utf8_lossy(&first.stderr),
        String::from_utf8_lossy(&second.stderr)
    );
}
