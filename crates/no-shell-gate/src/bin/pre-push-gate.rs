//! PRE-PUSH GATE — refuse to push unless a GREEN WORKSPACE RECEIPT postdates every
//! source file being pushed.
//!
//! # The defect this exists for, measured four times in one session
//!
//! I repeatedly put a test tally and a `git commit`/`git push` in the same shell
//! call. The tally printed `1 failed`, it scrolled past above the commit output,
//! and the push went out red. I then wrote a commit message stating the rule —
//!
//! > "a verification whose output shares a command with the action it gates cannot
//! > block that action. The tally must be its own step, and its exit code must be
//! > read."
//!
//! — and did it again twice more. **Writing the rule down did not change the
//! behaviour.** That is this repository's entire thesis, aimed back at its author:
//! a rule without a mechanism is a preference.
//!
//! # Why a receipt and not "run the suite here"
//!
//! Running `cargo test --workspace` takes ~60s. A pre-push hook that costs a
//! minute on every push gets bypassed with `--no-verify` within a day, and a gate
//! that is routinely bypassed is worse than no gate — the same over-strict-gate
//! death that `state-wildcard-lint` reached tonight at 89% false positives, when it
//! blocked every commit in the repo.
//!
//! So this hook is a **stat check**, costing microseconds. It demands that somebody
//! ran the suite and recorded the result, and that the recording is newer than the
//! code. It cannot be satisfied by luck or by patience.
//!
//! # Contract
//!
//! - `--record` : write the receipt. Requires the caller to pass the observed
//!   failing-suite count; a nonzero count writes NO receipt and exits nonzero.
//! - default    : verify. Refuses if the receipt is missing, malformed, records
//!   failures, or is older than the newest tracked `.rs` / `.toml` file.
//!
//! # What it cannot do
//!
//! `--record 0` is an assertion by its caller, not an observation by this binary.
//! Someone can record a green receipt without running anything, exactly as `touch`
//! satisfies the hook-freshness gate. Deriving the count here would mean running
//! the suite here, which is the cost this design exists to avoid. The honest
//! framing: this converts "I forgot to look" into "I would have to lie".

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const RECEIPT: &str = ".flywheel/workspace-green.receipt";

fn usage() -> String {
    format!(
        "usage: pre-push-gate [--record <failing_suite_count>] [--repo PATH]\n\
         \n\
         verify (default): refuse if {RECEIPT} is missing, records failures, or is\n\
         older than the newest tracked source file.\n\
         \n\
         --record N: write the receipt. N must be 0; any other value writes nothing\n\
         and exits 1, because a receipt is a claim of GREEN."
    )
}

fn repo_root_from(start: &Path) -> PathBuf {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return cur;
        }
        if !cur.pop() {
            return start.to_path_buf();
        }
    }
}

fn mtime_secs(p: &Path) -> Option<u64> {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Newest tracked source file, and its path.
///
/// Uses `git ls-files` rather than a directory walk so that build output, vendored
/// caches and untracked scratch cannot make the tree look newer than it is. That
/// mistake is already recorded in this repo: a wiring scan matched a vendored
/// `serde_json` copy inside `.rch-tmp/` and measured the wrong tree.
fn newest_tracked_source(root: &Path) -> Option<(PathBuf, u64)> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut best: Option<(PathBuf, u64)> = None;
    for raw in out.stdout.split(|b| *b == 0) {
        if raw.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(raw).into_owned();
        let keep = rel.ends_with(".rs") || rel.ends_with(".toml");
        if !keep {
            continue;
        }
        let p = root.join(&rel);
        if let Some(t) = mtime_secs(&p) {
            if best.as_ref().is_none_or(|(_, b)| t > *b) {
                best = Some((p, t));
            }
        }
    }
    best
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut record: Option<String> = None;
    let mut repo: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--record" => {
                i += 1;
                record = args.get(i).cloned();
            }
            "--repo" => {
                i += 1;
                repo = args.get(i).cloned();
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            // Git invokes a pre-push hook as `hook <remote-name> <remote-url>` and
            // pipes the ref list on stdin. Rejecting unknown POSITIONALS made the
            // installed hook exit 2 on every push with a usage error -- refusing for
            // the wrong reason, which is worse than not refusing, because the message
            // points the operator at their arguments instead of at their untested code.
            //
            // Measured by running git's real invocation before trusting the install.
            // Only unknown FLAGS are an error now.
            other if other.starts_with('-') => {
                eprintln!("PRE_PUSH_GATE_ERROR unknown flag {other}\n{}", usage());
                return ExitCode::from(2);
            }
            _ => {}
        }
        i += 1;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let root = repo.map(PathBuf::from).unwrap_or_else(|| repo_root_from(&cwd));
    let receipt = root.join(RECEIPT);

    if let Some(count) = record {
        let n: u64 = match count.parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("PRE_PUSH_GATE_ERROR --record expects an integer, got {count:?}");
                return ExitCode::from(2);
            }
        };
        if n != 0 {
            eprintln!(
                "PRE_PUSH_GATE_REFUSED recording {n} failing suite(s) — a receipt is a claim of \
                 GREEN, so none was written. Fix the failures and re-record."
            );
            return ExitCode::from(1);
        }
        if let Some(parent) = receipt.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let body = format!(
            "workspace_green_receipt\nrecorded_at_unix={now}\nfailing_suites=0\n\
             recorded_by=pre-push-gate\n\
             NOTE: the count is asserted by the caller, not observed by this binary.\n"
        );
        if let Err(e) = std::fs::write(&receipt, body) {
            eprintln!("PRE_PUSH_GATE_ERROR could not write receipt: {e}");
            return ExitCode::from(1);
        }
        println!("PRE_PUSH_GATE_RECORDED {}", receipt.display());
        return ExitCode::SUCCESS;
    }

    // verify
    let Some(receipt_t) = mtime_secs(&receipt) else {
        eprintln!(
            "PRE_PUSH_GATE_REFUSED no green receipt at {}\n\
             \n\
             Run the workspace suite as ITS OWN STEP, read the count, then record it:\n\
             \n\
               cargo test --workspace --no-fail-fast\n\
               pre-push-gate --record 0        # only if the count was 0\n\
             \n\
             This gate exists because its author put the tally and the push in one\n\
             shell call four times in one session and pushed red three of them.",
            receipt.display()
        );
        return ExitCode::from(1);
    };

    let body = std::fs::read_to_string(&receipt).unwrap_or_default();
    if !body.contains("failing_suites=0") {
        eprintln!(
            "PRE_PUSH_GATE_REFUSED receipt at {} does not record a green workspace",
            receipt.display()
        );
        return ExitCode::from(1);
    }

    match newest_tracked_source(&root) {
        None => {
            // ANTI-VACUITY: no tracked sources means the scan is broken, not that
            // everything is fresh. A gate that passes on an empty scan set reports
            // identically to one that verified something.
            eprintln!(
                "PRE_PUSH_GATE_ERROR git ls-files returned no .rs/.toml files — the scan is \
                 broken, and a broken scan is not a pass"
            );
            ExitCode::from(2)
        }
        Some((path, src_t)) if src_t > receipt_t => {
            eprintln!(
                "PRE_PUSH_GATE_REFUSED the receipt predates the code being pushed\n\
                 \n\
                 receipt: {}  ({}s old)\n\
                 newer:   {}\n\
                 \n\
                 Re-run the suite as its own step and re-record.",
                receipt.display(),
                src_t.saturating_sub(receipt_t),
                path.display()
            );
            ExitCode::from(1)
        }
        Some(_) => {
            println!("PRE_PUSH_GATE_OK green receipt postdates every tracked source");
            ExitCode::SUCCESS
        }
    }
}
