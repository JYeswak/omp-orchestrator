//! Wave OMP-COVERAGE-WIRING: every WIRE output from the authoritative surface
//! map must be visible to the supervisor census, and its census verdict must be
//! either a CITED live trigger or a NAMED finding.
//!
//! NO-CLAIM: this proves trigger registration and supervisor reachability only.
//! It does not prove that any coverage wave's implementation is correct.
//!
//! NO-CLAIM 2, and it is why the leg below pins a SET rather than a blanket: the
//! census reads LIVE surfaces (`git remote`, `crontab -l`, `~/Library/LaunchAgents`)
//! alongside the source tree, so its answer is a property of the HOST as well as the
//! checkout. Measured on both hosts this crate is built on, the unreachable set is the
//! same one name; a host that disagrees takes this leg RED and says so, which is the
//! intended behaviour for a load-dependent oracle.

use omp_orchestrator::{census_gates, GateReachability, COVERAGE_WAVE_OUTPUT_CRATES};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

const SURFACE_MAP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/plan/SURFACE-MAP.jsonl"
));

fn wire_output_crates() -> BTreeSet<String> {
    let mut crates = BTreeSet::new();
    for (line_number, line) in SURFACE_MAP.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap_or_else(|error| {
            panic!(
                "surface map line {} is invalid JSON: {error}",
                line_number + 1
            )
        });
        if row.get("disposition").and_then(Value::as_str) != Some("WIRE") {
            continue;
        }
        if let Some(crate_name) = row.get("maps_to_crate").and_then(Value::as_str) {
            assert!(
                !crate_name.trim().is_empty(),
                "WIRE row has an empty output crate"
            );
            crates.insert(crate_name.to_owned());
        }
    }
    crates
}

#[test]
fn surface_map_has_the_eleven_wave_outputs() {
    let outputs = wire_output_crates();
    assert_eq!(
        outputs.len(),
        11,
        "coverage output scan is stale or vacuous: {outputs:?}"
    );
    let declared = COVERAGE_WAVE_OUTPUT_CRATES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declared,
        outputs.iter().map(String::as_str).collect(),
        "production census registry diverged from the authoritative map"
    );
}

/// EVERY WAVE OUTPUT IS EITHER CITED OR NAMED.
///
/// # Why this leg no longer says "all of them are reachable"
///
/// It used to, and the claim was VACUOUS: `coverage_output_reachability` returned
/// `Reachable` whenever `crates/<name>/Cargo.toml` was a file, so no crate that
/// existed could fail it. `omp-orchestrator-uldvu` repaired that predicate to measure
/// INVOCATION, and the repair produced exactly one honest refusal —
/// `kernel-only-operator-hook`, which has no manifest caller, is absent from the
/// pre-commit hook binary, declares no `[package.metadata.gate]`, appears in no
/// workflow, and whose only crontab row is COMMENTED OUT. The blanket assertion
/// survived the repair unedited and has been asserting the negation of that finding
/// ever since.
///
/// **The finding is real and is NOT being amnestied here.** The tree already carries
/// the ruling — `census_reachability_is_not_existence.rs` asserts this crate must STAY
/// unreachable and calls it *"the census's one genuine BUILT != WIRED finding"*, and a
/// predicate that called it reachable would have widened back to existence. What this
/// leg owes is to state that finding EXACTLY instead of contradicting it.
///
/// # Why a set and not an ignore
///
/// `assert_eq!` on the membership makes every direction of drift red: a second crate
/// falling out fails, and WIRING this one also fails, because the pin then names a
/// crate that is no longer a finding. Neither can happen silently, which is the whole
/// difference between a pinned finding and a suppression.
#[test]
fn every_wave_output_is_either_cited_or_a_named_finding() {
    /// The census's genuine BUILT != WIRED findings, pinned by name.
    ///
    /// Measured 2026-09-11 on both hosts: hook binary byte-scan 0 hits (control:
    /// `no-shell-gate` 7, `crate-atom-gate` 5), `.github/` 0 hits, live crontab 0
    /// uncommented rows (control: `fleet-composite` 1), `~/Library/LaunchAgents` 0.
    const NAMED_FINDINGS: &[&str] = &["kernel-only-operator-hook"];

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let census = census_gates(repo_root);

    let outputs = wire_output_crates();
    // ANTI-VACUITY: an empty scan set is an ERROR, never a pass. `assert_eq!` over two
    // empty sets below would otherwise report identically to a fully-cited fleet.
    assert!(
        !outputs.is_empty(),
        "VACUOUS: the surface map yielded no WIRE outputs, so this leg verified nothing"
    );

    let mut findings = BTreeSet::new();
    let mut cited = 0usize;
    for output_crate in &outputs {
        let row = census
            .rows
            .iter()
            .find(|row| row.gate == *output_crate)
            .unwrap_or_else(|| panic!("coverage output {output_crate} has no census row"));
        match &row.reachability {
            GateReachability::Reachable { trigger } if !trigger.trim().is_empty() => cited += 1,
            GateReachability::Reachable { .. } => panic!(
                "coverage output {output_crate} is Reachable with an EMPTY trigger: a verdict \
                 that cites nothing is existence wearing a citation's clothes"
            ),
            GateReachability::Unreachable { reason } => {
                // A finding must still name what was consulted. An absence asserted in the
                // confident voice is the reason string this census was repaired away from.
                assert!(
                    reason.contains("surfaces probed"),
                    "{output_crate} is a finding whose reason does not name the surfaces it \
                     consulted: {reason}"
                );
                findings.insert(output_crate.as_str());
            }
            // A WIRE output with no trigger OF ANY KIND is the same class of finding as an
            // Unreachable one and must not fall through a wildcard into silence. It carries no
            // reason string, so there is nothing to check for surface-naming.
            GateReachability::NotInstalled => {
                findings.insert(output_crate.as_str());
            }
            // NOT a wiring finding and deliberately not absorbed into one: an unextracted crate
            // cannot be wired, and `repair-gate-trigger` sends the operator down a path that
            // cannot terminate. The surface map calling it a WIRE output is the inconsistency.
            GateReachability::NotExtracted { upstream, loc } => panic!(
                "coverage output {output_crate} is a WIRE row in the surface map but still lives \
                 in {upstream} ({loc} LOC unextracted). Extraction debt is not a trigger defect \
                 and this leg will not report it as one"
            ),
            GateReachability::Unprobed { reason } => panic!(
                "coverage output {output_crate} was never probed, which is neither a citation \
                 nor a finding: {reason}"
            ),
        }
    }

    // ANTI-VACUITY 2: the pin must not be satisfiable by every row refusing.
    assert!(
        cited > 0,
        "VACUOUS: not one wave output carried a live trigger; a census that cites nothing \
         reports identically to one that was never run"
    );
    assert_eq!(
        findings,
        NAMED_FINDINGS.iter().copied().collect::<BTreeSet<_>>(),
        "the BUILT != WIRED findings are a MEMBERSHIP, not a tolerance. A name that appeared \
         is an unwired coverage output nobody triaged; a name that vanished means the crate \
         was wired and this pin is now stale -- update it, do not widen it. cited={cited} of {}",
        outputs.len()
    );
}
#[test]
fn an_unreachable_wave_output_blocks_supervisor_decision() {
    let observation = omp_orchestrator::Observation {
        panes: Vec::new(),
        queue: omp_orchestrator::QueueState {
            ready_count: 0,
            readable: true,
        },
        gate_census: Some(omp_orchestrator::GateCensus {
            rows: vec![
                omp_orchestrator::GateCensusRow {
                    gate: "no-shell-gate".to_owned(),
                    reachability: GateReachability::Reachable {
                        trigger: "test-positive-control".to_owned(),
                    },
                    disposition: omp_orchestrator::CensusDisposition::Blocking,
                },
                omp_orchestrator::GateCensusRow {
                    gate: "finding-dispatch".to_owned(),
                    reachability: GateReachability::NotInstalled,
                    disposition: omp_orchestrator::CensusDisposition::Blocking,
                },
            ],
        }),
    };

    let decision = omp_orchestrator::decide(
        &observation,
        &omp_orchestrator::IdleAuthorization::Unauthorized { why: "test" },
    );
    match decision {
        omp_orchestrator::SupervisorDecision::GateUnwired { unwired } => {
            assert!(unwired.iter().any(|gate| gate == "finding-dispatch"));
        }
        other => panic!("unreachable coverage output bypassed decision gate: {other:?}"),
    }
}
