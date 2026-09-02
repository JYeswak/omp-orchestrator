#![forbid(unsafe_code)]
//! Pre-commit entry point for the staged build gate.
//!
//! **A plain `fn main() -> ExitCode`, with no entry macro, deliberately.** The
//! known-bad this gate exists to catch is `#[asupersync::main]` on
//! `async fn main() -> ExitCode`; a gate that can die of the class it polices is worse
//! than no gate, so the bounded subprocess work goes through the synchronous
//! `subprocess-contract::bounded_output`.
//!
//! Exit codes, stable:
//!   0  admitted (`GATE_NOT_APPLICABLE` or `STAGED_BUILD_GATE_PASS`)
//!   1  refused - a touched crate did not build, or diverged from the index
//!   2  the gate could not do its job (git unreadable, zero targets resolved)

use staged_build_gate::{
    classify_scope, first_cargo_error, fold, render_refusal, CrateVerdict, GateVerdict,
    StagedScope, BUILD_DEADLINE_SECS,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

fn main() -> ExitCode {
    let repo = match repo_root() {
        Ok(repo) => repo,
        Err(error) => {
            eprintln!("STAGED_BUILD_GATE_ERROR reason=repo_root detail=\"{error}\" next_action=run-inside-a-git-repo");
            return ExitCode::from(2);
        }
    };

    let staged = match git_lines(&repo, &["diff", "--cached", "--name-only"]) {
        Ok(lines) => lines,
        Err(error) => {
            eprintln!("STAGED_BUILD_GATE_ERROR reason=staged_set_unreadable detail=\"{error}\" next_action=inspect-git");
            return ExitCode::from(2);
        }
    };

    let touched = match classify_scope(&staged) {
        StagedScope::NotApplicable => {
            println!("GATE_NOT_APPLICABLE staged_paths={} crates_touched=0 -- no staged path lives under crates/", staged.len());
            return ExitCode::SUCCESS;
        }
        StagedScope::Crates(crates) => crates,
    };

    let mut rows: BTreeMap<String, CrateVerdict> = BTreeMap::new();
    for name in &touched {
        rows.insert(name.clone(), evaluate_crate(&repo, name));
    }

    match fold(&touched, rows) {
        GateVerdict::NotApplicable => {
            println!("GATE_NOT_APPLICABLE crates_touched=0");
            ExitCode::SUCCESS
        }
        GateVerdict::Pass { crates } => {
            println!(
                "STAGED_BUILD_GATE_PASS crates={} built=[{}]",
                crates.len(),
                crates.join(",")
            );
            ExitCode::SUCCESS
        }
        GateVerdict::ZeroTargets { touched } => {
            // Indicts this gate's own parser, not the commit, so the remedy differs
            // and the exit code does too.
            eprintln!(
                "STAGED_BUILD_GATE_ERROR reason=ZERO_BUILD_TARGETS touched=[{}] next_action=check-the-crate-path-parser -- crates were staged and not one resolved to a Cargo package; a path that resolves to no target silently passes everything",
                touched.join(",")
            );
            ExitCode::from(2)
        }
        GateVerdict::Refused { rows } => {
            eprintln!("{}", render_refusal(&rows));
            eprintln!(
                "STAGED_BUILD_GATE_REFUSED_SUMMARY refused={} deadline_secs={BUILD_DEADLINE_SECS} -- `--no-verify` remains an intentional, visible bypass",
                rows.len()
            );
            ExitCode::from(1)
        }
    }
}

fn evaluate_crate(repo: &Path, name: &str) -> CrateVerdict {
    let dir = repo.join("crates").join(name);
    if !dir.join("Cargo.toml").is_file() {
        return CrateVerdict::NoTarget;
    }
    let Some(package) = package_name(&dir.join("Cargo.toml")) else {
        return CrateVerdict::NoTarget;
    };

    // THE EVIDENCE BOUNDARY, checked BEFORE spending a build. `cargo` compiles the
    // worktree; the commit carries the index. If they differ for this crate, a build
    // is not evidence about the commit, so refuse rather than produce a confident
    // wrong answer.
    let scope = format!("crates/{name}/");
    match git_lines(repo, &["diff", "--name-only", "--", &scope]) {
        Ok(diverged) => {
            // Scoped: only a divergence that can change what `cargo build -p X`
            // compiles invalidates the build as evidence. A dirty integration test
            // cannot, and refusing on it is the over-strictness that gets a
            // commit-path gate routed around.
            let relevant: Vec<String> = diverged
                .into_iter()
                .filter(|path| staged_build_gate::build_relevant(path))
                .collect();
            if !relevant.is_empty() {
                return CrateVerdict::Diverged { paths: relevant };
            }
        }
        Err(error) => {
            return CrateVerdict::Unspawned {
                detail: format!("divergence check failed: {error}"),
            };
        }
    }

    // ITS OWN TARGET DIRECTORY, and this is not an optimisation.
    //
    // MEASURED 2026-09-02, by this gate failing on its own 750-line crate: the same
    // crate compiled in 4.03s under `cargo test` and then exceeded a 300s deadline
    // here. Nothing about the crate changed. `cargo` takes an exclusive lock on the
    // build directory, and this checkout had live `rch exec -- cargo test` processes
    // from two other agents, so the gate spent its whole deadline queued behind
    // peers and reported BUILD_TIMED_OUT about code that builds in four seconds.
    //
    // A commit-path gate that inherits the shared lock is hostage to every other
    // writer - the `--install` defect this bead exists to stop, reproduced one level
    // down. A separate target dir has its own lock, so contention is impossible
    // rather than merely unlikely. The volume measured 236Gi free at 73%.
    let target_dir = std::env::var_os("STAGED_BUILD_GATE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("target").join("staged-build-gate"));
    let mut command = Command::new(cargo_bin());
    command
        .current_dir(repo)
        .env("CARGO_TARGET_DIR", &target_dir)
        .arg("build")
        .arg("--quiet")
        .arg("-p")
        .arg(&package);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(BUILD_DEADLINE_SECS))
    {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            CrateVerdict::Pass
        }
        subprocess_contract::BoundedOutcome::Completed(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // A non-zero exit is evidence about the CODE only when a compiler
            // diagnostic accompanies it. Without one, the build did not run --
            // measured here as `[RCH] remote required; refusing local fallback` with
            // exit 103 and zero `error` lines -- and blaming the crate for that is
            // the same defect as reading a timeout as a verdict.
            match first_cargo_error(&stderr) {
                Some(first_error) => CrateVerdict::BuildFailed {
                    code: output.status.code(),
                    first_error,
                },
                None => CrateVerdict::BuildInconclusive {
                    code: output.status.code(),
                    stderr_tail: stderr
                        .lines()
                        .rev()
                        .take(3)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join(" | ")
                        .chars()
                        .take(240)
                        .collect(),
                },
            }
        }
        subprocess_contract::BoundedOutcome::TimedOut => CrateVerdict::TimedOut {
            deadline_secs: BUILD_DEADLINE_SECS,
        },
        subprocess_contract::BoundedOutcome::Unspawned(error) => CrateVerdict::Unspawned {
            detail: error.to_string(),
        },
    }
}

/// The `name = "..."` from `[package]`. Read rather than derived from the directory:
/// a crate directory and its package name are allowed to differ, and assuming they
/// match is how a real crate reports `NO_BUILD_TARGET`.
fn package_name(manifest: &Path) -> Option<String> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                continue;
            };
            return Some(rest.trim().trim_matches('"').to_owned());
        }
    }
    None
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

fn repo_root() -> Result<PathBuf, String> {
    let out = run_git(Path::new("."), &["rev-parse", "--show-toplevel"])?;
    let path = out.trim();
    if path.is_empty() {
        return Err("git printed an empty toplevel".to_owned());
    }
    Ok(PathBuf::from(path))
}

fn git_lines(repo: &Path, args: &[&str]) -> Result<Vec<String>, String> {
    Ok(run_git(repo, args)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Bounded, both pipes drained, own process group - the same contract the build uses.
/// A hook that can hang on `git` is a hook that stalls every writer in the checkout.
fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(30)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git {} exited {}: {}",
            args.join(" "),
            output
                .status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => {
            Err(format!("git {} exceeded 30s", args.join(" ")))
        }
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("git unspawnable: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest reader must take the name from `[package]`, not from the first
    /// `name =` in the file - `[[bin]]` and `[lib]` both have one, and this crate's
    /// own manifest has all three in that order.
    #[test]
    fn the_package_name_comes_from_the_package_section_only() {
        let dir = std::env::temp_dir().join(format!("sbg-manifest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("fixture dir");
        let manifest = dir.join("Cargo.toml");
        std::fs::write(
            &manifest,
            "[package]\nname = \"real-package\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"wrong_lib\"\n\n[[bin]]\nname = \"wrong-bin\"\n",
        )
        .expect("write manifest");
        assert_eq!(package_name(&manifest).as_deref(), Some("real-package"));

        // A manifest with no [package] resolves to no target rather than guessing.
        let virtual_manifest = dir.join("virtual.toml");
        std::fs::write(&virtual_manifest, "[workspace]\nmembers = [\"crates/*\"]\n")
            .expect("write virtual");
        assert_eq!(package_name(&virtual_manifest), None);

        // Alignment must not defeat the parser: this repo's NUMBERS.toml aligns
        // assignments with two spaces and a single-space pattern read 0 of 34 rows.
        let aligned = dir.join("aligned.toml");
        std::fs::write(&aligned, "[package]\nname     = \"aligned-pkg\"\n").expect("write aligned");
        assert_eq!(package_name(&aligned).as_deref(), Some("aligned-pkg"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// This crate's OWN manifest must parse - a positive control against the real
    /// file, because a parser that only passes fixtures is a fixture drifted from
    /// production (`fh` C38).
    #[test]
    fn this_crates_real_manifest_resolves_to_its_package_name() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert_eq!(
            package_name(&manifest).as_deref(),
            Some("staged-build-gate"),
            "the gate must be able to resolve its own manifest"
        );
    }
}
