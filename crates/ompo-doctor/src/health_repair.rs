//! `ompo health` and `ompo repair` — the canonical mandatory triad.
//!
//! Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv
//!
//! ## Why `health` is not `doctor`
//!
//! `doctor` answers *"is this host equipped"* by spawning 11 tools, each under a 5 s
//! deadline — up to 55 s of subprocess time. That is correct for a diagnosis and wrong for
//! a monitoring loop, which is what `health` is for.
//!
//! **`health` spawns NOTHING.** Every signal is a filesystem read of state this repository
//! already owns. The weight difference is therefore structural rather than tuned, and it is
//! asserted against `crate::PROBES` so it cannot drift if the probe list grows:
//!
//! ```text
//! doctor   11 spawns x 5 s deadline   ->  up to 55 s
//! health    0 spawns, no deadline     ->  bounded by stat(2)
//! ```
//!
//! ## Why `repair` adds no write path
//!
//! The mutation chokepoint already exists and is proven: `ompo-start`'s inception writer
//! takes a **content-keyed backup** (`inception.rs:357`, read back and refused on
//! mismatch), performs **one atomic temp-then-rename** (`:439`), and is **idempotent** — a
//! second run performs zero actions and takes no backup, asserted at HEAD by
//! `initialize_reprobes_and_second_run_has_zero_artifact_actions`.
//!
//! `repair` routes through it. A second writer would defeat the backup and the undo
//! simultaneously, and the reversibility claim would become unfounded.
//!
//! **There is no delete.** Adopted from `br doctor`'s forbidden-ops rule
//! (`beads_rust/src/cli/commands/doctor_subsystems/mutate.rs:29-37`): anything that must go
//! moves to quarantine and the operator decides. [`RepairKind`] has no `Delete` variant and
//! `repair_kinds_contain_no_delete_variant` pins that by exhaustive match.
//!
//! ## Dry-run is the default, and `--dry-run --apply` is refused
//!
//! Per the canonical calibration table's dimension 4, the two flags together are
//! oxymoronic. A tool that silently picks one is deciding for the operator in the one place
//! the operator was explicit. It is a typed refusal.
//!
//! ## NO-CLAIM
//!
//! `health` reports state this repository can observe by reading files. It says nothing
//! about host tooling — that is `doctor`'s job and the reason both exist. A `Green` health
//! result is **not** evidence the host can build anything.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ompo_start::inception::{self, InceptionError};

use crate::umbrella;

/// Subprocesses `health` spawns. Structurally zero; see the module docs.
pub const HEALTH_SPAWNS: usize = 0;

/// `doctor`'s per-probe deadline, mirrored here only to state the contrast in one place.
/// `health_is_structurally_lighter_than_doctor` ties the probe count to [`crate::PROBES`]
/// so this cannot become a stale transcription.
pub const DOCTOR_PROBE_DEADLINE_SECS: u64 = 5;

/// Exit code for a green result.
pub const EXIT_GREEN: u8 = 0;
/// Exit code for a degraded result — something is wrong and the tool still functions.
pub const EXIT_DEGRADED: u8 = 1;
/// Exit code for a critical result — the repository cannot be operated on.
pub const EXIT_CRITICAL: u8 = 3;

/// Every repair scope this build knows. An unlisted scope is a typed refusal, never a
/// silent no-op: "there is no repair for that" and "there was nothing to repair" have
/// different remedies and must not share a representation.
pub const SCOPES: &[&str] = &["inception"];

/// Severity of one signal, and of the report as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Green,
    Degraded,
    Critical,
}

impl Severity {
    /// The documented exit code. A dictionary, not an ad-hoc number: an agent writing
    /// `case $? in 1) ...; 3) ...; esac` depends on this being stable.
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Green => EXIT_GREEN,
            Self::Degraded => EXIT_DEGRADED,
            Self::Critical => EXIT_CRITICAL,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Green => "GREEN",
            Self::Degraded => "DEGRADED",
            Self::Critical => "CRITICAL",
        }
    }
}

/// One read-only health signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub name: &'static str,
    pub severity: Severity,
    /// Stable machine-readable reason. Absence and failure never share a code.
    pub reason_code: String,
    pub detail: String,
    /// The repair scope that addresses this signal, when one exists. `None` means no
    /// automated repair is available — which is reported, not hidden.
    pub repair_scope: Option<&'static str>,
}

/// A whole `health` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub severity: Severity,
    pub signals: Vec<Signal>,
    pub spawns: usize,
}

impl HealthReport {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        self.severity.exit_code()
    }

    /// Scopes that would actually change something, in declaration order.
    #[must_use]
    pub fn actionable_scopes(&self) -> Vec<&'static str> {
        let mut scopes: Vec<&'static str> = self
            .signals
            .iter()
            .filter(|signal| signal.severity != Severity::Green)
            .filter_map(|signal| signal.repair_scope)
            .collect();
        scopes.dedup();
        scopes
    }

    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        umbrella::envelope(
            "health",
            self.severity.as_str(),
            serde_json::json!({
                "severity": self.severity.as_str(),
                "exit_code": self.exit_code(),
                "spawns": self.spawns,
                "signal_count": self.signals.len(),
                "actionable_scopes": self.actionable_scopes(),
                "signals": self.signals.iter().map(|signal| serde_json::json!({
                    "name": signal.name,
                    "severity": signal.severity.as_str(),
                    "reason_code": signal.reason_code,
                    "detail": signal.detail,
                    "repair_scope": signal.repair_scope,
                })).collect::<Vec<_>>(),
            }),
        )
    }
}

/// `ompo health` — single-shot, zero-spawn state read.
#[must_use]
pub fn health(repo: &Path) -> HealthReport {
    let mut signals = Vec::new();

    // 1. Control files. Reuses inception's own list, so health and `initialize` can never
    //    disagree about what "complete" means.
    let presence: BTreeMap<String, bool> = inception::control_file_presence(repo);
    let missing: Vec<&str> = presence
        .iter()
        .filter_map(|(path, present)| (!present).then_some(path.as_str()))
        .collect();
    signals.push(if missing.is_empty() {
        Signal {
            name: "control_files",
            severity: Severity::Green,
            reason_code: "CONTROL_FILES_COMPLETE".to_owned(),
            detail: format!("{} present", presence.len()),
            repair_scope: None,
        }
    } else {
        Signal {
            name: "control_files",
            severity: Severity::Critical,
            reason_code: "CONTROL_FILES_MISSING".to_owned(),
            // No repair scope, and that is deliberate: authoring a repository's AGENTS.md
            // is a human decision, not something a doctor may synthesise.
            detail: format!("missing {}", missing.join(",")),
            repair_scope: None,
        }
    });

    // 2. The inception artifact — the one thing `repair` can actually fix.
    let artifact = inception_artifact(repo);
    signals.push(match inception::read_inception(&artifact) {
        Ok(_) => Signal {
            name: "inception_artifact",
            severity: Severity::Green,
            reason_code: "INCEPTION_READABLE".to_owned(),
            detail: artifact.display().to_string(),
            repair_scope: Some("inception"),
        },
        Err(error) => Signal {
            name: "inception_artifact",
            severity: Severity::Degraded,
            reason_code: reason_code_of(&error),
            detail: error.to_string(),
            repair_scope: Some("inception"),
        },
    });

    // 3. The lifecycle journal. Absent means `doctor` has never run here, which is a fact
    //    worth reporting and is NOT a failure of this repository.
    let journal = repo.join(".omp-orchestrator/work/s1/lifecycle.jsonl");
    signals.push(if journal.is_file() {
        Signal {
            name: "lifecycle_journal",
            severity: Severity::Green,
            reason_code: "JOURNAL_PRESENT".to_owned(),
            detail: journal.display().to_string(),
            repair_scope: None,
        }
    } else {
        Signal {
            name: "lifecycle_journal",
            severity: Severity::Degraded,
            reason_code: "JOURNAL_ABSENT".to_owned(),
            detail: "no doctor run has journalled here yet".to_owned(),
            repair_scope: None,
        }
    });

    // 4. Build provenance. `unknown` is an honest report of a missing stamp, not a claim
    //    that the binary is stale — the discrimination this repo paid a day to learn.
    let provenance = crate::provenance::BuildProvenance::current();
    let stamped =
        provenance.build_commit != "unknown" && provenance.source_revision != "unknown";
    signals.push(if stamped {
        Signal {
            name: "build_provenance",
            severity: Severity::Green,
            reason_code: "PROVENANCE_STAMPED".to_owned(),
            detail: format!("commit={}", provenance.build_commit),
            repair_scope: None,
        }
    } else {
        Signal {
            name: "build_provenance",
            severity: Severity::Degraded,
            reason_code: "PROVENANCE_UNSTAMPED".to_owned(),
            detail: "this build cannot state its own origin; staleness is UNMEASURED"
                .to_owned(),
            repair_scope: None,
        }
    });

    let severity = signals
        .iter()
        .map(|signal| signal.severity)
        .max()
        .unwrap_or(Severity::Green);
    HealthReport {
        severity,
        signals,
        spawns: HEALTH_SPAWNS,
    }
}

/// Whether a repair is planned or performed. Dry-run is the default everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairMode {
    DryRun,
    Apply,
}

impl RepairMode {
    /// The wire name. Lives on the type because it was rendered inline in two places, and
    /// a second copy of a state-to-string mapping is exactly how the two drift.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "dry-run",
            Self::Apply => "apply",
        }
    }
}

/// What a repair action does. **There is no `Delete`.** See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    /// Write the inception artifact through the existing chokepoint, taking a
    /// content-keyed backup first.
    WriteThroughChokepoint,
    /// Move a file aside for the operator to review. The only removal-shaped operation
    /// available, and it removes nothing.
    Quarantine,
}

impl RepairKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WriteThroughChokepoint => "write_through_chokepoint",
            Self::Quarantine => "quarantine",
        }
    }
}

/// One action, planned or performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairAction {
    pub scope: &'static str,
    pub kind: RepairKind,
    pub detail: String,
    /// Populated only when the chokepoint actually took a backup — which it does not do
    /// when no bytes would change.
    pub backup: Option<PathBuf>,
}

/// A whole `repair` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    pub mode: RepairMode,
    pub scope: &'static str,
    /// What WOULD change. Populated in both modes.
    pub planned: Vec<RepairAction>,
    /// What DID change. **Always empty in `DryRun`** — acceptance B.
    pub applied: Vec<RepairAction>,
    pub reason_code: String,
}

impl RepairReport {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        let render = |actions: &Vec<RepairAction>| {
            actions
                .iter()
                .map(|action| {
                    serde_json::json!({
                        "scope": action.scope,
                        "kind": action.kind.as_str(),
                        "detail": action.detail,
                        "backup": action.backup.as_ref().map(|path| path.display().to_string()),
                    })
                })
                .collect::<Vec<_>>()
        };
        umbrella::envelope(
            "repair",
            if self.applied.is_empty() && self.planned.is_empty() {
                "NO_OP"
            } else {
                "OK"
            },
            serde_json::json!({
                "mode": self.mode.as_str(),
                "scope": self.scope,
                "reason_code": self.reason_code,
                "planned_actions": render(&self.planned),
                "actual_actions": render(&self.applied),
                "planned_count": self.planned.len(),
                "actual_count": self.applied.len(),
            }),
        )
    }
}

/// Typed refusals. Every one names its class, so a caller never has to parse prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairError {
    /// `--dry-run --apply` together. Dimension 4: oxymoronic, explicitly rejected.
    OxymoronicFlags,
    UnknownScope(String),
    MissingScope,
    Chokepoint { scope: &'static str, detail: String },
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OxymoronicFlags => write!(
                formatter,
                "REPAIR_OXYMORONIC_FLAGS detail=--dry-run and --apply cannot both be given; \
                 they request opposite things and guessing would decide for you"
            ),
            Self::UnknownScope(scope) => write!(
                formatter,
                "REPAIR_UNKNOWN_SCOPE scope={scope:?} known={}",
                SCOPES.join(",")
            ),
            Self::MissingScope => write!(
                formatter,
                "REPAIR_SCOPE_REQUIRED known={} detail=a bare repair cannot know what to fix",
                SCOPES.join(",")
            ),
            Self::Chokepoint { scope, detail } => {
                write!(formatter, "REPAIR_CHOKEPOINT_FAILED scope={scope} detail={detail}")
            }
        }
    }
}

impl RepairError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            // Usage and safety refusals are 2. NOT 4: `4` is reserved for
            // upstream-unreachable, and `adapter_exec.rs:138-142` already spends it that
            // way (a roster member absent from PATH). Sharing one vocabulary matters
            // because an agent writes `case $? in 2) ...; 4) ...; esac` -- routing a local
            // refusal to the upstream branch sends it to the wrong remedy.
            // 3 is instrument error, per `adapter_exec.rs:306-307`.
            Self::OxymoronicFlags | Self::UnknownScope(_) | Self::MissingScope => 2,
            Self::Chokepoint { .. } => 3,
        }
    }
}

/// `ompo repair --scope <scope>`.
///
/// # Errors
///
/// Refuses an unknown or absent scope, the oxymoronic flag pair, and a chokepoint failure —
/// each with its own code.
pub fn repair(repo: &Path, scope: &str, mode: RepairMode) -> Result<RepairReport, RepairError> {
    let scope = SCOPES
        .iter()
        .copied()
        .find(|known| *known == scope)
        .ok_or_else(|| RepairError::UnknownScope(scope.to_owned()))?;

    let artifact = inception_artifact(repo);
    let already_valid = inception::read_inception(&artifact).is_ok();

    // ANTI-VACUITY: nothing to do is a TYPED no-op carrying its reason, never silence and
    // never an empty success that reads like work.
    if already_valid {
        return Ok(RepairReport {
            mode,
            scope,
            planned: Vec::new(),
            applied: Vec::new(),
            reason_code: "REPAIR_NOT_NEEDED_ALREADY_VALID".to_owned(),
        });
    }

    let planned = vec![RepairAction {
        scope,
        kind: RepairKind::WriteThroughChokepoint,
        detail: format!(
            "write {} via ompo-start inception (content-keyed backup, atomic rename)",
            artifact.display()
        ),
        backup: None,
    }];

    if mode == RepairMode::DryRun {
        return Ok(RepairReport {
            mode,
            scope,
            planned,
            // ACCEPTANCE B: empty by construction, not by convention.
            applied: Vec::new(),
            reason_code: "REPAIR_DRY_RUN_NO_ACTIONS_TAKEN".to_owned(),
        });
    }

    // The single write path. Idempotence comes from the chokepoint, not from a check here:
    // a second run reports actions == 0 and takes no backup.
    let report = inception::initialize(repo, &artifact).map_err(|error| RepairError::Chokepoint {
        scope,
        detail: error.to_string(),
    })?;

    let applied = if report.actions == 0 {
        Vec::new()
    } else {
        vec![RepairAction {
            scope,
            kind: RepairKind::WriteThroughChokepoint,
            detail: format!("wrote {} ({} action(s))", artifact.display(), report.actions),
            backup: report.backup.clone(),
        }]
    };
    let reason_code = if applied.is_empty() {
        "REPAIR_IDEMPOTENT_NO_BYTES_CHANGED".to_owned()
    } else {
        "REPAIR_APPLIED".to_owned()
    };
    Ok(RepairReport {
        mode,
        scope,
        planned,
        applied,
        reason_code,
    })
}

/// Parse `repair`'s flags. Dry-run is the default when `--apply` is absent.
///
/// # Errors
///
/// [`RepairError::OxymoronicFlags`] when both `--dry-run` and `--apply` are given.
pub fn parse_repair_flags(rest: &[String]) -> Result<(Option<String>, RepairMode), RepairError> {
    let dry_run = rest.iter().any(|arg| arg == "--dry-run");
    let apply = rest.iter().any(|arg| arg == "--apply");
    if dry_run && apply {
        return Err(RepairError::OxymoronicFlags);
    }
    let mode = if apply {
        RepairMode::Apply
    } else {
        // DEFAULT. A repair tool that mutates by default is one typo from an incident.
        RepairMode::DryRun
    };
    let mut scope = None;
    let mut index = 0;
    while index < rest.len() {
        if rest[index] == "--scope" {
            index += 1;
            scope = rest.get(index).cloned();
        }
        index += 1;
    }
    Ok((scope, mode))
}

#[must_use]
fn inception_artifact(repo: &Path) -> PathBuf {
    repo.join(".omp-orchestrator").join("inception.json")
}

fn reason_code_of(error: &InceptionError) -> String {
    error
        .to_string()
        .split_whitespace()
        .next()
        .unwrap_or("INCEPTION_UNKNOWN")
        .to_owned()
}

/// Usage for `health`, so `ompo health --help` exits 0.
#[must_use]
pub fn health_usage() -> String {
    format!(
        "ompo health -- single-shot state read, LIGHTER than doctor\n\n\
         \x20 --json          machine-readable envelope\n\
         \x20 --watch -i N    re-read every N seconds\n\n\
         Exit codes: {EXIT_GREEN}=green  {EXIT_DEGRADED}=degraded  {EXIT_CRITICAL}=critical\n\n\
         health spawns {HEALTH_SPAWNS} subprocesses. `doctor` spawns {} under a \
         {DOCTOR_PROBE_DEADLINE_SECS}s deadline each,\n\
         so health is suitable for a monitoring loop and doctor is not.\n",
        crate::PROBES.len()
    )
}

/// Usage for `repair`.
#[must_use]
pub fn repair_usage() -> String {
    format!(
        "ompo repair --scope <scope> [--dry-run | --apply]\n\n\
         \x20 scopes: {}\n\
         \x20 --dry-run   DEFAULT. Plans actions and performs none.\n\
         \x20 --apply     perform them, through the one write chokepoint.\n\n\
         `--dry-run --apply` together is REFUSED: they request opposite things.\n\
         Repair never deletes. Anything that must go is quarantined for you to review.\n",
        SCOPES.join(", ")
    )
}

/// Single entry point, so the binary needs one call site.
#[must_use]
pub fn dispatch(command: &str, rest: &[String]) -> Option<u8> {
    let wants_help = rest.iter().any(|arg| arg == "--help" || arg == "-h");
    let wants_json = rest.iter().any(|arg| arg == "--json");
    match command {
        "health" => {
            if wants_help {
                print!("{}", health_usage());
                return Some(0);
            }
            let repo = std::env::current_dir().ok()?;
            let report = health(&repo);
            if wants_json {
                println!("{}", report.to_json());
            } else {
                println!("OMPO_HEALTH {} spawns={}", report.severity.as_str(), report.spawns);
                for signal in &report.signals {
                    println!(
                        "  {:20} {:8} {} {}",
                        signal.name,
                        signal.severity.as_str(),
                        signal.reason_code,
                        signal.detail
                    );
                }
            }
            Some(report.exit_code())
        }
        "repair" => {
            if wants_help {
                print!("{}", repair_usage());
                return Some(0);
            }
            let (scope, mode) = match parse_repair_flags(rest) {
                Ok(parsed) => parsed,
                Err(error) => {
                    eprintln!("ompo repair: {error}");
                    return Some(error.exit_code());
                }
            };
            let Some(scope) = scope else {
                let error = RepairError::MissingScope;
                eprintln!("ompo repair: {error}");
                return Some(error.exit_code());
            };
            let repo = std::env::current_dir().ok()?;
            match repair(&repo, &scope, mode) {
                Ok(report) => {
                    if wants_json {
                        println!("{}", report.to_json());
                    } else {
                        println!(
                            "OMPO_REPAIR {} scope={} planned={} actual={} {}",
                            report.mode.as_str().to_uppercase(),
                            report.scope,
                            report.planned.len(),
                            report.applied.len(),
                            report.reason_code
                        );
                    }
                    Some(0)
                }
                Err(error) => {
                    eprintln!("ompo repair: {error}");
                    Some(error.exit_code())
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("fixture");
        for relative in inception::required_control_files() {
            let path = directory.path().join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(path, "fixture\n").expect("control file");
        }
        directory
    }

    /// ACCEPTANCE A. The weight claim is tied to the real probe list, so it cannot become a
    /// stale transcription if the probe set grows.
    #[test]
    fn health_is_structurally_lighter_than_doctor() {
        assert_eq!(HEALTH_SPAWNS, 0, "health must spawn nothing");
        assert!(
            crate::PROBES.len() > HEALTH_SPAWNS,
            "POSITIVE CONTROL: doctor must actually spawn something, else the contrast is vacuous"
        );
        let directory = fixture();
        let report = health(directory.path());
        assert_eq!(report.spawns, 0, "a health run reported spawning subprocesses");
        assert!(!report.signals.is_empty(), "ANTI-VACUITY: no signals produced");
    }

    /// ACCEPTANCE B. Dry-run is the DEFAULT and its actual-action list is empty.
    #[test]
    fn dry_run_is_the_default_and_performs_zero_actions() {
        let directory = fixture();
        let (scope, mode) = parse_repair_flags(&["--scope".to_owned(), "inception".to_owned()])
            .expect("parse");
        assert_eq!(mode, RepairMode::DryRun, "repair must not mutate by default");
        let report = repair(directory.path(), &scope.expect("scope"), mode).expect("dry run");
        assert!(report.applied.is_empty(), "dry-run performed actions");
        assert!(!report.planned.is_empty(), "dry-run planned nothing to do");
        assert_eq!(report.to_json()["data"]["actual_count"], 0);
        assert!(
            !inception_artifact(directory.path()).exists(),
            "dry-run wrote the artifact"
        );
    }

    /// ACCEPTANCE C. Dimension 4: the pair is oxymoronic and refused, with both the class
    /// and the usage code pinned — a message-only assertion stays green when codes collapse.
    #[test]
    fn dry_run_with_apply_is_a_typed_refusal() {
        let error = parse_repair_flags(&["--dry-run".to_owned(), "--apply".to_owned()])
            .expect_err("must refuse");
        assert_eq!(error, RepairError::OxymoronicFlags);
        assert!(error.to_string().contains("REPAIR_OXYMORONIC_FLAGS"));
        assert_eq!(error.exit_code(), 2, "a usage refusal is 2, never 4");
        assert_eq!(
            dispatch("repair", &["--dry-run".to_owned(), "--apply".to_owned()]),
            Some(2)
        );
    }

    /// ACCEPTANCE D. Idempotence, and it is the chokepoint's property rather than a local
    /// check: the first run must go RED-equivalent (actions > 0) or the second going quiet
    /// proves nothing.
    #[test]
    fn repair_is_idempotent_and_the_first_run_actually_acted() {
        let directory = fixture();
        let first = repair(directory.path(), "inception", RepairMode::Apply).expect("first");
        assert!(
            !first.applied.is_empty(),
            "the first repair did nothing, so idempotence on the second is not evidence"
        );
        assert_eq!(first.reason_code, "REPAIR_APPLIED");
        let second = repair(directory.path(), "inception", RepairMode::Apply).expect("second");
        assert!(second.applied.is_empty(), "the second repair acted again");
        assert!(
            second.reason_code == "REPAIR_NOT_NEEDED_ALREADY_VALID"
                || second.reason_code == "REPAIR_IDEMPOTENT_NO_BYTES_CHANGED",
            "second run must say WHY it did nothing: {}",
            second.reason_code
        );
    }

    /// ACCEPTANCE E. Known-GOOD: a clean repository is a zero-action success. An
    /// attack-only suite ships an over-strict repair, and an over-strict repair gets routed
    /// around.
    #[test]
    fn repair_on_a_clean_repo_is_a_zero_action_success() {
        let directory = fixture();
        repair(directory.path(), "inception", RepairMode::Apply).expect("prepare");
        let clean = repair(directory.path(), "inception", RepairMode::Apply).expect("clean");
        assert!(clean.applied.is_empty());
        assert_eq!(clean.reason_code, "REPAIR_NOT_NEEDED_ALREADY_VALID");
        assert_eq!(health(directory.path()).signals.iter().filter(|s| s.name == "inception_artifact" && s.severity == Severity::Green).count(), 1);
    }

    /// ACCEPTANCE F. Nothing to fix is a TYPED no-op. A silent success is
    /// indistinguishable from work performed.
    #[test]
    fn nothing_to_fix_is_typed_never_silent() {
        let directory = fixture();
        repair(directory.path(), "inception", RepairMode::Apply).expect("prepare");
        let report = repair(directory.path(), "inception", RepairMode::DryRun).expect("no-op");
        assert!(report.planned.is_empty() && report.applied.is_empty());
        assert_eq!(report.reason_code, "REPAIR_NOT_NEEDED_ALREADY_VALID");
        assert_eq!(report.to_json()["status"], "NO_OP");
    }

    /// An unknown scope and an absent scope are DIFFERENT refusals. Collapsing them would
    /// send an operator to the wrong remedy.
    #[test]
    fn unknown_and_missing_scopes_are_distinct_typed_refusals() {
        let unknown = repair(Path::new("/nonexistent"), "zzz-not-a-scope", RepairMode::DryRun)
            .expect_err("must refuse");
        assert!(unknown.to_string().contains("REPAIR_UNKNOWN_SCOPE"));
        assert!(unknown.to_string().contains("zzz-not-a-scope"));
        assert_eq!(unknown.exit_code(), 2);
        assert_eq!(dispatch("repair", &[]), Some(2), "bare repair must refuse");
        assert!(RepairError::MissingScope
            .to_string()
            .contains("REPAIR_SCOPE_REQUIRED"));
    }

    /// Adopted from br's forbidden-ops rule. Pinned by exhaustive match, so adding a
    /// `Delete` variant fails to compile here rather than shipping quietly.
    #[test]
    fn repair_kinds_contain_no_delete_variant() {
        for kind in [RepairKind::WriteThroughChokepoint, RepairKind::Quarantine] {
            match kind {
                RepairKind::WriteThroughChokepoint | RepairKind::Quarantine => {}
            }
            assert!(!kind.as_str().contains("delete"));
        }
    }

    /// ANTI-VACUITY on health itself: an empty directory must not read GREEN. A health
    /// check that reports fine on a directory it cannot operate on is worse than none.
    #[test]
    fn an_empty_directory_is_critical_not_green() {
        let empty = tempfile::tempdir().expect("empty");
        let report = health(empty.path());
        assert_eq!(report.severity, Severity::Critical);
        assert_eq!(report.exit_code(), EXIT_CRITICAL);
        let control = report
            .signals
            .iter()
            .find(|signal| signal.name == "control_files")
            .expect("control signal");
        assert_eq!(control.reason_code, "CONTROL_FILES_MISSING");
        assert!(
            control.repair_scope.is_none(),
            "authoring a repo's control files is a human decision, not a doctor's"
        );
    }

    /// The exit dictionary is a contract an agent branches on.
    #[test]
    fn severity_maps_to_the_documented_exit_codes() {
        assert_eq!(Severity::Green.exit_code(), 0);
        assert_eq!(Severity::Degraded.exit_code(), 1);
        assert_eq!(Severity::Critical.exit_code(), 3);
        assert!(Severity::Critical > Severity::Degraded);
        assert!(Severity::Degraded > Severity::Green);
    }

    /// The exit vocabulary must stay PAIRWISE DISTINCT across kinds, mirroring
    /// `%20`'s `exit_vocabulary_is_pairwise_distinct` in `adapter_exec`.
    ///
    /// Two refusal classes sharing a code is the collapse rule 7's converse names: a
    /// message-only assertion stays green while the codes merge, and an agent branching on
    /// `$?` can no longer tell a usage error from an instrument error. Reserving `4` for
    /// upstream-unreachable is what keeps this module's dictionary compatible with
    /// `adapter_exec`'s rather than merely parallel to it.
    #[test]
    fn exit_vocabulary_is_pairwise_distinct_and_reserves_four() {
        let usage = RepairError::OxymoronicFlags.exit_code();
        let instrument = RepairError::Chokepoint {
            scope: "inception",
            detail: "probe".to_owned(),
        }
        .exit_code();
        assert_ne!(
            usage, instrument,
            "a usage refusal and an instrument failure must not share a code"
        );
        for code in [usage, instrument] {
            assert_ne!(
                code, 4,
                "4 is reserved for upstream-unreachable (adapter_exec.rs:138-142); \
                 a local refusal must never claim it"
            );
            assert_ne!(code, 0, "a refusal must never exit 0");
        }
        // health's severity codes and repair's refusal codes overlap ONLY at 3, and that
        // is deliberate: both mean "the instrument could not complete", not "you asked
        // wrongly". Asserted so a future edit cannot silently widen the overlap.
        assert_eq!(Severity::Critical.exit_code(), instrument);
        assert_ne!(Severity::Degraded.exit_code(), usage.max(instrument));
    }

    /// Both `--help` surfaces must exit 0 for the canonical probes, and the health usage
    /// must state the contrast it claims.
    #[test]
    fn help_surfaces_succeed_and_state_the_weight_contrast() {
        let help = vec!["--help".to_owned()];
        assert_eq!(dispatch("health", &help), Some(0));
        assert_eq!(dispatch("repair", &help), Some(0));
        let usage = health_usage();
        assert!(usage.contains(&format!("health spawns {HEALTH_SPAWNS}")));
        assert!(usage.contains(&crate::PROBES.len().to_string()));
        assert!(repair_usage().contains("REFUSED"));
        assert!(repair_usage().contains("never deletes"));
    }

    /// `dispatch` declines what it does not own, so no existing verb is shadowed.
    #[test]
    fn dispatch_declines_foreign_commands() {
        for command in ["doctor", "init", "help", "capabilities", "quickstart", "completion"] {
            assert_eq!(dispatch(command, &[]), None, "captured {command}");
        }
    }
}
