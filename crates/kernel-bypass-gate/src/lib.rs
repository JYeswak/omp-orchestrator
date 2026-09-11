#![forbid(unsafe_code)]

//! kernel-bypass-gate — detects raw invocations that duplicate an existing kernel.
//!
//! THE MEASURED INDICTMENT (all pane 1's, all one night):
//!   observe  — hand-grepped pane capture for 12h while tick-monitor observe was
//!              installed and returned MORE (state, timer, liveness, session scoping).
//!   dispatch — raw key-injection while five dispatch crates were cron-scheduled.
//!   receipt  — grep -oE on a timer while the receiver-receipt crate existed.
//!   file     — raw bead filing while crates/finding existed (written 30 min earlier).
//!   queue    — the ready query piped to python while bv --robot-triage reported scores.
//!
//! THE MECHANISM: when a kernel was broken, the route was around it instead of through
//! it. Every handroll is locally cheaper and removes exactly the pressure that would
//! have fixed the kernel. That is why the kernels stay broken.
//!
//! WHAT THIS GATE DETECTS: a tracked .rs source file invoking a kernel's raw interface
//! from OUTSIDE the kernel crate that owns it. The kernel registry is DECLARED — a const
//! in this file — not inferred, so adding a kernel requires adding its crate to the
//! allowlist and the gate enforces the declaration.
//! ENFORCES: committed Rust source outside the owning crate cannot bypass a declared kernel
//! pattern without a named debt row and a non-slack ceiling.
//! STILL PASSES: the owning kernel crate, comments, and allowlisted internal sites remain clean;
//! the operator's uncommitted shell handrolls remain outside this source gate.
//! PROVENANCE: the registry and debt ceilings below are measured from the committed tree and
//! every allowance names its owner and the condition that removes it.
//!
//! THE LIMIT, stated in the gate's own output: this gate scans COMMITTED SOURCE ONLY.
//! It cannot see an operator handrolling in a shell, which is how five kernels were
//! bypassed that night. The operator half needs a PreToolUse hook and is a separate bead.
//!
//! # THE INSTALLABILITY PROBLEM, AND THE RATCHET THAT SOLVES IT
//!
//! MEASURED 2026-09-02, GitHub Actions run 33585450134, job 100108539617, step
//! "Scan checked-out workspace for kernel bypasses", producing command:
//!
//! ```text
//! cargo run --quiet -p kernel-bypass-gate -- .
//! KERNEL-BYPASS-GATE: 123 files scanned, 81 kernel bypass(es)
//! ##[error]Process completed with exit code 1.
//! ```
//!
//! Roughly forty crates handroll a kernel's raw interface. A gate that refuses all of
//! them refuses **every commit in the repo** the moment it is wired — and a gate that
//! cannot be installed is worth zero (`fh N043`). It was in fact worse than zero: this
//! step had **no reachable green leg at all**, so its red was structural, unreadable and
//! unread across twelve consecutive runs. A blanket allowance would be the opposite
//! failure: vacuously green forever.
//!
//! The resolution is the ratchet this repo already uses in `crate-atom-gate`
//! (`crates/crate-atom-gate/src/lib.rs:196-211`): a [`SystemicBypassAllowance`] declares
//! one whole kernel pattern repo-wide with an `owner`, a `dies_when`, and a **CEILING
//! that may only be LOWERED**. The gate therefore:
//!
//! - REFUSES a bypass whose pattern has no allowance row at all, so an undeclared kernel
//!   pattern is enforced absolutely from its first site — CONTINGENT on no allowance row
//!   ever naming that pattern: the None arm is the entire absolute enforcement, and adding
//!   a row moves the pattern out of it. A ledger reddened by a re-added row must be answered
//!   by DELETING the row, never by allowing it; the path of least resistance is the defect.
//! - REFUSES when a pattern's live count **exceeds** its ceiling, so a new handroll cannot
//!   silently widen a known gap;
//! - REFUSES a row whose ceiling sits **above** the live count (`CEILING_HAS_SLACK`),
//!   because an allowance with slack is an allowance that never shrinks.
//!
//! That makes this gate blocking against REGRESSION on the day it lands, while the
//! absolute debt is declared, owned, and mechanically forced downward.
//!
//! NO-CLAIM: the ratchet makes the debt *legible and non-growing*. It does not reduce it.
//! Roughly forty crates bypassing five kernels is the KERNEL-ONLY rule failing at scale
//! and is filed as its own bead — it is deliberately NOT buried in the ceiling table.

use std::fmt;
use std::path::Path;
use text_structure::code_only;

// THE NEEDLES ARE ASSEMBLED FROM PARTS, and that is load-bearing.
//
// MEASURED 2026-09-02, run 33585450134: seven of the eighty-one reported bypasses were
// `./crates/kernel-bypass-gate/src/lib.rs:38`–`:54` — the registry rows themselves. The
// gate flagged its own declaration, because the needle and the declaration were the same
// bytes. That is the same self-referential-checker class AGENTS.md enumerates, and it is
// the one instance `strip_line_comment` cannot reach: these needles were string literals
// in code, not prose in a comment.
//
// `concat!` expands at compile time, so the VALUE is the whole needle while the SOURCE
// TEXT of this file never contains it contiguously. Splitting protects the checker from
// its own source; blanking comments (`text_structure::code_only`) protects it from
// every other file's prose. Both are required — AGENTS.md records a
// doc comment *warning about* a needle that contained the needle and left a census GREEN
// while its subject had been deleted.
// BARE WORDS ARE NOT PATTERNS. Five bare-word needles lived here until 2026-09-11
// (`tmux capture-pane`, `tmux send-keys`, `robot-send`, `br ready`, `br create`).
// They were deleted, not bounded: 11 of the 12 live sites they matched were prose,
// not invocations (`.beads/issues.jsonl#3167` legs 1-10: 9 string literals, 2 flag
// collisions), and no bound fixes a literal (`"EMPTY_SURFACE: br ready"` has word
// neighbours on both sides). What remains are EXECUTION UNITS — `Command::new`
// literals that name the spawned program. A unit cannot appear in prose without
// naming a real spawn site, which is the property a bound was supposed to supply.
// The printf leg (`--robot-send-receipt=x` is clean) passes vacuously now: there is
// no `robot-send` needle left to collide with. That is the fix, not a gap in the leg.
const NEEDLE_SPAWN_TMUX: &str = concat!("Command::new(", "\"tmux\")");
const NEEDLE_SPAWN_NTM: &str = concat!("Command::new(", "\"ntm\")");
const NEEDLE_SPAWN_BR: &str = concat!("Command::new(", "\"br\")");

// THE SAME TRAP, ONE LAYER OVER. Splitting the needle table above took the gate's
// self-flagged rows from seven to THREE. The survivors were `lib.rs:100`, `:101` and
// `:113` — the human-readable KERNEL NAME `"dispatch robot-send"`, which contains the
// needle `robot-send` as a substring. The mitigation had been applied to the column
// everyone thinks of as "the pattern" and not to the column beside it. Measured, not
// reasoned: the count only went to zero after this const existed.
const KERNEL_DISPATCH_SEND: &str = concat!("dispatch ", "robot", "-send");

/// The kernel registry: maps a raw invocation pattern to the kernel that should
/// have been used. Each entry is (pattern, kernel_name, owning_crate).
///
/// The owning_crate is the ALLOWLIST: if the pattern appears in a file under
/// that crate's directory, it is a legitimate kernel-internal call and is NOT
/// a violation. A pattern in any OTHER crate is a bypass.
///
/// DECLARED, NOT INFERRED: adding a kernel requires adding its crate here.
///
/// EXECUTION UNITS ONLY. Every row names a `Command::new` program literal. A bare
/// word (`robot-send`, `br ready`) matches prose that merely MENTIONS the kernel —
/// error text, packet instructions, labels — which a bound cannot repair. Deleting
/// the row, rather than bounding the needle, is what removed the 11 false sites.
pub const KERNEL_REGISTRY: &[(&str, &str, &str)] = &[
    (NEEDLE_SPAWN_TMUX, "tick-monitor pane access", "tick-monitor"),
    // io67x: ntm invocation now has ONE owner, the `ntm-kernel` crate, which
    // builds the argv, spawns through `subprocess-contract` (fresh process
    // group, both pipes drained, deadline signals the GROUP) and returns a
    // typed answer that branches on EXIT before any payload field is read.
    // The three handrolls this replaced — ompo-doctor's cass-context spawn,
    // pane-dispatch-ready's agent-health probe and fast-dispatch's dialog
    // probe — each had to re-learn the same five payload traps; the kernel
    // encodes them once with a leg apiece. `tick-monitor` holds ZERO sites for
    // this needle (measured), so moving the allowlist here strands nothing.
    (NEEDLE_SPAWN_NTM, "ntm-kernel invocation", "ntm-kernel"),
    (
        NEEDLE_SPAWN_BR,
        "beads-workflow bead filing",
        "finding",
    ),
];

/// One whole kernel pattern excused repo-wide, with a ceiling that may only be lowered.
///
/// Modelled on `crate_atom_gate::SystemicAllowance`. The two non-numeric fields are the
/// point: a ceiling with no owner and no death condition is a permanent exemption wearing
/// a ratchet's clothes, and AGENTS.md is explicit that `no-shell-gate`'s exemption list is
/// empty *by design* — a carve-out is what let 160 scripts accrete in the repo this
/// substrate was extracted from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemicBypassAllowance {
    /// The raw invocation pattern excused.
    pub pattern: &'static str,
    /// Who owns closing it. Never a pid: an owner must outlive the row.
    pub owner: &'static str,
    /// The condition that DELETES this row. A row with no death condition is permanent.
    pub dies_when: &'static str,
    /// Live bypass sites for this pattern. **May only be LOWERED.**
    ///
    /// Checked in BOTH directions: a live count above it REFUSES (regression), and a live
    /// count below it also REFUSES (`CEILING_HAS_SLACK`) — because an allowance with slack
    /// is an allowance that never shrinks.
    pub ceiling: usize,
}

/// The bypass debt ledger.
///
/// PROVENANCE. Every ceiling below was measured on the COMMITTED tree, not the worktree,
/// with the gate's own sources overlaid so the figure describes the tree CI will scan
/// AFTER this commit rather than the one before it:
///
/// ```text
/// git rev-parse HEAD  -> 4fb865c8e2d1ef5a1cf2384f40dcd08f6b8f0cc8
/// git archive HEAD | tar -x -C "$SCRATCH/pin-4fb865c"
/// cp crates/kernel-bypass-gate/src/{lib,main}.rs "$SCRATCH/pin-4fb865c/crates/kernel-bypass-gate/src/"
/// cargo run --quiet -p kernel-bypass-gate -- "$SCRATCH/pin-4fb865c"
/// KERNEL-BYPASS-GATE: 172 files scanned, 92 kernel bypass(es)
/// ```
///
/// The same command against the dirty WORKTREE returned **105** over the same 172 files.
/// The thirteen extra sites are uncommitted edits in exactly two files —
/// `crates/fast-dispatch/src/main.rs` (7 -> 15) and `crates/loop-tick/src/lib.rs`
/// (1 -> 6) — which no lane in this batch declared. If those edits are committed, this
/// ledger refuses with `NEW_BYPASS`, and the correct response is to bump `robot-send`
/// 21 -> 32, `Command::new("tmux")` 22 -> 23 and `Command::new("ntm")` 9 -> 10 in the
/// same commit that lands them. That is the ratchet doing its job, not a defect.
///
/// A ceiling measured on a worktree is a ceiling measured on somebody else's half-saved
/// edit, which is why this is pinned to `git archive HEAD`.
///
/// RECORDED OBJECTION (`InventoryAndFence`, 2026-09-03, verbatim): *"Your ratchet
/// refusing in BOTH directions (CEILING_HAS_SLACK on under-count) will therefore fire
/// constantly against a tree that grows 18 crates/day — a bidirectional ratchet turns
/// every lane that deletes a bypass into a red. I'd make the under-count leg a WARN."*
/// That measurement is real (51 -> 69 crates in the 19h before this commit) but it counts
/// CRATES, and these eight rows count SITES per pattern. The bidirectional check is kept
/// because `crate-atom-gate` already refuses both ways for the stated reason — an
/// allowance with slack is an allowance that never shrinks — and because the refusal
/// message names a one-integer edit. If this costs more than it catches, this paragraph
/// is the evidence trail for reversing it.
/// REFILL 2026-09-11 (bead -9ub39). The ledger above described the bare-word
/// registry; when the five bare rows were deleted their violations went with them
/// and the emptied ledger refused the tree with UNDECLARED_PATTERN on the one
/// remaining true site. That row re-filled it for the execution-unit registry at
/// ceiling 1, the one live site being
/// `crates/ompo-doctor/src/liveness.rs` spawning `ntm` for cass context.
///
/// PAID OFF 2026-09-11 (bead io67x), AND THE ROW IS DELETED RATHER THAN ZEROED,
/// because `EmptyRow` refuses a zeroed row by name. The row's own `dies_when`
/// was "ompo-doctor liveness routes its cass-context spawn through a dispatch
/// kernel", and it now does: `ntm-kernel` owns the invocation, the registry
/// above names that crate as the allowlisted owner, and the three handrolls
/// (ompo-doctor liveness, pane-dispatch-ready agent-health, fast-dispatch
/// dialogs) route through it. Measured outside the owning crate: ZERO.
/// ⛔ THE CEILING WAS NEVER RAISED. It went 1 -> row deleted, which is the
/// pay-off path this file documents; a raise would have been the amnesty
/// pathology, and the ratchet is INTACT — a new `Command::new("ntm")` in any
/// non-owning crate now refuses with UNDECLARED_PATTERN instead of
/// NEW_BYPASS, which is a stricter refusal, not a weaker one.
pub const BYPASS_DEBT: &[SystemicBypassAllowance] = &[];
/// A detected kernel bypass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bypass {
    pub file: String,
    pub line: usize,
    pub pattern: String,
    pub kernel: String,
}

impl fmt::Display for Bypass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{} uses raw \"{}\" — use the {} kernel instead",
            self.file, self.line, self.pattern, self.kernel
        )
    }
}

/// The result of one lint pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReport {
    pub scanned: Vec<String>,
    pub violations: Vec<Bypass>,
}

impl GateReport {
    pub fn is_pass(&self) -> bool {
        !self.scanned.is_empty() && self.violations.is_empty()
    }
}

/// Why the ratchet refused. Each variant names the row a human must edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebtFault {
    /// A pattern with live sites has no allowance row at all.
    Undeclared { pattern: String, measured: usize },
    /// The live count exceeds the declared ceiling: new debt.
    NewBypass {
        pattern: String,
        owner: String,
        measured: usize,
        ceiling: usize,
    },
    /// The declared ceiling sits above the live count: an allowance that never shrinks.
    CeilingHasSlack {
        pattern: String,
        owner: String,
        measured: usize,
        ceiling: usize,
    },
    /// A row declares a ceiling of zero. Pay-off deletes the row; it does not zero it.
    EmptyRow { pattern: String, owner: String },
}

impl fmt::Display for DebtFault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebtFault::Undeclared { pattern, measured } => write!(
                formatter,
                "UNDECLARED_PATTERN pattern=\"{pattern}\" measured={measured} \
                 — no allowance row exists; this pattern is enforced absolutely"
            ),
            DebtFault::NewBypass {
                pattern,
                owner,
                measured,
                ceiling,
            } => write!(
                formatter,
                "NEW_BYPASS pattern=\"{pattern}\" measured={measured} ceiling={ceiling} \
                 owner={owner} — a handroll was ADDED; route it through the kernel"
            ),
            DebtFault::CeilingHasSlack {
                pattern,
                owner,
                measured,
                ceiling,
            } => write!(
                formatter,
                "CEILING_HAS_SLACK pattern=\"{pattern}\" measured={measured} \
                 ceiling={ceiling} owner={owner} — lower the ceiling to {measured}"
            ),
            DebtFault::EmptyRow { pattern, owner } => write!(
                formatter,
                "EMPTY_ROW pattern=\"{pattern}\" owner={owner} \
                 — a paid-off pattern must have its row DELETED, not zeroed"
            ),
        }
    }
}

/// The ratchet's verdict over one scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebtVerdict {
    pub faults: Vec<DebtFault>,
}

impl DebtVerdict {
    pub fn is_pass(&self) -> bool {
        self.faults.is_empty()
    }
}

/// Apply the debt ledger to a scan report.
///
/// Refuses in both directions, and refuses an undeclared pattern outright.
pub fn debt_verdict(report: &GateReport, ledger: &[SystemicBypassAllowance]) -> DebtVerdict {
    let mut faults = Vec::new();

    for (pattern, _kernel, _owning) in KERNEL_REGISTRY {
        let measured = report
            .violations
            .iter()
            .filter(|bypass| bypass.pattern == *pattern)
            .count();
        let row = ledger.iter().find(|row| row.pattern == *pattern);
        match row {
            None => {
                if measured > 0 {
                    faults.push(DebtFault::Undeclared {
                        pattern: (*pattern).to_owned(),
                        measured,
                    });
                }
            }
            Some(row) if row.ceiling == 0 => faults.push(DebtFault::EmptyRow {
                pattern: (*pattern).to_owned(),
                owner: row.owner.to_owned(),
            }),
            Some(row) if measured > row.ceiling => faults.push(DebtFault::NewBypass {
                pattern: (*pattern).to_owned(),
                owner: row.owner.to_owned(),
                measured,
                ceiling: row.ceiling,
            }),
            Some(row) if measured < row.ceiling => faults.push(DebtFault::CeilingHasSlack {
                pattern: (*pattern).to_owned(),
                owner: row.owner.to_owned(),
                measured,
                ceiling: row.ceiling,
            }),
            Some(_) => {}
        }
    }

    DebtVerdict { faults }
}
/// Comment handling lives in `text_structure::code_only`, not here.
///
/// Two hand matchers lived in this spot until 2026-09-11 (`blank_block_comments`,
/// `strip_line_comment`, ~80 lines of byte-loop comment scanning). They were deleted
/// when the gate routed through `code_only`, for the reason the lint now enforces:
/// every local comment-stripper is a second comment grammar that drifts from the
/// first. `code_only` blanks line and nested block comments while preserving string
/// literals (the `Command::new` units live inside them) and line numbers. A `/*`
/// inside a `//` comment cannot open a block; a `//` inside a string cannot close
/// code. The legs that pinned the old matchers now pin the routing instead.

/// Determine which crate a file belongs to, from a path containing `crates/<name>/`.
///
/// Takes the LAST `crates` component so an absolute path under a scratch checkout
/// (`…/head-tree/crates/foo/src/lib.rs`) resolves the same as a repo-relative one. The
/// previous shape required `crates` to be component ZERO, which is why the workspace scan
/// could not resolve an owning crate at all and dropped the allowlist entirely.
pub fn owning_crate(path: &str) -> Option<String> {
    let components: Vec<&str> = path.split('/').collect();
    let mut found = None;
    for (index, component) in components.iter().enumerate() {
        if *component == "crates" {
            if let Some(next) = components.get(index + 1) {
                found = Some((*next).to_owned());
            }
        }
    }
    found
}

/// Check a single file's source text for kernel bypasses.
///
/// Returns violations for lines that match a kernel's raw pattern where the
/// file's owning crate is NOT the kernel's owning crate (the allowlist).
/// Source is routed through `text_structure::code_only` before matching:
/// comments are blanked with offsets preserved, string literals are kept (the
/// execution units live inside them). Line numbers are read off the blanked
/// text, which carries every newline in place, so they match the source.
pub fn lint_source(file: &str, source: &str) -> Vec<Bypass> {
    let crate_name = owning_crate(file);
    let code = code_only(source);
    let code = code.as_ref();
    let mut violations = Vec::new();
    for (pattern, kernel, owning) in KERNEL_REGISTRY {
        for (start, _) in code.match_indices(pattern) {
            let is_owning = crate_name.as_deref() == Some(*owning);
            if !is_owning {
                violations.push(Bypass {
                    file: file.to_owned(),
                    line: code[..start].matches('\n').count() + 1,
                    pattern: (*pattern).to_string(),
                    kernel: (*kernel).to_string(),
                });
            }
        }
    }
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    violations
}

/// Scan a directory tree recursively for `.rs` files and lint each one.
pub fn lint_tree(root: &Path, skip_dirs: &[&str]) -> GateReport {
    let mut scanned = Vec::new();
    let mut violations = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if !skip_dirs.contains(&name) {
                    stack.push(path);
                }
                continue;
            }
            if !path.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            scanned.push(rel.clone());
            let source = std::fs::read_to_string(&path).unwrap_or_default();
            for bypass in lint_source(&rel, &source) {
                violations.push(bypass);
            }
        }
    }

    scanned.sort();
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    GateReport {
        scanned,
        violations,
    }
}

/// Scan `<root>/crates/*/src/` — the workspace variant CI invokes.
///
/// Routes every file through [`lint_source`], which is the function the specimen legs in
/// `tests/kernel_bypass.rs` exercise. It previously had a private near-duplicate matcher
/// that discarded the registry's owning-crate field, so the ALLOWLIST WAS NOT ENFORCED on
/// the only path CI ran: the tested path honoured the allowlist and the shipped path did
/// not. The dead-code warning on the unused allowlist resolver was in every CI log.
pub fn lint_workspace(root: &Path) -> GateReport {
    let crates_dir = root.join("crates");
    let mut scanned = Vec::new();
    let mut violations = Vec::new();

    let entries = match std::fs::read_dir(&crates_dir) {
        Ok(entries) => entries,
        Err(_) => {
            // Missing and empty crates/ are the same anti-vacuous input: no files were scanned.
            return GateReport {
                scanned: Vec::new(),
                violations: Vec::new(),
            };
        }
    };

    for entry in entries.flatten() {
        let src = entry.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut stack = vec![src.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if !path.extension().is_some_and(|ext| ext == "rs") {
                    continue;
                }
                let display = path.display().to_string();
                scanned.push(display.clone());
                let source = std::fs::read_to_string(&path).unwrap_or_default();
                violations.extend(lint_source(&display, &source));
            }
        }
    }

    scanned.sort();
    violations.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    GateReport {
        scanned,
        violations,
    }
}
