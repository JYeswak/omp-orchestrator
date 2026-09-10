//! `ompo upstream-report <adapter>` — the aggregator-mandatory surface from
//! `/canonical-cli-scoping`: a CLI that wraps upstream tools must be able to generate an
//! upstream-issue draft rather than leaving the operator to reconstruct one.
//!
//! Bead: `omp-orchestrator-jplf.7.2`. This is the OPERATOR SURFACE for the `UPSTREAM_BUG`
//! class already present in the adapter taxonomy; the evidence it files is exactly
//! [`crate::adapter_exec::AdapterVerdict`], so the report cannot describe a run that did not
//! happen.
//!
//! # Why a verdict class decides reportability
//!
//! Measured by `ompo doctor --adapter all` on the author's host (88 executed, 37 live,
//! 1 degraded, 50 unmeasurable): most non-`LIVE` rows are **local install gaps, not upstream
//! defects.** Fifty adapters are simply absent from `PATH`. Filing those upstream would be
//! fifty false reports, so [`classify`] refuses them with a typed reason instead — the same
//! `UNMEASURABLE`-is-not-a-failure distinction the executor already enforces on exit codes.
//!
//! # The four reportable conditions, each measured rather than imagined
//!
//! ```text
//! NoHelpContract      exit 0 with NO output          specimen: loop-queue-filter
//! UsageWithNonZero    prints usage, exits nonzero    11 specimens: fleet-monitor 2,
//!                                                    inbox-monitor 64, pane-dispatch-fence 78
//! ForeignResolution   the name resolves outside our  specimen: installer -> /usr/sbin/installer
//!                     install roots
//! Hangs               exceeded the probe deadline    0 specimens today; kept because a
//!                                                    timeout is a restrictive terminal
//! ```

use crate::adapter_exec::{AdapterStatus, AdapterVerdict};
use crate::provenance::BuildProvenance;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Where drafts land under `--apply`.
pub const DRAFT_DIR: &str = ".planning/upstream-issues";

/// What, if anything, is worth filing upstream about one adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reportable {
    /// Exited zero and said nothing: the documented surface is missing.
    NoHelpContract,
    /// Answered its documented surface and still exited nonzero.
    UsageWithNonZero { exit: i32 },
    /// The roster name resolves to a binary outside our install roots.
    ForeignResolution { resolved: PathBuf },
    /// Exceeded the probe deadline.
    Hangs,
}

impl Reportable {
    #[must_use]
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::NoHelpContract => "UPSTREAM_NO_HELP_CONTRACT",
            Self::UsageWithNonZero { .. } => "UPSTREAM_USAGE_WITH_NONZERO_EXIT",
            Self::ForeignResolution { .. } => "UPSTREAM_FOREIGN_RESOLUTION",
            Self::Hangs => "UPSTREAM_HANGS_ON_HELP",
        }
    }

    /// The one-line title of the draft.
    #[must_use]
    pub fn title(&self, adapter: &str) -> String {
        match self {
            Self::NoHelpContract => {
                format!("{adapter}: `--help` exits 0 and prints nothing")
            }
            Self::UsageWithNonZero { exit } => {
                format!("{adapter}: `--help` prints usage but exits {exit}")
            }
            Self::ForeignResolution { resolved } => format!(
                "{adapter}: the installed name resolves to {}, not to our build",
                resolved.display()
            ),
            Self::Hangs => format!("{adapter}: `--help` exceeded the probe deadline"),
        }
    }

    /// What a maintainer should change. A report that does not say what "fixed" looks like is
    /// a complaint rather than an issue.
    #[must_use]
    pub fn expected(&self) -> &'static str {
        match self {
            Self::NoHelpContract => {
                "`--help` writes a usage line to stdout or stderr. An agent cannot discover a \
                 surface that answers with nothing, and exit 0 asserts that it answered."
            }
            Self::UsageWithNonZero { .. } => {
                "`--help` exits 0. An agent branching on the exit code reads a usage request \
                 as a failure, so the two cannot be distinguished by a caller."
            }
            Self::ForeignResolution { .. } => {
                "the binary is installed under an owned install root, or the target is renamed \
                 so it does not collide with a host binary of the same name."
            }
            Self::Hangs => {
                "`--help` returns without blocking. A help path that waits on input, a lock or \
                 the network cannot be probed by any aggregator."
            }
        }
    }
}

/// Refusals — every one typed, because "nothing to report" and "could not tell" have different
/// remedies and a silent empty draft reports identically to both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotReportable {
    /// The adapter behaves correctly. Refusing here is the anti-vacuity arm: an aggregator that
    /// emits a draft for a healthy adapter trains its operator to ignore drafts.
    Healthy,
    /// Absent from `PATH`. A LOCAL install gap, not an upstream defect — 50 of 88 today.
    NotInstalled,
}

impl NotReportable {
    #[must_use]
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Healthy => "UPSTREAM_REPORT_NOTHING_TO_REPORT",
            Self::NotInstalled => "UPSTREAM_REPORT_NOT_AN_UPSTREAM_DEFECT",
        }
    }

    #[must_use]
    pub fn detail(&self) -> &'static str {
        match self {
            Self::Healthy => {
                "the adapter spawned, answered its documented surface and exited 0; there is \
                 nothing to file"
            }
            Self::NotInstalled => {
                "the adapter is absent from PATH, which is a local install gap rather than a \
                 defect in the adapter; install it and re-probe"
            }
        }
    }
}

/// Decide what a verdict is worth filing. Keyed on the verdict class and the recorded exit,
/// never on a fresh run: the report describes the run that produced it.
#[must_use]
pub fn classify(verdict: &AdapterVerdict) -> Result<Reportable, NotReportable> {
    match verdict.status {
        AdapterStatus::TimedOut => Ok(Reportable::Hangs),
        AdapterStatus::NoHelpContract => Ok(Reportable::NoHelpContract),
        AdapterStatus::NotInstalled => Err(NotReportable::NotInstalled),
        // The executor already keyed this arm on [`is_ours`], so the collision outranks the
        // exit code here by CONSTRUCTION rather than by a second check: the exit code on a
        // foreign row belongs to a binary that is not ours at all. A `Foreign` row without a
        // resolution is unreachable — the executor only reaches that arm from a `Some(path)`
        // — and the sentinel keeps the impossible case VISIBLE rather than silently dropping
        // the row from the draft.
        AdapterStatus::Foreign => Ok(Reportable::ForeignResolution {
            resolved: verdict
                .resolved
                .clone()
                .unwrap_or_else(|| PathBuf::from("unresolved")),
        }),
        AdapterStatus::Live => match verdict.exit {
            Some(0) | None => Err(NotReportable::Healthy),
            Some(exit) => Ok(Reportable::UsageWithNonZero { exit }),
        },
    }
}


/// A stable content id for the draft filename.
///
/// FNV-1a over the draft body rather than a hash crate: the crate has no hash dependency, and
/// adding one to name a file would be a dependency edge bought for a filename. The property
/// required is DETERMINISM — re-running `--apply` on unchanged evidence must not churn a new
/// file — not cryptographic strength, and this is documented so nobody mistakes it for one.
#[must_use]
pub fn content_id(body: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in body.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Render the draft. Every field a maintainer needs to reproduce, and nothing asserted that
/// the probe did not observe.
#[must_use]
pub fn draft(verdict: &AdapterVerdict, reportable: &Reportable) -> String {
    let provenance = BuildProvenance::current();
    let resolved = verdict
        .resolved
        .as_ref()
        .map_or_else(|| "unresolved".to_owned(), |p| p.display().to_string());
    let exit = verdict
        .exit
        .map_or_else(|| "none (never exited)".to_owned(), |c| c.to_string());
    format!(
        "# {title}\n\
         \n\
         **Reason code:** `{reason}`\n\
         **Adapter:** `{adapter}`\n\
         \n\
         ## Observed\n\
         \n\
         ```\n\
         argv     {argv}\n\
         resolved {resolved}\n\
         exit     {exit}\n\
         output   {detail}\n\
         verdict  {verdict_code}\n\
         ```\n\
         \n\
         ## Expected\n\
         \n\
         {expected}\n\
         \n\
         ## Reproduction\n\
         \n\
         ```\n\
         {argv}\n\
         echo $?\n\
         ```\n\
         \n\
         Read the exit code WITHOUT a pipe: `$?` after a pipeline reports the pipeline's status.\n\
         \n\
         ## Reporter environment\n\
         \n\
         ```\n\
         ompo package_version {version}\n\
         ompo build_commit    {commit}\n\
         ompo source_revision {revision}\n\
         probe deadline       {deadline}s, process group killed on expiry\n\
         ```\n\
         \n\
         ## NO-CLAIM\n\
         \n\
         This draft reports ONE probe of `--help` on ONE host. It does not establish that the \
         adapter is unhealthy, that the behaviour reproduces elsewhere, or that any other \
         subcommand is affected. The exit code is RECORDED, not treated as the verdict: across \
         this roster `--help` exits 0, 1, 2, 64, 78 and 255, and eleven adapters print usage \
         while exiting nonzero, so the code alone discriminates nothing.\n",
        title = reportable.title(&verdict.adapter),
        reason = reportable.reason_code(),
        adapter = verdict.adapter,
        argv = verdict.argv().join(" "),
        resolved = resolved,
        exit = exit,
        detail = if verdict.detail.is_empty() {
            "(none)"
        } else {
            &verdict.detail
        },
        verdict_code = verdict.status.reason_code(),
        expected = reportable.expected(),
        version = provenance.package_version,
        commit = provenance.build_commit,
        revision = provenance.source_revision,
        deadline = crate::adapter_exec::PROBE_DEADLINE.as_secs(),
    )
}

/// The `--json` envelope for one report decision.
#[must_use]
pub fn envelope(verdict: &AdapterVerdict, decision: &Result<Reportable, NotReportable>) -> Value {
    let data = match decision {
        Ok(reportable) => {
            let body = draft(verdict, reportable);
            json!({
                "adapter": verdict.adapter,
                "reportable": true,
                "reason_code": reportable.reason_code(),
                "title": reportable.title(&verdict.adapter),
                "draft_path": draft_path(&verdict.adapter, &body).display().to_string(),
                "content_id": content_id(&body),
                "verdict": verdict.to_json(),
            })
        }
        Err(not) => json!({
            "adapter": verdict.adapter,
            "reportable": false,
            "reason_code": not.reason_code(),
            "detail": not.detail(),
            "verdict": verdict.to_json(),
        }),
    };
    let status = if decision.is_ok() { "DEGRADED" } else { "OK" };
    crate::umbrella::envelope("upstream-report", status, data)
}

/// The deterministic draft path for an adapter and body.
#[must_use]
pub fn draft_path(adapter: &str, body: &str) -> PathBuf {
    Path::new(DRAFT_DIR).join(format!("{adapter}-{}.md", content_id(body)))
}

/// What `--apply` did. `Unchanged` is a first-class outcome, not a silent success: an
/// operator re-running the probe needs to know the draft was already on disk rather than
/// wondering whether the write happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// The draft was created.
    Written(PathBuf),
    /// A byte-identical draft was already present. Idempotent by content, not by timestamp.
    Unchanged(PathBuf),
}

impl Applied {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Written(path) | Self::Unchanged(path) => path,
        }
    }

    #[must_use]
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Written(_) => "UPSTREAM_REPORT_WRITTEN",
            Self::Unchanged(_) => "UPSTREAM_REPORT_UNCHANGED",
        }
    }
}

/// Write the draft under `repo`, and REFUSE when there is nothing to report.
///
/// `--apply` is the gated opt-in; printing is the default, so this is never reached by an
/// operator who only asked to look. The refusal is the anti-vacuity arm and it is typed: **an
/// empty draft on disk is worse than no draft**, because a directory of empty files trains an
/// operator to stop reading the directory.
///
/// The write is followed by a READBACK compare. A successful `fs::write` is the call
/// succeeding, not the content persisting — the distinction this repo already records for
/// commits, applied to a file.
pub fn apply(
    verdict: &AdapterVerdict,
    decision: &Result<Reportable, NotReportable>,
    repo: &Path,
) -> Result<Applied, String> {
    let reportable = match decision {
        Ok(reportable) => reportable,
        Err(not) => {
            return Err(format!(
                "{} adapter={:?} detail={}",
                not.reason_code(),
                verdict.adapter,
                not.detail()
            ))
        }
    };
    let body = draft(verdict, reportable);
    let path = repo.join(draft_path(&verdict.adapter, &body));
    if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing == body {
            return Ok(Applied::Unchanged(path));
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("UPSTREAM_REPORT_BAD_PATH path={}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "UPSTREAM_REPORT_MKDIR_FAILED path={} error={error}",
            parent.display()
        )
    })?;
    std::fs::write(&path, &body).map_err(|error| {
        format!(
            "UPSTREAM_REPORT_WRITE_FAILED path={} error={error}",
            path.display()
        )
    })?;
    let readback = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "UPSTREAM_REPORT_READBACK_FAILED path={} error={error}",
            path.display()
        )
    })?;
    if readback != body {
        return Err(format!(
            "UPSTREAM_REPORT_READBACK_MISMATCH path={} wrote={} read={}",
            path.display(),
            body.len(),
            readback.len()
        ));
    }
    Ok(Applied::Written(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(
        adapter: &str,
        status: AdapterStatus,
        resolved: Option<&str>,
        exit: Option<i32>,
        detail: &str,
    ) -> AdapterVerdict {
        AdapterVerdict {
            adapter: adapter.to_owned(),
            status,
            resolved: resolved.map(PathBuf::from),
            exit,
            detail: detail.to_owned(),
        }
    }

    #[test]
    fn a_healthy_adapter_is_refused_with_a_typed_reason_not_an_empty_draft() {
        let good = verdict(
            "tick-monitor",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/tick-monitor"),
            Some(0),
            "usage: tick-monitor [observe|watch]",
        );
        let error = classify(&good).expect_err("a healthy adapter must be refused");
        assert_eq!(error, NotReportable::Healthy);
        assert_eq!(error.reason_code(), "UPSTREAM_REPORT_NOTHING_TO_REPORT");
    }

    #[test]
    fn an_absent_adapter_is_a_local_install_gap_and_never_an_upstream_report() {
        // 50 of 88 adapters were in this state when the taxonomy was measured. Filing them
        // upstream would be 50 false reports.
        let absent = verdict(
            "gate-runner",
            AdapterStatus::NotInstalled,
            None,
            None,
            "no such file",
        );
        let error = classify(&absent).expect_err("absent must be refused");
        assert_eq!(error, NotReportable::NotInstalled);
        assert_ne!(
            error.reason_code(),
            NotReportable::Healthy.reason_code(),
            "'nothing to report' and 'not an upstream defect' must not share a code"
        );
    }

    #[test]
    fn the_measured_no_help_contract_specimen_is_reportable() {
        // loop-queue-filter --help: exit 0, no output. The one degraded row in 88.
        let specimen = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            Some("/home/operator/.local/bin/loop-queue-filter"),
            Some(0),
            "no output on stdout or stderr",
        );
        let report = classify(&specimen).expect("must be reportable");
        assert_eq!(report, Reportable::NoHelpContract);
        assert!(report
            .title("loop-queue-filter")
            .contains("exits 0 and prints nothing"));
    }

    #[test]
    fn usage_with_a_nonzero_exit_is_reportable_and_carries_the_code_it_saw() {
        // pane-dispatch-fence --help: exit 78 with a usage line. One of eleven.
        let specimen = verdict(
            "pane-dispatch-fence",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/pane-dispatch-fence"),
            Some(78),
            "pane-dispatch-fence: unknown argument: --help",
        );
        let report = classify(&specimen).expect("must be reportable");
        assert_eq!(report, Reportable::UsageWithNonZero { exit: 78 });
        assert!(report.title("pane-dispatch-fence").contains("exits 78"));
    }

    #[test]
    fn the_measured_foreign_resolution_outranks_the_exit_code() {
        // installer resolves to macOS /usr/sbin/installer and exits 255 with a usage line.
        // BOTH conditions hold; the collision is the one worth filing, because the exit code
        // belongs to a binary that is not ours at all. The executor now types that row
        // `Foreign`, so this leg asserts the DRAFT follows the typed status rather than
        // re-deriving foreignness from the path a second time.
        let foreign = verdict(
            "installer",
            AdapterStatus::Foreign,
            Some("/usr/sbin/installer"),
            Some(255),
            "Usage: installer [-help] [-dominfo]",
        );
        let report = classify(&foreign).expect("must be reportable");
        assert_eq!(
            report,
            Reportable::ForeignResolution {
                resolved: PathBuf::from("/usr/sbin/installer")
            },
            "a foreign resolution must not be reported as our adapter's exit code"
        );
    }

    #[test]
    fn a_timeout_is_reportable_and_never_silently_healthy() {
        let hung = verdict(
            "slow-adapter",
            AdapterStatus::TimedOut,
            None,
            None,
            "exceeded 5s deadline",
        );
        assert_eq!(classify(&hung).expect("reportable"), Reportable::Hangs);
    }

    #[test]
    fn every_reason_code_is_distinct_across_both_outcomes() {
        let codes = [
            Reportable::NoHelpContract.reason_code(),
            Reportable::UsageWithNonZero { exit: 1 }.reason_code(),
            Reportable::ForeignResolution {
                resolved: PathBuf::from("/x"),
            }
            .reason_code(),
            Reportable::Hangs.reason_code(),
            NotReportable::Healthy.reason_code(),
            NotReportable::NotInstalled.reason_code(),
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two outcomes share one code: {codes:?}"
        );
    }

    #[test]
    fn the_draft_carries_argv_resolution_exit_output_and_verdict_class() {
        let specimen = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            Some("/home/operator/.local/bin/loop-queue-filter"),
            Some(0),
            "no output on stdout or stderr",
        );
        let report = classify(&specimen).expect("reportable");
        let body = draft(&specimen, &report);
        for needle in [
            "loop-queue-filter --help",
            "/home/operator/.local/bin/loop-queue-filter",
            "UAD_ADAPTER_NO_HELP_CONTRACT",
            "UPSTREAM_NO_HELP_CONTRACT",
            "## Expected",
            "## Reproduction",
            "## NO-CLAIM",
        ] {
            assert!(body.contains(needle), "draft is missing {needle:?}");
        }
    }

    #[test]
    fn the_draft_carries_build_provenance_so_a_maintainer_knows_which_ompo_probed() {
        let specimen = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            None,
            Some(0),
            "x",
        );
        let report = classify(&specimen).expect("reportable");
        let body = draft(&specimen, &report);
        let provenance = BuildProvenance::current();
        assert!(body.contains(provenance.package_version));
        assert!(body.contains(provenance.build_commit));
        assert!(body.contains(provenance.source_revision));
    }

    #[test]
    fn apply_refuses_when_there_is_nothing_to_report_and_writes_no_file() {
        // ANTI-VACUITY: an empty draft on disk is worse than none. A directory of empty files
        // trains an operator to stop reading the directory.
        let dir = tempfile::tempdir().expect("fixture dir");
        let healthy = verdict(
            "tick-monitor",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/x"),
            Some(0),
            "usage",
        );
        let decision = classify(&healthy);
        let error = apply(&healthy, &decision, dir.path()).expect_err("must refuse");
        assert!(
            error.contains("UPSTREAM_REPORT_NOTHING_TO_REPORT"),
            "got {error}"
        );
        assert!(
            error.contains("tick-monitor"),
            "the refusal must name the adapter; got {error}"
        );
        assert!(
            !dir.path().join(DRAFT_DIR).exists(),
            "a refused apply must not even create the directory"
        );
    }

    #[test]
    fn apply_refuses_an_absent_adapter_with_the_local_gap_code_not_the_healthy_one() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let absent = verdict(
            "gate-runner",
            AdapterStatus::NotInstalled,
            None,
            None,
            "no such file",
        );
        let decision = classify(&absent);
        let error = apply(&absent, &decision, dir.path()).expect_err("must refuse");
        assert!(
            error.contains("UPSTREAM_REPORT_NOT_AN_UPSTREAM_DEFECT"),
            "got {error}"
        );
        assert!(
            !error.contains("UPSTREAM_REPORT_NOTHING_TO_REPORT"),
            "got {error}"
        );
    }

    #[test]
    fn apply_writes_the_draft_reads_it_back_and_is_idempotent_by_content() {
        let dir = tempfile::tempdir().expect("fixture dir");
        let broken = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            Some("/home/operator/.local/bin/loop-queue-filter"),
            Some(0),
            "no output on stdout or stderr",
        );
        let decision = classify(&broken);

        let first = apply(&broken, &decision, dir.path()).expect("first apply");
        let path = match &first {
            Applied::Written(path) => path.clone(),
            Applied::Unchanged(path) => {
                panic!("a fresh dir cannot be unchanged: {}", path.display())
            }
        };
        assert_eq!(first.reason_code(), "UPSTREAM_REPORT_WRITTEN");
        let on_disk = std::fs::read_to_string(&path).expect("draft on disk");
        assert!(
            on_disk.contains("UPSTREAM_NO_HELP_CONTRACT"),
            "the file must carry the reason code"
        );
        assert!(
            on_disk.contains("loop-queue-filter --help"),
            "the file must carry the argv"
        );

        // Re-running must NOT churn a second file: the path is content-keyed.
        let second = apply(&broken, &decision, dir.path()).expect("second apply");
        assert_eq!(second, Applied::Unchanged(path.clone()));
        assert_eq!(second.reason_code(), "UPSTREAM_REPORT_UNCHANGED");
        let count = std::fs::read_dir(dir.path().join(DRAFT_DIR))
            .expect("draft dir")
            .count();
        assert_eq!(count, 1, "a re-run must not create a second draft");
    }

    #[test]
    fn a_different_observation_writes_a_different_draft_rather_than_overwriting() {
        // The content id is the filename, so new evidence must not silently replace old
        // evidence about the same adapter.
        let dir = tempfile::tempdir().expect("fixture dir");
        let first_run = verdict(
            "fleet-monitor",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/fleet-monitor"),
            Some(2),
            "usage: fleet-monitor",
        );
        let later_run = verdict(
            "fleet-monitor",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/fleet-monitor"),
            Some(64),
            "usage: fleet-monitor",
        );
        apply(&first_run, &classify(&first_run), dir.path()).expect("first");
        apply(&later_run, &classify(&later_run), dir.path()).expect("second");
        let count = std::fs::read_dir(dir.path().join(DRAFT_DIR))
            .expect("dir")
            .count();
        assert_eq!(count, 2, "two distinct observations must be two drafts");
    }

    #[test]
    fn the_content_id_is_deterministic_and_discriminates() {
        let a = content_id("one body");
        assert_eq!(
            a,
            content_id("one body"),
            "same body must yield the same id"
        );
        assert_ne!(
            a,
            content_id("another body"),
            "different bodies must differ"
        );
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn the_draft_path_is_stable_for_unchanged_evidence() {
        let specimen = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            None,
            Some(0),
            "x",
        );
        let report = classify(&specimen).expect("reportable");
        let body = draft(&specimen, &report);
        let first = draft_path("loop-queue-filter", &body);
        assert_eq!(first, draft_path("loop-queue-filter", &body));
        assert!(first.starts_with(DRAFT_DIR));
        assert!(first.to_string_lossy().ends_with(".md"));
    }

    #[test]
    fn the_envelope_states_reportability_in_both_directions() {
        let healthy = verdict(
            "tick-monitor",
            AdapterStatus::Live,
            Some("/home/operator/.local/bin/x"),
            Some(0),
            "usage",
        );
        let value = envelope(&healthy, &classify(&healthy));
        assert_eq!(value["command"], "upstream-report");
        assert_eq!(value["status"], "OK");
        assert_eq!(value["data"]["reportable"], false);

        let broken = verdict(
            "loop-queue-filter",
            AdapterStatus::NoHelpContract,
            None,
            Some(0),
            "x",
        );
        let value = envelope(&broken, &classify(&broken));
        assert_eq!(value["status"], "DEGRADED");
        assert_eq!(value["data"]["reportable"], true);
        assert!(value["data"]["draft_path"]
            .as_str()
            .expect("path")
            .starts_with(DRAFT_DIR));
    }
}
