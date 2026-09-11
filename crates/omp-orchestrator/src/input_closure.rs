//! Does a crate's build read only COMMITTED content? And which claims may its result carry?
//!
//! # Why this exists (`omp-orchestrator-rluzf`, legs 4 and 6)
//!
//! `rch exec` syncs the WORKTREE, including untracked files, so every local and
//! remote green is a worktree green until someone proves otherwise. On
//! 2026-09-11 a dropped stash left 81 tracked files dirty across 36 crates, and
//! one of them — `omp-types/src/claim_strength.rs` — sits in the closure of at
//! least seven crates, so a single file made seven crates unmeasurable at once.
//! Two verdicts were demoted by hand within ten minutes of the method being
//! written down. **A hand method used three times is a missing crate.**
//!
//! # The two enumerations, because one cannot answer the other's question
//!
//! 1. **Committed inputs vs `HEAD`** — did anything my build reads drift?
//! 2. **Untracked files in the closure** — is there an input `HEAD` has never
//!    seen? Hashing against `HEAD` is structurally blind to this: there is
//!    nothing on the other side of the comparison. `members = ["crates/*"]`
//!    enrols untracked directories, and `rch` ships them, so such an input is
//!    PRESENT for every remote build and ABSENT from every clone — measured
//!    live as `gate-runner --plan` reporting 91 crates on a worker and 89 in a
//!    clone of the same commit.
//!
//! # The claim tier is the output that matters
//!
//! Step 1 is not a pass/fail gate before measuring; it decides WHICH CLAIMS the
//! measurement may carry. A checker that reports drift without naming the tier
//! leaves the reader to guess the thing the ladder exists to decide.
//!
//! ```text
//! empty drift set AND no untracked input  ->  Absolute            (a HEAD verdict)
//! non-empty, bracketed across the arms    ->  DifferentialsOnly   (known-bads survive)
//! non-empty, unbracketed                  ->  ResolutionStabilityOnly
//! ```
//! A clean closure beats a bracketed dirty one: bracketing DETECTS that your
//! closure moved, emptiness makes it impossible.
//!
//! # Members that a naive input set omits
//!
//! * `build.rs` — compiled AND EXECUTED, so a dirty one changes generated code
//!   while appearing in no source diff of any crate under test. First live
//!   specimen: `tick-monitor/build.rs`.
//! * `include_str!` / `include_bytes!` targets — data that is part of the
//!   compilation with no `mod` declaring it.
//! * the crate's OWN `Cargo.toml`, the workspace manifest, the lock, and
//!   `.cargo/config.toml`.
//! * `[dependencies]` path edges are in the unit; `[dev-dependencies]` edges OF
//!   A DEPENDENCY are NOT. That polarity is decided by the section header and a
//!   grep for the crate name cannot settle it.
//!
//! # Why the reads are injected
//!
//! `assess` takes readers rather than touching git, because the failure this
//! module must never reproduce is its own: an ad-hoc version of this check once
//! reported `DRIFT=9, all ABSENT_IN_HEAD` when the bug was a clobbered variable
//! in the caller. A checker that cannot tell ITS OWN BUG from YOUR DRIFT is the
//! single-valued instrument this repo keeps retiring, so `Unreadable` is a
//! distinct outcome and never collapses into either `Clean` or `Drifted`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Which claims a measurement over this closure is entitled to carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimTier {
    /// No committed input drifts and no untracked file is in the closure.
    Absolute,
    /// Something drifts. A known-bad differential still holds if the drift set
    /// was hashed across every arm; an absolute green does not.
    DifferentialsOnly,
    /// Something drifts and nobody proved it held still.
    ResolutionStabilityOnly,
}

impl ClaimTier {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Absolute => "ABSOLUTE",
            Self::DifferentialsOnly => "DIFFERENTIALS_ONLY",
            Self::ResolutionStabilityOnly => "RESOLUTION_STABILITY_ONLY",
        }
    }

    /// The one question a reader actually has.
    #[must_use]
    pub const fn head_verdict_available(self) -> bool {
        matches!(self, Self::Absolute)
    }
}

/// How one input differs. Distinct variants because the repairs differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// Tracked, committed, and the worktree bytes differ.
    DiffersFromHead,
    /// In the closure and absent from `HEAD` — a committed caller with an
    /// uncommitted callee is this shape, and no build can see it.
    AbsentInHead,
    /// Committed and missing from the worktree (a staged deletion).
    AbsentOnDisk,
}

impl DriftKind {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DiffersFromHead => "DIFFERS_FROM_HEAD",
            Self::AbsentInHead => "ABSENT_IN_HEAD",
            Self::AbsentOnDisk => "ABSENT_ON_DISK",
        }
    }
}

/// What a `HEAD` reader found. `Unreadable` is NOT `Absent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadRead {
    Found(Vec<u8>),
    Absent,
    Unreadable(String),
}

/// What a worktree reader found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskRead {
    Found(Vec<u8>),
    Absent,
    Unreadable(String),
}

/// A refusal. Never a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    /// The scan was empty, so "no drift" would be vacuous. An empty DIRTY set
    /// is a legitimate pass; an empty SCAN is an error.
    EmptyScan,
    /// The tree could not be read. Distinct from `Clean` by construction: if
    /// ABSENT and CLEAN looked identical the instrument could not answer the
    /// question it exists for.
    TreeUnreadable { path: PathBuf, detail: String },
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyScan => write!(
                f,
                "INPUT_CLOSURE_ERROR reason=EMPTY_SCAN detail=an empty scan cannot report a clean closure"
            ),
            Self::TreeUnreadable { path, detail } => write!(
                f,
                "INPUT_CLOSURE_ERROR reason=TREE_UNREADABLE path={} detail={detail}",
                path.display()
            ),
        }
    }
}

/// The verdict, carrying its own denominator and its own claim tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub scanned: usize,
    pub drift: Vec<(PathBuf, DriftKind)>,
    pub untracked: Vec<PathBuf>,
}

impl Verdict {
    #[must_use]
    pub fn tier(&self, bracketed: bool) -> ClaimTier {
        if self.drift.is_empty() && self.untracked.is_empty() {
            return ClaimTier::Absolute;
        }
        if bracketed {
            ClaimTier::DifferentialsOnly
        } else {
            ClaimTier::ResolutionStabilityOnly
        }
    }

    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.drift.is_empty() && self.untracked.is_empty()
    }

    /// One line, naming EVERY drifting path. A checker that reddens without
    /// naming which input drifted cannot tell you which one you got wrong.
    #[must_use]
    pub fn render(&self, crate_name: &str, bracketed: bool) -> String {
        let tier = self.tier(bracketed);
        if self.is_clean() {
            return format!(
                "INPUT_CLOSURE_CLEAN crate={crate_name} scanned={} drift=0 untracked=0 tier={} head_verdict={}",
                self.scanned,
                tier.label(),
                tier.head_verdict_available()
            );
        }
        let rows = self
            .drift
            .iter()
            .map(|(path, kind)| format!("{}:{}", path.display(), kind.code()))
            .collect::<Vec<_>>()
            .join(",");
        let untracked = self
            .untracked
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "INPUT_CLOSURE_DRIFT crate={crate_name} scanned={} drift={} untracked={} tier={} head_verdict={} drifting={} untracked_paths={}",
            self.scanned,
            self.drift.len(),
            self.untracked.len(),
            tier.label(),
            tier.head_verdict_available(),
            if rows.is_empty() { "NONE".to_owned() } else { rows },
            if untracked.is_empty() { "NONE".to_owned() } else { untracked },
        )
    }
}

/// Compare every input against `HEAD`, and report the untracked second
/// enumeration alongside it.
///
/// Readers are injected so this is measurable without a git tree, and so an
/// instrument failure is a REFUSAL rather than a drift report.
pub fn assess(
    inputs: &[PathBuf],
    untracked: &[PathBuf],
    head: &dyn Fn(&Path) -> HeadRead,
    disk: &dyn Fn(&Path) -> DiskRead,
) -> Result<Verdict, CheckError> {
    if inputs.is_empty() {
        return Err(CheckError::EmptyScan);
    }
    let mut drift = Vec::new();
    for path in inputs {
        let committed = head(path);
        let present = disk(path);
        if let HeadRead::Unreadable(detail) = &committed {
            return Err(CheckError::TreeUnreadable {
                path: path.clone(),
                detail: detail.clone(),
            });
        }
        if let DiskRead::Unreadable(detail) = &present {
            return Err(CheckError::TreeUnreadable {
                path: path.clone(),
                detail: detail.clone(),
            });
        }
        match (committed, present) {
            (HeadRead::Found(a), DiskRead::Found(b)) if a == b => {}
            (HeadRead::Found(_), DiskRead::Found(_)) => {
                drift.push((path.clone(), DriftKind::DiffersFromHead));
            }
            (HeadRead::Found(_), DiskRead::Absent) => {
                drift.push((path.clone(), DriftKind::AbsentOnDisk));
            }
            (HeadRead::Absent, _) => drift.push((path.clone(), DriftKind::AbsentInHead)),
            (HeadRead::Unreadable(_), _) | (_, DiskRead::Unreadable(_)) => unreachable!(
                "unreadable is returned above; keeping this arm explicit so a new \
                 variant cannot fall through to a clean verdict"
            ),
        }
    }
    let mut untracked = untracked.to_vec();
    untracked.sort();
    untracked.dedup();
    Ok(Verdict {
        scanned: inputs.len(),
        drift,
        untracked,
    })
}

/// Which manifest section a path dependency was declared in. The polarity that
/// decides closure membership, and a grep for the crate name cannot settle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Dependencies,
    DevDependencies,
    Other,
}

fn section_of(line: &str) -> Option<Section> {
    let header = line.trim();
    if !header.starts_with('[') {
        return None;
    }
    Some(match header {
        "[dependencies]" | "[build-dependencies]" => Section::Dependencies,
        "[dev-dependencies]" => Section::DevDependencies,
        _ => Section::Other,
    })
}

/// Path dependencies declared in `[dependencies]` / `[build-dependencies]`.
///
/// `[build-dependencies]` counts: a build script is compiled and run, so what
/// it links is part of the compilation.
#[must_use]
pub fn path_dependencies(manifest_text: &str) -> Vec<String> {
    let mut section = Section::Other;
    let mut found = Vec::new();
    for line in manifest_text.lines() {
        if let Some(next) = section_of(line) {
            section = next;
            continue;
        }
        if section != Section::Dependencies {
            continue;
        }
        if let Some(rest) = line.split_once("path = \"../") {
            if let Some(name) = rest.1.split('"').next() {
                if !name.is_empty() {
                    found.push(name.trim_end_matches('/').to_owned());
                }
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// `include_str!` / `include_bytes!` targets, resolved relative to the file
/// that names them. Part of the compilation with no `mod` declaring them.
#[must_use]
pub fn included_paths(source_dir: &Path, source_text: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for macro_name in ["include_str!", "include_bytes!"] {
        let mut rest = source_text;
        while let Some((_, tail)) = rest.split_once(macro_name) {
            rest = tail;
            let Some((_, after_quote)) = tail.split_once('"') else {
                break;
            };
            if let Some(target) = after_quote.split('"').next() {
                if !target.is_empty() {
                    out.push(source_dir.join(target));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The transitive crate set for `cargo test -p <root>`: the root plus every
/// `[dependencies]` path edge, reached transitively. A dependency's
/// `[dev-dependencies]` are NOT compiled and are excluded.
#[must_use]
pub fn closure_crates(root: &str, manifests: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    seen.insert(root.to_owned());
    let mut frontier = vec![root.to_owned()];
    while let Some(next) = frontier.pop() {
        let Some(text) = manifests.get(&next) else {
            continue;
        };
        for dep in path_dependencies(text) {
            if seen.insert(dep.clone()) {
                frontier.push(dep);
            }
        }
    }
    seen
}

/// Read a path's committed bytes with `git cat-file`.
///
/// Distinguishes ABSENT (git answered: no such path at HEAD) from UNREADABLE
/// (git could not answer at all). Collapsing those two is the defect this
/// module's negative control exists for: an ad-hoc version read every failure
/// as absence and reported 9 phantom drifts.
fn git_head_read(repo: &Path, path: &Path) -> HeadRead {
    let spec = format!("HEAD:{}", path.display());
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "blob"])
        .arg(&spec)
        .output();
    match output {
        Err(error) => HeadRead::Unreadable(format!("git not invocable: {error}")),
        Ok(done) if done.status.success() => HeadRead::Found(done.stdout),
        Ok(done) => {
            let stderr = String::from_utf8_lossy(&done.stderr).to_lowercase();
            // `git cat-file` says "does not exist" / "exists on disk, but not
            // in" for a genuinely absent path. Anything else — no repository,
            // a corrupt object, a bad HEAD — is an instrument failure.
            if stderr.contains("does not exist") || stderr.contains("not in 'head'") {
                HeadRead::Absent
            } else {
                HeadRead::Unreadable(String::from_utf8_lossy(&done.stderr).trim().to_owned())
            }
        }
    }
}

fn disk_read(repo: &Path, path: &Path) -> DiskRead {
    match std::fs::read(repo.join(path)) {
        Ok(bytes) => DiskRead::Found(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => DiskRead::Absent,
        Err(error) => DiskRead::Unreadable(error.to_string()),
    }
}

/// Every input `cargo test -p <root>` reads, discovered from the manifests.
///
/// Tracked files only, because this half answers "did my COMMITTED inputs
/// drift". The untracked question needs its own enumeration — see
/// [`untracked_in_closure`].
pub fn closure_inputs(repo: &Path, root: &str) -> Result<Vec<PathBuf>, CheckError> {
    let manifest_text = |crate_name: &str| {
        std::fs::read_to_string(repo.join("crates").join(crate_name).join("Cargo.toml")).ok()
    };
    let mut manifests: BTreeMap<String, String> = BTreeMap::new();
    let mut frontier = vec![root.to_owned()];
    while let Some(next) = frontier.pop() {
        if manifests.contains_key(&next) {
            continue;
        }
        let Some(text) = manifest_text(&next) else {
            continue;
        };
        for dep in path_dependencies(&text) {
            frontier.push(dep);
        }
        manifests.insert(next, text);
    }
    let crates = closure_crates(root, &manifests);
    let tracked = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files"])
        .output()
        .map_err(|error| CheckError::TreeUnreadable {
            path: repo.to_path_buf(),
            detail: format!("git ls-files not invocable: {error}"),
        })?;
    if !tracked.status.success() {
        return Err(CheckError::TreeUnreadable {
            path: repo.to_path_buf(),
            detail: String::from_utf8_lossy(&tracked.stderr).trim().to_owned(),
        });
    }
    let mut inputs: BTreeSet<PathBuf> = BTreeSet::new();
    for line in String::from_utf8_lossy(&tracked.stdout).lines() {
        let Some(rest) = line.strip_prefix("crates/") else {
            continue;
        };
        let Some((crate_name, tail)) = rest.split_once('/') else {
            continue;
        };
        if !crates.contains(crate_name) {
            continue;
        }
        let is_input = tail == "Cargo.toml"
            || tail == "build.rs"
            || (tail.ends_with(".rs") && tail.starts_with("src/"))
            // Only the ROOT crate's tests are compiled by `-p <root>`.
            || (crate_name == root && tail.ends_with(".rs") && tail.starts_with("tests/"));
        if is_input {
            inputs.insert(PathBuf::from(line));
        }
    }
    // include_str!/include_bytes! targets: part of the compilation with no
    // `mod` declaring them, so no source walk finds them.
    let sources: Vec<PathBuf> = inputs
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .cloned()
        .collect();
    for source in sources {
        if let Ok(text) = std::fs::read_to_string(repo.join(&source)) {
            let dir = source.parent().unwrap_or(Path::new(".")).to_path_buf();
            for target in included_paths(&dir, &text) {
                inputs.insert(target);
            }
        }
    }
    for shared in ["Cargo.toml", "Cargo.lock", ".cargo/config.toml"] {
        if repo.join(shared).is_file() {
            inputs.insert(PathBuf::from(shared));
        }
    }
    Ok(inputs.into_iter().collect())
}

/// The SECOND enumeration. Hashing against `HEAD` is structurally blind to an
/// input `HEAD` has never seen, because there is nothing on the other side of
/// the comparison.
pub fn untracked_in_closure(repo: &Path, root: &str) -> Result<Vec<PathBuf>, CheckError> {
    let manifests: BTreeMap<String, String> = std::fs::read_dir(repo.join("crates"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            std::fs::read_to_string(entry.path().join("Cargo.toml")).map(|t| (name, t)).ok()
        })
        .collect();
    let crates = closure_crates(root, &manifests);
    let dirs: Vec<String> = crates.iter().map(|c| format!("crates/{c}")).collect();
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .args(&dirs)
        .output()
        .map_err(|error| CheckError::TreeUnreadable {
            path: repo.to_path_buf(),
            detail: format!("git status not invocable: {error}"),
        })?;
    if !output.status.success() {
        return Err(CheckError::TreeUnreadable {
            path: repo.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("?? "))
        .map(PathBuf::from)
        .collect())
}

/// The live check: both enumerations, against a real git tree.
pub fn check_crate(repo: &Path, root: &str) -> Result<Verdict, CheckError> {
    let inputs = closure_inputs(repo, root)?;
    let untracked = untracked_in_closure(repo, root)?;
    assess(
        &inputs,
        &untracked,
        &|path| git_head_read(repo, path),
        &|path| disk_read(repo, path),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn head_map(rows: &[(&str, &[u8])]) -> BTreeMap<PathBuf, Vec<u8>> {
        rows.iter().map(|(k, v)| (p(k), v.to_vec())).collect()
    }

    #[test]
    fn drift_names_the_specific_file_not_merely_that_something_drifted() {
        // LEG 4. A checker that reddens on any dirty file cannot tell you which
        // one you got wrong.
        let inputs = vec![p("a.rs"), p("b.rs"), p("c.rs")];
        let committed = head_map(&[("a.rs", b"same"), ("b.rs", b"old"), ("c.rs", b"same")]);
        let worktree = head_map(&[("a.rs", b"same"), ("b.rs", b"NEW"), ("c.rs", b"same")]);
        let verdict = assess(
            &inputs,
            &[],
            &|path| {
                committed
                    .get(path)
                    .map_or(HeadRead::Absent, |b| HeadRead::Found(b.clone()))
            },
            &|path| {
                worktree
                    .get(path)
                    .map_or(DiskRead::Absent, |b| DiskRead::Found(b.clone()))
            },
        )
        .expect("readable tree");
        assert_eq!(verdict.drift, vec![(p("b.rs"), DriftKind::DiffersFromHead)]);
        assert_eq!(verdict.scanned, 3);
        let line = verdict.render("demo", false);
        assert!(line.contains("b.rs:DIFFERS_FROM_HEAD"), "{line}");
        assert!(!line.contains("a.rs"), "a clean input must not be named: {line}");
    }

    #[test]
    fn a_clean_closure_with_no_untracked_input_licenses_an_absolute_claim() {
        let inputs = vec![p("a.rs")];
        let verdict = assess(
            &inputs,
            &[],
            &|_| HeadRead::Found(b"same".to_vec()),
            &|_| DiskRead::Found(b"same".to_vec()),
        )
        .expect("readable");
        assert!(verdict.is_clean());
        assert_eq!(verdict.tier(false), ClaimTier::Absolute);
        assert!(verdict.tier(false).head_verdict_available());
        assert!(verdict.render("demo", false).contains("head_verdict=true"));
    }

    #[test]
    fn an_untracked_input_denies_the_absolute_claim_even_with_zero_drift() {
        // The stricter definition: "empty drift set" was NOT enough. Hashing
        // against HEAD is structurally blind to an input HEAD has never seen,
        // and `members = ["crates/*"]` + rch's untracked sync makes such an
        // input present for every remote build and absent from every clone.
        let inputs = vec![p("a.rs")];
        let verdict = assess(
            &inputs,
            &[p("crates/ghost/src/lib.rs")],
            &|_| HeadRead::Found(b"same".to_vec()),
            &|_| DiskRead::Found(b"same".to_vec()),
        )
        .expect("readable");
        assert_eq!(verdict.drift, vec![], "no COMMITTED input drifted");
        assert!(!verdict.is_clean(), "and yet the closure is not clean");
        assert_eq!(verdict.tier(true), ClaimTier::DifferentialsOnly);
        assert!(!verdict.tier(true).head_verdict_available());
        let line = verdict.render("demo", true);
        assert!(line.contains("crates/ghost/src/lib.rs"), "{line}");
    }

    #[test]
    fn an_empty_scan_is_an_error_and_never_a_clean_verdict() {
        // LEG 5, anti-vacuity: an empty DIRTY set is a legitimate pass; an
        // empty SCAN is an error, or a checker that enumerated nothing reports
        // the same thing as a spotless repo.
        let error = assess(&[], &[], &|_| HeadRead::Absent, &|_| DiskRead::Absent)
            .expect_err("an empty scan cannot pass");
        assert_eq!(error, CheckError::EmptyScan);
        assert!(error.to_string().contains("EMPTY_SCAN"));
    }

    #[test]
    fn an_unreadable_tree_is_distinguishable_from_a_clean_one() {
        // LEG 6, negative control (rule 8i). This is not theoretical: an
        // ad-hoc version of this check reported `DRIFT=9, all ABSENT_IN_HEAD`
        // when the real cause was a clobbered variable in the caller, and it
        // nearly became a report that the repo had got worse.
        let inputs = vec![p("a.rs")];
        let error = assess(
            &inputs,
            &[],
            &|_| HeadRead::Unreadable("not a git repository".to_owned()),
            &|_| DiskRead::Found(b"x".to_vec()),
        )
        .expect_err("an unreadable tree is not a verdict");
        let CheckError::TreeUnreadable { path, detail } = &error else {
            panic!("expected TREE_UNREADABLE, got {error:?}");
        };
        assert_eq!(path, &p("a.rs"));
        assert!(detail.contains("not a git repository"));
        assert!(error.to_string().contains("TREE_UNREADABLE"));

        let clean = assess(
            &inputs,
            &[],
            &|_| HeadRead::Found(b"x".to_vec()),
            &|_| DiskRead::Found(b"x".to_vec()),
        )
        .expect("readable");
        assert_ne!(
            error.to_string(),
            clean.render("demo", false),
            "ABSENT and CLEAN must not look identical"
        );
    }

    #[test]
    fn absent_in_head_and_differs_are_separate_kinds_with_separate_codes() {
        // A committed caller with an uncommitted callee is ABSENT_IN_HEAD, and
        // it is repaired by committing the callee, not by reverting a file.
        // One bucket would send both repairs to the wrong place.
        let inputs = vec![p("committed.rs"), p("never-committed.rs")];
        let verdict = assess(
            &inputs,
            &[],
            &|path| {
                if path == p("committed.rs").as_path() {
                    HeadRead::Found(b"old".to_vec())
                } else {
                    HeadRead::Absent
                }
            },
            &|_| DiskRead::Found(b"new".to_vec()),
        )
        .expect("readable");
        assert_eq!(
            verdict.drift,
            vec![
                (p("committed.rs"), DriftKind::DiffersFromHead),
                (p("never-committed.rs"), DriftKind::AbsentInHead),
            ]
        );
        assert_ne!(
            DriftKind::DiffersFromHead.code(),
            DriftKind::AbsentInHead.code()
        );
        assert_ne!(DriftKind::AbsentOnDisk.code(), DriftKind::AbsentInHead.code());
    }

    #[test]
    fn a_staged_deletion_is_absent_on_disk_not_a_clean_input() {
        let inputs = vec![p("deleted.rs")];
        let verdict = assess(
            &inputs,
            &[],
            &|_| HeadRead::Found(b"content".to_vec()),
            &|_| DiskRead::Absent,
        )
        .expect("readable");
        assert_eq!(verdict.drift, vec![(p("deleted.rs"), DriftKind::AbsentOnDisk)]);
    }

    #[test]
    fn dev_dependencies_of_a_dependency_are_not_in_the_compilation_unit() {
        // The polarity a grep cannot settle, measured on this repo's real
        // shape: `kernel-only-operator-hook` depends on `lifecycle-event`,
        // which depends on `omp-types` under [dependencies] -- so omp-types IS
        // in the unit. `text-structure` enters only as `subprocess-contract`'s
        // [dev-dependencies], so it is NOT.
        let manifests: BTreeMap<String, String> = [
            (
                "root".to_owned(),
                "[dependencies]\nlifecycle-event = { path = \"../lifecycle-event\" }\nsubprocess-contract = { path = \"../subprocess-contract\" }\n".to_owned(),
            ),
            (
                "lifecycle-event".to_owned(),
                "[dependencies]\nomp-types = { path = \"../omp-types\" }\n".to_owned(),
            ),
            (
                "subprocess-contract".to_owned(),
                "[dev-dependencies]\ntext-structure = { path = \"../text-structure\" }\n".to_owned(),
            ),
            ("omp-types".to_owned(), String::new()),
            ("text-structure".to_owned(), String::new()),
        ]
        .into_iter()
        .collect();
        let closure = closure_crates("root", &manifests);
        assert!(closure.contains("omp-types"), "transitive [dependencies] edge: {closure:?}");
        assert!(
            !closure.contains("text-structure"),
            "a dependency's [dev-dependencies] are not compiled: {closure:?}"
        );
        assert_eq!(closure.len(), 4, "{closure:?}");
    }

    #[test]
    fn build_dependencies_are_in_the_unit_because_a_build_script_is_compiled_and_run() {
        let manifests: BTreeMap<String, String> = [
            (
                "root".to_owned(),
                "[build-dependencies]\ngen = { path = \"../gen\" }\n".to_owned(),
            ),
            ("gen".to_owned(), String::new()),
        ]
        .into_iter()
        .collect();
        assert!(closure_crates("root", &manifests).contains("gen"));
    }

    #[test]
    fn include_str_targets_are_inputs_with_no_mod_declaring_them() {
        // The most invisible member after build.rs: data that is part of the
        // compilation and appears in no module tree.
        let found = included_paths(
            Path::new("crates/demo/src"),
            "const A: &str = include_str!(\"../fixtures/a.txt\");\n\
             const B: &[u8] = include_bytes!(\"b.bin\");\n",
        );
        assert_eq!(
            found,
            vec![
                PathBuf::from("crates/demo/src/../fixtures/a.txt"),
                PathBuf::from("crates/demo/src/b.bin"),
            ]
        );
        assert!(included_paths(Path::new("x"), "no macros here").is_empty());
    }

    #[test]
    fn the_tier_is_the_output_a_reader_actually_needs() {
        let dirty = Verdict {
            scanned: 3,
            drift: vec![(p("x.rs"), DriftKind::DiffersFromHead)],
            untracked: Vec::new(),
        };
        // Same measurement, two tiers, decided by whether the arms were bracketed.
        assert_eq!(dirty.tier(true), ClaimTier::DifferentialsOnly);
        assert_eq!(dirty.tier(false), ClaimTier::ResolutionStabilityOnly);
        assert!(!dirty.tier(true).head_verdict_available());
        assert_ne!(
            dirty.render("demo", true),
            dirty.render("demo", false),
            "the tier must be visible in the output, not inferred by the reader"
        );
    }

    /// LEG 4 OF rluzf, RUN AGAINST THE REAL TREE: the check must ANSWER or
    /// REFUSE, never guess.
    ///
    /// `kernel-only-operator-hook` is the subject because it is the crate the
    /// `rluzf` revert moved to the top rung, so its closure is the one whose
    /// answer is known: 26 tracked inputs across 6 crates, drift 0, untracked
    /// 0. Dirtying any one of them must produce `INPUT_CLOSURE_DRIFT` NAMING
    /// THAT PATH — a checker that reddens without naming which input drifted
    /// cannot tell you which one you got wrong.
    ///
    /// An unreadable tree (a build host with no `.git`, which is the normal
    /// case on a Contabo worker) is UNMEASURABLE and is reported as such. It is
    /// NOT a pass, and it is NOT drift: that is the whole reason `HeadRead`
    /// carries three variants instead of two.
    #[test]
    fn the_live_check_answers_or_refuses_but_never_guesses() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root is two levels above the crate manifest");
        match check_crate(repo, "kernel-only-operator-hook") {
            Ok(verdict) => {
                assert!(
                    verdict.scanned > 0,
                    "a real crate's closure is never empty: {verdict:?}"
                );
                println!(
                    "{}",
                    verdict.render("kernel-only-operator-hook", false)
                );
                // The tier is derived, never asserted as a constant: this leg
                // must stay honest whether or not the repo is clean today.
                assert_eq!(
                    verdict.tier(false).head_verdict_available(),
                    verdict.is_clean(),
                    "a HEAD verdict is available exactly when the closure is clean"
                );
            }
            Err(CheckError::TreeUnreadable { path, detail }) => {
                println!(
                    "INPUT_CLOSURE_UNMEASURABLE path={} detail={detail}",
                    path.display()
                );
            }
            Err(CheckError::EmptyScan) => {
                panic!("the closure of a real crate is never empty; this is an instrument failure")
            }
        }
    }

    /// LEG 4, THROUGH THE LIVE GIT PATH, IN A HERMETIC FIXTURE REPO.
    ///
    /// The real-tree leg above cannot do this on a build host: `rch` syncs the
    /// worktree but the worker's `HEAD` is not a valid object
    /// (`fatal: invalid object name 'HEAD'`), so `check_crate` there correctly
    /// reports UNMEASURABLE. That is the negative control firing in
    /// production — and it is also, measured by this crate's own tool, why
    /// "does HEAD compile" is not answerable from a worker.
    ///
    /// So the known-bad runs against a repo this test builds and commits
    /// itself: dirty ONE input, require `DriftKind::DiffersFromHead` NAMING
    /// that path, restore it, require clean.
    #[test]
    fn dirtying_one_input_names_that_input_and_restoring_it_passes() {
        let temp = tempfile::tempdir().expect("fixture repo");
        let repo = temp.path();
        let git = |args: &[&str]| {
            let done = std::process::Command::new("git")
                .arg("-C")
                .arg(repo)
                .args([
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "user.name=fixture",
                ])
                .args(args)
                .output()
                .expect("git invocable");
            assert!(
                done.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&done.stderr)
            );
        };
        let write = |rel: &str, body: &str| {
            let path = repo.join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, body).expect("write fixture file");
        };

        git(&["init", "--quiet"]);
        write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
        write(
            "crates/leaf/Cargo.toml",
            "[package]\nname = \"leaf\"\n[dependencies]\n",
        );
        write("crates/leaf/src/lib.rs", "pub fn leaf() {}\n");
        write(
            "crates/subject/Cargo.toml",
            "[package]\nname = \"subject\"\n[dependencies]\nleaf = { path = \"../leaf\" }\n",
        );
        write("crates/subject/src/lib.rs", "pub fn subject() {}\n");
        git(&["add", "-A"]);
        git(&["commit", "--quiet", "-m", "fixture"]);

        let clean = check_crate(repo, "subject").expect("a committed fixture is readable");
        assert!(
            clean.is_clean(),
            "a freshly committed tree must be clean: {}",
            clean.render("subject", false)
        );
        assert_eq!(clean.tier(false), ClaimTier::Absolute);
        // The transitive [dependencies] edge is IN the scan, so dirtying the
        // dependency is what the next assertion exercises.
        assert!(clean.scanned >= 5, "scan was {}", clean.scanned);

        // KNOWN-BAD: dirty the DEPENDENCY, not the subject, because the whole
        // point is that a crate's verdict depends on files outside it.
        std::fs::write(repo.join("crates/leaf/src/lib.rs"), "pub fn leaf() { /* x */ }\n")
            .expect("dirty the dependency");
        let dirty = check_crate(repo, "subject").expect("still readable");
        assert_eq!(
            dirty.drift,
            vec![(
                PathBuf::from("crates/leaf/src/lib.rs"),
                DriftKind::DiffersFromHead
            )],
            "the drift must NAME the file: {}",
            dirty.render("subject", false)
        );
        assert_eq!(dirty.tier(false), ClaimTier::ResolutionStabilityOnly);
        assert_eq!(dirty.tier(true), ClaimTier::DifferentialsOnly);
        assert!(!dirty.tier(true).head_verdict_available());

        // UNTRACKED SECOND ENUMERATION, on a tree with ZERO committed drift:
        // an input HEAD has never seen. `git status` is the only instrument
        // that can see it; hashing against HEAD structurally cannot.
        std::fs::write(repo.join("crates/leaf/src/lib.rs"), "pub fn leaf() {}\n")
            .expect("restore the dependency");
        let restored = check_crate(repo, "subject").expect("readable");
        assert!(
            restored.is_clean(),
            "restoring must pass: {}",
            restored.render("subject", false)
        );
        write("crates/leaf/src/ghost.rs", "pub fn ghost() {}\n");
        let ghosted = check_crate(repo, "subject").expect("readable");
        assert!(ghosted.drift.is_empty(), "no COMMITTED input drifted");
        assert_eq!(
            ghosted.untracked,
            vec![PathBuf::from("crates/leaf/src/ghost.rs")]
        );
        assert!(
            !ghosted.is_clean(),
            "zero drift is NOT a clean closure while an untracked input exists: {}",
            ghosted.render("subject", false)
        );
    }
}
