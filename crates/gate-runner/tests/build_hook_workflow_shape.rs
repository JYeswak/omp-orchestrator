//! Shape legs for `.github/workflows/build-hook-macos.yml` (129ny).
//!
//! The gate.yml legs cannot cover this file (they hardcode that path), and a
//! builder workflow that fails to parse starts ZERO jobs and reports NOTHING --
//! indistinguishable from a pass. So this file gets the same text-shape
//! treatment: no YAML loader is strict about duplicate keys in-tree
//! (`yaml.safe_load` takes the last silently), and the mapping-key walk below
//! is the instrument this repo already uses to catch the collapse class.
//!
//! What is asserted: one job; no duplicate mapping key under any parent (the
//! 2026-09-06 collapse was a missing key halving coverage, same family);
//! upload gated on success (no `if:`, no continue-on-error); build precedes
//! upload; the arch proof names arm64; the runner is macOS.
use std::path::PathBuf;

fn read_workflow() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".github/workflows/build-hook-macos.yml");
    std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "BUILD_HOOK_WORKFLOW_UNREADABLE path={} detail={error} -- an unreadable builder is an ERROR, never a pass",
            path.display()
        )
    })
}

/// (indent, is_list_item, key) for mapping-looking lines. Comment and blank
/// lines are skipped. Residual, stated: block-scalar (`|`) content containing
/// a colon at mapping indent would misread -- none exists in this file, and
/// the same class bounds the house text-shape instrument generally.
fn mapping_lines(text: &str) -> Vec<(usize, bool, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.trim().is_empty() || trimmed_end.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let rest = line.trim_start();
        let (is_item, rest) = match rest.strip_prefix("- ") {
            Some(stripped) => (true, stripped),
            None => (false, rest),
        };
        let Some(colon) = rest.find(':') else { continue };
        let key = rest[..colon].trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            continue;
        }
        out.push((indent, is_item, key.to_owned()));
    }
    out
}

#[test]
fn builder_declares_exactly_one_job() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let anchor = lines
        .iter()
        .position(|line| line.trim_end() == "jobs:")
        .expect("BUILD_HOOK_NO_JOBS_ANCHOR -- no `jobs:` line; nothing can run");
    let jobs: Vec<&str> = lines[anchor + 1..]
        .iter()
        .filter_map(|line| {
            let rest = line.strip_prefix("  ")?;
            if rest.starts_with(' ') || rest.starts_with('#') {
                return None;
            }
            rest.strip_suffix(':').map(|_| *line)
        })
        .collect();
    assert_eq!(
        jobs.len(),
        1,
        "builder must declare exactly ONE job; found {}: {:?}",
        jobs.len(),
        jobs
    );
}

#[test]
fn no_duplicate_mapping_key_under_any_parent() {
    // Indent-stack walk: a key may repeat across the file, never twice under
    // one parent. A duplicate silently collapses to the last value -- the
    // 2026-09-06 halving with no error anywhere.
    // Entries are (indent, is_sequence_scope, children). A `- ` list item
    // opens a FRESH scope: keys repeat across items freely (every step has
    // `name:`) but never twice within one item. Without the fresh scope, the
    // second step's `name:` would false-fire against the first's.
    let text = read_workflow();
    let mut stack: Vec<(usize, bool, Vec<String>)> = vec![(0, false, Vec::new())];
    for (indent, is_item, key) in mapping_lines(&text) {
        if is_item {
            while stack.len() > 1 && stack.last().is_some_and(|(ind, _, _)| *ind >= indent) {
                stack.pop();
            }
            stack.push((indent, true, Vec::new()));
        } else {
            while stack.len() > 1
                && stack.last().is_some_and(|(ind, is_seq, _)| {
                    *ind > indent || (*ind == indent && !is_seq)
                })
            {
                stack.pop();
            }
        }
        let (_, _, siblings) = stack.last_mut().expect("root scope");
        assert!(
            !siblings.contains(&key),
            "duplicate mapping key under one parent: {key:?}"
        );
        siblings.push(key.clone());
        if !is_item {
            stack.push((indent, false, Vec::new()));
        }
    }
}

#[test]
fn upload_runs_only_on_success() {
    let text = read_workflow();
    let lines: Vec<&str> = text.lines().collect();
    let upload = lines
        .iter()
        .position(|line| line.contains("actions/upload-artifact"))
        .expect("no upload-artifact step: the artifact would never ship");
    let window = lines[upload.saturating_sub(6)..upload].join("\n");
    assert!(
        !window.contains("if:"),
        "upload step must not carry `if:` -- a RED job must ship nothing"
    );
    assert!(
        !text.contains("continue-on-error"),
        "continue-on-error anywhere ships artifacts past failure"
    );
}

#[test]
fn build_precedes_upload_and_arch_is_pinned() {
    let text = read_workflow();
    let build = text
        .find("cargo build --release --bin pre-commit-gate")
        .expect("no native build step");
    let upload = text
        .find("actions/upload-artifact")
        .expect("no upload step");
    assert!(
        build < upload,
        "build must precede upload: an upload ordered first ships staleness"
    );
    assert!(
        text.contains("Mach-O 64-bit executable arm64"),
        "no arch proof: a wrong-platform artifact must fail loudly, not ship"
    );
    assert!(
        text.contains("runs-on: macos-"),
        "builder must run on macOS: ubuntu cannot emit a native Mach-O"
    );
}
