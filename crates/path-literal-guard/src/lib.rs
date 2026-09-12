//! The home-path-literal gate for this repository (beads omp-orchestrator-npq, -oej2).
//!
//! WHAT THIS MECHANICALLY ENFORCES — the floor, and no more: a `.rs` file under
//! `<repo>/crates/*/src/` must contain zero occurrences of the current user's home-path
//! literal. A reintroduced literal — a new hardcoded checkout, a pasted fixture, a
//! `HOME` fallback — turns the gate RED naming the file and line.
//!
//! WHY THIS GATE EXISTS: a hardcoded repository root COMPILES fine after a move and
//! then silently reads the WRONG repository — the failure is a wrong answer, not an
//! error. -7ai killed the literals in the three ported crates; this gate is -7ai
//! acceptance #1 ("zero occurrences ... anywhere in crates/*/src") enforced where it
//! actually matters: on the tree the extraction copies INTO.
//!
//! # TWO NAMED MODES, and why both must exist (omp-orchestrator-oej2)
//!
//! This gate used to have exactly one scope — the whole working tree — while the
//! pre-commit wrapper that calls it keys on the STAGED set. Measured 2026-09-02:
//!
//! ```text
//! git diff --cached --name-only                -> AGENTS.md      (one file, a DOC)
//! git status --porcelain -- crates/agent-mail-native/src/lib.rs
//!                                              -> ?? ...         (UNTRACKED)
//! .git/hooks/pre-commit -> MULTI-GATE REFUSED: path-literal-guard: 3 hardcoded
//!                          home-path literal(s): crates/agent-mail-native/src/lib.rs:80
//! ```
//!
//! A commit staging one Markdown file was refused by three literals in another agent's
//! untracked scratch. On a five-agent shared checkout that makes the repository
//! effectively single-writer whenever anybody holds uncommitted crate work, and it was
//! the root cause of a whole session's commit-block cascade: agents held clean work for
//! over an hour blocked by crates they had never touched, one agent unstaged verified
//! files to protect them (emptying the index and manufacturing a vacuous green), and
//! another swept 31 files across three agents' crates because polling a repo-wide gate
//! was the only way to learn "may I commit".
//!
//! So the scope is now explicit and named, never implied:
//!
//! * [`ScanMode::StagedPaths`] — [`scan_paths`]. What the hook uses. A literal in an
//!   unstaged or untracked file CANNOT refuse an unrelated commit.
//! * [`ScanMode::RepoWide`] — [`scan`]. What CI, a full audit, and the arrival gate
//!   use. Deleting this mode would trade one blind spot for another, so it stays.
//!
//! Both modes share ONE eligibility predicate, [`is_in_scan_scope`], and one line
//! scanner. Scoping narrows WHICH files are read; it does not weaken WHAT is looked
//! for. `staged_mode_over_the_whole_tree_equals_repo_wide` is the standing proof.
//!
//! # THREE OUTCOMES, not two (with omp-orchestrator-calr)
//!
//! [`Verdict`] distinguishes CLEAN from NOTHING-TO-CHECK. Staging only `AGENTS.md`
//! leaves this gate zero eligible files: that is *nothing to check*, and reporting it
//! as *clean* is the vacuous-green inversion. In repo-wide mode an empty scan set is
//! instead a [`Verdict::VacuousError`] — a repo whose crates tree vanished, or a gate
//! pointed at the wrong root, must fail loudly.
//!
//! WHAT STILL PASSES — do not read this gate as more than it is:
//! * NON-`.rs` FILES. Cargo.toml, fixtures, cron/registry TOML are invisible here
//!   (npq acceptance #7 names those out of scope and unmeasured).
//! * NON-SRC `.rs`. `tests/`, `benches/`, `build.rs` are outside the scan set by the
//!   bead's own wording (`crates/*/src`).
//! * OTHER MACHINES' HOMES. The needle is this fleet's home literal; a different
//!   user's path elsewhere is not caught until it runs on this fleet.
//! * IN REPO-WIDE MODE THE SCAN IS THE WORKING TREE, untracked files included. In
//!   staged mode it is the staged set and nothing else — a green there says nothing
//!   about the rest of the repo, which is what [`declared_scope_line`] prints.
//!
//! # The self-reference, handled without a carve-out
//!
//! The needle is built by `concat!`, so this gate's own source never contains the
//! contiguous literal it forbids, and the inline test module below CONSTRUCTS the
//! literal at runtime with `format!` rather than spelling it. `crates/path-literal-guard/src`
//! IS scanned by both modes and yields zero hits;
//! `the_guards_own_source_is_clean_without_an_exclusion` is the standing proof.
//! Five sibling crates use the same split-needle idiom. Nothing is suppressed.
//!
//! INCONSISTENCY WITH `state-wildcard-lint`, stated rather than papered over: that gate
//! prunes `tests/` because a fires-on-known-bad fixture MUST contain the pattern it
//! checks. This gate does NOT prune an inline `#[cfg(test)]` module inside `src/`, and
//! should not: a hardcoded home path in a test still breaks on another machine, so
//! pruning it would weaken the check. The two gates differ deliberately, for reasons
//! that do not transfer.

#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// The home-path literal this gate forbids.
pub const USER_HOME_LITERAL: &str = concat!("/Users/", "josh");

/// Host-local selector binary. Split so this source never contains the contiguous
/// path that would itself trip the gate (09.10 / bcrn.4).
pub const HOST_BV_LITERAL: &str = concat!("/opt/homebrew/bin/", "bv");

/// Every contiguous literal a `crates/*/src` file must not spell.
pub const FORBIDDEN_LITERALS: &[&str] = &[USER_HOME_LITERAL, HOST_BV_LITERAL];

/// Which files a scan covered. Named, because a verdict whose scope is implied is a
/// verdict a reader cannot check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    /// Every `.rs` file under `<root>/crates/*/src` — CI, full audits, arrival checks.
    RepoWide,
    /// Only the paths handed in, filtered by [`is_in_scan_scope`] — the pre-commit hook.
    StagedPaths,
}

impl fmt::Display for ScanMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RepoWide => formatter.write_str("repo-wide"),
            Self::StagedPaths => formatter.write_str("staged"),
        }
    }
}

/// One forbidden-literal occurrence, named by file and line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Path of the offending file, relative to the scan root when possible.
    pub file: PathBuf,
    /// 1-indexed line number of the occurrence.
    pub line: usize,
}

impl fmt::Display for Hit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.file.display(), self.line)
    }
}

/// Why a handed-in path was not read. Recorded so a green never silently implies a
/// path was covered when it was not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Not a `.rs` file under `crates/*/src` — outside this gate's floor.
    OutOfScope,
    /// In scope but absent from the worktree: a staged deletion or rename source.
    Absent,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfScope => write!(formatter, "outside {}", scan_scope_description()),
            Self::Absent => formatter.write_str("absent from the worktree (staged deletion)"),
        }
    }
}

/// A handed-in path this gate did not read, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub file: PathBuf,
    pub reason: SkipReason,
}

/// One DECLARED suppression. Never inferred.
///
/// Keyed by file plus a substring of the offending line, never by line number, so a row
/// does not silently re-target when the file shifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllowRow {
    /// Repo-relative path, as a report names it.
    pub file: &'static str,
    /// A substring the offending line must contain for this row to apply.
    pub line_contains: &'static str,
    pub reason: &'static str,
}

/// The complete suppression set.
///
/// EMPTY BY DESIGN, and measured so as of 2026-09-02: no `.rs` file under
/// `crates/*/src` carries the contiguous literal, including this crate's own source and
/// the five siblings that reason about the needle. They all build it with `concat!`,
/// which is a MECHANISM rather than an exclusion — see the module docs. A row asserting
/// a suppression that does not happen would be stale, and [`apply_allowlist`] reports
/// stale rows so a carve-out cannot rot into a standing one.
pub const DECLARED_ALLOWLIST: &[AllowRow] = &[];

/// A hit a DECLARED row suppressed, carried so the suppression is visible rather than
/// subtracted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedHit {
    pub hit: Hit,
    pub reason: &'static str,
}

/// Partition hits against a suppression set.
///
/// Returns `(kept, allowed, stale)`. `stale` names rows that matched nothing.
///
/// STALE IS ONLY MEANINGFUL IN REPO-WIDE MODE. In staged mode a row legitimately matches
/// nothing whenever its file is not part of this commit, so [`scan_paths`] never treats
/// a stale row as an error — otherwise every scoped run would refuse for a row about
/// somebody else's file, which is the very defect this bead exists to remove.
pub fn apply_allowlist(
    hits: Vec<Hit>,
    lines: &[String],
    allowlist: &[AllowRow],
) -> (Vec<Hit>, Vec<AllowedHit>, Vec<AllowRow>) {
    let mut used = vec![false; allowlist.len()];
    let mut kept = Vec::new();
    let mut allowed = Vec::new();
    for (index, hit) in hits.into_iter().enumerate() {
        let text = lines.get(index).map(String::as_str).unwrap_or("");
        let matched = allowlist.iter().position(|row| {
            Path::new(row.file) == hit.file.as_path() && text.contains(row.line_contains)
        });
        match matched {
            Some(row) => {
                used[row] = true;
                allowed.push(AllowedHit {
                    hit,
                    reason: allowlist[row].reason,
                });
            }
            None => kept.push(hit),
        }
    }
    let stale = allowlist
        .iter()
        .zip(used.iter())
        .filter(|(_, hit)| !**hit)
        .map(|(row, _)| *row)
        .collect();
    (kept, allowed, stale)
}

/// The gate's verdict. THREE outcomes, not two: a scan that covered nothing is not a
/// scan that found nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Non-empty scan set, zero surviving hits.
    Clean,
    /// At least one surviving hit.
    Violation,
    /// Staged mode with zero eligible paths: this gate has no opinion on this commit.
    NothingToCheck,
    /// Repo-wide mode with an empty scan set, or a stale DECLARED row: an ERROR.
    VacuousError,
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => formatter.write_str("CLEAN"),
            Self::Violation => formatter.write_str("VIOLATION"),
            Self::NothingToCheck => formatter.write_str("NOTHING_TO_CHECK"),
            Self::VacuousError => formatter.write_str("VACUOUS_ERROR"),
        }
    }
}

/// The result of one scan, in one named mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanReport {
    /// Which files this scan covered.
    pub mode: ScanMode,
    /// Every `.rs` file actually read, in sorted order — printed with the verdict so a
    /// reader can see the scan set and never mistake a vacuous pass for coverage.
    pub scanned: Vec<PathBuf>,
    /// Handed-in paths that were not read, and why. Always empty in repo-wide mode.
    pub skipped: Vec<Skipped>,
    /// Every surviving forbidden-literal occurrence, named by file and line.
    pub hits: Vec<Hit>,
    /// Hits a DECLARED row suppressed, with the reason.
    pub allowed: Vec<AllowedHit>,
    /// DECLARED rows that matched nothing. Only an error in repo-wide mode.
    pub stale_allowlist: Vec<AllowRow>,
}

impl ScanReport {
    /// The gate's verdict.
    pub fn verdict(&self) -> Verdict {
        if !self.hits.is_empty() {
            return Verdict::Violation;
        }
        match self.mode {
            ScanMode::RepoWide => {
                if self.scanned.is_empty() || !self.stale_allowlist.is_empty() {
                    Verdict::VacuousError
                } else {
                    Verdict::Clean
                }
            }
            ScanMode::StagedPaths => {
                if self.scanned.is_empty() {
                    Verdict::NothingToCheck
                } else {
                    Verdict::Clean
                }
            }
        }
    }

    /// Green only when files were actually covered AND zero hits survived.
    ///
    /// NOTHING-TO-CHECK is deliberately NOT a pass here: callers must branch on
    /// [`ScanReport::verdict`] so they cannot report a vacuous green.
    pub fn is_pass(&self) -> bool {
        self.verdict() == Verdict::Clean
    }

    /// One line stating exactly what this verdict covers. A gate whose scope is
    /// invisible cannot be argued with, and a scoped green that reads like a repo-wide
    /// one is the next overclaim.
    pub fn declared_scope_line(&self) -> String {
        let common = format!(
            "the needle is built by concat! so this gate's own source never contains the \
             contiguous literal; DECLARED allowlist rows: {}",
            DECLARED_ALLOWLIST.len()
        );
        match self.mode {
            ScanMode::RepoWide => format!(
                "DECLARED SCOPE repo-wide: every .rs under {}, {} file(s) read, \
                 untracked included; DEFERRED from the sweep: {:?} (in the gate's floor, not \
                 walked here). An empty scan set is an ERROR, not a pass. {common}.",
                repo_wide_scope_description(),
                self.scanned.len(),
                repo_wide_deferred_subdirs()
            ),
            ScanMode::StagedPaths => {
                let out = self
                    .skipped
                    .iter()
                    .filter(|s| s.reason == SkipReason::OutOfScope)
                    .count();
                let absent = self
                    .skipped
                    .iter()
                    .filter(|s| s.reason == SkipReason::Absent)
                    .count();
                format!(
                    "DECLARED SCOPE staged set only: {} staged path(s) read, {} outside \
                     {}, {} absent from the worktree. A literal in an \
                     UNSTAGED or UNTRACKED file is not this commit's problem and cannot \
                     refuse it; only `cargo test -p path-literal-guard` (repo-wide mode) \
                     claims the repository is clean. {common}.",
                    self.scanned.len(),
                    out,
                    scan_scope_description(),
                    absent
                )
            }
        }
    }
}

/// This repository's root, derived from this crate's manifest (`<repo>/crates/<crate>`,
/// two `ancestors` up) — never a constant, so the gate moves with the repo.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the repository root")
        .to_path_buf()
}

/// THE ONE ELIGIBILITY PREDICATE, shared by both modes.
///
/// True for a repo-relative path that is a `.rs` file under one of [`SCANNED_CRATE_SUBDIRS`].
/// Both modes route through this, so a scoped run and a sweep can never disagree about
/// WHAT is in scope — only about which subset was read.
/// The per-crate subdirectories this gate scans, and the SINGLE source of the printed scope.
///
/// `omp-orchestrator-k0h1e`. `src` alone was the scope, and `%20` proved the gap by staging a real
/// home-path literal under `tests/` and watching it LAND (`25a7e5b`, reverted `fd9f6f3`): both
/// gates printed `GATE_NOT_APPLICABLE` and said so honestly, so the hole was declared rather than
/// hidden — and a declared hole still ships the literal.
///
/// ITEM 5 IS NOT PRESERVATION, IT IS THE WORK. The scope was PRINTED but HAND-TYPED at five
/// message sites across two crates. That is the count-in-prose defect wearing a different noun: a
/// scope in prose is wrong the moment the predicate moves, and widening `src` to `{src,tests}`
/// would have left four sites claiming otherwise. Every runtime message now derives from this
/// const, so the predicate and its advertisement cannot disagree.
pub const SCANNED_CRATE_SUBDIRS: &[&str] = &["src", "tests"];

/// The printed scope, derived. `crates/*/{src,tests}/**.rs` today; whatever the const says after.
pub fn scan_scope_description() -> String {
    if SCANNED_CRATE_SUBDIRS.len() == 1 {
        return format!("crates/*/{}/**.rs", SCANNED_CRATE_SUBDIRS[0]);
    }
    format!("crates/*/{{{}}}/**.rs", SCANNED_CRATE_SUBDIRS.join(","))
}

/// The subdirectories the REPO-WIDE sweep actually walks, and a DECLARED subset of the
/// staged floor rather than a second opinion about it.
///
/// `omp-orchestrator-k0h1e` widened the GATE (staged mode) to `{src,tests}` and left the
/// sweep on `src` because eight `crates/*/tests` files then carried literals. CI run
/// 34667870386 (`c6ebdd9b`) and a worktree re-run (worker=contabo-3) both classified
/// `crates/*/tests` CLEAN -- `the_repo_wide_narrowing_is_load_bearing` reddened and said
/// WIDEN. The deferral expired as designed. `omp-orchestrator-64wxc`.
pub const REPO_WIDE_SUBDIRS: &[&str] = &["src", "tests"];

/// The printed repo-wide scope, derived from the floor the walker uses — never from the staged
/// one. Advertising `{src,tests}` while reading only `src` is an advertisement not pinned to its
/// predicate, which is the exact defect this unit exists to delete.
#[must_use]
pub fn repo_wide_scope_description() -> String {
    if REPO_WIDE_SUBDIRS.len() == 1 {
        return format!("crates/*/{}/**.rs", REPO_WIDE_SUBDIRS[0]);
    }
    format!("crates/*/{{{}}}/**.rs", REPO_WIDE_SUBDIRS.join(","))
}

/// The subdirs in the gate's floor that the sweep defers, derived from both consts.
#[must_use]
pub fn repo_wide_deferred_subdirs() -> Vec<&'static str> {
    SCANNED_CRATE_SUBDIRS
        .iter()
        .filter(|subdir| !REPO_WIDE_SUBDIRS.contains(*subdir))
        .copied()
        .collect()
}

pub fn is_in_scan_scope(relative: &Path) -> bool {
    if !relative
        .extension()
        .is_some_and(|extension| extension == "rs")
    {
        return false;
    }
    let parts: Vec<&std::ffi::OsStr> = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .collect();
    // crates / <crate> / <subdir> / ... / <file>.rs
    parts.len() >= 4
        && parts[0] == "crates"
        && SCANNED_CRATE_SUBDIRS
            .iter()
            .any(|subdir| parts[2] == std::ffi::OsStr::new(subdir))
}

/// Read one file and collect hits, returning them with the offending line texts.
///
/// An unreadable file is a panic, not a skip: a scan that silently skipped a file
/// reports identically to one that covered it (anti-vacuity, C88).
fn scan_one(root: &Path, path: &Path, hits: &mut Vec<Hit>, lines: &mut Vec<String>) {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    for (index, line) in text.lines().enumerate() {
        if FORBIDDEN_LITERALS.iter().any(|needle| line.contains(needle)) {
            hits.push(Hit {
                file: path.strip_prefix(root).unwrap_or(path).to_path_buf(),
                line: index + 1,
            });
            lines.push(line.to_owned());
        }
    }
}

fn finish(
    mode: ScanMode,
    mut scanned: Vec<PathBuf>,
    skipped: Vec<Skipped>,
    hits: Vec<Hit>,
    lines: Vec<String>,
) -> ScanReport {
    scanned.sort();
    let (hits, allowed, stale) = apply_allowlist(hits, &lines, DECLARED_ALLOWLIST);
    // A row that matches nothing is only evidence of rot when the sweep was total.
    let stale_allowlist = match mode {
        ScanMode::RepoWide => stale,
        ScanMode::StagedPaths => Vec::new(),
    };
    ScanReport {
        mode,
        scanned,
        skipped,
        hits,
        allowed,
        stale_allowlist,
    }
}

/// REPO-WIDE mode: scan `<root>/crates/*/src/` recursively for `.rs` files carrying the
/// home literal. This is the sweep — CI, full audits, arrival checks.
///
/// An unreadable directory is a panic, not a skip (anti-vacuity, C88). A MISSING crates
/// directory yields the empty scan set, which is [`Verdict::VacuousError`].
pub fn scan(root: &Path) -> ScanReport {
    let crates_dir = root.join("crates");
    let mut scanned = Vec::new();
    let mut hits = Vec::new();
    let mut lines = Vec::new();
    let mut stack = Vec::new();

    let entries = match fs::read_dir(&crates_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return finish(ScanMode::RepoWide, scanned, Vec::new(), hits, lines)
        }
        Err(error) => panic!("cannot read {}: {error}", crates_dir.display()),
    };
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|error| panic!("cannot enumerate {}: {error}", crates_dir.display()));
        for subdir in REPO_WIDE_SUBDIRS {
            let directory = entry.path().join(subdir);
            if directory.is_dir() {
                stack.push(directory);
            }
        }
    }

    while let Some(directory) = stack.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!("cannot enumerate {}: {error}", directory.display())
            });
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !path.extension().is_some_and(|extension| extension == "rs") {
                continue;
            }
            scanned.push(path.clone());
            scan_one(root, &path, &mut hits, &mut lines);
        }
    }

    finish(ScanMode::RepoWide, scanned, Vec::new(), hits, lines)
}

/// STAGED mode: scan only the handed-in paths, filtered by [`is_in_scan_scope`].
///
/// This is what the pre-commit hook calls. A literal in an unstaged or untracked file is
/// invisible here BY DESIGN — see the module docs for the measurement that made this
/// necessary. Paths outside the scope, and in-scope paths absent from the worktree
/// (staged deletions), are recorded in [`ScanReport::skipped`] rather than dropped, so
/// the verdict can state what it did not cover.
pub fn scan_paths<P: AsRef<Path>>(root: &Path, paths: &[P]) -> ScanReport {
    let mut scanned = Vec::new();
    let mut skipped = Vec::new();
    let mut hits = Vec::new();
    let mut lines = Vec::new();

    for path in paths {
        let given = path.as_ref();
        let relative = given.strip_prefix(root).unwrap_or(given);
        if !is_in_scan_scope(relative) {
            skipped.push(Skipped {
                file: relative.to_path_buf(),
                reason: SkipReason::OutOfScope,
            });
            continue;
        }
        let absolute = if given.is_absolute() {
            given.to_path_buf()
        } else {
            root.join(relative)
        };
        if !absolute.is_file() {
            skipped.push(Skipped {
                file: relative.to_path_buf(),
                reason: SkipReason::Absent,
            });
            continue;
        }
        scanned.push(absolute.clone());
        scan_one(root, &absolute, &mut hits, &mut lines);
    }

    finish(ScanMode::StagedPaths, scanned, skipped, hits, lines)
}

/// Scan STAGED BLOBS: the bytes the commit is made of, supplied by the caller.
///
/// THE DEFECT THIS CLOSES (`omp-orchestrator-249hz`, seventh instance, and GATE 3's copy
/// of the one GATE 4 closed at `4bf0d0a`). [`scan_paths`] selects the STAGED SET and then
/// reads each file from the WORKTREE. In a twelve-agent shared checkout the two trees
/// diverge constantly and both directions are real: a home-path literal that IS staged
/// but already repaired in the worktree passes the gate and LANDS -- the false green, and
/// it needs nothing exotic, only `git add`, keep fixing, then a pathless `git commit` --
/// while one present only in the unstaged worktree refuses a commit that does not contain
/// it.
///
/// The caller supplies `(repo-relative name, source)` read with `git show :<path>`, so
/// this function touches no filesystem and cannot read the wrong tree. Scope is still
/// decided HERE by [`is_in_scan_scope`], so the staged and repo-wide modes cannot drift
/// apart on which files count; `staged_mode_over_the_whole_tree_equals_repo_wide` remains
/// the standing proof.
///
/// AN OUT-OF-SCOPE PATH IS STILL RECORDED as `SkipReason::OutOfScope`, because a green
/// that silently implies coverage is this gate's other failure mode. There is no `Absent`
/// arm here: absence from the INDEX is a staged deletion, which the caller detects before
/// it ever has bytes to hand over.
///
/// [`scan_paths`] survives for `path-literal-guard --staged <paths>` and the repo-wide
/// sweep, where the WORKTREE is the subject the operator asked about. The commit path is
/// not that caller.
pub fn scan_sources<N: AsRef<str>, S: AsRef<str>>(sources: &[(N, S)]) -> ScanReport {
    let mut scanned = Vec::new();
    let mut skipped = Vec::new();
    let mut hits = Vec::new();
    let mut lines = Vec::new();

    for (name, source) in sources {
        let relative = Path::new(name.as_ref());
        if !is_in_scan_scope(relative) {
            skipped.push(Skipped {
                file: relative.to_path_buf(),
                reason: SkipReason::OutOfScope,
            });
            continue;
        }
        scanned.push(relative.to_path_buf());
        for (index, line) in source.as_ref().lines().enumerate() {
            if FORBIDDEN_LITERALS.iter().any(|needle| line.contains(needle)) {
                hits.push(Hit {
                    file: relative.to_path_buf(),
                    line: index + 1,
                });
                lines.push(line.to_owned());
            }
        }
    }

    finish(ScanMode::StagedPaths, scanned, skipped, hits, lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The literal is CONSTRUCTED, never spelled: this module is scanned by both modes
    /// and must not trip the gate it tests.
    fn planted_line() -> String {
        format!("const REPO: &str = \"{USER_HOME_LITERAL}\";\n")
    }

    fn planted_host_bv_line() -> String {
        format!("const BV: &str = \"{HOST_BV_LITERAL}\";\n")
    }

    /// 09.10: hard-coding the host selector path in crates/*/src is a Violation.
    #[test]
    fn hardcoding_host_bv_path_is_a_violation() {
        let root = std::env::temp_dir().join(format!("plg-host-bv-{}", std::process::id()));
        let src = root.join("crates/loop-queue-filter/src");
        fs::create_dir_all(&src).expect("create fixture tree");
        fs::write(src.join("selector.rs"), planted_host_bv_line()).expect("plant host bv path");
        let report = scan(&root);
        assert_eq!(report.verdict(), Verdict::Violation, "{report:?}");
        assert_eq!(report.hits.len(), 1, "{:?}", report.hits);
        let _ = fs::remove_dir_all(&root);
    }

    /// The scanner finds a planted literal in a fake `crates/*/src` tree.
    #[test]
    fn scanner_names_file_and_line_of_a_planted_literal() {
        let root = std::env::temp_dir().join(format!("plg-selftest-{}", std::process::id()));
        let src = root.join("crates/example/src");
        fs::create_dir_all(&src).expect("create fixture tree");
        fs::write(src.join("lib.rs"), "fn main() {}\n").expect("write clean file");
        let dirty = src.join("dirty.rs");
        fs::write(&dirty, "const X: &str = \"placeholder\";\n").expect("write dirty file");
        // Plant the literal the way a real reintroduction would: contiguous in the text.
        fs::write(&dirty, planted_line()).expect("write planted file");

        let report = scan(&root);
        assert_eq!(report.mode, ScanMode::RepoWide);
        assert_eq!(report.scanned.len(), 2, "scan set: {:?}", report.scanned);
        assert_eq!(
            report.hits.len(),
            1,
            "planted literal must be caught: {:?}",
            report.hits
        );
        assert_eq!(report.verdict(), Verdict::Violation);
        let hit = &report.hits[0];
        assert!(
            hit.file.ends_with("dirty.rs"),
            "wrong file: {}",
            hit.file.display()
        );
        assert_eq!(hit.line, 1, "wrong line: {hit:?}");

        let _ = fs::remove_dir_all(&root);
    }

    /// The gate is not vacuous: a root with no `crates/*/src` trees cannot pass.
    #[test]
    fn empty_scan_set_is_not_a_pass() {
        let root = std::env::temp_dir().join(format!("plg-empty-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create empty root");
        let report = scan(&root);
        assert!(report.scanned.is_empty(), "expected an empty scan set");
        assert!(
            !report.is_pass(),
            "an empty scan set is an ERROR, never a pass"
        );
        assert_eq!(report.verdict(), Verdict::VacuousError);
        let _ = fs::remove_dir_all(&root);
    }

    /// THE LEG THAT PROVES omp-orchestrator-oej2: the same literal, in a file that is
    /// not part of the change, must not refuse the change.
    #[test]
    fn staged_mode_ignores_a_literal_outside_the_staged_set() {
        let root = std::env::temp_dir().join(format!("plg-scoped-{}", std::process::id()));
        let src = root.join("crates/example/src");
        fs::create_dir_all(&src).expect("create fixture tree");
        fs::write(src.join("clean.rs"), "fn main() {}\n").expect("write clean file");
        fs::write(src.join("other.rs"), planted_line()).expect("write unstaged dirty file");

        // Repo-wide sees it: the literal is genuinely there, and the sweep still says so.
        let sweep = scan(&root);
        assert_eq!(sweep.verdict(), Verdict::Violation, "{sweep:?}");

        // Scoped to the clean file only: GREEN. This is the direction that matters.
        let scoped = scan_paths(&root, &["crates/example/src/clean.rs"]);
        assert_eq!(scoped.mode, ScanMode::StagedPaths);
        assert_eq!(scoped.scanned.len(), 1, "{:?}", scoped.scanned);
        assert!(scoped.hits.is_empty(), "{:?}", scoped.hits);
        assert_eq!(scoped.verdict(), Verdict::Clean);

        // Scoped to the dirty file: RED, naming it. Scoping narrows the file set, it
        // does not weaken the check.
        let caught = scan_paths(&root, &["crates/example/src/other.rs"]);
        assert_eq!(caught.verdict(), Verdict::Violation);
        assert_eq!(caught.hits.len(), 1, "{:?}", caught.hits);
        assert!(caught.hits[0].file.ends_with("other.rs"));

        let _ = fs::remove_dir_all(&root);
    }

    /// ISOMORPHISM. Scoping must narrow WHICH files are read and nothing else: staged
    /// mode handed every eligible path must reach the same verdict as the sweep.
    #[test]
    fn staged_mode_over_the_whole_tree_equals_repo_wide() {
        let root = std::env::temp_dir().join(format!("plg-iso-{}", std::process::id()));
        let src = root.join("crates/example/src");
        fs::create_dir_all(src.join("nested")).expect("create fixture tree");
        fs::write(src.join("clean.rs"), "fn main() {}\n").expect("write clean file");
        fs::write(src.join("nested/deep.rs"), planted_line()).expect("write nested dirty file");

        let sweep = scan(&root);
        let every: Vec<PathBuf> = sweep.scanned.clone();
        let scoped = scan_paths(&root, &every);

        assert_eq!(scoped.scanned, sweep.scanned, "same files read");
        assert_eq!(scoped.hits, sweep.hits, "same hits, same file:line");
        assert_eq!(scoped.verdict(), sweep.verdict());

        let _ = fs::remove_dir_all(&root);
    }

    /// THREE OUTCOMES. Staging only a doc leaves this gate nothing to check, which is
    /// NOT clean — reporting it as clean is the vacuous-green inversion (calr).
    #[test]
    fn staged_set_with_no_eligible_file_is_nothing_to_check_not_clean() {
        let root = std::env::temp_dir().join(format!("plg-ntc-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/example/src")).expect("create fixture tree");
        // k0h1e: the second path used to be crates/example/tests/it.rs, which is now IN scope.
        // NothingToCheck must stay reachable, so the input is a path outside the widened floor.
        let report = scan_paths(&root, &["AGENTS.md", "crates/example/benches/b.rs"]);
        assert!(report.scanned.is_empty(), "{:?}", report.scanned);
        assert_eq!(report.verdict(), Verdict::NothingToCheck);
        assert!(
            !report.is_pass(),
            "nothing-to-check must not read as a pass"
        );
        assert_eq!(report.skipped.len(), 2);
        assert!(report
            .skipped
            .iter()
            .all(|s| s.reason == SkipReason::OutOfScope));
        let _ = fs::remove_dir_all(&root);
    }

    /// A staged DELETION must not panic the hook, and must be reported as uncovered
    /// rather than silently counted as clean.
    #[test]
    fn staged_deletion_is_recorded_as_absent_not_read() {
        let root = std::env::temp_dir().join(format!("plg-del-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/example/src")).expect("create fixture tree");
        let report = scan_paths(&root, &["crates/example/src/gone.rs"]);
        assert!(report.scanned.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, SkipReason::Absent);
        assert_eq!(report.verdict(), Verdict::NothingToCheck);
        let _ = fs::remove_dir_all(&root);
    }

    /// The shared predicate: both modes agree on the floor, including the boundaries the
    /// module docs promise are OUT of scope.
    #[test]
    fn the_printed_scope_is_derived_from_the_predicate_not_typed() {
        // ITEM 5. The scope was PRINTED but HAND-TYPED at five sites across two crates, which is
        // the count-in-prose defect wearing a different noun: a scope in prose is wrong the moment
        // the predicate moves. Assert the description names EVERY scanned subdir and nothing else.
        let described = scan_scope_description();
        for subdir in SCANNED_CRATE_SUBDIRS {
            assert!(
                described.contains(subdir),
                "the printed scope {described} omits {subdir}, which the predicate accepts"
            );
        }
        assert!(
            described.starts_with("crates/*/") && described.ends_with("/**.rs"),
            "the description must stay a path glob a reader can act on: {described}"
        );
        // ANTI-VACUITY: an empty subdir list would describe a scope that accepts nothing while
        // is_in_scan_scope returned false for everything -- a silently disabled gate.
        assert!(
            !SCANNED_CRATE_SUBDIRS.is_empty(),
            "an empty scanned-subdir list is a disabled gate, not a narrow one"
        );
    }

    /// ITEM 3 — NON-REGRESSION ON `oej2`, and it is the leg that matters.
    ///
    /// Widening the floor must not widen the SELECTION. A commit staging one unrelated file must
    /// not be refused because some other file — untracked or merely unstaged — carries a literal.
    /// That fleet block is why the narrow scope existed, so trading a silent gap for a fleet stall
    /// would be strictly worse than leaving the gap.
    #[test]
    fn a_literal_in_an_unstaged_test_file_cannot_refuse_an_unrelated_commit() {
        let root = std::env::temp_dir().join(format!("plg-k0h1e-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/example/src")).expect("create fixture src");
        fs::create_dir_all(root.join("crates/example/tests")).expect("create fixture tests");
        fs::write(root.join("crates/example/src/lib.rs"), "fn main() {}\n").expect("clean file");
        // The violation lives in a tests/ file that is NOW IN SCOPE but is NOT in the staged set.
        let planted = format!("const P: &str = \"{}\";\n", USER_HOME_LITERAL);
        fs::write(root.join("crates/example/tests/it.rs"), &planted).expect("planted file");

        // POSITIVE CONTROL FIRST: staged, that same file MUST refuse -- otherwise the negative
        // below passes for a gate that cannot see tests/ at all, which is the bug being fixed.
        let staged_it = scan_paths(&root, &["crates/example/tests/it.rs"]);
        assert_eq!(
            staged_it.verdict(),
            Verdict::Violation,
            "a literal STAGED in a tests/ file must be refused: {staged_it:?}"
        );

        // THE NON-REGRESSION: stage only the clean file. The planted literal is one directory
        // away, in scope, and unstaged -- it must be invisible.
        let unrelated = scan_paths(&root, &["crates/example/src/lib.rs"]);
        assert_eq!(
            unrelated.verdict(),
            Verdict::Clean,
            "an unstaged literal must not refuse an unrelated commit -- that is the oej2 fleet \
             block this scope was narrowed to prevent: {unrelated:?}"
        );
        assert_eq!(unrelated.scanned.len(), 1, "only the staged file may be read");

        let _ = fs::remove_dir_all(&root);
    }

    /// ITEM 5 AT THE SECOND SITE, RE-BASELINED 2026-09-12 (64wxc grade). `k0h1e` widened the
    /// GATE's floor to `{src,tests}` while the repo-wide walker still read `src` alone, so the
    /// repo-wide line ADVERTISED `crates/*/{src,tests}/**.rs` for a walker that never opened a
    /// `tests/` directory -- a label describing a predicate its command never evaluates.
    ///
    /// ⛔ `1a436be` RETIRED THAT DEFERRAL IN THE PRODUCT AND LEFT THIS LEG PINNING IT. With
    /// `REPO_WIDE_SUBDIRS = {src,tests}` the deferred set is EMPTY, so both loops below iterate
    /// NOTHING, while the old `scanned.len() == 1` still asserted a one-file sweep. The fixture
    /// carries one file per walked subdir, so the sweep correctly read 2 and the leg correctly
    /// reddened: the fix was right and this assertion was stale.
    ///
    /// The property that SURVIVES the retirement is narrower than the original: **the advertised
    /// line never claims a subdir the walker does not open.** It is asserted here in both
    /// directions -- see the rule-11 pin at the end, which fails if a NON-walked subdir ever
    /// appears in `scanned` -- so this leg cannot go vacuous again when the floors next diverge.
    #[test]
    fn the_repo_wide_line_never_advertises_a_subdir_the_sweep_defers() {
        let deferred = repo_wide_deferred_subdirs();
        let described = repo_wide_scope_description();
        for subdir in &deferred {
            assert!(
                !described.contains(subdir),
                "the sweep advertises {subdir} in {described} but its walker is seeded from \
                 {REPO_WIDE_SUBDIRS:?}"
            );
        }

        // ⛔ THE EMPTY CASE IS THE LIVE CASE, AND A LOOP OVER AN EMPTY SET IS NOT AN ASSERTION.
        // Post-1a436be the deferred set is empty, so the loop above iterates nothing. Assert the
        // EQUALITY explicitly instead: with no deferral the advertised scope must name every
        // walked subdir. Without this branch the leg is silent exactly when the floors agree.
        if deferred.is_empty() {
            for subdir in REPO_WIDE_SUBDIRS {
                assert!(
                    described.contains(subdir),
                    "no subdir is deferred, so the advertised scope must name every walked \
                     subdir: {subdir} missing from {described}"
                );
            }
        }

        // The narrowing must be DECLARED, not silent: the runtime line names every deferred
        // subdir, so a reader learns the sweep is narrower than the gate from the verdict itself.
        let root = std::env::temp_dir().join(format!("plg-sweepfloor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("crates/example/src")).expect("create fixture src");
        fs::create_dir_all(root.join("crates/example/tests")).expect("create fixture tests");
        fs::write(root.join("crates/example/src/lib.rs"), "fn main() {}\n").expect("clean file");
        let planted = format!("const P: &str = \"{}\";\n", USER_HOME_LITERAL);
        fs::write(root.join("crates/example/tests/it.rs"), &planted).expect("planted file");
        // The rule-11 pin below needs a file the walker must NOT open, or it passes vacuously --
        // which is the same defect this leg was re-baselined to remove. `benches` is outside
        // REPO_WIDE_SUBDIRS, and this file carries the literal so a walker that opened it would
        // both widen `scanned` AND change the verdict.
        fs::create_dir_all(root.join("crates/example/benches"))
            .expect("create fixture benches");
        fs::write(root.join("crates/example/benches/b.rs"), &planted).expect("unwalked file");

        let report = scan(&root);
        let line = report.declared_scope_line();
        assert!(
            line.contains(&described),
            "the repo-wide line must print the floor it walked: {line}"
        );
        for subdir in &deferred {
            assert!(
                line.contains("DEFERRED") && line.contains(subdir),
                "a deferred subdir must be named in the verdict, not omitted: {line}"
            );
        }
        // POSITIVE CONTROL for the same fixture: the file the sweep defers IS refused when it
        // reaches the gate, so this leg cannot pass for a scope that sees tests/ nowhere at all.
        assert_eq!(
            scan_paths(&root, &["crates/example/tests/it.rs"]).verdict(),
            Verdict::Violation,
            "the gate's floor must still refuse the literal the sweep defers"
        );
        // DERIVED, NEVER HARD-CODED: the fixture plants exactly one file per WALKED subdir, so the
        // expected count is a function of REPO_WIDE_SUBDIRS. A literal here is what made this leg
        // pin a retired contract, and a literal would do it again on the next floor change.
        assert_eq!(
            report.scanned.len(),
            REPO_WIDE_SUBDIRS.len(),
            "the sweep reads exactly one file per walked subdir, derived from \
             {REPO_WIDE_SUBDIRS:?}, not a frozen count: {:?}",
            report.scanned
        );

        // ⛔ RULE 11 -- PIN THE WRONG ANSWER TOO. Asserting only that the walked subdirs ARE read
        // proves a STATE; also asserting that a NON-walked subdir is NOT read proves the
        // DISTINCTION between the advertisement and the walker, which is this leg's whole subject.
        // Without it, a walker that opened everything would pass every assertion above.
        assert!(
            !report
                .scanned
                .iter()
                .any(|path| path.components().any(|c| c.as_os_str() == "benches")),
            "a file under a subdir OUTSIDE {REPO_WIDE_SUBDIRS:?} must never appear in the sweep, \
             or the advertised floor understates what the walker opens: {:?}",
            report.scanned
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// ANTI-VACUITY ON THE SUBSET ITSELF. An empty sweep floor is a disabled sweep, and a floor
    /// carrying a subdir the gate does not accept would make the sweep claim MORE than the gate
    /// enforces -- the overclaim direction, which is worse than the narrowing.
    #[test]
    fn the_sweep_floor_is_a_nonempty_subset_of_the_gate_floor() {
        assert!(
            !REPO_WIDE_SUBDIRS.is_empty(),
            "an empty sweep floor reads as a clean repository while opening nothing"
        );
        for subdir in REPO_WIDE_SUBDIRS {
            assert!(
                SCANNED_CRATE_SUBDIRS.contains(subdir),
                "the sweep walks {subdir}, which the gate's own predicate rejects"
            );
        }
    }

    #[test]
    fn eligibility_predicate_matches_the_declared_floor() {
        for inside in [
            "crates/x/src/lib.rs",
            "crates/x/src/bin/y.rs",
            "crates/x/src/a/b/c.rs",
            // omp-orchestrator-k0h1e: tests/ moved INTO the floor. %20 proved the gap by staging
            // a real home-path literal under tests/ and watching it LAND (25a7e5b, reverted at
            // fd9f6f3). This row used to sit in the `outside` list below.
            "crates/x/tests/it.rs",
            "crates/x/tests/nested/it.rs",
        ] {
            assert!(
                is_in_scan_scope(Path::new(inside)),
                "{inside} must be in scope"
            );
        }
        // STILL OUTSIDE, and deliberately so: widening `src` to `{src,tests}` must not become
        // "every .rs anywhere". benches/, build.rs and a bare src/ outside crates/ stay out.
        for outside in [
            "crates/x/benches/b.rs",
            "crates/x/build.rs",
            "crates/x/src/lib.toml",
            "crates/x/Cargo.toml",
            "src/main.rs",
            "AGENTS.md",
            "docs/plan/PLAN.md",
        ] {
            assert!(
                !is_in_scan_scope(Path::new(outside)),
                "{outside} must be out of scope"
            );
        }
    }

    /// A DECLARED row suppresses exactly its own hit and is reported, not vanished; a
    /// row that matches nothing comes back stale.
    #[test]
    fn allowlist_suppresses_its_row_and_reports_a_stale_one() {
        let hit = Hit {
            file: PathBuf::from("crates/x/src/lib.rs"),
            line: 7,
        };
        let other = Hit {
            file: PathBuf::from("crates/y/src/lib.rs"),
            line: 9,
        };
        let rows = &[AllowRow {
            file: "crates/x/src/lib.rs",
            line_contains: "SPECIMEN",
            reason: "declared",
        }];
        let lines = vec!["// SPECIMEN row".to_owned(), "real".to_owned()];
        let (kept, allowed, stale) = apply_allowlist(vec![hit, other], &lines, rows);
        assert_eq!(kept.len(), 1);
        assert!(kept[0].file.ends_with("crates/y/src/lib.rs"));
        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].reason, "declared");
        assert!(stale.is_empty());

        let (_, _, stale) = apply_allowlist(Vec::new(), &[], rows);
        assert_eq!(stale.len(), 1, "a row that suppresses nothing must surface");
    }

    /// Every row, if one is ever added, must say WHY.
    #[test]
    fn declared_allowlist_rows_carry_a_reason() {
        for row in DECLARED_ALLOWLIST {
            assert!(row.reason.len() > 40, "row {row:?} needs a real reason");
            assert!(!row.line_contains.is_empty(), "row {row:?} needs a needle");
        }
    }

    /// The declared scope line must say which mode it covers, and the staged one must
    /// refuse to imply repository cleanliness.
    #[test]
    fn declared_scope_line_names_the_mode_and_its_limit() {
        let sweep = ScanReport {
            mode: ScanMode::RepoWide,
            scanned: vec![PathBuf::from("crates/x/src/lib.rs")],
            skipped: Vec::new(),
            hits: Vec::new(),
            allowed: Vec::new(),
            stale_allowlist: Vec::new(),
        };
        let line = sweep.declared_scope_line();
        assert!(line.contains("repo-wide"), "{line}");
        assert!(line.contains("ERROR"), "{line}");

        let scoped = ScanReport {
            mode: ScanMode::StagedPaths,
            scanned: vec![PathBuf::from("crates/x/src/lib.rs")],
            skipped: vec![Skipped {
                file: PathBuf::from("AGENTS.md"),
                reason: SkipReason::OutOfScope,
            }],
            hits: Vec::new(),
            allowed: Vec::new(),
            stale_allowlist: Vec::new(),
        };
        let line = scoped.declared_scope_line();
        assert!(line.contains("staged set only"), "{line}");
        assert!(line.contains("UNTRACKED"), "{line}");
        assert!(line.contains("1 outside"), "{line}");
    }
}
