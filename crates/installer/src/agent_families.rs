//! Ratified supported-agent roster + detection (b09-x282, t12-6t0k).
//!
//! The ten families are RATIFIED, not derived: no source in reach yields ten,
//! so the conductor ratified this list with per-row citations (b09 ruling).
//! A row without a citation re-creates the ungradeable roster this file
//! exists to avoid. Swap rule (from the ratification): a path helper for
//! goose/cline/kilo swaps one in -- say which row dropped.

use super::{AgentScan, InstallError};

/// (family, citation). Order is stable and meaningless; detect by name.
pub const SUPPORTED_AGENT_FAMILIES: &[(&str, &str)] = &[
    (
        "claude-code",
        "MEASURED on this machine: ~/.claude present; ~/.omp/profiles/claude",
    ),
    (
        "codex-cli",
        "MEASURED: ~/.omp/profiles/codex; OMP bundle; caut doctor reports a codex CLI at /opt/homebrew/bin/codex, authenticated via OAuth",
    ),
    (
        "gemini-cli",
        "OMP bundle, 13 path-helper hits -- strongest external signal of the set",
    ),
    (
        "opencode",
        "OMP bundle, 9 path-helper hits",
    ),
    (
        "cursor",
        "OMP bundle, 6 path-helper hits (token contaminated elsewhere; the helper form is the citation)",
    ),
    (
        "windsurf",
        "OMP bundle, 4 path-helper hits, with mcp_config.json and memories/global_rules.md",
    ),
    (
        "copilot",
        "OMP bundle; github 5 path-helper hits",
    ),
    (
        "aider",
        "MEASURED on this machine: ~/.aider present",
    ),
    ("amp", "OMP bundle string"),
    ("crush", "OMP bundle string"),
];

/// Is this name a supported family?
pub fn is_supported_family(name: &str) -> bool {
    SUPPORTED_AGENT_FAMILIES
        .iter()
        .any(|(family, _)| *family == name)
}

/// Detect which supported families are present in an observed name set.
///
/// CLOSED WORLD: only roster members enter the scan. Observing solely
/// unsupported names is an empty scan ([`InstallError::EmptyAgentScan`]) --
/// the roster is the ratified support set, and an installer cannot install
/// for families outside it. Empty observed is the same error, never clean.
pub fn detect_agent_families(observed: &[&str]) -> Result<AgentScan, InstallError> {
    let supported: Vec<&str> = observed
        .iter()
        .copied()
        .filter(|name| is_supported_family(name))
        .collect();
    crate::classify_agent_scan(&supported)
}

/// Roster members absent from an observed set. t12's hiding primitive: hide
/// one family and the report must name it.
pub fn missing_families(observed: &[&str]) -> Vec<String> {
    SUPPORTED_AGENT_FAMILIES
        .iter()
        .map(|(family, _)| (*family).to_owned())
        .filter(|family| !observed.contains(&family.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstallError;
    #[test]
    fn roster_families_are_cited_and_distinct() {
        // NO absolute count: a ratchet keyed on len() is red by construction
        // the next time the ratified list legitimately changes. Assert
        // per-family properties instead; the ten-name expectation lives in
        // b09's bead, not in a number here.
        let mut names: Vec<&str> = SUPPORTED_AGENT_FAMILIES
            .iter()
            .map(|(family, _)| *family)
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(
            names.len(),
            SUPPORTED_AGENT_FAMILIES.len(),
            "family names must be distinct"
        );
        for (family, citation) in SUPPORTED_AGENT_FAMILIES {
            assert!(!family.is_empty(), "empty family name");
            assert!(!citation.is_empty(), "{family} carries no citation");
        }
    }

    #[test]
    fn empty_observed_is_error_never_clean() {
        assert!(matches!(
            detect_agent_families(&[]),
            Err(InstallError::EmptyAgentScan)
        ));
    }

    #[test]
    fn all_observed_yields_all_named() {
        let observed: Vec<&str> = SUPPORTED_AGENT_FAMILIES
            .iter()
            .map(|(family, _)| *family)
            .collect();
        let scan = detect_agent_families(&observed).expect("ten named is a scan");
        assert_eq!(scan.families.len(), SUPPORTED_AGENT_FAMILIES.len());
        assert!(missing_families(&observed).is_empty());
    }

    #[test]
    fn hide_one_names_the_missing_family() {
        for hidden in ["claude-code", "crush"] {
            let observed: Vec<&str> = SUPPORTED_AGENT_FAMILIES
                .iter()
                .map(|(family, _)| *family)
                .filter(|family| *family != hidden)
                .collect();
            assert_eq!(missing_families(&observed), vec![hidden.to_owned()]);
        }
    }

    #[test]
    fn unsupported_names_never_enter_the_scan() {
        assert!(matches!(
            detect_agent_families(&["not-a-family"]),
            Err(InstallError::EmptyAgentScan)
        ));
    }
}
