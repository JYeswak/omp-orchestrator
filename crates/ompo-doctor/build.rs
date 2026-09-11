//! Generate the adapter roster from the WORKSPACE, not from a literal.
//!
//! `omp-orchestrator-jplf.7.2` / `UAD-REGISTRY` + `LAW-UAD-ROSTER-DERIVED`: adding a bin
//! target to this workspace must change `ompo`'s adapter roster with NO source edit. A
//! hand-maintained list is the defect -- three integers in this repo's history (20, 23, 48)
//! were all false by the time they were read.
//!
//! WHY THIS DOES NOT GREP `[[bin]]`. Measured 2026-09-07: `tick-monitor` declares
//! `[[bin]]` ZERO times and is nonetheless a bin target present on PATH, because cargo
//! auto-discovers `src/main.rs` and `src/bin/*.rs`. A `[[bin]]` grep therefore INVERTS the
//! census. This replicates cargo's own discovery rules instead:
//!
//!   * every explicit `[[bin]]` block's `name`
//!   * an implicit bin named after the package when `src/main.rs` exists and no explicit
//!     block already claims that path
//!   * one implicit bin per `src/bin/*.rs`, named after the file stem, unless claimed
//!
//! THE ORACLE IS A TEST, NOT THIS FILE. `roster_tracks_the_runner_not_a_literal` compares
//! the generated roster against `cargo metadata --no-deps` target kinds. If cargo's rules and
//! these rules ever disagree, that test goes RED naming the difference -- which is the only
//! reason hand-parsing the manifest is admissible here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const UNKNOWN: &str = "unknown";

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let crates_dir = manifest_dir
        .parent()
        .expect("crates/<pkg> has a parent")
        .to_path_buf();
    let build_commit = provenance_value("OMPO_BUILD_COMMIT", &manifest_dir);
    let source_revision = provenance_value("OMPO_SOURCE_REVISION", &manifest_dir);
    println!("cargo:rustc-env=OMPO_BUILD_COMMIT={build_commit}");
    println!("cargo:rustc-env=OMPO_SOURCE_REVISION={source_revision}");
    println!("cargo:rerun-if-env-changed=OMPO_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=OMPO_SOURCE_REVISION");
    watch_git_inputs(&manifest_dir);

    println!("cargo:rerun-if-changed={}", crates_dir.display());

    // THE ROSTER MUST HONOUR `exclude`, NOT JUST `members`.
    //
    // `read_dir` over `crates/` replicates the `members = ["crates/*"]` glob and NOTHING
    // else, so an excluded directory still carries a `Cargo.toml` and still enumerated —
    // producing a roster with one bin target cargo does not build. Measured 2026-09-11:
    // `roster_tracks_the_runner_not_a_literal` reported "89 oracle targets, 90 generated.
    // EXTRA in the roster (addresses nothing): [\"omp-idle-dispatch\"]", and
    // `capabilities_drift_is_red` failed on the same one-name delta through
    // `ompo capabilities --json`. Two red legs, one cause, one key.
    //
    // The workspace manifest is a real build input for this reason: adding an `exclude`
    // entry changes the generated roster, so a stale cache here would re-introduce the
    // drift after the manifest was fixed.
    let workspace_manifest = crates_dir
        .parent()
        .expect("crates/ has a parent")
        .join("Cargo.toml");
    println!("cargo:rerun-if-changed={}", workspace_manifest.display());
    let excluded = excluded_dirs(&workspace_manifest);

    let mut adapters: BTreeSet<String> = BTreeSet::new();
    let entries = std::fs::read_dir(&crates_dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", crates_dir.display()));
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        // An excluded directory is NOT a workspace member, so its bins are not cargo
        // targets and must not enter the roster. Skipped here rather than filtered later
        // so the anti-vacuity assert below still guards the real population.
        if dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| excluded.contains(name))
        {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", manifest.display());
        let text = match std::fs::read_to_string(&manifest) {
            Ok(text) => text,
            // A member whose manifest cannot be read is NOT silently skipped into a smaller
            // roster: a shrinking roster is how this surface goes vacuously green.
            Err(error) => panic!("cannot read {}: {error}", manifest.display()),
        };
        for name in bin_names(&dir, &text) {
            adapters.insert(name);
        }
    }

    assert!(
        !adapters.is_empty(),
        "UAD_EMPTY_ROSTER: zero bin targets discovered under {} -- an empty roster is an \
         ERROR, never a pass",
        crates_dir.display()
    );

    let rows: Vec<String> = adapters.iter().map(|name| format!("    {name:?},")).collect();
    let generated = format!(
        "// @generated by crates/ompo-doctor/build.rs -- do not edit.\n\
         pub static ADAPTERS: &[&str] = &[\n{}\n];\n",
        rows.join("\n")
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("adapters.rs");
    std::fs::write(&out, generated)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", out.display()));
}

/// Directory names the workspace manifest EXCLUDES, as bare basenames.
///
/// Hand-parsed for the same reason the rest of this file is: `build.rs` cannot call
/// `cargo metadata` without recursing into cargo. `roster_tracks_the_runner_not_a_literal`
/// is what makes that admissible — it compares this generator against cargo's own target
/// discovery and goes RED naming the difference in both directions, so a parsing gap here
/// surfaces as a named failure rather than as a quietly wrong roster.
///
/// Accepts the single-line form (`exclude = ["crates/foo"]`) and the multi-line array,
/// stopping at the closing bracket so a later key cannot leak in. An ABSENT or unreadable
/// manifest yields an EMPTY set, which is the correct default: it excludes nothing and
/// leaves the roster a superset, which `roster_tracks_the_runner_not_a_literal` catches.
/// The opposite default would silently shrink the roster, and a shrinking roster is how
/// this surface goes vacuously green.
fn excluded_dirs(workspace_manifest: &Path) -> BTreeSet<String> {
    let Ok(text) = std::fs::read_to_string(workspace_manifest) else {
        return BTreeSet::new();
    };
    let mut excluded = BTreeSet::new();
    let mut in_array = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !in_array {
            let Some(rest) = trimmed.strip_prefix("exclude") else {
                continue;
            };
            let Some(rest) = rest.trim_start().strip_prefix('=') else {
                continue;
            };
            in_array = true;
            collect_quoted_basenames(rest, &mut excluded);
        } else {
            collect_quoted_basenames(trimmed, &mut excluded);
        }
        if trimmed.contains(']') {
            in_array = false;
        }
    }
    excluded
}

/// Push the LAST path component of every double-quoted entry in `line`.
///
/// `exclude` entries are workspace-relative paths (`crates/omp-idle-dispatch`); the
/// roster loop walks `crates/` and compares directory names, so the basename is the
/// join key.
fn collect_quoted_basenames(line: &str, out: &mut BTreeSet<String>) {
    for (index, piece) in line.split('"').enumerate() {
        // Odd indices are the insides of quote pairs.
        if index % 2 == 1 && !piece.is_empty() {
            let basename = piece.rsplit('/').next().unwrap_or(piece);
            if !basename.is_empty() {
                out.insert(basename.to_owned());
            }
        }
    }
}

include!("src/revision_env.rs");

fn provenance_value(name: &str, manifest_dir: &Path) -> String {
    let git = git_head(manifest_dir);
    // Presence semantics live in revision_env (unit-pinned); this adapter only
    // supplies the two sources in preference order.
    clean_env_value(std::env::var(name).ok()).unwrap_or(git)
}

/// Resolve the build's source identity from checkout HEAD, with a deterministic fallback.
fn git_head(manifest_dir: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let head = String::from_utf8(output.stdout).ok()?.trim().to_owned();
            (!head.is_empty()).then_some(head)
        })
        .unwrap_or_else(|| UNKNOWN.to_owned())
}

/// Watch every git input that can change the value returned by git_head.
fn watch_git_inputs(manifest_dir: &Path) {
    let git_dir = Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let raw = String::from_utf8(output.stdout).ok()?.trim().to_owned();
            if raw.is_empty() {
                return None;
            }
            let path = PathBuf::from(raw);
            Some(if path.is_absolute() { path } else { manifest_dir.join(path) })
        });
    let Some(git_dir) = git_dir else { return; };
    for name in ["HEAD", "index", "packed-refs"] {
        println!("cargo:rerun-if-changed={}", git_dir.join(name).display());
    }
    if let Ok(contents) = std::fs::read_to_string(git_dir.join("HEAD")) {
        if let Some(reference) = contents.strip_prefix("ref: ").map(str::trim) {
            if !reference.is_empty() {
                println!("cargo:rerun-if-changed={}", git_dir.join(reference).display());
            }
        }
    }
}

/// Every bin target name cargo would discover for one package directory.
fn bin_names(dir: &Path, manifest: &str) -> Vec<String> {
    let package_name = table_value(manifest, "[package]", "name");
    let explicit = explicit_bins(manifest);

    let mut names: Vec<String> = explicit.iter().map(|(name, _)| name.clone()).collect();
    let claimed: BTreeSet<&str> = explicit.iter().map(|(_, path)| path.as_str()).collect();

    if dir.join("src/main.rs").is_file() && !claimed.contains("src/main.rs") {
        if let Some(name) = package_name {
            names.push(name);
        }
    }
    if let Ok(bins) = std::fs::read_dir(dir.join("src/bin")) {
        for bin in bins.flatten() {
            let path = bin.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let relative = format!(
                "src/bin/{}",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("")
            );
            if claimed.contains(relative.as_str()) {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_owned());
            }
        }
    }
    names
}

/// `name` from the first `[[bin]]` blocks, paired with the `path` each one claims.
fn explicit_bins(manifest: &str) -> Vec<(String, String)> {
    let mut bins = Vec::new();
    let mut in_bin = false;
    let mut name: Option<String> = None;
    let mut path: Option<String> = None;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_bin {
                if let Some(found) = name.take() {
                    bins.push((found, path.take().unwrap_or_default()));
                } else {
                    path = None;
                }
            }
            in_bin = trimmed == "[[bin]]";
            continue;
        }
        if !in_bin {
            continue;
        }
        if let Some(value) = key_value(trimmed, "name") {
            name = Some(value);
        } else if let Some(value) = key_value(trimmed, "path") {
            path = Some(value);
        }
    }
    if in_bin {
        if let Some(found) = name {
            bins.push((found, path.unwrap_or_default()));
        }
    }
    bins
}

/// `key = "value"` from inside a named table, first occurrence.
fn table_value(manifest: &str, table: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == table;
            continue;
        }
        if inside {
            if let Some(value) = key_value(trimmed, key) {
                return Some(value);
            }
        }
    }
    None
}

fn key_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}
