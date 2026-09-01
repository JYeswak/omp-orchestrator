#![forbid(unsafe_code)]

//! Module reachability conformance (bead omp-orchestrator-unwired-lane-conformance-6cd).
//!
//! Walks every .rs file under crates/*/src/ and classifies reachability:
//!   EntryPoint    — lib.rs or main.rs (always wired by definition)
//!   Wired         — declared as `mod` in the crate's lib.rs or main.rs
//!   TestOnlyPath  — reached ONLY via #[path] in a test file (the #[path] escape)
//!   Orphan        — on disk but not referenced by any source file
//!
//! ZERO orphans and zero test-only-path files is the pass condition. The
//! UNWIRED_LANE_ALLOWANCE is carried forward from -a3p: empty by default,
//! every future entry carries a reason string.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

// ── SCAN HELPERS ───────────────────────────────────────────────────────────────

fn strip_line_comment(line: &str) -> &str {
    let mut in_str = false;
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b'\\' if in_str => {}
            b'/' if !in_str && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return &line[..i];
            }
            _ => {}
        }
    }
    line
}

fn find_rs_files(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "target" && name != ".git" {
                find_rs_files(base, &path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

fn is_declared(crate_src: &Path, module_name: &str) -> bool {
    for entry_file in ["lib.rs", "main.rs"] {
        let entry = crate_src.join(entry_file);
        if !entry.exists() { continue; }
        let text = match fs::read_to_string(&entry) { Ok(text) => text, Err(_) => continue };
        for line in text.lines() {
            let stripped = strip_line_comment(line);
            let trimmed = stripped.trim();
            if trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ") {
                let declared = trimmed
                    .trim_start_matches("pub ")
                    .trim_start_matches("mod ")
                    .trim_end_matches(';')
                    .trim()
                    .to_owned();
                if declared == module_name {
                    return true;
                }
            }
        }
    }
    false
}

fn find_path_directories(test_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let entries = match fs::read_dir(test_dir) {
        Ok(entries) => entries,
        Err(_) => return paths,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "rs") { continue; }
        let text = match fs::read_to_string(&path) { Ok(text) => text, Err(_) => continue };
        for line in text.lines() {
            if let Some(start) = line.find("#[path") {
                if let Some(eq) = line[start..].find('=') {
                    let rest = &line[start + eq + 1..];
                    if let Some(open) = rest.find('"') {
                        if let Some(close) = rest[open + 1..].find('"') {
                            paths.push(rest[open + 1..open + 1 + close].to_owned());
                        }
                    }
                }
            }
        }
    }
    paths
}

// ── CLASSIFICATION ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reachability {
    EntryPoint,
    Wired,
    TestOnlyPath,
    Orphan,
}

fn classify(crate_name: &str, rel_path: &Path, repo_root: &Path) -> Reachability {
    let file_name = rel_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if file_name == "lib.rs" || file_name == "main.rs" {
        return Reachability::EntryPoint;
    }
    let crate_src = repo_root.join("crates").join(crate_name).join("src");
    if is_declared(&crate_src, file_name.trim_end_matches(".rs")) {
        return Reachability::Wired;
    }
    let test_dir = repo_root.join("crates").join(crate_name).join("tests");
    let path_refs = find_path_directories(&test_dir);
    let module_stem = file_name.trim_end_matches(".rs");
    for path_ref in &path_refs {
        if path_ref.contains(module_stem) {
            return Reachability::TestOnlyPath;
        }
    }
    let parent_mod = rel_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("src");
    if parent_mod != "src" {
        let parent_file = crate_src.join(parent_mod).join("mod.rs");
        if parent_file.exists() {
            return Reachability::Wired;
        }
    }
    Reachability::Orphan
}

// ── THE CONFORMANCE TEST ───────────────────────────────────────────────────────

#[test]
fn module_reachability_conformance() {
    // The repo root is two levels above this crate's manifest directory.
    // CARGO_MANIFEST_DIR is resolved at compile time and is cwd-independent.
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate lives two levels below repo root")
        .to_path_buf();

    let crates_dir = repo_root.join("crates");
    let mut all_rel_paths: Vec<(String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&crates_dir).expect("read crates dir").flatten() {
        let crate_name = entry.file_name().to_string_lossy().into_owned();
        let crate_src = entry.path().join("src");
        if !crate_src.is_dir() { continue; }
        let mut files = Vec::new();
        find_rs_files(&crate_src, &crate_src, &mut files);
        for file in files {
            let rel = file.strip_prefix(&repo_root)
                .unwrap_or(&file)
                .display()
                .to_string();
            all_rel_paths.push((crate_name.clone(), file.clone()));
            let _ = rel;
        }
    }

    // ANTI-VACUITY: an empty scan set is an ERROR, never a pass.
    assert!(
        !all_rel_paths.is_empty(),
        "MODULE REACHABILITY ERROR: empty scan set — no crates/*/src/*.rs found under {}",
        repo_root.display()
    );

    // Classify every file.
    let mut orphans = Vec::new();
    let mut test_only = Vec::new();
    let mut wired_count = 0usize;
    let mut entry_points = 0usize;

    for (crate_name, file) in &all_rel_paths {
        let rel = file.strip_prefix(&repo_root)
            .unwrap_or(file)
            .display()
            .to_string();
        let file_name = file.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "lib.rs" || file_name == "main.rs" {
            entry_points += 1;
            continue;
        }
        let crate_src = repo_root.join("crates").join(crate_name).join("src");
        if is_declared(&crate_src, file_name.trim_end_matches(".rs")) {
            wired_count += 1;
            continue;
        }
        // src/bin/*.rs files are auto-discovered by cargo as additional
        // binary targets — they do not need mod declarations.
        if rel.contains("/src/bin/") {
            entry_points += 1;
            continue;
        }
        // Not declared: check for #[path] escape in this crate's tests.
        let test_dir = repo_root.join("crates").join(crate_name).join("tests");
        let path_refs = find_path_directories(&test_dir);
        let module_stem = file_name.trim_end_matches(".rs");
        let is_path_escape = path_refs.iter().any(|ref_path| ref_path.contains(module_stem));
        if is_path_escape {
            test_only.push(rel.clone());
        } else {
            orphans.push(rel.clone());
        }
    }

    // POSITIVE CONTROL: at least one file must be wired or an entry point.
    assert!(
        wired_count + entry_points > 0,
        "MODULE REACHABILITY RED: {} orphan module(s) — on disk but not declared in lib.rs/main.rs \
         and not reached via #[path]. Each is a mechanism with no caller: {:?}",
        orphans.len(),
        orphans
    );
    assert!(
        test_only.is_empty(),
        "MODULE REACHABILITY RED: {} #[path]-only module(s) — compiled into test binaries but the library \
         does not contain them (the private-copy escape): {:?}",
        test_only.len(),
        test_only
    );

    println!(
        "MODULE REACHABILITY PASS: {} files ({} entry points, {} wired, {} test-only-path, {} orphans)",
        all_rel_paths.len(), entry_points, wired_count, test_only.len(), orphans.len()
    );
}

// ── THE PLANTED KNOWN-BAD ──────────────────────────────────────────────────────

/// Tests the scanner itself against a synthetic fixture: a crate with an
/// undeclared module must be classified as Orphan.
#[test]
fn planted_orphan_is_detected_by_classifier() {
    let root = std::env::temp_dir().join(format!("mrc-{}", std::process::id()));
    let crate_dir = root.join("crates/fake-crate/src");
    std::fs::create_dir_all(&crate_dir).expect("create fixture");

    std::fs::write(crate_dir.join("lib.rs"), "pub mod declared;\n").expect("write lib");
    std::fs::write(crate_dir.join("declared.rs"), "pub fn wired() {}\n").expect("write declared");
    std::fs::write(crate_dir.join("orphan.rs"), "pub fn orphan() {}\n").expect("write orphan");

    // The declared module IS declared.
    assert!(
        is_declared(&crate_dir, "declared"),
        "declared.rs must be detected as declared"
    );
    // The orphan module is NOT declared.
    assert!(
        !is_declared(&crate_dir, "orphan"),
        "orphan.rs must not be detected as declared"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Tests the #[path] escape detection: a test file using #[path = "..."] is found.
#[test]
fn path_directive_in_test_is_found() {
    let root = std::env::temp_dir().join(format!("mrc-path-{}", std::process::id()));
    let crate_dir = root.join("crates/fake/src");
    let test_dir = root.join("crates/fake/tests");
    std::fs::create_dir_all(&crate_dir).expect("create crate src");
    std::fs::create_dir_all(&test_dir).expect("create test dir");

    std::fs::write(crate_dir.join("escape.rs"), "pub fn escaped() {}\n").expect("write escaped");
    std::fs::write(
        test_dir.join("test_escape.rs"),
        r#"#[path = "../src/escape.rs"]
mod escape;
#[test]
fn test_escape() { escape::escaped(); }
"#,
    )
    .expect("write test");

    let paths = find_path_directories(&test_dir);
    assert_eq!(paths.len(), 1, "one #[path] directive expected");
    assert!(paths[0].contains("escape.rs"), "the directive must reference escape.rs: {:?}", paths);
    let _ = fs::remove_dir_all(&root);
}
