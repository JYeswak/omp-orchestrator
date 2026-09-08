use std::fs;
use std::path::{Path, PathBuf};
use text_structure::code_only;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellReference {
    path: PathBuf,
    line: usize,
    token: String,
}

fn is_path_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '/' | '.' | '-' | '_')
}

fn code_shell_references(source: &str, path: &Path) -> Vec<ShellReference> {
    let code = code_only(source);
    let mut references = Vec::new();
    for (offset, _) in code.match_indices("bin/") {
        if offset > 0
            && code[..offset]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            continue;
        }
        let token: String = code[offset..]
            .chars()
            .take_while(|character| is_path_char(*character))
            .collect();
        if !token.ends_with(".sh") {
            continue;
        }
        let line = code[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1;
        references.push(ShellReference {
            path: path.to_owned(),
            line,
            token,
        });
    }
    references
}

fn rust_files(root: &Path, own_path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|error| {
        format!(
            "SHELL_REGROWTH_SCAN_UNREADABLE path={} detail={error}",
            root.display()
        )
    })?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("SHELL_REGROWTH_SCAN_UNREADABLE detail={error}"))?;
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, own_path, out)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") && path != own_path {
            out.push(path);
        }
    }
    Ok(())
}

fn scan_files(files: Vec<PathBuf>) -> Result<Vec<ShellReference>, String> {
    if files.is_empty() {
        return Err("SHELL_REGROWTH_SCAN_EMPTY reason=no_rust_sources".to_owned());
    }
    let mut scanned_bytes = 0usize;
    let mut references = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path).map_err(|error| {
            format!(
                "SHELL_REGROWTH_SCAN_UNREADABLE path={} detail={error}",
                path.display()
            )
        })?;
        scanned_bytes += source.len();
        references.extend(code_shell_references(&source, &path));
    }
    if scanned_bytes == 0 {
        return Err("SHELL_REGROWTH_SCAN_EMPTY reason=zero_source_bytes".to_owned());
    }
    Ok(references)
}

fn scan_n7mjb_sources(repo: &Path) -> Result<Vec<ShellReference>, String> {
    let own_path = repo.join("crates/loop-coverage/tests/forbidden_shell_regrowth.rs");
    let mut files = Vec::new();
    // Scope the gate to the affected porting surfaces; the wider workspace has separate owners.
    for relative in [
        "crates/loop-coverage/src",
        "crates/loop-coverage/tests",
        "crates/fast-dispatch/src",
        "crates/cargo-lane-budget/src",
        "crates/cargo-lane-budget/tests",
    ] {
        rust_files(&repo.join(relative), &own_path, &mut files)?;
    }
    scan_files(files)
}

#[test]
fn affected_rust_sources_have_no_forbidden_shell_paths() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root resolves");
    let references = scan_n7mjb_sources(&repo)
        .unwrap_or_else(|error| panic!("shell regrowth scan failed: {error}"));
    assert!(references.is_empty(), "forbidden shell references: {references:?}");
}

#[test]
fn comments_are_ignored_but_code_literals_fire() {
    let comment = "// bin/comment-only.sh\n/* bin/block-only.sh */\n";
    assert!(code_shell_references(comment, Path::new("fixture.rs")).is_empty());

    let code = r#"let path = "bin/zzz-planted.sh";"#;
    assert_eq!(
        code_shell_references(code, Path::new("fixture.rs")),
        vec![ShellReference {
            path: PathBuf::from("fixture.rs"),
            line: 1,
            token: "bin/zzz-planted.sh".to_owned(),
        }]
    );
}

#[test]
fn detector_own_source_is_excluded_and_empty_scan_is_typed() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root resolves");
    let references = scan_n7mjb_sources(&repo).expect("real source corpus is readable");
    assert!(
        references.is_empty(),
        "the detector's own planted specimen leaked: {references:?}"
    );

    let error = scan_files(Vec::new()).expect_err("an empty corpus must be typed");
    assert_eq!(error, "SHELL_REGROWTH_SCAN_EMPTY reason=no_rust_sources");

    let empty = PathBuf::from("/path/that/cannot/exist/n7mjb");
    let error = rust_files(&empty, Path::new("/none"), &mut Vec::new())
        .expect_err("an absent scan root must be typed");
    assert!(error.starts_with("SHELL_REGROWTH_SCAN_UNREADABLE"), "{error}");
}
