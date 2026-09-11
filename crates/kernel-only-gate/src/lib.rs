//! kernel-only-gate — narrow installable commit-path gate over three handroll
//! families: raw tmux capture/send, raw `br create`, and their `Command::new`
//! spawn roots.
//!
//! # Scope split with kernel-bypass-gate (read before extending either)
//!
//! `kernel-bypass-gate` is the broad ownership census: nine patterns, debt
//! ceilings, workspace-wide. It is RED on the live tree and has no reachable
//! executor, so it cannot gate a commit. This crate is the narrow installable
//! half: eight patterns over three families, structural matching through
//! [`text_structure::code_only`], a declared ownership table with no ceilings,
//! and green on the current tree by construction. Different pattern sets are
//! NOT the split — the split is census-plus-ratchet versus commit-path refusal.
//!
//! # Matching rule
//!
//! Every needle is assembled from parts (`concat!`) so this file never contains
//! a raw needle contiguously — otherwise the self-leg would fire on its own
//! source. Whole-text search over `code_only` output (comments blanked, line
//! structure preserved), offset converted to 1-based line. A match inside a
//! string literal still fires: that is the safe direction for a refusal gate,
//! and the legs pin it rather than hide it.
//!
//! # Explicitly out of scope, with reasons
//!
//! * `ntm` spawns: supervisory L4 verification spawns (`ompo-doctor liveness`)
//!   are sanctioned by the S1 contract; no dispatch kernel owns that intent.
//! * `br ready`: reading queue state, not dispatching through it.
//! * Interpreter spawns (`python3`, `sh`): rule scope undecided (see bead
//!   comment 880); this gate must not preempt it.
//! * `tests/` trees: harness setup code may drive raw interfaces to build
//!   fixtures. Eligible paths are `.rs` under `crates/` with no `tests`
//!   segment.
//!
//! # Limit, printed on every run
//!
//! This gate reads source text. It cannot see an operator handrolling in a
//! shell — that half is a separate PreToolUse bead, and any report implying
//! otherwise is a lie this module refuses to tell.

#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

/// The single production conversion site is [`HandrollScanReport::verdict`]; there is
/// no second mapping from hits to outcome anywhere in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Non-empty scan set, zero surviving hits.
    Clean,
    /// At least one surviving hit.
    Violation,
    /// Staged mode with zero eligible paths: no opinion on this commit.
    NothingToCheck,
    /// Repo-wide mode with an empty scan set: an ERROR, never a pass.
    VacuousError,
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => write!(formatter, "CLEAN"),
            Self::Violation => write!(formatter, "VIOLATION"),
            Self::NothingToCheck => write!(formatter, "NOTHING_TO_CHECK"),
            Self::VacuousError => write!(formatter, "VACUOUS_SCAN_SET"),
        }
    }
}

/// One actionable finding. `kernel` is load-bearing, never decorative: a
/// finding that does not name the replacement kernel fails the known-bad leg.
///
/// NAMED `HandrollHit`, NOT `Hit` — and `HandrollScanReport`, not `ScanReport`. This crate
/// became tracked in 6f0dfbe, at which point both names began colliding with
/// `path-literal-guard`'s public `Hit` and `ScanReport` and
/// `no_public_type_name_collisions_across_crates` refused. The collision is real even though the
/// domains are disjoint (a handroll-family match versus a path-literal match), and the remedy is
/// a specific name rather than an allowance row: nothing consumes either type across a crate
/// boundary (`grep -rn 'kernel_only_gate::'` outside this crate -> 0 hits), so the fix costs two
/// names instead of a permanent exception with nothing to expire it.
///
/// The rename landed HERE and not in `path-literal-guard` for a second, non-aesthetic reason:
/// that crate IS in `HOOK_SOURCE_CRATES` (`no-shell-gate/src/commit_ratchets.rs:18-24`), so a
/// byte under its `src` would stale the installed hook and refuse every commit in the tree. This
/// crate is not in that list, so the same fix costs the fleet nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandrollHit {
    pub file: String,
    pub line: usize,
    pub pattern: String,
    pub kernel: String,
}

impl fmt::Display for HandrollHit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{} uses raw \"{}\" — use the {} kernel instead",
            self.file, self.line, self.pattern, self.kernel
        )
    }
}

/// A raw shape, the kernel that owns the capability, and the crates that
/// implement that kernel. Ownership is DECLARED here and nowhere else: a new
/// kernel does not silently exempt itself, and a non-implementing crate can
/// never be added without that lie sitting in this table in review.
struct Pattern {
    needle: &'static str,
    kernel: &'static str,
    owners: &'static [&'static str],
}
// Every needle below is assembled from parts (`concat!`) so this file never
// contains a raw needle contiguously — otherwise the self-leg would fire on
// its own source.
const P_TMUX_CAP_ARGV: &str = concat!("\"tmux\", \"", "capture-pane\"");
const P_TMUX_CAP_SHELL: &str = concat!("tmux ", "capture-pane");
const P_TMUX_SEND_ARGV: &str = concat!("\"tmux\", \"", "send-keys\"");
const P_TMUX_SEND_SHELL: &str = concat!("tmux ", "send-keys");
const P_BR_CREATE_ARGV: &str = concat!("\"br\", \"", "create\"");
const P_BR_CREATE_SHELL: &str = concat!("br ", "create");
const P_SPAWN_TMUX: &str = concat!("Command::new(", "\"tmux\")");
const P_SPAWN_BR: &str = concat!("Command::new(", "\"br\")");

const PATTERNS: &[Pattern] = &[
    Pattern {
        needle: P_TMUX_CAP_ARGV,
        kernel: "tick-monitor observe",
        owners: &["tick-monitor"],
    },
    Pattern {
        needle: P_TMUX_CAP_SHELL,
        kernel: "tick-monitor observe",
        owners: &["tick-monitor"],
    },
    Pattern {
        needle: P_TMUX_SEND_ARGV,
        kernel: "dispatch robot-send",
        owners: &[],
    },
    Pattern {
        needle: P_TMUX_SEND_SHELL,
        kernel: "dispatch robot-send",
        owners: &[],
    },
    Pattern {
        needle: P_BR_CREATE_ARGV,
        kernel: "beads-workflow bead filing",
        owners: &["finding"],
    },
    Pattern {
        needle: P_BR_CREATE_SHELL,
        kernel: "beads-workflow bead filing",
        owners: &["finding"],
    },
    Pattern {
        needle: P_SPAWN_TMUX,
        kernel: "tick-monitor pane access",
        owners: &["tick-monitor"],
    },
    Pattern {
        needle: P_SPAWN_BR,
        kernel: "beads-workflow bead filing",
        owners: &["finding"],
    },
];

/// Scope line printed on every run: what was checked and the hard limit.
pub const SCOPE_LINE: &str = "kernel-only-gate: SOURCE ONLY -- scans .rs source text under crates/ (never tests/); operator shell commands are invisible to this gate (separate PreToolUse bead)";

/// Crate owning `crates/<name>/...`. `None` outside a crates tree claims no
/// ownership and therefore never exempts.
fn owning_crate(file: &str) -> Option<&str> {
    let mut parts = file.split('/');
    while let Some(part) = parts.next() {
        if part == "crates" {
            return parts.next();
        }
    }
    None
}

fn is_eligible(path: &str) -> bool {
    path.ends_with(".rs")
        && path.starts_with("crates/")
        && !path.split('/').any(|part| part == "tests")
}

/// Outcome of one scan pass over staged paths or a walked tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandrollScanReport {
    pub hits: Vec<HandrollHit>,
    pub scanned: Vec<String>,
    staged: bool,
}

impl HandrollScanReport {
    pub fn verdict(&self) -> Verdict {
        if self.scanned.is_empty() {
            // Same empty set, two meanings: a commit with no eligible paths
            // is out of scope (no opinion); a repo-wide walk that found
            // nothing to check is an instrument failure (error).
            return if self.staged {
                Verdict::NothingToCheck
            } else {
                Verdict::VacuousError
            };
        }
        if self.hits.is_empty() {
            Verdict::Clean
        } else {
            Verdict::Violation
        }
    }

    pub fn scope_line(&self) -> String {
        format!(
            "{} -- scanned={} hits={}",
            SCOPE_LINE,
            self.scanned.len(),
            self.hits.len()
        )
    }
}
/// line structure.
pub fn scan_source(file: &str, source: &str) -> Vec<HandrollHit> {
    let code: Cow<'_, str> = text_structure::code_only(source);
    let mut hits = Vec::new();
    for pattern in PATTERNS {
        if owning_crate(file).is_some_and(|owner| pattern.owners.contains(&owner)) {
            continue;
        }
        let mut base = 0;
        while let Some(relative) = code[base..].find(pattern.needle) {
            let offset = base + relative;
            let line = code[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1;
            hits.push(HandrollHit {
                file: file.to_owned(),
                line,
                pattern: pattern.needle.to_owned(),
                kernel: pattern.kernel.to_owned(),
            });
            base = offset + 1;
        }
    }
    hits
}

fn read_scored(root: &Path, relative: &str) -> Option<(String, String)> {
    let text = std::fs::read_to_string(root.join(relative)).ok()?;
    Some((relative.to_owned(), text))
}

/// Commit-path entry: score exactly the staged paths that fall in scope.
/// Zero eligible paths is [`Verdict::NothingToCheck`], not clean.
pub fn scan_paths<P: AsRef<str>>(root: &Path, paths: &[P]) -> HandrollScanReport {
    let mut report = HandrollScanReport {
        hits: Vec::new(),
        scanned: Vec::new(),
        staged: true,
    };
    for path in paths {
        let relative = path.as_ref();
        if !is_eligible(relative) {
            continue;
        }
        if let Some((file, text)) = read_scored(root, relative) {
            report.scanned.push(file.clone());
            report.hits.extend(scan_source(&file, &text));
        }
    }
    report
}

/// Repo-wide entry: walk `crates/` for eligible files. An empty scan set is
/// [`Verdict::VacuousError`], never a pass.
pub fn scan_tree(root: &Path) -> HandrollScanReport {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir);
        let mut entries: Vec<_> = entries.into_iter().flatten().flatten().collect();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    continue;
                }
                visit(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }
    let mut report = HandrollScanReport {
        hits: Vec::new(),
        scanned: Vec::new(),
        staged: false,
    };
    let mut files = Vec::new();
    visit(&root.join("crates"), &mut files);
    for path in files {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if !is_eligible(&relative) {
            continue;
        }
        if let Some((file, text)) = read_scored(root, &relative) {
            report.scanned.push(file.clone());
            report.hits.extend(scan_source(&file, &text));
        }
    }
    report
}
