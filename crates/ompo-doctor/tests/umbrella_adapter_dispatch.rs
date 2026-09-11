//! The invariant suite `docs/contracts/umbrella_adapter_dispatch.md` names.
//!
//! Bead `omp-orchestrator-jplf.7.2`. Every test below is one of the contract's named laws and
//! carries that law's id, so a reader can go from a RED test to the clause it violates.
//!
//! CARGO IS INVOKED AS `env!("CARGO")`, NEVER AS `cargo`. A bare `cargo` in this repo goes
//! through an `rch` wrapper that offloads to Linux workers and refuses local fallback; that
//! refusal is an UNRUN, not a result, and it would make this suite's oracle unavailable
//! rather than red. `env!("CARGO")` is the real binary cargo invoked the test with.

use std::collections::BTreeSet;
use std::process::Command;

fn ompo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ompo"))
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Bin-target names straight from cargo — THE ORACLE for the generated roster.
fn cargo_bin_targets() -> BTreeSet<String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
        .current_dir(repo_root())
        .output()
        .expect("cargo metadata must run");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON");
    let packages = value["packages"].as_array().expect("packages array");
    let mut names = BTreeSet::new();
    for package in packages {
        for target in package["targets"].as_array().expect("targets array") {
            let kinds = target["kind"].as_array().expect("kind array");
            if kinds.iter().any(|kind| kind == "bin") {
                names.insert(target["name"].as_str().expect("target name").to_owned());
            }
        }
    }
    assert!(
        !names.is_empty(),
        "the ORACLE returned zero bin targets -- an empty oracle proves nothing and must not \
         be read as agreement"
    );
    names
}

/// `LAW-UAD-ROSTER-DERIVED` — the roster tracks the runner, not a literal.
///
/// This is the test that makes hand-parsing manifests in `build.rs` admissible: if cargo's
/// target-discovery rules and the generator's ever disagree, this goes RED naming the exact
/// difference in both directions.
#[test]
fn roster_tracks_the_runner_not_a_literal() {
    let oracle = cargo_bin_targets();
    let generated: BTreeSet<String> = ompo_doctor::umbrella::adapters()
        .iter()
        .map(|name| (*name).to_owned())
        .collect();

    let missing: Vec<&String> = oracle.difference(&generated).collect();
    let extra: Vec<&String> = generated.difference(&oracle).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "roster drifted from cargo: {} oracle targets, {} generated. \
         MISSING from the roster (unaddressable): {missing:?}. \
         EXTRA in the roster (addresses nothing): {extra:?}",
        oracle.len(),
        generated.len()
    );
}

/// `LAW-UAD-EVERY-TARGET-ADDRESSABLE` — every name the live runner reports is reachable as
/// `ompo help <adapter>` with a usage line.
///
/// Runs against the ORACLE's list, not the roster's, so a roster that quietly shrank cannot
/// make this pass by having fewer things to check.
#[test]
fn every_live_target_has_a_usage_line() {
    let oracle = cargo_bin_targets();
    let mut unreachable = Vec::new();
    for adapter in &oracle {
        let out = ompo().args(["help", adapter]).output().expect("ompo runs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || !stdout.contains(&format!("usage: ompo help {adapter}")) {
            unreachable.push(format!(
                "{adapter} (exit {:?}, stdout {:?})",
                out.status.code(),
                stdout.lines().next().unwrap_or("")
            ));
        }
    }
    assert!(
        unreachable.is_empty(),
        "{} of {} live bin targets are NOT addressable through the umbrella: {unreachable:?}",
        unreachable.len(),
        oracle.len()
    );
}

/// `LAW-UAD-UNKNOWN-IS-TWO` — an unknown adapter exits 2 AND the message names the rejected
/// string. The exit code alone cannot do it: 2 is also the answer to an unknown verb and to a
/// missing flag value, so three distinct caller mistakes share one code.
#[test]
fn unknown_adapter_exits_two_and_names_it() {
    let out = ompo()
        .args(["help", "definitely-not-a-workspace-target"])
        .output()
        .expect("ompo runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown adapter must exit 2; got {:?}",
        out.status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("definitely-not-a-workspace-target"),
        "the refusal MUST name the rejected adapter; got {stderr:?}"
    );
    assert!(
        stderr.contains("UAD_UNKNOWN_ADAPTER"),
        "the refusal must be typed, not prose; got {stderr:?}"
    );

    // And an unknown VERB must be distinguishable from an unknown ADAPTER despite sharing
    // exit 2 -- which is the whole reason the message is the assertion.
    let verb = ompo().arg("frobnicate").output().expect("ompo runs");
    assert_eq!(verb.status.code(), Some(2));
    let verb_err = String::from_utf8_lossy(&verb.stderr);
    assert!(
        verb_err.contains("UAD_UNKNOWN_VERB") && verb_err.contains("frobnicate"),
        "an unknown verb must name itself and be typed differently from an unknown adapter; \
         got {verb_err:?}"
    );
}

/// `LAW-UAD-CAPABILITIES-GOLDEN` — the declared capability set must equal the implemented
/// one. A drifted golden still parses, so a schema check alone passes.
#[test]
fn capabilities_drift_is_red() {
    let out = ompo()
        .args(["capabilities", "--json"])
        .output()
        .expect("ompo runs");
    assert!(out.status.success(), "capabilities must succeed");
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("capabilities emits JSON");

    assert_eq!(value["schema_version"], "omp.umbrella/v1");
    assert_eq!(value["command"], "capabilities");
    assert_eq!(value["status"], "OK");

    let declared: BTreeSet<String> = value["data"]["adapters"]
        .as_array()
        .expect("adapters array")
        .iter()
        .map(|v| v.as_str().expect("adapter name").to_owned())
        .collect();
    let oracle = cargo_bin_targets();
    assert_eq!(
        declared, oracle,
        "the DECLARED adapter list and the live target set must agree; drift here is exactly \
         what a golden artifact is for"
    );
    assert_eq!(
        value["data"]["adapter_count"].as_u64().unwrap() as usize,
        declared.len(),
        "a declared count that disagrees with its own list is a self-inconsistent artifact"
    );

    let probe_ids = value["data"]["probe_ids"]
        .as_array()
        .expect("probe_ids array");
    assert_eq!(
        probe_ids.len(),
        ompo_doctor::PROBES.len(),
        "one declared probe id per implemented probe"
    );
    for id in probe_ids {
        let raw = id.as_str().expect("string id");
        ompo_doctor::umbrella::ProbeId::new(raw)
            .expect("a declared probe id must satisfy the namespace at construction");
    }

    // The non-JSON form must carry the same integers as structured =N values, never prose.
    let plain = ompo().arg("capabilities").output().expect("ompo runs");
    let text = String::from_utf8_lossy(&plain.stdout);
    assert!(
        text.contains(&format!("adapters={}", declared.len())),
        "the plain form must emit a structured =N adapter count; got {text:?}"
    );
}

/// `ompo init` exists as a COMMAND and is idempotent through it.
///
/// THE DEFECT THIS CLOSES: the init mechanism was fully built at
/// `ompo-start/src/inception.rs:556` and reachable by NO operator, which is the single reason
/// `%7` returned CHANGES REQUESTED on both L2 observability beads. The mechanism's idempotence
/// is already proven by `initialize_reprobes_and_second_run_has_zero_artifact_actions`; what
/// is asserted here is the COMMAND SURFACE, and that the receipt survives it.

/// Real git repo with a HEAD commit for `init` fixtures. Identity is
/// fixture-scoped (-c flags), never worker config.
fn git_init_commit(root: &std::path::Path) {
    for args in [
        ["init", "-q"].as_slice(),
        ["add", "-A"].as_slice(),
        ["-c", "user.name=fixture", "-c", "user.email=fixture@local", "commit", "-qm", "fixture"]
            .as_slice(),
    ] {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .expect("git must exist for repo fixtures");
        assert!(status.success(), "git {args:?} failed in fixture");
    }
}
#[test]
fn init_is_a_reachable_verb_and_reports_zero_actions_on_a_second_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path();
    // A real git repo with a HEAD commit: `init` requires source_revision
    // (git rev-parse HEAD must succeed) and the project AGENTS.md stamp.
    // An empty `.git/` dir satisfies neither (ig4fn).
    std::fs::create_dir_all(root.join("docs")).expect("docs dir");
    for name in ["AGENTS.md", "CLAUDE.md", "Cargo.toml", "README.md", "SCHEMAS.toml"] {
        std::fs::write(root.join(name), b"fixture\n").expect("control file");
    }
    std::fs::write(root.join("AGENTS.md"), b"# omp-orchestrator fixture\n")
        .expect("stamped AGENTS.md");
    std::fs::write(root.join("docs/decisions.jsonl"), b"{\"id\":\"fixture\"}\n")
        .expect("decisions");
    git_init_commit(root);
    let artifact = root.join(".omp-orchestrator").join("inception.json");

    let first = ompo()
        .args(["init", "--repo"])
        .arg(root)
        .arg("--output")
        .arg(&artifact)
        .arg("--json")
        .output()
        .expect("ompo runs");
    assert!(
        first.status.success(),
        "first init failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_value: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("init emits JSON");
    assert_eq!(first_value["command"], "init");
    assert_eq!(first_value["schema_version"], "omp.umbrella/v1");
    assert_eq!(
        first_value["data"]["actions"], 1,
        "a first init must WRITE the artifact"
    );
    assert!(artifact.is_file(), "the artifact must exist after init");

    let second = ompo()
        .args(["init", "--repo"])
        .arg(root)
        .arg("--output")
        .arg(&artifact)
        .arg("--json")
        .output()
        .expect("ompo runs");
    assert!(
        second.status.success(),
        "second init failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_value: serde_json::Value =
        serde_json::from_slice(&second.stdout).expect("init emits JSON");
    assert_eq!(
        second_value["data"]["actions"], 0,
        "IDEMPOTENCE: an unchanged input must produce ZERO artifact actions on the second run, \
         and the command surface must not lose that receipt"
    );
}

/// A missing flag value is a caller mistake (2), and an unknown init argument names itself.
#[test]
fn init_refuses_a_missing_flag_value_by_name() {
    let out = ompo().args(["init", "--output"]).output().expect("runs");
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("UAD_MISSING_VALUE") && stderr.contains("--output"),
        "got {stderr:?}"
    );
}

/// SUPERSEDED EXPECTATION, recorded rather than silently re-pointed. This leg used to assert
/// `exit 2` + `UAD_ADAPTER_SCOPED_DOCTOR_UNIMPLEMENTED`, and it was correct while the flag
/// was a placeholder. `omp-orchestrator-jplf.7.2` built the executor, so the OLD assertion now
/// pins behaviour the change deliberately removed — this repo deletes such a test rather than
/// re-pinning it to new text, and the deletion is the visible half of the decision.
///
/// The PROPERTY is preserved: an accepted flag that does nothing is worse than one that
/// refuses, so the axis must produce a TYPED verdict that NAMES the adapter. What changed is
/// only which verdicts are admissible.
#[test]
fn the_adapter_axis_on_doctor_executes_and_never_silently_ignores() {
    let out = ompo()
        .args(["doctor", "--adapter", "tick-monitor"])
        .output()
        .expect("runs");
    let code = out.status.code();
    // 0 live · 1 degraded · 4 unmeasurable. NEVER 2: the name IS in the roster, so an
    // invocation error would mean the roster check rejected a member.
    assert!(
        matches!(code, Some(0) | Some(1) | Some(4)),
        "the axis must EXECUTE a roster member, not refuse it; got {code:?}"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("UAD_ADAPTER_"),
        "a typed reason code is mandatory -- silence is the failure this leg guards; got {combined:?}"
    );
    assert!(
        combined.contains("tick-monitor"),
        "the verdict must name what was asked for; got {combined:?}"
    );
    assert!(
        !combined.contains("UAD_ADAPTER_SCOPED_DOCTOR_UNIMPLEMENTED"),
        "the placeholder must be GONE, not merely unreached; got {combined:?}"
    );
}

/// KNOWN-BAD, and it is the leg the `--adapter` arm was missing entirely: an adapter absent
/// from the roster must be refused BY MESSAGE, because `exit 2` is also this binary's answer
/// to an unknown verb. `help` already satisfied this; `doctor --adapter` did not, and reported
/// an absent name as merely unimplemented.
#[test]
fn doctor_adapter_refuses_a_name_absent_from_the_roster_and_names_it() {
    let out = ompo()
        .args(["doctor", "--adapter", "zzz-not-an-adapter"])
        .output()
        .expect("runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown adapter is an INVOCATION error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("UAD_UNKNOWN_ADAPTER"), "got {stderr:?}");
    assert!(stderr.contains("absent_from_roster"), "got {stderr:?}");
    assert!(stderr.contains("zzz-not-an-adapter"), "got {stderr:?}");
}

/// The positional spelling `docs/plan/07-installability.md:128` prescribes reaches the SAME
/// executor as the flag, and a typo still reads as an unknown argument rather than being
/// swallowed into the adapter axis.
#[test]
fn the_positional_adapter_spelling_reaches_the_same_executor_and_a_typo_does_not() {
    let positional = ompo().args(["doctor", "tick-monitor"]).output().expect("runs");
    let flagged = ompo()
        .args(["doctor", "--adapter", "tick-monitor"])
        .output()
        .expect("runs");
    assert_eq!(
        positional.status.code(),
        flagged.status.code(),
        "two spellings of one axis must not disagree"
    );
    let typo = ompo().args(["doctor", "zzz-not-an-adapter"]).output().expect("runs");
    assert_eq!(typo.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&typo.stderr);
    assert!(
        stderr.contains("unknown argument"),
        "a non-roster token must stay an unknown ARGUMENT; got {stderr:?}"
    );
}

/// The plan spells capabilities as a doctor sub-verb; both spellings must reach ONE
/// implementation. An alias is not a second definition (`fh C47`).
#[test]
fn doctor_capabilities_and_capabilities_are_the_same_answer() {
    let direct = ompo()
        .args(["capabilities", "--json"])
        .output()
        .expect("runs");
    let via_doctor = ompo()
        .args(["doctor", "capabilities", "--json"])
        .output()
        .expect("runs");
    assert!(direct.status.success() && via_doctor.status.success());
    assert_eq!(
        direct.stdout, via_doctor.stdout,
        "two spellings must not produce two answers"
    );
}
