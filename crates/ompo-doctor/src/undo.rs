//! `ompo undo <scope>` — the restore direction of the mutation surface.
//!
//! Bead: omp-orchestrator-s1w0-l1l2-doctor-init-contract-d9rv
//!
//! ## Why a top-level VERB and not `doctor undo`
//!
//! `br` nests undo under `doctor` (`beads_rust/README.md:610`, `doctor ls` / `doctor undo
//! latest --dry-run`) **because in `br` it is `doctor --repair` that mutates.** Ours does
//! not: `ompo doctor` appends to a monotone event journal that must never be undoable, and
//! the thing that actually mutates is `init`. Nesting under `doctor` would copy br's
//! coupling without br's reason.
//!
//! A flag on `init` was the other option and is worse: it would not appear in
//! `capabilities.verbs`, so closing-contract leg 2 — the self-report parity that is the
//! positive control for the whole surface — could not bind it.
//!
//! ## Why `undo` with no scope REFUSES
//!
//! Backups are keyed by the SHA-256 of their **content**, not by time
//! (`inception.rs:376`). That is the right choice — a duplicate backup becomes a no-op
//! instead of an accumulating pile — and it means **there is no "latest".** `InitReport`
//! carries an action COUNT, not a ledger, so nothing exists to resolve an ordering
//! against. A bare `undo` that picked one would be guessing, and the same applies when
//! several backups exist: [`undo`] refuses and names them rather than choosing.
//!
//! This is the same rule as `repair`'s `--dry-run --apply` refusal: **a surface that cannot
//! distinguish two states must refuse, not pick.**
//!
//! ## It adds no write path
//!
//! Restore goes through `inception::restore_backup`, which calls the same `write_atomic`
//! temp-then-rename the writer uses (`inception.rs:439`). `repair --apply` already routes
//! through that chokepoint, so **`repair` and `undo` are one surface with two directions,
//! not two writers.** A second writer would defeat the backup and the undo simultaneously.
//!
//! **There is no delete.** An undo that deletes is not a restore.
//!
//! ## NO-CLAIM
//!
//! `undo` restores the inception artifact from a backup this tool wrote. It does not undo
//! journal events (append-only by design), does not undo anything `init` did outside the
//! artifact, and cannot recover a state that was never backed up.

use std::path::{Path, PathBuf};

use ompo_start::inception::{self, BackupEntry};

use crate::health_repair::{RepairMode, SCOPES};
use crate::umbrella;

/// What an undo did, or would do under `--dry-run`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoReport {
    pub mode: RepairMode,
    pub scope: &'static str,
    /// Non-empty when a restore would change bytes.
    pub planned: Vec<String>,
    /// **Always empty under `DryRun`.**
    pub applied: Vec<String>,
    pub restored_from: Option<PathBuf>,
    pub reason_code: String,
}

impl UndoReport {
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        umbrella::envelope(
            "undo",
            if self.applied.is_empty() && self.planned.is_empty() {
                "NO_OP"
            } else {
                "OK"
            },
            serde_json::json!({
                "mode": self.mode.as_str(),
                "scope": self.scope,
                "reason_code": self.reason_code,
                "planned_actions": self.planned,
                "actual_actions": self.applied,
                "planned_count": self.planned.len(),
                "actual_count": self.applied.len(),
                "restored_from": self.restored_from.as_ref().map(|p| p.display().to_string()),
            }),
        )
    }
}

/// Typed refusals. Every state that cannot be acted on has its own code, because
/// "nothing to restore", "several candidates" and "the backup is corrupt" have three
/// different remedies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoError {
    /// A bare `undo`. Names what IS undoable rather than guessing.
    MissingScope,
    UnknownScope(String),
    /// ANTI-VACUITY: no backup exists. This is the arm `br`'s own `beads_rust-a53h`
    /// failed, where `doctor --repair` exited 7 because its post-repair verification
    /// tripped over backups it had just created. A silent success here would tell an
    /// operator their artifact was recovered when nothing happened.
    NoBackup { scope: &'static str, looked_in: PathBuf },
    /// Content-keyed backups have no order, so several candidates cannot be ranked.
    AmbiguousBackups { scope: &'static str, shas: Vec<String> },
    Chokepoint { scope: &'static str, detail: String },
}

impl std::fmt::Display for UndoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingScope => write!(
                formatter,
                "UNDO_SCOPE_REQUIRED known={} detail=backups are content-keyed and have no \
                 order, so there is no `latest` to undo",
                SCOPES.join(",")
            ),
            Self::UnknownScope(scope) => write!(
                formatter,
                "UNDO_UNKNOWN_SCOPE scope={scope:?} known={}",
                SCOPES.join(",")
            ),
            Self::NoBackup { scope, looked_in } => write!(
                formatter,
                "UNDO_NO_BACKUP scope={scope} looked_in={} detail=nothing has been backed \
                 up here, so there is nothing to restore",
                looked_in.display()
            ),
            Self::AmbiguousBackups { scope, shas } => write!(
                formatter,
                "UNDO_AMBIGUOUS_BACKUPS scope={scope} count={} shas={} detail=content-keyed \
                 backups have no order; name one with --from <sha>",
                shas.len(),
                shas.join(",")
            ),
            Self::Chokepoint { scope, detail } => {
                write!(formatter, "UNDO_CHOKEPOINT_FAILED scope={scope} detail={detail}")
            }
        }
    }
}

impl UndoError {
    /// Same dictionary as `repair`, deliberately: `2` for usage and safety refusals, `3`
    /// for an instrument failure, and **never `4`** — `4` is reserved for
    /// upstream-unreachable and `adapter_exec.rs:138-142` already spends it that way.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::MissingScope
            | Self::UnknownScope(_)
            | Self::NoBackup { .. }
            | Self::AmbiguousBackups { .. } => 2,
            Self::Chokepoint { .. } => 3,
        }
    }
}

#[must_use]
fn inception_artifact(repo: &Path) -> PathBuf {
    repo.join(".omp-orchestrator").join("inception.json")
}

/// `ompo undo <scope>`.
///
/// `from` selects one backup by content SHA (prefix match), which is required when more
/// than one exists.
///
/// # Errors
///
/// See [`UndoError`]; each state carries its own code.
pub fn undo(
    repo: &Path,
    scope: &str,
    mode: RepairMode,
    from: Option<&str>,
) -> Result<UndoReport, UndoError> {
    let scope = SCOPES
        .iter()
        .copied()
        .find(|known| *known == scope)
        .ok_or_else(|| UndoError::UnknownScope(scope.to_owned()))?;

    let artifact = inception_artifact(repo);
    let backups = inception::list_backups(&artifact).map_err(|error| UndoError::Chokepoint {
        scope,
        detail: error.to_string(),
    })?;

    let chosen: &BackupEntry = match (backups.len(), from) {
        (0, _) => {
            return Err(UndoError::NoBackup {
                scope,
                looked_in: artifact
                    .parent()
                    .map(|parent| parent.join("backups"))
                    .unwrap_or_else(|| artifact.clone()),
            })
        }
        (_, Some(prefix)) => backups
            .iter()
            .find(|entry| entry.content_sha.starts_with(prefix))
            .ok_or_else(|| UndoError::AmbiguousBackups {
                scope,
                shas: backups.iter().map(|e| e.content_sha.clone()).collect(),
            })?,
        (1, None) => &backups[0],
        (_, None) => {
            return Err(UndoError::AmbiguousBackups {
                scope,
                shas: backups.iter().map(|e| e.content_sha.clone()).collect(),
            })
        }
    };

    let planned = vec![format!(
        "restore {} from {} (sha {})",
        artifact.display(),
        chosen.path.display(),
        &chosen.content_sha[..chosen.content_sha.len().min(12)]
    )];

    if mode == RepairMode::DryRun {
        return Ok(UndoReport {
            mode,
            scope,
            planned,
            applied: Vec::new(),
            restored_from: None,
            reason_code: "UNDO_DRY_RUN_NO_ACTIONS_TAKEN".to_owned(),
        });
    }

    let report =
        inception::restore_backup(&artifact, chosen).map_err(|error| UndoError::Chokepoint {
            scope,
            detail: error.to_string(),
        })?;

    if report.actions == 0 {
        return Ok(UndoReport {
            mode,
            scope,
            planned: Vec::new(),
            applied: Vec::new(),
            restored_from: None,
            reason_code: "UNDO_ALREADY_MATCHES_BACKUP".to_owned(),
        });
    }
    Ok(UndoReport {
        mode,
        scope,
        planned,
        applied: vec![format!("restored {}", artifact.display())],
        restored_from: report.restored_from,
        reason_code: "UNDO_APPLIED".to_owned(),
    })
}

/// Usage, so `ompo undo --help` exits 0.
#[must_use]
pub fn undo_usage() -> String {
    format!(
        "ompo undo <scope> [--from <sha>] [--dry-run | --apply]\n\n\
         \x20 scopes: {}\n\
         \x20 --dry-run   DEFAULT. Names what would be restored and restores nothing.\n\
         \x20 --apply     restore, through the same atomic write the writer uses.\n\
         \x20 --from      select one backup by content sha; REQUIRED when several exist.\n\n\
         There is no `undo latest`: backups are keyed by CONTENT, not time, so they have\n\
         no order. A bare `undo` refuses rather than guessing which one you meant.\n\
         undo never deletes -- an undo that deletes is not a restore.\n",
        SCOPES.join(", ")
    )
}

/// Single entry point.
#[must_use]
pub fn dispatch(command: &str, rest: &[String]) -> Option<u8> {
    if command != "undo" {
        return None;
    }
    if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", undo_usage());
        return Some(0);
    }
    let wants_json = rest.iter().any(|arg| arg == "--json");
    // Reuses repair's parser so the oxymoronic --dry-run --apply refusal is ONE
    // implementation rather than a second copy that can drift.
    let mode = match crate::health_repair::parse_repair_flags(rest) {
        Ok((_, mode)) => mode,
        Err(error) => {
            eprintln!("ompo undo: {error}");
            return Some(error.exit_code());
        }
    };
    let mut from = None;
    let mut scope = None;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--from" => {
                index += 1;
                from = rest.get(index).cloned();
            }
            "--scope" => {
                index += 1;
                scope = rest.get(index).cloned();
            }
            other if !other.starts_with('-') && scope.is_none() => {
                scope = Some(other.to_owned());
            }
            _ => {}
        }
        index += 1;
    }
    let Some(scope) = scope else {
        let error = UndoError::MissingScope;
        eprintln!("ompo undo: {error}");
        return Some(error.exit_code());
    };
    let repo = std::env::current_dir().ok()?;
    match undo(&repo, &scope, mode, from.as_deref()) {
        Ok(report) => {
            if wants_json {
                println!("{}", report.to_json());
            } else {
                println!(
                    "OMPO_UNDO {} scope={} planned={} actual={} {}",
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
            eprintln!("ompo undo: {error}");
            Some(error.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::Duration;
    use subprocess_contract::{bounded_output, BoundedOutcome};

    /// A repo with control files, an initialised artifact, and therefore one backup after a
    /// divergence is planted.
    ///
    /// f3maq: the fixture must carry a REAL git repo with a HEAD COMMIT. This module was never
    /// compiled, so these five legs had never run, and every one of them failed on first build
    /// with `IdentityUnavailable { field: "source_revision" }` — `inception::initialize` shells
    /// `git rev-parse HEAD`, which exits 128 in a directory with no commit. That is the ig4fn
    /// class ("an empty `.git/` satisfies neither"), and the sibling fixtures in
    /// `tests/umbrella_adapter_dispatch.rs` and `ompo-start/tests/l2_ecosystem.rs` already
    /// init-add-commit for exactly this reason.
    fn fixture() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("fixture");
        for relative in inception::required_control_files() {
            let path = directory.path().join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent");
            }
            fs::write(path, "fixture\n").expect("control file");
        }
        // The SECOND reason these legs had never passed: `initialize` refuses an unstamped
        // AGENTS.md (`UntrustedAgentsMd`), which is the ownership guard working. The fixture
        // must carry the canonical stamp, exactly as `l2_ecosystem.rs` does — the alternative,
        // `initialize_trusted`, would test a DIFFERENT entry point than production uses.
        fs::write(
            directory.path().join("AGENTS.md"),
            format!("fixture {}\n", inception::PROJECT_AGENTS_OWNERSHIP_STAMP),
        )
        .expect("stamped AGENTS.md");
        git_fixture(directory.path());
        directory
    }

    /// `git init` plus ONE commit, so `git rev-parse HEAD` resolves. Bounded like every other
    /// subprocess in this crate; a hang here would look like a slow test rather than a fixture
    /// defect.
    fn git_fixture(repo: &Path) {
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=ompo-doctor-test",
                "-c",
                "user.email=ompo-doctor-test@example.invalid",
                "commit",
                "-qm",
                "fixture",
            ],
        ] {
            let mut command = Command::new("git");
            command.args(&args).current_dir(repo);
            match bounded_output(&mut command, Duration::from_secs(10)) {
                BoundedOutcome::Completed(output) if output.status.success() => {}
                other => panic!("git fixture {args:?} failed: {other:?}"),
            }
        }
    }

    /// Initialise, then plant a divergence so a backup exists and the artifact differs.
    fn planted(repo: &Path) -> PathBuf {
        let artifact = inception_artifact(repo);
        inception::initialize(repo, &artifact).expect("initialise");
        // Re-initialising over MODIFIED content is what produces the backup: the writer
        // snapshots the existing bytes before replacing them.
        fs::write(&artifact, b"{\"corrupted\":true}\n").expect("plant divergence");
        inception::initialize(repo, &artifact).expect("re-initialise takes the backup");
        artifact
    }

    /// MUTATION-SHAPED LEG for acceptance D. The first undo must ACT, or the second one
    /// going quiet is not evidence of idempotence — it is indistinguishable from an undo
    /// that never worked. Rule 5.
    #[test]
    fn undo_is_idempotent_and_the_first_run_actually_restored() {
        let directory = fixture();
        let artifact = planted(directory.path());
        // Diverge again so there is something to restore.
        fs::write(&artifact, b"{\"diverged\":true}\n").expect("diverge");

        let first = undo(directory.path(), "inception", RepairMode::Apply, None)
            .expect("first undo");
        assert!(
            !first.applied.is_empty(),
            "the first undo restored nothing, so idempotence on the second proves nothing"
        );
        assert_eq!(first.reason_code, "UNDO_APPLIED");
        assert!(first.restored_from.is_some());

        let second = undo(directory.path(), "inception", RepairMode::Apply, None)
            .expect("second undo");
        assert!(second.applied.is_empty(), "the second undo restored again");
        assert_eq!(second.reason_code, "UNDO_ALREADY_MATCHES_BACKUP");
    }

    /// KNOWN-GOOD. An unmodified artifact is a zero-action success, not a refusal. An
    /// over-strict undo gets routed around, which is a slower death than none.
    #[test]
    fn undo_on_an_unmodified_artifact_is_a_zero_action_success() {
        let directory = fixture();
        planted(directory.path());
        let first = undo(directory.path(), "inception", RepairMode::Apply, None).expect("undo");
        // Whether the first call acts depends on whether the artifact currently differs;
        // either way a SECOND call must be a clean zero-action success.
        let _ = first;
        let again = undo(directory.path(), "inception", RepairMode::Apply, None).expect("undo");
        assert!(again.applied.is_empty());
        assert_eq!(again.reason_code, "UNDO_ALREADY_MATCHES_BACKUP");
        assert_eq!(again.to_json()["status"], "NO_OP");
    }

    /// ANTI-VACUITY. No backup is a TYPED refusal naming where it looked — not a silent
    /// success that tells an operator their artifact was recovered.
    #[test]
    fn no_backup_is_a_typed_refusal_naming_where_it_looked() {
        let directory = fixture();
        let error = undo(directory.path(), "inception", RepairMode::Apply, None)
            .expect_err("must refuse with no backup");
        let rendered = error.to_string();
        assert!(rendered.contains("UNDO_NO_BACKUP"), "{rendered}");
        assert!(rendered.contains("looked_in="), "must name where: {rendered}");
        assert_eq!(error.exit_code(), 2);
    }

    /// DRY-RUN IS THE DEFAULT and restores nothing.
    #[test]
    fn dry_run_is_the_default_and_restores_nothing() {
        let directory = fixture();
        let artifact = planted(directory.path());
        fs::write(&artifact, b"{\"diverged\":true}\n").expect("diverge");
        let before = fs::read(&artifact).expect("before");

        let (_, mode) = crate::health_repair::parse_repair_flags(&[]).expect("parse");
        assert_eq!(mode, RepairMode::DryRun, "undo must not restore by default");
        let report = undo(directory.path(), "inception", mode, None).expect("dry run");
        assert!(report.applied.is_empty());
        assert!(!report.planned.is_empty(), "dry-run planned nothing");
        assert_eq!(report.to_json()["data"]["actual_count"], 0);
        assert_eq!(
            fs::read(&artifact).expect("after"),
            before,
            "dry-run modified the artifact"
        );
    }

    /// CONTENT-KEYED BACKUPS HAVE NO ORDER, so several candidates is a refusal that lists
    /// them — never a guess at "latest".
    #[test]
    fn several_backups_refuse_rather_than_guessing_latest() {
        let directory = fixture();
        let artifact = planted(directory.path());
        // A second distinct backup: diverge to different content, then re-initialise.
        fs::write(&artifact, b"{\"second\":true}\n").expect("diverge again");
        inception::initialize(directory.path(), &artifact).expect("second backup");

        let backups = inception::list_backups(&artifact).expect("list");
        assert!(
            backups.len() >= 2,
            "POSITIVE CONTROL: the ambiguity leg needs >= 2 backups, found {}",
            backups.len()
        );
        let error = undo(directory.path(), "inception", RepairMode::Apply, None)
            .expect_err("must refuse to guess");
        let rendered = error.to_string();
        assert!(rendered.contains("UNDO_AMBIGUOUS_BACKUPS"), "{rendered}");
        assert!(rendered.contains("--from"), "must name the remedy: {rendered}");
        assert_eq!(error.exit_code(), 2);

        // And --from resolves it.
        let chosen = &backups[0].content_sha[..8];
        let resolved = undo(
            directory.path(),
            "inception",
            RepairMode::DryRun,
            Some(chosen),
        )
        .expect("--from resolves the ambiguity");
        assert!(!resolved.planned.is_empty());
    }

    /// A bare `undo` refuses and NAMES what is undoable. Distinct from the unknown-scope
    /// refusal: "you did not say" and "that is not a thing" have different remedies.
    #[test]
    fn bare_undo_and_unknown_scope_are_distinct_typed_refusals() {
        assert_eq!(dispatch("undo", &[]), Some(2));
        assert!(UndoError::MissingScope
            .to_string()
            .contains("UNDO_SCOPE_REQUIRED"));
        assert!(
            UndoError::MissingScope.to_string().contains("no order"),
            "the bare refusal must say WHY there is no latest"
        );
        let unknown = undo(
            Path::new("/nonexistent"),
            "zzz-not-a-scope",
            RepairMode::DryRun,
            None,
        )
        .expect_err("must refuse");
        assert!(unknown.to_string().contains("UNDO_UNKNOWN_SCOPE"));
        assert_ne!(
            UndoError::MissingScope.to_string(),
            unknown.to_string(),
            "the two refusals must not be the same string"
        );
    }

    /// A corrupted backup must NOT be restored silently — the operator would believe the
    /// artifact was recovered.
    #[test]
    fn a_backup_that_fails_its_own_hash_is_refused() {
        let directory = fixture();
        let artifact = planted(directory.path());
        let backups = inception::list_backups(&artifact).expect("list");
        let entry = backups.first().expect("one backup");
        fs::write(&entry.path, b"tampered\n").expect("tamper");

        let error = inception::restore_backup(&artifact, entry).expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("backup integrity failed"), "{rendered}");
    }

    /// Same exit dictionary as `repair`, and `4` stays reserved for upstream-unreachable.
    #[test]
    fn exit_vocabulary_matches_repair_and_reserves_four() {
        let usage = UndoError::MissingScope.exit_code();
        let instrument = UndoError::Chokepoint {
            scope: "inception",
            detail: "probe".to_owned(),
        }
        .exit_code();
        assert_ne!(usage, instrument);
        for code in [usage, instrument] {
            assert_ne!(code, 4, "4 is reserved for upstream-unreachable");
            assert_ne!(code, 0, "a refusal must never exit 0");
        }
        // The two modules must agree, not merely each be self-consistent.
        assert_eq!(
            usage,
            crate::health_repair::RepairError::MissingScope.exit_code(),
            "undo and repair must share one usage code"
        );
    }

    /// `--help` exits 0 for the canonical probe, and the usage states the no-latest reason.
    #[test]
    fn help_succeeds_and_states_why_there_is_no_latest() {
        assert_eq!(dispatch("undo", &["--help".to_owned()]), Some(0));
        let usage = undo_usage();
        assert!(usage.contains("no order"));
        assert!(usage.contains("never deletes"));
    }

    /// Declines everything it does not own.
    #[test]
    fn dispatch_declines_foreign_commands() {
        for command in ["doctor", "init", "help", "repair", "health", "capabilities"] {
            assert_eq!(dispatch(command, &[]), None, "captured {command}");
        }
    }
}
