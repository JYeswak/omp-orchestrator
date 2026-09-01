#![forbid(unsafe_code)]

//! Thin process/filesystem adapter for `wired-but-inert-guard`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};
use std::time::Duration;

use subprocess_contract::{bounded_output, BoundedOutcome};


use wired_but_inert_guard::{
    analyze, render_capabilities_json, render_error_json, render_human, render_json,
    render_selftest_json, render_why_json, run_selftest, WIRED_ROWS, CRONTAB_ENV, REPO_ENV,
};

const USAGE: &str = "usage: wired-but-inert-guard [guard|status|why [GATE]|capabilities] [--json] [--repo <PATH>]\n\n\
             guard [--selftest] [--json]\n\
             --check is an alias for guard; --selftest runs all eight assertions.\n\
             repository root precedence: --repo flag > WIRED_GUARD_REPO env > upward walk\n\
             from the cwd for a .git or .beads marker; no marker and no override is a loud\n\
             error, never a default.\n";

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
fn run_bounded(mut command: Command, label: &str) -> Result<Output, String> {
    match bounded_output(&mut command, COMMAND_TIMEOUT) {
        BoundedOutcome::Completed(output) => Ok(output),
        BoundedOutcome::TimedOut => Err(format!("{label} timed out after {}s", COMMAND_TIMEOUT.as_secs())),
        BoundedOutcome::Unspawned(error) => Err(format!("spawn {label}: {error}")),
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("usage error: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn input_error(error: impl std::fmt::Display, json: bool) -> ExitCode {
    if json {
        println!("{}", render_error_json(&error.to_string()));
    } else {
        eprintln!("wired-but-inert-guard: RED input error: {error}");
    }
    ExitCode::from(2)
}

/// Marker entries that identify a repository root while walking up from the cwd.
/// `.git` may be a directory (plain checkout) or a file (worktree/submodule).
const REPO_MARKERS: [&str; 2] = [".git", ".beads"];

/// Fail-closed repository resolution. Every variant names what could not be found: the
/// historic bug here was returning the bare cwd, which compiled fine and then silently
/// scanned whatever unrelated repository contained the working directory.
#[derive(Debug)]
enum RepoRootError {
    /// An explicit source (`--repo` or `WIRED_GUARD_REPO`) was set but empty.
    ExplicitEmpty { source: String },
    /// No repository marker found walking up from `from`.
    NotFound { from: PathBuf },
}

impl std::fmt::Display for RepoRootError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExplicitEmpty { source } => write!(formatter, "{source} is set but empty"),
            Self::NotFound { from } => write!(
                formatter,
                "no repository marker ({}) found at or above {}; pass --repo <PATH> or set {REPO_ENV}",
                REPO_MARKERS.join(" or "),
                from.display()
            ),
        }
    }
}

impl std::error::Error for RepoRootError {}

/// Walk up from `start`, returning the first ancestor that holds a repository marker.
/// Mirrors `beads_rust`'s `canonical_source_repo`: identity is derived from a discovered
/// marker's parent, never from a constant.
fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(directory) = current {
        if REPO_MARKERS.iter().any(|marker| directory.join(marker).exists()) {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

/// Pure resolution, highest precedence first: `--repo` flag > `WIRED_GUARD_REPO` env >
/// upward marker walk from `start`. Pure with respect to the process so precedence is
/// unit-testable.
fn resolve_repo_root(
    flag: Option<&str>,
    env_value: Option<String>,
    start: &Path,
) -> Result<PathBuf, RepoRootError> {
    if let Some(flag) = flag {
        if flag.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty { source: "--repo".to_owned() });
        }
        return Ok(PathBuf::from(flag));
    }
    if let Some(value) = env_value {
        if value.trim().is_empty() {
            return Err(RepoRootError::ExplicitEmpty { source: REPO_ENV.to_owned() });
        }
        return Ok(PathBuf::from(value));
    }
    discover_repo_root(start).ok_or_else(|| RepoRootError::NotFound { from: start.to_path_buf() })
}

fn repo_root(flag: Option<&str>) -> Result<PathBuf, String> {
    let cwd = env::current_dir()
        .map_err(|error| format!("cannot read the current directory: {error}"))?;
    let env_value = match env::var(REPO_ENV) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(format!("cannot read {REPO_ENV}: {error}")),
    };
    resolve_repo_root(flag, env_value, &cwd).map_err(|error| error.to_string())
}

fn cron_contents() -> Result<String, String> {
    if let Some(snapshot) = env::var_os(CRONTAB_ENV) {
        let path = PathBuf::from(snapshot);
        return fs::read_to_string(&path).map_err(|error| format!("cannot read crontab snapshot {}: {error}", path.display()));
    }

    // `crontab -l` exits non-zero when the user has no crontab.  Its stdout is still the exact
    // command-substitution payload the shell oracle uses; a spawn failure is not treated as an
    // empty schedule because that would turn an unavailable input into a false receipt.
    let mut command = Command::new("crontab");
    command.arg("-l");
    let output = run_bounded(command, "crontab -l")?;
    String::from_utf8(output.stdout).map_err(|error| format!("crontab output is not UTF-8: {error}"))
}

fn tracked_files(repo: &Path) -> Result<Vec<String>, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(["ls-files", "--", "bin/*", ".github/*"]);
    let output = run_bounded(command, "git ls-files")?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(format!("git ls-files failed ({}{})", output.status, if detail.is_empty() { String::new() } else { format!(": {detail}") }));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("git file list is not UTF-8: {error}"))?;
    Ok(text
        .lines()
        .filter(|line| !line.is_empty())
        .filter(|line| repo.join(line).is_file())
        .map(str::to_owned)
        .collect())
}

fn scan(repo_flag: Option<&str>) -> Result<wired_but_inert_guard::GuardReport, String> {
    let repo = repo_root(repo_flag)?;
    let cron = cron_contents()?;
    let files = tracked_files(&repo)?;
    analyze(&repo, &cron, &files).map_err(|error| error.to_string())
}

fn provenance() -> (&'static str, &'static str) {
    match env::var("WIRED_GUARD_INVOKER").ok().as_deref() {
        Some(value) if value.starts_with("SCHEDULED") => ("SCHEDULED", "cron_parent"),
        _ => ("MANUAL", "unproven_parent"),
    }
}

fn with_provenance(mut json: String) -> String {
    let (invoker, proof) = provenance();
    if json.ends_with('}') {
        json.pop();
        json.push_str(&format!(",\"invoker\":\"{invoker}\",\"invoker_proof\":\"{proof}\"}}"));
    }
    json
}

fn print_report(
    report: &wired_but_inert_guard::GuardReport,
    json: bool,
    why_mode: bool,
    gate_filter: Option<&str>,
) -> ExitCode {
    if json {
        if why_mode {
            println!("{}", with_provenance(render_why_json(report, gate_filter)));
        } else {
            println!("{}", with_provenance(render_json(report)));
        }
    } else {
        let (invoker, proof) = provenance();
        print!("{}invoker={invoker} invoker_proof={proof}\n", render_human(report));
    }
    if report.is_pass() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn parse_json(args: &[String]) -> Result<(bool, Vec<String>), String> {
    let mut json = false;
    let mut rest = Vec::new();
    for arg in args {
        if arg == "--json" {
            if json {
                return Err("--json may be supplied only once".to_owned());
            }
            json = true;
        } else {
            rest.push(arg.clone());
        }
    }
    Ok((json, rest))
}

/// Extract `--repo PATH` / `--repo=PATH` from `args`, removing those entries in place so
/// per-command validation still rejects anything unexpected.
fn extract_repo_flag(args: &mut Vec<String>) -> Result<Option<String>, String> {
    let mut flag: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--repo" {
            let value = args
                .get(index + 1)
                .ok_or("--repo requires a path")?
                .clone();
            args.drain(index..=index + 1);
            flag = Some(value);
            continue;
        }
        if let Some(value) = args[index].strip_prefix("--repo=").map(str::to_owned) {
            args.remove(index);
            flag = Some(value);
            continue;
        }
        index += 1;
    }
    Ok(flag)
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw == ["--help"] || raw == ["-h"] {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }

    let (command, command_args) = match raw.split_first() {
        Some((command, args)) => (command.as_str(), args.to_vec()),
        None => ("guard", Vec::new()),
    };
    let (json, mut args) = match parse_json(&command_args) {
        Ok(parsed) => parsed,
        Err(error) => return usage_error(&error),
    };
    let repo_flag = match extract_repo_flag(&mut args) {
        Ok(flag) => flag,
        Err(error) => return usage_error(&error),
    };

    if command == "capabilities" {
        if !args.is_empty() {
            return usage_error("capabilities accepts only --json");
        }
        println!("{}", render_capabilities_json());
        return ExitCode::SUCCESS;
    }
    if command == "--selftest" {
        if !args.is_empty() {
            return usage_error("--selftest accepts only --json");
        }
        let report = run_selftest();
        if json {
            println!("{}", render_selftest_json(&report));
        } else {
            for assertion in &report.assertions {
                println!("  {}  {}", if assertion.got == assertion.want { "PASS" } else { "FAIL" }, assertion.name);
            }
            println!("\nSELFTEST {} ({} assertions)", if report.is_pass() { "PASS" } else { "FAIL" }, report.checked);
        }
        return if report.is_pass() { ExitCode::SUCCESS } else { ExitCode::from(1) };
    }

    let (why_mode, gate_filter) = match command {
        "guard" | "check" | "--check" => {
            if args == ["--selftest"] {
                let report = run_selftest();
                if json {
                    println!("{}", render_selftest_json(&report));
                } else {
                    println!("SELFTEST {} ({} assertions)", if report.is_pass() { "PASS" } else { "FAIL" }, report.checked);
                }
                return if report.is_pass() { ExitCode::SUCCESS } else { ExitCode::from(1) };
            }
            if !args.is_empty() {
                return usage_error("guard accepts --selftest and --json");
            }
            (false, None)
        }
        "status" => {
            if !args.is_empty() {
                return usage_error("status accepts only --json");
            }
            (false, None)
        }
        "why" => {
            if args.len() > 1 {
                return usage_error("why accepts at most one declared GATE");
            }
            let gate = args.first().map(String::as_str);
            if let Some(gate) = gate {
                if !WIRED_ROWS.iter().any(|row| row.gate == gate) {
                    return usage_error("why requires one of the declared gate paths");
                }
            }
            (true, gate)
        }
        _ => return usage_error("unknown operation"),
    };

    let report = match scan(repo_flag.as_deref()) {
        Ok(report) => report,
        Err(error) => return input_error(error, json),
    };
    print_report(&report, json, why_mode, gate_filter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Best-effort temp directory with cleanup on drop; keeps the crate dependency-free.
    struct TempDir(PathBuf);

    impl TempDir {
        fn create(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "wired-but-inert-guard-test-{}-{}-{}",
                label,
                std::process::id(),
                unique
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn repo_flag_beats_env_beats_discovery() {
        let root = TempDir::create("precedence");
        let nested = root.path().join("a/b/c");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(root.path().join(".git")).expect("create .git marker");

        let flag_target = TempDir::create("flag-target");
        let env_target = TempDir::create("env-target");

        let resolved = resolve_repo_root(
            Some(flag_target.path().to_str().expect("utf-8 path")),
            Some(env_target.path().to_string_lossy().into_owned()),
            &nested,
        )
        .expect("flag must win");
        assert_eq!(resolved, flag_target.path());

        let resolved =
            resolve_repo_root(None, Some(env_target.path().to_string_lossy().into_owned()), &nested)
                .expect("env must win over discovery");
        assert_eq!(resolved, env_target.path());

        let resolved = resolve_repo_root(None, None, &nested).expect("discovery must find the marker");
        assert_eq!(resolved, root.path());
    }

    #[test]
    fn discovery_walks_up_for_git_and_beads_markers() {
        let git_root = TempDir::create("git-marker");
        let nested = git_root.path().join("deeply/nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(git_root.path().join(".git")).expect("create .git marker");
        assert_eq!(discover_repo_root(&nested), Some(git_root.path().to_path_buf()));

        let beads_root = TempDir::create("beads-marker");
        let nested = beads_root.path().join("x");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::create_dir(beads_root.path().join(".beads")).expect("create .beads marker");
        assert_eq!(discover_repo_root(&nested), Some(beads_root.path().to_path_buf()));
    }

    #[test]
    fn known_bad_no_repo_above_cwd_fails_loudly_naming_the_markers() {
        let nowhere = TempDir::create("known-bad");
        let start = nowhere.path().join("plain");
        fs::create_dir_all(&start).expect("create start directory");

        let error = match resolve_repo_root(None, None, &start) {
            Ok(found) => panic!("a marker-free directory must not resolve; found {}", found.display()),
            Err(error) => error,
        };
        // KNOWN-BAD: the typed error must name the markers and the start directory. The
        // historic behavior here was returning the bare cwd, which silently scanned
        // whatever unrelated repository contained it.
        assert!(
            matches!(error, RepoRootError::NotFound { ref from } if *from == start),
            "wrong error for a marker-free directory: {error:?}"
        );
        let message = error.to_string();
        assert!(message.contains(".git") && message.contains(".beads"), "message must name the markers: {message}");
        assert!(
            message.contains(start.to_string_lossy().as_ref()),
            "message must name the start directory: {message}"
        );
        assert!(message.contains(REPO_ENV), "message must name the escape hatch env: {message}");
    }

    #[test]
    fn empty_explicit_sources_are_errors_not_defaults() {
        let start = Path::new("/");
        let error = resolve_repo_root(Some("   "), None, start).expect_err("empty --repo is an error");
        assert!(matches!(error, RepoRootError::ExplicitEmpty { .. }), "wrong error: {error:?}");
        assert!(error.to_string().contains("--repo"), "message must name --repo: {error}");

        let error = resolve_repo_root(None, Some(String::new()), start).expect_err("empty env is an error");
        assert!(matches!(error, RepoRootError::ExplicitEmpty { .. }), "wrong error: {error:?}");
        assert!(error.to_string().contains(REPO_ENV), "message must name the env var: {error}");
    }

    #[test]
    fn extract_repo_flag_consumes_both_forms_and_leaves_the_rest() {
        let mut args = vec![
            "--repo".to_owned(),
            "/elsewhere".to_owned(),
            "--json".to_owned(),
            "some-gate".to_owned(),
        ];
        let flag = extract_repo_flag(&mut args).expect("valid --repo form");
        assert_eq!(flag.as_deref(), Some("/elsewhere"));
        assert_eq!(args, vec!["--json".to_owned(), "some-gate".to_owned()]);

        let mut args = vec!["--repo=/other".to_owned()];
        let flag = extract_repo_flag(&mut args).expect("valid --repo= form");
        assert_eq!(flag.as_deref(), Some("/other"));
        assert!(args.is_empty());

        let mut args = vec!["--repo".to_owned()];
        assert!(extract_repo_flag(&mut args).is_err(), "missing value must be a usage error");
    }

    /// The home-path literal this gate forbids. Built by `concat!` so the scanning source
    /// itself never contains the contiguous literal (the gate must not catch its own needle).
    const USER_HOME_LITERAL: &str = concat!("/Users/", "josh");

    /// Count home-path literals in this crate's own `src/`, recursively.
    fn hardcoded_user_path_hits(src: &Path) -> (Vec<String>, usize) {
        let mut hits = Vec::new();
        let mut scanned = 0usize;
        let mut stack = vec![src.to_path_buf()];
        while let Some(directory) = stack.pop() {
            let entries = fs::read_dir(&directory)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    scanned += 1;
                    let text = fs::read_to_string(&path)
                        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
                    for (index, line) in text.lines().enumerate() {
                        if line.contains(USER_HOME_LITERAL) {
                            hits.push(format!("{}:{}", path.display(), index + 1));
                        }
                    }
                }
            }
        }
        (hits, scanned)
    }

    #[test]
    fn no_hardcoded_user_paths_in_src() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let (hits, scanned) = hardcoded_user_path_hits(&src);
        // Anti-vacuity: a scan that saw no source files proves nothing.
        assert!(scanned >= 2, "vacuous scan: only {scanned} source files under {}", src.display());
        assert!(
            hits.is_empty(),
            "hardcoded home-path literal(s) reintroduced (this test exists so a \
             reintroduction turns RED): {hits:?}"
        );
    }
}
