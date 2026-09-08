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

/// Exit code for an installed artifact whose verb set exactly matches this source.
pub const PARITY_EXIT_CURRENT: u8 = 0;
/// Exit code for an installed artifact whose verb set differs from this source.
pub const PARITY_EXIT_STALE: u8 = 1;
/// Exit code when the installed artifact cannot be measured safely.
pub const PARITY_EXIT_UNMEASURED: u8 = 4;
const PARITY_DEADLINE: Duration = Duration::from_secs(10);

/// Typed comparison of one installed ompo artifact's advertised verbs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledVerbParityProbe {
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
    let missing = expected.iter().filter(|verb| !installed_set.contains(**verb)).map(|verb| (*verb).to_owned()).collect::<Vec<_>>();
    let unexpected = installed_set.iter().filter(|verb| !expected.contains(**verb)).map(|verb| (*verb).to_owned()).collect::<Vec<_>>();
    let status = if missing.is_empty() && unexpected.is_empty() { "CURRENT" } else { "STALE" };
    VerbParityComparison { status, missing, unexpected }
}

/// Parse the exact capabilities envelope emitted by an installed ompo binary.
pub fn parse_installed_capabilities_verbs(raw: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(raw).map_err(|error| format!("capabilities response is invalid JSON: {error}"))?;
    if value.get("schema_version").and_then(Value::as_str) != Some(crate::umbrella::SCHEMA_VERSION) {
        return Err("capabilities response has an unsupported schema_version".to_owned());
    }
    if value.get("command").and_then(Value::as_str) != Some("capabilities") {
        return Err("capabilities response is not the capabilities command".to_owned());
    }
    if value.get("status").and_then(Value::as_str) != Some("OK") {
        return Err("capabilities response did not report status=OK".to_owned());
    }
    let verbs = value.get("data").and_then(Value::as_object).and_then(|data| data.get("verbs")).and_then(Value::as_array).ok_or_else(|| "capabilities response has no data.verbs array".to_owned())?;
    if verbs.is_empty() {
        return Err("capabilities response has an empty data.verbs array".to_owned());
    }
    let mut parsed = Vec::with_capacity(verbs.len());
    let mut seen = BTreeSet::new();
    for verb in verbs {
        let verb = verb.as_str().filter(|verb| !verb.is_empty()).ok_or_else(|| "capabilities response data.verbs contains a non-string or empty verb".to_owned())?;
        if !seen.insert(verb) {
            return Err(format!("capabilities response data.verbs repeats {verb:?}"));
        }
        parsed.push(verb.to_owned());
    }
    Ok(parsed)
}

fn parity_result(installed_path: String, status: &'static str, exit_code: u8, reason_code: String, message: String, detail: String, installed_verbs: Vec<String>, missing: Vec<String>, unexpected: Vec<String>) -> InstalledVerbParityProbe {
    InstalledVerbParityProbe { status, exit_code, reason_code, message, detail, provenance: BuildProvenance::current(), installed_path, installed_verbs, missing, unexpected }
}

/// Execute an installed ompo path and compare its capabilities verbs with this source.
#[must_use]
pub fn probe_installed_verb_parity(installed: &Path) -> InstalledVerbParityProbe {
    let installed_path = installed.display().to_string();
    let mut command = Command::new(installed);
    command.args(["capabilities", "--json"]);
    let output = match bounded_output(&mut command, PARITY_DEADLINE) {
        BoundedOutcome::TimedOut => return parity_result(installed_path, "UNMEASURED", PARITY_EXIT_UNMEASURED, "OMPO_VERB_PARITY_TIMEOUT".to_owned(), "installed ompo could not be measured".to_owned(), format!("capabilities probe exceeded {}s", PARITY_DEADLINE.as_secs()), Vec::new(), Vec::new(), Vec::new()),
        BoundedOutcome::Unspawned(error) => return parity_result(installed_path, "UNMEASURED", PARITY_EXIT_UNMEASURED, "OMPO_VERB_PARITY_UNRUNNABLE".to_owned(), "installed ompo could not be measured".to_owned(), format!("capabilities probe could not be spawned: {error}"), Vec::new(), Vec::new(), Vec::new()),
        BoundedOutcome::Completed(output) => output,
    };
    if !output.status.success() {
        return parity_result(installed_path, "UNMEASURED", PARITY_EXIT_UNMEASURED, "OMPO_VERB_PARITY_CHILD_FAILED".to_owned(), "installed ompo could not be measured".to_owned(), format!("capabilities probe exited {} detail={}", output.status, first_line(&output.stderr)), Vec::new(), Vec::new(), Vec::new());
    }
    let raw = match std::str::from_utf8(&output.stdout) {
        Ok(raw) => raw,
        Err(error) => return parity_result(installed_path, "UNMEASURED", PARITY_EXIT_UNMEASURED, "OMPO_VERB_PARITY_INVALID_UTF8".to_owned(), "installed ompo could not be measured".to_owned(), format!("capabilities response is not UTF-8: {error}"), Vec::new(), Vec::new(), Vec::new()),
    };
    let installed_verbs = match parse_installed_capabilities_verbs(raw) {
        Ok(verbs) => verbs,
        Err(detail) => return parity_result(installed_path, "UNMEASURED", PARITY_EXIT_UNMEASURED, "OMPO_VERB_PARITY_MALFORMED".to_owned(), "installed ompo could not be measured".to_owned(), detail, Vec::new(), Vec::new(), Vec::new()),
    };
    let comparison = compare_verb_parity(&installed_verbs);
    let provenance = BuildProvenance::current();
    let detail = if comparison.status == "CURRENT" { format!("source_revision={} build_commit={} installed_path={} verbs={}", provenance.source_revision, provenance.build_commit, installed.display(), installed_verbs.len()) } else { format!("source_revision={} build_commit={} installed_path={} missing={:?} unexpected={:?}", provenance.source_revision, provenance.build_commit, installed.display(), comparison.missing, comparison.unexpected) };
    let message = if comparison.status == "CURRENT" { "installed verb set matches current source" } else { "installed verb set differs from current source" };
    let (exit_code, reason_code) = if comparison.status == "CURRENT" { (PARITY_EXIT_CURRENT, "OMPO_VERB_PARITY_CURRENT") } else { (PARITY_EXIT_STALE, "OMPO_VERB_PARITY_STALE") };
    parity_result(installed_path, comparison.status, exit_code, reason_code.to_owned(), message.to_owned(), detail, installed_verbs, comparison.missing, comparison.unexpected)
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
        .filter(|name| !live_bin_targets.iter().any(|target| target.as_str() == **name))
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
        .args(["metadata", "--no-deps", "--format-version", "1", "--offline"])
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
                .ok_or_else(|| format!("cargo metadata target in {package_name} has no kind array"))?;
            if kinds.iter().any(|kind| kind.as_str() == Some("bin")) {
                let name = target
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("cargo metadata bin target in {package_name} has no name"))?;
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
        let (phantom, unregistered) = registry_parity(
            &["alpha", "planted-phantom"],
            &live,
        );
        assert_eq!(phantom, vec!["planted-phantom"]);
        assert_eq!(unregistered, vec!["planted-unregistered"]);
    }

    #[test]
    fn metadata_parser_preserves_empty_for_probe_classification() {
        let targets = parse_bin_targets(br#"{"packages":[]}"#).unwrap();
        assert!(targets.is_empty(), "the probe must classify this empty oracle explicitly");
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
        let installed = crate::umbrella::VERBS.iter().map(|verb| (*verb).to_owned()).collect::<Vec<_>>();
        let comparison = compare_verb_parity(&installed);
        assert_eq!(comparison.status, "CURRENT");
        assert!(comparison.missing.is_empty());
        assert!(comparison.unexpected.is_empty());
    }

    #[test]
    fn installed_verbs_report_missing_expected_verb() {
        let installed = crate::umbrella::VERBS.iter().copied().filter(|verb| *verb != "parity").map(str::to_owned).collect::<Vec<_>>();
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
}
