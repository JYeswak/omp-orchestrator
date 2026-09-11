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

/// A typed reason for an environment that prevented a truthful gate result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnmeasurablePrecondition {
    MissingPath { path: String, detail: String },
    MissingExecutable { executable: String, detail: String },
    MissingScheduler { scheduler: String, detail: String },
    MissingDaemon { endpoint: String, detail: String },
    PolicyUnavailable { policy: String, detail: String },
    AllTestsSkipped { expected: usize, skipped: usize, detail: String },
    FixtureScopeUnavailable {
        root: String,
        conflicting_marker: String,
        detail: String,
    },
}

impl UnmeasurablePrecondition {
    /// The leading token of the `Display` form, ON ITS OWN.
    ///
    /// # Why the code has to be separable from the message
    ///
    /// The full `Display` carries a free-text `detail=` that contains spaces and absolute paths,
    /// so it cannot appear inside a comma-joined work list without destroying the list's grammar.
    /// The code can, and **the code is what selects the remedy**: `MISSING_EXECUTABLE` means
    /// *reach the tool*, `POLICY_UNAVAILABLE` means *supply the oracle*, and neither means *fix
    /// the test*. `omp-orchestrator-86zjl` leg 3 exists because collapsing those into one
    /// `unmeasurable=4` sends four different repairs to the same wrong place.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingPath { .. } => "MISSING_PATH",
            Self::MissingExecutable { .. } => "MISSING_EXECUTABLE",
            Self::MissingScheduler { .. } => "MISSING_SCHEDULER",
            Self::MissingDaemon { .. } => "MISSING_DAEMON",
            Self::PolicyUnavailable { .. } => "POLICY_UNAVAILABLE",
            Self::AllTestsSkipped { .. } => "ALL_TESTS_SKIPPED",
            Self::FixtureScopeUnavailable { .. } => "FIXTURE_SCOPE_UNAVAILABLE",
        }
    }
}

fn one_line_detail(detail: &str) -> String {
    detail
        .chars()
        .map(|character| match character {
            '\n' | '\r' => ' ',
            character => character,
        })
        .collect()
}

impl fmt::Display for UnmeasurablePrecondition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { path, detail } => write!(formatter, "MISSING_PATH path={path} detail={}", one_line_detail(detail)),
            Self::MissingExecutable { executable, detail } => write!(formatter, "MISSING_EXECUTABLE executable={executable} detail={}", one_line_detail(detail)),
            Self::MissingScheduler { scheduler, detail } => write!(formatter, "MISSING_SCHEDULER scheduler={scheduler} detail={}", one_line_detail(detail)),
            Self::MissingDaemon { endpoint, detail } => write!(formatter, "MISSING_DAEMON endpoint={endpoint} detail={}", one_line_detail(detail)),
            Self::PolicyUnavailable { policy, detail } => write!(formatter, "POLICY_UNAVAILABLE policy={policy} detail={}", one_line_detail(detail)),
            Self::AllTestsSkipped { expected, skipped, detail } => write!(formatter, "ALL_TESTS_SKIPPED expected={expected} skipped={skipped} detail={}", one_line_detail(detail)),
            Self::FixtureScopeUnavailable { root, conflicting_marker, detail } => write!(formatter, "FIXTURE_SCOPE_UNAVAILABLE root={root} conflicting_marker={conflicting_marker} detail={}", one_line_detail(detail)),
        }
    }
}
/// Crates that legitimately contribute no test invocation, each with a reason.
///
/// **EMPTY BY DESIGN, and that is the measured outcome rather than an aspiration.** The bead
/// anticipated `asupersync-conformance` needing a row here for having "ZERO test targets". It does
/// not: it has **six** #[test] functions in a #[cfg(test)] module, so it contributes a --lib
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
    /// At least one invocation failed. Any measured environment limitation is preserved beside the failure.
    Failed {
        failing: Vec<String>,
        unmeasurable: Option<UnmeasurablePrecondition>,
    },
    /// The environment could not answer — e.g. `br` absent on a Contabo worker. Neither a pass nor
    /// a failure, and never folded into either.
    Unmeasurable { reason: UnmeasurablePrecondition },
    /// Fewer results than the roster expected.
    Short { expected: usize, observed: usize },
    /// No invocations, with its disposition stated.
    NoTests { disposition: NoTestsDisposition },
}

impl CrateVerdict {
    /// One operator-facing row for this verdict.
    ///
    /// # Why this is a method and not inlined in `GateReport::render`
    ///
    /// `--run` streams each verdict the moment its crate finishes, so the same row is emitted
    /// twice: once live and once in the final report. Two `format!` sites drift — measured in this
    /// very crate, where `failing_targets=unknown` reached an operator because the streamed shape
    /// and the parsed shape were maintained separately. One function makes them byte-identical by
    /// construction, and `the_streamed_row_is_byte_identical_to_the_reported_row` asserts it.
    #[must_use]
    pub fn render_row(&self, name: &str) -> String {
        match self {
            Self::Passed { targets } => format!("PASS crate={name} targets={targets}\n"),
            Self::Failed {
                failing,
                unmeasurable,
            } => {
                let environment = unmeasurable
                    .as_ref()
                    .map(|reason| format!(" unmeasurable={reason}"))
                    .unwrap_or_default();
                format!(
                    "FAIL crate={name} failing_targets={}{}{newline}",
                    failing.join(","),
                    environment,
                    newline = '\n'
                )
            }
            Self::Unmeasurable { reason } => {
                format!("UNMEASURABLE crate={name} reason={reason}\n")
            }
            Self::Short { expected, observed } => format!(
                "SHORT crate={name} expected={expected} observed={observed} \
                 reason=a_target_vanished_or_failed_to_compile\n"
            ),
            Self::NoTests { disposition } => match disposition {
                NoTestsDisposition::DeclaredException { reason } => {
                    format!("NO_TESTS crate={name} disposition=declared reason={reason}\n")
                }
                NoTestsDisposition::Undeclared => format!(
                    "NO_TESTS crate={name} disposition=UNDECLARED \
                     reason=a_crate_with_no_tests_cannot_gate_anything\n"
                ),
            },
        }
    }

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

    /// The remedy-selecting qualifier for this verdict, when the class alone does not pick one.
    ///
    /// `None` for `PASS`, `FAIL` and `SHORT`: their remedy is already named on their own row
    /// (`failing_targets=`, `expected=/observed=`) and appending a second token would make the
    /// work list claim more than the verdict knows. `UNMEASURABLE` and `NO_TESTS` are the two
    /// classes where the CLASS is not the remedy — an absent executable and an absent oracle are
    /// both `unmeasurable=…` and are repaired in different places.
    #[must_use]
    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Passed { .. } | Self::Failed { .. } | Self::Short { .. } => None,
            Self::Unmeasurable { reason } => Some(reason.code()),
            Self::NoTests { disposition } => Some(match disposition {
                NoTestsDisposition::DeclaredException { .. } => "DECLARED",
                NoTestsDisposition::Undeclared => "UNDECLARED",
            }),
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
        out.push_str(&self.work_list());
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
            out.push_str(&verdict.render_row(name));
        }
        out
    }

    /// ONE line per non-PASS class, naming every crate in it — `omp-orchestrator-86zjl`.
    ///
    /// ```text
    /// GATE_RUNNER_FAILING count=17 names=agent-mail-native,dispatch-silence-watch,…
    /// GATE_RUNNER_UNMEASURABLE count=4 names=admission-reason:POLICY_UNAVAILABLE,finding:MISSING_EXECUTABLE,…
    /// GATE_RUNNER_SHORT count=0 names=NONE
    /// GATE_RUNNER_NO_TESTS count=0 names=NONE
    /// ```
    ///
    /// # Why this exists when [`CrateVerdict::render_row`] already names every crate
    ///
    /// **The rows were never missing, and they reconcile exactly. They were UNADDRESSABLE, which
    /// is a different defect with a different repair.** Measured twice independently on CI run
    /// `34543513267` (462,955 bytes, head `568f2dfe`):
    ///
    /// ```text
    /// grep -c GATE_RUNNER                      ->   9   the whole documented marker family
    /// of those 9, naming a crate verdict       ->   0   <- THE DEFECT, one prefix wide
    /// distinct FAIL crate= names               ->  17   == the aggregate's fail=17  ✓
    /// distinct UNMEASURABLE crate= names       ->   4   == unmeasurable=4           ✓
    /// distinct PASS crate= names               ->  67   == pass=67, and 67+17+4=88  ✓
    /// ```
    ///
    /// So nothing was wrong with the arithmetic. What was wrong is that **the one marker a
    /// reader is told to grep cannot see a single crate identity.** The bead that produced this
    /// method quotes those nine lines verbatim as *"THE ENTIRE per-crate output"* of the log —
    /// an honest reading of `grep GATE_RUNNER` over half a megabyte, and wrong by ~250 rows.
    /// AGENTS.md already records the class: *an instrument that cannot return the other answer
    /// is not a measurement.*
    ///
    /// Two lesser frictions ride along, and both argue for a SUMMARY line rather than for
    /// tagging the rows. Every row is printed twice — streamed as its crate lands, then again
    /// in the report, byte-identically and on purpose — so `grep -c 'FAIL crate='` answers 34,
    /// not 17. And `CHECK_FAIL crate=` contains `FAIL crate=` as a SUBSTRING, which lifts that
    /// 34 to 42. A reader who does not already know both facts cannot reconcile the log by
    /// counting.
    ///
    /// **Why not simply prefix the 250 rows instead.** Because the census is valuable precisely
    /// because it is short: `grep GATE_RUNNER` returning nine readable lines is the summary, and
    /// prefixing every row would return 250 and destroy it. The repair is to put the identities
    /// INTO the summary — which is what this is. Four lines, so the census becomes thirteen and
    /// carries the names.
    ///
    /// The line is therefore (a) inside the marker family a reader actually greps, (b) singular,
    /// so counting it cannot double, and (c) self-reconciling: `count` and the length of `names`
    /// come from the same filter over the same map that produced the aggregate, so the three
    /// numbers cannot disagree.
    ///
    /// **Consumer:** an operator or agent turning a red run into claimable work — 86zjl's words,
    /// *"no agent can claim a failing crate, no bead can cite one, and nobody can tell whether
    /// today's 17 are yesterday's 17."* The last of those is why the names are sorted and on one
    /// line: two runs' work lists diff.
    ///
    /// **Deletion condition:** when the per-crate rows themselves become addressable — a distinct
    /// non-substring token per class, emitted exactly once per run — this line is redundant and
    /// must go, because two places naming the same seventeen crates is the drift this crate
    /// already pays a byte-identity test to avoid.
    ///
    /// **`count=0 names=NONE` is emitted for an empty class on purpose.** An absent line and a
    /// zero line are different facts, and this repository has paid for reading the first as the
    /// second. A green run says so in four lines rather than in silence.
    fn work_list(&self) -> String {
        let mut out = String::new();
        for (token, class) in [
            ("GATE_RUNNER_FAILING", "FAIL"),
            ("GATE_RUNNER_UNMEASURABLE", "UNMEASURABLE"),
            ("GATE_RUNNER_SHORT", "SHORT"),
            ("GATE_RUNNER_NO_TESTS", "NO_TESTS"),
        ] {
            let names: Vec<String> = self
                .verdicts
                .iter()
                .filter(|(_, verdict)| verdict.code() == class)
                .map(|(name, verdict)| match verdict.reason_code() {
                    Some(reason) => format!("{name}:{reason}"),
                    None => name.clone(),
                })
                .collect();
            out.push_str(&format!(
                "{token} count={} names={}\n",
                names.len(),
                if names.is_empty() {
                    "NONE".to_owned()
                } else {
                    names.join(",")
                }
            ));
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
    pub unmeasurable: Option<UnmeasurablePrecondition>,
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
    build_report_scoped(roster, roster, observations, ledger)
}

/// As [`build_report`], but drift is computed against a SEPARATE full roster.
///
/// # Why this split exists
///
/// `--only` narrows what is RUN; it does not narrow the workspace. Comparing the ledger against a
/// filtered roster reported **87 spurious** `in_ledger_absent_from_workspace` rows — and because
/// [`GateReport::exit_code`] ranks drift ABOVE gate failures, a scoped run returned
/// [`EXIT_LEDGER_DRIFT`] regardless of the real verdict, making `--only` useless for exactly the
/// decision it exists to support.
///
/// Found by this crate's own anti-vacuity leg (`--run --only <nonexistent>`), which is the case
/// that exists to prove an empty scope is an ERROR — and which surfaced a second defect on the way.

/// The verdict for ONE crate, from its roster entry and whatever was observed for it.
///
/// # Why this is public and separate
///
/// `--run` streams a verdict the moment its crate finishes, so the per-crate decision has to
/// exist before any `GateReport` does. Recomputing it a second way in the streaming path is how
/// a live row and a final row come to disagree — and a disagreement there is worse than no
/// streaming at all, because an operator would be reading two different answers to one question.
/// `build_report_scoped` calls this in a loop; `main` calls it once per crate as it lands.
///
/// `None` observations mean the crate was never run, which is `Short`, never `Passed`: a crate
/// absent from the observation map has produced no evidence, and absence of evidence must not
/// render as a pass.
#[must_use]
pub fn verdict_for(entry: &RosterEntry, observed: Option<&Observed>) -> CrateVerdict {
    if entry.invocations.is_empty() {
        let disposition = NO_TESTS_ALLOWANCE
            .iter()
            .find(|(name, _)| *name == entry.crate_name)
            .map_or(NoTestsDisposition::Undeclared, |(_, reason)| {
                NoTestsDisposition::DeclaredException { reason }
            });
        return CrateVerdict::NoTests { disposition };
    }
    let Some(obs) = observed else {
        return CrateVerdict::Short {
            expected: entry.expected(),
            observed: 0,
        };
    };
    if !obs.failed.is_empty() {
        return CrateVerdict::Failed {
            failing: obs.failed.clone(),
            unmeasurable: obs.unmeasurable.clone(),
        };
    }
    if let Some(reason) = &obs.unmeasurable {
        return CrateVerdict::Unmeasurable {
            reason: reason.clone(),
        };
    }
    let seen = obs.passed.len() + obs.failed.len();
    if seen < entry.expected() {
        return CrateVerdict::Short {
            expected: entry.expected(),
            observed: seen,
        };
    }
    CrateVerdict::Passed { targets: seen }
}

#[must_use]
pub fn build_report_scoped(
    roster: &[RosterEntry],
    full_roster: &[RosterEntry],
    observations: &BTreeMap<String, Observed>,
    ledger: &BTreeSet<String>,
) -> GateReport {
    let derived: BTreeSet<String> = full_roster.iter().map(|e| e.crate_name.clone()).collect();
    let mut verdicts = BTreeMap::new();

    for entry in roster {
        let verdict = verdict_for(entry, observations.get(&entry.crate_name));
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

/// A gate's own declared check invocation, read from ITS OWN manifest.
///
/// # Why this is not a list in this crate
///
/// The roster's TEST half is fully derivable: a crate's test targets are what cargo builds. The
/// RUN half is not — `undrained-pipe-lint` wants `.`, `porting-gate` wants
/// `--repo . --crate porting-gate`, `commit-build-fence` wants `init` then `check`. Those are
/// semantics, and no amount of metadata inspection recovers them.
///
/// A central list in `gate-runner` would mean a new gate crate needs an edit HERE, which is the
/// YAML fan-out moved into Rust — the precise failure `fsu7` item 5 forbids. So each gate declares
/// its own invocation in its own `Cargo.toml`:
///
/// ```toml
/// [package.metadata.gate]
/// checks = [["--repo", "{repo}"]]
/// ```
///
/// The declaration travels with the crate, so adding a gate touches only that gate.
///
/// `{repo}` and `{scratch}` are substituted by the runner, because a check needing an absolute
/// path must not hardcode one — that is `path-literal-guard`'s own subject, and an author's home
/// directory baked into a manifest is exactly how it fires.
///
/// # Multi-bin crates
///
/// MEASURED 2026-09-07: `gate.yml` invokes **15 distinct gate bins across 13 crates**, and
/// `no-shell-gate` alone hosts **three** — `no-shell-gate`, `gate-reachability` and
/// `head-compiles-gate`. So a crate-keyed schema cannot express the fan-out, and each phase names
/// its own bin. `%6`'s ruling replaced `jobs` with `bins` as item 3's denominator for exactly this
/// reason: the fan-out grows INSIDE jobs, so a job count is structurally blind to it.
///
/// Both stanza forms are accepted, because a single-bin gate should not pay for a multi-bin one:
///
/// ```toml
/// checks = [["--repo", "{repo}"]]                                   # crate's default bin
/// checks = [{ bin = "gate-reachability", args = ["--root", "{repo}"] }]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPhase {
    /// `None` means the crate's default bin — the bare-array form.
    pub bin: Option<String>,
    pub args: Vec<String>,
    /// This phase MUTATES and is therefore setup, not a check. **Declared, never inferred.**
    ///
    /// `%6`'s ruling on `etyur` hazard 1, adopting the recommendation that a declared check must
    /// be side-effect-free. Two failure modes make it binding, and both go here because they are
    /// the *why*:
    ///
    /// 1. **A gate that mutates the tree it gates cannot distinguish two states.** *"The tree was
    ///    already conformant"* and *"I made it conformant"* produce the same green, which is
    ///    unfalsifiable from outside.
    /// 2. **A verdict that depends on which worker ran it is not a verdict.** `commit-build-fence`
    ///    writes `git_dir(repo)/omp-build-registration.json` (`main.rs:279`), and `.git/` is
    ///    excluded from the rch overlay — so *"execute every declared check"* is host-coupled by
    ///    construction.
    pub setup: bool,
    /// An INTENTIONAL zero-argument invocation, distinguishable from a truncated stanza.
    ///
    /// `%6`'s ruling on hazard 2. A bare `[]` cannot distinguish *"no arguments"* from *"someone
    /// truncated this"*, and **a field that cannot distinguish those two states IS the defect** —
    /// picking a reading only chooses which failure is silent:
    ///
    /// ```text
    /// [] read as "no arguments"  -> a TRUNCATED stanza runs and reports PASS
    /// [] read as "truncated"     -> a LEGITIMATE zero-arg check can never be declared
    /// ```
    ///
    /// So `[]` is REFUSED, and the intent is carried by a marker a truncation cannot forge: the
    /// table form with `takes_no_args = true`. A truncation deletes text; it does not invent a
    /// key.
    pub takes_no_args: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckInvocation {
    pub crate_name: String,
    /// One phase per sequential invocation. `commit-build-fence` needs two; most need one.
    pub phases: Vec<CheckPhase>,
}

/// Read every crate's declared checks from `cargo metadata`'s `package.metadata` passthrough.
///
/// A crate with no stanza declares no check, which is not an error: most crates are libraries and
/// only a gate has a repo-scanning verb.
pub fn derive_checks(metadata_json: &str) -> Result<Vec<CheckInvocation>, RosterError> {
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
    let mut checks = Vec::new();
    for package in packages {
        let Some(name) = package.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(declared) = package
            .get("metadata")
            .and_then(|m| m.get("gate"))
            .and_then(|g| g.get("checks"))
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let mut phases = Vec::new();
        for phase in declared {
            // Bare array -> the crate's default bin. Table -> a NAMED bin, which multi-bin
            // crates need. Anything else is UNREADABLE rather than "no check declared":
            // silently treating a broken declaration as absent is how a gate stops running
            // while everything reads green.
            if let Some(argv) = phase.as_array() {
                // HAZARD 2: a bare `[]` is REFUSED. An empty argv cannot distinguish an
                // intentional zero-argument check from a truncated stanza, and the bare-array
                // form has no room to say which. The table form with `takes_no_args = true`
                // carries the intent in a key a truncation cannot invent.
                if argv.is_empty() {
                    return Err(RosterError::MetadataUnreadable {
                        detail: format!(
                            "{name}: metadata.gate.checks has a bare `[]` phase, which is \
                             AMBIGUOUS -- an intentional zero-argument check and a truncated \
                             stanza are indistinguishable in that form. Declare it as \
                             {{ bin = \"<bin>\", args = [], takes_no_args = true }}"
                        ),
                    });
                }
                phases.push(CheckPhase {
                    bin: None,
                    args: argv
                        .iter()
                        .filter_map(|a| a.as_str().map(str::to_owned))
                        .collect(),
                    setup: false,
                    takes_no_args: false,
                });
            } else if let Some(table) = phase.as_object() {
                let Some(bin) = table.get("bin").and_then(serde_json::Value::as_str) else {
                    return Err(RosterError::MetadataUnreadable {
                        detail: format!(
                            "{name}: a metadata.gate.checks table phase must name a `bin`"
                        ),
                    });
                };
                let args: Vec<String> = table
                    .get("args")
                    .and_then(serde_json::Value::as_array)
                    .map(|argv| {
                        argv.iter()
                            .filter_map(|a| a.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let takes_no_args = table
                    .get("takes_no_args")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                if args.is_empty() && !takes_no_args {
                    return Err(RosterError::MetadataUnreadable {
                        detail: format!(
                            "{name}: table phase `{bin}` declares no args and does not set \
                             `takes_no_args = true` -- an empty argv is AMBIGUOUS, not empty"
                        ),
                    });
                }
                phases.push(CheckPhase {
                    bin: Some(bin.to_owned()),
                    args,
                    setup: table
                        .get("setup")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    takes_no_args,
                });
            } else {
                return Err(RosterError::MetadataUnreadable {
                    detail: format!(
                        "{name}: metadata.gate.checks must be a list of argv lists or \
                         {{ bin, args }} tables"
                    ),
                });
            }
        }
        // HAZARD 1, the ENFORCEABLE form: SETUP MUST BE DECLARED, NEVER INFERRED.
        //
        // A side effect cannot be detected statically, so the refusal keys on the shape that
        // carries one: THE SAME BIN INVOKED TWICE. `commit-build-fence` runs
        // `commit-build-fence init` then `commit-build-fence check`, and the `init` writes
        // `git_dir(repo)/omp-build-registration.json` — a sequence, where everything before the
        // last invocation exists to set up the last one.
        //
        // KEYING ON POSITION INSTEAD WAS WRONG AND A FIXTURE CAUGHT IT. My first version refused
        // any phase before the last that did not declare setup, which refused `no-shell-gate` —
        // three DISTINCT bins (`no-shell-gate`, `gate-reachability`, `head-compiles-gate`), all
        // genuine checks, none setup. A multi-BIN crate is a fan-out; a repeated bin is a
        // sequence. `a_crate_hosting_several_gate_bins_keeps_all_of_them` went RED and it was
        // right: the rule, not the fixture, was over-broad.
        let mut refusal = None;
        for (index, phase) in phases.iter().enumerate() {
            let later_same_bin = phases[index + 1..].iter().any(|next| next.bin == phase.bin);
            if later_same_bin && !phase.setup {
                refusal = Some(format!(
                    "{name}: phase {index} invokes bin `{}` which is invoked again later and does \
                     not declare `setup = true`. A repeated bin is a SEQUENCE, so every earlier \
                     invocation exists to set up the last one -- and a declared CHECK must be \
                     side-effect-free, because a gate that mutates the tree it gates cannot \
                     distinguish \"already conformant\" from \"I made it conformant\": both \
                     produce the same green",
                    phase.bin.as_deref().unwrap_or("<default>")
                ));
                break;
            }
        }
        if let Some(detail) = refusal {
            return Err(RosterError::MetadataUnreadable { detail });
        }
        if !phases.is_empty() {
            checks.push(CheckInvocation {
                crate_name: name.to_owned(),
                phases,
            });
        }
    }
    checks.sort_by(|a, b| a.crate_name.cmp(&b.crate_name));
    Ok(checks)
}

/// Substitute `{repo}` and `{scratch}` in a declared argv.
#[must_use]
pub fn expand(argv: &[String], repo: &str, scratch: &str) -> Vec<String> {
    argv.iter()
        .map(|arg| arg.replace("{repo}", repo).replace("{scratch}", scratch))
        .collect()
}

/// What a former `gate.yml` job did, and what now subsumes it.
///
/// `fsu7` item 10 forbids deleting a job to reduce the count: the 2026-09-06 defect was a MISSING
/// job key that collapsed two jobs into one, and the remedy was a one-line RESTORE. So subsumption
/// is asserted MECHANICALLY rather than described in prose — a table in a commit message cannot
/// fail, and this can.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subsumption {
    /// Its `cargo test -p X` half is in the derived roster, and its `cargo run` half (if any) is a
    /// declared check.
    Covered { tests: bool, checks: bool },
    /// The job ran something this runner cannot yet express. NAMED, never dropped.
    NotSubsumed { reason: String },
}

/// Decide, per former job crate, whether the entry point subsumes it.
///
/// `ran_binary` says whether the old job invoked the crate's binary as well as its tests: a job
/// with a run half needs a declared check, a test-only job does not.
#[must_use]
pub fn subsumption(
    job_crate: &str,
    ran_binary: bool,
    roster: &[RosterEntry],
    checks: &[CheckInvocation],
) -> Subsumption {
    let tests = roster
        .iter()
        .any(|e| e.crate_name == job_crate && !e.invocations.is_empty());
    let declared = checks.iter().any(|c| c.crate_name == job_crate);
    if !tests {
        return Subsumption::NotSubsumed {
            reason: format!("{job_crate} contributes no test invocation to the derived roster"),
        };
    }
    if ran_binary && !declared {
        return Subsumption::NotSubsumed {
            reason: format!(
                "{job_crate} ran its BINARY in gate.yml and declares no \
                 [package.metadata.gate] checks — its test half is covered, its run half is NOT"
            ),
        };
    }
    Subsumption::Covered {
        tests,
        checks: declared,
    }
}
