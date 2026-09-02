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
//! # Toolchain parity — added after this gate certified 67 red CI runs
//!
//! 2026-09-02. `gh run list --limit 200` returned **67 runs and 67 failures**,
//! every one since 2026-09-01T05:59:47Z. Three independently-failing jobs died at
//! the same line: `error[E0554]: #![feature] may not be used on the stable
//! release channel`, compiling `asupersync`. The runner had stable; this
//! workspace is nightly. And for all 67 of those pushes, THIS BINARY printed
//! `PRE_PUSH_GATE_OK`.
//!
//! It was not lying about what it checked. It was lying by omission about what
//! that check was worth: a receipt recorded by a nightly compiler says nothing
//! about a stable one, and the gate never mentioned a compiler at all. A green
//! receipt that certifies a build CI cannot reproduce is worse than no receipt,
//! because somebody reads it and stops looking.
//!
//! So the verify path now also refuses when the repo pins no toolchain (CI would
//! then use the runner default, and the receipt cannot speak for it), or when the
//! compiler this repo resolves differs from the one the pin names, or when the
//! receipt was recorded under a different compiler than the pin now names. This
//! is a LOCAL, OFFLINE check: it reads `rust-toolchain.toml` and runs `rustc -vV`.
//! It never reaches GitHub. A gate that needs the network fails closed on a plane
//! and gets routed around within a day.
//!
//! # What it cannot do
//!
//! `--record 0` is an assertion by its caller, not an observation by this binary.
//! Someone can record a green receipt without running anything, exactly as `touch`
//! satisfies the hook-freshness gate. Deriving the count here would mean running
//! the suite here, which is the cost this design exists to avoid. The honest
//! framing: this converts "I forgot to look" into "I would have to lie".
//!
//! Nor does toolchain parity mean CI will pass. It means CI will compile the same
//! source with the same compiler, so a red run is now evidence about our code
//! instead of evidence about our channel. The gate does not know CI's job list,
//! does not know whether those jobs are correct, and cannot see a run's verdict.
//! It says so on success, in its own output.

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

/// The channel `rust-toolchain.toml` pins, if it pins one.
///
/// Deliberately a hand parse rather than a TOML dependency: this is a pre-push
/// hook, `[toolchain] channel = "…"` is the only key it needs, and every crate
/// added here is a crate that must build before anyone can push.
fn pinned_channel(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix("channel") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let value = rest.trim().trim_matches('"').trim_matches('\'').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// `commit-hash` from `rustc -vV` — for the compiler the rustup shim resolves in
/// `root` (`toolchain: None`), or for one named explicitly.
///
/// The commit hash and not the version string: `1.100.0-nightly` names a range of
/// compilers, and the whole point here is to compare two exact ones.
fn rustc_commit(root: &Path, toolchain: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("rustc");
    if let Some(tc) = toolchain {
        cmd.arg(format!("+{tc}"));
    }
    cmd.arg("-vV").current_dir(root);
    // Generous bound on purpose: with a pin present and the toolchain not yet
    // installed, the rustup shim DOWNLOADS it on this call. That happens once per
    // machine and takes minutes; the steady-state cost is milliseconds.
    let out = match subprocess_contract::bounded_output(
        &mut cmd,
        std::time::Duration::from_secs(600),
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) if out.status.success() => out,
        _ => return None,
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("commit-hash:").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// The compiler CI will use, proven equal to the compiler this repo resolves.
struct Parity {
    channel: String,
    commit: String,
}

/// Refuse unless the repo pins a toolchain AND that pin is what this checkout
/// actually resolves. `Err` carries the operator-facing refusal reason.
fn toolchain_parity(root: &Path) -> Result<Parity, String> {
    let Some(channel) = pinned_channel(root) else {
        return Err(format!(
            "no [toolchain] channel in {}\n\
             \n\
             Without a pin, CI compiles with whatever the runner ships (stable) while\n\
             this receipt was recorded by whatever you happen to have. That mismatch\n\
             produced 67 consecutive red CI runs on 2026-09-01, every one of them\n\
             preceded by a PRE_PUSH_GATE_OK from this binary.",
            root.join("rust-toolchain.toml").display()
        ));
    };
    let Some(pinned) = rustc_commit(root, Some(&channel)) else {
        return Err(format!(
            "the pinned toolchain {channel:?} could not be resolved\n\
             \n\
             CI installs it from rust-toolchain.toml, so a receipt recorded without it\n\
             cannot claim parity. Install it, then re-record:\n\
             \n\
               rustup toolchain install {channel}"
        ));
    };
    let Some(active) = rustc_commit(root, None) else {
        return Err(
            "`rustc -vV` did not run in this checkout — the parity check is broken, and a \
             broken check is not a pass"
                .to_string(),
        );
    };
    if active != pinned {
        return Err(format!(
            "this checkout resolves a different compiler than CI will use\n\
             \n\
             here: {active}\n\
             CI:   {pinned}  (rust-toolchain.toml pins {channel})\n\
             \n\
             A `+toolchain`, a `rustup override`, or RUSTUP_TOOLCHAIN in the environment\n\
             will do this. Clear it, re-run the suite, re-record."
        ));
    }
    Ok(Parity {
        channel,
        commit: pinned,
    })
}

/// Newest tracked source file, and its path.
///
/// Uses `git ls-files` rather than a directory walk so that build output, vendored
/// caches and untracked scratch cannot make the tree look newer than it is. That
/// mistake is already recorded in this repo: a wiring scan matched a vendored
/// `serde_json` copy inside `.rch-tmp/` and measured the wrong tree.
fn newest_tracked_source(root: &Path) -> Option<(PathBuf, u64)> {
    // Bounded: this runs pre-push; a wedged git must yield "no newer file"
    // (the freshness gate then does what it does with that evidence) instead
    // of hanging every push.
    let mut ls_command = std::process::Command::new("git");
    ls_command.args(["ls-files", "-z"]).current_dir(root);
    let out = match subprocess_contract::bounded_output(
        &mut ls_command,
        std::time::Duration::from_secs(10),
    ) {
        subprocess_contract::BoundedOutcome::Completed(out) if out.status.success() => out,
        _ => return None,
    };
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
        // Parity is checked at RECORD time as well as verify time, so a receipt can
        // never be written under a compiler CI will not use. Verify re-checks it
        // because the pin can be bumped after the fact.
        let parity = match toolchain_parity(&root) {
            Ok(p) => p,
            Err(why) => {
                eprintln!("PRE_PUSH_GATE_REFUSED will not record a green receipt: {why}");
                return ExitCode::from(1);
            }
        };
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
             toolchain_channel={}\nrustc_commit={}\n\
             NOTE: the count is asserted by the caller, not observed by this binary.\n\
             NOTE: the toolchain fields ARE observed — `rustc -vV` ran here.\n",
            parity.channel, parity.commit
        );
        if let Err(e) = std::fs::write(&receipt, body) {
            eprintln!("PRE_PUSH_GATE_ERROR could not write receipt: {e}");
            return ExitCode::from(1);
        }
        println!(
            "PRE_PUSH_GATE_RECORDED {} (toolchain {} / {})",
            receipt.display(),
            parity.channel,
            parity.commit
        );
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

    // TOOLCHAIN PARITY. Ordered after the receipt checks and before the freshness
    // check purely so the cheapest refusals come first; all three are required.
    let parity = match toolchain_parity(&root) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("PRE_PUSH_GATE_REFUSED toolchain parity with CI is unproven: {why}");
            return ExitCode::from(1);
        }
    };
    match body
        .lines()
        .find_map(|l| l.strip_prefix("rustc_commit=").map(str::trim))
    {
        None => {
            // ANTI-VACUITY, again: a receipt with no toolchain field predates this
            // check. It is not "probably fine" — it is exactly the shape of receipt
            // that certified 67 red runs, so it does not get grandfathered.
            eprintln!(
                "PRE_PUSH_GATE_REFUSED receipt at {} records no compiler\n\
                 \n\
                 It was written before this gate checked toolchain parity, so it cannot\n\
                 support the claim. Re-run the suite as its own step and re-record.",
                receipt.display()
            );
            return ExitCode::from(1);
        }
        Some(recorded) if recorded != parity.commit => {
            eprintln!(
                "PRE_PUSH_GATE_REFUSED the receipt was recorded by a different compiler \
                 than CI will use\n\
                 \n\
                 receipt: {recorded}\n\
                 CI:      {}  (rust-toolchain.toml pins {})\n\
                 \n\
                 The pin moved after the receipt was written, or the suite ran under an\n\
                 override. Re-run the suite and re-record.",
                parity.commit, parity.channel
            );
            return ExitCode::from(1);
        }
        Some(_) => {}
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
            println!(
                "PRE_PUSH_GATE_OK green receipt postdates every tracked source, and was \
                 recorded by {} ({}) — the compiler CI installs from rust-toolchain.toml",
                parity.channel, parity.commit
            );
            println!(
                "PRE_PUSH_GATE_NO_CLAIM this gate did NOT run the suite (the count is the \
                 caller's assertion), did NOT contact GitHub, and does NOT know whether CI's \
                 job list matches the local gates or whether those gates are correct. It \
                 claims exactly two things: somebody recorded a green workspace after the \
                 newest tracked source changed, and they did it with the compiler CI uses."
            );
            ExitCode::SUCCESS
        }
    }
}
