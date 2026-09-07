#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const OWNER_FILE: &str = ".omp-cargo-target.owner";
const OWNER_SCHEMA: &str = "omp.cargo-target-owner/v1";
const OWNER_NAME: &str = "omp-orchestrator";

fn main() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");

    // DERIVE the build id rather than hoping an operator exports it.
    //
    // Measured 2026-09-01, and every link of the night's worst outage runs through this one
    // fail-open. `BUILD_ID` was `option_env!("OMP_BUILD_ID")` falling back to "unversioned",
    // and build.rs only WATCHED the variable - it never produced one. A build with the env
    // unset therefore shipped an anonymous binary and said nothing about it:
    //
    //   * `tick-monitor` carries no build-id mechanism at all. The installer's identity rule
    //     refuses a binary it cannot verify, so a routine install DELETED it and the resident
    //     supervisor refused every 30s for hours with "tick-monitor: process not found". The
    //     fleet sat idle because a binary could not say what it was built from.
    //   * `pane-truth` is the same class, recorded in AGENTS.md as a MISMATCH that "can never
    //     clear" - a fixed point that trains operators to ignore the gate entirely.
    //   * An install run tonight WITH the env set still produced `build_id=unversioned`,
    //     because cargo reused a cached artifact. The variable is not a reliable input even
    //     when someone remembers it.
    //
    // So fix the producer, not the consumers: ask git. An explicit OMP_BUILD_ID still wins,
    // so a release can stamp something deliberate, but ABSENCE now resolves to the commit
    // instead of to an anonymous string.
    //
    // NO-CLAIM: this makes the id PRESENT, not TRUE. A dirty tree stamps its HEAD sha and the
    // `-dirty` suffix is the only thing that says so; with git unavailable it yields
    // `nogit-<epoch>`, which is at least honest about being underived.
    let build_id = env::var("OMP_BUILD_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            let head = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty());
            match head {
                Some(sha) => {
                    let dirty = std::process::Command::new("git")
                        .args(["status", "--porcelain"])
                        .output()
                        .ok()
                        .map(|o| !o.stdout.is_empty())
                        .unwrap_or(false);
                    if dirty {
                        format!("{sha}-dirty")
                    } else {
                        sha
                    }
                }
                None => format!(
                    "nogit-{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                ),
            }
        });
    println!("cargo:rustc-env=OMP_BUILD_ID={build_id}");

    // WATCH the inputs the id is derived from, or the derivation never re-runs.
    //
    // Measured 2026-09-01: build.rs was changed to DERIVE `build_id` from git, and the
    // very next install still shipped `build_id=unversioned`. The comment above blames
    // "cargo reused a cached artifact" — but that is not bad luck, it is this omission.
    // build.rs emitted only `rerun-if-env-changed`, so cargo had no reason to re-run it
    // when HEAD moved, and a derived id that is never RE-derived is a cached id that
    // silently describes a different commit than the one being built.
    //
    // This is the identity half of the same failure as the claim fence: a mechanism was
    // correct in source and inert in the installed artifact. `~/.local/bin/omp-orchestrator`
    // still reported `build_id=unversioned` at 18:18 tonight for exactly this reason,
    // while its `--version` output was being trusted to prove what it was built from.
    //
    // So watch every derivation input: HEAD, whatever ref HEAD names, packed-refs (a
    // packed branch has no loose ref file), and the index (which carries the dirty bit).
    watch_git_inputs();
    println!("cargo:rerun-if-env-changed=CARGO_TARGET_DIR");
    println!("cargo:rerun-if-env-changed=FRANKEN_CARGO_TARGET_ROOT");
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");

    if let Err(error) = register_target_directory() {
        panic!("{error}");
    }
}

/// Emits `cargo:rerun-if-changed` for every input `build_id` is derived from.
///
/// The paths must be ABSOLUTE. A relative `rerun-if-changed` is resolved against the
/// package root (`crates/omp-orchestrator`), and the git directory is two levels above
/// it, so a relative form would silently watch a path that does not exist — which fails
/// the same open way this function exists to close.
fn watch_git_inputs() {
    let git_dir = std::process::Command::new("git")
        .args(["rev-parse", "--absolute-git-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_owned()))
        .filter(|p| p.is_dir());
    let git_dir = match git_dir {
        Some(dir) => dir,
        // With git unavailable the id already resolved to `nogit-<epoch>`, which is
        // different on every run, so there is nothing to invalidate.
        None => return,
    };

    for input in ["HEAD", "index", "packed-refs"] {
        let path = git_dir.join(input);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    // A loose branch ref: HEAD is a symref to it, and it is what actually moves on
    // commit. `.git/HEAD` itself only changes on checkout.
    let reference = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty());
    if let Some(reference) = reference {
        let path = git_dir.join(reference);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn register_target_directory() -> Result<(), String> {
    let repo_root = absolute_path(Path::new(env!("CARGO_MANIFEST_DIR")))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "CARGO_TARGET_POLICY_REFUSED code=repo-root-unavailable".to_owned())?;
    let out_dir = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| "CARGO_TARGET_POLICY_REFUSED code=out-dir-unavailable".to_owned())?;
    let target = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| resolve_path(&repo_root, &path))
        .or_else(|| infer_target_directory(&out_dir))
        .ok_or_else(|| "CARGO_TARGET_POLICY_REFUSED code=target-unavailable".to_owned())?;
    let target = absolute_path(&target);
    let registered_root = env::var_os("FRANKEN_CARGO_TARGET_ROOT")
        .map(PathBuf::from)
        .map(|path| absolute_path(&resolve_path(&repo_root, &path)));

    if !is_under(&target, &repo_root)
        && !registered_root
            .as_deref()
            .is_some_and(|root| is_under(&target, root))
    {
        return Err(format!(
            "CARGO_TARGET_POLICY_REFUSED code=unowned-target target={} repo={} registered_root={}",
            target.display(),
            repo_root.display(),
            registered_root
                .as_deref()
                .map_or_else(|| "none".to_owned(), |root| root.display().to_string())
        ));
    }

    let marker = target.join(OWNER_FILE);
    let record = format!(
        "schema={OWNER_SCHEMA}\nowner={OWNER_NAME}\ntarget={}\ntoolchain={}\npid={}\ncreated_epoch={}\n",
        target.display(),
        env::var("RUSTUP_TOOLCHAIN").unwrap_or_else(|_| env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned())),
        std::process::id(),
        now_epoch(),
    );
    fs::create_dir_all(&target).map_err(|error| {
        format!(
            "CARGO_TARGET_POLICY_REFUSED code=target-create-failed target={} reason={error}",
            target.display()
        )
    })?;
    match OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(mut file) => {
            file.write_all(record.as_bytes()).and_then(|_| file.sync_data()).map_err(|error| {
                format!(
                    "CARGO_TARGET_POLICY_REFUSED code=owner-record-write-failed target={} reason={error}",
                    target.display()
                )
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read_to_string(&marker).map_err(|read_error| {
                format!(
                    "CARGO_TARGET_POLICY_REFUSED code=owner-record-read-failed target={} reason={read_error}",
                    target.display()
                )
            })?;
            if !owner_record_matches(&existing, &target) {
                return Err(format!(
                    "CARGO_TARGET_POLICY_REFUSED code=owner-record-conflict target={}",
                    target.display()
                ));
            }
        }
        Err(error) => {
            return Err(format!(
                "CARGO_TARGET_POLICY_REFUSED code=owner-record-create-failed target={} reason={error}",
                target.display()
            ));
        }
    }
    Ok(())
}

fn owner_record_matches(record: &str, target: &Path) -> bool {
    let fields = record
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect::<std::collections::BTreeMap<_, _>>();
    fields.get("schema") == Some(&OWNER_SCHEMA)
        && fields.get("owner") == Some(&OWNER_NAME)
        && fields.get("target") == Some(&target.to_string_lossy().as_ref())
}

fn infer_target_directory(out_dir: &Path) -> Option<PathBuf> {
    out_dir.ancestors().find_map(|ancestor| {
        let name = ancestor.file_name()?.to_str()?;
        match name {
            "debug" | "release" => ancestor.parent().map(Path::to_path_buf),
            _ => None,
        }
    })
}

fn resolve_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize(path)
    } else {
        normalize(&env::current_dir().unwrap_or_else(|_| PathBuf::from("."))).join(path)
    }
}

fn is_under(path: &Path, root: &Path) -> bool {
    path == root || path.strip_prefix(root).is_ok()
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        PathBuf::new()
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            Component::Normal(value) => result.push(value),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    result
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
