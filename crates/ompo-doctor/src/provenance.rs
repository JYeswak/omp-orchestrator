#![forbid(unsafe_code)]

//! Build provenance and compile-time registry freshness.
//!
//! The adapter roster remains compile-time generated for fast, self-contained capabilities. This
//! module supplies the missing freshness boundary: `ompo doctor` can compare that baked roster with
//! Cargo's current bin-target oracle when a workspace is available. A missing Cargo oracle is
//! `UNMEASURED`, never a stale or current verdict.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

const UNKNOWN: &str = "unknown";
const METADATA_DEADLINE: Duration = Duration::from_secs(30);

/// Build-time identity for an ompo artifact. The builder may leave the revision fields UNKNOWN
/// when the source checkout has no readable VCS metadata; UNKNOWN is explicit, never fabricated.
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_COMMIT: &str = match option_env!("OMPO_BUILD_COMMIT") {
    Some(value) => value,
    None => UNKNOWN,
};
pub const SOURCE_REVISION: &str = match option_env!("OMPO_SOURCE_REVISION") {
    Some(value) => value,
    None => UNKNOWN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BuildProvenance {
    pub package_version: &'static str,
    pub build_commit: &'static str,
    pub source_revision: &'static str,
}

impl BuildProvenance {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            package_version: PACKAGE_VERSION,
            build_commit: BUILD_COMMIT,
            source_revision: SOURCE_REVISION,
        }
    }

    #[must_use]
    pub fn status(self) -> &'static str {
        if self.build_commit != UNKNOWN && self.source_revision != UNKNOWN {
            "KNOWN"
        } else {
            "UNKNOWN"
        }
    }
}

/// Typed result of comparing the baked adapter ids with live Cargo bin targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegistryProbe {
    pub status: &'static str,
    pub reason_code: String,
    pub detail: String,
    pub provenance: BuildProvenance,
    pub provenance_status: &'static str,
    pub baked_adapter_count: usize,
    pub live_bin_target_count: Option<usize>,
    pub phantom: Vec<String>,
    pub unregistered: Vec<String>,
}

/// Exit code for an installed artifact whose verb set exactly matches this source, from a
/// build that can name the revision it is speaking for.
pub const PARITY_EXIT_CURRENT: u8 = 0;
/// Exit code for an installed artifact whose verb set differs from this source.
pub const PARITY_EXIT_STALE: u8 = 1;
/// Exit code when this build cannot anchor a staleness verdict to a revision.
///
/// `3` is INSTRUMENT ERROR on the ladder `umbrella::usage()` documents: `0` success, `1`
/// degraded, `2` usage or safety refusal, `3` instrument error, `4` upstream unreachable.
///
/// NOT `0`: an UNKNOWN that exits `0` is the same false green in a new coat, and `$?` is the
/// only axis a shell-level caller branches on.
/// NOT `4`: the installed artifact answered, parsed, and was compared -- nothing upstream was
/// unreachable. What could not answer is THIS binary, about its own origin, which is the
/// instrument. `adapter_exec.rs:200-207` keeps `3` and `4` apart for exactly this reason.
/// NOT `1`: degraded asserts the subject is wrong. UNKNOWN asserts we do not know, and
/// sending a reader to reinstall something we never measured is the wrong remedy.
pub const PARITY_EXIT_UNKNOWN: u8 = 3;
/// Exit code when the installed artifact cannot be measured safely.
pub const PARITY_EXIT_UNMEASURED: u8 = 4;
const PARITY_DEADLINE: Duration = Duration::from_secs(10);

/// Reason codes live as consts rather than literals at their use sites so the test that pins
/// one and the code that emits it cannot drift apart.
pub const PARITY_REASON_CURRENT: &str = "OMPO_VERB_PARITY_CURRENT";
pub const PARITY_REASON_STALE: &str = "OMPO_VERB_PARITY_STALE";
/// The reason code NAMES the missing input. One flat `UNSTAMPED` code would tell a reader a
/// stamp is missing without telling them which one to supply.
pub const PARITY_REASON_UNSTAMPED_BUILD_COMMIT: &str = "OMPO_VERB_PARITY_UNSTAMPED_BUILD_COMMIT";
pub const PARITY_REASON_UNSTAMPED_SOURCE_REVISION: &str =
    "OMPO_VERB_PARITY_UNSTAMPED_SOURCE_REVISION";
pub const PARITY_REASON_UNSTAMPED_BOTH: &str =
    "OMPO_VERB_PARITY_UNSTAMPED_BUILD_COMMIT_AND_SOURCE_REVISION";
pub const PARITY_REASON_PROVENANCE_DIVERGED: &str = "OMPO_VERB_PARITY_PROVENANCE_DIVERGED";

/// Axes `parity` does NOT measure, published in its own payload.
///
/// MEASURED 2026-09-10: the installed artifact advertised 87 adapters against a source roster
/// of 88 while this probe answered CURRENT. A verb set is blind to content, and a reader who
/// is not told which axes went unmeasured stops looking. Teaching `parity` to compare rosters
/// is a separate change and is deliberately NOT done here: the fix is the verdict, not the
/// comparison.
pub const PARITY_UNMEASURED_AXES: &[&str] = &["adapter_roster", "artifact_content"];

/// Typed comparison of one installed ompo artifact's advertised verbs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledVerbParityProbe {
    /// The VERDICT: what this run is entitled to claim. Never `CURRENT` from a build that
    /// cannot name the revision it is speaking for.
    pub status: &'static str,
    pub exit_code: u8,
    pub reason_code: String,
    pub message: String,
    pub detail: String,
    pub provenance: BuildProvenance,
    pub installed_path: String,
    pub installed_verbs: Vec<String>,
    pub missing: Vec<String>,
    pub unexpected: Vec<String>,
    /// The OBSERVATION, carried separately from the verdict: what the verb-set comparison
    /// alone found. An UNKNOWN verdict still publishes this, so downgrading a verdict never
    /// discards the evidence underneath it.
    pub verb_set_status: &'static str,
    /// Provenance inputs this build could not supply. Empty on a stamped build.
    pub missing_provenance: Vec<&'static str>,
    /// Always [`PARITY_UNMEASURED_AXES`]. Present in the payload so the blindness is in-band
    /// rather than something the reader has to already know.
    pub unmeasured_axes: &'static [&'static str],
}

/// Pure parity result used by the subprocess probe and its unit tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbParityComparison {
    pub status: &'static str,
    pub missing: Vec<String>,
    pub unexpected: Vec<String>,
}

/// Compare installed verbs with the canonical source umbrella verb set.
#[must_use]
pub fn compare_verb_parity(installed: &[String]) -> VerbParityComparison {
    let expected: BTreeSet<&str> = crate::umbrella::VERBS.iter().copied().collect();
    let installed_set: BTreeSet<&str> = installed.iter().map(String::as_str).collect();
    let missing = expected
        .iter()
        .filter(|verb| !installed_set.contains(**verb))
        .map(|verb| (*verb).to_owned())
        .collect::<Vec<_>>();
    let unexpected = installed_set
        .iter()
        .filter(|verb| !expected.contains(**verb))
        .map(|verb| (*verb).to_owned())
        .collect::<Vec<_>>();
    let status = if missing.is_empty() && unexpected.is_empty() {
        "CURRENT"
    } else {
        "STALE"
    };
    VerbParityComparison {
        status,
        missing,
        unexpected,
    }
}

/// What one parity run may CLAIM, given what it could SEE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityVerdict {
    pub status: &'static str,
    pub exit_code: u8,
    pub reason_code: &'static str,
    pub message: &'static str,
    pub missing_provenance: Vec<&'static str>,
}

impl ParityVerdict {
    /// The installed artifact itself could not be read: upstream-unreachable, never a verdict
    /// about staleness.
    fn unmeasured(reason_code: &'static str) -> Self {
        Self {
            status: "UNMEASURED",
            exit_code: PARITY_EXIT_UNMEASURED,
            reason_code,
            message: "installed ompo could not be measured",
            missing_provenance: Vec::new(),
        }
    }
}

/// Provenance inputs this build cannot supply, in a stable order.
#[must_use]
pub fn missing_provenance(provenance: BuildProvenance) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if provenance.build_commit == UNKNOWN {
        missing.push("build_commit");
    }
    if provenance.source_revision == UNKNOWN {
        missing.push("source_revision");
    }
    missing
}

/// Decide what a verb-set comparison entitles this build to CLAIM.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-ompo-parity-blind-current-72abf`). The previous
/// code mapped `comparison.status` straight onto the verdict, so a build reporting
/// `source_revision=unknown build_commit=unknown` answered `CURRENT` with `rc=0` on stdout --
/// a green from an instrument that cannot name the revision it is speaking for. Measured the
/// same minute, that same artifact was one adapter behind source. An oracle that cannot
/// anchor its verdict must not render one: it is worse than no oracle, because a green trains
/// the reader to stop looking.
///
/// The blind gate is UNCONDITIONAL on purpose, not only over the `CURRENT` leg. A blind build
/// that also finds a verb difference returns UNKNOWN and publishes `verb_set_status=STALE`
/// beside it, so the finding survives while the verdict declines to over-claim.
///
/// `ompo health` already refuses this same input with `PROVENANCE_UNSTAMPED`, detail *"this
/// build cannot state its own origin; staleness is UNMEASURED"*. The wording below is that
/// one, deliberately: two oracles reading the same unstamped binary now speak one vocabulary.
#[must_use]
pub fn parity_verdict(
    provenance: BuildProvenance,
    comparison: &VerbParityComparison,
) -> ParityVerdict {
    let missing = missing_provenance(provenance);
    if !missing.is_empty() {
        let reason_code = match missing.as_slice() {
            ["build_commit"] => PARITY_REASON_UNSTAMPED_BUILD_COMMIT,
            ["source_revision"] => PARITY_REASON_UNSTAMPED_SOURCE_REVISION,
            _ => PARITY_REASON_UNSTAMPED_BOTH,
        };
        return ParityVerdict {
            status: "UNKNOWN",
            exit_code: PARITY_EXIT_UNKNOWN,
            reason_code,
            message: "this build cannot state its own origin; the verb set was compared and \
                      staleness is UNMEASURED",
            missing_provenance: missing,
        };
    }
    if provenance.build_commit != provenance.source_revision {
        return ParityVerdict {
            status: "UNKNOWN",
            exit_code: PARITY_EXIT_UNKNOWN,
            reason_code: PARITY_REASON_PROVENANCE_DIVERGED,
            message: "this build carries two different revisions, so \"current source\" names \
                      no single revision; staleness is UNMEASURED",
            missing_provenance: Vec::new(),
        };
    }
    if comparison.status == "CURRENT" {
        ParityVerdict {
            status: "CURRENT",
            exit_code: PARITY_EXIT_CURRENT,
            reason_code: PARITY_REASON_CURRENT,
            message: "installed verb set matches current source",
            missing_provenance: Vec::new(),
        }
    } else {
        ParityVerdict {
            status: "STALE",
            exit_code: PARITY_EXIT_STALE,
            reason_code: PARITY_REASON_STALE,
            message: "installed verb set differs from current source",
            missing_provenance: Vec::new(),
        }
    }
}

/// Parse the exact capabilities envelope emitted by an installed ompo binary.
pub fn parse_installed_capabilities_verbs(raw: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("capabilities response is invalid JSON: {error}"))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(crate::umbrella::SCHEMA_VERSION)
    {
        return Err("capabilities response has an unsupported schema_version".to_owned());
    }
    if value.get("command").and_then(Value::as_str) != Some("capabilities") {
        return Err("capabilities response is not the capabilities command".to_owned());
    }
    if value.get("status").and_then(Value::as_str) != Some("OK") {
        return Err("capabilities response did not report status=OK".to_owned());
    }
    let verbs = value
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("verbs"))
        .and_then(Value::as_array)
        .ok_or_else(|| "capabilities response has no data.verbs array".to_owned())?;
    if verbs.is_empty() {
        return Err("capabilities response has an empty data.verbs array".to_owned());
    }
    let mut parsed = Vec::with_capacity(verbs.len());
    let mut seen = BTreeSet::new();
    for verb in verbs {
        let verb = verb
            .as_str()
            .filter(|verb| !verb.is_empty())
            .ok_or_else(|| {
                "capabilities response data.verbs contains a non-string or empty verb".to_owned()
            })?;
        if !seen.insert(verb) {
            return Err(format!("capabilities response data.verbs repeats {verb:?}"));
        }
        parsed.push(verb.to_owned());
    }
    Ok(parsed)
}

fn parity_result(
    installed_path: String,
    verdict: ParityVerdict,
    detail: String,
    installed_verbs: Vec<String>,
    comparison: Option<VerbParityComparison>,
) -> InstalledVerbParityProbe {
    let (verb_set_status, missing, unexpected) = match comparison {
        Some(comparison) => (comparison.status, comparison.missing, comparison.unexpected),
        None => ("UNMEASURED", Vec::new(), Vec::new()),
    };
    InstalledVerbParityProbe {
        status: verdict.status,
        exit_code: verdict.exit_code,
        reason_code: verdict.reason_code.to_owned(),
        message: verdict.message.to_owned(),
        detail,
        provenance: BuildProvenance::current(),
        installed_path,
        installed_verbs,
        missing,
        unexpected,
        verb_set_status,
        missing_provenance: verdict.missing_provenance,
        unmeasured_axes: PARITY_UNMEASURED_AXES,
    }
}

/// Execute an installed ompo path and compare its capabilities verbs with this source.
#[must_use]
pub fn probe_installed_verb_parity(installed: &Path) -> InstalledVerbParityProbe {
    let installed_path = installed.display().to_string();
    let mut command = Command::new(installed);
    command.args(["capabilities", "--json"]);
    let output = match bounded_output(&mut command, PARITY_DEADLINE) {
        BoundedOutcome::TimedOut => {
            return parity_result(
                installed_path,
                ParityVerdict::unmeasured("OMPO_VERB_PARITY_TIMEOUT"),
                format!("capabilities probe exceeded {}s", PARITY_DEADLINE.as_secs()),
                Vec::new(),
                None,
            )
        }
        BoundedOutcome::Unspawned(error) => {
            return parity_result(
                installed_path,
                ParityVerdict::unmeasured("OMPO_VERB_PARITY_UNRUNNABLE"),
                format!("capabilities probe could not be spawned: {error}"),
                Vec::new(),
                None,
            )
        }
        BoundedOutcome::Completed(output) => output,
    };
    if !output.status.success() {
        return parity_result(
            installed_path,
            ParityVerdict::unmeasured("OMPO_VERB_PARITY_CHILD_FAILED"),
            format!(
                "capabilities probe exited {} detail={}",
                output.status,
                first_line(&output.stderr)
            ),
            Vec::new(),
            None,
        );
    }
    let raw = match std::str::from_utf8(&output.stdout) {
        Ok(raw) => raw,
        Err(error) => {
            return parity_result(
                installed_path,
                ParityVerdict::unmeasured("OMPO_VERB_PARITY_INVALID_UTF8"),
                format!("capabilities response is not UTF-8: {error}"),
                Vec::new(),
                None,
            )
        }
    };
    let installed_verbs = match parse_installed_capabilities_verbs(raw) {
        Ok(verbs) => verbs,
        Err(detail) => {
            return parity_result(
                installed_path,
                ParityVerdict::unmeasured("OMPO_VERB_PARITY_MALFORMED"),
                detail,
                Vec::new(),
                None,
            )
        }
    };
    let comparison = compare_verb_parity(&installed_verbs);
    let provenance = BuildProvenance::current();
    let verdict = parity_verdict(provenance, &comparison);
    let detail = format!(
        "source_revision={} build_commit={} installed_path={} verb_set_status={} \
         missing={:?} unexpected={:?} missing_provenance={:?} unmeasured_axes={:?}",
        provenance.source_revision,
        provenance.build_commit,
        installed.display(),
        comparison.status,
        comparison.missing,
        comparison.unexpected,
        verdict.missing_provenance,
        PARITY_UNMEASURED_AXES,
    );
    parity_result(
        installed_path,
        verdict,
        detail,
        installed_verbs,
        Some(comparison),
    )
}

/// Compare adapter ids in both directions, preserving each offending id.
#[must_use]
pub fn registry_parity(
    registered: &[&str],
    live_bin_targets: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let registered: BTreeSet<&str> = registered.iter().copied().collect();
    let phantom = registered
        .iter()
        .filter(|name| {
            !live_bin_targets
                .iter()
                .any(|target| target.as_str() == **name)
        })
        .map(|name| (*name).to_owned())
        .collect();
    let unregistered = live_bin_targets
        .iter()
        .filter(|name| !registered.contains(name.as_str()))
        .cloned()
        .collect();
    (phantom, unregistered)
}

/// Probe the current workspace without changing the baked capabilities roster.
///
/// `CURRENT` means the live bin-target set exactly matches the baked registry. `STALE` names the
/// symmetric difference. `UNMEASURED` means Cargo or its JSON oracle could not answer.
#[must_use]
pub fn probe_registry(repo: &Path, registered: &[&str]) -> RegistryProbe {
    let provenance = BuildProvenance::current();
    let base = |status: &'static str,
                reason_code: &str,
                detail: String,
                live_bin_target_count: Option<usize>,
                phantom: Vec<String>,
                unregistered: Vec<String>| RegistryProbe {
        status,
        reason_code: reason_code.to_owned(),
        detail,
        provenance,
        provenance_status: provenance.status(),
        baked_adapter_count: registered.len(),
        live_bin_target_count,
        phantom,
        unregistered,
    };

    let mut command = Command::new("cargo");
    command
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(repo);
    let output = match bounded_output(&mut command, METADATA_DEADLINE) {
        BoundedOutcome::TimedOut => {
            return base(
                "UNMEASURED",
                "OMPO_REGISTRY_METADATA_TIMEOUT",
                format!("cargo metadata exceeded {}s", METADATA_DEADLINE.as_secs()),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        BoundedOutcome::Unspawned(error) => {
            return base(
                "UNMEASURED",
                "OMPO_REGISTRY_METADATA_UNSPAWNED",
                format!("cargo metadata could not be spawned: {error}"),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        BoundedOutcome::Completed(output) => output,
    };
    if !output.status.success() {
        return base(
            "UNMEASURED",
            "OMPO_REGISTRY_METADATA_FAILED",
            format!(
                "cargo metadata exit={} detail={}",
                output.status,
                first_line(&output.stderr)
            ),
            None,
            Vec::new(),
            Vec::new(),
        );
    }

    let live_bin_targets = match parse_bin_targets(&output.stdout) {
        Ok(targets) if !targets.is_empty() => targets,
        Ok(_) => {
            return base(
                "UNMEASURED",
                "OMPO_REGISTRY_EMPTY_ORACLE",
                "cargo metadata returned zero bin targets".to_owned(),
                Some(0),
                Vec::new(),
                Vec::new(),
            )
        }
        Err(detail) => {
            return base(
                "UNMEASURED",
                "OMPO_REGISTRY_METADATA_INVALID",
                detail,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
    };
    let (phantom, unregistered) = registry_parity(registered, &live_bin_targets);
    if phantom.is_empty() && unregistered.is_empty() {
        base(
            "CURRENT",
            "OMPO_REGISTRY_CURRENT",
            "baked adapter ids match live Cargo bin targets".to_owned(),
            Some(live_bin_targets.len()),
            phantom,
            unregistered,
        )
    } else {
        base(
            "STALE",
            "OMPO_REGISTRY_STALE",
            format!(
                "registry differs from live Cargo targets: phantom={} unregistered={}",
                phantom.len(),
                unregistered.len()
            ),
            Some(live_bin_targets.len()),
            phantom,
            unregistered,
        )
    }
}

fn parse_bin_targets(text: &[u8]) -> Result<BTreeSet<String>, String> {
    let value: Value = serde_json::from_slice(text)
        .map_err(|error| format!("cargo metadata JSON is invalid: {error}"))?;
    let packages = value
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata JSON has no packages array".to_owned())?;
    let mut targets = BTreeSet::new();
    for package in packages {
        let package_name = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "cargo metadata package has no name".to_owned())?;
        let package_targets = package
            .get("targets")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("cargo metadata package {package_name} has no targets array"))?;
        for target in package_targets {
            let kinds = target
                .get("kind")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!("cargo metadata target in {package_name} has no kind array")
                })?;
            if kinds.iter().any(|kind| kind.as_str() == Some("bin")) {
                let name = target.get("name").and_then(Value::as_str).ok_or_else(|| {
                    format!("cargo metadata bin target in {package_name} has no name")
                })?;
                targets.insert(name.to_owned());
            }
        }
    }
    Ok(targets)
}

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no diagnostic")
        .chars()
        .take(240)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_has_explicit_unknown_state_without_builder_metadata() {
        let provenance = BuildProvenance::current();
        assert!(!provenance.package_version.is_empty());
        assert!(matches!(provenance.status(), "KNOWN" | "UNKNOWN"));
    }

    #[test]
    fn registry_parity_fires_in_both_directions() {
        let live = ["alpha", "planted-unregistered"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let (phantom, unregistered) = registry_parity(&["alpha", "planted-phantom"], &live);
        assert_eq!(phantom, vec!["planted-phantom"]);
        assert_eq!(unregistered, vec!["planted-unregistered"]);
    }

    #[test]
    fn metadata_parser_preserves_empty_for_probe_classification() {
        let targets = parse_bin_targets(br#"{"packages":[]}"#).unwrap();
        assert!(
            targets.is_empty(),
            "the probe must classify this empty oracle explicitly"
        );
    }

    #[test]
    fn metadata_parser_extracts_only_bin_targets() {
        let targets = parse_bin_targets(
            br#"{"packages":[{"name":"alpha","targets":[{"name":"alpha","kind":["bin"]},{"name":"alpha-lib","kind":["lib"]}]}]}"#,
        )
        .expect("valid metadata");
        assert_eq!(targets.into_iter().collect::<Vec<_>>(), vec!["alpha"]);
    }
    #[test]
    fn installed_verbs_exactly_match_source() {
        let installed = crate::umbrella::VERBS
            .iter()
            .map(|verb| (*verb).to_owned())
            .collect::<Vec<_>>();
        let comparison = compare_verb_parity(&installed);
        assert_eq!(comparison.status, "CURRENT");
        assert!(comparison.missing.is_empty());
        assert!(comparison.unexpected.is_empty());
    }

    #[test]
    fn installed_verbs_report_missing_expected_verb() {
        let installed = crate::umbrella::VERBS
            .iter()
            .copied()
            .filter(|verb| *verb != "parity")
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let comparison = compare_verb_parity(&installed);
        assert_eq!(comparison.status, "STALE");
        assert_eq!(comparison.missing, vec!["parity"]);
        assert!(comparison.unexpected.is_empty());
    }

    #[test]
    fn malformed_and_empty_capabilities_responses_are_rejected() {
        assert!(parse_installed_capabilities_verbs("not-json").is_err());
        assert!(parse_installed_capabilities_verbs("{}").is_err());
        let empty = r#"{"schema_version":"omp.umbrella/v1","command":"capabilities","status":"OK","data":{"verbs":[]}}"#;
        assert!(parse_installed_capabilities_verbs(empty).is_err());
    }

    /// The two comparison inputs, built from the REAL source verb set so a test can never
    /// pass against a verb list that drifted away from `umbrella::VERBS`.
    fn matching_verbs() -> VerbParityComparison {
        let installed = crate::umbrella::VERBS
            .iter()
            .map(|verb| (*verb).to_owned())
            .collect::<Vec<_>>();
        let comparison = compare_verb_parity(&installed);
        assert_eq!(comparison.status, "CURRENT", "fixture must be the good leg");
        comparison
    }

    fn differing_verbs() -> VerbParityComparison {
        let installed = crate::umbrella::VERBS
            .iter()
            .copied()
            .filter(|verb| *verb != "parity")
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let comparison = compare_verb_parity(&installed);
        assert_eq!(comparison.status, "STALE", "fixture must be the bad leg");
        comparison
    }

    fn stamps(build_commit: &'static str, source_revision: &'static str) -> BuildProvenance {
        BuildProvenance {
            package_version: PACKAGE_VERSION,
            build_commit,
            source_revision,
        }
    }

    /// FIRES-ON-KNOWN-BAD, on the specimen measured 2026-09-10 at HEAD=08e9bc3:
    /// `~/.local/bin/ompo parity --installed ~/.local/bin/ompo --json` returned
    /// `status=CURRENT reason_code=OMPO_VERB_PARITY_CURRENT rc=0` on stdout while its own
    /// payload said `source_revision=unknown build_commit=unknown`. Twenty verbs matched on
    /// both sides; the artifact was still one adapter behind source.
    ///
    /// BOTH AXES ARE PINNED. A message-only assertion survives the exit code collapsing back
    /// to `0`, and a code-only assertion survives the verdict string reverting to `CURRENT`.
    #[test]
    fn blind_build_cannot_report_current_even_when_every_verb_matches() {
        let verdict = parity_verdict(stamps(UNKNOWN, UNKNOWN), &matching_verbs());
        assert_eq!(verdict.status, "UNKNOWN");
        assert_ne!(
            verdict.status, "CURRENT",
            "a green from an instrument that cannot name its own revision is the defect"
        );
        assert_eq!(verdict.exit_code, PARITY_EXIT_UNKNOWN);
        assert_ne!(
            verdict.exit_code, PARITY_EXIT_CURRENT,
            "an UNKNOWN that exits 0 is the same false green in a new coat"
        );
        assert_eq!(verdict.reason_code, PARITY_REASON_UNSTAMPED_BOTH);
        assert_eq!(
            verdict.missing_provenance,
            vec!["build_commit", "source_revision"]
        );
    }

    /// The reason code must name WHICH input was missing: a reader who is told only that a
    /// stamp is absent cannot tell which one to supply.
    #[test]
    fn unstamped_reason_code_names_the_input_that_was_missing() {
        let good = "0123456789abcdef0123456789abcdef01234567";
        let only_commit_missing = parity_verdict(stamps(UNKNOWN, good), &matching_verbs());
        assert_eq!(
            only_commit_missing.reason_code,
            PARITY_REASON_UNSTAMPED_BUILD_COMMIT
        );
        assert_eq!(only_commit_missing.missing_provenance, vec!["build_commit"]);
        assert_eq!(only_commit_missing.exit_code, PARITY_EXIT_UNKNOWN);

        let only_revision_missing = parity_verdict(stamps(good, UNKNOWN), &matching_verbs());
        assert_eq!(
            only_revision_missing.reason_code,
            PARITY_REASON_UNSTAMPED_SOURCE_REVISION
        );
        assert_eq!(
            only_revision_missing.missing_provenance,
            vec!["source_revision"]
        );
        assert_eq!(only_revision_missing.exit_code, PARITY_EXIT_UNKNOWN);
    }

    /// KNOWN-GOOD. A verdict that can only say UNKNOWN is not a fix, it is an over-strict
    /// gate, and an over-strict gate gets routed around. Same build, same function, other
    /// direction.
    #[test]
    fn stamped_build_with_matching_verbs_is_still_current() {
        let good = "0123456789abcdef0123456789abcdef01234567";
        let verdict = parity_verdict(stamps(good, good), &matching_verbs());
        assert_eq!(verdict.status, "CURRENT");
        assert_eq!(verdict.exit_code, PARITY_EXIT_CURRENT);
        assert_eq!(verdict.reason_code, PARITY_REASON_CURRENT);
        assert!(verdict.missing_provenance.is_empty());
    }

    /// A true red survives the gate. The provenance guard downgrades a claim of health; it
    /// must never upgrade a detected difference into a shrug.
    #[test]
    fn stamped_build_with_differing_verbs_is_stale_not_unknown() {
        let good = "0123456789abcdef0123456789abcdef01234567";
        let verdict = parity_verdict(stamps(good, good), &differing_verbs());
        assert_eq!(verdict.status, "STALE");
        assert_eq!(verdict.exit_code, PARITY_EXIT_STALE);
        assert_eq!(verdict.reason_code, PARITY_REASON_STALE);
    }

    /// Blind AND stale: the verdict declines to over-claim, and the observation is still
    /// carried, so downgrading the verdict never deletes the finding underneath it.
    #[test]
    fn blind_build_keeps_the_verb_finding_beside_the_unknown_verdict() {
        let comparison = differing_verbs();
        let verdict = parity_verdict(stamps(UNKNOWN, UNKNOWN), &comparison);
        assert_eq!(verdict.status, "UNKNOWN");
        assert_eq!(verdict.exit_code, PARITY_EXIT_UNKNOWN);
        let probe = parity_result(
            "/fixture/ompo".to_owned(),
            verdict,
            "fixture".to_owned(),
            Vec::new(),
            Some(comparison),
        );
        assert_eq!(probe.status, "UNKNOWN");
        assert_eq!(probe.verb_set_status, "STALE");
        assert_eq!(probe.missing, vec!["parity"]);
        assert_eq!(probe.unmeasured_axes, PARITY_UNMEASURED_AXES);
    }

    /// Two different revisions on one artifact leave "current source" naming no single
    /// revision. Known is not the same as coherent.
    #[test]
    fn divergent_stamps_cannot_anchor_a_verdict() {
        let verdict = parity_verdict(
            stamps(
                "0123456789abcdef0123456789abcdef01234567",
                "89abcdef0123456789abcdef0123456789abcdef",
            ),
            &matching_verbs(),
        );
        assert_eq!(verdict.status, "UNKNOWN");
        assert_ne!(verdict.status, "CURRENT");
        assert_eq!(verdict.exit_code, PARITY_EXIT_UNKNOWN);
        assert_eq!(verdict.reason_code, PARITY_REASON_PROVENANCE_DIVERGED);
    }

    /// The four parity codes must stay pairwise distinct. Collapsing UNKNOWN onto UNMEASURED
    /// would tell a caller the installed artifact was unreachable when it answered perfectly;
    /// collapsing it onto CURRENT restores the defect this bead exists for.
    #[test]
    fn parity_exit_vocabulary_is_pairwise_distinct() {
        let codes = [
            PARITY_EXIT_CURRENT,
            PARITY_EXIT_STALE,
            PARITY_EXIT_UNKNOWN,
            PARITY_EXIT_UNMEASURED,
        ];
        let mut unique = codes.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), codes.len(), "codes collided: {codes:?}");
    }
}
