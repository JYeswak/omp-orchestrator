//! Armed-gate watcher (bead omp-orchestrator-l6hsl).
//!
//! Three gates are DISARMED behind `env::var(...) == Ok("1")`, each carrying
//! a death condition of the form "watch it the day it is armed". Nothing
//! evaluated "is it armed now": the conditions are prose in rows and
//! comments, so the day one is armed, the accepted gaps silently become live
//! unmeasured surfaces. This module is the watcher.
//!
//! CATEGORY RULING (acceptance item 3, verified at source): the
//! dies_when-referent resolver (poumg.3) CANNOT carry this predicate.
//! A referent asks whether a named thing EXISTS, resolved against committed
//! inventory records; this asks whether a RUNTIME CONFIGURATION STATE holds,
//! readable only from the process environment. Extending the resolver would
//! smuggle ambient state into a pure record check -- the impure-into-pure
//! direction this repo forbids. A second checker here is not the registry
//! defect (that was N checkers for ONE class); this is one checker per
//! class across TWO classes with different input authorities.
//!
//! The core is pure over an injected env reader: production passes
//! `std::env::var`, legs pass fakes. Reading `std::env` inside the core
//! would make every leg hostage to the machine it runs on. Findings are
//! OBSERVATIONS, never refusals: refusing on armed would make arming
//! unusable, and arming is a conscious operator act with a separate blast
//! radius. The REDDENING happens in tests, per the acceptance.

/// One env-disarmable gate: the var that arms it and the death condition
/// that becomes void the day it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateArm {
    pub gate: &'static str,
    pub var: &'static str,
    /// Short pointer to the condition, not the condition itself: prose
    /// evaluation is the operator's job, which is what the finding asks for.
    pub death: &'static str,
}

/// Every `== Ok("1")` arm-guard in the tree, measured 2026-09-12 (a
/// completeness sweep for that exact shape; path/config overrides such as
/// OMP_GATE_FIRING_LEDGER and OMP_UDS_TARGET_GATE_REGISTRY are a different
/// class and are NOT watched here).
pub const ARMED_GATES: &[GateArm] = &[
    GateArm {
        gate: "staged-build-gate",
        var: "OMP_STAGED_BUILD_GATE",
        death: "TIER 2 promotion; residual mode-bit strictness open (pre-commit-gate.rs:610-616)",
    },
    GateArm {
        gate: "crate-atom-gate",
        var: "OMP_CRATE_ATOM_GATE",
        death: "ratchet re-expressed per-crate before arming, per 9yf5s answer (c) (pre-commit-gate.rs:630-632)",
    },
    GateArm {
        gate: "r1-breadth-gate",
        var: "OMP_R1_BREADTH_GATE",
        death: "exception row preferred over the switch; attribution-scoped verdict (pre-commit-gate.rs:226-228)",
    },
];

/// Armed findings for the gates whose var reads `"1"`. Only the exact
/// arming value counts: the call sites compare `== Ok("1")`, and a watcher
/// that fires on `"true"` or `"0"` would report arming the gate itself
/// would ignore.
pub fn check_armed(
    read_env: &dyn Fn(&str) -> Option<String>,
    gates: &[GateArm],
) -> Result<Vec<String>, &'static str> {
    if gates.is_empty() {
        return Err("ARMED_WATCHER_EMPTY_GATE_SET — zero registered gates; \
                    a leg that finds nothing and passes is indistinguishable \
                    from one that works");
    }
    let mut findings = Vec::new();
    for gate in gates {
        if read_env(gate.var).as_deref() == Some("1") {
            findings.push(format!(
                "ARMED_GATE gate={} var={} death={} — this var is set: the \
                 accepted gap is live and its death condition is now void; \
                 re-examine before trusting this gate",
                gate.gate, gate.var, gate.death
            ));
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(vars: &[(&str, &str)]) -> HashMap<String, String> {
        vars.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    fn read(map: &HashMap<String, String>) -> impl Fn(&str) -> Option<String> + '_ {
        move |key| map.get(key).cloned()
    }

    /// KNOWN-BAD: arming one gate reddens naming the gate AND its now-void
    /// death condition.
    #[test]
    fn arming_one_gate_names_gate_and_void_condition() {
        let map = env(&[("OMP_R1_BREADTH_GATE", "1")]);
        let findings = check_armed(&read(&map), ARMED_GATES).expect("non-empty set");
        assert_eq!(findings.len(), 1, "exactly the armed gate: {findings:?}");
        assert!(
            findings[0].contains("gate=r1-breadth-gate")
                && findings[0].contains("OMP_R1_BREADTH_GATE")
                && findings[0].contains("exception row preferred"),
            "must name gate, var, and void condition: {findings:?}"
        );
    }

    /// KNOWN-GOOD: all three unset passes silent. An over-strict watcher
    /// that reddens on the disarmed default is useless and gets routed
    /// around -- the disarmed state is the accepted, current state.
    #[test]
    fn all_unset_passes_silent() {
        let map = env(&[]);
        let findings = check_armed(&read(&map), ARMED_GATES).expect("non-empty set");
        assert!(findings.is_empty(), "disarmed default must pass: {findings:?}");
    }

    /// Only the exact arming value counts: the call sites compare
    /// `== Ok("1")`, so `"0"`, `"true"` and `""` must stay silent. A watcher
    /// firing where the gate stays dark is a false alarm that trains
    /// operators to ignore the true one.
    #[test]
    fn only_the_exact_arming_value_counts() {
        for value in ["0", "true", "", "2"] {
            let map = env(&[("OMP_CRATE_ATOM_GATE", value)]);
            let findings = check_armed(&read(&map), ARMED_GATES).expect("non-empty set");
            assert!(
                findings.is_empty(),
                "value {value:?} must not arm: {findings:?}"
            );
        }
        let map = env(&[("OMP_CRATE_ATOM_GATE", "1")]);
        let findings = check_armed(&read(&map), ARMED_GATES).expect("non-empty set");
        assert_eq!(findings.len(), 1);
    }

    /// ANTI-VACUITY: an empty gate set is an ERROR, never a pass.
    #[test]
    fn an_empty_gate_set_is_an_error() {
        let map = env(&[("OMP_CRATE_ATOM_GATE", "1")]);
        assert_eq!(
            check_armed(&read(&map), &[]),
            Err("ARMED_WATCHER_EMPTY_GATE_SET — zero registered gates; \
                 a leg that finds nothing and passes is indistinguishable \
                 from one that works")
        );
    }
}
