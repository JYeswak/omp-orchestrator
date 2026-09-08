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
}
