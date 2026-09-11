#![forbid(unsafe_code)]

//! Commit-path adapters for the four ratchets that previously ran only as test targets.
//!
//! This module deliberately consumes the staged index, not the mutable worktree, for content
//! checks. The pre-commit binary is the single trigger; a test-only ratchet is not a commit gate
//! until this module calls it from that binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use subprocess_contract::{bounded_output, BoundedOutcome};
use text_structure::{code_only, toml_code_only, yaml_code_only};

const GIT_READ_DEADLINE: Duration = Duration::from_secs(10);
const HOOK_SOURCE_CRATES: &[&str] = &[
    "no-shell-gate",
    "state-wildcard-lint",
    "path-literal-guard",
    "orchestration-tick-gate",
    "undrained-pipe-lint",
];

/// The commit-path result of running the four ratchet adapters.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CommitRatchetReport {
    pub observations: Vec<String>,
    pub refusals: Vec<String>,
}

/// Run all four ratchet adapters for one staged commit.
///
/// The empty-index decision is made by the caller before this function because an empty index is
/// the trigger's own terminal outcome. The other three ratchets are scoped to the staged change
/// or to the installed hook that would enforce it.
#[must_use]
pub fn run(repo_root: &Path, staged: &[String], deletions: &[String]) -> CommitRatchetReport {
    let mut report = CommitRatchetReport::default();
    plan_citations(repo_root, staged, &mut report);
    census_membership(repo_root, staged, &mut report);
    hook_freshness(repo_root, &mut report);
    let _ = deletions;
    report
}

fn plan_citations(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let paths: Vec<&str> = staged
        .iter()
        .map(String::as_str)
        .filter(|path| is_numbered_plan_markdown(path))
        .collect();
    if paths.is_empty() {
        report
            .observations
            .push("plan_citations: GATE_NOT_APPLICABLE reason=no_staged_numbered_plan_markdown".to_owned());
        return;
    }

    let mut findings = Vec::new();
    for path in paths {
        let source = match staged_blob(repo_root, path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(source) => source,
                Err(error) => {
                    report.refusals.push(format!(
                        "plan_citations: ERROR path={path} reason=STAGED_NOT_UTF8 detail={error}"
                    ));
                    continue;
                }
            },
            Err(error) => {
                report
                    .refusals
                    .push(format!("plan_citations: ERROR path={path} reason=STAGED_READ detail={error}"));
                continue;
            }
        };
        findings.extend(scan_plan_document(path, &source));
    }

    if findings.is_empty() {
        report
            .observations
            .push(format!("plan_citations: CLEAN staged_plan_files={}", staged_plan_count(staged)));
    } else {
        report.refusals.extend(findings.into_iter().map(|finding| {
            format!(
                "plan_citations: REFUSED file={}:{} kind={} token={} reason=RATCHET",
                finding.path, finding.line, finding.kind, finding.token
            )
        }));
    }
}

fn census_membership(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let crates: Vec<String> = staged
        .iter()
        .filter_map(|path| path.strip_prefix("crates/"))
        .filter_map(|path| path.strip_suffix("/Cargo.toml"))
        .filter(|name| !name.contains('/') && !name.is_empty())
        .map(str::to_owned)
        .collect();
    if crates.is_empty() {
        report
            .observations
            .push("census_membership: GATE_NOT_APPLICABLE reason=no_staged_crate_manifest".to_owned());
        return;
    }

    let mut changed = Vec::new();
    for crate_name in crates {
        let manifest = repo_root.join("crates").join(&crate_name).join("Cargo.toml");
        let source_dir = manifest
            .parent()
            .map(|path| path.join("src"))
            .unwrap_or_default();
        if !manifest.is_file() {
            report.refusals.push(format!(
                "census_membership: REFUSED crate={crate_name} reason=MANIFEST_UNREADABLE path={}"
                , manifest.display()
            ));
            continue;
        }
        if !source_dir.is_dir() {
            report.refusals.push(format!(
                "census_membership: REFUSED crate={crate_name} reason=NO_CENSUS_ROW detail=manifest_without_src={}"
                , manifest.display()
            ));
            continue;
        }
        if !has_external_caller(repo_root, &crate_name) {
            if has_allowance_row(repo_root, &crate_name) {
                report.observations.push(format!(
                    "census_membership: ALLOWANCE crate={crate_name} reason=NO_SOURCE_VISIBLE_CALLER"
                ));
            } else {
                report.refusals.push(format!(
                    "census_membership: REFUSED crate={crate_name} reason=NO_REACHABLE_TRIGGER detail=no_caller_or_allowance"
                ));
            }
            continue;
        }
        changed.push(crate_name);
    }
    if !changed.is_empty() {
        report.observations.push(format!(
            "census_membership: CLEAN changed_crates={} caller_or_allowance=present",
            changed.join(",")
        ));
    }
}

fn hook_freshness(repo_root: &Path, report: &mut CommitRatchetReport) {
    let hook = repo_root.join(".git/hooks/pre-commit");
    let hook_mtime = match fs::metadata(&hook).and_then(|meta| meta.modified()) {
        Ok(mtime) => mtime,
        Err(error) => {
            report.refusals.push(format!(
                "hook_freshness: REFUSED reason=HOOK_UNREADABLE path={} detail={error}",
                hook.display()
            ));
            return;
        }
    };
    let Some((newest_path, newest_mtime)) = newest_hook_source(repo_root) else {
        report.observations.push(
            "hook_freshness: GATE_NOT_APPLICABLE reason=NO_HOOK_SOURCES_IN_THIS_CHECKOUT".to_owned(),
        );
        return;
    };
    if newest_mtime > hook_mtime {
        report.refusals.push(format!(
            "hook_freshness: REFUSED reason=STALE_HOOK hook={} source={}",
            hook.display(),
            newest_path.display()
        ));
    } else {
        report.observations.push(format!(
            "hook_freshness: CLEAN hook={} newest_source={}",
            hook.display(),
            newest_path.display()
        ));
    }
}

fn newest_hook_source(repo_root: &Path) -> Option<(PathBuf, SystemTime)> {
    let mut newest = None;
    for crate_name in HOOK_SOURCE_CRATES {
        let source_root = repo_root.join("crates").join(crate_name).join("src");
        visit_rust_sources(&source_root, &mut newest);
    }
    newest
}

fn visit_rust_sources(root: &Path, newest: &mut Option<(PathBuf, SystemTime)>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rust_sources(&path, newest);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let Ok(mtime) = fs::metadata(&path).and_then(|meta| meta.modified()) else {
            continue;
        };
        if newest.as_ref().is_none_or(|(_, current)| mtime > *current) {
            *newest = Some((path, mtime));
        }
    }
}

fn has_external_caller(repo_root: &Path, crate_name: &str) -> bool {
    let hyphen = crate_name.to_owned();
    let underscore = crate_name.replace('-', "_");
    let roots = [repo_root.join("crates"), repo_root.join(".github/workflows")];
    roots.iter().any(|root| contains_external_reference(root, crate_name, &hyphen, &underscore))
}

fn contains_external_reference(
    root: &Path,
    owned_crate: &str,
    hyphen: &str,
    underscore: &str,
) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some(owned_crate)
                && root.file_name().and_then(|name| name.to_str()) == Some("crates")
            {
                continue;
            }
            if contains_external_reference(&path, owned_crate, hyphen, underscore) {
                return true;
            }
            continue;
        }
        let is_source = matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml" | "yml" | "yaml")
        );
        if !is_source {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let code = code_text_for_path(&path, &text);
        if code.contains(hyphen) || code.contains(underscore) {
            return true;
        }
    }
    false
}

/// Code text for census matching, dispatched by file extension (bead -9ub39).
///
/// Routes through the kernels instead of a local comment grammar: Rust through
/// `code_only` (which nests block comments correctly — the old loop closed at
/// the first `*/` without depth), TOML through `toml_code_only` (which is
/// `"`-quote-aware — the old loop stripped `#` inside strings), YAML through
/// `yaml_code_only` (which additionally requires whitespace before `#`).
/// Anything else passes through untouched, exactly as before.
fn code_text_for_path(path: &Path, text: &str) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => code_only(text).into_owned(),
        Some("toml") => text
            .lines()
            .map(|line| toml_code_only(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
        Some("yml" | "yaml") => text
            .lines()
            .map(|line| yaml_code_only(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => text.to_owned(),
    }
}

fn has_allowance_row(repo_root: &Path, crate_name: &str) -> bool {
    let path = repo_root.join("crates/no-shell-gate/tests/wired_lanes.rs");
    fs::read_to_string(path)
        .map(|text| text.contains(&format!("\"{crate_name}\"")))
        .unwrap_or(false)
}

fn staged_blob(repo_root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let spec = format!(":{path}");
    let mut command = Command::new("git");
    command.current_dir(repo_root).args(["show", &spec]);
    match bounded_output(&mut command, GIT_READ_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(output.stdout),
        BoundedOutcome::Completed(output) => Err(format!(
            "git show {spec} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        BoundedOutcome::TimedOut => Err(format!("git show {spec} exceeded deadline; group killed")),
        BoundedOutcome::Unspawned(error) => Err(error.to_string()),
    }
}

fn is_numbered_plan_markdown(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("docs/plan/") else {
        return false;
    };
    let Some(first) = relative.split('/').next() else {
        return false;
    };
    first.starts_with(|character: char| character.is_ascii_digit()) && path.ends_with(".md")
}

fn staged_plan_count(staged: &[String]) -> usize {
    staged
        .iter()
        .filter(|path| is_numbered_plan_markdown(path))
        .count()
}

#[derive(Debug, PartialEq, Eq)]
struct PlanFinding {
    kind: &'static str,
    path: String,
    line: usize,
    token: String,
}

fn scan_plan_document(path: &str, source: &str) -> Vec<PlanFinding> {
    let mut findings = Vec::new();
    let mut fenced = false;
    for (line_index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        findings.extend(citation_findings(path, line_index + 1, line));
        if !count_is_documented(line) {
            findings.extend(count_findings(path, line_index + 1, line));
        }
    }
    findings
}

fn citation_findings(path: &str, line: usize, text: &str) -> Vec<PlanFinding> {
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
            while token_start > 0 && !is_plan_boundary(bytes[token_start - 1]) {
                token_start -= 1;
            }
            findings.push(PlanFinding {
                kind: "citation",
                path: path.to_owned(),
                line,
                token: text[token_start..digits_end].to_owned(),
            });
            search_from = digits_end;
        }
    }
    findings
}

fn count_findings(path: &str, line: usize, text: &str) -> Vec<PlanFinding> {
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
        let Some(unit) = UNITS.iter().find(|unit| text[unit_start..].starts_with(**unit)) else {
            index = digits_end;
            continue;
        };
        findings.push(PlanFinding {
            kind: "count",
            path: path.to_owned(),
            line,
            token: text[index..unit_start + unit.len()].to_owned(),
        });
        index = unit_start + unit.len();
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

fn is_plan_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_ratchet_names_added_line_citations_and_fenced_text_passes() {
        let findings = scan_plan_document(
            "docs/plan/00-brief.md",
            "ok\ncrates/foo/src/lib.rs:42\n```\ncrates/bar/src/lib.rs:99\n```\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].kind, "citation");
    }

    #[test]
    fn census_ignores_comment_only_references() {
        let rust = code_text_for_path(
            Path::new("fixture.rs"),
            "// unowned-gate\n/* unowned-gate */\nfn clean() {}\n",
        );
        assert!(!rust.contains("unowned-gate"));
        let toml = code_text_for_path(Path::new("fixture.toml"), "# unowned-gate\nname = \"clean\"\n");
        assert!(!toml.contains("unowned-gate"));
    }

    #[test]
    fn absent_sources_are_a_typed_census_nonapplicable_not_a_false_clean() {
        let report = run(Path::new("/nonexistent/yqun"), &[], &[]);
        assert!(report
            .observations
            .iter()
            .any(|line| line.contains("plan_citations: GATE_NOT_APPLICABLE")));
        assert!(report
            .observations
            .iter()
            .any(|line| line.contains("census_membership: GATE_NOT_APPLICABLE")));
        assert!(report
            .refusals
            .iter()
            .any(|line| line.contains("hook_freshness: REFUSED")));
    }
}
