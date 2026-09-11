//! `9gta3` item 1: a verdict is streamed AND durable the moment its crate finishes.
//!
//! # The measurement this exists to prevent repeating
//!
//! The first full `--run` in this repository's history, 2026-09-07: **1289 seconds, 2471 bytes of
//! output, ZERO verdict rows** until the very end, then all 88 at once. The report was rendered
//! once after the loop, so an interruption at crate 80 of 88 destroyed the record that 79 had
//! passed. A 21-minute measurement that cannot be banked mid-flight costs its entire cost every
//! time it is interrupted.
//!
//! # Why the kill leg spawns the real binary
//!
//! The property is "survives a KILL", and a kill is precisely the case an unflushed buffer does
//! not survive. Asserting `bank_append` in-process would test the function that was written to
//! pass it; only killing a real process proves the bytes reached the filesystem first.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A throwaway workspace: a `.git` marker so `repo_root()` stops here, and N tiny crates with one
/// lib test each so the derived roster is non-empty.
///
/// `.git` is an EMPTY DIRECTORY, not a repository. The zero-worktree policy permits a test to
/// create a worktree only if it deletes it; this creates no worktree at all — it creates the
/// marker `repo_root()` looks for, which is a different thing and worth saying so nobody reads
/// this as an exception being taken.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str, crates: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!("gate-runner-stream-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git")).expect("marker");
        let members: Vec<String> = crates.iter().map(|c| format!("\"crates/{c}\"")).collect();
        std::fs::write(
            root.join("Cargo.toml"),
            format!(
                "[workspace]\nresolver = \"2\"\nmembers = [{}]\n",
                members.join(", ")
            ),
        )
        .expect("workspace manifest");
        for c in crates {
            let dir = root.join("crates").join(c);
            std::fs::create_dir_all(dir.join("src")).expect("crate dir");
            std::fs::write(
                dir.join("Cargo.toml"),
                format!("[package]\nname = \"{c}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
            )
            .expect("crate manifest");
            // One passing lib test, so the crate contributes a `--lib` invocation and can PASS.
            std::fs::write(
                dir.join("src/lib.rs"),
                "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
            )
            .expect("crate source");
        }
        // A COMMITTED ROSTER, one row per member.
        //
        // Required since the ledger read became a three-way fact: an ABSENT
        // `docs/gate-roster.txt` is now a typed refusal (`GATE_RUNNER_LEDGER_UNREAD
        // reason=absent`, exit 7) rather than an empty set silently compared against the
        // derived roster. These fixtures previously carried no roster at all, so every
        // streaming leg was measuring the coerced-empty path without saying so.
        //
        // Writing the roster to AGREE with the members keeps these legs about STREAMING —
        // drift and unreadability are other crates' legs and must not leak in here.
        std::fs::create_dir_all(root.join("docs")).expect("docs dir");
        std::fs::write(
            root.join("docs/gate-roster.txt"),
            format!("{}\n", crates.join("\n")),
        )
        .expect("roster");
        Self { root }
    }

    fn bank(&self) -> PathBuf {
        self.root.join("bank.rows")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn runner() -> &'static str {
    env!("CARGO_BIN_EXE_gate-runner")
}

fn banked_rows(bank: &Path) -> Vec<String> {
    std::fs::read_to_string(bank)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// KNOWN-GOOD: a clean run banks one row per crate, and every banked row is a real verdict row.
#[test]
fn a_clean_run_banks_one_verdict_row_per_crate() {
    let fx = Fixture::new("clean", &["alpha", "beta"]);
    let out = Command::new(runner())
        .args(["--run"])
        .current_dir(&fx.root)
        .env("GATE_RUNNER_BANK", fx.bank())
        .env_remove("CARGO")
        .output()
        .expect("the runner must be spawnable");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let rows = banked_rows(&fx.bank());
    assert_eq!(
        rows.len(),
        2,
        "two crates must bank two rows; banked={rows:?} stdout={stdout}"
    );
    for row in &rows {
        assert!(
            row.starts_with("PASS crate=")
                || row.starts_with("FAIL crate=")
                || row.starts_with("UNMEASURABLE crate=")
                || row.starts_with("SHORT crate=")
                || row.starts_with("NO_TESTS crate="),
            "a banked line must be a verdict row, got {row:?}"
        );
    }

    // The streamed rows are also in stdout, BEFORE the summary, so an operator sees progress.
    for row in &rows {
        assert!(
            stdout.contains(row.as_str()),
            "every banked row must also have been streamed: missing {row:?} from {stdout}"
        );
    }
}

/// FIRES-ON-KNOWN-BAD, and it is the whole point of `9gta3` item 1: kill the run mid-way and the
/// verdicts already earned must still be on disk.
///
/// Before this change the same kill produced an EMPTY bank and an empty stdout, because the report
/// was rendered once after the loop. The assertion below is exactly the claim that was false.
#[test]
fn a_killed_run_keeps_the_verdicts_it_already_earned() {
    // Enough crates that a kill can land between two of them.
    let fx = Fixture::new("killed", &["alpha", "beta", "gamma", "delta"]);
    let mut child = Command::new(runner())
        .args(["--run"])
        .current_dir(&fx.root)
        .env("GATE_RUNNER_BANK", fx.bank())
        .env_remove("CARGO")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the runner must be spawnable");

    // Wait for the FIRST row to be banked, then kill. Bounded: a hang must fail the test, never
        // wedge the suite.
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut first_seen = Vec::new();
    while Instant::now() < deadline {
        let rows = banked_rows(&fx.bank());
        if !rows.is_empty() {
            first_seen = rows;
            break;
        }
        if let Ok(Some(_)) = child.try_wait() {
            break; // finished before we could observe a partial bank
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        !first_seen.is_empty(),
        "no row was banked within the deadline, so mid-flight durability is UNMEASURED -- this is \
         an ERROR, not a pass: an empty observation must never satisfy this leg"
    );

    // THE CLAIM: what was banked before the kill is still there after it.
    let after = banked_rows(&fx.bank());
    assert!(
        after.len() >= first_seen.len(),
        "the bank SHRANK across a kill: before={first_seen:?} after={after:?}"
    );
    for row in &first_seen {
        assert!(
            after.contains(row),
            "a verdict earned before the kill was lost: {row:?} missing from {after:?}"
        );
    }
    for row in &after {
        assert!(
            row.starts_with("PASS crate=") || row.contains(" crate="),
            "a surviving row must still be parseable, got {row:?}"
        );
    }
}

/// ANTI-VACUITY on the bank path itself: an unwritable bank must NOT be silently ignored, and it
/// must NOT be mistaken for "no verdicts".
///
/// The run still has to work — stdout is the operator's channel and the bank is the resume
/// channel, so losing the bank degrades resumability, not reporting. That distinction is asserted
/// here rather than described.
#[test]
fn an_unwritable_bank_is_named_and_does_not_silence_the_stream() {
    let fx = Fixture::new("unwritable", &["alpha"]);
    // A path whose PARENT is a file: `create_dir_all` cannot succeed, so every append fails.
    let blocker = fx.root.join("not-a-dir");
    std::fs::write(&blocker, b"x").expect("blocker");
    let bank = blocker.join("bank.rows");

    let out = Command::new(runner())
        .args(["--run"])
        .current_dir(&fx.root)
        .env("GATE_RUNNER_BANK", &bank)
        .env_remove("CARGO")
        .output()
        .expect("spawnable");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stderr.contains("GATE_RUNNER_BANK_UNWRITABLE"),
        "an unwritable bank must be NAMED on stderr, not swallowed; stderr={stderr}"
    );
    assert!(
        stdout.contains(" crate=alpha"),
        "the verdict must still stream when the bank is unwritable; stdout={stdout}"
    );
}

/// The streamed row and the reported row come from ONE function, so they cannot drift.
///
/// This is the cheap structural half of the same guarantee: `render_row` is what both paths call,
/// and `GateReport::render` must contain exactly what the stream emitted.
#[test]
fn the_streamed_row_is_byte_identical_to_the_reported_row() {
    use gate_runner::{CrateVerdict, GateReport, UnmeasurablePrecondition};
    use std::collections::{BTreeMap, BTreeSet};

    let cases = vec![
        ("a", CrateVerdict::Passed { targets: 3 }),
        (
            "b",
            CrateVerdict::Failed {
                failing: vec!["x".to_owned(), "y".to_owned()],
                unmeasurable: None,
            },
        ),
        (
            "c",
            CrateVerdict::Unmeasurable {
                reason: UnmeasurablePrecondition::AllTestsSkipped {
                    expected: 1,
                    skipped: 1,
                    detail: "fixture".to_owned(),
                },
            },
        ),
        (
            "d",
            CrateVerdict::Short {
                expected: 4,
                observed: 1,
            },
        ),
    ];
    let mut verdicts = BTreeMap::new();
    for (name, v) in &cases {
        verdicts.insert((*name).to_owned(), v.clone());
    }
    let report = GateReport {
        verdicts,
        ledger_only: BTreeSet::new(),
        workspace_only: BTreeSet::new(),
    };
    let rendered = report.render();
    for (name, v) in &cases {
        let row = v.render_row(name);
        assert!(
            rendered.contains(&row),
            "the report must contain the streamed row byte for byte: {row:?} not in {rendered}"
        );
    }
}
