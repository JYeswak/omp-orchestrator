//! Mechanical environment-surface parity for the three shell lanes and the composite Python lane.
//!
//! The historical files are intentionally read from the port commit's parent because the current
//! tree deleted them. Every git invocation clears its environment first; ambient PATH, LANG, and
//! TMUX_TMPDIR cannot make this check appear healthy.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const PORT_COMMIT: &str = "45c613d^";

struct Contract {
    historical: &'static str,
    replacement: &'static [&'static str],
}

const CONTRACTS: &[Contract] = &[
    Contract {
        historical: "bin/fleet-composite.py",
        replacement: &[
            "crates/fleet-composite/src/lib.rs",
            "crates/fleet-composite/src/main.rs",
        ],
    },
    Contract {
        historical: "bin/omp-idle-dispatch.sh",
        replacement: &[
            "crates/omp-idle-dispatch/src/lib.rs",
            "crates/omp-idle-dispatch/src/main.rs",
        ],
    },
    Contract {
        historical: "bin/omp-idle-dispatch-selftest.sh",
        replacement: &["crates/omp-idle-dispatch/tests/environment.rs"],
    },
    Contract {
        historical: "bin/wired-but-inert-guard.sh",
        replacement: &[
            "crates/wired-but-inert-guard/src/lib.rs",
            "crates/wired-but-inert-guard/src/main.rs",
        ],
    },
];

#[derive(Debug, Default, PartialEq, Eq)]
struct ShellSurface {
    exports: BTreeSet<String>,
    inputs: BTreeSet<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct RustSurface {
    reads: BTreeSet<String>,
    sets: BTreeSet<String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live below the repository root")
        .to_path_buf()
}

fn env_name(candidate: &str) -> Option<String> {
    let end = candidate
        .find(|character: char| {
            !(character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_')
        })
        .unwrap_or(candidate.len());
    let name = &candidate[..end];
    if name.is_empty() {
        return None;
    }
    Some(name.to_owned())
}

fn is_external_name(name: &str) -> bool {
    name == "HOME"
        || name == "PATH"
        || name == "TMUX_TMPDIR"
        || name == "LC_ALL"
        || name.starts_with("FLEET_")
        || name.starts_with("OMP_DISPATCH_")
        || name.starts_with("WIRED_GUARD_")
}

fn historical_source(repo: &Path, path: &str) -> String {
    let revision = format!("{PORT_COMMIT}:{path}");
    let output = Command::new("/usr/bin/git")
        .current_dir(repo)
        .env_clear()
        .env("HOME", "/tmp")
        .env("PATH", "/usr/bin:/bin")
        .args(["show", revision.as_str()])
        .output()
        .unwrap_or_else(|error| panic!("spawn git show {revision}: {error}"));
    assert!(
        output.status.success(),
        "git show {revision} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("historical source is UTF-8")
}

fn shell_surface(source: &str) -> ShellSurface {
    let mut assigned = BTreeSet::new();
    let mut surface = ShellSurface::default();

    for line in source.lines() {
        let trimmed = line.trim_start();
        let assignment = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        if let Some((left, _)) = assignment.split_once('=') {
            if let Some(name) = env_name(left.trim()) {
                assigned.insert(name.clone());
                if trimmed.starts_with("export ") && is_external_name(&name) {
                    surface.exports.insert(name);
                }
            }
        }

        let mut rest = line;
        while let Some(start) = rest.find("${") {
            let candidate = &rest[start + 2..];
            if let Some(name) = env_name(candidate) {
                if is_external_name(&name) && !assigned.contains(&name) {
                    surface.inputs.insert(name);
                }
            }
            rest = &candidate[2.min(candidate.len())..];
        }

        let mut rest = line;
        while let Some(start) = rest.find("os.getenv(\"") {
            let candidate = &rest[start + "os.getenv(\"".len()..];
            if let Some(end) = candidate.find('"') {
                let name = &candidate[..end];
                if is_external_name(name) {
                    surface.inputs.insert(name.to_owned());
                }
                rest = &candidate[end + 1..];
            } else {
                break;
            }
        }
    }

    surface
}

fn string_constants(source: &str) -> BTreeMap<String, String> {
    let mut constants = BTreeMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("pub const ")
            .or_else(|| trimmed.strip_prefix("const "))
        else {
            continue;
        };
        let Some((name, value)) = rest.split_once("&str =") else {
            continue;
        };
        let Some(start) = value.find('"') else {
            continue;
        };
        let Some(end) = value[start + 1..].find('"') else {
            continue;
        };
        constants.insert(
            name.trim().to_owned(),
            value[start + 1..start + 1 + end].to_owned(),
        );
    }
    constants
}

fn replacement_source(repo: &Path, paths: &[&str]) -> String {
    let mut source = String::new();
    for relative in paths {
        let path = repo.join(relative);
        assert!(path.is_file(), "replacement is absent: {}", path.display());
        source.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
        );
        source.push('\n');
    }
    source
}

fn rust_surface(source: &str) -> RustSurface {
    let constants = string_constants(source);
    let mut surface = RustSurface::default();

    for function in ["var_os(", "var(", "env_or(", "env_u64(", "set_var("] {
        let mut rest = source;
        while let Some(start) = rest.find(function) {
            let candidate = &rest[start + function.len()..];
            let argument = candidate.trim_start();
            let (name, consumed) = if let Some(value) = argument.strip_prefix('"') {
                let Some(end) = value.find('"') else {
                    break;
                };
                (value[..end].to_owned(), end + 2)
            } else {
                let end = argument
                    .find(|character: char| {
                        !(character.is_ascii_uppercase()
                            || character.is_ascii_digit()
                            || character == '_')
                    })
                    .unwrap_or(argument.len());
                let identifier = &argument[..end];
                let Some(value) = constants.get(identifier) else {
                    rest = &argument[end.min(argument.len())..];
                    continue;
                };
                (value.clone(), end + 1)
            };
            if is_external_name(&name) {
                if function == "set_var(" {
                    surface.sets.insert(name);
                } else {
                    surface.reads.insert(name);
                }
            }
            rest = &candidate[consumed.min(candidate.len())..];
        }
    }

    surface
}

#[test]
fn every_historical_environment_surface_is_covered_by_its_rust_replacement() {
    let repo = repo_root();
    let Some(source_repo) = std::env::var_os("CONTROL_PLANE_REPO").map(PathBuf::from) else {
        println!("DIFFERENTIAL DID NOT RUN: test=every_historical_environment_surface_is_covered_by_its_rust_replacement reason=source_repo_unconfigured detail=CONTROL_PLANE_REPO");
        return;
    };
    for contract in CONTRACTS {
        let historical = historical_source(&source_repo, contract.historical);
        let replacement = replacement_source(&repo, contract.replacement);
        let old = shell_surface(&historical);
        let new = rust_surface(&replacement);

        let missing_exports: BTreeSet<_> = old.exports.difference(&new.sets).cloned().collect();
        let mut covered = new.reads.clone();
        covered.extend(new.sets.iter().cloned());
        let missing_inputs: BTreeSet<_> = old.inputs.difference(&covered).cloned().collect();

        assert!(
            missing_exports.is_empty(),
            "{} -> {:?} dropped exported environment keys: {:?}; Rust sets {:?}",
            contract.historical,
            contract.replacement,
            missing_exports,
            new.sets
        );
        assert!(
            missing_inputs.is_empty(),
            "{} -> {:?} dropped environment inputs: {:?}; Rust reads {:?} and sets {:?}",
            contract.historical,
            contract.replacement,
            missing_inputs,
            new.reads,
            new.sets
        );
        println!(
            "ENV_PARITY PASS {} -> {:?} exports={:?} inputs={:?}",
            contract.historical, contract.replacement, old.exports, old.inputs
        );
    }
}
