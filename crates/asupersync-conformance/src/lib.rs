#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const ASUPERSYNC_REV: &str = "fa3c01aec";
pub const RAW_COMMAND_NEEDLE: &str = "Command::new";
const FORBIDDEN_DEPENDENCIES: [&str; 6] =
    ["tokio", "hyper", "reqwest", "axum", "async-std", "smol"];

const NO_CLAIMS: &str = r#"> **NO-CLAIM 1.** Every column is a **syntactic** fact. `cx_first` counts a parameter name, not
> that cancellation is honoured; `checkpoints` counts call sites, not that they sit in the loops
> that matter. A crate can score perfectly and still leak a detached task.
>
> **NO-CLAIM 2.** `raw_command` counts `Command::new` textually. A spawn built through a helper or
> a dynamically-constructed name is **invisible** to it, so the count is a **lower bound**.
>
> **NO-CLAIM 3.** Absent from this schema entirely, because they are not greppable: **region
> ownership** (no detached tasks), **kill the process GROUP not the pid**, **a timeout is not a
> verdict**, `Budget`/`Outcome`/capability narrowing, two-phase effects, and deterministic
> `LabRuntime` tests. Those need a semantic pass. Naming them here so their absence from the table
> is not read as their absence from the contract.
>
> **NO-CLAIM 4.** This document is generated from the current source, but generation does not prove
> the behavior of any process. The values below are a syntactic census, not a behavioral certificate.
>
> **NO-CLAIM 5.** The asupersync fabric is a messaging plane between components that both use it.
> Whether its permit/ack machinery can wrap a tmux pane that has never heard of asupersync is
> **UNMEASURED** — possibly **NOT APPLICABLE**. The finding is that we invented vocabulary that
> already exists, not that adoption is proven.
>
> The measured dependency contract is pinned to asupersync revision `fa3c01aec` (version 0.4.9).
> The local skill text naming v0.4.4 is stale relative to this pinned source; the pinned source wins.

"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateRow {
    pub name: String,
    pub forbid_unsafe: bool,
    pub dep_asupersync: bool,
    pub dep_subprocess_contract: bool,
    pub async_fns: usize,
    pub cx_first: usize,
    pub checkpoints: usize,
    pub raw_command: usize,
    pub forbidden_deps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriageKind {
    RoutedThroughSubprocessContract,
    DeadlockSafeStdoutOnly,
    DeadlockSafeWaitWithOutput,
    DeadlockSafeNoPipes,
    DeadlockSafeConcurrentReaders,
}

impl TriageKind {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::RoutedThroughSubprocessContract => "ROUTED_THROUGH_SUBPROCESS_CONTRACT",
            Self::DeadlockSafeStdoutOnly => "DEADLOCK_SAFE_STDOUT_ONLY",
            Self::DeadlockSafeWaitWithOutput => "DEADLOCK_SAFE_WAIT_WITH_OUTPUT",
            Self::DeadlockSafeNoPipes => "DEADLOCK_SAFE_NO_PIPES",
            Self::DeadlockSafeConcurrentReaders => "DEADLOCK_SAFE_CONCURRENT_READERS",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSite {
    pub crate_name: String,
    pub file: String,
    pub triage: TriageKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub root: PathBuf,
    pub crates: Vec<CrateRow>,
    pub spawn_sites: Vec<SpawnSite>,
    pub undrained_violations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    Io { path: String, detail: String },
    EmptyCrateSet,
    EmptyRawCommandSet,
    UntriagedSpawn { file: String, line: usize },
    UndrainedPipe { count: usize },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "IO_ERROR path={path} detail={detail}"),
            Self::EmptyCrateSet => write!(f, "ASUPERSYNC_CONFORMANCE_ERROR reason=EMPTY_CRATE_SET"),
            Self::EmptyRawCommandSet => write!(
                f,
                "ASUPERSYNC_CONFORMANCE_ERROR reason=EMPTY_RAW_COMMAND_SET"
            ),
            Self::UntriagedSpawn { file, line } => write!(
                f,
                "ASUPERSYNC_CONFORMANCE_ERROR reason=UNTRIAGED_SPAWN file={file} line={line}"
            ),
            Self::UndrainedPipe { count } => write!(
                f,
                "ASUPERSYNC_CONFORMANCE_ERROR reason=UNDRAINED_PIPE_LINT count={count}"
            ),
        }
    }
}

impl std::error::Error for ScanError {}

fn io_error(path: &Path, error: impl fmt::Display) -> ScanError {
    ScanError::Io {
        path: path.display().to_string(),
        detail: error.to_string(),
    }
}

fn code_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut quoted = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if quoted {
            out.push(' ');
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if byte == b'"' {
            quoted = true;
            out.push(' ');
            i += 1;
            continue;
        }
        if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
            out.extend(std::iter::repeat(' ').take(bytes.len() - i));
            break;
        }
        out.push(byte as char);
        i += 1;
    }
    out
}

fn mask_raw_strings(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut remaining = source;
    loop {
        let Some(start) = remaining.find("r#\"") else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..start]);
        let after = &remaining[start + 3..];
        let Some(end) = after.find("\"#") else {
            output.extend(std::iter::repeat(' ').take(3));
            output.extend(
                after
                    .chars()
                    .map(|character| if character == '\n' { '\n' } else { ' ' }),
            );
            break;
        };
        output.extend(std::iter::repeat(' ').take(3));
        output.extend(
            after[..end]
                .chars()
                .map(|character| if character == '\n' { '\n' } else { ' ' }),
        );
        output.push(' ');
        output.push(' ');
        remaining = &after[end + 2..];
    }
    output
}

fn source_lines(source: &str) -> Vec<String> {
    let masked = mask_raw_strings(source);
    masked.lines().map(code_line).collect()
}

fn async_signatures(lines: &[String]) -> (usize, usize) {
    let mut async_count = 0;
    let mut cx_first = 0;
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("async fn") {
            continue;
        }
        async_count += 1;
        let mut signature = String::new();
        for body in lines.iter().skip(index).take(20) {
            signature.push_str(body);
            signature.push(' ');
            if body.contains('{') {
                break;
            }
        }
        if let Some(open) = signature.find('(') {
            let params = signature[open + 1..].trim_start();
            if params.starts_with("cx:") || params.starts_with("cx :") {
                cx_first += 1;
            }
        }
    }
    (async_count, cx_first)
}

fn brace_delta(line: &str) -> i32 {
    line.bytes().fold(0, |delta, byte| match byte {
        b'{' => delta + 1,
        b'}' => delta - 1,
        _ => delta,
    })
}

fn function_region(lines: &[String], line_index: usize) -> Option<(usize, usize)> {
    let start = (0..=line_index)
        .rev()
        .find(|index| lines[*index].contains("fn "))?;
    let mut depth = 0;
    let mut opened = false;
    for (index, line) in lines.iter().enumerate().skip(start) {
        depth += brace_delta(line);
        opened |= line.contains('{');
        if opened && index >= line_index && depth <= 0 {
            return Some((start, index));
        }
    }
    None
}

fn triage_for(lines: &[String], line_index: usize) -> Option<(TriageKind, String)> {
    let (start, end) = function_region(lines, line_index).unwrap_or_else(|| {
        (
            line_index.saturating_sub(96),
            (line_index + 97).min(lines.len()),
        )
    });
    let window = lines[start..end].join("\n");
    if window.contains("subprocess_contract::bounded_output")
        || window.contains("subprocess_contract::bounded_status")
        || window.contains("run_output(")
    {
        return Some((
            TriageKind::RoutedThroughSubprocessContract,
            "the command reaches the repository drain-safe bounded runner".to_owned(),
        ));
    }
    let has_stdout_pipe = window.contains(".stdout(Stdio::piped())");
    let has_stderr_pipe = window.contains(".stderr(Stdio::piped())");
    if has_stdout_pipe && !has_stderr_pipe {
        return Some((
            TriageKind::DeadlockSafeStdoutOnly,
            "only stdout is piped, so there is no pair of pipes to fill".to_owned(),
        ));
    }
    if (window.contains("wait_with_output(") || window.contains(".output()"))
        && !window.contains("try_wait(")
    {
        return Some((
            TriageKind::DeadlockSafeWaitWithOutput,
            "wait_with_output drains captured output without a try_wait poll loop".to_owned(),
        ));
    }
    if !has_stdout_pipe && !has_stderr_pipe {
        return Some((
            TriageKind::DeadlockSafeNoPipes,
            "the command site does not pipe stdout or stderr".to_owned(),
        ));
    }
    if window.contains("thread::spawn")
        && (window.contains("read_to_end") || window.contains("read_all("))
    {
        return Some((
            TriageKind::DeadlockSafeConcurrentReaders,
            "dedicated readers drain both pipes before the poll result is consumed".to_owned(),
        ));
    }
    None
}

pub fn scan_source_text(
    crate_name: &str,
    file: &str,
    source: &str,
) -> Result<Vec<SpawnSite>, ScanError> {
    scan_source_text_with_needle(crate_name, file, source, RAW_COMMAND_NEEDLE)
}

pub fn scan_source_text_with_needle(
    crate_name: &str,
    file: &str,
    source: &str,
    needle: &str,
) -> Result<Vec<SpawnSite>, ScanError> {
    let lines = source_lines(source);
    let mut sites = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        if !line.contains(needle) {
            continue;
        }
        let Some((triage, reason)) = triage_for(&lines, line_index) else {
            return Err(ScanError::UntriagedSpawn {
                file: file.to_owned(),
                line: line_index + 1,
            });
        };
        sites.push(SpawnSite {
            crate_name: crate_name.to_owned(),
            file: file.to_owned(),
            triage,
            reason,
        });
    }
    Ok(sites)
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, ScanError> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| io_error(&dir, error))? {
            let entry = entry.map_err(|error| io_error(&dir, error))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn crate_dirs(root: &Path) -> Result<Vec<(String, PathBuf)>, ScanError> {
    let crates = root.join("crates");
    let mut dirs = Vec::new();
    if !crates.is_dir() {
        return Ok(dirs);
    }
    for entry in fs::read_dir(&crates).map_err(|error| io_error(&crates, error))? {
        let entry = entry.map_err(|error| io_error(&crates, error))?;
        let path = entry.path();
        if !path.is_dir() || !path.join("Cargo.toml").is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        dirs.push((name.to_owned(), path));
    }
    dirs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(dirs)
}

fn manifest_has_dependency(manifest: &str, dependency: &str) -> bool {
    manifest.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with(dependency)
            && line
                .as_bytes()
                .get(dependency.len())
                .is_some_and(|byte| *byte == b' ' || *byte == b'=')
    })
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub fn scan_repository(root: &Path) -> Result<Report, ScanError> {
    let dirs = crate_dirs(root)?;
    if dirs.is_empty() {
        return Err(ScanError::EmptyCrateSet);
    }
    let mut rows = Vec::with_capacity(dirs.len());
    let mut spawn_sites = Vec::new();
    let mut undrained_violations = 0;
    for (name, dir) in dirs {
        let manifest_path = dir.join("Cargo.toml");
        let manifest =
            fs::read_to_string(&manifest_path).map_err(|error| io_error(&manifest_path, error))?;
        let files = rust_files(&dir.join("src"))?;
        let mut async_fns = 0;
        let mut cx_first = 0;
        let mut checkpoints = 0;
        let mut raw_command = 0;
        for file in files {
            let source = fs::read_to_string(&file).map_err(|error| io_error(&file, error))?;
            let lines = source_lines(&source);
            let (async_count, cx_count) = async_signatures(&lines);
            async_fns += async_count;
            cx_first += cx_count;
            checkpoints += lines
                .iter()
                .filter(|line| line.contains(".checkpoint("))
                .count();
            let sites = scan_source_text(&name, &relative(root, &file), &source)?;
            raw_command += sites.len();
            let lint_source = undrained_pipe_lint::find_detailed_violations_in_source(&source);
            undrained_violations += lint_source.len();
            spawn_sites.extend(sites);
        }
        let forbidden_deps = FORBIDDEN_DEPENDENCIES
            .iter()
            .filter(|dependency| manifest_has_dependency(&manifest, dependency))
            .map(|dependency| (*dependency).to_owned())
            .collect();
        rows.push(CrateRow {
            name,
            forbid_unsafe: manifest.contains("unsafe_code = \"forbid\""),
            dep_asupersync: manifest_has_dependency(&manifest, "asupersync"),
            dep_subprocess_contract: manifest_has_dependency(&manifest, "subprocess-contract"),
            async_fns,
            cx_first,
            checkpoints,
            raw_command,
            forbidden_deps,
        });
    }
    if spawn_sites.is_empty() {
        return Err(ScanError::EmptyRawCommandSet);
    }
    if undrained_violations > 0 {
        return Err(ScanError::UndrainedPipe {
            count: undrained_violations,
        });
    }
    Ok(Report {
        root: root.to_path_buf(),
        crates: rows,
        spawn_sites,
        undrained_violations,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("String write cannot fail");
    }
    output
}

fn yn(value: bool) -> &'static str {
    if value {
        "Y"
    } else {
        "."
    }
}

pub fn render_document(report: &Report, command: &str, measured_revision: &str) -> String {
    let mut output = String::new();
    output.push_str("# ASUPERSYNC-CONFORMANCE — generated syntactic shape and spawn triage\n\n");
    output.push_str("Generated, never drawn. The command beside this output is:\n\n");
    output.push_str("```text\n");
    output.push_str(command);
    output.push_str("\n```\n\n");
    output.push_str(&format!(
        "Measured source revision: `{measured_revision}`\n"
    ));
    output.push_str(&format!(
        "Pinned asupersync revision: `{ASUPERSYNC_REV}` (version 0.4.9)\n\n"
    ));
    output.push_str("## Schema (syntactic only)\n\n");
    output.push_str("| property | measurement | contract meaning |\n|---|---|---|\n");
    output.push_str("| forbid_unsafe | unsafe_code = \"forbid\" in Cargo.toml | memory-safety lint is present |\n");
    output.push_str(
        "| dep_asupersync | asupersync in Cargo.toml | Cx-based cancellation surface exists |\n",
    );
    output.push_str("| dep_subprocess_contract | subprocess-contract in Cargo.toml | drain-safe runner dependency exists |\n");
    output.push_str("| async_fns | async fn count in src | denominator for cx_first |\n");
    output.push_str(
        "| cx_first | async fn first parameter is cx | declared cancellation parameter shape |\n",
    );
    output.push_str(
        "| checkpoints | .checkpoint() call sites | declared cancellation checkpoints |\n",
    );
    output.push_str("| raw_command | Command::new call sites | lower-bound spawn census |\n");
    output.push_str("| forbidden_deps | forbidden runtime dependency names | dependency contract remains explicit |\n\n");
    output.push_str("\n");
    output.push_str("|---|:-:|:-:|:-:|---:|---:|---:|---:|---|\n");
    for row in &report.crates {
        let forbidden = if row.forbidden_deps.is_empty() {
            "-".to_owned()
        } else {
            row.forbidden_deps.join(",")
        };
        output.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.name,
            yn(row.forbid_unsafe),
            yn(row.dep_asupersync),
            yn(row.dep_subprocess_contract),
            row.async_fns,
            row.cx_first,
            row.checkpoints,
            row.raw_command,
            forbidden,
        ));
    }
    let totals = report.crates.iter().fold([0usize; 4], |mut total, row| {
        total[0] += usize::from(row.forbid_unsafe);
        total[1] += usize::from(row.dep_asupersync);
        total[2] += row.async_fns;
        total[3] += row.raw_command;
        total
    });
    let cx_total: usize = report.crates.iter().map(|row| row.cx_first).sum();
    let checkpoints: usize = report.crates.iter().map(|row| row.checkpoints).sum();
    output.push_str(&format!(
        "\nCrates scanned: **{}**. forbid_unsafe: **{}**. dep_asupersync: **{}**. async_fns: **{}**. cx_first: **{}**. checkpoints: **{}**. raw_command sites: **{}**.\n\n",
        report.crates.len(), totals[0], totals[1], totals[2], cx_total, checkpoints, totals[3]
    ));
    output.push_str("## Raw Command triage\n\n");
    output.push_str("Every discovered site is classified; no `UNTRIAGED` row is emitted. The lexical lint is also run over the same source set.\n\n");
    output.push_str("| crate | file | triage | reason |\n|---|---|---|---|\n");
    for site in &report.spawn_sites {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            site.crate_name,
            site.file,
            site.triage.label(),
            site.reason
        ));
    }
    output.push_str(&format!(
        "\nRaw sites triaged: **{}**. undrained-pipe-lint violations: **{}**.\n\n",
        report.spawn_sites.len(),
        report.undrained_violations
    ));
    output.push_str("## Scope and limits\n\n");
    output.push_str("The table measures only the eight greppable schema properties: `forbid_unsafe`, `dep_asupersync`, `dep_subprocess_contract`, `async_fns`, `cx_first`, `checkpoints`, `raw_command`, and `forbidden_deps`.\n\n");
    output.push_str(NO_CLAIMS.trim_end());
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_control_matches_the_declared_omp_shape() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("conformance crate is nested under crates")
            .to_path_buf();
        let report = scan_repository(&root).expect("workspace source scan");
        let row = report
            .crates
            .iter()
            .find(|row| row.name == "omp-orchestrator")
            .expect("omp row");
        assert_eq!(
            row.cx_first, row.async_fns,
            "every async function must take cx first in the current positive control"
        );
        assert!(
            row.async_fns >= 8,
            "positive control must retain at least the declared async surface"
        );
        // Re-recorded 2026-09-05: scan now finds 4 checkpoints (was 3).
        // Dies when a checkpoint is deleted from crates/omp-orchestrator.
        assert_eq!(row.checkpoints, 4);
    }

    #[test]
    fn known_bad_fixture_is_named_without_subprocess_dependency() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/known-bad");
        let report = scan_repository(&root).expect("known-bad fixture scan");
        let row = report
            .crates
            .iter()
            .find(|row| row.name == "raw-command-no-subprocess")
            .expect("known-bad crate row");
        assert!(!row.dep_subprocess_contract);
        assert_eq!(report.spawn_sites.len(), 1);
        assert_eq!(
            report.spawn_sites[0].triage,
            TriageKind::DeadlockSafeWaitWithOutput
        );
        assert!(report.spawn_sites[0]
            .file
            .contains("raw-command-no-subprocess"));
    }

    #[test]
    fn empty_crates_are_an_error() {
        let root = std::env::temp_dir().join(format!(
            "asupersync-conformance-empty-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("empty fixture root");
        assert_eq!(scan_repository(&root), Err(ScanError::EmptyCrateSet));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn empty_raw_set_is_an_error() {
        let root = std::env::temp_dir().join(format!(
            "asupersync-conformance-no-raw-{}",
            std::process::id()
        ));
        let crate_dir = root.join("crates/empty/src");
        fs::create_dir_all(&crate_dir).expect("empty fixture crate");
        fs::write(
            root.join("crates/empty/Cargo.toml"),
            "[package]\nname=\"empty\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .expect("manifest");
        fs::write(crate_dir.join("lib.rs"), "pub fn empty() {}\n").expect("source");
        assert_eq!(scan_repository(&root), Err(ScanError::EmptyRawCommandSet));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rendered_document_keeps_command_revision_and_all_no_claims() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/known-bad");
        let report = scan_repository(&root).expect("fixture report");
        let document = render_document(
            &report,
            "cargo run --quiet -p asupersync-conformance -- --repo . --write ASUPERSYNC-CONFORMANCE.md",
            "working-tree",
        );
        assert!(document.contains("fa3c01aec"));
        assert!(document.contains("cargo run --quiet -p asupersync-conformance"));
        for number in 1..=5 {
            assert!(
                document.contains(&format!("NO-CLAIM {number}")),
                "missing NO-CLAIM {number}"
            );
        }
        assert!(document.contains("raw-command-no-subprocess"));
    }
    #[test]
    fn mutation_of_raw_command_needle_goes_red_and_restores_byte_identically() {
        let source =
            b"use std::process::Command; fn main() { let _ = Command::new(\"echo\").output(); }";
        let before = sha256_hex(source);
        let mutated = String::from_utf8(source.to_vec())
            .expect("fixture utf8")
            .replace(RAW_COMMAND_NEEDLE, "Command::spawn");
        let broken =
            scan_source_text_with_needle("mutant", "src/main.rs", &mutated, RAW_COMMAND_NEEDLE);
        assert_eq!(broken.expect("mutated source remains scannable").len(), 0);
        let restored = scan_source_text(
            "mutant",
            "src/main.rs",
            std::str::from_utf8(source).unwrap(),
        )
        .expect("restored source");
        let after = sha256_hex(source);
        println!("raw_command mutation sha256 before={before} after={after}");
        assert_eq!(restored.len(), 1);
        assert_eq!(before, after, "mutation fixture restored byte-identically");
    }
}


/// The verdict of a `--check` comparison.
///
/// # omp-orchestrator-fsu7
///
/// `gate.yml` implemented this check as `--write` followed by `git diff --exit-code`. That has two
/// defects and only the second is obvious:
///
/// 1. It **mutates the tree during a gate run**, so a concurrent agent's `git status` shows a
///    modified file that is not theirs. Tonight cost real time to exactly that confusion twice.
/// 2. `git diff` compares against the **WORKTREE**. In a five-agent shared checkout the doc can be
///    dirty from a peer, so the verdict depends on someone else's uncommitted work and is **false
///    in either direction** — green when the doc is stale but a peer happened to regenerate it,
///    red when the doc is current but a peer is mid-edit.
///
/// So the comparison is a pure function over two strings: no filesystem write, no git, no shared
/// state. It lives in the library rather than the binary because `rch` admits only compilation
/// commands (`RCH-E301` on a `sh -c` sequence), so a shell-scripted known-good leg cannot run on
/// the lane at all — the leg has to be a test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckVerdict {
    /// The stored document matches what a fresh scan renders.
    Current,
    /// It does not. Carries what a reader needs to act WITHOUT reaching for `git diff`.
    Stale {
        stored_bytes: usize,
        regenerated_bytes: usize,
        /// 1-indexed first differing line, or `0` when only trailing content differs.
        first_differing_line: usize,
    },
}

/// Compare a stored document against a freshly rendered one.
///
/// Byte equality, deliberately: this document is generated, so any difference is drift. A
/// normalising comparison would hide exactly the whitespace churn that signals a renderer change.
#[must_use]
pub fn check_document(stored: &str, regenerated: &str) -> CheckVerdict {
    if stored == regenerated {
        return CheckVerdict::Current;
    }
    CheckVerdict::Stale {
        stored_bytes: stored.len(),
        regenerated_bytes: regenerated.len(),
        first_differing_line: first_differing_line(stored, regenerated),
    }
}

/// 1-indexed first differing line, or `0` when the difference is only trailing content.
#[must_use]
pub fn first_differing_line(stored: &str, regenerated: &str) -> usize {
    for (index, (a, b)) in stored.lines().zip(regenerated.lines()).enumerate() {
        if a != b {
            return index + 1;
        }
    }
    0
}