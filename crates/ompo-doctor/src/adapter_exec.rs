//! Per-adapter EXECUTION for the `ompo` umbrella — the `UAD-ADDRESS` half that
//! `docs/contracts/umbrella_adapter_dispatch.md:94-95` names as "named and not built".
//!
//! Bead: `omp-orchestrator-jplf.7.2`. Prescribed by `docs/plan/07-installability.md:128`
//! (`doctor [<adapter>]` — "diagnose every subsystem, or one adapter") and `:219`.
//!
//! THIS MODULE OWNS EXECUTION ONLY. Addressability lives in [`crate::umbrella`] and system
//! probe semantics live in [`crate::run_doctor`]; per `fh C47` one identity-bearing contract
//! gets one canonical definition, and the identity here is "what happened when we ran it".
//!
//! # Why the exit code is NOT the verdict
//!
//! Measured across the 38 roster adapters that are on `PATH` (2026-09-07): `--help` exits
//! `0`, `1`, `2`, `64`, `78` and `255`, and **eleven adapters print a usage line while
//! exiting nonzero** (`fleet-monitor` 2, `inbox-monitor` 64, `pane-dispatch-fence` 78).
//! Keying the verdict on the exit code would report eleven live, self-documenting binaries
//! as failures — an over-strict verb, which gets routed around.
//!
//! The converse is also measured and is why output-presence is the key rather than a
//! fallback: `loop-queue-filter --help` exits **0 with no output at all**. So the exit code
//! is wrong in BOTH directions and is recorded rather than believed.
//!
//! # Why the resolved path is recorded
//!
//! An adapter name is a workspace bin target; `PATH` may resolve it to something else
//! entirely. Measured: roster name `installer` resolves to macOS `/usr/sbin/installer`
//! ("Usage: installer [-help] [-dominfo]"), and `07-installability.md:556` already records
//! `pane-truth` as "the foreign pane-truth binary". A verdict that does not say WHAT it ran
//! cannot be checked, which is `%8`'s provenance finding on the adapter axis.

use crate::umbrella;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

/// Bounded deadline for one adapter probe. A timeout is a restrictive terminal: it maps to
/// [`AdapterStatus::TimedOut`] and NEVER to a pass.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// The argv this executor probes an adapter with. `--help` is chosen because it is the one
/// argument the CLI discipline requires of every adapter and the only one with no side
/// effects; a per-adapter doctor argv is a later bead and would live in a declared table.
pub const PROBE_ARGS: &[&str] = &["--help"];

/// Truncation bound for captured child output, matching the system probe loop.
const DETAIL_CHARS: usize = 240;

/// What executing one adapter established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStatus {
    /// Spawned and produced output on stdout or stderr. The adapter is installed and
    /// answers its documented surface. Says NOTHING about health.
    Live,
    /// Spawned and produced NO output. Specimen: `loop-queue-filter --help` exits 0 silently.
    NoHelpContract,
    /// Could not be spawned — absent from `PATH`. UNMEASURABLE, not a failure: the same
    /// class as the seven `UNMEASURABLE` crates in the 88-crate roster run.
    NotInstalled,
    /// Exceeded [`PROBE_DEADLINE`]. UNMEASURABLE and never a pass.
    TimedOut,
}

impl AdapterStatus {
    /// The typed reason code. Every arm is distinct, so no two causes share one token.
    #[must_use]
    pub fn reason_code(self) -> &'static str {
        match self {
            Self::Live => "UAD_ADAPTER_LIVE",
            Self::NoHelpContract => "UAD_ADAPTER_NO_HELP_CONTRACT",
            Self::NotInstalled => "UAD_ADAPTER_NOT_INSTALLED",
            Self::TimedOut => "UAD_ADAPTER_TIMED_OUT",
        }
    }

    /// The envelope status from `docs/plan/07-installability.md:119`.
    #[must_use]
    pub fn envelope_status(self) -> &'static str {
        match self {
            Self::Live => "OK",
            Self::NoHelpContract => "DEGRADED",
            Self::NotInstalled | Self::TimedOut => "UNKNOWN",
        }
    }

    /// True when this arm establishes nothing about the adapter.
    #[must_use]
    pub fn is_unmeasurable(self) -> bool {
        matches!(self, Self::NotInstalled | Self::TimedOut)
    }
}

/// One adapter's execution record. `resolved` and `exit` are recorded even where they do not
/// decide the verdict, because a reader cannot audit a verdict whose inputs are hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterVerdict {
    pub adapter: String,
    pub status: AdapterStatus,
    /// The `PATH` resolution actually executed, or `None` when nothing was spawned.
    pub resolved: Option<PathBuf>,
    /// The child's exit code, `None` when it never exited (unspawned or killed).
    pub exit: Option<i32>,
    /// The child's first non-empty output line, truncated. This field exists because a
    /// `CHECK_FAIL` row carrying only crate/bin/exit/argv is uncausable by construction —
    /// measured on 11 of 12 failing rows in the 88-crate roster run.
    pub detail: String,
}

impl AdapterVerdict {
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec![self.adapter.clone()];
        argv.extend(PROBE_ARGS.iter().map(|arg| (*arg).to_owned()));
        argv
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "adapter": self.adapter,
            "status": self.status.envelope_status(),
            "reason_code": self.status.reason_code(),
            "resolved": self.resolved.as_ref().map(|path| path.display().to_string()),
            "exit": self.exit,
            "detail": self.detail,
            "argv": self.argv(),
        })
    }
}

/// Exit codes. `2` belongs to the caller's argv parser, not here.
pub const EXIT_ALL_LIVE: u8 = 0;
pub const EXIT_DEGRADED: u8 = 1;
/// Distinct from [`EXIT_DEGRADED`] on purpose: "we could not measure it" and "it failed" have
/// different remedies, and one colour for both sends the reader to the wrong repair.
///
/// It is `4` rather than `3` because `main.rs` already spends `3` on `EXIT_INSTRUMENT` ("the
/// umbrella itself could not answer"). An unreachable adapter and a broken instrument are two
/// causes, and collapsing them into one code is precisely the defect a message-only assertion
/// stays green through.
pub const EXIT_UNMEASURABLE: u8 = 4;

fn first_output_line(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .chars()
        .take(DETAIL_CHARS)
        .collect()
}

fn resolve_on_path(adapter: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(adapter))
        .find(|candidate| candidate.is_file())
}

/// `LAW-UAD-UNKNOWN-IS-TWO`: an adapter absent from the roster is refused by MESSAGE, since
/// the exit code cannot distinguish it from an unknown verb. Mirrors [`umbrella::help_for`],
/// which is the arm the `doctor --adapter` flag currently omits.
pub fn require_adapter(adapter: &str) -> Result<(), String> {
    let roster = umbrella::roster_or_error()?;
    if !roster.contains(&adapter) {
        return Err(format!(
            "UAD_UNKNOWN_ADAPTER adapter={adapter:?} reason=absent_from_roster \
             roster_size={} hint=`ompo capabilities --json` enumerates every adapter",
            roster.len()
        ));
    }
    Ok(())
}

/// Execute one roster adapter under a bounded deadline and return what happened.
pub fn execute(adapter: &str) -> Result<AdapterVerdict, String> {
    require_adapter(adapter)?;
    Ok(execute_unchecked(adapter))
}

/// The spawn half, without the roster check, so the roster and execution properties can be
/// asserted independently rather than through one another.
#[must_use]
pub fn execute_unchecked(adapter: &str) -> AdapterVerdict {
    let resolved = resolve_on_path(adapter);
    let mut command = Command::new(adapter);
    command.args(PROBE_ARGS);
    let (status, exit, detail) = match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            let detail = first_output_line(&output);
            let code = output.status.code();
            if detail.is_empty() {
                (AdapterStatus::NoHelpContract, code, "no output on stdout or stderr".to_owned())
            } else {
                (AdapterStatus::Live, code, detail)
            }
        }
        BoundedOutcome::TimedOut => (
            AdapterStatus::TimedOut,
            None,
            format!("exceeded {}s deadline; process group killed", PROBE_DEADLINE.as_secs()),
        ),
        BoundedOutcome::Unspawned(error) => (AdapterStatus::NotInstalled, None, error.to_string()),
    };
    AdapterVerdict {
        adapter: adapter.to_owned(),
        status,
        resolved,
        exit,
        detail,
    }
}

/// Execute the whole roster. The roster is never empty by construction and
/// [`umbrella::roster_or_error`] is the runtime guard for the same property.
pub fn execute_all() -> Result<Vec<AdapterVerdict>, String> {
    let roster = umbrella::roster_or_error()?;
    Ok(roster.iter().map(|name| execute_unchecked(name)).collect())
}

/// The exit code for a verdict set. `Err` on an empty set: a verb that executed nothing must
/// not report identically to one that executed everything and found nothing wrong.
pub fn exit_code(verdicts: &[AdapterVerdict]) -> Result<u8, String> {
    if verdicts.is_empty() {
        return Err("UAD_EXECUTE_EMPTY_SET reason=zero_verdicts \
                    detail=an empty execution set is an ERROR, never a pass"
            .to_owned());
    }
    if verdicts.iter().any(|v| v.status == AdapterStatus::NoHelpContract) {
        return Ok(EXIT_DEGRADED);
    }
    if verdicts.iter().any(|v| v.status.is_unmeasurable()) {
        return Ok(EXIT_UNMEASURABLE);
    }
    Ok(EXIT_ALL_LIVE)
}

/// Adapters that ANSWERED and still exited nonzero.
///
/// RENAMED from `usage_with_nonzero_exit` on `%8`'s finding, and the rename IS the fix: the
/// predicate was right and the NAME overclaimed. `staged-build-gate` exits `1` with
/// `STAGED_BUILD_GATE_REFUSED … reason=STAGED_…`, and `pre-commit-gate` exits `3` with
/// `NOTHING_TO_CHECK: no staged files to check` — healthy tools reporting a STATUS through an
/// exit code, which is not a usage failure and was being counted as one.
///
/// What the field measures is now exactly what it says: **a live adapter whose `--help`
/// exited nonzero.** The defect that makes it worth counting is unchanged and covers the
/// status-reporting rows too — **an agent branching on `$?` cannot distinguish a usage
/// request from a failure from a status.** The ambiguity is the defect; the intent behind it
/// is not measurable from the wire, which is why the name no longer claims to know it.
#[must_use]
pub fn nonzero_exit_on_help(verdicts: &[AdapterVerdict]) -> usize {
    verdicts
        .iter()
        .filter(|v| v.status == AdapterStatus::Live && !matches!(v.exit, Some(0) | None))
        .count()
}

/// Of the nonzero-exit rows, those whose captured line CARRIES A USAGE MARKER.
///
/// **A FLOOR, not a partition.** Its two measured misclassifications are named in
/// `the_usage_marker_is_a_floor_with_named_misclassifications`: `omp-idle-dispatch` prints a
/// bare synopsis with no "usage" word and `pane-dispatch-fence` prints
/// `unknown argument: --help`, so both are usage complaints this marker misses and the true
/// usage-shaped count is **>= this value**. A per-adapter declared list would make it exact
/// and is precisely the hand-maintained second inventory `UAD-NO-SECOND-COUNT` forbids, so
/// the floor ships with its bound stated rather than being made exact by hand.
#[must_use]
pub fn nonzero_exit_with_usage_marker(verdicts: &[AdapterVerdict]) -> usize {
    verdicts
        .iter()
        .filter(|v| {
            v.status == AdapterStatus::Live
                && !matches!(v.exit, Some(0) | None)
                && usage_marker(&v.detail)
        })
        .count()
}

/// True when a captured line reads as a usage/help line. Lowercased substring, so `Usage:`,
/// `usage error:` and `inbox-monitor: usage:` all match.
#[must_use]
pub fn usage_marker(detail: &str) -> bool {
    detail.to_ascii_lowercase().contains("usage")
}

/// Every distinct exit code observed, with how many adapters produced it, ascending.
///
/// Keyed on `Option<i32>` rendered as a string so "never exited" is a VISIBLE bucket rather
/// than being folded into some sentinel integer.
#[must_use]
pub fn exit_histogram(verdicts: &[AdapterVerdict]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for verdict in verdicts {
        let key = verdict
            .exit
            .map_or_else(|| "none".to_owned(), |code| format!("{code:03}"));
        *counts.entry(key).or_default() += 1;
    }
    counts.into_iter().collect()
}

/// The `--json` envelope for one or many adapters, carrying its own denominator so a reader
/// can tell "executed nothing" from "executed everything".
pub fn envelope(verdicts: &[AdapterVerdict]) -> Result<Value, String> {
    let exit = exit_code(verdicts)?;
    let roster = umbrella::roster_or_error()?;
    let live = verdicts.iter().filter(|v| v.status == AdapterStatus::Live).count();
    let degraded = verdicts
        .iter()
        .filter(|v| v.status == AdapterStatus::NoHelpContract)
        .count();
    let unmeasurable = verdicts.iter().filter(|v| v.status.is_unmeasurable()).count();
    let status = if degraded > 0 {
        "DEGRADED"
    } else if unmeasurable > 0 {
        "UNKNOWN"
    } else {
        "OK"
    };
    Ok(umbrella::envelope(
        "doctor",
        status,
        json!({
            "axis": "adapter",
            "executed": verdicts.len(),
            "roster_size": roster.len(),
            "live": live,
            "degraded": degraded,
            "unmeasurable": unmeasurable,
            "nonzero_exit_on_help": nonzero_exit_on_help(verdicts),
            "nonzero_exit_with_usage_marker": nonzero_exit_with_usage_marker(verdicts),
            "exit_histogram": exit_histogram(verdicts)
                .into_iter()
                .map(|(code, count)| json!({"exit": code, "adapters": count}))
                .collect::<Vec<_>>(),
            "exit": exit,
            "probe_argv": PROBE_ARGS,
            "deadline_secs": PROBE_DEADLINE.as_secs(),
            "adapters": verdicts.iter().map(AdapterVerdict::to_json).collect::<Vec<_>>(),
        }),
    ))
}

/// One human-readable line per verdict.
#[must_use]
pub fn render(verdict: &AdapterVerdict) -> String {
    let resolved = verdict
        .resolved
        .as_ref()
        .map_or_else(|| "unresolved".to_owned(), |p| p.display().to_string());
    let exit = verdict
        .exit
        .map_or_else(|| "none".to_owned(), |code| code.to_string());
    format!(
        "{} adapter={} resolved={} exit={} detail={}",
        verdict.status.reason_code(),
        verdict.adapter,
        resolved,
        exit,
        verdict.detail
    )
}

/// The whole-roster selector. `ompo doctor all` and `ompo doctor --adapter all`.
pub const AXIS_ALL: &str = "all";

/// Invocation error — mirrors `main.rs`'s `EXIT_BAD_INVOCATION`. The two live in different
/// crates (a binary's consts are not importable), so `exit_vocabulary_is_pairwise_distinct`
/// is the guard against them drifting into a shared value.
pub const EXIT_BAD_INVOCATION: u8 = 2;
/// Instrument error — mirrors `main.rs`'s `EXIT_INSTRUMENT`.
pub const EXIT_INSTRUMENT: u8 = 3;

/// True when `token` addresses the adapter axis: the roster selector, or a roster member.
///
/// This is what lets a bare `ompo doctor <adapter>` — the spelling
/// `docs/plan/07-installability.md:128` prescribes — route to the same executor as
/// `--adapter` WITHOUT swallowing a typo: a token that is neither keeps the caller's
/// `unknown argument` refusal.
#[must_use]
pub fn is_axis_selector(token: &str) -> bool {
    token == AXIS_ALL || umbrella::is_adapter(token)
}

/// Execute one adapter or the whole roster, print the report, and return the exit code.
///
/// Lives here rather than in `main.rs` so the shared binary file carries only argv parsing:
/// `main.rs` is `%7`'s verb-wiring seam and this is the executor's own logic.
pub fn run_axis(named: &str, json: bool) -> u8 {
    let verdicts = if named == AXIS_ALL {
        match execute_all() {
            Ok(verdicts) => verdicts,
            Err(error) => {
                eprintln!("ompo doctor: {error}");
                return EXIT_INSTRUMENT;
            }
        }
    } else {
        match execute(named) {
            Ok(verdict) => vec![verdict],
            Err(error) => {
                eprintln!("ompo doctor: {error}");
                return EXIT_BAD_INVOCATION;
            }
        }
    };
    let code = match exit_code(&verdicts) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("ompo doctor: {error}");
            return EXIT_INSTRUMENT;
        }
    };
    if json {
        match envelope(&verdicts).and_then(|value| {
            serde_json::to_string(&value).map_err(|error| format!("cannot encode report: {error}"))
        }) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("ompo doctor: {error}");
                return EXIT_INSTRUMENT;
            }
        }
    } else {
        for verdict in &verdicts {
            println!("{}", render(verdict));
        }
        println!(
            "OMPO_DOCTOR_ADAPTERS executed={} live={} degraded={} unmeasurable={} \
             nonzero_exit_on_help={} usage_marker_floor={} exit={code}",
            verdicts.len(),
            verdicts.iter().filter(|v| v.status == AdapterStatus::Live).count(),
            verdicts
                .iter()
                .filter(|v| v.status == AdapterStatus::NoHelpContract)
                .count(),
            verdicts.iter().filter(|v| v.status.is_unmeasurable()).count(),
            nonzero_exit_on_help(&verdicts),
            nonzero_exit_with_usage_marker(&verdicts),
        );
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_adapter_absent_from_the_roster_is_refused_by_message_not_by_code() {
        let error = require_adapter("zzz-not-an-adapter").expect_err("must refuse");
        assert!(
            error.contains("UAD_UNKNOWN_ADAPTER"),
            "reason code missing: {error}"
        );
        assert!(
            error.contains("absent_from_roster"),
            "the discriminating reason is missing: {error}"
        );
        assert!(
            error.contains("zzz-not-an-adapter"),
            "the refusal must NAME the rejected string: {error}"
        );
    }

    #[test]
    fn a_real_roster_adapter_passes_the_roster_check() {
        let roster = umbrella::roster_or_error().expect("roster");
        let first = roster.first().expect("non-empty roster");
        require_adapter(first).expect("a roster member must be accepted");
    }

    #[test]
    fn an_absent_binary_is_unmeasurable_and_never_live() {
        let verdict = execute_unchecked("zzz-no-such-binary-on-path");
        assert_eq!(verdict.status, AdapterStatus::NotInstalled);
        assert!(verdict.status.is_unmeasurable());
        assert_eq!(verdict.resolved, None);
        assert_eq!(verdict.exit, None);
        assert_eq!(verdict.status.envelope_status(), "UNKNOWN");
        assert!(!verdict.detail.is_empty(), "the spawn error must be carried");
    }

    #[test]
    fn every_status_has_a_distinct_reason_code_and_the_two_unmeasurable_arms_differ() {
        let codes = [
            AdapterStatus::Live.reason_code(),
            AdapterStatus::NoHelpContract.reason_code(),
            AdapterStatus::NotInstalled.reason_code(),
            AdapterStatus::TimedOut.reason_code(),
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len(), "two causes share one token: {codes:?}");
        assert_ne!(
            AdapterStatus::NotInstalled.reason_code(),
            AdapterStatus::TimedOut.reason_code()
        );
    }

    #[test]
    fn exit_vocabulary_is_pairwise_distinct() {
        // The guard named in the EXIT_BAD_INVOCATION doc comment: these four values are
        // mirrored from `main.rs`, which a library cannot import from, so drift into a
        // shared value is caught here rather than by a reader noticing.
        let codes = [
            EXIT_ALL_LIVE,
            EXIT_DEGRADED,
            EXIT_BAD_INVOCATION,
            EXIT_INSTRUMENT,
            EXIT_UNMEASURABLE,
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two causes share one exit code: {codes:?}"
        );
    }

    #[test]
    fn the_axis_selector_accepts_the_roster_and_all_and_refuses_a_typo() {
        assert!(is_axis_selector(AXIS_ALL));
        let roster = umbrella::roster_or_error().expect("roster");
        assert!(is_axis_selector(roster.first().expect("non-empty")));
        assert!(
            !is_axis_selector("zzz-not-an-adapter"),
            "a typo must stay an unknown argument rather than becoming an adapter"
        );
    }

    #[test]
    fn an_empty_execution_set_is_an_error_not_a_pass() {
        let error = exit_code(&[]).expect_err("empty set must refuse");
        assert!(error.contains("UAD_EXECUTE_EMPTY_SET"), "got {error}");
        let envelope_error = envelope(&[]).expect_err("empty envelope must refuse");
        assert!(envelope_error.contains("UAD_EXECUTE_EMPTY_SET"), "got {envelope_error}");
    }

    fn verdict(adapter: &str, status: AdapterStatus) -> AdapterVerdict {
        AdapterVerdict {
            adapter: adapter.to_owned(),
            status,
            resolved: None,
            exit: None,
            detail: "fixture".to_owned(),
        }
    }

    #[test]
    fn unmeasurable_does_not_report_as_a_pass_and_does_not_report_as_a_failure() {
        let unknown = [verdict("a", AdapterStatus::NotInstalled)];
        assert_eq!(exit_code(&unknown).expect("code"), EXIT_UNMEASURABLE);
        assert_ne!(exit_code(&unknown).expect("code"), EXIT_ALL_LIVE);
        assert_ne!(exit_code(&unknown).expect("code"), EXIT_DEGRADED);
    }

    #[test]
    fn a_degraded_adapter_outranks_an_unmeasurable_one() {
        let mixed = [
            verdict("a", AdapterStatus::NotInstalled),
            verdict("b", AdapterStatus::NoHelpContract),
            verdict("c", AdapterStatus::Live),
        ];
        assert_eq!(exit_code(&mixed).expect("code"), EXIT_DEGRADED);
    }

    #[test]
    fn all_live_is_the_only_zero() {
        let live = [verdict("a", AdapterStatus::Live), verdict("b", AdapterStatus::Live)];
        assert_eq!(exit_code(&live).expect("code"), EXIT_ALL_LIVE);
    }

    #[test]
    fn the_nonzero_exit_count_is_derived_not_hand_measured() {
        // The figure this replaces was a shell census: "eleven adapters print usage while
        // exiting nonzero". The executor recorded `exit` on every row from the start, so the
        // count was always derivable and simply never derived.
        let rows = [
            AdapterVerdict { adapter: "a".into(), status: AdapterStatus::Live, resolved: None, exit: Some(0), detail: "usage".into() },
            AdapterVerdict { adapter: "b".into(), status: AdapterStatus::Live, resolved: None, exit: Some(78), detail: "usage".into() },
            AdapterVerdict { adapter: "c".into(), status: AdapterStatus::Live, resolved: None, exit: Some(64), detail: "usage".into() },
            // NOT counted: it never answered, so it is not "answered AND exited nonzero".
            AdapterVerdict { adapter: "d".into(), status: AdapterStatus::NoHelpContract, resolved: None, exit: Some(0), detail: "none".into() },
            // NOT counted: absent binaries never exited at all.
            AdapterVerdict { adapter: "e".into(), status: AdapterStatus::NotInstalled, resolved: None, exit: None, detail: "absent".into() },
        ];
        assert_eq!(nonzero_exit_on_help(&rows), 2);
        assert_eq!(nonzero_exit_on_help(&[]), 0);
    }

    fn live(adapter: &str, exit: i32, detail: &str) -> AdapterVerdict {
        AdapterVerdict {
            adapter: adapter.to_owned(),
            status: AdapterStatus::Live,
            resolved: None,
            exit: Some(exit),
            detail: detail.to_owned(),
        }
    }

    #[test]
    fn a_status_reported_through_an_exit_code_is_still_counted_and_no_longer_called_usage() {
        // %8's finding. `pre-commit-gate` exits 3 with NOTHING_TO_CHECK and `staged-build-gate`
        // exits 1 with STAGED_BUILD_GATE_REFUSED -- healthy tools reporting a STATUS. They are
        // counted, because an agent branching on `$?` still cannot tell them from a failure,
        // and the field name no longer claims they are usage failures.
        let rows = [
            live("pre-commit-gate", 3, "NOTHING_TO_CHECK: no staged files to check"),
            live("staged-build-gate", 1, "STAGED_BUILD_GATE_REFUSED crate=x reason=STAGED_WORKTREE"),
        ];
        assert_eq!(nonzero_exit_on_help(&rows), 2, "the ambiguity is the defect, so both count");
        assert_eq!(
            nonzero_exit_with_usage_marker(&rows),
            0,
            "neither carries a usage marker, so neither inflates the usage-shaped floor"
        );
    }

    #[test]
    fn the_usage_marker_is_a_floor_with_named_misclassifications() {
        // MEASURED on the live roster. The marker catches nine of twelve; these two are usage
        // complaints it misses, which is why the field is documented as a FLOOR and not as a
        // partition. Making it exact needs a per-adapter declared list -- the second inventory
        // UAD-NO-SECOND-COUNT forbids -- so the bound is published instead.
        let missed = [
            live("omp-idle-dispatch", 2, "omp-idle-dispatch [status|why|capabilities|run]"),
            live("pane-dispatch-fence", 78, "pane-dispatch-fence: unknown argument: --help"),
        ];
        assert_eq!(nonzero_exit_on_help(&missed), 2);
        assert_eq!(
            nonzero_exit_with_usage_marker(&missed),
            0,
            "these are the named misses; if this becomes 2 the marker improved and this leg must be re-read"
        );
        let caught = [
            live("fleet-monitor", 2, "usage: fleet-monitor [--all|--self]"),
            live("installer", 255, "Usage: installer [-help]"),
            live("fleet-composite", 2, "usage error: unknown command --help"),
            live("inbox-monitor", 64, "inbox-monitor: usage:"),
        ];
        assert_eq!(nonzero_exit_with_usage_marker(&caught), 4, "all four spellings must match");
    }

    #[test]
    fn the_marker_floor_never_exceeds_the_total_it_is_drawn_from() {
        let rows = [
            live("a", 2, "usage: a"),
            live("b", 3, "NOTHING_TO_CHECK"),
            AdapterVerdict { adapter: "c".into(), status: AdapterStatus::Live, resolved: None, exit: Some(0), detail: "usage: c".into() },
        ];
        let total = nonzero_exit_on_help(&rows);
        let floor = nonzero_exit_with_usage_marker(&rows);
        assert!(floor <= total, "a floor above its own total is incoherent: {floor} > {total}");
        assert_eq!(total, 2);
        assert_eq!(floor, 1, "the exit-0 row carries a usage marker and must NOT be counted");
    }

    #[test]
    fn the_exit_histogram_keeps_never_exited_as_a_visible_bucket() {
        let rows = [
            AdapterVerdict { adapter: "a".into(), status: AdapterStatus::Live, resolved: None, exit: Some(0), detail: "x".into() },
            AdapterVerdict { adapter: "b".into(), status: AdapterStatus::Live, resolved: None, exit: Some(0), detail: "x".into() },
            AdapterVerdict { adapter: "c".into(), status: AdapterStatus::NotInstalled, resolved: None, exit: None, detail: "x".into() },
        ];
        let histogram = exit_histogram(&rows);
        assert_eq!(histogram, vec![("000".to_owned(), 2), ("none".to_owned(), 1)]);
        assert!(
            histogram.iter().any(|(code, _)| code == "none"),
            "never-exited must not be folded into a sentinel integer"
        );
    }

    #[test]
    fn the_envelope_carries_its_own_denominator() {
        let verdicts = [verdict("a", AdapterStatus::Live)];
        let value = envelope(&verdicts).expect("envelope");
        assert_eq!(value["command"], "doctor");
        assert_eq!(value["data"]["axis"], "adapter");
        assert_eq!(value["data"]["executed"], 1);
        let roster = umbrella::roster_or_error().expect("roster").len();
        assert_eq!(value["data"]["roster_size"], roster);
        assert_eq!(value["schema_version"], umbrella::SCHEMA_VERSION);
    }

    #[test]
    fn the_rendered_line_names_the_adapter_the_code_and_the_detail() {
        let line = render(&verdict("pane-truth", AdapterStatus::NotInstalled));
        assert!(line.contains("UAD_ADAPTER_NOT_INSTALLED"), "got {line}");
        assert!(line.contains("adapter=pane-truth"), "got {line}");
        assert!(line.contains("detail=fixture"), "got {line}");
        assert!(line.contains("resolved=unresolved"), "got {line}");
    }
}
