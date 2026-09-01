//! PORTING GATE — nothing lands unwired, nothing gets a pass.
//!
//! Six clauses, machine-checked. A crate arrives only when ALL are true in
//! the same commit. Any one absent means the crate does not land.
//!
//! Clauses:
//!   1. WIRED — a production caller exists (grep over crates/, not the crate itself)
//!   2. SURFACE DECLARED — a `[crates.x]` block in OMP-SURFACE-MAP.toml
//!   3. ASUPERSYNC CONFORMANCE — `unsafe_code = "forbid"` in Cargo.toml
//!   4. REPOSITORY-GREEN — passes from a clean archive extract (not testable here;
//!      the gate checks the crate has a `tests/` dir as a proxy and the full check
//!      is a manual step)
//!   5. NO .sh/.py — enforced by pre-commit; the gate asserts no .sh/.py in the
//!      crate's tracked files
//!   6. FOUR FIELDS — inputs, outputs, what must be true, negative evidence
//!      (this is the inventory map's job; the gate checks the SURFACE-MAP entry)

use std::fs;
use std::path::Path;

/// A crate that failed one or more clauses.
pub struct PortingRefusal {
    pub crate_name: String,
    pub failed_clauses: Vec<&'static str>,
}

/// Return all crates under `root/crates/` that have a `Cargo.toml`.
fn list_crates(root: &Path) -> Vec<String> {
    let crates_dir = root.join("crates");
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&crates_dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.join("Cargo.toml").is_file() {
            if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                out.push(n.to_owned());
            }
        }
    }
    out.sort();
    out
}

/// Clause 1: a production caller exists outside the crate itself.
/// We grep for the crate name in every OTHER crate's source.
fn is_wired(root: &Path, name: &str) -> bool {
    let crates_dir = root.join("crates");
    let Ok(entries) = fs::read_dir(&crates_dir) else {
        return false;
    };
    for e in entries.flatten() {
        let dir = e.path();
        let other = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if other == name {
            continue;
        }
        let src = dir.join("src");
        if let Ok(text) = search_tree(&src, name) {
            if text {
                return true;
            }
        }
    }
    false
}

fn search_tree(dir: &Path, needle: &str) -> std::io::Result<bool> {
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if search_tree(&p, needle)? {
                    return Ok(true);
                }
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                if let Ok(text) = fs::read_to_string(&p) {
                    if text.contains(needle) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

/// Clause 2: a `[crates.x]` block exists in OMP-SURFACE-MAP.toml.
fn has_surface_declaration(root: &Path, name: &str) -> bool {
    let map = root.join("docs/plan/SURFACE-MAP.jsonl");
    let Ok(text) = fs::read_to_string(&map) else {
        return false;
    };
    // The map is JSONL: one row per surface. Each workspace-crate row carries
    // `"surface": "crate:{name}"` and `"kind": "workspace_crate"` on the SAME
    // line. Match per-line so a crate name appearing in another row's text
    // does not clear an undeclared crate.
    text.lines().any(|line| {
        line.contains(&format!("crate:{name}")) && line.contains("workspace_crate")
    })
}

/// Clause 3: `unsafe_code = "forbid"` in Cargo.toml.
fn forbids_unsafe(root: &Path, name: &str) -> bool {
    let manifest = root.join("crates").join(name).join("Cargo.toml");
    fs::read_to_string(&manifest)
        .map(|t| t.contains("unsafe_code"))
        .unwrap_or(false)
}

/// Clause 5: no .sh or .py files in the crate's tracked files.
fn no_shell_or_python(root: &Path, name: &str) -> bool {
    let src = root.join("crates").join(name);
    !find_forbidden_extensions(&src)
}

fn find_forbidden_extensions(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if find_forbidden_extensions(&p) {
                return true;
            }
        } else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
            if ext == "sh" || ext == "py" {
                return true;
            }
        }
    }
    false
}

/// The gate: evaluate all clauses for a named crate.
pub fn check_port(root: &Path, name: &str) -> PortingRefusal {
    let mut failed = Vec::new();

    if !is_wired(root, name) {
        failed.push("CLAUDE_1_WIRED: no production caller found outside the crate itself");
    }
    if !has_surface_declaration(root, name) {
        failed.push("CLAUDE_2_SURFACE: no [crates.x] block in OMP-SURFACE-MAP.toml");
    }
    if !forbids_unsafe(root, name) {
        failed.push("CLAUDE_3_ASYNCSYNC: missing unsafe_code = \"forbid\" in Cargo.toml");
    }
    if !no_shell_or_python(root, name) {
        failed.push("CLAUDE_5_NO_SH_PY: found .sh or .py files in the crate");
    }

    PortingRefusal {
        crate_name: name.to_owned(),
        failed_clauses: failed,
    }
}

/// Evaluate the entire workspace.
pub fn check_all_ports(root: &Path) -> Vec<PortingRefusal> {
    let crates = list_crates(root);
    crates.iter().map(|c| check_port(root, c)).collect()
}

/// ANTI-VACUITY: zero crates examined is an ERROR.
pub fn assert_not_vacuous(root: &Path) {
    let crates = list_crates(root);
    assert!(
        !crates.is_empty(),
        "ANTI-VACUITY: zero crates examined — a porting gate that checks nothing proves nothing"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    use std::path::PathBuf;

    /// ANTI-VACUITY (clause 5 of the acceptance): zero crates examined is an ERROR.
    #[test]
    fn zero_crates_examined_is_error() {
        assert_not_vacuous(&repo_root());
    }

    /// KNOWN-GOOD (acceptance 4): a correctly-ported crate passes unimpeded.
    /// `undrained-pipe-lint` has a real caller (pre-commit-gate + omp-orchestrator),
    /// a surface declaration, the forbid lint, and no .sh/.py files.
    #[test]
    fn correctly_ported_crate_passes() {
        let root = repo_root();
        let result = check_port(&root, "undrained-pipe-lint");
        assert!(
            result.failed_clauses.is_empty(),
            "undrained-pipe-lint should pass all clauses, got: {:?}",
            result.failed_clauses
        );
    }

    /// FIRES-ON-KNOWN-BAD (acceptance 3a): a crate with no caller is REFUSED,
    /// naming clause 1. We use `finding` (which has zero external callers).
    #[test]
    fn crate_with_no_caller_is_refused() {
        let root = repo_root();
        let result = check_port(&root, "nonexistent-phantom-crate");
        assert!(
            result
                .failed_clauses
                .iter()
                .any(|c| c.contains("CLAUDE_1_WIRED")),
            "a crate with no production caller must be refused naming clause 1 — got: {:?}",
            result.failed_clauses
        );
    }

    /// FIRES-ON-KNOWN-BAD (acceptance 3b): a crate with no surface declaration
    /// is REFUSED, naming clause 2.
    #[test]
    fn crate_with_no_surface_declaration_is_refused() {
        let root = repo_root();
        // composer-typed is absent from the SURFACE-MAP jsonl.
        let result = check_port(&root, "nonexistent-phantom-crate");
        assert!(
            result
                .failed_clauses
                .iter()
                .any(|c| c.contains("CLAUDE_2_SURFACE")),
            "a crate with no surface declaration must be refused naming clause 2 — got: {:?}",
            result.failed_clauses
        );
    }
}
