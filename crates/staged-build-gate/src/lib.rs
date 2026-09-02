#![forbid(unsafe_code)]
//! PRE-COMMIT BUILD GATE - every crate a commit touches must compile.
//!
//! # The defect
//!
//! Measured 2026-09-02 (`6fot`): a commit containing a crate that cannot compile is
//! in `HEAD`.
//!
//! ```text
//! git status --porcelain -- crates/plan-assemble/   -> empty  (landed, not in progress)
//! cargo build -p plan-assemble
//!   error: asupersync entry macros support only `()` or `Result<(), E>` return types
//!     --> crates/plan-assemble/src/main.rs:258:20
//!   error[E0601]: `main` function not found in crate `plan_assemble`
//! ```
//!
//! `installer --install` builds the WHOLE workspace, so one uncompilable crate blocked
//! installing EVERY binary - three fixes were committed-and-inert for want of an
//! install, including `y6v5`, which cannot observe its own acceptance until the
//! supervisor can be installed.
//!
//! **Why nothing caught it.** The pre-commit hook runs six gates over the staged file
//! set - no-shell, path-literal, state-wildcard, orchestration-tick,
//! pre-delete-citation, preregistration. **Every one is a property of the file TEXT.
//! Not one compiles anything.** `cargo test` lives in CI, and CI has failed 67
//! consecutive runs, so its red is indistinguishable from its normal state.
//!
//! # This gate has NO entry macro, deliberately
//!
//! The known-bad is an `#[asupersync::main]` on `async fn main() -> ExitCode`. A gate
//! that exists to catch that class must not be able to die of it, so `main` here is a
//! plain `fn main() -> ExitCode` and the bounded subprocess work goes through
//! `subprocess-contract::bounded_output`, which is synchronous.
//!
//! # The evidence boundary this gate refuses to cross
//!
//! `cargo build -p X` compiles the **worktree**, and the commit carries the **index**.
//! With five writers in one checkout those differ routinely - measured the same day:
//! `HEAD` carried the broken `plan-assemble` while the worktree already held an
//! uncommitted fix, so a worktree build reported CLEAN about a tree that does not
//! compile. A build of the worktree is therefore not evidence about the commit unless
//! the two agree, so divergence is a **typed refusal**, not a warning.
//!
//! Materialising the index instead (`git checkout-index --prefix`) would remove the
//! caveat and is not done here: it rebuilds from a cold target directory on a volume
//! measured at 99% full, and `git worktree` is forbidden repo-wide.

use std::collections::{BTreeMap, BTreeSet};

/// Seconds a single per-crate build may take before the gate refuses.
///
/// Bounded because an unbounded hook is worse than no hook: it stalls every writer in
/// a shared checkout and reads as a hang rather than a refusal.
pub const BUILD_DEADLINE_SECS: u64 = 300;

/// A deadline shorter than a cold dependency build turns the gate into a coin flip.
/// Compile-time, so a filtered test run cannot skip it - the shape `e5b25a2` proved
/// bites with `E0080: evaluation panicked`.
const _: () = assert!(
    BUILD_DEADLINE_SECS >= 60,
    "a build deadline under one minute refuses honest builds and will be routed around"
);

/// What the staged set means for this gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedScope {
    /// No staged path lives under `crates/`. Reachable, and NOT a pass over nothing:
    /// the caller must emit `GATE_NOT_APPLICABLE` so a skipped run is legible.
    NotApplicable,
    /// Staged paths name these crate directories.
    Crates(BTreeSet<String>),
}

/// The verdict for one crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrateVerdict {
    /// `cargo build -p <crate>` exited 0.
    Pass,
    /// It exited non-zero. `first_error` is the first `error` line, so the refusal
    /// names the cause instead of making an operator re-run the build to find it.
    BuildFailed {
        code: Option<i32>,
        first_error: String,
    },
    /// Non-zero exit with **no compiler diagnostic at all**. Measured, and the reason
    /// this variant exists: the `cargo` on `PATH` here is an RCH shim that can exit
    /// **103** with `[RCH] remote required; refusing local fallback (no admissible
    /// workers: hard_preflight=3, ...)` and zero `error` lines. The build never ran.
    ///
    /// Reporting that as `BuildFailed` **slanders the code for an infrastructure
    /// refusal** — the same class as "a timeout is not a verdict", applied to exit
    /// codes. Restrictive either way, so the commit is still refused; but the reason
    /// and the remedy differ, and an operator sent to fix a crate that compiles fine
    /// stops trusting the gate.
    ///
    /// Carries a stderr tail so the next reader sees the actual bytes rather than
    /// re-deriving them.
    BuildInconclusive {
        code: Option<i32>,
        stderr_tail: String,
    },
    /// Staged content and worktree content differ for this crate, so a worktree build
    /// says nothing about what is being committed. **Refuses; never builds anyway.**
    Diverged { paths: Vec<String> },
    /// `crates/<X>/` was staged but no Cargo package resolves there. This is the
    /// mis-parsed-path case, and it must be an ERROR: a path that resolves to no
    /// target silently passes everything it was supposed to check.
    NoTarget,
    /// The deadline elapsed. Restrictive - it carries no verdict about the code,
    /// which is why it is a distinct variant and not folded into `BuildFailed`.
    TimedOut { deadline_secs: u64 },
    /// `cargo` could not be spawned. Distinct from `TimedOut` because the remedy
    /// differs: a missing toolchain, not a slow build.
    Unspawned { detail: String },
}

impl CrateVerdict {
    /// Whether this verdict permits the commit. Only `Pass` does; every unknown is
    /// restrictive, which is the fail-closed rule stated as code rather than prose.
    pub fn admits(&self) -> bool {
        matches!(self, CrateVerdict::Pass)
    }

    pub fn label(&self) -> &'static str {
        match self {
            CrateVerdict::Pass => "PASS",
            CrateVerdict::BuildFailed { .. } => "BUILD_FAILED",
            CrateVerdict::BuildInconclusive { .. } => "BUILD_INCONCLUSIVE",
            CrateVerdict::Diverged { .. } => "STAGED_WORKTREE_DIVERGENCE",
            CrateVerdict::NoTarget => "NO_BUILD_TARGET",
            CrateVerdict::TimedOut { .. } => "BUILD_TIMED_OUT",
            CrateVerdict::Unspawned { .. } => "CARGO_UNSPAWNED",
        }
    }

    /// The action an operator can actually take. A refusal that names no remedy gets
    /// routed around, and this gate sits on the commit path.
    pub fn next_action(&self) -> &'static str {
        match self {
            CrateVerdict::Pass => "none",
            CrateVerdict::BuildFailed { .. } => "fix-the-crate-or-unstage-it",
            // NOT "fix-the-crate": the code was never compiled.
            CrateVerdict::BuildInconclusive { .. } => "fix-the-build-toolchain-then-re-run",
            CrateVerdict::Diverged { .. } => "stage-the-whole-crate-or-commit-only-what-builds",
            CrateVerdict::NoTarget => "check-the-crate-path-parser",
            // The FIRST remedy this shipped with was "raise-the-deadline", and it was
            // wrong: measured, a timeout here meant the gate was queued behind another
            // agent's `cargo` on the shared build lock, not that the build was slow.
            // Raising the deadline would have hidden that for longer. The gate now
            // uses its own target dir, so a timeout that survives THAT is a genuinely
            // slow build - and the remedy names both, in the order to check them.
            CrateVerdict::TimedOut { .. } => "check-target-dir-isolation-then-split-the-commit",
            CrateVerdict::Unspawned { .. } => "install-or-fix-cargo",
        }
    }
}

/// The gate's whole answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// No crate was touched. Legible, and distinct from a pass.
    NotApplicable,
    /// Every touched crate built.
    Pass { crates: Vec<String> },
    /// At least one crate did not admit the commit.
    Refused { rows: BTreeMap<String, CrateVerdict> },
    /// Crates were touched and NONE resolved to a build target. Separated from
    /// `Refused` because it indicts the gate's own parser rather than the commit,
    /// and the remedy is therefore different.
    ZeroTargets { touched: Vec<String> },
}

impl GateVerdict {
    pub fn admits(&self) -> bool {
        matches!(self, GateVerdict::NotApplicable | GateVerdict::Pass { .. })
    }
}

/// Which crate directories a staged path set touches.
///
/// Only the segment immediately under `crates/` counts, and a bare `crates/` entry or
/// a path with nothing after the crate name names no crate.
pub fn touched_crates<S: AsRef<str>>(staged: &[S]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in staged {
        let path = path.as_ref();
        let Some(rest) = path.strip_prefix("crates/") else {
            continue;
        };
        let Some((name, tail)) = rest.split_once('/') else {
            // `crates/foo` with no tail is the directory entry itself, not a file
            // inside a crate. Counting it would invent a crate from a rename.
            continue;
        };
        if name.is_empty() || tail.is_empty() {
            continue;
        }
        out.insert(name.to_owned());
    }
    out
}

/// Classify the staged set before any subprocess runs.
pub fn classify_scope<S: AsRef<str>>(staged: &[S]) -> StagedScope {
    let crates = touched_crates(staged);
    if crates.is_empty() {
        StagedScope::NotApplicable
    } else {
        StagedScope::Crates(crates)
    }
}

/// Fold per-crate verdicts into the gate's answer.
pub fn fold(touched: &BTreeSet<String>, rows: BTreeMap<String, CrateVerdict>) -> GateVerdict {
    if touched.is_empty() {
        return GateVerdict::NotApplicable;
    }
    // ANTI-VACUITY. Crates were touched and not one resolved to a target: the parser
    // is wrong, and reporting PASS here is exactly how a mis-parsed path lets
    // everything through while reading green.
    if !rows.is_empty() && rows.values().all(|v| *v == CrateVerdict::NoTarget) {
        return GateVerdict::ZeroTargets {
            touched: touched.iter().cloned().collect(),
        };
    }
    // An empty row set against a non-empty touched set means nothing was evaluated.
    // That is the same defect one level up and must not read as a pass.
    if rows.is_empty() {
        return GateVerdict::ZeroTargets {
            touched: touched.iter().cloned().collect(),
        };
    }
    if rows.values().all(CrateVerdict::admits) {
        GateVerdict::Pass {
            crates: rows.keys().cloned().collect(),
        }
    } else {
        GateVerdict::Refused {
            rows: rows.into_iter().filter(|(_, v)| !v.admits()).collect(),
        }
    }
}

/// The first `error` line cargo emitted, so a refusal names its cause.
///
/// Prefers a line beginning `error` at column zero: cargo indents the `-->` location
/// lines and the `= note:` lines beneath it, and a scan that took the first line
/// merely CONTAINING "error" would return a filename from the location line.
pub fn first_cargo_error(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim_end)
        .find(|line| line.starts_with("error"))
        .map(|line| line.trim().to_owned())
}

/// Render one line per refused crate. Asserted on in tests per gate rule 7: the
/// emitted TEXT is the interface, because it is all an operator sees at commit time.
pub fn render_refusal(rows: &BTreeMap<String, CrateVerdict>) -> String {
    let mut lines = Vec::new();
    for (name, verdict) in rows {
        let detail = match verdict {
            CrateVerdict::BuildFailed { code, first_error } => {
                let code = code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
                format!("exit={code} first_error=\"{first_error}\"")
            }
            CrateVerdict::Diverged { paths } => format!("diverged_paths={}", paths.join(",")),
            CrateVerdict::TimedOut { deadline_secs } => format!("deadline_secs={deadline_secs}"),
            CrateVerdict::Unspawned { detail } => format!("detail=\"{detail}\""),
            CrateVerdict::BuildInconclusive { code, stderr_tail } => {
                let code = code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
                format!("exit={code} stderr_tail=\"{stderr_tail}\"")
            }
            CrateVerdict::NoTarget | CrateVerdict::Pass => String::new(),
        };
        lines.push(format!(
            "STAGED_BUILD_GATE_REFUSED crate={name} reason={} next_action={} {detail}",
            verdict.label(),
            verdict.next_action()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(pairs: &[(&str, CrateVerdict)]) -> BTreeMap<String, CrateVerdict> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect()
    }

    #[test]
    fn only_paths_inside_a_crate_name_a_crate() {
        let staged = [
            "crates/plan-assemble/src/main.rs",
            "crates/plan-assemble/Cargo.toml",
            "crates/no-shell-gate/tests/numbers.rs",
            "docs/plan/00-brief.md",
            "AGENTS.md",
            // Neither of these names a crate: one is the directory entry, the other
            // has no file under the crate name.
            "crates/orphan",
            "crates/",
        ];
        let touched = touched_crates(&staged);
        assert_eq!(
            touched.iter().cloned().collect::<Vec<_>>(),
            vec!["no-shell-gate".to_owned(), "plan-assemble".to_owned()]
        );
    }

    /// The two empty cases are DIFFERENT facts and must not collapse.
    #[test]
    fn a_commit_touching_no_crate_is_not_applicable_not_a_pass() {
        let scope = classify_scope(&["docs/plan/00-brief.md", "AGENTS.md"]);
        assert_eq!(scope, StagedScope::NotApplicable);
        assert_eq!(fold(&BTreeSet::new(), BTreeMap::new()), GateVerdict::NotApplicable);
    }

    /// ANTI-VACUITY, the case the bead names: crates touched, zero targets resolved.
    /// A mis-parsed path must indict the parser, never pass the commit.
    #[test]
    fn crates_touched_but_no_targets_is_an_error_and_names_the_parser() {
        let touched: BTreeSet<String> = ["ghost".to_owned()].into_iter().collect();
        let verdict = fold(&touched, rows(&[("ghost", CrateVerdict::NoTarget)]));
        assert_eq!(
            verdict,
            GateVerdict::ZeroTargets {
                touched: vec!["ghost".to_owned()]
            }
        );
        assert!(!verdict.admits());
        // And an empty row set against touched crates is the same defect one level up.
        let nothing_evaluated = fold(&touched, BTreeMap::new());
        assert!(!nothing_evaluated.admits());
        assert!(matches!(nothing_evaluated, GateVerdict::ZeroTargets { .. }));
    }

    /// KNOWN-GOOD, mandatory. An attack-only suite ships an over-strict gate, and an
    /// over-strict gate on the commit path is routed around within a day.
    #[test]
    fn a_commit_whose_crates_all_build_is_admitted() {
        let touched: BTreeSet<String> = ["a".to_owned(), "b".to_owned()].into_iter().collect();
        let verdict = fold(
            &touched,
            rows(&[("a", CrateVerdict::Pass), ("b", CrateVerdict::Pass)]),
        );
        assert!(verdict.admits());
        assert_eq!(
            verdict,
            GateVerdict::Pass {
                crates: vec!["a".to_owned(), "b".to_owned()]
            }
        );
    }

    /// FIRES-ON-KNOWN-BAD: the exact specimen in the tree at `e463bbb`.
    #[test]
    fn the_real_entry_macro_known_bad_is_refused_and_named() {
        let stderr = "\
   Compiling plan-assemble v0.1.0
error: asupersync entry macros support only `()` or `Result<(), E>` return types
  --> crates/plan-assemble/src/main.rs:258:20
error[E0601]: `main` function not found in crate `plan_assemble`
  --> crates/plan-assemble/src/main.rs:750:2
";
        let first = first_cargo_error(stderr).expect("cargo error must be extracted");
        assert_eq!(
            first,
            "error: asupersync entry macros support only `()` or `Result<(), E>` return types"
        );
        let touched: BTreeSet<String> = ["plan-assemble".to_owned()].into_iter().collect();
        let verdict = fold(
            &touched,
            rows(&[(
                "plan-assemble",
                CrateVerdict::BuildFailed {
                    code: Some(101),
                    first_error: first.clone(),
                },
            )]),
        );
        assert!(!verdict.admits());
        let GateVerdict::Refused { rows: refused } = &verdict else {
            panic!("must refuse, got {verdict:?}");
        };
        // Gate rule 7: assert on the emitted TEXT.
        let text = render_refusal(refused);
        assert!(text.contains("crate=plan-assemble"), "{text}");
        assert!(text.contains("reason=BUILD_FAILED"), "{text}");
        assert!(text.contains("entry macros support only"), "{text}");
        assert!(text.contains("next_action=fix-the-crate-or-unstage-it"), "{text}");
    }

    /// The downstream `E0601` must not be reported as the cause. The entry macro
    /// rejects the return type, bails, and leaves the crate with no `main` at all -
    /// so the second error points at the last line of the file and reads as unrelated.
    #[test]
    fn the_first_error_is_the_cause_not_the_downstream_artifact() {
        let reordered = "\
error[E0601]: `main` function not found in crate `plan_assemble`
error: asupersync entry macros support only `()` or `Result<(), E>` return types
";
        assert!(first_cargo_error(reordered).unwrap().starts_with("error[E0601]"));
        // And a location line must never be mistaken for the error: it is indented,
        // and a `contains("error")` scan would have returned the filename.
        let only_location = "  --> crates/plan-assemble/src/main.rs:258:20\n";
        assert_eq!(first_cargo_error(only_location), None);
        assert_eq!(first_cargo_error(""), None);
    }

    /// THE MISATTRIBUTION LEG, and it is the one this gate got wrong first.
    ///
    /// A non-zero exit with no compiler diagnostic is not evidence about the code.
    /// Measured: the `cargo` on `PATH` is an RCH shim that exits 103 with
    /// `[RCH] remote required; refusing local fallback` and zero `error` lines, so the
    /// first version of this gate reported `BUILD_FAILED first_error="non-zero exit
    /// with no error line"` for a crate that compiles in four seconds.
    ///
    /// Both verdicts still REFUSE the commit. The difference is who gets sent to fix
    /// what, and a gate that sends an operator to fix working code stops being trusted.
    #[test]
    fn an_exit_without_a_diagnostic_does_not_blame_the_code() {
        let rch_refusal = "[RCH] remote required; refusing local fallback (no admissible workers: hard_preflight=3,active_project_exclusion=1)\n";
        assert_eq!(
            first_cargo_error(rch_refusal),
            None,
            "an infrastructure refusal carries no compiler diagnostic"
        );
        let inconclusive = CrateVerdict::BuildInconclusive {
            code: Some(103),
            stderr_tail: rch_refusal.trim().to_owned(),
        };
        assert!(!inconclusive.admits(), "still refuses the commit");
        assert_eq!(inconclusive.label(), "BUILD_INCONCLUSIVE");
        assert_eq!(inconclusive.next_action(), "fix-the-build-toolchain-then-re-run");
        assert_ne!(
            inconclusive.next_action(),
            CrateVerdict::BuildFailed {
                code: Some(101),
                first_error: "error: boom".into()
            }
            .next_action(),
            "the two remedies must differ or the distinction buys nothing"
        );
        let mut rows = BTreeMap::new();
        rows.insert("staged-build-gate".to_owned(), inconclusive);
        let text = render_refusal(&rows);
        assert!(text.contains("reason=BUILD_INCONCLUSIVE"), "{text}");
        assert!(text.contains("[RCH] remote required"), "{text}");
    }

    /// Every non-Pass verdict is restrictive. A timeout is NOT a verdict about the
    /// code, and an unspawnable cargo is not a passing build.
    #[test]
    fn every_unknown_is_restrictive() {
        for verdict in [
            CrateVerdict::BuildFailed {
                code: None,
                first_error: "error: boom".into(),
            },
            CrateVerdict::BuildInconclusive {
                code: Some(103),
                stderr_tail: "[RCH] remote required".into(),
            },
            CrateVerdict::Diverged {
                paths: vec!["crates/x/src/lib.rs".into()],
            },
            CrateVerdict::NoTarget,
            CrateVerdict::TimedOut {
                deadline_secs: BUILD_DEADLINE_SECS,
            },
            CrateVerdict::Unspawned {
                detail: "no such file".into(),
            },
        ] {
            assert!(!verdict.admits(), "{} must not admit", verdict.label());
            assert_ne!(verdict.next_action(), "none", "{} owes a remedy", verdict.label());
        }
        assert!(CrateVerdict::Pass.admits());
    }

    /// The divergence refusal is the evidence boundary: a worktree build says nothing
    /// about a commit whose index differs. Measured the same day - `HEAD` carried the
    /// broken `plan-assemble` while the worktree already held an uncommitted fix, so a
    /// worktree build reported CLEAN about a tree that does not compile.
    #[test]
    fn divergence_refuses_rather_than_building_the_wrong_tree() {
        let touched: BTreeSet<String> = ["plan-assemble".to_owned()].into_iter().collect();
        let verdict = fold(
            &touched,
            rows(&[(
                "plan-assemble",
                CrateVerdict::Diverged {
                    paths: vec!["crates/plan-assemble/src/main.rs".into()],
                },
            )]),
        );
        assert!(!verdict.admits());
        let GateVerdict::Refused { rows: refused } = &verdict else {
            panic!("must refuse, got {verdict:?}");
        };
        let text = render_refusal(refused);
        assert!(text.contains("reason=STAGED_WORKTREE_DIVERGENCE"), "{text}");
        assert!(text.contains("crates/plan-assemble/src/main.rs"), "{text}");
    }

    /// One passing crate must not launder a failing sibling.
    #[test]
    fn a_single_failure_refuses_the_whole_commit() {
        let touched: BTreeSet<String> = ["good".to_owned(), "bad".to_owned()].into_iter().collect();
        let verdict = fold(
            &touched,
            rows(&[
                ("good", CrateVerdict::Pass),
                (
                    "bad",
                    CrateVerdict::BuildFailed {
                        code: Some(101),
                        first_error: "error: boom".into(),
                    },
                ),
            ]),
        );
        assert!(!verdict.admits());
        let GateVerdict::Refused { rows: refused } = &verdict else {
            panic!("must refuse");
        };
        // Only the failures are reported: a refusal listing passes buries its cause.
        assert_eq!(refused.len(), 1);
        assert!(refused.contains_key("bad"));
    }
}
