//! OMP version drift: does the installed OMP match the last census?
//!
//! OMP ships almost daily and the user installs on their own cadence, so every
//! version-bound claim in this repo (census counts, the pane-state route
//! decision) goes stale as a matter of routine rather than exception. This
//! module answers ONE question, purely: does the installed version equal the
//! version the newest census artifact was measured at?
//!
//! ANTI-VACUITY: a missing or empty side is `Unknown`, never `Current`. An
//! absent census reading as "in tune" is the exact false-green this exists to
//! kill (measured: the 2026-08-31 census sat at `state=UNKNOWN` for eleven
//! days while every version-bound sentence kept citing it).
//!
//! SINGLE IMPLEMENTATION: the `drift` subcommand of `omp-surface-align` and
//! the `omp_drift` commit-path arm both call [`check_drift`]. A second
//! hand-written comparison is how two callers disagree about what "stale"
//! means. Bead: omp-orchestrator-oqbeb.

use std::path::{Path, PathBuf};

/// Directory, relative to the repo root, holding census artifacts.
pub const CENSUS_DIR: &str = ".flywheel/inventory-artifacts";
/// Filename prefix of census artifacts.
pub const CENSUS_PREFIX: &str = "omp-inventory-map-";
/// Suffix of censuses this reader parses. Legacy `.json.gz` artifacts are
/// deliberately NOT read: adding a compression dependency for one legacy file
/// is the wrong trade, and silently skipping it would be a slice. A tree
/// holding only legacy artifacts reports `Unknown::LegacyCompressedCensus`,
/// which names the remedy (re-census lands `.json`).
pub const CENSUS_SUFFIX: &str = ".json";

/// The drift verdict. Comparable by tests; rendered by callers with a fixed
/// prefix so a message-only assertion cannot pass on the wrong verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftVerdict {
    /// Installed and census agree. The version is carried so the caller
    /// prints WHAT matched, never a bare "clean".
    Current { version: String },
    /// Installed moved past the census. Both sides carried: a drift report
    /// naming one version is unactionable.
    Drifted { installed: String, census: String },
    /// Either side could not be established. The reason is carried because
    /// "unknown" without a reason is a refusal a reader cannot satisfy.
    Unknown { reason: DriftUnknown },
}

/// Why the drift question could not be answered. Each variant is a distinct
/// remedy, so they are distinct variants rather than one string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftUnknown {
    /// No `omp-inventory-map-*.json` under the census dir.
    NoCensusArtifact,
    /// Only legacy `.json.gz` artifacts exist; re-census to land `.json`.
    LegacyCompressedCensus,
    /// The newest artifact could not be read or parsed.
    CensusUnreadable { detail: String },
    /// The artifact parsed but carries no usable `data.omp_version`.
    CensusVersionAbsent,
    /// The installed probe produced nothing usable.
    InstalledUnknown { detail: String },
}

impl DriftUnknown {
    /// Machine-stable reason token, asserted by tests (rule 7: the message
    /// half of a leg must pin what the mutation moves, and here the reason
    /// token IS the discriminating substring).
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::NoCensusArtifact => "no_census_artifact".to_owned(),
            Self::LegacyCompressedCensus => "legacy_compressed_census".to_owned(),
            Self::CensusUnreadable { detail } => format!("census_unreadable:{detail}"),
            Self::CensusVersionAbsent => "census_version_absent".to_owned(),
            Self::InstalledUnknown { detail } => format!("installed_unknown:{detail}"),
        }
    }
}

/// Normalize one version side: trim, drop a single leading `omp/` prefix (the
/// census stores `omp/18.0.11` while a hand-passed probe may carry `18.0.11`),
/// trim again. Empty after normalization counts as absent.
fn normalize(raw: &str) -> Option<String> {
    let core = raw.trim().strip_prefix("omp/").unwrap_or(raw.trim()).trim();
    if core.is_empty() {
        None
    } else {
        Some(core.to_owned())
    }
}

/// Pure drift decision over two optional version strings.
///
/// `None`/empty on either side is `Unknown`, never `Current`: the caller that
/// cannot establish what it measured has no standing to declare agreement.
#[must_use]
pub fn check_drift(installed: Option<&str>, census: Option<&str>) -> DriftVerdict {
    let installed = installed.and_then(normalize);
    let census = census.and_then(normalize);
    match (installed, census) {
        (Some(installed), Some(census)) if installed == census => {
            DriftVerdict::Current { version: installed }
        }
        (Some(installed), Some(census)) => DriftVerdict::Drifted { installed, census },
        (None, _) => DriftVerdict::Unknown {
            reason: DriftUnknown::InstalledUnknown {
                detail: "no usable installed version".to_owned(),
            },
        },
        (Some(_), None) => DriftVerdict::Unknown {
            reason: DriftUnknown::CensusVersionAbsent,
        },
    }
}
/// Render the verdict as its ONE canonical line. Callers print this verbatim;
/// tests pin it byte-for-byte so a rewording that inverts the meaning fails.
#[must_use]
pub fn render_verdict(verdict: &DriftVerdict) -> String {
    match verdict {
        DriftVerdict::Current { version } => {
            format!("OMP_DRIFT state=CURRENT version={version}")
        }
        DriftVerdict::Drifted { installed, census } => {
            format!("OMP_DRIFT state=DRIFTED installed={installed} census={census}")
        }
        DriftVerdict::Unknown { reason } => {
            format!("OMP_DRIFT state=UNKNOWN reason={}", reason.reason())
        }
    }
}

/// Newest parseable census artifact: `omp-inventory-map-*.json` under the
/// census dir, lexicographically last (artifacts are date-stamped, so lexical
/// order IS chronological order — the same convention the roster ledger
/// relies on).
#[must_use]
pub fn latest_census_artifact(repo_root: &Path) -> CensusSearch {
    let dir = repo_root.join(CENSUS_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return CensusSearch::None,
    };
    let mut newest: Option<PathBuf> = None;
    let mut saw_legacy = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(CENSUS_PREFIX) && name.ends_with(CENSUS_SUFFIX) {
            let current_name = newest.as_ref().and_then(|current: &PathBuf| {
                current.file_name().map(|c| c.to_string_lossy().into_owned())
            });
            if current_name.is_none_or(|current_name| current_name < name) {
                newest = Some(entry.path());
            }
        } else if name.starts_with(CENSUS_PREFIX) && name.ends_with(".json.gz") {
            saw_legacy = true;
        }
    }
    match newest {
        Some(path) => CensusSearch::Found(path),
        None if saw_legacy => CensusSearch::LegacyOnly,
        None => CensusSearch::None,
    }
}

/// Outcome of searching for a census artifact. Three arms, because "no file"
/// and "only an unreadable file" have different remedies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CensusSearch {
    Found(PathBuf),
    LegacyOnly,
    None,
}

/// Extract `data.omp_version` from a census artifact's bytes.
pub fn census_version_from_bytes(bytes: &[u8]) -> Result<String, DriftUnknown> {
    let text =
        std::str::from_utf8(bytes).map_err(|e| DriftUnknown::CensusUnreadable {
            detail: format!("not_utf8:{e}"),
        })?;
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| DriftUnknown::CensusUnreadable {
            detail: format!("not_json:{e}"),
        })?;
    let version = value
        .get("data")
        .and_then(|d| d.get("omp_version"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if normalize(version).is_none() {
        return Err(DriftUnknown::CensusVersionAbsent);
    }
    Ok(version.trim().to_owned())
}

/// Full live check: newest artifact plus an installed version string.
/// Pure over its inputs except the filesystem read, which is the subject.
#[must_use]
pub fn check_repo_drift(repo_root: &Path, installed: Option<&str>) -> DriftVerdict {
    match latest_census_artifact(repo_root) {
        CensusSearch::None => DriftVerdict::Unknown {
            reason: DriftUnknown::NoCensusArtifact,
        },
        CensusSearch::LegacyOnly => DriftVerdict::Unknown {
            reason: DriftUnknown::LegacyCompressedCensus,
        },
        CensusSearch::Found(path) => {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return DriftVerdict::Unknown {
                        reason: DriftUnknown::CensusUnreadable {
                            detail: format!("read:{e}"),
                        },
                    };
                }
            };
            match census_version_from_bytes(&bytes) {
                Ok(census) => check_drift(installed, Some(&census)),
                Err(reason) => DriftVerdict::Unknown { reason },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// KNOWN-GOOD: equal versions agree, and the line names the version.
    #[test]
    fn equal_versions_are_current_and_name_the_version() {
        let verdict = check_drift(Some("omp/18.1.18"), Some("omp/18.1.18"));
        assert_eq!(
            verdict,
            DriftVerdict::Current {
                version: "18.1.18".to_owned()
            }
        );
        assert_eq!(render_verdict(&verdict), "OMP_DRIFT state=CURRENT version=18.1.18");
    }

    /// Prefix asymmetry is normalization, not drift: the census stores the
    /// `omp/` prefix while a hand-passed probe may not.
    #[test]
    fn prefix_asymmetry_is_not_drift() {
        assert!(matches!(
            check_drift(Some("18.1.18"), Some("omp/18.1.18")),
            DriftVerdict::Current { .. }
        ));
    }

    /// FIRES-ON-KNOWN-BAD: the live-measured pair (installed 18.1.18 against
    /// the 18.0.11 census) must DRIFT with BOTH versions in the line.
    #[test]
    fn the_measured_pair_drifts_with_both_versions_named() {
        let verdict = check_drift(Some("omp/18.1.18"), Some("omp/18.0.11"));
        assert_eq!(
            verdict,
            DriftVerdict::Drifted {
                installed: "18.1.18".to_owned(),
                census: "18.0.11".to_owned(),
            }
        );
        assert_eq!(
            render_verdict(&verdict),
            "OMP_DRIFT state=DRIFTED installed=18.1.18 census=18.0.11"
        );
    }

    /// ANTI-VACUITY: every absent side is UNKNOWN with its own reason, never
    /// CURRENT. A gate that cannot read its inputs must say so.
    #[test]
    fn absent_sides_are_unknown_never_current() {
        assert_eq!(
            render_verdict(&check_drift(None, Some("omp/18.0.11"))),
            "OMP_DRIFT state=UNKNOWN reason=installed_unknown:no usable installed version"
        );
        assert_eq!(
            render_verdict(&check_drift(Some("omp/18.1.18"), None)),
            "OMP_DRIFT state=UNKNOWN reason=census_version_absent"
        );
        assert_eq!(
            render_verdict(&check_drift(Some("omp/18.1.18"), Some("  "))),
            "OMP_DRIFT state=UNKNOWN reason=census_version_absent"
        );
        assert_eq!(
            render_verdict(&check_drift(None, None)),
            "OMP_DRIFT state=UNKNOWN reason=installed_unknown:no usable installed version"
        );
    }

    /// The legacy `.json.gz` census parses for its version field shape: the
    /// fixture mirrors the real 2026-08-31 artifact's envelope so the reader
    /// stays honest about what it refuses to parse.
    #[test]
    fn census_version_extraction_reads_the_real_envelope() {
        let fixture = r#"{"schema_version":"x","data":{"omp_version":"omp/18.0.11"}}"#;
        assert_eq!(
            census_version_from_bytes(fixture.as_bytes()),
            Ok("omp/18.0.11".to_owned())
        );
        assert_eq!(
            census_version_from_bytes(b"{}"),
            Err(DriftUnknown::CensusVersionAbsent)
        );
        assert!(matches!(
            census_version_from_bytes(b"not json"),
            Err(DriftUnknown::CensusUnreadable { .. })
        ));
    }
}
