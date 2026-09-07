//! `omp-orchestrator-2sx1` acceptance: the finding kernel's OPERATOR SURFACE.
//!
//! Every leg drives the real bin as a subprocess. `CARGO_BIN_EXE_finding` only
//! exists when a bin target does, so leg 1 is satisfied by this file compiling
//! and linking at all — a bin-less crate cannot produce that variable.
//!
//! The publisher is injected at the PROCESS boundary (`--br <program>`) rather
//! than mocked in-process, so the known-good leg exercises the real
//! `BrPublisher` argv, the real spool, and the real `mark_published` rename.
//! Disclosed rather than hidden: `/bin/echo` stands in for `br` and echoes its
//! argv, so the "bead id" is that argv line. That proves the plumbing end to end;
//! it does not prove `br` itself accepts the row, which is `br`'s own suite.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_finding");

/// Distinct exit codes, mirrored from `src/main.rs`. A leg that asserts only
/// `rc != 0` goes green on unrelated breakage (`AGENTS.md` rule 7), so every
/// negative leg below pins BOTH the code and the message token.
const EXIT_USAGE: i32 = 2;
const EXIT_MISSING_FIELD: i32 = 3;
const EXIT_PUBLISH: i32 = 5;
const EXIT_SPOOL_UNREADABLE: i32 = 7;

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "finding-opsurface-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch root");
        Self { root }
    }
    fn path(&self, leaf: &str) -> PathBuf {
        self.root.join(leaf)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Ephemeral by construction: removed before the test process exits, so
        // it never becomes unattributable durable scratch.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("the finding bin must be executable")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("spool dir must exist after a file()")
        .map(|entry| entry.expect("dir entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// LEG 2 — the bin exposes the two subcommands the bead names, and `--help`
/// answers for each. A usage text that does not name the subcommand is how an
/// operator concludes a verb does not exist, which is the defect being fixed.
#[test]
fn the_two_named_subcommands_are_reachable_and_self_describing() {
    for subcommand in ["file", "pending"] {
        let output = run(&[subcommand, "--help"]);
        assert!(
            output.status.success(),
            "{subcommand} --help must exit 0, got {:?}: {}",
            output.status.code(),
            stderr(&output)
        );
        let text = stdout(&output);
        assert!(
            text.contains(subcommand),
            "{subcommand} --help must name the subcommand it documents, got: {text}"
        );
    }
    // The recovery ACTUATOR is present too. `pending` only reports; without
    // `recover` the spool guarantee would still have no operator surface — the
    // same defect one level down.
    let output = run(&["recover", "--help"]);
    assert!(output.status.success(), "recover --help must exit 0");
}

/// LEG 3 — FIRES-ON-KNOWN-BAD, and it asserts the MESSAGE, not just the code.
/// Each field is dropped in turn: a leg that only removed ACCEPTANCE would pass
/// against a bin that hardcoded that one field name.
#[test]
fn an_incomplete_finding_is_refused_and_the_message_names_the_missing_field() {
    let scratch = Scratch::new("incomplete");
    let cases: [(&str, Vec<&str>); 4] = [
        (
            "ACCEPTANCE",
            vec!["--what", "w", "--why", "y", "--labels", "l"],
        ),
        ("WHY", vec!["--what", "w", "--acceptance", "a", "--labels", "l"]),
        ("WHAT", vec!["--why", "y", "--acceptance", "a", "--labels", "l"]),
        ("LABELS", vec!["--what", "w", "--why", "y", "--acceptance", "a"]),
    ];
    for (field, base) in cases {
        let spool = scratch.path(&format!("spool-{field}"));
        let mut args = vec!["file"];
        args.extend(base);
        args.extend(["--spool", spool.to_str().expect("utf-8 scratch path")]);
        let output = run(&args);
        assert_eq!(
            output.status.code(),
            Some(EXIT_MISSING_FIELD),
            "missing {field} must exit {EXIT_MISSING_FIELD}: {}",
            stderr(&output)
        );
        let text = stderr(&output);
        assert!(
            text.contains(&format!("field={field}")),
            "the refusal must NAME the missing field {field}, got: {text}"
        );
        // ANTI-VACUITY on the refusal itself: nothing may be spooled or
        // published for a finding the kernel rejected.
        assert!(
            !spool.exists(),
            "a refused finding must leave no spool row, found {:?}",
            entries(&spool)
        );
    }
}

/// LEG 4 — KNOWN-GOOD. An attack-only suite ships an over-strict gate, and an
/// over-strict gate gets routed around. A well-formed finding must file, leave a
/// `.filed-<id>` row, and drain `pending` to EMPTY.
#[test]
fn a_well_formed_finding_files_and_drains_the_pending_sweep() {
    let scratch = Scratch::new("known-good");
    let spool = scratch.path("spool");
    let spool_arg = spool.to_str().expect("utf-8 scratch path");

    let before = run(&["pending", "--spool", spool_arg]);
    assert_eq!(
        before.status.code(),
        Some(EXIT_SPOOL_UNREADABLE),
        "an ABSENT spool dir is not an empty sweep: {}",
        stderr(&before)
    );

    let output = run(&[
        "file",
        "--what",
        "operator surface reachable",
        "--why",
        "zero bin targets",
        "--acceptance",
        "the bin runs",
        "--labels",
        "kernel,operator-surface",
        "--priority",
        "0",
        "--spool",
        spool_arg,
        "--br",
        "/bin/echo",
    ]);
    assert!(
        output.status.success(),
        "a complete finding must file, got {:?}: {}",
        output.status.code(),
        stderr(&output)
    );
    let id = stdout(&output).trim().to_owned();
    assert!(!id.is_empty(), "file must print the published id");
    // The kernel's argv is what actually reached the publisher, so the echoed id
    // proves BrPublisher's flags were used rather than reconstructed here.
    assert!(
        id.contains("create") && id.contains("--labels") && id.contains("--no-daemon"),
        "the echoed id must show BrPublisher's real argv, got: {id}"
    );

    let rows = entries(&spool);
    assert_eq!(rows.len(), 1, "exactly one spool row expected, got {rows:?}");
    assert!(
        rows[0].starts_with("finding-") && rows[0].contains(".filed-"),
        "the row must be RENAMED to .filed-<id>, never deleted, got {rows:?}"
    );

    let after = run(&["pending", "--spool", spool_arg]);
    assert!(after.status.success(), "pending must succeed on a real dir");
    assert!(
        stdout(&after).contains("FINDING_PENDING_EMPTY") && stdout(&after).contains("count=0"),
        "a filed finding must drain the sweep, got: {}",
        stdout(&after)
    );
}

/// LEG 5 — ANTI-VACUITY. "I could not look" and "there is nothing there" are
/// OPPOSITE conditions. Three outcomes, three distinct tokens, two distinct exit
/// codes; conflating any pair is the defect that produced twelve confident zeros.
#[test]
fn an_unreadable_spool_is_never_silently_an_empty_sweep() {
    let scratch = Scratch::new("vacuity");
    let absent = scratch.path("never-created");
    let empty = scratch.path("empty");
    std::fs::create_dir_all(&empty).expect("empty spool dir");
    std::fs::write(scratch.path("pending-row"), "not a directory").expect("decoy file");

    let absent_out = run(&["pending", "--spool", absent.to_str().expect("utf-8")]);
    assert_eq!(
        absent_out.status.code(),
        Some(EXIT_SPOOL_UNREADABLE),
        "an absent dir must be UNREADABLE, not empty"
    );
    assert!(stderr(&absent_out).contains("FINDING_SPOOL_UNREADABLE"));

    let empty_out = run(&["pending", "--spool", empty.to_str().expect("utf-8")]);
    assert!(empty_out.status.success(), "an empty dir is a legitimate zero");
    assert!(
        stdout(&empty_out).contains("FINDING_PENDING_EMPTY"),
        "an empty sweep must be NAMED, got: {}",
        stdout(&empty_out)
    );

    // A file where a directory belongs is also "could not look", not "nothing".
    let not_a_dir = run(&[
        "pending",
        "--spool",
        scratch.path("pending-row").to_str().expect("utf-8"),
    ]);
    assert_eq!(
        not_a_dir.status.code(),
        Some(EXIT_SPOOL_UNREADABLE),
        "a non-directory spool path must be UNREADABLE, not empty"
    );

    // And the two outcomes must not share a token, or a grep-based check cannot
    // tell them apart even though the codes differ.
    assert!(
        !stdout(&empty_out).contains("UNREADABLE"),
        "the empty and unreadable tokens must be distinct"
    );
}

/// A publish failure is a DIFFERENT cause from a missing field, and the exit
/// codes must say so. Without this, leg 3 could be satisfied by a bin that
/// returned 3 for everything.
#[test]
fn a_dead_publisher_is_a_distinct_cause_from_an_incomplete_finding() {
    let scratch = Scratch::new("publish-fail");
    let spool = scratch.path("spool");
    // `/usr/bin/true` succeeds with EMPTY stdout, which the kernel refuses as
    // "success without an id" — a publisher that lies is not a filed finding.
    let output = run(&[
        "file",
        "--what",
        "w",
        "--why",
        "y",
        "--acceptance",
        "a",
        "--labels",
        "l",
        "--spool",
        spool.to_str().expect("utf-8"),
        "--br",
        "/usr/bin/true",
    ]);
    assert_eq!(
        output.status.code(),
        Some(EXIT_PUBLISH),
        "a publisher returning no id must exit {EXIT_PUBLISH}, not the missing-field code: {}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("FINDING_PUBLISH_FAILED"));

    // THE SPOOL GUARANTEE: the row survives a failed publish, so `recover` can
    // finish the job. A publish failure that deleted the row would lose the
    // finding, which is the whole thing the spool exists to prevent.
    let rows = entries(&spool);
    assert_eq!(rows.len(), 1, "the spool row must SURVIVE a failed publish, got {rows:?}");
    assert!(
        rows[0].ends_with(".pending"),
        "the surviving row must still be pending, got {rows:?}"
    );
    let sweep = run(&["pending", "--spool", spool.to_str().expect("utf-8")]);
    assert!(
        stdout(&sweep).contains("FINDING_PENDING_ROWS") && stdout(&sweep).contains("count=1"),
        "the recovery sweep must SEE the survivor, got: {}",
        stdout(&sweep)
    );
}

/// An unknown verb must not be mistaken for a usable one. This is the reading an
/// operator gets today for every finding verb, and it must be distinguishable
/// from a real failure of a real verb.
#[test]
fn an_unknown_subcommand_is_a_usage_error_and_not_a_silent_success() {
    let output = run(&["publish-everything"]);
    assert_eq!(output.status.code(), Some(EXIT_USAGE));
    assert!(stderr(&output).contains("FINDING_USAGE"));
    let bare = run(&[]);
    assert_eq!(bare.status.code(), Some(EXIT_USAGE), "no args must not exit 0");
}
