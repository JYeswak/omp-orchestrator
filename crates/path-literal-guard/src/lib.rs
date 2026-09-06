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
            Self::OutOfScope => formatter.write_str("outside crates/*/src/**.rs"),
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
                "DECLARED SCOPE repo-wide: every .rs under crates/*/src, {} file(s) read, \
                 untracked included. An empty scan set is an ERROR, not a pass. {common}.",
                self.scanned.len()
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
                     crates/*/src/**.rs, {} absent from the worktree. A literal in an \
                     UNSTAGED or UNTRACKED file is not this commit's problem and cannot \
                     refuse it; only `cargo test -p path-literal-guard` (repo-wide mode) \
                     claims the repository is clean. {common}.",
                    self.scanned.len(),
                    out,
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
/// True for a repo-relative path that is a `.rs` file under `crates/<crate>/src/`.
/// Both modes route through this, so a scoped run and a sweep can never disagree about
/// WHAT is in scope — only about which subset was read.
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
    // crates / <crate> / src / ... / <file>.rs
    parts.len() >= 4 && parts[0] == "crates" && parts[2] == "src"
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
        let src = entry.path().join("src");
        if src.is_dir() {
            stack.push(src);
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
        let report = scan_paths(&root, &["AGENTS.md", "crates/example/tests/it.rs"]);
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
    fn eligibility_predicate_matches_the_declared_floor() {
        for inside in [
            "crates/x/src/lib.rs",
            "crates/x/src/bin/y.rs",
            "crates/x/src/a/b/c.rs",
        ] {
            assert!(
                is_in_scan_scope(Path::new(inside)),
                "{inside} must be in scope"
            );
        }
        for outside in [
            "crates/x/tests/it.rs",
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
