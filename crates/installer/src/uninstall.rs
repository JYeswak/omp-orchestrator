#![forbid(unsafe_code)]

//! Read-only owned-artifact enumeration for the L0 uninstall apply child.
//!
//! This module never removes, renames, writes, or mutates configuration. It
//! converts sealed installer inputs into a deterministic plan and refuses any
//! path whose ownership or filesystem boundary is not provable.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::skill_install::SKILL_FILE_NAME;
use crate::{AgentOutcome, AgentScan, InputManifest, RepoOwnership, SealedInstallReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactKind {
    Binary,
    Hook,
    Skill,
    Report,
    LaunchEntry,
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Binary => "binary",
            Self::Hook => "hook",
            Self::Skill => "skill",
            Self::Report => "report",
            Self::LaunchEntry => "launch_entry",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactPresence {
    Present,
    Absent,
    Residue,
}

impl fmt::Display for ArtifactPresence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Residue => "residue",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPath {
    pub path: PathBuf,
    pub root: PathBuf,
    pub ownership: RepoOwnership,
}

impl UninstallPath {
    #[must_use]
    pub fn this_repo(path: PathBuf, root: PathBuf) -> Self {
        Self {
            path,
            root,
            ownership: RepoOwnership::ThisRepo,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedArtifactRow {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    pub ownership: RepoOwnership,
    pub presence: ArtifactPresence,
    pub deletable: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UninstallRefusal {
    InputManifest { detail: String },
    EmptyCategory { kind: ArtifactKind },
    EmptyFamilyScan,
    SkillOutcomeMismatch { family: String, detail: String },
    DuplicatePath { path: PathBuf },
    PathEscape { path: PathBuf, root: PathBuf },
    SymlinkRefused { path: PathBuf },
    RootUnreadable { root: PathBuf, detail: String },
    PathUnreadable { path: PathBuf, detail: String },
}

impl fmt::Display for UninstallRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputManifest { detail } => write!(formatter, "L0_UNINSTALL_INPUT_MANIFEST: {detail}"),
            Self::EmptyCategory { kind } => write!(formatter, "L0_UNINSTALL_EMPTY_CATEGORY kind={kind}"),
            Self::EmptyFamilyScan => formatter.write_str("L0_UNINSTALL_EMPTY_FAMILY_SCAN"),
            Self::SkillOutcomeMismatch { family, detail } => {
                write!(formatter, "L0_UNINSTALL_SKILL_OUTCOME_MISMATCH family={family}: {detail}")
            }
            Self::DuplicatePath { path } => {
                write!(formatter, "L0_UNINSTALL_DUPLICATE_PATH path={}", path.display())
            }
            Self::PathEscape { path, root } => write!(
                formatter,
                "L0_UNINSTALL_PATH_ESCAPE path={} root={}",
                path.display(),
                root.display()
            ),
            Self::SymlinkRefused { path } => {
                write!(formatter, "L0_UNINSTALL_SYMLINK_REFUSED path={}", path.display())
            }
            Self::RootUnreadable { root, detail } => {
                write!(formatter, "L0_UNINSTALL_ROOT_UNREADABLE root={} detail={detail}", root.display())
            }
            Self::PathUnreadable { path, detail } => {
                write!(formatter, "L0_UNINSTALL_PATH_UNREADABLE path={} detail={detail}", path.display())
            }
        }
    }
}

impl std::error::Error for UninstallRefusal {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallPlan {
    pub input_manifest: InputManifest,
    pub rows: Vec<OwnedArtifactRow>,
    pub apply_eligible: bool,
    pub restrictive_reason: Option<String>,
}

/// Inputs owned by the install pipeline. Every path is accompanied by the
/// root that authorizes it; no HOME or family roster is guessed here.
pub struct UninstallSource<'a> {
    pub input_manifest: &'a InputManifest,
    pub binaries: &'a [UninstallPath],
    pub hooks: &'a [UninstallPath],
    pub agent_scan: &'a AgentScan,
    pub skill_roots: &'a BTreeMap<String, PathBuf>,
    pub skill_outcomes: &'a [AgentOutcome],
    pub sealed_report: &'a SealedInstallReport,
    pub report_root: &'a Path,
    pub launch_entries: &'a [UninstallPath],
}

fn input_manifest_copy(manifest: &InputManifest) -> Result<InputManifest, UninstallRefusal> {
    match manifest {
        InputManifest::Full { digest } if !digest.trim().is_empty() => Ok(manifest.clone()),
        InputManifest::Full { .. } => Err(UninstallRefusal::InputManifest {
            detail: "FULL manifest digest is empty".to_owned(),
        }),
        InputManifest::Partial {
            bound_kind,
            bound_value,
            source,
        } => Err(UninstallRefusal::InputManifest {
            detail: format!("PARTIAL bound={bound_kind} value={bound_value} source={source}"),
        }),
        InputManifest::Refused { reason } => Err(UninstallRefusal::InputManifest {
            detail: format!("REFUSED reason={reason}"),
        }),
    }
}

fn validate_root(root: &Path) -> Result<PathBuf, UninstallRefusal> {
    if !root.is_absolute() {
        return Err(UninstallRefusal::PathEscape {
            path: root.to_owned(),
            root: root.to_owned(),
        });
    }
    root.canonicalize().map_err(|error| UninstallRefusal::RootUnreadable {
        root: root.to_owned(),
        detail: error.to_string(),
    })
}

fn validate_path(path: &UninstallPath) -> Result<ArtifactPresence, UninstallRefusal> {
    let root = validate_root(&path.root)?;
    if !path.path.is_absolute() {
        return Err(UninstallRefusal::PathEscape {
            path: path.path.clone(),
            root,
        });
    }
    let relative = path.path.strip_prefix(&path.root).map_err(|_| UninstallRefusal::PathEscape {
        path: path.path.clone(),
        root: path.root.clone(),
    })?;
    if relative.components().any(|component| matches!(component, Component::ParentDir | Component::RootDir)) {
        return Err(UninstallRefusal::PathEscape {
            path: path.path.clone(),
            root: path.root.clone(),
        });
    }
    match std::fs::symlink_metadata(&path.path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(UninstallRefusal::SymlinkRefused {
                path: path.path.clone(),
            });
        }
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Ok(ArtifactPresence::Residue),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ArtifactPresence::Absent);
        }
        Err(error) => {
            return Err(UninstallRefusal::PathUnreadable {
                path: path.path.clone(),
                detail: error.to_string(),
            });
        }
    }
    if let Some(parent) = path.path.parent() {
        if let Ok(parent) = parent.canonicalize() {
            if !parent.starts_with(&root) {
                return Err(UninstallRefusal::PathEscape {
                    path: path.path.clone(),
                    root,
                });
            }
        }
    }
    Ok(ArtifactPresence::Present)
}

fn row_for(kind: ArtifactKind, path: &UninstallPath) -> Result<OwnedArtifactRow, UninstallRefusal> {
    let presence = validate_path(path)?;
    let deletable = matches!(path.ownership, RepoOwnership::ThisRepo)
        && matches!(presence, ArtifactPresence::Present);
    let reason = match (&path.ownership, presence) {
        (RepoOwnership::ThisRepo, ArtifactPresence::Present) => "owned artifact present".to_owned(),
        (RepoOwnership::ThisRepo, ArtifactPresence::Absent) => "owned artifact absent".to_owned(),
        (RepoOwnership::ThisRepo, ArtifactPresence::Residue) => "expected path is not a regular file".to_owned(),
        (RepoOwnership::Foreign { repo }, _) => format!("foreign owner={repo}; never deletable"),
        (RepoOwnership::Unknown, _) => "ownership unknown; never deletable".to_owned(),
    };
    Ok(OwnedArtifactRow {
        kind,
        path: path.path.clone(),
        ownership: path.ownership.clone(),
        presence,
        deletable,
        reason,
    })
}

fn add_row(
    rows: &mut Vec<OwnedArtifactRow>,
    seen: &mut BTreeSet<PathBuf>,
    kind: ArtifactKind,
    path: &UninstallPath,
) -> Result<(), UninstallRefusal> {
    if !seen.insert(path.path.clone()) {
        return Err(UninstallRefusal::DuplicatePath {
            path: path.path.clone(),
        });
    }
    rows.push(row_for(kind, path)?);
    Ok(())
}

fn add_residue(
    rows: &mut Vec<OwnedArtifactRow>,
    seen: &mut BTreeSet<PathBuf>,
    kind: ArtifactKind,
    path: PathBuf,
    root: PathBuf,
) -> Result<(), UninstallRefusal> {
    if !seen.insert(path.clone()) {
        return Err(UninstallRefusal::DuplicatePath { path });
    }
    validate_path(&UninstallPath {
        path: path.clone(),
        root,
        ownership: RepoOwnership::Unknown,
    })?;
    rows.push(OwnedArtifactRow {
        kind,
        path,
        ownership: RepoOwnership::Unknown,
        presence: ArtifactPresence::Residue,
        deletable: false,
        reason: "residue is not in sealed owned inputs; never deletable".to_owned(),
    });
    Ok(())
}

/// Build the complete deterministic uninstall plan. This function performs
/// filesystem reads only; it never mutates an artifact or configuration file.
pub fn plan_uninstall(source: &UninstallSource<'_>) -> Result<UninstallPlan, UninstallRefusal> {
    let input_manifest = input_manifest_copy(source.input_manifest)?;
    if source.binaries.is_empty() {
        return Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::Binary });
    }
    if source.hooks.is_empty() {
        return Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::Hook });
    }
    if source.launch_entries.is_empty() {
        return Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::LaunchEntry });
    }
    if source.agent_scan.families.is_empty() {
        return Err(UninstallRefusal::EmptyFamilyScan);
    }
    if source.skill_roots.is_empty() || source.skill_outcomes.is_empty() {
        return Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::Skill });
    }
    if source.report_root.as_os_str().is_empty() {
        return Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::Report });
    }

    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for path in source.binaries {
        add_row(&mut rows, &mut seen, ArtifactKind::Binary, path)?;
    }
    for path in source.hooks {
        add_row(&mut rows, &mut seen, ArtifactKind::Hook, path)?;
    }
    for path in source.launch_entries {
        add_row(&mut rows, &mut seen, ArtifactKind::LaunchEntry, path)?;
    }

    let mut families = BTreeSet::new();
    for family in &source.agent_scan.families {
        if family.trim().is_empty() || !families.insert(family) {
            return Err(UninstallRefusal::SkillOutcomeMismatch {
                family: family.clone(),
                detail: "family scan is empty or duplicated".to_owned(),
            });
        }
        let root = source.skill_roots.get(family).ok_or_else(|| UninstallRefusal::SkillOutcomeMismatch {
            family: family.clone(),
            detail: "skill root missing".to_owned(),
        })?;
        let matching = source.skill_outcomes.iter().filter(|outcome| outcome.family == *family).collect::<Vec<_>>();
        if matching.len() != 1 || matching[0].outcome.trim().is_empty() {
            return Err(UninstallRefusal::SkillOutcomeMismatch {
                family: family.clone(),
                detail: "sealed skill outcome missing or duplicated".to_owned(),
            });
        }
        let expected = root.join(SKILL_FILE_NAME);
        add_row(
            &mut rows,
            &mut seen,
            ArtifactKind::Skill,
            &UninstallPath::this_repo(expected.clone(), root.clone()),
        )?;
        if root.is_dir() {
            for entry in std::fs::read_dir(root).map_err(|error| UninstallRefusal::RootUnreadable {
                root: root.clone(),
                detail: error.to_string(),
            })? {
                let entry = entry.map_err(|error| UninstallRefusal::RootUnreadable {
                    root: root.clone(),
                    detail: error.to_string(),
                })?;
                if entry.path() != expected {
                    add_residue(&mut rows, &mut seen, ArtifactKind::Skill, entry.path(), root.clone())?;
                }
            }
        }
    }
    if source.skill_outcomes.iter().any(|outcome| !families.iter().any(|family| *family == &outcome.family)) {
        return Err(UninstallRefusal::SkillOutcomeMismatch {
            family: "<unlisted>".to_owned(),
            detail: "sealed outcome names a family absent from AgentScan".to_owned(),
        });
    }
    if source.skill_outcomes != source.sealed_report.report.agent_outcomes.as_slice() {
        return Err(UninstallRefusal::SkillOutcomeMismatch {
            family: "<sealed-report>".to_owned(),
            detail: "B11 outcomes differ from the B12 sealed report".to_owned(),
        });
    }

    let report_paths = std::iter::once(source.sealed_report.artifact.clone())
        .chain(source.sealed_report.report.backups.iter().cloned());
    for path in report_paths {
        add_row(
            &mut rows,
            &mut seen,
            ArtifactKind::Report,
            &UninstallPath::this_repo(path, source.report_root.to_owned()),
        )?;
    }

    rows.sort_by(|left, right| (left.kind, &left.path).cmp(&(right.kind, &right.path)));
    let restrictive_reason = rows
        .iter()
        .find(|row| !row.deletable && matches!(row.ownership, RepoOwnership::Foreign { .. } | RepoOwnership::Unknown) )
        .map(|row| row.reason.clone())
        .or_else(|| rows.iter().find(|row| matches!(row.presence, ArtifactPresence::Residue)).map(|row| row.reason.clone()));
    Ok(UninstallPlan {
        input_manifest,
        apply_eligible: restrictive_reason.is_none(),
        rows,
        restrictive_reason,
    })
}

/// Alias used by the future apply child; it remains read-only in this child.
pub fn check_uninstall(source: &UninstallSource<'_>) -> Result<UninstallPlan, UninstallRefusal> {
    plan_uninstall(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AttemptIdentity, IdentityCheck, InstallReport};

    struct Fixture {
        root: PathBuf,
        manifest: InputManifest,
        binaries: Vec<UninstallPath>,
        hooks: Vec<UninstallPath>,
        scan: AgentScan,
        roots: BTreeMap<String, PathBuf>,
        outcomes: Vec<AgentOutcome>,
        report: SealedInstallReport,
        report_root: PathBuf,
        launches: Vec<UninstallPath>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("installer-uninstall-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture root");
            let bin_root = root.join("bin");
            let hook_root = root.join("hooks");
            let skill_root = root.join("skills/codex-cli");
            let report_root = root.join("report");
            let launch_root = root.join("launchd");
            for dir in [&bin_root, &hook_root, &skill_root, &report_root, &launch_root] {
                std::fs::create_dir_all(dir).expect("fixture dir");
            }
            let binary = bin_root.join("ompo");
            let hook = hook_root.join("pre-commit");
            let skill = skill_root.join(SKILL_FILE_NAME);
            let report_path = report_root.join("install-report.json");
            let launch = launch_root.join("ai.zeststream.installer.plist");
            for path in [&binary, &hook, &skill, &report_path, &launch] {
                std::fs::write(path, b"owned").expect("fixture artifact");
            }
            let outcomes = vec![AgentOutcome { family: "codex-cli".to_owned(), outcome: "created".to_owned() }];
            let identity = IdentityCheck {
                binary_name: "ompo".to_owned(),
                repo_ownership: RepoOwnership::ThisRepo,
                head_sha: "head".to_owned(),
                build_id_in_binary: Some("head".to_owned()),
                version_output: Some("head".to_owned()),
                consistent: true,
            };
            let report = InstallReport {
                agent_outcomes: outcomes.clone(),
                backups: Vec::new(),
                path_hits: Vec::new(),
                digest: "digest".to_owned(),
                identity,
                attempt_identity: AttemptIdentity { pane: "%60".to_owned(), incarnation: "1".to_owned(), attempt: "test".to_owned() },
                install_metrics: None,
                durability_metric: None,
            };
            Self {
                root,
                manifest: InputManifest::Full { digest: "manifest".to_owned() },
                binaries: vec![UninstallPath::this_repo(binary, bin_root)],
                hooks: vec![UninstallPath::this_repo(hook, hook_root)],
                scan: AgentScan { families: vec!["codex-cli".to_owned()] },
                roots: BTreeMap::from([("codex-cli".to_owned(), skill_root)]),
                outcomes,
                report: SealedInstallReport { report, artifact: report_path, write_bytes: 5, file_fsynced: true, parent_fsynced: true, readback_bytes: 5, artifact_inode: None },
                report_root,
                launches: vec![UninstallPath::this_repo(launch, launch_root)],
            }
        }

        fn source(&self) -> UninstallSource<'_> {
            UninstallSource {
                input_manifest: &self.manifest,
                binaries: &self.binaries,
                hooks: &self.hooks,
                agent_scan: &self.scan,
                skill_roots: &self.roots,
                skill_outcomes: &self.outcomes,
                sealed_report: &self.report,
                report_root: &self.report_root,
                launch_entries: &self.launches,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn plan_covers_all_categories_and_is_deterministic() {
        let fixture = Fixture::new();
        let plan = plan_uninstall(&fixture.source()).expect("complete owned fixture");
        assert!(plan.apply_eligible);
        assert!(plan.restrictive_reason.is_none());
        let kinds = plan.rows.iter().map(|row| row.kind).collect::<BTreeSet<_>>();
        assert_eq!(kinds, BTreeSet::from([ArtifactKind::Binary, ArtifactKind::Hook, ArtifactKind::Skill, ArtifactKind::Report, ArtifactKind::LaunchEntry]));
        let keys = plan.rows.iter().map(|row| (row.kind, row.path.clone())).collect::<Vec<_>>();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "plan ordering must be deterministic");
    }

    #[test]
    fn foreign_and_unknown_rows_are_never_apply_eligible() {
        let mut fixture = Fixture::new();
        fixture.binaries[0].ownership = RepoOwnership::Foreign { repo: "other-repo".to_owned() };
        let plan = plan_uninstall(&fixture.source()).expect("foreign row is a plan, not an action");
        assert!(!plan.apply_eligible);
        assert!(plan.rows.iter().any(|row| matches!(row.ownership, RepoOwnership::Foreign { .. }) && !row.deletable));
        fixture.binaries[0].ownership = RepoOwnership::Unknown;
        let plan = plan_uninstall(&fixture.source()).expect("unknown row is a plan, not an action");
        assert!(!plan.apply_eligible);
        assert!(plan.rows.iter().any(|row| matches!(row.ownership, RepoOwnership::Unknown) && !row.deletable));
    }

    #[test]
    fn empty_duplicate_escape_and_symlink_inputs_refuse() {
        let mut fixture = Fixture::new();
        fixture.hooks.clear();
        assert!(matches!(plan_uninstall(&fixture.source()), Err(UninstallRefusal::EmptyCategory { kind: ArtifactKind::Hook })));

        let mut fixture = Fixture::new();
        fixture.launches.push(fixture.binaries[0].clone());
        assert!(matches!(plan_uninstall(&fixture.source()), Err(UninstallRefusal::DuplicatePath { .. })));

        let mut fixture = Fixture::new();
        fixture.binaries[0].path = fixture.binaries[0].root.join("../escape");
        assert!(matches!(plan_uninstall(&fixture.source()), Err(UninstallRefusal::PathEscape { .. })));

        #[cfg(unix)]
        {
            let mut fixture = Fixture::new();
            let link = fixture.binaries[0].root.join("link");
            std::os::unix::fs::symlink(&fixture.binaries[0].path, &link).expect("symlink fixture");
            fixture.binaries[0].path = link;
            assert!(matches!(plan_uninstall(&fixture.source()), Err(UninstallRefusal::SymlinkRefused { .. })));
        }
    }

    #[test]
    fn absent_and_residue_are_reported_without_mutation() {
        let mut fixture = Fixture::new();
        std::fs::remove_file(&fixture.binaries[0].path).expect("remove expected artifact");
        let extra = fixture.roots["codex-cli"].join("unexpected");
        std::fs::write(&extra, b"residue").expect("residue fixture");
        let plan = plan_uninstall(&fixture.source()).expect("absence and residue are observable data");
        assert!(plan.rows.iter().any(|row| row.kind == ArtifactKind::Binary && row.presence == ArtifactPresence::Absent));
        assert!(plan.rows.iter().any(|row| row.kind == ArtifactKind::Skill && row.presence == ArtifactPresence::Residue));
        assert!(!plan.apply_eligible);
    }
}
