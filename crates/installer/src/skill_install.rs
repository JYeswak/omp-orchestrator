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
