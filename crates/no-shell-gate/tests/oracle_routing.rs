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
/// Three forms, all read AFTER comments are stripped:
///
/// * the exact string literal `"tmux"` / `"ntm"` — the argv form, however the spawn is
///   wrapped (`Command::new`, `PathBuf::from`, a helper taking `&str`);
/// * the `TMUX_BIN` / `NTM_BIN` environment override;
/// * the `tmux_count` / `ntm_count` derived field, which is how a crate reads a surface
///   second-hand out of another lane's JSON. `loop-tick` reaches both surfaces only
///   this way and would otherwise be invisible.
///
/// The exactness is load-bearing. A bare substring search for `ntm` matches prose in
/// assertion messages, `NO_CLAIM` strings, and other crates' names in specimen tables;
/// measured on this tree it produced six false dual-surface readers, including
/// `pane-truth`, whose own documentation says "NTM labels are never consulted".
fn surface_evidence(code: &str, binary: &str) -> usize {
    let literal = format!("\"{binary}\"");
    let env_override = format!("{}_BIN", binary.to_uppercase());
    let derived = format!("{binary}_count");
    code.matches(&literal).count()
        + code.matches(&env_override).count()
        + code.matches(&derived).count()
}

/// Strip `//` and `/* */` comments while KEEPING string literals.
///
/// Keeping strings is not an oversight: the surface names live inside argv literals, so
/// stripping strings destroys the signal entirely. Stripping comments is equally
/// required — six gates in this repo have matched their own prose about the pattern
/// they were hunting.
fn strip_comments(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '/' && bytes.get(i + 1) == Some(&'/') {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && bytes.get(i + 1) == Some(&'*') {
            i += 2;
            while i < bytes.len() && !(bytes[i] == '*' && bytes.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        if c == '"' {
            out.push(c);
            i += 1;
            while i < bytes.len() {
                if bytes[i] == '\\' {
                    out.push(bytes[i]);
                    if let Some(next) = bytes.get(i + 1) {
                        out.push(*next);
                    }
                    i += 2;
                    continue;
                }
                out.push(bytes[i]);
                let closed = bytes[i] == '"';
                i += 1;
                if closed {
                    break;
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
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
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Concatenate `crates/<name>/src/**/*.rs`. Deliberately NOT `tests/`.
fn crate_sources(root: &Path, name: &str) -> String {
    let mut body = String::new();
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
                body.push_str(&text);
                body.push('\n');
            }
        }
    }
    body
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
        let code = strip_comments(&crate_sources(root, &name));
        let tmux = surface_evidence(&code, "tmux");
        let ntm = surface_evidence(&code, "ntm");
        if tmux == 0 || ntm == 0 {
            continue;
        }
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
/// reason. SEEDED FROM THIS GATE'S OWN SCAN of `/Users/josh/Developer/omp-orchestrator`
/// on 2026-09-02 — 12 dual-surface readers, 3 routed, 9 allowed below.
///
/// Seeding a ceiling from a neighbouring measurement is how a mutation probe passed at
/// 42 when the tree held 41. Every row here was produced by
/// [`dual_surface_readers`] itself, in this checkout.
///
/// Adding a row is cheap and auditable; leaving one out is a build failure, and so is
/// leaving one in after the crate starts routing. That asymmetry is the point.
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
                Routing::DeclaredOnly => "DECLARES oracle-compare but names NO comparator \
                                          (spawn helpers do not count)",
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
        .filter(|c| readers.get(*c).is_some_and(|r| r.routing == Routing::Routed))
        .collect();
    assert!(
        repaired.is_empty(),
        "{} allowance row(s) name a crate that NOW ROUTES: {:?}\n\
         DELETE THOSE ROWS. An allowance that outlives its defect is how a repaired gap \
         keeps reading as broken, and it is what stops this list from shrinking.",
        repaired.len(),
        repaired
    );

    let stale: Vec<&str> = UNROUTED_ALLOWANCE
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !readers.contains_key(*c))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowance row(s) name a crate that no longer reads both surfaces: {:?}\n\
         Remove them — an exemption nobody needs is an exemption nobody rechecks.",
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
    assert!(this_file.exists(), "the gate must be able to find itself to prove exclusion");
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
#[test]
fn the_crate_roster_is_derived_from_git_and_covers_the_tree() {
    let root = repo_root();
    let tracked = tracked_crates(&root);
    assert!(
        tracked.len() >= 40,
        "only {} tracked crate manifests found. A hand list is how a census went stale \
         at 27 while the tree held 51; if git is failing here the roster is a lie.",
        tracked.len()
    );
    let on_disk = std::fs::read_dir(root.join("crates"))
        .expect("crates/ exists")
        .flatten()
        .filter(|e| e.path().join("Cargo.toml").exists())
        .count();
    assert_eq!(
        tracked.len(),
        on_disk,
        "tracked crate count {} does not match the {on_disk} on disk — an untracked \
         crate is invisible to this gate and its surfaces go unchecked",
        tracked.len()
    );
}
