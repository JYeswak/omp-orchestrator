//! Per-family skill installation (b11-uegf).
//!
//! Consumes the B09 [`AgentScan`] plus deterministic caller-configured skill
//! roots (family -> directory). Roots are explicit parameters, never guessed
//! from HOME, and must be absolute. Produces exactly one [`AgentOutcome`] per
//! detected family with one of four literal outcomes: `created` (absent file
//! written), `merged` (different file rewritten through the merge seam with
//! prior bytes backed up), `already` (identical bytes present, untouched),
//! `failed` (nothing written for this family).
//!
//! The transaction shape reuses [`merge_hooks`], not a second convention:
//! stage bytes in a dot-prefixed temp file under the destination root (same
//! filesystem), read them back (verify), publish the verified bytes through
//! `merge_hooks` (which backups-then-writes and rolls back its own
//! failures), delete the stage file on every path. A post-write readback
//! compares destination bytes; a mismatch restores from the backup record
//! `merge_hooks` returned (same records, same convention) and the family
//! reports `failed`. Publication here is backup-guarded single writes, not
//! rename-atomic; rename-atomicity lives in `publish_atomic` for artifact
//! flows.
//!
//! Overall success is a separate flag: partial outcomes (including `failed`
//! rows) are always returned so `seal_install_report` can check
//! completeness. An empty scan is [`InstallError::EmptyAgentScan`],
//! mirroring seal.

use super::{merge_hooks, AgentOutcome, AgentScan, HookWrite, InstallError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The installer-managed receipt each family root carries. Content is the
/// installing binary plus HEAD (see [`skill_manifest_bytes`]) -- derived
/// from the install itself, never invented bundle text.
pub const SKILL_FILE_NAME: &str = "INSTALLER.md";

/// Canonical per-family skill roots under an explicit install root. The
/// repo root is a caller parameter (run_install passes its own); nothing
/// here reads HOME or guesses a location.
#[must_use]
pub fn canonical_skill_roots(repo_root: &Path) -> BTreeMap<String, PathBuf> {
    super::agent_families::SUPPORTED_AGENT_FAMILIES
        .iter()
        .map(|(family, _)| {
            (
                (*family).to_owned(),
                repo_root
                    .join(".omp-orchestrator")
                    .join("skills")
                    .join(family),
            )
        })
        .collect()
}

/// Deterministic install-derived receipt bytes: the binary this install
/// published plus the HEAD it built. Same install always yields same bytes,
/// so reruns are `already` rather than churn.
#[must_use]
pub fn skill_manifest_bytes(binary_name: &str, head_sha: &str) -> Vec<u8> {
    format!("# installer-managed agent skills\nbinary: {binary_name}\nhead: {head_sha}\n")
        .into_bytes()
}

/// The sealed phase result run_install consumes: per-family outcomes plus
/// the seal digest proving the report closed over exactly those outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPhaseReport {
    pub outcomes: Vec<AgentOutcome>,
    pub digest: String,
}

/// Production phase for run_install: detector -> executor -> seal.
/// Observed is the full ratified roster (the install provisions the
/// supported set; per-machine narrowing needs a machine-scan surface no
/// bead specifies). The detector still runs, so an emptied roster refuses
/// EmptyAgentScan structurally rather than installing nothing cleanly.
/// Any failed family, or a seal refusal, is a typed refusal carrying every
/// per-family outcome -- never success, never silent.
pub fn install_skills_phase(
    repo_root: &Path,
    binary_name: &str,
    head_sha: &str,
    ownership: &super::RepoOwnership,
    installed_binary: &Path,
) -> Result<SkillPhaseReport, InstallError> {
    use super::agent_families::{detect_agent_families, SUPPORTED_AGENT_FAMILIES};
    let observed: Vec<&str> = SUPPORTED_AGENT_FAMILIES
        .iter()
        .map(|(family, _)| *family)
        .collect();
    let scan = detect_agent_families(&observed)?;
    let roots = canonical_skill_roots(repo_root);
    let manifest = skill_manifest_bytes(binary_name, head_sha);
    let install = install_agent_skills(&scan, &roots, SKILL_FILE_NAME, &manifest)?;
    if !install.success {
        let failed: Vec<String> = install
            .outcomes
            .iter()
            .filter(|row| row.outcome == "failed")
            .map(|row| row.family.clone())
            .collect();
        return Err(InstallError::SkillInstallFailed {
            reason: format!("families failed: {}", failed.join(",")),
            outcomes: install.outcomes,
        });
    }
    let identity = super::verify_identity(installed_binary, head_sha, ownership);
    let sealed = super::seal_install_report(
        &scan,
        install.outcomes.clone(),
        Vec::new(),
        vec![installed_binary.to_path_buf()],
        identity,
    )?;
    Ok(SkillPhaseReport {
        outcomes: install.outcomes,
        digest: sealed.digest,
    })
}

/// Per-family outcomes plus the overall verdict. `success` is false when
/// any family reports `failed`; the outcomes still carry every family so
/// completeness is checkable downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstallReport {
    pub outcomes: Vec<AgentOutcome>,
    pub success: bool,
}

/// Install one skill file for every detected family.
///
/// `skill_roots` maps each family to its destination directory;
/// `skill_name` must be a plain file name and is joined under each root.
/// A family with no configured root, a non-absolute root, or an
/// unwriteable destination reports `failed` without attempting writes
/// outside its root.
pub fn install_agent_skills(
    scan: &AgentScan,
    skill_roots: &BTreeMap<String, PathBuf>,
    skill_name: &str,
    skill_bytes: &[u8],
) -> Result<SkillInstallReport, InstallError> {
    if scan.families.is_empty() {
        return Err(InstallError::EmptyAgentScan);
    }
    let mut outcomes = Vec::with_capacity(scan.families.len());
    for family in &scan.families {
        outcomes.push(install_one_family(
            family,
            skill_roots.get(family),
            skill_name,
            skill_bytes,
        ));
    }
    let success = outcomes.iter().all(|row| row.outcome != "failed");
    Ok(SkillInstallReport { outcomes, success })
}

fn failed(family: &str) -> AgentOutcome {
    AgentOutcome {
        family: family.to_owned(),
        outcome: "failed".to_owned(),
    }
}

fn install_one_family(
    family: &str,
    root: Option<&PathBuf>,
    skill_name: &str,
    skill_bytes: &[u8],
) -> AgentOutcome {
    let row = |outcome: &str| AgentOutcome {
        family: family.to_owned(),
        outcome: outcome.to_owned(),
    };
    // Root policy: explicit and validated. A missing root, a relative
    // root, or an embedded separator that could escape the root reports
    // failed with no write attempted anywhere.
    let Some(root) = root else {
        return failed(family);
    };
    if !root.is_absolute() {
        return failed(family);
    }
    if skill_name.is_empty() || skill_name.contains('/') {
        return failed(family);
    }
    let dest = root.join(skill_name);
    // Identical bytes present: already, untouched (no write, no backup).
    match std::fs::read(&dest) {
        Ok(current) if current == skill_bytes => return row("already"),
        _ => {}
    }
    if std::fs::create_dir_all(root).is_err() {
        return failed(family);
    }
    let existed = std::fs::symlink_metadata(&dest).is_ok();
    // Stage under the destination root (same filesystem), then verify the
    // staged bytes before they go anywhere near the destination.
    let stage = stage_path(root, skill_name);
    let staged_ok = std::fs::write(&stage, skill_bytes)
        .is_ok()
        && std::fs::read(&stage).is_ok_and(|staged| staged == skill_bytes);
    if !staged_ok {
        let _ = std::fs::remove_file(&stage);
        return failed(family);
    }
    let outcome = match merge_hooks(
        &[HookWrite {
            path: dest.clone(),
            merged: skill_bytes.to_vec(),
        }],
        None,
    ) {
        Ok(backups) => {
            // Post-write readback: the destination must carry the staged
            // bytes. A mismatch restores from merge_hooks' own backup
            // record (existed -> copy back; new -> delete) and fails.
            if std::fs::read(&dest).is_ok_and(|landed| landed == skill_bytes) {
                Some(if existed { "merged" } else { "created" })
            } else {
                let backup = &backups[0];
                if backup.existed {
                    let _ = std::fs::copy(&backup.backup, &dest);
                } else {
                    let _ = std::fs::remove_file(&dest);
                }
                None
            }
        }
        Err(_) => None,
    };
    let _ = std::fs::remove_file(&stage);
    match outcome {
        Some(word) => row(word),
        None => failed(family),
    }
}

fn stage_path(root: &Path, skill_name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    root.join(format!(
        ".{skill_name}.stage.{}-{nanos}",
        std::process::id()
    ))
}
