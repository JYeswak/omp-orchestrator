//! SPAWN CONTRACT GATE — a crate that spawns a child process must route through
//! `subprocess-contract`, or carry a named allowance with a reason.
//!
//! # The binding contract this enforces
//!
//! `AGENTS.md`, on asupersync 0.4.9:
//!
//! > Every subprocess — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work
//! > with a deadline. […] **Kill the process GROUP, never the pid.** […] **Drain
//! > both pipes.** Undrained stdout+stderr with a `try_wait()` poll deadlocks past
//! > ~64 KiB. […] **A timeout is not a verdict.**
//!
//! Each of those is a property of *how* a child is spawned, so a bare
//! `Command::new` cannot satisfy them. `subprocess-contract` is where they live.
//!
//! # Computed state
//!
//! The test walks every workspace crate's Rust sources, counts code-level
//! Command::new expressions (ignoring comments and string literals), and checks
//! the corresponding manifest dependency with a syntax-aware parser. The count
//! is deliberately computed at test time: a copied measurement becomes stale as
//! extraction waves land.
//!
//! The dependency check is a routing floor, not proof that every call site uses
//! the kernel. The good and mutation legs below keep the census attributable.
//! Per-call-site proof still requires the source-level audit performed alongside
//! each extraction wave.
//!
//! # Why an allowance list rather than a hard failure
//!
//! Lints and test harnesses spawn `cargo`, `git` and `grep` in a build context
//! where a deadline is the harness's job, not theirs. Forcing them through the
//! runtime contract would be ceremony. So the list is **explicit and reasoned**,
//! following `franken_lean`'s `UNWIRED_LANE_ALLOWANCE` shape — an exception is a
//! named row with a reason, never silence.
//!
//! # What this does NOT prove
//!
//! Declaring the dependency is not using it. A crate can depend on
//! `subprocess-contract` and still call `Command::new` directly beside it —
//! `subprocess-contract` itself does, legitimately, since it *is* the wrapper.
//! Proving every call site routes correctly needs per-site analysis, which is
//! unbuilt. This gate raises the floor from *no relationship at all* to *declared
//! relationship or stated reason*, which is strictly weaker than the contract and
//! strictly stronger than nothing.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

static SPAWN_CENSUS_LOCK: Mutex<()> = Mutex::new(());

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// Crates permitted to spawn without the runtime contract, each with the reason.
///
/// Adding a row is cheap and auditable; leaving one out is a build failure. That
/// asymmetry is the point.
const SPAWN_ALLOWANCE: &[(&str, &str)] = &[
    (
        "subprocess-contract",
        "it IS the wrapper — its own Command::new calls are the implementation",
    ),
    (
        "no-shell-gate",
        "test harness: spawns cargo/git/grep to derive figures, under the harness's own deadline",
    ),
    (
        "pre-delete-citation-check",
        "pre-commit-time check, bounded by the hook's lifetime rather than a runtime deadline",
    ),
    (
        "receiver-receipt",
        "single tmux capture-pane read at hook time; ALLOWANCE IS WEAK — this is runtime-adjacent \
         and should route through the contract once the fence lands",
    ),
    (
        "installer",
        "PROVISIONAL, and the most user-visible of these: 5 sites. A hung install is a failure \
         a real adopter experiences directly, so this is the allowance whose absence of a \
         deadline costs the most",
    ),
    (
        "dispatch-silence-watch",
        "PROVISIONAL: 2 sites. Wired as a path dependency this session (gate-wiring-wave2-at2), \
         so it is newly live and inherits no deadline yet",
    ),
    (
        "tick-monitor",
        "PROVISIONAL, recorded as debt not as a decision: 4 sites spawning tmux from the \
         orchestrator's primary sensor. This is the allowance most likely to be wrong, and \
         GradeCrates flagged its sibling in round 13",
    ),
    (
        "omp-rpc-session",
        "PROVISIONAL, recorded as debt: 3 sites spawning the OMP RPC child, which is exactly \
         the cancellable-work-with-a-deadline case the contract exists for",
    ),
];

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    if bytes.get(cursor) == Some(&b'b') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0usize;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    cursor += 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            let mut end = cursor + 1;
            let mut closed = true;
            for _ in 0..hashes {
                if bytes.get(end) != Some(&b'#') {
                    closed = false;
                    break;
                }
                end += 1;
            }
            if closed {
                return Some(end);
            }
        }
        cursor += 1;
    }
    None
}

fn count_spawn_expressions(source: &str) -> usize {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment(usize),
        String,
        Char,
    }

    let bytes = source.as_bytes();
    let mut state = State::Code;
    let mut cursor = 0usize;
    let mut count = 0usize;
    while cursor < bytes.len() {
        match state {
            State::Code => {
                if let Some(end) = raw_string_end(bytes, cursor) {
                    cursor = end;
                    continue;
                }
                if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'/') {
                    state = State::LineComment;
                    cursor += 2;
                } else if bytes.get(cursor) == Some(&b'/')
                    && bytes.get(cursor + 1) == Some(&b'*')
                {
                    state = State::BlockComment(1);
                    cursor += 2;
                } else if bytes.get(cursor) == Some(&b'"') {
                    state = State::String;
                    cursor += 1;
                } else if bytes.get(cursor) == Some(&b'\'') {
                    state = State::Char;
                    cursor += 1;
                } else if bytes[cursor..].starts_with(b"Command::new") {
                    count += 1;
                    cursor += b"Command::new".len();
                } else {
                    cursor += 1;
                }
            }
            State::LineComment => {
                if bytes[cursor] == b'\n' {
                    state = State::Code;
                }
                cursor += 1;
            }
            State::BlockComment(mut depth) => {
                if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'*') {
                    depth += 1;
                    state = State::BlockComment(depth);
                    cursor += 2;
                } else if bytes.get(cursor) == Some(&b'*')
                    && bytes.get(cursor + 1) == Some(&b'/')
                {
                    depth -= 1;
                    cursor += 2;
                    if depth == 0 {
                        state = State::Code;
                    } else {
                        state = State::BlockComment(depth);
                    }
                } else {
                    cursor += 1;
                    state = State::BlockComment(depth);
                }
            }
            State::String | State::Char => {
                let quote = match state {
                    State::String => b'"',
                    State::Char => b'\'',
                    _ => unreachable!(),
                };
                if bytes[cursor] == 92 {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == quote {
                    state = State::Code;
                    cursor += 1;
                } else {
                    cursor += 1;
                }
            }
        }
    }
    count
}

fn crates_with_spawn(root: &Path) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return out;
    };
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let name = match dir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let mut count = 0usize;
        let mut stack = vec![dir.join("src")];
        while let Some(d) = stack.pop() {
            let Ok(items) = std::fs::read_dir(&d) else {
                continue;
            };
            for it in items.flatten() {
                let p = it.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|x| x != "rs") {
                    continue;
                }
                if let Ok(body) = std::fs::read_to_string(&p) {
                    count += count_spawn_expressions(&body);
                }
            }
        }
        if count > 0 {
            out.push((name, count));
        }
    }
    out.sort();
    out
}

fn manifest_has_dependency(manifest: &str, dependency: &str) -> bool {
    let mut in_dependencies = false;
    for raw_line in manifest.lines() {
        let line = raw_line.split_once('#').map_or(raw_line, |(head, _)| head).trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_dependencies = line == "[dependencies]"
                || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
            if line
                .strip_prefix("[dependencies.")
                .and_then(|section| section.strip_suffix(']'))
                == Some(dependency)
            {
                return true;
            }
            continue;
        }
        if in_dependencies {
            let Some((key, _value)) = line.split_once('=') else {
                continue;
            };
            if key.trim().trim_matches('"') == dependency {
                return true;
            }
        }
    }
    false
}

fn declares_contract(root: &Path, crate_name: &str) -> bool {
    let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
    std::fs::read_to_string(manifest)
        .map(|text| manifest_has_dependency(&text, "subprocess-contract"))
        .unwrap_or(false)
}

fn unrouted_spawners(root: &Path) -> Result<Vec<String>, String> {
    let spawners = crates_with_spawn(root);
    if spawners.is_empty() {
        return Err(
            "ANTI-VACUITY: no crate contains Command::new; the spawn census is empty".to_owned(),
        );
    }
    let allowed: std::collections::HashSet<&str> =
        SPAWN_ALLOWANCE.iter().map(|(c, _)| *c).collect();
    let unrouted: Vec<String> = spawners
        .iter()
        .filter(|(c, _)| !declares_contract(root, c) && !allowed.contains(c.as_str()))
        .map(|(c, n)| format!("{c} ({n} site(s))"))
        .collect();
    if unrouted.is_empty() {
        Ok(unrouted)
    } else {
        Err(format!(
            "{} crate(s) spawn child processes with no subprocess-contract dependency and no allowance row: {:?}",
            unrouted.len(), unrouted
        ))
    }
}

#[test]
fn every_spawning_crate_routes_through_the_contract_or_is_allowed() {
    let _lock = SPAWN_CENSUS_LOCK.lock().expect("spawn census lock");
    let root = repo_root();
    let unrouted = unrouted_spawners(&root).unwrap_or_else(|error| panic!("{error}"));
    assert!(unrouted.is_empty());
}

struct InTreeSpecimen {
    path: PathBuf,
}

impl Drop for InTreeSpecimen {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn known_bad_in_tree_spawn_specimen_is_named() {
    let _lock = SPAWN_CENSUS_LOCK.lock().expect("spawn census lock");
    let path = repo_root()
        .join("crates")
        .join(format!("w3yb1-spawn-specimen-{}", std::process::id()));
    std::fs::create_dir_all(path.join("src")).expect("specimen source directory");
    std::fs::write(
        path.join("Cargo.toml"),
        "[package]\nname=\"w3yb1-spawn-specimen\"\nversion=\"0.0.0\"\nedition=\"2024\"\n",
    )
    .expect("specimen manifest");
    std::fs::write(
        path.join("src/lib.rs"),
        "pub fn violates_contract() { let _ = std::process::Command::new(\"omp\"); }\n",
    )
    .expect("specimen source");
    let _fixture = InTreeSpecimen { path: path.clone() };
    let error = unrouted_spawners(&repo_root()).expect_err(&format!(
        "known-bad specimen must be refused: {}",
        path.display()
    ));
    assert!(error.contains("w3yb1-spawn-specimen"), "{error}");
}

#[test]
fn empty_spawn_census_is_an_error() {
    let root = std::env::temp_dir().join(format!("w3yb1-empty-census-{}", std::process::id()));
    std::fs::create_dir_all(root.join("crates")).expect("empty census fixture");
    let result = unrouted_spawners(&root);
    std::fs::remove_dir_all(&root).expect("remove empty census fixture");
    let error = result.expect_err("empty spawn scan must be an error");
    assert!(error.contains("ANTI-VACUITY"), "{error}");
}

#[test]
fn every_allowance_row_names_a_crate_that_still_spawns() {
    // An allowance for a crate that no longer spawns is a stale exemption, and a
    // stale exemption is how an allowlist quietly becomes a rubber stamp. This is
    // the same rot the gate census had when it hardcoded three crates as
    // permanently Unreachable.
    let root = repo_root();
    let spawners: std::collections::HashSet<String> = crates_with_spawn(&root)
        .into_iter()
        .map(|(c, _)| c)
        .collect();
    let stale: Vec<&str> = SPAWN_ALLOWANCE
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !spawners.contains(*c))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowance row(s) name a crate that no longer spawns: {:?}\n\
         Remove them — an exemption nobody needs is an exemption nobody rechecks.",
        stale.len(),
        stale
    );
}

#[test]
fn every_allowance_row_carries_a_reason() {
    let empty: Vec<&str> = SPAWN_ALLOWANCE
        .iter()
        .filter(|(_, why)| why.trim().len() < 20)
        .map(|(c, _)| *c)
        .collect();
    assert!(
        empty.is_empty(),
        "{} allowance row(s) carry no usable reason: {:?}\n\
         A row without a reason is silence with extra steps.",
        empty.len(),
        empty
    );
}

#[test]
fn census_ignores_comments_and_string_literals() {
    let source = r#"
        // Command::new("comment")
        let _text = "Command::new(\"string\")";
        let _real = Command::new("echo");
    "#;
    assert_eq!(count_spawn_expressions(source), 1);
}

#[test]
fn census_known_positive_is_specific() {
    assert_eq!(count_spawn_expressions("let _ = Command::new(\"echo\");"), 1);
}

#[test]
fn manifest_parser_accepts_both_dependency_assignment_spellings() {
    assert!(manifest_has_dependency(
        "[dependencies]\nsubprocess-contract = { path = \"../subprocess-contract\" }",
        "subprocess-contract"
    ));
    assert!(manifest_has_dependency(
        "[dependencies]\nsubprocess-contract={path=\"../subprocess-contract\"}",
        "subprocess-contract"
    ));
    assert!(!manifest_has_dependency(
        "[package]\nname=\"subprocess-contract\"",
        "subprocess-contract"
    ));
}
