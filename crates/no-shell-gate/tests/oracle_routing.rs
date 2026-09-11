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

/// Crate roster from `git ls-files`, never a hand list.
///
/// A hand list is how a census went stale at 27 crates while the tree held 51. Deriving
/// it also means an untracked scratch crate is not scanned, and that a `git` failure
/// yields an EMPTY roster, which the anti-vacuity assertion turns into a hard error
/// rather than a clean bill.
fn tracked_crates(root: &Path) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("ls-files")
        .arg("--")
        .arg("crates/*/Cargo.toml")
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("crates/"))
        .filter_map(|rest| rest.strip_suffix("/Cargo.toml"))
        // `git ls-files crates/*/Cargo.toml` also returns fixture manifests nested
        // under tests/ (4 on 2026-09-05). The disk census is top-level crates/*/
        // only; nested paths are not workspace members and must not inflate the
        // roster. Dies when git pathspec stops matching nested Cargo.toml.
        .filter(|name| !name.contains('/'))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
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
    let readers = dual_surface_readers(&root);
    assert!(
        !readers.is_empty(),
        "ANTI-VACUITY: no crate invokes both tmux and ntm. This workspace exists to \
         reconcile those two surfaces, so a zero here means the scan is broken — most \
         likely `git ls-files` failed and the roster came back empty. That is the \
         silent-false-zero defect, not a clean bill."
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
    let readers = dual_surface_readers(&root);
    assert!(!readers.is_empty(), "ANTI-VACUITY: empty reader set");

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
    let tracked: BTreeSet<String> = tracked_crates(&root).into_iter().collect();
    assert!(
        tracked.len() >= 40,
        "only {} tracked crate manifests found. A hand list is how a census went stale \
         at 27 while the tree held 51; if git is failing here the roster is a lie.",
        tracked.len()
    );
    let on_disk: BTreeSet<String> = std::fs::read_dir(root.join("crates"))
        .expect("crates/ exists")
        .flatten()
        .filter(|entry| entry.path().join("Cargo.toml").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    let untracked: Vec<&String> = on_disk.difference(&tracked).collect();
    let phantom: Vec<&String> = tracked.difference(&on_disk).collect();
    assert!(
        untracked.is_empty() && phantom.is_empty(),
        "the git-derived roster ({} crates) and the tree on disk ({} crates) name different \
         sets.\n\
         ON DISK BUT UNTRACKED ({}): {:?}\n\
           -> a crate `git ls-files` cannot see is a crate this gate never scans, and it is \
              invisible to CI while still being a workspace member through the `crates/*` glob. \
              Track it or remove it; do NOT widen this assertion.\n\
         TRACKED BUT ABSENT FROM DISK ({}): {:?}\n\
           -> either a deleted crate whose manifest is still indexed, or an environment whose \
              git index does not match the worktree it was handed. Name the environment before \
              believing either half.",
        tracked.len(),
        on_disk.len(),
        untracked.len(),
        untracked,
        phantom.len(),
        phantom
    );
}
