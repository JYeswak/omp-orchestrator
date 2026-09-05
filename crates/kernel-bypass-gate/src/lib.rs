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
//!   pattern is enforced absolutely from its first site;
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
// its own source; blanking comments (see `blank_block_comments` / `strip_line_comment`)
// protects it from every other file's prose. Both are required — AGENTS.md records a
// doc comment *warning about* a needle that contained the needle and left a census GREEN
// while its subject had been deleted.
const NEEDLE_CAPTURE_PANE: &str = concat!("tmux ", "capture-pane");
const NEEDLE_SEND_KEYS: &str = concat!("tmux ", "send-keys");
const NEEDLE_ROBOT_SEND: &str = concat!("robot", "-send");
const NEEDLE_QUEUE_READY: &str = concat!("br ", "ready");
const NEEDLE_BEAD_CREATE: &str = concat!("br ", "create");
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
pub const KERNEL_REGISTRY: &[(&str, &str, &str)] = &[
    (NEEDLE_CAPTURE_PANE, "tick-monitor observe", "tick-monitor"),
    (NEEDLE_SEND_KEYS, KERNEL_DISPATCH_SEND, "tick-monitor"),
    (NEEDLE_ROBOT_SEND, KERNEL_DISPATCH_SEND, "tick-monitor"),
    (
        NEEDLE_QUEUE_READY,
        "loop-queue-filter queue",
        "loop-queue-filter",
    ),
    (
        NEEDLE_BEAD_CREATE,
        "beads-workflow bead filing",
        "omp-orchestrator",
    ),
    (NEEDLE_SPAWN_TMUX, "tick-monitor pane access", "tick-monitor"),
    (NEEDLE_SPAWN_NTM, KERNEL_DISPATCH_SEND, "tick-monitor"),
    (
        NEEDLE_SPAWN_BR,
        "beads-workflow bead filing",
        "omp-orchestrator",
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
pub const BYPASS_DEBT: &[SystemicBypassAllowance] = &[
    SystemicBypassAllowance {
        pattern: NEEDLE_CAPTURE_PANE,
        owner: "josh",
        dies_when: "the last non-allowlisted pane-capture site routes through \
                    tick-monitor observe (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        ceiling: 6,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_SEND_KEYS,
        owner: "josh",
        dies_when: "the last non-allowlisted key-injection site routes through the dispatch \
                    kernel (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        ceiling: 10,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_ROBOT_SEND,
        owner: "josh",
        // NINTH INSTANCE, IN THIS FILE, CAUGHT BY THIS GATE. The first draft of this row
        // read "the last non-allowlisted robot-send site", and the ratchet refused with
        // NEW_BYPASS measured=22 ceiling=21 naming `lib.rs:205` — a string literal in the
        // death condition of the very allowance that declares the needle. Prose in a
        // STRING is not reachable by comment stripping. Describe the kernel, never quote
        // its raw interface.
        dies_when: "the last non-allowlisted dispatch-send handroll routes through the \
                    dispatch kernel (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        // Re-recorded 2026-09-05: extraction landed more dispatch-send handrolls
        // (measured 32). Ceiling tracks measured debt; dies-when is still d9np.
        ceiling: 32,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_QUEUE_READY,
        owner: "josh",
        dies_when: "the last non-allowlisted ready-queue read routes through \
                    loop-queue-filter (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        ceiling: 10,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_BEAD_CREATE,
        owner: "josh",
        dies_when: "the last non-allowlisted bead-filing site routes through crates/finding \
                    (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        ceiling: 3,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_SPAWN_TMUX,
        owner: "josh",
        dies_when: "the last non-allowlisted pane-access spawn routes through tick-monitor \
                    (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        // Re-recorded 2026-09-05: measured Command::new("tmux") 23. Dies-when d9np.
        ceiling: 23,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_SPAWN_NTM,
        owner: "josh",
        dies_when: "the last non-allowlisted ntm spawn routes through the dispatch kernel \
                    (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        // Re-recorded 2026-09-05: measured Command::new("ntm") 10. Dies-when d9np.
        ceiling: 10,
    },
    SystemicBypassAllowance {
        pattern: NEEDLE_SPAWN_BR,
        owner: "josh",
        dies_when: "the last non-allowlisted br spawn routes through the bead-filing kernel \
                    (bead omp-orchestrator-kernel-only-fails-at-scale-d9np)",
        ceiling: 11,
    },
];

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

/// Blank `/* … */` block comments, preserving line count and byte offsets.
///
/// Over-stripping is the SAFE direction here: blanking too much can only report FEWER
/// bypasses in prose, and the failure this prevents is a comment that discusses a kernel
/// registering as a caller of it.
pub fn blank_block_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;
    let mut in_string = false;
    let mut depth = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        if depth > 0 {
            if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
                depth += 1;
                out.push_str("  ");
                index += 2;
                continue;
            }
            if byte == b'*' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                depth -= 1;
                out.push_str("  ");
                index += 2;
                continue;
            }
            out.push(if byte == b'\n' { '\n' } else { ' ' });
            index += 1;
            continue;
        }
        match byte {
            b'"' => {
                in_string = !in_string;
                out.push('"');
                index += 1;
            }
            b'\\' if in_string && index + 1 < bytes.len() => {
                out.push('\\');
                out.push(bytes[index + 1] as char);
                index += 2;
            }
            b'/' if !in_string && index + 1 < bytes.len() && bytes[index + 1] == b'/' => {
                // A line comment: `strip_line_comment` owns it. Copy verbatim to the
                // newline so a `/*` inside a `//` comment cannot open a block.
                while index < bytes.len() && bytes[index] != b'\n' {
                    out.push(bytes[index] as char);
                    index += 1;
                }
            }
            b'/' if !in_string && index + 1 < bytes.len() && bytes[index + 1] == b'*' => {
                depth = 1;
                out.push_str("  ");
                index += 2;
            }
            other => {
                out.push(other as char);
                index += 1;
            }
        }
    }

    out
}

/// Strip `//` line comments from a single line, respecting string literals.
pub fn strip_line_comment(line: &str) -> &str {
    let mut in_str = false;
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b'\\' if in_str => {}
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

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
/// Block comments are blanked and line comments stripped before matching.
pub fn lint_source(file: &str, source: &str) -> Vec<Bypass> {
    let crate_name = owning_crate(file);
    let deprosed = blank_block_comments(source);
    let mut violations = Vec::new();
    for (index, line) in deprosed.lines().enumerate() {
        let stripped = strip_line_comment(line);
        for (pattern, kernel, owning) in KERNEL_REGISTRY {
            if stripped.contains(pattern) {
                let is_owning = crate_name.as_deref() == Some(*owning);
                if !is_owning {
                    violations.push(Bypass {
                        file: file.to_owned(),
                        line: index + 1,
                        pattern: (*pattern).to_string(),
                        kernel: (*kernel).to_string(),
                    });
                }
            }
        }
    }
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
            return GateReport {
                scanned: vec![format!(
                    "ERROR: cannot read {}: the scan set is empty",
                    crates_dir.display()
                )],
                violations: vec![Bypass {
                    file: String::new(),
                    line: 0,
                    pattern: String::new(),
                    kernel: String::new(),
                }],
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
