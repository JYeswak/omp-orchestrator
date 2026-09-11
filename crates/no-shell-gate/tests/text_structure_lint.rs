#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use text_structure::code_only;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("gate crate is beneath workspace")
        .to_path_buf()
}

fn rust_files(root: &Path, crate_name: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for relative in ["src", "tests"] {
        let dir = root.join("crates").join(crate_name).join(relative);
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn gate_crates(root: &Path) -> Result<Vec<String>, String> {
    let census = omp_orchestrator::census_gates(root);
    let mut names: Vec<String> = census.rows.into_iter().map(|row| row.gate).collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err("RAW_TEXT_MATCH_SCAN_EMPTY: census_gates produced no gate rows".to_owned());
    }
    if names.iter().all(|name| rust_files(root, name).is_empty()) {
        return Err(
            "RAW_TEXT_MATCH_SCAN_EMPTY: census gates have no source or test files".to_owned(),
        );
    }
    Ok(names)
}

fn raw_checker_line(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with("//") || line.contains("structure-keyed:") {
        return false;
    }
    if line.contains("text_structure::")
        || line.contains("code_only(")
        || line.contains("manifest_deps(")
        || line.contains("code_and_literals")
        || line.contains("assert!")
        || line.contains("assert_eq!")
        || line.contains("panic!")
        || line.contains("expect(")
    {
        return false;
    }
    if line.contains("grep -c") || line.contains("grep -rl") {
        return true;
    }
    line.contains("source.contains(\"")
        || line.contains("body.contains(\"")
        || line.contains("contents.contains(\"")
        || line.contains("production.contains(\"")
        || line.contains("real_source.contains(\"")
        || line.contains("bundle.contains(\"")
}
fn scan_raw_checker_lines(root: &Path) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    for crate_name in gate_crates(root)? {
        if crate_name == "text-structure" {
            continue;
        }
        for path in rust_files(root, &crate_name) {
            if path.ends_with("text_structure_lint.rs") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|error| {
                format!(
                    "RAW_TEXT_MATCH_READ_ERROR path={} error={error}",
                    path.display()
                )
            })?;
            let code = code_only(&text);
            // DERIVED 2026-09-11 (bead -9ub39): a local comment-matcher is any
            // defined function whose name pairs a stripping verb with a structure
            // noun. The five literal names this replaced missed `strip_line_comment`
            // and `blank_block_comments` — the gate's own matchers — while covering
            // nothing else. A fixture-only `fn` inside a string literal can still
            // fire this; that residual matches the old rule's and is triaged as
            // route-or-rename on sight.
            let matcher_defs = defined_matcher_names(&code);
            let local_matcher = !matcher_defs.is_empty();
            if local_matcher
                && !code.contains("text_structure::code_only")
                && !code.contains("code_and_literals")
                && !text.contains("structure-keyed:")
            {
                findings.push(format!(
                    "RAW_TEXT_MATCH_IN_GATE {}:local matcher definition {:?}",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    matcher_defs
                ));
            }
            // omp-orchestrator-nar5l: DELETED, not repointed. This read
            // `|| path.ends_with("crates/omp-orchestrator/src/main.rs")` -- a path absent from
            // TREE, INDEX and WORKTREE, so the clause exempted NOTHING. Repointing it at
            // `resident.rs` would EXTEND an exemption to a live file that never had one, which is
            // gate self-weakening; a dead exemption is deleted, never migrated.
            let skip_literal_scan = path.to_string_lossy().contains("/tests/");
            let mut in_test_module = false;
            for (line_number, (raw, line)) in text.lines().zip(code.lines()).enumerate() {
                if raw.contains("#[cfg(test)]") {
                    in_test_module = true;
                    continue;
                }
                if skip_literal_scan || in_test_module || raw.trim_start().starts_with("//") {
                    continue;
                }
                if raw_checker_line(line) && !line.contains("command") {
                    findings.push(format!(
                        "RAW_TEXT_MATCH_IN_GATE {}:{}: {}",
                        path.strip_prefix(root).unwrap_or(&path).display(),
                        line_number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    Ok(findings)
}
/// A defined function whose name pairs a stripping verb with a structure noun
/// is a local comment-matcher, whatever else it claims to be.
fn is_local_matcher_name(name: &str) -> bool {
    let verb = name.contains("strip")
        || name.contains("mask")
        || name.contains("blank")
        || name.contains("code_only");
    let noun = name.contains("comment") || name.contains("toml") || name.contains("code");
    verb && noun
}
/// Function names actually DEFINED in blanked source (`fn name(`/either spacing).
///
/// `#[test]`-attributed definitions are specimens, not shipped grammars: three
/// test names describe stripping the subject performs (`line_cite_rot`,
/// `wired-but-inert-guard`) without implementing any. The shipped matcher, if
/// one exists, is a non-test function and still fires. A matcher hiding inside
/// a test body would escape; that residual is documented, not closed.
fn defined_matcher_names(code: &str) -> Vec<&str> {
    let lines: Vec<&str> = code.lines().collect();
    let mut names = Vec::new();
    for (number, line) in lines.iter().enumerate() {
        let Some(fn_at) = line.find("fn ") else {
            continue;
        };
        let rest = &line[fn_at + 3..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if !rest[end..].trim_start().starts_with('(') {
            continue;
        }
        if !is_local_matcher_name(&rest[..end]) {
            continue;
        }
        let mut test_attributed = false;
        let mut back = number;
        while back > 0 {
            back -= 1;
            let above = lines[back].trim();
            if above.is_empty() {
                continue;
            }
            if above.starts_with("#[") {
                test_attributed |= above.contains("#[test");
                continue;
            }
            break;
        }
        if !test_attributed {
            names.push(&rest[..end]);
        }
    }
    names
}

/// `#[test]` specimens do not fire even when their names describe stripping.
#[test]
fn derived_matcher_rule_spares_test_specimens() {
    let defs = defined_matcher_names("#[test]\nfn mutation_without_comment_stripping_counts_comment() {}\n");
    assert!(defs.is_empty(), "test specimens must not fire: {defs:?}");
    let defs = defined_matcher_names("fn strip_line_comment(line: &str) -> &str {}\n");
    assert_eq!(defs, vec!["strip_line_comment"], "plain helpers still fire: {defs:?}");
}

#[test]
fn gate_checker_scan_is_nonempty_and_clean() {
    let root = repo_root();
    let findings = scan_raw_checker_lines(&root).expect("gate scan must be readable");
    assert!(findings.is_empty(), "{}", findings.join("\n"));
}

#[test]
fn known_bad_comment_is_not_a_caller_and_lint_refuses_raw_checker() {
    let root = std::env::temp_dir().join(format!("xrtc-lint-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let path = root.join("crates").join("fixture-gate").join("src");
    fs::create_dir_all(&path).expect("fixture source");
    let file = path.join("lib.rs");
    fs::write(
        &file,
        "// let n = body.contains(\"needle\");\nfn real() { call_needle(); }\n",
    )
    .expect("write fixture");
    let source = fs::read_to_string(&file).expect("read fixture");
    let code = code_only(&source);
    assert!(!code.contains("body.contains(\"needle\")"));
    assert!(code.contains("fn real"));
    assert!(raw_checker_line("let n = body.contains(\"needle\");"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn known_good_structure_keyed_call_is_allowed() {
    assert!(!raw_checker_line(
        "let n = text_structure::code_only(body).contains(\"needle\");"
    ));
}
/// DERIVATION PROOF (bead -9ub39): the rule fires on the SHAPE, not the list.
/// `strip_block_comments` was never enumerated and fires; lookalikes pass.
#[test]
fn derived_matcher_rule_catches_shape_not_list() {
    for name in [
        "strip_line_comment",
        "blank_block_comments",
        "strip_comments",
        "strip_rust_comments",
        "mask_non_code",
        "strip_toml_comment",
        "strip_block_comments",
        "code_only",
        "code_only_lines",
    ] {
        assert!(is_local_matcher_name(name), "{name} must fire");
    }
    for name in [
        "mask_line",
        "code_string_literals",
        "path_component",
        "describe_line",
        "encode_toml",
        "observe_panes",
    ] {
        assert!(!is_local_matcher_name(name), "{name} must pass");
    }
    let defs = defined_matcher_names("fn strip_block_comments(x: &str) { }\nfn real() { }\n");
    assert_eq!(defs, vec!["strip_block_comments"], "only the matcher shape: {defs:?}");
}
#[test]
fn empty_gate_scan_is_an_error() {
    let root = std::env::temp_dir().join(format!("xrtc-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("empty scan root");
    let result = scan_raw_checker_lines(&root);
    assert!(result.is_err(), "empty gate scan must refuse, not pass");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn known_bad_mutation_flips_the_shared_matcher() {
    let source = "// let n = body.contains(\"needle\");\\n";
    let mutated = source.replacen("// ", "", 1);
    assert!(code_only(source).contains("body.contains(\"needle\")") == false);
    assert!(code_only(&mutated).contains("body.contains(\"needle\")"));
}
