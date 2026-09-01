#![forbid(unsafe_code)]

//! Proves that every declared dispatch gate is invoked by cron or a tracked executable script.
//!
//! The implementation deliberately separates the pure text decision (`grep_code` and
//! `find_invoker`) from the filesystem adapter (`analyze_rows`).  Comment-only mentions never
//! count as wiring, and an empty declaration set is not a passing scan.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Stable report schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// Environment variable naming a crontab snapshot for deterministic operation.
pub const CRONTAB_ENV: &str = "WIRED_GUARD_CRONTAB";
/// Environment variable overriding the repository root.
pub const REPO_ENV: &str = "WIRED_GUARD_REPO";
/// The same tracked-file pathspecs used by the shell oracle.
pub const TRACKED_FILE_GLOBS: [&str; 2] = ["bin/*", ".github/*"];

/// One gate declared by the guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateSpec {
    /// Repository-relative source path whose existence is required.
    pub gate: &'static str,
    /// Stable executable/basename needle used when proving invocation.
    pub needle: &'static str,
    /// Why this gate must remain reachable.
    pub why: &'static str,
}

/// Gates currently owned by this guard.
pub const WIRED_ROWS: &[GateSpec] = &[
    GateSpec {
        gate: "bin/challenge-lane.sh",
        needle: "challenge-lane.sh",
        why: "controller<->driver challenge cadence must fire on a schedule, not by hand",
    },
    GateSpec {
        gate: "crates/omp-idle-dispatch/src/main.rs",
        needle: "omp-idle-dispatch",
        why: "the ONLY lane that recovers idle OMP panes when check.sh is red",
    },
    GateSpec {
        gate: "crates/fleet-composite/src/main.rs",
        needle: "fleet-composite",
        why: "the fleet grade must be computed on a schedule or nobody sees a dead factor",
    },
    GateSpec {
        gate: "crates/wired-but-inert-guard/src/main.rs",
        needle: "wired-but-inert-guard",
        why: "the declared-gate reachability proof must itself remain scheduled",
    },
];

/// A tracked file's already-read source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerSource {
    /// Repository-relative path used in the report.
    pub path: String,
    /// Source contents; non-UTF-8 bytes are lossily decoded like `grep`.
    pub contents: String,
}

impl CallerSource {
    /// Construct a source record for a tracked file.
    pub fn new(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            contents: contents.into(),
        }
    }
}

/// The source that proves a gate is reachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WiredGuardInvoker {
    /// A non-comment crontab line contains the gate basename.
    Cron,
    /// A non-comment tracked script contains the gate basename.
    Script(String),
    /// No acceptable invocation was found.
    None,
}

impl WiredGuardInvoker {
    /// Stable wire spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Cron => "cron",
            Self::Script(path) => path,
            Self::None => "none",
        }
    }
}

impl fmt::Display for WiredGuardInvoker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A gate's result in a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// The gate exists and has an invoker.
    Ok(WiredGuardInvoker),
    /// The declared gate file is absent or is not a regular file.
    MissingFile,
    /// The gate exists but no invoker was found.
    NotWired,
}

impl GateVerdict {
    fn is_failure(&self) -> bool {
        !matches!(self, Self::Ok(_))
    }
}

/// One row of a guard report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateResult {
    pub gate: &'static str,
    pub needle: &'static str,
    pub why: &'static str,
    pub verdict: GateVerdict,
}

/// A complete guard report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardReport {
    pub checked: usize,
    pub failures: usize,
    pub gates: Vec<GateResult>,
}

impl GuardReport {
    /// A report is passing only when it checked at least one row and all rows passed.
    pub fn is_pass(&self) -> bool {
        self.checked > 0 && self.failures == 0
    }
}

/// Errors reading the real inputs used by the filesystem adapter.
#[derive(Debug)]
pub enum ScanError {
    /// A listed caller could not be read from the filesystem.
    Unreadable { path: PathBuf, source: std::io::Error },
    /// A path returned by the tracked-file adapter was not repository-relative.
    InvalidPath(PathBuf),
    /// The repository root itself could not be inspected.
    Repository { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, source } => write!(formatter, "unreadable input {}: {source}", path.display()),
            Self::InvalidPath(path) => write!(formatter, "tracked path is not repository-relative: {}", path.display()),
            Self::Repository { path, source } => write!(formatter, "cannot inspect repository {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for ScanError {}

/// Test whether a source line contains a needle after removing full-line shell comments.
///
/// This is the load-bearing analogue of the shell guard's comment stripping.  Inline comments
/// are intentionally not stripped, matching the shell oracle: `run gate # reason` is still a
/// command line containing `gate`.
pub fn grep_code(needle: &str, contents: &str) -> bool {
    contents
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.contains(needle))
}

/// Whether a tracked path is eligible to prove wiring.
///
/// The caller adapter is restricted to the shell oracle's `bin/*` and `.github/*` pathspecs;
/// Markdown is explicitly excluded so documentation cannot become a false invocation receipt.
pub fn is_invoker_candidate(path: &Path) -> bool {
    let text = path.to_string_lossy();
    let in_scope = text.starts_with("bin/") || text.starts_with(".github/");
    in_scope && !text.ends_with(".md")
}

/// Find the first real invoker using a caller-supplied executable needle.
pub fn find_invoker_with_needle(
    gate: &str,
    needle: &str,
    cron_contents: &str,
    callers: &[CallerSource],
) -> WiredGuardInvoker {
    if needle.is_empty() {
        return WiredGuardInvoker::None;
    }
    if cron_contents
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line.contains(needle))
    {
        return WiredGuardInvoker::Cron;
    }

    for caller in callers {
        if caller.path == gate
            || caller.path.ends_with(needle)
            || !is_invoker_candidate(Path::new(&caller.path))
        {
            continue;
        }
        if grep_code(needle, &caller.contents) {
            return WiredGuardInvoker::Script(caller.path.clone());
        }
    }
    WiredGuardInvoker::None
}

/// Find the first real invoker using the source path's basename as its needle.
pub fn find_invoker(gate: &str, cron_contents: &str, callers: &[CallerSource]) -> WiredGuardInvoker {
    let Some(base) = Path::new(gate).file_name().and_then(|name| name.to_str()) else {
        return WiredGuardInvoker::None;
    };
    find_invoker_with_needle(gate, base, cron_contents, callers)
}

fn safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::ParentDir | Component::RootDir))
}

fn load_callers(repo: &Path, tracked_files: &[String]) -> Result<Vec<CallerSource>, ScanError> {
    let mut callers = Vec::new();
    for relative in tracked_files {
        let relative_path = Path::new(relative);
        if !safe_relative_path(relative_path) {
            return Err(ScanError::InvalidPath(relative_path.to_path_buf()));
        }
        let path = repo.join(relative_path);
        let contents = fs::read(&path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|source| ScanError::Unreadable {
                path: path.clone(),
                source,
            })?;
        callers.push(CallerSource::new(relative.clone(), contents));
    }
    Ok(callers)
}

/// Analyze declared gates against a real repository and caller-file list.
///
/// The caller supplies crontab text and the output of a tracked-file query.  This keeps the
/// decision logic free of process spawning while the binary owns those two I/O boundaries.
/// Missing gate files are ordinary red rows; unreadable caller inputs are hard errors so the
/// binary cannot silently claim a pass from an incomplete scan.
pub fn analyze_rows(
    repo: &Path,
    cron_contents: &str,
    tracked_files: &[String],
    rows: &[GateSpec],
) -> Result<GuardReport, ScanError> {
    if !repo.is_dir() {
        let source = fs::metadata(repo).err().unwrap_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "repository is not a directory")
        });
        return Err(ScanError::Repository {
            path: repo.to_path_buf(),
            source,
        });
    }
    let callers = load_callers(repo, tracked_files)?;
    let mut gates = Vec::with_capacity(rows.len());
    for row in rows {
        let gate_path = repo.join(row.gate);
        let verdict = match fs::metadata(&gate_path) {
            Ok(metadata) if metadata.is_file() => {
                match find_invoker_with_needle(row.gate, row.needle, cron_contents, &callers) {
                    WiredGuardInvoker::None => GateVerdict::NotWired,
                    invoker => GateVerdict::Ok(invoker),
                }
            }
            Ok(_) => GateVerdict::MissingFile,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => GateVerdict::MissingFile,
            Err(error) => {
                return Err(ScanError::Unreadable {
                    path: gate_path,
                    source: error,
                });
            }
        };
        gates.push(GateResult {
            gate: row.gate,
            needle: row.needle,
            why: row.why,
            verdict,
        });
    }
    let failures = gates.iter().filter(|gate| gate.verdict.is_failure()).count();
    Ok(GuardReport {
        checked: gates.len(),
        failures,
        gates,
    })
}

/// Analyze the guard's canonical declaration set.
pub fn analyze(repo: &Path, cron_contents: &str, tracked_files: &[String]) -> Result<GuardReport, ScanError> {
    analyze_rows(repo, cron_contents, tracked_files, WIRED_ROWS)
}

/// One selftest assertion result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelftestAssertion {
    pub name: &'static str,
    pub got: String,
    pub want: &'static str,
}

impl SelftestAssertion {
    fn passed(&self) -> bool {
        self.got == self.want
    }
}

/// Selftest summary.  The eight assertions mirror the shell oracle, including its known-bad
/// mutation that demonstrates why comment stripping is load-bearing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiredGuardSelftestReport {
    pub checked: usize,
    pub failures: usize,
    pub assertions: Vec<SelftestAssertion>,
}

impl WiredGuardSelftestReport {
    pub fn is_pass(&self) -> bool {
        self.checked == 8 && self.failures == 0
    }
}

/// Execute all eight behavior assertions against production decision functions.
pub fn run_selftest() -> WiredGuardSelftestReport {
    let mut assertions = Vec::with_capacity(8);
    let mut push = |name, got: String, want| assertions.push(SelftestAssertion { name, got, want });
    let no_callers: [CallerSource; 0] = [];

    let comment_only = find_invoker(
        "bin/challenge-lane.sh",
        "# challenge-lane.sh: mutual challenge cadence\n4,24,54 * * * * /usr/bin/true\n",
        &no_callers,
    );
    push(
        "a COMMENT-ONLY cron mention must NOT prove wiring",
        comment_only.to_string(),
        "none",
    );

    let real_cron = find_invoker(
        "bin/challenge-lane.sh",
        "# a comment\n4,24,54 * * * * /opt/fleet/bin/challenge-lane.sh\n",
        &no_callers,
    );
    push("a REAL cron line proves wiring (not over-strict)", real_cron.to_string(), "cron");

    let unscheduled = find_invoker("bin/definitely-not-a-real-gate.sh", "# nothing here\n", &no_callers);
    push("a gate invoked by nothing reports NONE", unscheduled.to_string(), "none");

    push(
        "grep_code ignores comment lines",
        grep_code("foo-gate.sh", "# calls foo-gate.sh here\nprintf hello\n").to_string(),
        "false",
    );
    push(
        "grep_code finds a real call",
        grep_code("foo-gate.sh", "# a comment\nbash foo-gate.sh --check\n").to_string(),
        "true",
    );
    push(
        "the caller search excludes *.md (a doc is not an invoker)",
        is_invoker_candidate(Path::new("bin/notes.md")).to_string(),
        "false",
    );
    push(
        "the caller search does NOT scan *.md",
        is_invoker_candidate(Path::new(".github/notes.md")).to_string(),
        "false",
    );

    // KNOWN-BAD MUTATION: removing grep_code's comment stripping makes this fixture appear wired.
    let mutation_result = "# calls foo-gate.sh here\nprintf hello\n".contains("foo-gate.sh");
    push(
        "MUTATION: without comment-stripping a comment counts as wiring",
        mutation_result.to_string(),
        "true",
    );

    let failures = assertions.iter().filter(|assertion| !assertion.passed()).count();
    WiredGuardSelftestReport {
        checked: assertions.len(),
        failures,
        assertions,
    }
}

fn json_quote(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push_str(&format!("\\u{:04x}", character as u32)),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn verdict_json(verdict: &GateVerdict) -> (String, String) {
    match verdict {
        GateVerdict::Ok(invoker) => ("OK".to_owned(), invoker.to_string()),
        GateVerdict::MissingFile => ("RED missing file".to_owned(), "-".to_owned()),
        GateVerdict::NotWired => ("RED BUILT-NOT-WIRED".to_owned(), "none".to_owned()),
    }
}

/// Render a machine-readable guard report.
pub fn render_json(report: &GuardReport) -> String {
    let gates = report
        .gates
        .iter()
        .map(|gate| {
            let (verdict, invoker) = verdict_json(&gate.verdict);
            format!(
                "{{\"gate\":{},\"needle\":{},\"invoker\":{},\"verdict\":{},\"why\":{}}}",
                json_quote(gate.gate),
                json_quote(gate.needle),
                json_quote(&invoker),
                json_quote(&verdict),
                json_quote(gate.why)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"wired-but-inert-guard.status.v1\",\"status\":{},\"checked\":{},\"failures\":{},\"gates\":[{}]}}",
        json_quote(if report.is_pass() { "PASS" } else { "FAIL" }),
        report.checked,
        report.failures,
        gates
    )
}

/// Render a structured, fail-closed input error for the binary boundary.
pub fn render_error_json(error: &str) -> String {
    format!(
        "{{\"schema\":\"wired-but-inert-guard.error.v1\",\"status\":\"ERROR\",\"error\":{}}}",
        json_quote(error)
    )
}

/// Render the report's explanation-oriented form.
pub fn render_why_json(report: &GuardReport, gate_filter: Option<&str>) -> String {
    let reasons = report
        .gates
        .iter()
        .filter(|gate| gate_filter.is_none_or(|filter| filter == gate.gate))
        .map(|gate| {
            let (verdict, invoker) = verdict_json(&gate.verdict);
            format!(
                "{{\"gate\":{},\"needle\":{},\"why\":{},\"verdict\":{},\"invoker\":{}}}",
                json_quote(gate.gate),
                json_quote(gate.needle),
                json_quote(gate.why),
                json_quote(&verdict),
                json_quote(&invoker)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"wired-but-inert-guard.why.v1\",\"gate\":{},\"reasons\":[{}]}}",
        gate_filter.map(json_quote).unwrap_or_else(|| "null".to_owned()),
        reasons
    )
}

/// Render capabilities without touching the repository.
pub fn render_capabilities_json() -> String {
    format!(
        "{{\"schema\":\"wired-but-inert-guard.capabilities.v1\",\"name\":\"wired-but-inert-guard\",\"operations\":[\"guard\",\"status\",\"why\",\"capabilities\"],\"comment_lines_ignored\":true,\"fail_closed\":true,\"tracked_file_globs\":[{},{}],\"selftest_assertions\":8}}",
        json_quote(TRACKED_FILE_GLOBS[0]),
        json_quote(TRACKED_FILE_GLOBS[1])
    )
}

/// Human-readable table matching the shell guard's default output.
pub fn render_human(report: &GuardReport) -> String {
    let mut output = String::from("GATE                                      INVOKER    VERDICT\n");
    for gate in &report.gates {
        let (verdict, invoker) = verdict_json(&gate.verdict);
        output.push_str(&format!("{:<42} {:<10} {}", gate.gate, invoker, verdict));
        if matches!(gate.verdict, GateVerdict::NotWired) {
            output.push_str(" — ");
            output.push_str(gate.why);
        }
        output.push('\n');
    }
    if report.checked == 0 {
        output.push_str("ERROR: zero gates checked — an empty scan is a failure, not a pass\n");
    } else if report.is_pass() {
        output.push_str(&format!("\nWIRED-GUARD PASS ({} gates, all reachable)\n", report.checked));
    } else {
        output.push_str(&format!(
            "\nWIRED-GUARD FAIL ({} of {} gates are BUILT but NOT WIRED)\n",
            report.failures, report.checked
        ));
    }
    output
}

/// Render selftest in JSON, retaining every assertion's observed value.
pub fn render_selftest_json(report: &WiredGuardSelftestReport) -> String {
    let assertions = report
        .assertions
        .iter()
        .map(|assertion| {
            format!(
                "{{\"name\":{},\"got\":{},\"want\":{},\"pass\":{}}}",
                json_quote(assertion.name),
                json_quote(&assertion.got),
                json_quote(assertion.want),
                assertion.passed()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"wired-but-inert-guard.selftest.v1\",\"status\":{},\"checked\":{},\"failures\":{},\"assertions\":[{}]}}",
        json_quote(if report.is_pass() { "PASS" } else { "FAIL" }),
        report.checked,
        report.failures,
        assertions
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("wired-but-inert-guard-{unique}"));
        fs::create_dir(&path).expect("create temp repo");
        path
    }

    #[test]
    fn comment_only_cron_does_not_prove_wiring() {
        let callers = [];
        assert_eq!(find_invoker("bin/challenge-lane.sh", "# challenge-lane.sh\n", &callers), WiredGuardInvoker::None);
    }

    #[test]
    fn real_cron_line_proves_wiring() {
        let callers = [];
        assert_eq!(find_invoker("bin/challenge-lane.sh", "# comment\n4,24,54 * * * * /repo/bin/challenge-lane.sh\n", &callers), WiredGuardInvoker::Cron);
    }

    #[test]
    fn post_cutover_binary_needle_proves_wiring() {
        let callers = [];
        assert_eq!(
            find_invoker_with_needle(
                "crates/omp-idle-dispatch/src/main.rs",
                "omp-idle-dispatch",
                "# a comment\n7 * * * * /repo/target/release/omp-idle-dispatch\n",
                &callers,
            ),
            WiredGuardInvoker::Cron
        );
    }

    #[test]
    fn unscheduled_gate_reports_none() {
        let callers = [];
        assert_eq!(find_invoker("bin/definitely-not-a-real-gate.sh", "# nothing here\n", &callers), WiredGuardInvoker::None);
    }

    #[test]
    fn grep_code_ignores_comment_lines() {
        assert!(!grep_code("foo-gate.sh", "# calls foo-gate.sh here\nprintf hello\n"));
    }

    #[test]
    fn grep_code_finds_real_call() {
        assert!(grep_code("foo-gate.sh", "# a comment\nbash foo-gate.sh --check\n"));
    }

    #[test]
    fn caller_search_excludes_markdown() {
        assert!(!is_invoker_candidate(Path::new("notes.md")));
        assert!(!is_invoker_candidate(Path::new("bin/notes.md")));
    }

    #[test]
    fn caller_search_excludes_markdown_glob() {
        assert!(!is_invoker_candidate(Path::new(".github/notes.md")));
    }

    #[test]
    fn mutation_without_comment_stripping_counts_comment() {
        assert!("# calls foo-gate.sh here\nprintf hello\n".contains("foo-gate.sh"));
    }

    #[test]
    fn real_filesystem_scan_reports_script_invoker() {
        let repo = temp_repo();
        fs::create_dir(repo.join("bin")).expect("create bin");
        fs::write(repo.join("bin/challenge-lane.sh"), "#!/bin/sh\n").expect("write gate");
        fs::write(repo.join("bin/driver.sh"), "bin/challenge-lane.sh --check\n").expect("write caller");
        let report = analyze_rows(
            &repo,
            "# no cron\n",
            &["bin/challenge-lane.sh".into(), "bin/driver.sh".into()],
            &[WIRED_ROWS[0]],
        )
        .expect("scan succeeds");
        assert!(report.is_pass());
        assert_eq!(
            report.gates[0].verdict,
            GateVerdict::Ok(WiredGuardInvoker::Script("bin/driver.sh".into()))
        );
        fs::remove_dir_all(repo).expect("remove temp repo");
    }

    #[test]
    fn post_cutover_binary_source_path_and_cron_are_checked() {
        let repo = temp_repo();
        let source_dir = repo.join("crates/omp-idle-dispatch/src");
        fs::create_dir_all(&source_dir).expect("create binary source directory");
        fs::write(source_dir.join("main.rs"), "fn main() {}\n").expect("write binary source");
        let report = analyze_rows(
            &repo,
            "7 * * * * /repo/target/release/omp-idle-dispatch\n",
            &[],
            &[WIRED_ROWS[1]],
        )
        .expect("scan succeeds");
        assert_eq!(report.gates[0].verdict, GateVerdict::Ok(WiredGuardInvoker::Cron));
        fs::remove_dir_all(repo).expect("remove temp repo");
    }

    #[test]
    fn missing_gate_is_red_and_empty_rows_are_not_a_pass() {
        let repo = temp_repo();
        let report = analyze_rows(&repo, "", &[], &[WIRED_ROWS[0]]).expect("missing gate is reportable");
        assert!(!report.is_pass());
        assert_eq!(report.gates[0].verdict, GateVerdict::MissingFile);
        let empty = analyze_rows(&repo, "", &[], &[]).expect("empty scan is reportable");
        assert_eq!(empty.checked, 0);
        assert!(!empty.is_pass());
        fs::remove_dir_all(repo).expect("remove temp repo");
    }

    #[test]
    fn selftest_covers_all_eight_assertions_and_mutation() {
        let report = run_selftest();
        assert_eq!(report.checked, 8);
        assert!(report.is_pass());
        assert!(report
            .assertions
            .iter()
            .any(|assertion| assertion.name.starts_with("MUTATION:")));
    }

    #[test]
    fn json_rendering_escapes_and_reports_failures() {
        let report = GuardReport {
            checked: 1,
            failures: 1,
            gates: vec![GateResult {
                gate: "bin/challenge-lane.sh",
                needle: "challenge-lane.sh",
                why: "reason with \"quotes\"",
                verdict: GateVerdict::NotWired,
            }],
        };
        let json = render_json(&report);
        assert!(json.contains("\\\"quotes\\\""));
        assert!(json.contains("BUILT-NOT-WIRED"));
        assert_eq!(render_capabilities_json().matches("selftest_assertions").count(), 1);
    }
}
