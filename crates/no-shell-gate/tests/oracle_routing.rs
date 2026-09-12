//! ORACLE ROUTING GATE — a crate that reads BOTH the tmux surface and the ntm surface
//! must route its comparison through `oracle-compare`, or carry a named allowance with
//! a reason.
//!
//! # What this enforces
//!
//! For every crate whose `src/` invokes both surfaces:
//!
//! * its manifest declares `oracle-compare`, AND
//! * its sources name at least one of `oracle-compare`'s COMPARISON symbols —
//!   not merely one of its subprocess helpers (see [`COMPARISON_SYMBOLS`]);
//! * or it appears in [`UNROUTED_ALLOWANCE`] with a reason.
//!
//! [`UNROUTED_ALLOWANCE`] is checked in BOTH directions. A dual-surface reader that is
//! neither routed nor declared fails; a declared crate that has since STARTED routing
//! also fails, with a message telling the reader to delete the row. An allowance that
//! outlives its defect is how a repaired gap keeps reading as broken, and it is what
//! stops this list from shrinking.
//!
//! # Why a comparison symbol and not just the dependency edge
//!
//! A `Cargo.toml` edge is NAMED; calling the comparator is EXECUTED, and the difference
//! is not academic. Measured 2026-09-02 across both checkouts:
//!
//! ```text
//!   crate                            dependency  comparator calls  reads both surfaces
//!   control-plane/fleet-arc-report   yes         0                 yes
//!   control-plane/arc-checkin        yes         0                 no (ntm only)
//!   omp-orchestrator/tick-dispatch   yes         0                 no (ntm only)
//! ```
//!
//! All three call `oracle_compare::spawn_timeout` / `spawn_timeout_stdin` — the crate's
//! bounded-subprocess helpers — and no comparator at all. A dependency-edge gate scores
//! every one of them as routed. `fleet-arc-report` is the damning row: it reads both
//! surfaces, handrolls the comparison, and a manifest probe certified it as fixed.
//!
//! # What still passes, and what a green run does NOT establish
//!
//! * **A crate that depends on `oracle-compare`, names a comparison symbol somewhere,
//!   and STILL handrolls its real comparison beside it passes.** Proving every
//!   comparison site routes needs per-site dataflow analysis, which is unbuilt. This
//!   gate raises the floor from *no relationship at all* to *declared relationship plus
//!   a comparator in the source, or a stated reason*.
//! * **A green run does not establish that the allowed crates SHOULD route.** Some read
//!   two surfaces for reasons that are not a comparison — `kernel-only-operator-hook`
//!   pattern-matches command lines, it does not reconcile state. The allowance makes
//!   each such choice VISIBLE and reasoned; it does not make the choice.
//! * **This gate reads the WORKING TREE, not `HEAD`.** In a shared checkout with
//!   concurrent agents a green result can reflect a sibling's in-flight edit rather
//!   than committed state. That hole produced three wrong readings of this very crate
//!   set in one hour on 2026-09-02, in both directions — a crate credited with a
//!   dependency a sibling had just added, and a rewrite reverted as if it were noise.
//!   The crate ROSTER is taken from `git ls-files`, so an untracked scratch crate is
//!   not scanned; the CONTENTS are whatever is on disk.
//! * The scan covers `crates/<name>/src/**/*.rs` only. This file lives in `tests/`, so
//!   the gate does not match itself — which matters, because its own source names every
//!   crate it checks and both surface tokens. [`the_scan_does_not_include_its_own_source`]
//!   pins that.
//!
//! # Precedent
//!
//! Shape follows `spawn_contract.rs` in this crate, and the bidirectional-allowance and
//! executed-vs-named discipline follows `franken_lean`'s
//! `fln-conformance/tests/contract_roots.rs`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

// ONE implementation of the "which tree is this and may I adjudicate it" vocabulary,
// borrowed rather than re-derived: `omp-inventory-map` is already a path dependency of
// this crate, and a second copy of the doctrine is the drift defect `hook_digest` was
// consolidated to avoid.
use omp_inventory_map::types_inventory::{binding_environment, CensusSource};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// Symbols that mean the crate USES the shared comparator.
///
/// `spawn_timeout`, `spawn_timeout_stdin` and `child_cannot_open_fd` are deliberately
/// absent: they are `oracle-compare`'s bounded-subprocess helpers, and a crate can call
/// them while handrolling every comparison it makes. Three crates measurably do.
const COMPARISON_SYMBOLS: &[&str] = &[
    "compare_counts",
    "compare_sets",
    "harvest_verdict",
    "harvest_expect",
    "set_delta",
    "OracleCompareVerdict",
    "OracleCompareRules",
    "OracleCompareRule",
    "CountArm",
    "SetArm",
];

/// Evidence that a crate invokes one of the two surfaces, per surface.
///
/// Four forms, and the mask differs BY FORM because the evidence differs by form:
///
/// Read from [`text_structure::code_only`] — comments masked, STRING LITERALS KEPT, because an
/// argv literal is itself the signal:
///
/// * the exact string literal `"tmux"` / `"ntm"` — the argv form, however the spawn is
///   wrapped (`Command::new`, `PathBuf::from`, a helper taking `&str`);
/// * the `TMUX_BIN` / `NTM_BIN` environment override;
/// * the `tmux_count` / `ntm_count` derived field, which is how a crate reads a surface
///   second-hand out of another lane's JSON. `loop-tick` reaches both surfaces only
///   this way and would otherwise be invisible.
///
/// Read from [`text_structure::code_and_literals`] — comments AND literal bodies masked,
/// because here a bare occurrence inside a message is prose:
///
/// * the complete identifier `TMUX` / `NTM`, which is the SHARED KERNEL CONSTANT
///   `tick_monitor::TMUX` / `tick_monitor::NTM` (`crates/tick-monitor/src/kernel.rs:7,10`,
///   re-exported at `src/lib.rs:31`). Identifier boundaries, so it does not double-count
///   `TMUX_BIN` and does not match `TMUX_TMPDIR`, which names a socket directory and is not
///   an invocation.
///
/// # Why the fourth form was added, measured 2026-09-11
///
/// **The surfaces were centralised into a shared constant and this recogniser went blind to
/// every crate that adopted it.** Keying on the literal `"tmux"` scored `pane-oracle-diff` at
/// `tmux=0 ntm=0` and `oracle-pane-state-differential` at `tmux=1 ntm=0` — the two crates
/// [`the_crates_that_route_are_recognised_as_routing`] names as THE reference implementations
/// of routing, both of which spawn `Command::new(tick_monitor::TMUX)` and
/// `Command::new(tick_monitor::NTM)`. Five allowance rows (`fleet-truth`, `fleet-monitor`,
/// `fast-dispatch`, `omp-idle-dispatch`, `tick-dispatch`) simultaneously read as STALE for the
/// same reason, and [`no_allowance_row_outlives_the_defect_it_records`] said to delete them.
/// **Deleting them would have removed five live DEBT exemptions for crates that do read both
/// surfaces** — the recogniser was blind, the rows were not stale. This is the same
/// scan-set defect `exit_codes` paid for when `git grep -l 'ExitCode|process::exit'` omitted
/// the one file that declares `EXIT_CANNOT_OBSERVE = 78`: a census narrower than the thing it
/// counts reports a clean bill.
///
/// The exactness of the first three forms remains load-bearing and is NOT relaxed here. A bare
/// substring search for `ntm` matches prose in assertion messages, `NO_CLAIM` strings, and
/// other crates' names in specimen tables; measured on this tree it produced six false
/// dual-surface readers. One of those six was `pane-truth` — and on the constant form
/// `pane-truth` is now a TRUE reader (`src/lib.rs:669,675,722` spawn `tick_monitor::TMUX`,
/// `:207` resolves `ntm`), so the old note was right about the instrument and wrong as a
/// standing claim about the crate. Its documentation says NTM LABELS are never consulted,
/// which is a narrower claim than never invoking `ntm`.
fn surface_evidence(source: &str, binary: &str) -> usize {
    let argv = text_structure::code_only(source);
    let literal = format!("\"{binary}\"");
    let env_override = format!("{}_BIN", binary.to_uppercase());
    let derived = format!("{binary}_count");
    let named = argv.matches(&literal).count()
        + argv.matches(&env_override).count()
        + argv.matches(&derived).count();
    let masked = text_structure::code_and_literals(source);
    named + text_structure::identifier_occurrences(&masked, &binary.to_uppercase())
}

/// Strip `//` and `/* */` comments while KEEPING string literals.
///
/// Keeping strings is not an oversight: the surface names live inside argv literals, so
/// stripping strings destroys the signal entirely. Stripping comments is equally
/// required — six gates in this repo have matched their own prose about the pattern
/// they were hunting.
fn strip_comments(source: &str) -> String {
    text_structure::code_only(source).into_owned()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Routing {
    /// Declares the dependency AND names a comparator.
    Routed,
    /// Declares the dependency and names NO comparator — the executed-vs-named gap.
    DeclaredOnly,
    /// No relationship at all.
    Handroll,
}

fn routing_of(manifest: &str, code: &str) -> Routing {
    if !manifest.contains("oracle-compare") {
        return Routing::Handroll;
    }
    if COMPARISON_SYMBOLS.iter().any(|s| code.contains(s)) {
        Routing::Routed
    } else {
        Routing::DeclaredOnly
    }
}

/// Crate roster from the COMMIT, never a hand list and never the index.
///
/// # Why `ls-tree HEAD` and not `ls-files`, changed 2026-09-12
///
/// This read `git ls-files`, which is the INDEX — a mutable local artifact that is not
/// the thing CI sees. Measured across the four rch workers on 2026-09-11, the index is
/// not even consistently present, let alone faithful: `.git` ABSENT on one box, PRESENT
/// with an UNBORN `HEAD` on another, and PRESENT-but-FOSSIL on the rest — 85 / 333 / 0 /
/// 86 tracked paths against 1136+ locally. The leg below had already recorded the
/// symptom ("on the rch worker the same leg on the same commit reported 85/94") and
/// attributed it to "the worker's index is not a faithful mirror"; that diagnosis was
/// right and the instrument was never changed to match it.
///
/// The COMMIT is the correct surface by SUBJECT, not merely the more robust one: the
/// question this roster answers is "which crates can CI see", and CI checks out a
/// commit. A crate that is staged but uncommitted is genuinely invisible to CI, and a
/// commit-derived roster says so where an index-derived one hides it.
///
/// A failure is TYPED, never an empty vector. An empty roster silently turns every
/// consumer into a vacuous scan — on a gitless box `dual_surface_readers` returned an
/// empty map and the legs over it passed while measuring nothing.
fn committed_crates(root: &Path) -> Result<(Vec<String>, String), CensusSource> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| format!("cannot run `git {}`: {error}", args.join(" ")))?;
        if !out.status.success() {
            return Err(format!(
                "`git {}` failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };

    let rev = match git(&["rev-parse", "HEAD"]) {
        Ok(text) => text.trim().to_owned(),
        Err(reason) => return Err(CensusSource::Worktree { reason }),
    };
    let listing = match git(&["ls-tree", "-r", "--name-only", &rev, "--", "crates"]) {
        Ok(text) => text,
        Err(reason) => return Err(CensusSource::Worktree { reason }),
    };

    let mut names: Vec<String> = listing
        .lines()
        .filter_map(|line| line.strip_prefix("crates/"))
        .filter_map(|rest| rest.strip_suffix("/Cargo.toml"))
        // Fixture manifests nested under a crate's `tests/` are not workspace members
        // and must not inflate the roster (4 of them on 2026-09-05).
        .filter(|name| !name.contains('/'))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(CensusSource::Worktree {
            reason: format!(
                "commit {} lists no crates/<name>/Cargo.toml — an empty roster is an ERROR, \
                 never a clean bill",
                &rev[..rev.len().min(12)]
            ),
        });
    }
    Ok((names, rev))
}

/// The roster, or a NAMED UNKNOWN — the one place this file decides whether a box may
/// speak about the repository at all.
///
/// # Why every consumer must ask, not just the roster leg
///
/// `tracked_crates` returning an empty vector made every downstream scan VACUOUS, and
/// the anti-vacuity guards below then fired with "the scan is broken — most likely
/// `git ls-files` failed". They were right about the mechanism and wrong about the
/// class: on a box that cannot name a commit the scan is not broken, it is
/// UNPERFORMED, and those two demand opposite responses. Measured 2026-09-12 on
/// contabo-4: three legs RED with anti-vacuity messages, cause
/// `git rev-parse HEAD -> ambiguous argument 'HEAD'`. A red that no edit to this
/// repository can clear is the shape that gets routed around.
///
/// IN THE ORACLE it stays fatal. CI checks out a commit, so a CI that cannot read one
/// is a broken checkout, and passing there is how the only box that adjudicates these
/// legs stops adjudicating them.
fn roster_or_unmeasured(root: &Path, leg: &str) -> Option<(Vec<String>, String)> {
    match committed_crates(root) {
        Ok(roster) => Some(roster),
        Err(source) => {
            let reason = source
                .blocked_reason()
                .expect("a failed roster carries its reason")
                .to_owned();
            assert!(
                binding_environment().is_none(),
                "{} is the oracle for {leg} and it could not read the commit: {reason} \
                 -- fix the checkout, never the assertion",
                binding_environment().unwrap_or_default()
            );
            eprintln!(
                "ROSTER UNMEASURED leg={leg} reason={reason} -- the crate roster comes from \
                 the commit and this box cannot name one, so every verdict below it would be \
                 a statement about the environment"
            );
            None
        }
    }
}

/// The roster, or an empty one — for consumers that measure a PROPERTY OF EACH CRATE
/// rather than the roster itself.
///
/// Callers MUST carry their own anti-vacuity guard; the typed cause is available from
/// [`committed_crates`] and is what [`roster_or_unmeasured`] reports.
fn tracked_crates(root: &Path) -> Vec<String> {
    committed_crates(root)
        .map(|(names, _)| names)
        .unwrap_or_default()
}

/// Every `crates/<name>/src/**/*.rs`, sorted by path. Deliberately NOT `tests/`.
///
/// # Why this is a list and the order is fixed, measured 2026-09-11
///
/// It concatenated the files in `read_dir` order and every caller masked the JOINED text.
/// Both halves of that are unsound, and together they produced a verdict that flipped
/// between machines on one commit:
///
/// * **Masking a concatenation lets one file blank the next.** `code_only` and
///   `code_and_literals` blank an unterminated `/*` to the end of their INPUT.
///   `crates/omp-orchestrator/src/input_closure.rs` holds 4 `/*` against 1 `*/` and
///   `src/lib.rs` holds 5 against 3 — legitimately, inside string literals and specimen
///   text — so once the surrounding parse is misaligned by a lifetime apostrophe, a runaway
///   block comment blanks every file joined after it.
/// * **`read_dir` order is not defined**, so WHICH files get blanked depends on the
///   filesystem. Measured on commit `7c90fbd`: `omp-orchestrator` scored `tmux=15 ntm=6` on
///   APFS and `tmux=0 ntm=0` on the rch worker over a byte-identical 786,891-byte scan set,
///   which made `no_allowance_row_outlives_the_defect_it_records` demand the deletion of a
///   live allowance row on the lane and not on the Mac. A crate that spawns
///   `tick_monitor::TMUX` fourteen times read as touching neither surface.
///
/// Per-file masking is what `exit_codes::scan` already does, and it is immune by
/// construction: a construct unterminated in one file cannot reach another. Sorting makes
/// the census reproducible rather than merely correct on this laptop.
fn crate_source_files(root: &Path, name: &str) -> Vec<String> {
    let mut texts: Vec<(PathBuf, String)> = Vec::new();
    let mut stack = vec![root.join("crates").join(name).join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                texts.push((path, text));
            }
        }
    }
    texts.sort_by(|left, right| left.0.cmp(&right.0));
    texts.into_iter().map(|(_, text)| text).collect()
}

/// The same files joined, in the same fixed order. For callers that need one blob.
fn crate_sources(root: &Path, name: &str) -> String {
    let mut body = String::new();
    for text in crate_source_files(root, name) {
        body.push_str(&text);
        body.push('\n');
    }
    body
}

/// Surface evidence for a whole crate, masked PER FILE and summed. Never over the join.
fn crate_surface_evidence(root: &Path, name: &str, binary: &str) -> usize {
    crate_source_files(root, name)
        .iter()
        .map(|text| surface_evidence(text, binary))
        .sum()
}

#[derive(Debug, Clone)]
struct Reader {
    tmux: usize,
    ntm: usize,
    routing: Routing,
}

/// Every tracked crate that invokes BOTH surfaces, with how it compares them.
fn dual_surface_readers(root: &Path) -> BTreeMap<String, Reader> {
    let mut readers = BTreeMap::new();
    for name in tracked_crates(root) {
        // Both scans mask PER FILE. `routing_of` gets the per-file-stripped texts rejoined,
        // not a stripped join: a runaway `/*` in one file must not be able to blank a
        // COMPARISON_SYMBOL in another. See `crate_source_files` for the measurement.
        let tmux = crate_surface_evidence(root, &name, "tmux");
        let ntm = crate_surface_evidence(root, &name, "ntm");
        if tmux == 0 || ntm == 0 {
            continue;
        }
        let code = crate_source_files(root, &name)
            .iter()
            .map(|text| strip_comments(text))
            .collect::<Vec<String>>()
            .join("\n");
        let manifest = std::fs::read_to_string(root.join("crates").join(&name).join("Cargo.toml"))
            .unwrap_or_default();
        readers.insert(
            name,
            Reader {
                tmux,
                ntm,
                routing: routing_of(&manifest, &code),
            },
        );
    }
    readers
}

/// Dual-surface readers permitted to compare without the shared kernel, each with the
/// reason. SEEDED FROM THIS GATE'S OWN SCAN of this workspace on 2026-09-02 — 12
/// dual-surface readers, 3 routed, 9 allowed below. (The seeding note named the author's
/// absolute checkout path until 2026-09-11; `path-literal-guard` refused it, correctly —
/// a home path in a gate's doctrine is exactly the literal that makes a gate unportable.)
///
/// Seeding a ceiling from a neighbouring measurement is how a mutation probe passed at
/// 42 when the tree held 41. Every row here was produced by
/// [`dual_surface_readers`] itself, in this checkout.
///
/// Adding a row is cheap and auditable; leaving one out is a build failure, and so is
/// leaving one in after the crate starts routing. That asymmetry is the point.
///
/// **Re-seeded 2026-09-11, from the same function, after [`surface_evidence`] learned the
/// shared-kernel constant form: 17 dual-surface readers, 3 routed, 14 allowed below.** The
/// census grew by 5 and NOT because crates changed — the recogniser had been blind to
/// `tick_monitor::TMUX` / `NTM`. Four rows are new (`tick-monitor`, `pane-truth`,
/// `ompo-doctor`, `receiver-receipt`); the other five reappeared under rows that were about
/// to be deleted as stale. Each new row names WHY it is not a comparison and what kills it,
/// because a row with only a crate name in it is an exemption nobody can recheck.
const UNROUTED_ALLOWANCE: &[(&str, &str)] = &[
    (
        "fleet-reconcile",
        "THE MOST DAMNING ROW and the one to fix next: its entire documented purpose is \
         'NTM vs tmux ground-truth compare'. The dedicated reconciliation crate does not \
         use the reconciliation kernel. 9 tmux / 10 ntm sites. This allowance is DEBT, \
         not a decision",
    ),
    (
        "loop-tick",
        "PROVISIONAL: reaches both surfaces second-hand via fleet-truth's tmux_count / \
         ntm_count JSON and decides a verdict from them. That is a comparison, and it is \
         handrolled — but converting it means changing what fleet-truth emits, so it is \
         blocked on fleet-reconcile landing first",
    ),
    (
        "fleet-truth",
        "PROVISIONAL: the ground-truth inspection register, and the producer of the \
         tmux_count / ntm_count pair loop-tick compares. It is the natural place for the \
         kernel to live and should convert alongside fleet-reconcile",
    ),
    (
        "fleet-monitor",
        "PROVISIONAL: the OBSERVE lane and the only scheduled writer of the standing \
         admission verdict, so a wrong comparison here silently gates every dispatch. \
         1 tmux / 1 ntm site — small surface, large blast radius",
    ),
    (
        "fast-dispatch",
        "PROVISIONAL: admits on a fresh standing PASS and selects FREE panes, so it \
         inherits fleet-monitor's verdict rather than forming its own. Convert after \
         fleet-monitor, or it will be routed through a kernel fed by a handrolled input",
    ),
    (
        "omp-idle-dispatch",
        "PROVISIONAL: the fail-closed idle dispatch lane — the same shape as \
         refill-idle-panes, which converted this session. This is the strongest \
         candidate for the SECOND conversion and the row most likely to be wrong",
    ),
    (
        "loop-driver",
        "PROVISIONAL: a deadline-bounded driver for the loop tick. It spawns both \
         binaries and checks they exist; whether it COMPARES their outputs has not been \
         established, so this row records an unexamined crate rather than a decision",
    ),
    (
        "omp-orchestrator",
        "PROVISIONAL: the resident supervisor. 8 tmux / 2 ntm sites across an async \
         invoke() layer whose comparison semantics are not obviously the kernel's \
         synchronous ones. Needs its own bead, not a blanket conversion",
    ),
    (
        "kernel-only-operator-hook",
        "LIKELY CORRECT AS-IS: a PreToolUse hook that pattern-matches operator COMMAND \
         LINES for kernel bypasses. It names both binaries because it forbids calling \
         them directly; it never reconciles two observations, so there is nothing for a \
         comparator to compare",
    ),
    (
        "tick-dispatch",
        "DECLARED-ONLY: the crate lists oracle-compare so dual-surface reads are named, \
         but src/ never calls a COMPARISON_SYMBOL — spawn helpers only. Same \
         executed-vs-named gap as the gate's own docs. Dies when tick-dispatch/src \
         names a COMPARISON_SYMBOL, at which point this row must be deleted",
    ),
    (
        "tick-monitor",
        "LIKELY CORRECT AS-IS: it is the crate that DECLARES both surface names — \
         `pub const TMUX` / `pub const NTM` at src/kernel.rs:7,10, re-exported at \
         src/lib.rs:31. Every other row in this table reads the surfaces THROUGH it. Its own \
         src/main.rs:136,162 spawns tmux capture-pane only and never spawns ntm, so there is \
         no second observation for a comparator to reconcile. Dies when tick-monitor/src \
         invokes `ntm` itself rather than just naming it for others",
    ),
    (
        "pane-truth",
        "PROVISIONAL, and this row corrects a standing claim: the gate's own header cited \
         pane-truth as a FALSE dual-surface reader, which was true of the substring \
         instrument and is not true of the crate. It resolves `ntm` through \
         PANE_TRUTH_NTM_BIN (src/lib.rs:207) and spawns `tick_monitor::TMUX` \
         (src/lib.rs:669,675,722), then PROVENANCE-TAGS each row `tmux` or `ntm` \
         (src/lib.rs:228,623). Tagging a row with where it came from is not reconciling two \
         counts of the same fact, and its documented claim is the narrower one — NTM LABELS \
         are never consulted. Whether the tagged rows are later compared has NOT been \
         established here. Dies when pane-truth compares a tmux census against an ntm census",
    ),
    (
        "ompo-doctor",
        "LIKELY CORRECT AS-IS: PROBES (src/lib.rs:47-56) declares one liveness row per \
         binary — `tmux -V`, `ntm --version` — and each is run and reported on its own. The \
         only deeper reach is ntm-side (src/liveness.rs:212,404); src/liveness.rs contains no \
         tmux reference at all, so the two surfaces are never two readings of one fact. A \
         per-surface probe is a census of AVAILABILITY, not a comparison. Dies when \
         ompo-doctor reads a pane count from both surfaces in one verdict",
    ),
    (
        "receiver-receipt",
        "LIKELY CORRECT AS-IS, and the weakest evidence in the table: the `TMUX` hit is a real \
         capture-pane (src/bin/receiver-receipt.rs:127), while the `NTM` hit is a transport \
         LABEL built inside an in-file `#[cfg(test)]` module (src/irc_delivery.rs:142) and is \
         not an invocation at all. `crate_sources` sweeps src/ including inline test modules, \
         which is deliberate — narrowing it would hide real invocations — so this row records \
         a recogniser limit rather than a crate decision. Dies when receiver-receipt names \
         `ntm` outside a test module",
    ),
];

fn allowed() -> BTreeSet<&'static str> {
    UNROUTED_ALLOWANCE.iter().map(|(c, _)| *c).collect()
}

#[test]
fn every_dual_surface_reader_routes_through_oracle_compare_or_is_allowed() {
    let root = repo_root();
    // The roster comes from the commit, so the SOURCE is settled before any count is
    // believed: an unmeasured roster makes every scan below vacuous, and a vacuous
    // scan must read as UNKNOWN, never as "the scan is broken".
    let Some(_) = roster_or_unmeasured(
        &root,
        "every_dual_surface_reader_routes_through_oracle_compare_or_is_allowed",
    ) else {
        return;
    };
    let readers = dual_surface_readers(&root);
    assert!(
        !readers.is_empty(),
        "ANTI-VACUITY: no crate invokes both tmux and ntm, and the roster IS readable \
         here — so this is the scan breaking, not an environment that cannot name a \
         commit (that case returns above, named). That is the silent-false-zero defect, \
         not a clean bill."
    );
    let allowed = allowed();
    let unrouted: Vec<String> = readers
        .iter()
        .filter(|(name, r)| r.routing != Routing::Routed && !allowed.contains(name.as_str()))
        .map(|(name, r)| {
            let how = match r.routing {
                Routing::DeclaredOnly => {
                    "DECLARES oracle-compare but names NO comparator \
                                          (spawn helpers do not count)"
                }
                _ => "no oracle-compare dependency at all",
            };
            format!("{name} (tmux={} ntm={}): {how}", r.tmux, r.ntm)
        })
        .collect();
    assert!(
        unrouted.is_empty(),
        "{} crate(s) read BOTH the tmux and ntm surfaces and compare them without the \
         shared kernel:\n{:#?}\n\n\
         `oracle-compare` already carries the rules a handrolled comparator keeps \
         omitting: empty_oracle_is_error, unreadable_is_error, \
         empty_product_is_disagreement, disagree_is_finding. A comparator missing them \
         renders 'the arms agree' and 'the probe is broken' identically, and the second \
         one exits 0.\n\n\
         Either route the comparison through oracle-compare, or add a row to \
         UNROUTED_ALLOWANCE with the reason.",
        unrouted.len(),
        unrouted
    );
}

#[test]
fn no_allowance_row_outlives_the_defect_it_records() {
    let root = repo_root();
    let Some(_) = roster_or_unmeasured(&root, "no_allowance_row_outlives_the_defect_it_records")
    else {
        return;
    };
    let readers = dual_surface_readers(&root);
    assert!(
        !readers.is_empty(),
        "ANTI-VACUITY: empty reader set on a box whose roster IS readable — the scan is \
         broken, which is a different finding from an unmeasurable commit"
    );

    let repaired: Vec<&str> = UNROUTED_ALLOWANCE
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| {
            readers
                .get(*c)
                .is_some_and(|r| r.routing == Routing::Routed)
        })
        .collect();
    assert!(
        repaired.is_empty(),
        "{} allowance row(s) name a crate that NOW ROUTES: {:?}\n\
         DELETE THOSE ROWS. An allowance that outlives its defect is how a repaired gap \
         keeps reading as broken, and it is what stops this list from shrinking.",
        repaired.len(),
        repaired
    );

    let roster: BTreeSet<String> = tracked_crates(&root).into_iter().collect();
    let stale: Vec<String> = UNROUTED_ALLOWANCE
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !readers.contains_key(*c))
        .map(|c| {
            // The three causes look identical in a bare name list, so each is measured here.
            if !roster.contains(c) {
                format!("{c}: NOT IN THE GIT-DERIVED ROSTER — never scanned")
            } else {
                let files = crate_source_files(&root, c);
                format!(
                    "{c}: in the roster, {} file(s) / {} byte(s) scanned, tmux={} ntm={}",
                    files.len(),
                    files.iter().map(String::len).sum::<usize>(),
                    crate_surface_evidence(&root, c, "tmux"),
                    crate_surface_evidence(&root, c, "ntm")
                )
            }
        })
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowance row(s) name a crate that no longer reads both surfaces:\n{:#?}\n\n\
         THREE DIFFERENT THINGS LOOK LIKE THIS AND THE NAME ALONE CANNOT TELL THEM APART, \
         which is why each row above carries its measurement:\n\
         (a) THE CRATE STOPPED READING BOTH SURFACES — one surface reads 0 over a plausible \
         byte count. Remove the row: an exemption nobody needs is an exemption nobody \
         rechecks, and it is what stops this list shrinking.\n\
         (b) `surface_evidence` STOPPED SEEING THE INVOCATION — fix the recogniser, not the \
         table. Measured 2026-09-11: five rows read as stale at once (`fleet-truth`, \
         `fleet-monitor`, `fast-dispatch`, `omp-idle-dispatch`, `tick-dispatch`) because the \
         surfaces had been centralised into `tick_monitor::TMUX` / `NTM` and this gate keyed \
         only on the `\"tmux\"` argv literal. Deleting them on that message would have \
         removed five live DEBT exemptions for crates that do read both surfaces.\n\
         (c) THE ENVIRONMENT NEVER SCANNED IT — `NOT IN THE GIT-DERIVED ROSTER`, or a byte \
         count far below what the tree holds. The roster comes from `git ls-files`, and on \
         the rch worker that index is not a faithful mirror of the synced worktree: the same \
         commit that yields 92 tracked crates on the Mac yields 85-86 there. A row is NOT \
         stale because the lane could not see the crate.\n\n\
         THE DISCRIMINATOR: several rows going stale in one run is (b) or (c); a byte count \
         of 0 or a missing roster entry is (c). Read the crate's src/ before deleting a row.",
        stale.len(),
        stale
    );
}

/// KNOWN-GOOD LEG. An attack-only suite ships an over-strict gate, and an over-strict
/// gate gets routed around. The crates that DO route must pass, and there must be some.
#[test]
fn the_crates_that_route_are_recognised_as_routing() {
    let root = repo_root();
    let Some(_) = roster_or_unmeasured(&root, "the_crates_that_route_are_recognised_as_routing")
    else {
        return;
    };
    let readers = dual_surface_readers(&root);
    let routed: Vec<&String> = readers
        .iter()
        .filter(|(_, r)| r.routing == Routing::Routed)
        .map(|(n, _)| n)
        .collect();
    assert!(
        !routed.is_empty(),
        "ANTI-VACUITY on the positive leg: NO dual-surface reader is recognised as \
         routing. A gate that recognises nothing as correct cannot be satisfied, and an \
         unsatisfiable gate is deleted rather than obeyed. `pane-oracle-diff` and \
         `oracle-pane-state-differential` both call compare_* — if they are not here, \
         the recogniser is broken, not the crates."
    );
    for name in ["pane-oracle-diff", "oracle-pane-state-differential"] {
        assert_eq!(
            readers.get(name).map(|r| r.routing),
            Some(Routing::Routed),
            "{name} calls oracle-compare's comparator directly and MUST be recognised as \
             routing; it is the reference implementation of the thing this gate demands"
        );
    }
    assert_eq!(
        readers.get("refill-idle-panes").map(|r| r.routing),
        Some(Routing::Routed),
        "refill-idle-panes was converted for bead omp-orchestrator-oe2 and must read as \
         routed; if it does not, the conversion regressed"
    );
}

/// FIRES ON KNOWN-BAD, and it is the leg whose ABSENCE let the blindness live.
///
/// The three argv-shaped forms each had a case in
/// [`the_surface_predicate_distinguishes_an_invocation_from_prose`]; the shared-kernel
/// constant had none, because it did not exist when that leg was written and nothing failed
/// when the code adopted it. So this pins all four corners of the fourth form: the constant
/// counts, a bare surface name inside an operator MESSAGE does not, a socket-directory
/// variable does not, and the env override is not double-counted by the identifier rule.
///
/// Unlike the four census legs in this file, this one reads no `git` and no worktree, so it
/// is measurable on the rch lane, where a git-derived roster reads a fossil.
#[test]
fn the_shared_kernel_constant_counts_and_a_bare_name_in_a_message_does_not() {
    let via_constant = "let mut c = Command::new(tick_monitor::TMUX);\n\
                        let mut d = Command::new(tick_monitor::NTM);\n";
    assert_eq!(
        surface_evidence(via_constant, "tmux"),
        1,
        "spawning through the shared kernel constant IS an invocation; keying only on the \
         `\"tmux\"` literal scored pane-oracle-diff, the reference implementation of \
         routing, at zero on both surfaces"
    );
    assert_eq!(surface_evidence(via_constant, "ntm"), 1);

    // The real line from crates/pane-oracle-diff/src/main.rs:130.
    let message = "eprintln!(\"  NTM UNDERCOUNTS by {n}: a pane ntm cannot see is a pane \
                   the controller will never dispatch to.\");\n";
    assert_eq!(
        surface_evidence(message, "ntm"),
        0,
        "a bare surface name inside an operator message is PROSE. This is why the constant \
         form reads from `code_and_literals` while the argv forms read from `code_only`: one \
         mask cannot serve both, and a single mask is how six false dual-surface readers \
         were once measured on this tree"
    );

    let socket_dir = "std::env::set_var(\"TMUX_TMPDIR\", home.join(\".tmux-sockets\"));\n";
    assert_eq!(
        surface_evidence(socket_dir, "tmux"),
        0,
        "TMUX_TMPDIR names a socket DIRECTORY, not the binary — the identifier boundary is \
         what keeps it out, and pane-oracle-diff sets it two lines from a real spawn"
    );

    let env_override = "let tmux = std::env::var(\"TMUX_BIN\").unwrap_or_default();\n";
    assert_eq!(
        surface_evidence(env_override, "tmux"),
        1,
        "the env override counts ONCE: `TMUX` must not also match inside `TMUX_BIN`, or \
         every crate using the override would report double evidence"
    );
}

#[test]
fn every_allowance_row_carries_a_reason() {
    let empty: Vec<&str> = UNROUTED_ALLOWANCE
        .iter()
        .filter(|(_, why)| why.trim().len() < 40)
        .map(|(c, _)| *c)
        .collect();
    assert!(
        empty.is_empty(),
        "{} allowance row(s) carry no usable reason: {:?}\n\
         A row without a reason is silence with extra steps.",
        empty.len(),
        empty
    );
    let mut seen = BTreeSet::new();
    for (crate_name, _) in UNROUTED_ALLOWANCE {
        assert!(
            seen.insert(*crate_name),
            "{crate_name} appears twice in UNROUTED_ALLOWANCE; a duplicated row means one \
             of the two reasons is never read"
        );
    }
}

/// FIRES ON KNOWN-BAD. The recogniser must reject a manifest edge with no comparator,
/// which is the exact state three measured crates are in.
#[test]
fn a_dependency_edge_without_a_comparator_is_not_routing() {
    let manifest = "[dependencies]\noracle-compare = { path = \"../oracle-compare\" }\n";
    let spawn_only = "let out = oracle_compare::spawn_timeout_stdin(cmd, DEADLINE, raw)?;";
    assert_eq!(
        routing_of(manifest, spawn_only),
        Routing::DeclaredOnly,
        "calling only the bounded-subprocess helper is NOT routing a comparison — this \
         is control-plane/fleet-arc-report exactly, which a manifest probe certified"
    );
    let comparing = "let v = oracle_compare::compare_sets(oracle, product, &rules);";
    assert_eq!(
        routing_of(manifest, comparing),
        Routing::Routed,
        "and a real comparator call MUST be recognised, or the gate is unsatisfiable"
    );
    assert_eq!(
        routing_of("[dependencies]\nserde_json = \"1\"\n", comparing),
        Routing::Handroll,
        "a comparator symbol without the dependency cannot compile; treat it as absent"
    );
}

/// FIRES ON KNOWN-BAD. The surface predicate must tell an invocation from prose.
#[test]
fn the_surface_predicate_distinguishes_an_invocation_from_prose() {
    let prose = strip_comments(
        "//! Ground-truth tmux pane state. NTM labels are never consulted.\n\
         const NO_CLAIM: &str = \"this lane cannot observe NTM packets or tmux sessions\";\n",
    );
    assert_eq!(
        surface_evidence(&prose, "ntm"),
        0,
        "prose mentioning NTM inside a longer string is not an invocation — this is \
         pane-truth, which a substring search scored as a dual-surface reader"
    );
    assert_eq!(surface_evidence(&prose, "tmux"), 0);

    let invocation = strip_comments(
        "// spawns tmux and ntm\n\
         let mut c = Command::new(\"tmux\");\n\
         let b = PathBuf::from(\"ntm\");\n",
    );
    assert_eq!(
        surface_evidence(&invocation, "tmux"),
        1,
        "an argv literal IS an invocation, however the spawn is wrapped"
    );
    assert_eq!(surface_evidence(&invocation, "ntm"), 1);

    let derived = "let t = v[\"tmux_count\"].as_u64(); let n = v[\"ntm_count\"].as_u64();";
    assert_eq!(
        surface_evidence(derived, "tmux"),
        1,
        "reading a surface second-hand out of another lane's JSON still reads it"
    );
    assert_eq!(surface_evidence(derived, "ntm"), 1);

    let overridden = "let bin = env_or(\"NTM_BIN\", \"/usr/local/bin/ntm-shim\");";
    assert!(
        surface_evidence(overridden, "ntm") >= 1,
        "an env override names the surface even when the default path does not"
    );
}

/// FIRES ON KNOWN-BAD. Comment stripping must remove documentation about the pattern
/// while leaving argv literals intact — and must not be fooled by a `//` inside a string.
#[test]
fn comment_stripping_removes_prose_but_keeps_argv_literals() {
    let stripped = strip_comments(
        "//! This crate never calls \"ntm\".\n\
         /* nor \"tmux\", historically */\n\
         let c = Command::new(\"tmux\");\n",
    );
    assert_eq!(
        surface_evidence(&stripped, "ntm"),
        0,
        "a doc comment containing the literal must not count; six gates in this repo \
         have matched their own documentation"
    );
    assert_eq!(
        surface_evidence(&stripped, "tmux"),
        1,
        "and the one real invocation must survive"
    );
    let url = strip_comments("let u = \"https://example.invalid/ntm\"; // trailing\n");
    assert!(
        url.contains("https://example.invalid/ntm"),
        "a `//` inside a string literal must not start a comment: {url}"
    );
    assert_eq!(surface_evidence(&url, "ntm"), 0);
}

/// The gate must not match itself. Its own source names every crate it checks and both
/// surface tokens; if `tests/` were ever swept in, `no-shell-gate` would appear as the
/// largest dual-surface reader in the tree and the allowance would have to grow a row
/// for the gate itself.
#[test]
fn the_scan_does_not_include_its_own_source() {
    let root = repo_root();
    let this_file = root
        .join("crates")
        .join("no-shell-gate")
        .join("tests")
        .join("oracle_routing.rs");
    assert!(
        this_file.exists(),
        "the gate must be able to find itself to prove exclusion"
    );
    let scanned = crate_sources(&root, "no-shell-gate");
    let own = std::fs::read_to_string(&this_file).expect("this file is readable");
    let needle: String = own.lines().take(1).collect();
    assert!(
        !scanned.contains(&needle),
        "the scan swept crates/no-shell-gate/tests/ — it must cover src/ only"
    );
    assert!(
        !dual_surface_readers(&root).contains_key("no-shell-gate"),
        "no-shell-gate must not appear as a dual-surface reader; if it does, the scan \
         is reading test sources"
    );
}

/// The roster must come from git, and a git failure must be loud rather than empty.
///
/// # Why this compares SETS and prints the difference, changed 2026-09-11
///
/// It compared two integers and said only `left: 85, right: 94`. Two counts can differ for
/// three unrelated reasons and the reader cannot tell which: a crate on disk that is
/// untracked, a tracked crate whose directory is gone, or an environment where `git` does not
/// see the tree the filesystem shows. **All three were live on the same commit.** On the Mac
/// the answer was 92/94 — two complete-but-untracked crate directories, `crates/kernel-only-gate`
/// and `crates/omp-host-tool-guard`, authored 2026-09-08/09 and workspace members via the
/// `crates/*` glob in the root manifest, so they BUILD and are invisible to every git-derived
/// census including CI's. On the rch worker the same leg on the same commit reported 85/94, and
/// no commit in the preceding forty has 85 top-level crate manifests (`HEAD` has 92, the
/// thirty-eight before it have 91) — so the worker's index is not a faithful mirror of the
/// worktree rch synced, and seven more crates vanish from the roster there. A leg whose verdict
/// moves with the environment is measuring the environment; naming the members is what makes
/// that visible instead of arithmetic.
///
/// The assertion is now strictly stronger: equal counts with different members used to pass.
#[test]
fn the_crate_roster_is_derived_from_git_and_covers_the_tree() {
    let root = repo_root();
    let on_disk: BTreeSet<String> = std::fs::read_dir(root.join("crates"))
        .expect("crates/ exists")
        .flatten()
        .filter(|entry| entry.path().join("Cargo.toml").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    // SOURCE BEFORE COUNT, through the same helper every consumer uses. The paragraph
    // above diagnosed an environment whose git does not mirror the worktree it was
    // handed, and then compared the numbers anyway. A box that cannot name a commit
    // has not measured a SMALL roster; it has measured NOTHING.
    let Some((names, rev)) =
        roster_or_unmeasured(&root, "the_crate_roster_is_derived_from_git_and_covers_the_tree")
    else {
        // The disk half is readable everywhere, so it is still held to account:
        // an empty crates/ means neither side of this comparison exists.
        assert!(
            !on_disk.is_empty(),
            "anti-vacuity: crates/ holds no crate on disk either, so neither half of \
             this comparison exists"
        );
        return;
    };
    let tracked: BTreeSet<String> = names.into_iter().collect();
    assert!(
        tracked.len() >= 40,
        "only {} crate manifests in commit {}. A hand list is how a census went stale \
         at 27 while the tree held 51; a roster this small means the read is a lie.",
        tracked.len(),
        &rev[..rev.len().min(12)]
    );
    let untracked: Vec<&String> = on_disk.difference(&tracked).collect();
    let phantom: Vec<&String> = tracked.difference(&on_disk).collect();
    assert!(
        untracked.is_empty() && phantom.is_empty(),
        "the COMMIT-derived roster ({} crates in {}) and the tree on disk ({} crates) name \
         different sets.\n\
         ON DISK BUT NOT IN THE COMMIT ({}): {:?}\n\
           -> a crate the commit does not carry is a crate CI never sees, while it remains a \
              workspace member through the `crates/*` glob and builds locally. Commit it or \
              remove it; do NOT widen this assertion. Staged-but-uncommitted counts here, \
              deliberately: CI checks out the commit, not your index.\n\
         IN THE COMMIT BUT ABSENT FROM DISK ({}): {:?}\n\
           -> a deleted crate whose manifest is still committed, or a worktree that does not \
              match the commit it was checked out from.",
        tracked.len(),
        &rev[..rev.len().min(12)],
        on_disk.len(),
        untracked.len(),
        untracked,
        phantom.len(),
        phantom
    );
}

/// THE ROSTER INSTRUMENT ITSELF, on a repository this test builds — because the arm
/// that matters cannot be reached from the lane.
///
/// Every rch worker measured on 2026-09-11/12 fails `git rev-parse HEAD` at the repo
/// root (`.git` absent on one box, present with an UNBORN `HEAD` on others), so on the
/// lane the four legs above take the UNMEASURED arm and prove nothing about the
/// adjudicating one. A suppression whose non-suppressed path is never exercised is how
/// a gate is switched off by accident. A fixture repo is reachable everywhere, so the
/// discriminating case is measured here rather than assumed.
///
/// THE DISCRIMINATOR IS `staged`: it is in the INDEX and not in the COMMIT. An
/// `ls-files` roster contains it, an `ls-tree HEAD` roster does not, and that single
/// crate is the whole difference between the instrument this file used to have and the
/// one it has now. `untracked` holds the other end: a crate git cannot see at all,
/// which the roster must still exclude, because that exclusion IS the finding the leg
/// above exists to make (`kernel-only-gate` and `omp-host-tool-guard` were found
/// exactly this way).
#[test]
fn the_roster_reads_the_commit_and_names_a_tree_that_has_none() {
    let root = std::env::temp_dir().join(format!("oracle-routing-roster-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let plant = |name: &str| {
        let dir = root.join("crates").join(name);
        std::fs::create_dir_all(dir.join("src")).expect("crate dir");
        std::fs::write(
            dir.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
        )
        .expect("manifest");
        std::fs::write(dir.join("src/lib.rs"), "pub struct Planted;\n").expect("source");
    };
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("`git {}` did not spawn: {e}", args.join(" ")));
        assert!(
            out.status.success(),
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    };

    // A TREE WITH NO COMMIT is the lane's own shape, and it must be a NAMED refusal
    // rather than an empty roster. Checked before `git init` and again after it, so
    // both "no repository" and "repository with an unborn HEAD" are covered — the two
    // distinct failures actually observed on contabo-3 and contabo-1/4.
    plant("committed-crate");
    match committed_crates(&root) {
        Err(source) => assert!(
            source
                .blocked_reason()
                .is_some_and(|reason| !reason.trim().is_empty()),
            "a tree with no git must carry its reason: {source:?}"
        ),
        Ok(roster) => panic!("a tree with no git cannot yield a roster: {roster:?}"),
    }
    git(&["init", "--quiet", "--initial-branch=main"]);
    match committed_crates(&root) {
        Err(source) => assert!(
            source
                .blocked_reason()
                .is_some_and(|reason| reason.contains("HEAD")),
            "an unborn HEAD must be named as such: {source:?}"
        ),
        Ok(roster) => panic!("a repository with no commit cannot yield a roster: {roster:?}"),
    }

    git(&["add", "crates/committed-crate"]);
    git(&[
        "-c",
        "user.name=roster fixture",
        "-c",
        "user.email=roster@fixture.invalid",
        "commit",
        "--quiet",
        "-m",
        "roster fixture [test]",
    ]);
    plant("staged-crate");
    git(&["add", "crates/staged-crate"]);
    plant("untracked-crate");

    let (names, rev) = committed_crates(&root).expect("a committed tree yields a roster");
    assert_eq!(rev.len(), 40, "the roster must name the commit it read: {rev}");
    assert_eq!(
        names,
        vec!["committed-crate".to_owned()],
        "the roster is the COMMIT: `staged-crate` is in the index and not in the commit, so \
         CI cannot see it; `untracked-crate` is in neither. An `ls-files` roster would \
         contain `staged-crate` and that is the exact defect this instrument replaced."
    );

    // And the helper every consumer calls agrees with the function under it.
    let through_helper = roster_or_unmeasured(&root, "fixture")
        .expect("a committed tree is adjudicable")
        .0;
    assert_eq!(through_helper, names, "the helper must not re-derive the roster");

    std::fs::remove_dir_all(&root).expect("fixture cleanup");
}
