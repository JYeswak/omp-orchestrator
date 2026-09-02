#![forbid(unsafe_code)]

//! Drift-class gate for plan prose.
//!
//! The gate rejects numeric `path:line` citations outside Markdown fences and rejects bare tree
//! counts unless the same line carries a dated HISTORICAL boundary or a NUMBERS.toml figure
//! reference. The zero ceilings are a ratchet: a new violation cannot be absorbed by increasing a
//! baseline. This gate does not prove that a named construct is the right one.

use std::fs;
use std::path::{Path, PathBuf};

const MAX_NUMERIC_CITATIONS: usize = 0;
const MAX_UNDOCUMENTED_COUNTS: usize = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    kind: &'static str,
    file: String,
    line: usize,
    token: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("no-shell-gate must be nested under the workspace root")
        .to_path_buf()
}

fn plan_files(root: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(root.join("docs/plan"))
        .expect("docs/plan must be readable")
        .map(|entry| entry.expect("directory entry must be readable").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(|ch: char| ch.is_ascii_digit()) && name.ends_with(".md")
                })
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn is_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';'
        )
}

fn citation_findings(file: &Path, line_number: usize, text: &str) -> Vec<Finding> {
    const EXTENSIONS: &[&str] = &[".rs:", ".md:", ".toml:", ".ts:", ".go:", ".jsonl:"];
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    for extension in EXTENSIONS {
        let mut search_from = 0;
        while let Some(relative) = text[search_from..].find(extension) {
            let extension_start = search_from + relative;
            let digits_start = extension_start + extension.len();
            let digits_end = digits_start
                + bytes[digits_start..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
            if digits_end == digits_start {
                search_from = digits_start;
                continue;
            }
            let mut token_start = extension_start;
            while token_start > 0 && !is_boundary(bytes[token_start - 1]) {
                token_start -= 1;
            }
            findings.push(Finding {
                kind: "citation",
                file: file.display().to_string(),
                line: line_number,
                token: text[token_start..digits_end].to_owned(),
            });
            search_from = digits_end;
        }
    }
    findings
}

fn count_findings(file: &Path, line_number: usize, text: &str) -> Vec<Finding> {
    const UNITS: &[&str] = &[
        "crates",
        "test functions",
        "tests",
        "binary targets",
        "targets",
        "packages",
        "rows",
        "edges",
        "leaves",
    ];
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() || (index > 0 && bytes[index - 1].is_ascii_digit()) {
            index += 1;
            continue;
        }
        let digits_end = index
            + bytes[index..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
        if digits_end - index > 4
            || digits_end == bytes.len()
            || !bytes[digits_end].is_ascii_whitespace()
        {
            index = digits_end.max(index + 1);
            continue;
        }
        let mut unit_start = digits_end;
        while unit_start < bytes.len() && bytes[unit_start].is_ascii_whitespace() {
            unit_start += 1;
        }
        let Some(unit) = UNITS
            .iter()
            .find(|unit| text[unit_start..].starts_with(**unit))
        else {
            index = digits_end;
            continue;
        };
        let token_end = unit_start + unit.len();
        findings.push(Finding {
            kind: "count",
            file: file.display().to_string(),
            line: line_number,
            token: text[index..token_end].to_owned(),
        });
        index = token_end;
    }
    findings
}

fn count_is_documented(text: &str) -> bool {
    let has_date = (text.contains("2026-") || text.contains("2025-"))
        && text
            .as_bytes()
            .windows(5)
            .any(|window| window[0].is_ascii_digit());
    (text.contains("HISTORICAL") && has_date)
        || (text.contains("NUMBERS.toml") && text.contains("figures"))
}

fn scan_lines(
    lines: impl IntoIterator<Item = (String, usize, String)>,
    enabled: bool,
) -> Vec<Finding> {
    if !enabled {
        return Vec::new();
    }
    let mut fenced = false;
    let mut findings = Vec::new();
    for (file, line_number, text) in lines {
        if text.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        findings.extend(citation_findings(Path::new(&file), line_number, &text));
        if !count_is_documented(&text) {
            findings.extend(count_findings(Path::new(&file), line_number, &text));
        }
    }
    findings
}

fn render_findings(findings: &[Finding], scanned_lines: usize) -> String {
    if scanned_lines == 0 {
        return "ERROR: EMPTY_SCAN_SET".to_owned();
    }
    if findings.is_empty() {
        return format!("CLEAN: scanned {scanned_lines} plan prose lines");
    }
    let detail = findings
        .iter()
        .map(|finding| {
            format!(
                "{} {}:{} {}",
                finding.kind, finding.file, finding.line, finding.token
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("VIOLATION: {detail}")
}

fn ratchet_check(findings: &[Finding]) -> Result<(), String> {
    let citations = findings
        .iter()
        .filter(|finding| finding.kind == "citation")
        .count();
    let counts = findings
        .iter()
        .filter(|finding| finding.kind == "count")
        .count();
    if citations > MAX_NUMERIC_CITATIONS {
        return Err(format!(
            "citation ceiling exceeded: {citations} > {MAX_NUMERIC_CITATIONS}"
        ));
    }
    if counts > MAX_UNDOCUMENTED_COUNTS {
        return Err(format!(
            "count ceiling exceeded: {counts} > {MAX_UNDOCUMENTED_COUNTS}"
        ));
    }
    Ok(())
}

fn scan_repo(root: &Path) -> (String, Vec<Finding>) {
    let files = plan_files(root);
    let mut lines = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("plan file must be readable");
        for (line_number, line) in text.lines().enumerate() {
            lines.push((file.display().to_string(), line_number + 1, line.to_owned()));
        }
    }
    let scanned_lines = lines.len();
    let findings = scan_lines(lines, true);
    (render_findings(&findings, scanned_lines), findings)
}

fn fixture_lines(source: &str) -> Vec<(String, usize, String)> {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| ("fixture.md".to_owned(), index + 1, line.to_owned()))
        .collect()
}

#[test]
fn known_good_constructs_fences_and_documented_counts_are_clean() {
    let source = "construct main.rs:send_and_verify\n```text\nmain.rs:123\n26 crates\n```\n26 crates HISTORICAL as of 2026-09-01\n50 crates from NUMBERS.toml figures.workspace_crates";
    let report = render_findings(
        &scan_lines(fixture_lines(source), true),
        source.lines().count(),
    );
    assert!(report.starts_with("CLEAN:"), "expected CLEAN, got {report}");
}

#[test]
fn known_bad_bare_citation_and_count_are_named_in_text() {
    let source = "main.rs:123\n26 crates";
    let report = render_findings(
        &scan_lines(fixture_lines(source), true),
        source.lines().count(),
    );
    assert!(
        report.starts_with("VIOLATION:"),
        "expected VIOLATION, got {report}"
    );
    assert!(
        report.contains("citation fixture.md:1 main.rs:123"),
        "missing citation: {report}"
    );
    assert!(
        report.contains("count fixture.md:2 26 crates"),
        "missing count: {report}"
    );
}

#[test]
fn empty_scan_set_is_an_error_not_a_pass() {
    let report = render_findings(&scan_lines(Vec::new(), true), 0);
    assert_eq!(report, "ERROR: EMPTY_SCAN_SET");
}

#[test]
fn disabling_drift_predicates_silences_known_bad_mutation() {
    let source = "main.rs:123\n26 crates";
    let enabled_findings = scan_lines(fixture_lines(source), true);
    let disabled_findings = scan_lines(fixture_lines(source), false);
    assert!(render_findings(&enabled_findings, source.lines().count()).starts_with("VIOLATION:"));
    assert!(render_findings(&disabled_findings, source.lines().count()).starts_with("CLEAN:"));
}

#[test]
fn test_function_registry_names_all_three_denominators() {
    let numbers = fs::read_to_string(repo_root().join("NUMBERS.toml"))
        .expect("NUMBERS.toml must be readable");
    assert!(
        numbers.contains("#[test] ATTRIBUTES"),
        "missing attribute-count denominator note"
    );
    assert!(
        numbers.contains("PACKAGE AGGREGATE"),
        "missing package aggregate denominator note"
    );
    assert!(
        numbers.contains("LIB SUITE"),
        "missing library-suite denominator note"
    );
}

#[test]
fn plan_citations_name_constructs() {
    let (report, findings) = scan_repo(&repo_root());
    ratchet_check(&findings)
        .unwrap_or_else(|error| panic!("real plan drift ratchet: {error}; {report}"));
    assert!(
        report.starts_with("CLEAN:"),
        "real plan drift gate: {report}"
    );
}
