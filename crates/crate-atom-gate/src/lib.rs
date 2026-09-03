#![forbid(unsafe_code)]

//! **What a crate IS here, as a decision function.**
//!
//! # Why this exists
//!
//! Measured 2026-09-03 over the 68 workspace packages: 66 `[lib]`, 58 `[[bin]]`, 60 with a
//! test target, **0 in-crate fuzz targets** (4 live under `fuzz/`, covering 4 crates),
//! **0 claim rows** (no `registries/`), **0 SLO rows** (no `slo.yaml`), and 30 with no
//! in-repo caller. The plan's gate items are all instances of ONE missing rule: nothing
//! says what a crate must contain, so each crate is whatever its author stopped at.
//!
//! # This crate is PURE
//!
//! Part 1 of the atom it enforces says *"the typed kernel: pure decision function(s) over
//! typed inputs, no I/O, no `Command`"*. A gate that violated its own part 1 would be the
//! self-referential defect this repo has hit six times. So the binary measures and this
//! library decides; the only thing crossing the boundary is [`CrateFacts`].
//!
//! # THE INSTALLABILITY PROBLEM, AND THE RATCHET THAT SOLVES IT
//!
//! 68 crates x 9 parts is 612 rows, and roughly 500 are MISSING today because four parts
//! (fuzz, claim, SLO, oracle) have near-zero coverage repo-wide. A gate that refused every
//! one of them would refuse **every commit in the repo** the moment it was wired — and a
//! gate that cannot be installed is worth zero (`fh N043`). A blanket allowance for all
//! 500 would be the opposite failure: vacuously green forever.
//!
//! The resolution is the ratchet this repo already uses for its gate census: a
//! [`SystemicAllowance`] declares a whole part with an `owner`, a `dies_when`, and a
//! **CEILING that may only be lowered**. The gate therefore:
//!
//! - REFUSES immediately when a crate is missing a part with neither a per-crate nor a
//!   systemic allowance — so parts with real coverage stay enforced;
//! - REFUSES when the count of systemically-allowed misses **exceeds its ceiling** — so a
//!   new crate cannot silently widen a known gap;
//! - REFUSES a systemic row whose ceiling is above the live count, because an allowance
//!   with slack is an allowance that never shrinks.
//!
//! That makes the gate blocking against REGRESSION on the day it lands, while the absolute
//! gaps are declared, owned, and mechanically forced downward.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The nine parts of the atom.
///
/// Numbered because the report is consumed by the plan-to-bead materializer (`n92x`),
/// which labels each filed bead `crate:<name>` and `part:<n>`. The discriminants are the
/// label values and are therefore part of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Part {
    /// 1 — the typed kernel: pure decision functions, no I/O.
    Lib = 1,
    /// 2 — a thin CLI over the lib with a versioned envelope and the exit lattice.
    Bin = 2,
    /// 3 — `Verdict`-typed output where a timeout is `Unrun`, never Pass or Refused.
    Verdict = 3,
    /// 4 — `tests/` carrying the five named legs.
    Tests = 4,
    /// 5 — an in-crate fuzz target for every parser/classifier/lattice.
    Fuzz = 5,
    /// 6 — a claim row with a strength and a justifier at least as strong.
    Claim = 6,
    /// 7 — an SLO row where the crate sits on a tick path.
    Slo = 7,
    /// 8 — a named external oracle and the differential test that runs it.
    Oracle = 8,
    /// 9 — a wired caller, proved by trigger REACHABILITY rather than by text.
    WiredCaller = 9,
}

impl Part {
    /// Every part, in order. The single source for iteration, so a tenth part cannot be
    /// added without every consumer seeing it.
    pub const ALL: [Self; 9] = [
        Self::Lib,
        Self::Bin,
        Self::Verdict,
        Self::Tests,
        Self::Fuzz,
        Self::Claim,
        Self::Slo,
        Self::Oracle,
        Self::WiredCaller,
    ];

    /// The label value the materializer files as `part:<n>`.
    pub const fn number(self) -> u8 {
        self as u8
    }

    /// A short name for the row output.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lib => "lib",
            Self::Bin => "bin",
            Self::Verdict => "verdict",
            Self::Tests => "tests",
            Self::Fuzz => "fuzz",
            Self::Claim => "claim",
            Self::Slo => "slo",
            Self::Oracle => "oracle",
            Self::WiredCaller => "wired-caller",
        }
    }
}

/// The five legs part 4 requires, by name.
///
/// MEASURED 2026-09-03: exactly ONE crate in the workspace carries any of these names
/// (`inbox-monitor`, `fires_on_known_bad`). Every other crate names its tests as
/// descriptive sentences. So part 4 is MISSING almost everywhere, and that is the finding
/// rather than a reason to weaken the rule — a leg you cannot find by name is a leg the
/// next agent cannot know to preserve.
pub const REQUIRED_TEST_LEGS: [&str; 5] = [
    "fires_on_known_bad",
    "passes_known_good",
    "mutation_goes_red",
    "empty_scan_is_error",
    "claim_header",
];

/// How a crate's production call site was reached.
///
/// Part 9 is REACHABILITY, not text. A `WIRED_CALLERS` const naming a hook proves a
/// declaration; this proves the trigger exists on the machine that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Another workspace package depends on this one by path.
    ManifestDependency { dependent: String },
    /// The crate's own binary is invoked by an installed git hook, whose bytes contain the
    /// invocation. Carries the machine, because a hook is untracked and per-clone: the
    /// same crate is reachable here and unreachable in a fresh checkout elsewhere.
    InstalledHook { hook: String, machine: String },
    /// A launchd or cron unit names the binary.
    ScheduledUnit { unit: String, machine: String },
    /// Named in another crate's source as a spawned binary.
    SpawnedBy { source: String },
}

impl fmt::Display for Caller {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestDependency { dependent } => write!(f, "manifest-dep:{dependent}"),
            Self::InstalledHook { hook, machine } => write!(f, "hook:{hook}@{machine}"),
            Self::ScheduledUnit { unit, machine } => write!(f, "unit:{unit}@{machine}"),
            Self::SpawnedBy { source } => write!(f, "spawned-by:{source}"),
        }
    }
}

/// Everything the binary measured about one crate. The library reads nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrateFacts {
    /// Package name from `cargo metadata`, never from a directory listing.
    pub name: String,
    /// Has a `[lib]` target.
    pub has_lib: bool,
    /// Has at least one `[[bin]]` target.
    pub has_bin: bool,
    /// Textual evidence that the crate's output is `Verdict`-typed with an `Unrun`-like
    /// arm. TEXTUAL, and the report says so: this cannot prove a timeout maps to `Unrun`.
    pub verdict_markers: BTreeSet<String>,
    /// Test function names found under `tests/`.
    pub test_fn_names: BTreeSet<String>,
    /// In-crate fuzz targets (`<crate>/fuzz/fuzz_targets/*.rs`).
    pub in_crate_fuzz_targets: BTreeSet<String>,
    /// Claim ids from the registry that name this crate.
    pub claim_ids: BTreeSet<String>,
    /// SLO row keys that name this crate.
    pub slo_rows: BTreeSet<String>,
    /// The named external oracle, if the registry declares one.
    pub oracle: Option<String>,
    /// Whether this crate sits on a tick path, which is what makes part 7 apply.
    pub on_tick_path: bool,
    /// Whether the crate declares a parser/classifier/lattice, which is what makes part 5
    /// apply. A crate with no such kernel has nothing to fuzz.
    pub has_fuzzable_kernel: bool,
    /// Reachable production callers.
    pub callers: Vec<Caller>,
}

/// One crate may be excused one part, by a named owner, until a stated condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateAllowance {
    /// The crate excused.
    pub crate_name: String,
    /// The part excused.
    pub part: Part,
    /// Who owns closing it. Never a pid: an owner must outlive the row.
    pub owner: String,
    /// The condition that DELETES this row. A row with no death condition is permanent.
    pub dies_when: String,
}

/// A whole part excused repo-wide, with a ceiling that may only be lowered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemicAllowance {
    /// The part excused.
    pub part: Part,
    /// Who owns closing it.
    pub owner: String,
    /// The condition that deletes the row.
    pub dies_when: String,
    /// The number of crates currently missing this part. **May only be LOWERED.**
    ///
    /// Checked in BOTH directions: a live count above it REFUSES (regression), and a live
    /// count below it also REFUSES (`CEILING_HAS_SLACK`) — because an allowance with slack
    /// is one that never shrinks, and this repo has measured that failure.
    pub ceiling: usize,
}

/// Which table a row belongs to. A TYPE rather than a string tag, so the parser's match
/// is exhaustive and a third table name cannot be filed as one of these two by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableKind {
    /// `[[allowance]]` — one crate, one part.
    Allowance,
    /// `[[systemic]]` — a whole part, with a ceiling.
    Systemic,
}

/// The parsed allowance registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowances {
    /// Per-crate rows.
    pub per_crate: Vec<CrateAllowance>,
    /// Repo-wide part rows.
    pub systemic: Vec<SystemicAllowance>,
}

impl Allowances {
    fn for_crate(&self, crate_name: &str, part: Part) -> Option<&CrateAllowance> {
        self.per_crate
            .iter()
            .find(|row| row.crate_name == crate_name && row.part == part)
    }

    fn for_part(&self, part: Part) -> Option<&SystemicAllowance> {
        self.systemic.iter().find(|row| row.part == part)
    }
}

/// What one (crate, part) cell says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartStatus {
    /// The part is there, with the evidence that showed it.
    Present { evidence: String },
    /// The part does not apply to this crate, with the reason.
    ///
    /// Distinct from `Present` on purpose: a crate with no parser has nothing to fuzz, and
    /// counting that as coverage would inflate the numbers the materializer files from.
    NotApplicable { reason: String },
    /// Absent, and excused by a named row.
    Allowed {
        /// `crate` or `systemic`.
        scope: &'static str,
        owner: String,
        dies_when: String,
    },
    /// Absent, unexcused. This is what refuses the build.
    Missing { detail: String },
}

impl PartStatus {
    /// The status word for the row output and the JSON report.
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Present { .. } => "present",
            Self::NotApplicable { .. } => "n/a",
            Self::Allowed { .. } => "ALLOWED",
            Self::Missing { .. } => "MISSING",
        }
    }

    /// Does this cell refuse the build?
    pub const fn refuses(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Crate name.
    pub crate_name: String,
    /// Which part.
    pub part: Part,
    /// Its status.
    pub status: PartStatus,
}

impl Row {
    /// The human row: `crate x part -> status`.
    pub fn render(&self) -> String {
        let detail = match &self.status {
            PartStatus::Present { evidence } => evidence.clone(),
            PartStatus::NotApplicable { reason } => reason.clone(),
            PartStatus::Allowed {
                scope,
                owner,
                dies_when,
            } => format!("{scope} owner={owner} dies_when=\"{dies_when}\""),
            PartStatus::Missing { detail } => detail.clone(),
        };
        format!(
            "{:<34} part{} {:<12} {:<8} {}",
            self.crate_name,
            self.part.number(),
            self.part.name(),
            self.status.word(),
            detail
        )
    }
}

/// The gate's verdict over a whole scan.
///
/// Four arms because a timeout and an unreadable input are NOT refusals: an instrument
/// that could not look must never render as a subject that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// Every applicable part is present or allowed, and every ceiling is exact.
    Pass,
    /// At least one unexcused MISSING, or a ceiling breached.
    Refused { reasons: Vec<String> },
    /// Nothing was scanned. An empty scan is NOT a pass.
    Unrun { reason: String },
    /// The gate itself could not run correctly.
    InstrumentError { reason: String },
}

impl GateVerdict {
    /// The exit code lattice from part 2: 0 pass, 1 refuse, 2 unrun, 3 instrument error.
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Refused { .. } => 1,
            Self::Unrun { .. } => 2,
            Self::InstrumentError { .. } => 3,
        }
    }

    /// The closed status vocabulary for the JSON envelope.
    pub const fn status(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Refused { .. } => "REFUSED",
            Self::Unrun { .. } => "UNRUN",
            Self::InstrumentError { .. } => "INSTRUMENT_ERROR",
        }
    }
}

/// Assess one crate against the nine parts.
///
/// # Applicability comes before absence
///
/// Parts 5 and 7 are conditional by the atom's own wording — *"for every
/// parser/classifier/lattice"* and *"where the crate sits on a tick path"*. A crate with
/// neither is `NotApplicable`, which is deliberately not `Present`: rolling the two
/// together would let a crate with no kernel inflate the coverage the materializer reads.
pub fn assess_crate(facts: &CrateFacts, allowances: &Allowances) -> Vec<Row> {
    Part::ALL
        .iter()
        .map(|part| Row {
            crate_name: facts.name.clone(),
            part: *part,
            status: assess_part(facts, *part, allowances),
        })
        .collect()
}

fn assess_part(facts: &CrateFacts, part: Part, allowances: &Allowances) -> PartStatus {
    let raw = raw_status(facts, part);
    let PartStatus::Missing { detail } = raw else {
        return raw;
    };
    if let Some(row) = allowances.for_crate(&facts.name, part) {
        return PartStatus::Allowed {
            scope: "crate",
            owner: row.owner.clone(),
            dies_when: row.dies_when.clone(),
        };
    }
    if let Some(row) = allowances.for_part(part) {
        return PartStatus::Allowed {
            scope: "systemic",
            owner: row.owner.clone(),
            dies_when: row.dies_when.clone(),
        };
    }
    PartStatus::Missing { detail }
}

fn raw_status(facts: &CrateFacts, part: Part) -> PartStatus {
    match part {
        Part::Lib => flag(facts.has_lib, "[lib] target", "no [lib] target in cargo metadata"),
        Part::Bin => flag(
            facts.has_bin,
            "[[bin]] target",
            "no [[bin]] target: the lib has no thin CLI over it",
        ),
        Part::Verdict => {
            if facts.verdict_markers.is_empty() {
                PartStatus::Missing {
                    detail: "no Verdict-like type: no Unrun/InstrumentError arm found in src"
                        .to_owned(),
                }
            } else {
                PartStatus::Present {
                    evidence: format!("markers={}", joined(&facts.verdict_markers)),
                }
            }
        }
        Part::Tests => {
            let absent: Vec<&str> = REQUIRED_TEST_LEGS
                .iter()
                .copied()
                .filter(|leg| {
                    !facts
                        .test_fn_names
                        .iter()
                        .any(|name| name.contains(leg))
                })
                .collect();
            if absent.is_empty() {
                PartStatus::Present {
                    evidence: format!("all five legs present of {} tests", facts.test_fn_names.len()),
                }
            } else {
                PartStatus::Missing {
                    detail: format!("absent legs: {}", absent.join(",")),
                }
            }
        }
        Part::Fuzz => {
            if !facts.has_fuzzable_kernel {
                return PartStatus::NotApplicable {
                    reason: "no parser/classifier/lattice declared in src".to_owned(),
                };
            }
            if facts.in_crate_fuzz_targets.is_empty() {
                PartStatus::Missing {
                    detail: "fuzzable kernel with no <crate>/fuzz/fuzz_targets/*.rs".to_owned(),
                }
            } else {
                PartStatus::Present {
                    evidence: joined(&facts.in_crate_fuzz_targets),
                }
            }
        }
        Part::Claim => {
            if facts.claim_ids.is_empty() {
                PartStatus::Missing {
                    detail: "no row in registries/claims.toml names this crate".to_owned(),
                }
            } else {
                PartStatus::Present {
                    evidence: joined(&facts.claim_ids),
                }
            }
        }
        Part::Slo => {
            if !facts.on_tick_path {
                return PartStatus::NotApplicable {
                    reason: "not on a tick path".to_owned(),
                };
            }
            if facts.slo_rows.is_empty() {
                PartStatus::Missing {
                    detail: "on a tick path with no SLO row and no safe_mode_trigger".to_owned(),
                }
            } else {
                PartStatus::Present {
                    evidence: joined(&facts.slo_rows),
                }
            }
        }
        Part::Oracle => match &facts.oracle {
            Some(oracle) => PartStatus::Present {
                evidence: format!("oracle={oracle}"),
            },
            None => PartStatus::Missing {
                detail: "no named external oracle and no differential test".to_owned(),
            },
        },
        Part::WiredCaller => {
            if facts.callers.is_empty() {
                PartStatus::Missing {
                    detail: "NO REACHABLE TRIGGER: no manifest dependent, no installed hook, \
                             no scheduled unit, no spawn site"
                        .to_owned(),
                }
            } else {
                PartStatus::Present {
                    evidence: facts
                        .callers
                        .iter()
                        .map(Caller::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                }
            }
        }
    }
}

fn flag(present: bool, evidence: &str, detail: &str) -> PartStatus {
    if present {
        PartStatus::Present {
            evidence: evidence.to_owned(),
        }
    } else {
        PartStatus::Missing {
            detail: detail.to_owned(),
        }
    }
}

fn joined(set: &BTreeSet<String>) -> String {
    set.iter().cloned().collect::<Vec<_>>().join(",")
}

/// The whole-scan verdict.
///
/// # Anti-vacuity first
///
/// An empty package list is `Unrun`, never `Pass`. A scan that examined nothing reports
/// identically to a clean one otherwise, and this repo has paid for that six times.
pub fn verdict(rows: &[Row], scanned_crates: usize, allowances: &Allowances) -> GateVerdict {
    if scanned_crates == 0 {
        return GateVerdict::Unrun {
            reason: "SCAN_EMPTY: cargo metadata returned zero packages -- nothing was \
                     checked, which is not a pass"
                .to_owned(),
        };
    }
    if rows.is_empty() {
        return GateVerdict::InstrumentError {
            reason: format!(
                "{scanned_crates} crates scanned and ZERO rows produced: the assessor is broken"
            ),
        };
    }
    let mut reasons: Vec<String> = rows
        .iter()
        .filter_map(|row| {
            // EXHAUSTIVE by construction. `state-wildcard-lint` refused the first draft of
            // this function for a `_ => ""` arm over `PartStatus`, and it was right: a
            // tenth status added later would have fallen into that arm and produced a
            // refusal reason with an EMPTY detail -- a row naming a crate and a part with
            // no reason to act on. Matching the one variant that carries a detail, and
            // dropping the rest by returning `None`, makes the omission impossible instead
            // of quiet.
            let detail = match &row.status {
                PartStatus::Missing { detail } => detail,
                PartStatus::Present { .. }
                | PartStatus::NotApplicable { .. }
                | PartStatus::Allowed { .. } => return None,
            };
            Some(format!(
                "MISSING {} part{} {} -- {detail}",
                row.crate_name,
                row.part.number(),
                row.part.name(),
            ))
        })
        .collect();
    reasons.extend(ceiling_breaches(rows, allowances));
    if reasons.is_empty() {
        GateVerdict::Pass
    } else {
        GateVerdict::Refused { reasons }
    }
}

/// Ceiling arithmetic, in both directions.
///
/// A live count ABOVE the ceiling is a regression: a new crate widened a known gap.
/// A live count BELOW it is `CEILING_HAS_SLACK`: the gap shrank and the row did not, so
/// the next regression would be invisible. Both refuse, and the second is the leg that
/// makes the ratchet actually ratchet.
pub fn ceiling_breaches(rows: &[Row], allowances: &Allowances) -> Vec<String> {
    let mut live: BTreeMap<Part, usize> = BTreeMap::new();
    for row in rows {
        if matches!(&row.status, PartStatus::Allowed { scope, .. } if *scope == "systemic") {
            *live.entry(row.part).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    for row in &allowances.systemic {
        let count = live.get(&row.part).copied().unwrap_or(0);
        if count > row.ceiling {
            out.push(format!(
                "CEILING_BREACHED part{} {} live={count} ceiling={} owner={} -- a crate \
                 widened a known gap; wire the part or lower nothing and explain",
                row.part.number(),
                row.part.name(),
                row.ceiling,
                row.owner
            ));
        } else if count < row.ceiling {
            out.push(format!(
                "CEILING_HAS_SLACK part{} {} live={count} ceiling={} owner={} -- the gap \
                 shrank and the row did not. Lower the ceiling to {count}; an allowance \
                 with slack cannot detect the next regression",
                row.part.number(),
                row.part.name(),
                row.ceiling,
                row.owner
            ));
        }
    }
    out
}

/// Parse the allowance registry.
///
/// A hand-rolled reader for the two table shapes, because this crate must not take a TOML
/// dependency to read seven fields — and because a malformed row is REFUSED by name rather
/// than skipped. A skipped allowance row is a silently unexcused crate.
pub fn parse_allowances(text: &str) -> Result<Allowances, String> {
    let mut out = Allowances::default();
    let mut kind: Option<TableKind> = None;
    let mut fields: BTreeMap<String, String> = BTreeMap::new();
    let mut line_no = 0usize;
    let mut start_line = 0usize;

    let mut flush = |kind: Option<TableKind>,
                     fields: &BTreeMap<String, String>,
                     at: usize,
                     out: &mut Allowances|
     -> Result<(), String> {
        let Some(kind) = kind else { return Ok(()) };
        let get = |key: &str| -> Result<String, String> {
            fields
                .get(key)
                .cloned()
                .ok_or_else(|| format!("ALLOWANCE_MALFORMED line={at} missing field {key:?}"))
        };
        let part = parse_part(&get("part")?, at)?;
        // Exhaustive over a TYPE rather than over string tags. The first draft matched
        // on `&'static str` with a `_` arm, which meant a third table name would have been
        // silently filed as systemic -- an allowance nobody declared.
        match kind {
            TableKind::Allowance => out.per_crate.push(CrateAllowance {
                crate_name: get("crate")?,
                part,
                owner: non_empty(get("owner")?, "owner", at)?,
                dies_when: non_empty(get("dies_when")?, "dies_when", at)?,
            }),
            TableKind::Systemic => out.systemic.push(SystemicAllowance {
                part,
                owner: non_empty(get("owner")?, "owner", at)?,
                dies_when: non_empty(get("dies_when")?, "dies_when", at)?,
                ceiling: get("ceiling")?.parse().map_err(|_| {
                    format!("ALLOWANCE_MALFORMED line={at} ceiling is not a number")
                })?,
            }),
        }
        Ok(())
    };

    for line in text.lines() {
        line_no += 1;
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "[[allowance]]" || trimmed == "[[systemic]]" {
            flush(kind, &fields, start_line, &mut out)?;
            fields.clear();
            kind = Some(if trimmed == "[[allowance]]" {
                TableKind::Allowance
            } else {
                TableKind::Systemic
            });
            start_line = line_no;
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            fields.insert(
                key.trim().to_owned(),
                value.trim().trim_matches('"').to_owned(),
            );
        } else {
            return Err(format!(
                "ALLOWANCE_MALFORMED line={line_no}: not a table header and not key = value"
            ));
        }
    }
    flush(kind, &fields, start_line, &mut out)?;

    let mut seen: BTreeSet<Part> = BTreeSet::new();
    for row in &out.systemic {
        if !seen.insert(row.part) {
            return Err(format!(
                "ALLOWANCE_MALFORMED duplicate systemic row for part{} -- two ceilings for \
                 one part cannot both be authoritative",
                row.part.number()
            ));
        }
    }
    Ok(out)
}

fn non_empty(value: String, field: &str, at: usize) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(format!(
            "ALLOWANCE_MALFORMED line={at} field {field:?} is empty -- an allowance with no \
             {field} is a permanent exception wearing a process costume"
        ));
    }
    Ok(value)
}

fn parse_part(value: &str, at: usize) -> Result<Part, String> {
    let number: u8 = value
        .parse()
        .map_err(|_| format!("ALLOWANCE_MALFORMED line={at} part {value:?} is not 1-9"))?;
    Part::ALL
        .iter()
        .copied()
        .find(|part| part.number() == number)
        .ok_or_else(|| format!("ALLOWANCE_MALFORMED line={at} part {number} is not 1-9"))
}

/// The production call sites this gate is invoked from.
///
/// `franken_lean`'s pattern, and the allowance is EMPTY BY DESIGN: a gate that is not
/// invoked is worth zero, so an exception here would defeat the crate's own part 9. A test
/// resolves both rows against the tree.
pub const WIRED_CALLERS: &[(&str, &str)] = &[
    (
        "crates/no-shell-gate/src/bin/pre-commit-gate.rs",
        "multi-gate dispatcher, on staged Cargo.toml or crates/*/tests/*",
    ),
    (
        "crates/orchestration-tick-gate/src/main.rs",
        "supervisor admission, before any dispatch",
    ),
];

/// Deliberately empty. See [`WIRED_CALLERS`].
pub const UNWIRED_ALLOWANCE: &[(&str, &str)] = &[];
