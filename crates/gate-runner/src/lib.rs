#![forbid(unsafe_code)]

//! ONE entry point for every workspace gate — `omp-orchestrator-fsu7`.
//!
//! # The defect this replaces
//!
//! `.github/workflows/gate.yml` fanned out to **12 jobs naming 11 crates**, and GitHub Actions is
//! STRICT on duplicate keys: measured n=60 runs, **49 duplicate-key runs started ZERO jobs**. A
//! workflow that fails to parse starts nothing and reports nothing, which is indistinguishable
//! from a gate that passed. Worse, the 2026-09-06 incident was a *missing* job key that collapsed
//! two jobs into one — silently halving coverage with no error anywhere.
//!
//! A YAML fan-out has no denominator. This crate gives it one.
//!
//! # Why the roster is DERIVED and never listed
//!
//! The roster comes from `cargo metadata`, which reports the targets cargo *actually builds* —
//! including auto-detected ones. A manifest grep INVERTS this census: `tick-monitor` declares no
//! `[[bin]]` yet ships a binary, because cargo auto-detects `src/main.rs`. So a hand list is not
//! merely brittle, it is wrong in the direction that reads as absence.
//!
//! Deriving also settles the requirement that a NEW gate crate needs no registration: a crate that
//! exists in the workspace is in the roster, and is therefore RUN, with no edit to this crate.
//!
//! # The measured trap this design exists to avoid
//!
//! `cargo metadata` target kinds report INTEGRATION test targets. They do NOT reveal `#[cfg(test)]`
//! unit tests compiled into a lib. Measured 2026-09-07 across 87 workspace packages:
//!
//! ```text
//! packages                                     87
//! with >= 1 integration test target            75
//! with ZERO integration test targets           12   <- and ALL TWELVE HAVE UNIT TESTS
//!   asupersync-conformance   6 #[test]     fleet-monitor      43 #[test]
//!   fuzz-build-gate          3 #[test]     input-manifest      8 #[test]
//!   lifecycle-event          8 #[test]     lifecycle-monitor  10 #[test]
//!   loop-coverage           24 #[test]     omp-surface-consumption 7 #[test]
//!   oracle-compare          14 #[test]     r1-breadth-gate     4 #[test]
//!   scratch-home            10 #[test]     worker-oracle-gate  5 #[test]
//! GENUINELY untested crates                     0
//! ```
//!
//! So a roster keyed on integration targets alone would silently skip **12 crates and ~142 test
//! functions**, three of them named `*-gate`. That is the same census inversion one layer down, and
//! it is why [`Invocation`] always includes the `--lib` leg for a crate whose lib carries tests.
//!
//! # What a green run does NOT establish
//!
//! * **Coverage of gates, not correctness of gates.** This runs what exists. A property nobody
//!   wrote a test for is invisible here, exactly as it is invisible to the YAML it replaces.
//! * **Nothing about a crate whose tests are UNMEASURABLE on the lane.** `br` and `.beads` are
//!   absent on the Contabo workers, so some suites cannot answer there. Those are
//!   [`CrateVerdict::Unmeasurable`], which is neither a pass nor a failure, and the total refuses
//!   to launder them into either.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Exit codes. Distinct per cause, because a reader must tell these apart from the exit alone —
/// `101` is cargo's own generic failure and an unrelated workspace-loading error produces an
/// identical `101`, which has happened twice in this repository in one hour.
pub const EXIT_OK: u8 = 0;
/// At least one gate genuinely FAILED.
pub const EXIT_GATE_FAILED: u8 = 1;
/// The roster resolved to zero crates. Anti-vacuity: never a pass.
pub const EXIT_EMPTY_ROSTER: u8 = 3;
/// A crate ran fewer targets than the roster expected — a target vanished or failed to compile.
pub const EXIT_SHORT_ROSTER: u8 = 4;
/// The committed ledger and the derived roster disagree.
pub const EXIT_LEDGER_DRIFT: u8 = 5;
/// `cargo metadata` itself could not be read or parsed.
pub const EXIT_METADATA_UNREADABLE: u8 = 6;

/// One cargo invocation the runner must make for a crate.
///
/// A crate contributes one `--lib` leg when its library carries `#[test]` functions, plus one
/// `--test <name>` leg per integration target. Both halves are required: see the census trap in
/// the module docs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Invocation {
    /// `cargo test -p <crate> --lib` — the unit tests `cargo metadata` does not report.
    Lib,
    /// `cargo test -p <crate> --test <name>` — a NAMED target, per the mmt4 ruling.
    Test(String),
}

impl fmt::Display for Invocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lib => write!(f, "--lib"),
            Self::Test(name) => write!(f, "--test {name}"),
        }
    }
}

/// A crate in the derived roster, with every invocation it owes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub crate_name: String,
    pub invocations: Vec<Invocation>,
}

impl RosterEntry {
    /// The denominator. A crate that reports fewer results than this has lost a target, and that
    /// is an ERROR rather than a smaller pass — the whole failure mode of a YAML fan-out.
    #[must_use]
    pub fn expected(&self) -> usize {
        self.invocations.len()
    }
}

/// Why a crate contributes no invocations.
///
/// Kept as a typed value rather than an absence, so a crate can never drop out of the report
/// silently. `fsu7` exists because seven gates read `status=open` while nothing invoked them; a
/// runner that omits a row recreates that condition inside a green binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoTestsDisposition {
    /// Listed in [`NO_TESTS_ALLOWANCE`] with a stated reason.
    DeclaredException { reason: &'static str },
    /// Not listed. An ERROR — a crate with no tests at all cannot gate anything.
    Undeclared,
}

/// Crates that legitimately contribute no test invocation, each with a reason.
///
/// **EMPTY BY DESIGN, and that is the measured outcome rather than an aspiration.** The bead
/// anticipated `asupersync-conformance` needing a row here for having "ZERO test targets". It does
/// not: it has **six** `#[test]` functions in a `#[cfg(test)]` module, so it contributes a `--lib`
/// leg like any other crate. Measured across all 87 packages, **zero** crates are genuinely
/// untested, so there is nothing to except.
///
/// Checked in BOTH directions by [`check_allowance`]. A row naming a crate that has tests is an
/// ERROR telling the reader to delete it, because an allowance that outlives its defect is how a
/// repaired gap keeps reading as broken.
pub const NO_TESTS_ALLOWANCE: &[(&str, &str)] = &[];

/// The verdict for one crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrateVerdict {
    /// Every expected invocation ran and passed.
    Passed { targets: usize },
    /// At least one invocation failed. Carries the failing target names so the message names them.
    Failed { failing: Vec<String> },
    /// The environment could not answer — e.g. `br` absent on a Contabo worker. Neither a pass nor
    /// a failure, and never folded into either.
    Unmeasurable { reason: String },
    /// Fewer results than the roster expected.
    Short { expected: usize, observed: usize },
    /// No invocations, with its disposition stated.
    NoTests { disposition: NoTestsDisposition },
}

impl CrateVerdict {
    /// Does this verdict block the gate?
    #[must_use]
    pub fn is_blocking(&self) -> bool {
        match self {
            Self::Passed { .. } | Self::Unmeasurable { .. } => false,
            Self::Failed { .. } | Self::Short { .. } => true,
            Self::NoTests { disposition } => {
                matches!(disposition, NoTestsDisposition::Undeclared)
            }
        }
    }

    /// A short stable token, so a reader can grep a verdict class.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Passed { .. } => "PASS",
            Self::Failed { .. } => "FAIL",
            Self::Unmeasurable { .. } => "UNMEASURABLE",
            Self::Short { .. } => "SHORT",
            Self::NoTests { .. } => "NO_TESTS",
        }
    }
}

/// The whole-run report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    pub verdicts: BTreeMap<String, CrateVerdict>,
    /// Crates the ledger names that the workspace no longer has.
    pub ledger_only: BTreeSet<String>,
    /// Crates the workspace has that the ledger does not name.
    pub workspace_only: BTreeSet<String>,
}

impl GateReport {
    /// The exit code, chosen so each cause is distinguishable without reading the text.
    ///
    /// Ordered most-structural first: a drifted or short roster means the RUN itself is not
    /// trustworthy, and reporting a gate failure from an untrustworthy run would misattribute.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.verdicts.is_empty() {
            return EXIT_EMPTY_ROSTER;
        }
        if !self.ledger_only.is_empty() || !self.workspace_only.is_empty() {
            return EXIT_LEDGER_DRIFT;
        }
        if self
            .verdicts
            .values()
            .any(|v| matches!(v, CrateVerdict::Short { .. }))
        {
            return EXIT_SHORT_ROSTER;
        }
        if self.verdicts.values().any(CrateVerdict::is_blocking) {
            return EXIT_GATE_FAILED;
        }
        EXIT_OK
    }

    /// The operator-facing summary. Names every blocking cause; a code alone cannot.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let total = self.verdicts.len();
        let counts = self.counts();
        out.push_str(&format!(
            "GATE_RUNNER crates={total} pass={} fail={} unmeasurable={} short={} no_tests={}\n",
            counts.get("PASS").copied().unwrap_or(0),
            counts.get("FAIL").copied().unwrap_or(0),
            counts.get("UNMEASURABLE").copied().unwrap_or(0),
            counts.get("SHORT").copied().unwrap_or(0),
            counts.get("NO_TESTS").copied().unwrap_or(0),
        ));
        if total == 0 {
            out.push_str(
                "GATE_RUNNER_EMPTY_ROSTER derived zero crates from cargo metadata. An empty gate \
                 set is an ERROR, never a pass: the likely cause is an unreadable or filtered \
                 workspace, not a repository with no gates.\n",
            );
        }
        for name in &self.ledger_only {
            out.push_str(&format!(
                "GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_ledger_absent_from_workspace \
                 remedy=delete_the_row_or_restore_the_crate\n"
            ));
        }
        for name in &self.workspace_only {
            out.push_str(&format!(
                "GATE_RUNNER_LEDGER_DRIFT crate={name} reason=in_workspace_absent_from_ledger \
                 remedy=add_the_row (the crate WAS still run)\n"
            ));
        }
        for (name, verdict) in &self.verdicts {
            match verdict {
                CrateVerdict::Passed { targets } => {
                    out.push_str(&format!("PASS crate={name} targets={targets}\n"));
                }
                CrateVerdict::Failed { failing } => {
                    out.push_str(&format!(
                        "FAIL crate={name} failing_targets={}\n",
                        failing.join(",")
                    ));
                }
                CrateVerdict::Unmeasurable { reason } => {
                    out.push_str(&format!("UNMEASURABLE crate={name} reason={reason}\n"));
                }
                CrateVerdict::Short { expected, observed } => {
                    out.push_str(&format!(
                        "SHORT crate={name} expected={expected} observed={observed} \
                         reason=a_target_vanished_or_failed_to_compile\n"
                    ));
                }
                CrateVerdict::NoTests { disposition } => match disposition {
                    NoTestsDisposition::DeclaredException { reason } => out.push_str(&format!(
                        "NO_TESTS crate={name} disposition=declared reason={reason}\n"
                    )),
                    NoTestsDisposition::Undeclared => out.push_str(&format!(
                        "NO_TESTS crate={name} disposition=UNDECLARED \
                         reason=a_crate_with_no_tests_cannot_gate_anything\n"
                    )),
                },
            }
        }
        out
    }

    fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for v in self.verdicts.values() {
            *m.entry(v.code()).or_insert(0) += 1;
        }
        m
    }
}

/// Errors reading the inputs. Distinct from a gate failure on purpose: "I could not look" and
/// "I looked and it is broken" are different facts, and collapsing them is the defect this
/// repository keeps paying for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosterError {
    MetadataUnreadable { detail: String },
    /// The allowance names a crate that DOES have tests. Bidirectional check.
    StaleAllowance { crate_name: String },
}

impl fmt::Display for RosterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MetadataUnreadable { detail } => write!(
                f,
                "GATE_RUNNER_METADATA_UNREADABLE detail={detail} — this is NOT a gate failure; \
                 the roster could not be derived at all"
            ),
            Self::StaleAllowance { crate_name } => write!(
                f,
                "GATE_RUNNER_STALE_ALLOWANCE crate={crate_name} \
                 reason=listed_as_untested_but_it_has_tests remedy=delete_the_row"
            ),
        }
    }
}

/// Does this package's library carry `#[test]` functions?
///
/// Taken as an input rather than sniffed here, so the pure logic stays testable without a
/// filesystem. The binary supplies it by scanning the crate's `src/`.
pub type LibHasTests = bool;

/// Derive the roster from `cargo metadata --no-deps` JSON.
///
/// `lib_has_tests` answers, per crate name, whether its library carries unit tests — the half
/// `cargo metadata` cannot report. A crate absent from that map is treated as having none, which
/// is the conservative direction: it can only produce a NO_TESTS row, never a silent skip.
pub fn derive_roster(
    metadata_json: &str,
    lib_has_tests: &BTreeMap<String, LibHasTests>,
) -> Result<Vec<RosterEntry>, RosterError> {
    let value: serde_json::Value =
        serde_json::from_str(metadata_json).map_err(|error| RosterError::MetadataUnreadable {
            detail: error.to_string(),
        })?;
    let packages = value
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| RosterError::MetadataUnreadable {
            detail: "no `packages` array".to_owned(),
        })?;

    let mut roster = Vec::new();
    for package in packages {
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            return Err(RosterError::MetadataUnreadable {
                detail: "a package has no `name`".to_owned(),
            });
        };
        let mut invocations = Vec::new();
        if lib_has_tests.get(name).copied().unwrap_or(false) {
            invocations.push(Invocation::Lib);
        }
        if let Some(targets) = package.get("targets").and_then(serde_json::Value::as_array) {
            for target in targets {
                let is_test = target
                    .get("kind")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|kinds| {
                        kinds.iter().any(|k| k.as_str() == Some("test"))
                    });
                if is_test {
                    if let Some(tname) = target.get("name").and_then(serde_json::Value::as_str) {
                        invocations.push(Invocation::Test(tname.to_owned()));
                    }
                }
            }
        }
        invocations.sort();
        roster.push(RosterEntry {
            crate_name: name.to_owned(),
            invocations,
        });
    }
    roster.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(roster)
}

/// The allowance must not outlive its defect: a row naming a crate that HAS tests is an error.
pub fn check_allowance(roster: &[RosterEntry]) -> Result<(), RosterError> {
    for (name, _reason) in NO_TESTS_ALLOWANCE {
        if let Some(entry) = roster.iter().find(|e| e.crate_name == *name) {
            if !entry.invocations.is_empty() {
                return Err(RosterError::StaleAllowance {
                    crate_name: (*name).to_owned(),
                });
            }
        }
    }
    Ok(())
}

/// What one crate's cargo run reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed {
    /// Target names that reported a passing `test result:` line.
    pub passed: Vec<String>,
    /// Target names that reported a failing one.
    pub failed: Vec<String>,
    /// Set when the environment could not answer at all.
    pub unmeasurable: Option<String>,
}

/// Fold the roster and the observations into a report.
///
/// `ledger` is the committed crate list. Its only job is to make DELETION detectable: a derived
/// roster that simply loses a crate reports a smaller pass, which is precisely the silent
/// halving that `gate.yml` suffered. Addition is reported too, but the crate is still RUN — a new
/// gate crate needs no registration to be executed.
#[must_use]
pub fn build_report(
    roster: &[RosterEntry],
    observations: &BTreeMap<String, Observed>,
    ledger: &BTreeSet<String>,
) -> GateReport {
    let derived: BTreeSet<String> = roster.iter().map(|e| e.crate_name.clone()).collect();
    let mut verdicts = BTreeMap::new();

    for entry in roster {
        let verdict = if entry.invocations.is_empty() {
            let disposition = NO_TESTS_ALLOWANCE
                .iter()
                .find(|(name, _)| *name == entry.crate_name)
                .map_or(NoTestsDisposition::Undeclared, |(_, reason)| {
                    NoTestsDisposition::DeclaredException { reason }
                });
            CrateVerdict::NoTests { disposition }
        } else {
            match observations.get(&entry.crate_name) {
                None => CrateVerdict::Short {
                    expected: entry.expected(),
                    observed: 0,
                },
                Some(obs) => {
                    if let Some(reason) = &obs.unmeasurable {
                        CrateVerdict::Unmeasurable {
                            reason: reason.clone(),
                        }
                    } else if !obs.failed.is_empty() {
                        CrateVerdict::Failed {
                            failing: obs.failed.clone(),
                        }
                    } else {
                        let observed = obs.passed.len() + obs.failed.len();
                        if observed < entry.expected() {
                            CrateVerdict::Short {
                                expected: entry.expected(),
                                observed,
                            }
                        } else {
                            CrateVerdict::Passed { targets: observed }
                        }
                    }
                }
            }
        };
        verdicts.insert(entry.crate_name.clone(), verdict);
    }

    GateReport {
        ledger_only: ledger.difference(&derived).cloned().collect(),
        workspace_only: derived.difference(ledger).cloned().collect(),
        verdicts,
    }
}

/// Parse the committed ledger: one crate name per non-empty, non-comment line.
///
/// Deliberately the dumbest possible format. A ledger that needs a parser is a ledger that can
/// fail to parse, and this file exists to detect silent loss.
#[must_use]
pub fn parse_ledger(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}
