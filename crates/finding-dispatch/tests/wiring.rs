use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("finding-dispatch lives under crates/")
        .to_path_buf()
}

fn code_without_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_depth = 0usize;

    while index < bytes.len() {
        if block_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                block_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                block_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
        } else if bytes.get(index..index + 2) == Some(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
        } else if bytes.get(index..index + 2) == Some(b"/*") {
            block_depth = 1;
            index += 2;
        } else {
            output.push(bytes[index] as char);
            index += 1;
        }
    }

    output
}

#[test]
fn finding_dispatch_has_an_external_production_caller() {
    let root = repo_root();
    // omp-orchestrator-nar5l: this read named `crates/omp-orchestrator/src/main.rs`, a path
    // absent from TREE, INDEX and WORKTREE -- the crate is lib-plus-`src/bin/`. Both needles
    // asserted below live in `resident.rs` (`git grep -ln` on each), so the CLAIM was true and
    // only the address was dead. Repointed at the file that carries the behaviour, never at
    // `lib.rs`, which would resolve without carrying it.
    // AND THE FAILURE NAMES ITS OWN PATH. `.expect("omp-orchestrator production source")` panics
    // with an `Os { code: 2 }` and NO path, so a dead citation here produced an unattributable
    // failure -- the reader cannot tell which file went missing. That is the same
    // error-attribution defect nar5l is about, inside nar5l's own known-bad leg.
    let production = root.join("crates/omp-orchestrator/src/resident.rs");
    let main = fs::read_to_string(&production).unwrap_or_else(|error| {
        panic!(
            "cited production source is unreadable: {} detail={error}",
            production.display()
        )
    });
    let code = code_without_comments(&main);

    assert!(
        code.contains("finding_dispatch::finding_for(decision, seen)"),
        "production code must invoke finding-dispatch; deleting the call site must fail this test"
    );
    assert!(
        code.contains("file_supervisor_finding(cx, config, tick, &decision).await?"),
        "the supervisor cycle must reach the finding caller; deleting the wiring must fail this test"
    );

    let manifest = fs::read_to_string(root.join("crates/omp-orchestrator/Cargo.toml"))
        .expect("omp-orchestrator manifest");
    assert!(
        manifest.contains("finding = { path = \"../finding\" }")
            && manifest.contains("finding-dispatch = { path = \"../finding-dispatch\" }"),
        "both finding crates must remain declared dependencies"
    );
}

#[test]
fn known_wired_subprocess_contract_is_a_positive_control() {
    let source = fs::read_to_string(repo_root().join("crates/ack-spine/src/ack.rs"))
        .expect("known wired production source");
    let code = code_without_comments(&source);
    assert!(
        code.contains("subprocess_contract::run_output"),
        "positive control must find a known external subprocess-contract caller"
    );
}
