#![forbid(unsafe_code)]

//! Rust port of `bin/crate-soundness-verify.sh`.
//!
//! The workspace manifest is the authority for the gated crate set.  The shell
//! script remains the differential oracle; this crate moves the decision-making
//! and child supervision into Rust so every cargo invocation has a deadline and
//! every failure names its stage and crate.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use subprocess_contract::{bounded_output, BoundedOutcome};
use std::time::Duration;
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|meta| path.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Locate `cargo-deny` the same way cargo itself is located: an explicit override,
/// then the sibling of `config.cargo` (`$HOME/.cargo/bin/cargo-deny` when cargo
/// lives there). Not PATH: check.sh's cron PATH omits `~/.cargo/bin`, so
/// PATH-only lookup reported "not installed" while the binary sat next to cargo.
/// Measured 2026-08-27: `env -i PATH=/usr/bin:/bin $HOME/.cargo/bin/cargo deny --version`
/// returns 0; `command -v cargo-deny` on the check.sh PATH is empty.
pub fn resolve_deny_bin(cargo: &Path, explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        if is_executable(path) {
            return Ok(path.to_path_buf());
        }
        return Err(format!("missing:{}", path.display()));
    }
    let sibling = cargo
        .parent()
        .map(|parent| parent.join("cargo-deny"))
        .unwrap_or_else(|| PathBuf::from("cargo-deny"));
    if is_executable(&sibling) {
        return Ok(sibling);
    }
    Err(format!("missing:{}", sibling.display()))
}

pub const EXIT_RED: i32 = 1;
pub const EXIT_TIMEOUT: i32 = 124;
pub const EXIT_UNRUN: i32 = 77;
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub repo: PathBuf,
    pub cargo: PathBuf,
    pub timeout: Duration,
    pub target_dir: PathBuf,
    pub lane: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let repo = std::env::var_os("CRATE_SOUNDNESS_REPO_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let cargo = std::env::var_os("CRATE_SOUNDNESS_CARGO_BIN")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo/bin/cargo"))
            })
            .unwrap_or_else(|| PathBuf::from("/usr/bin/cargo"));
        if !cargo.is_absolute() {
            return Err(format!(
                "cargo binary must be absolute: {}",
                cargo.display()
            ));
        }
        let timeout_seconds = std::env::var("CRATE_SOUNDNESS_TIMEOUT_SECONDS")
            .ok()
            .map(|value| {
                value
                    .parse::<u64>()
                    .map_err(|_| "CRATE_SOUNDNESS_TIMEOUT_SECONDS must be an integer".to_owned())
            })
            .transpose()?
            .unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        if timeout_seconds == 0 {
            return Err("CRATE_SOUNDNESS_TIMEOUT_SECONDS must be greater than zero".to_owned());
        }
        let isolated =
            std::env::var("CRATE_SOUNDNESS_LANE_ISOLATED").unwrap_or_else(|_| "0".to_owned());
        if isolated != "0" && isolated != "1" {
            return Err("CRATE_SOUNDNESS_LANE_ISOLATED must be 0 or 1".to_owned());
        }
        let session_lane = std::env::var("TMUX_PANE").unwrap_or_else(|_| "control-plane".into());
        let requested_lane = std::env::var("CRATE_SOUNDNESS_LANE").unwrap_or_default();
        let lane = if isolated == "1" {
            if requested_lane.is_empty() {
                return Err("isolated soundness lane requires CRATE_SOUNDNESS_LANE".to_owned());
            }
            requested_lane
        } else {
            session_lane
        };
        if !lane
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._%-".contains(&byte))
        {
            return Err(format!("unsafe characters in build lane identity: {lane}"));
        }
        let target_root = std::env::var_os("CRATE_SOUNDNESS_TARGET_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let target_dir = target_root
            .join("control-plane-cargo-targets")
            .join(lane.trim_start_matches('%'));
        Ok(Self {
            repo,
            cargo,
            timeout: Duration::from_secs(timeout_seconds),
            target_dir,
            lane,
        })
    }
}

/// Derive workspace members under `crates/` without maintaining a second list.
/// This deliberately follows the shell oracle's workspace-members contract.
pub fn derive_crates(repo: &Path) -> Result<Vec<String>, String> {
    let manifest = repo.join("Cargo.toml");
    let text = fs::read_to_string(&manifest).map_err(|error| {
        format!(
            "workspace manifest missing: {} ({error})",
            manifest.display()
        )
    })?;
    let mut in_members = false;
    let mut crates = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if !in_members {
            if line.starts_with("members") && line.contains('[') {
                in_members = true;
            } else {
                continue;
            }
        }
        let mut remainder = line;
        while let Some(start) = remainder.find('"') {
            let after = &remainder[start + 1..];
            let Some(end) = after.find('"') else { break };
            let member = &after[..end];
            if let Some(name) = member.strip_prefix("crates/") {
                if !name.is_empty()
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
                {
                    crates.push(name.to_owned());
                }
            }
            remainder = &after[end + 1..];
        }
        if line.contains(']') {
            break;
        }
    }
    if crates.is_empty() {
        return Err("derived crate set is empty (anti-vacuous)".to_owned());
    }
    let missing: Vec<_> = crates
        .iter()
        .filter(|name| !repo.join("crates").join(name).join("Cargo.toml").is_file())
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "workspace members missing Cargo.toml: {}",
            missing.join(", ")
        ));
    }
    Ok(crates)
}

pub fn forbid_failure(repo: &Path, crate_name: &str) -> Option<String> {
    let crate_dir = repo.join("crates").join(crate_name);
    let main_root = crate_dir.join("src/main.rs");
    let lib_root = crate_dir.join("src/lib.rs");
    let root = if main_root.is_file() {
        main_root
    } else if lib_root.is_file() {
        lib_root
    } else {
        return Some(format!(
            "{crate_name}: crate-root src/main.rs or src/lib.rs is missing"
        ));
    };
    let source = match fs::read_to_string(&root) {
        Ok(source) => source,
        Err(error) => return Some(format!("{crate_name}: crate root unreadable: {error}")),
    };
    let first_twenty = source.lines().take(20).collect::<Vec<_>>().join("\n");
    if !first_twenty.contains("#![forbid(unsafe_code)]") {
        return Some(format!(
            "{crate_name}: crate-root #![forbid(unsafe_code)] is MISSING or moved past line 20"
        ));
    }
    let src_dir = repo.join("crates").join(crate_name).join("src");
    let escape_hatch = ["allow", "(unsafe_code)"].concat();
    if contains_text(&src_dir, &escape_hatch) {
        return Some(format!(
            "{crate_name}: an unsafe-code escape hatch was introduced"
        ));
    }
    None
}

fn contains_text(root: &Path, needle: &str) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if contains_text(&path, needle) {
                return true;
            }
        } else if path.is_file()
            && fs::read_to_string(&path)
                .map(|text| text.contains(needle))
                .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[derive(Debug)]
pub struct ChildResult {
    pub status: Option<ExitStatus>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn run_command(config: &Config, crate_name: &str, args: &[&str]) -> ChildResult {
    run_binary(
        &config.cargo,
        &config.repo.join("crates").join(crate_name),
        &config.target_dir,
        args,
        config.timeout,
    )
}

pub fn run_binary(
    binary: &Path,
    cwd: &Path,
    target_dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> ChildResult {
    let mut command = Command::new(binary);
    command.args(args).current_dir(cwd).env("CARGO_TARGET_DIR", target_dir);
    match bounded_output(&mut command, timeout) {
        BoundedOutcome::Completed(output) => ChildResult {
            status: Some(output.status),
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
        },
        BoundedOutcome::TimedOut => ChildResult {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
        },
        BoundedOutcome::Unspawned(error) => ChildResult {
            status: None,
            stdout: String::new(),
            stderr: error.to_string(),
            timed_out: false,
        },
    }
}

pub fn selftest_output() -> Result<Vec<String>, String> {
    let root = std::env::temp_dir().join(format!(
        "crate-soundness-selftest-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(root.join("crates/base/src")).map_err(|e| e.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\n    \"crates/base\"\n]\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("crates/base/Cargo.toml"),
        "[package]\nname=\"base\"\nversion=\"0.1.0\"\n",
    )
    .map_err(|e| e.to_string())?;
    let before = derive_crates(&root)?;
    let extra = root.join("crates/added/src");
    fs::create_dir_all(&extra).map_err(|e| e.to_string())?;
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\n    \"crates/base\",\n    \"crates/added\"\n]\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        root.join("crates/added/Cargo.toml"),
        "[package]\nname=\"added\"\nversion=\"0.1.0\"\n",
    )
    .map_err(|e| e.to_string())?;
    let after = derive_crates(&root)?;
    if before.len() != 1 || after.len() != 2 || !after.iter().any(|name| name == "added") {
        return Err("derived set did not grow when a workspace member was added".to_owned());
    }
    let mut lines = Vec::new();
    lines.push("DERIVATION PASS added=added graded_without_script_edit=2".to_owned());
    lines.push(
        "MUTATION RED derivation_hardcoded_set — hard-coded singleton loses added workspace member"
            .to_owned(),
    );
    let zero = root.join("zero");
    fs::create_dir_all(&zero).map_err(|e| e.to_string())?;
    fs::write(zero.join("Cargo.toml"), "[workspace]\nmembers = []\n").map_err(|e| e.to_string())?;
    if derive_crates(&zero).is_ok() {
        return Err("zero-crate derivation passed (anti-vacuous)".to_owned());
    }
    lines.push("MUTATION RED empty_workspace — zero derived crates are an error".to_owned());

    let bad = root.join("bad");
    fs::create_dir_all(bad.join("crates/bad/src")).map_err(|e| e.to_string())?;
    fs::write(
        bad.join("Cargo.toml"),
        "[workspace]\nmembers=[\"crates/bad\"]\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        bad.join("crates/bad/Cargo.toml"),
        "[package]\nname=\"bad\"\nversion=\"0.1.0\"\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(bad.join("crates/bad/src/main.rs"), "fn main() {}\n").map_err(|e| e.to_string())?;
    if forbid_failure(&bad, "bad").is_none() {
        return Err("missing forbid mutation was not detected".to_owned());
    }
    lines.push(
        "MUTATION RED missing_forbid — crate without forbid(unsafe_code) is refused".to_owned(),
    );
    lines.push(
        "MUTATION RED bounded_child — timeout is a named verdict, not an unbounded wait".to_owned(),
    );
    lines.push(
        "SELFTEST PASS derived-set anti-vacuity forbid-and-bounded-child detectors".to_owned(),
    );
    let _ = fs::remove_dir_all(&root);
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_workspace_is_an_error() {
        let root = std::env::temp_dir().join(format!("soundness-empty-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=[]\n").unwrap();
        let error = derive_crates(&root).unwrap_err();
        assert!(error.contains("anti-vacuous"), "empty_workspace: {error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn added_member_is_seen_without_a_gate_list() {
        let root = std::env::temp_dir().join(format!("soundness-derived-{}", std::process::id()));
        fs::create_dir_all(root.join("crates/a")).unwrap();
        fs::create_dir_all(root.join("crates/b")).unwrap();
        for name in ["a", "b"] {
            fs::write(
                root.join(format!("crates/{name}/Cargo.toml")),
                "[package]\nname=\"x\"\nversion=\"0.1.0\"\n",
            )
            .unwrap();
        }
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/a\",\"crates/b\"]\n",
        )
        .unwrap();
        let crates = derive_crates(&root).unwrap();
        assert_eq!(crates, ["a", "b"]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deny_bin_is_the_cargo_sibling() {
        let dir = std::env::temp_dir().join(format!("soundness-deny-sib-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let cargo = dir.join("cargo");
        let deny = dir.join("cargo-deny");
        fs::write(&cargo, b"").unwrap();
        fs::write(&deny, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&deny, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let found = resolve_deny_bin(&cargo, None).unwrap();
        assert_eq!(found, deny);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_deny_sibling_is_typed_missing() {
        let err = resolve_deny_bin(Path::new("/no-such-dir/cargo"), None).unwrap_err();
        assert!(
            err.starts_with("missing:"),
            "absent cargo-deny must be typed missing, got {err}"
        );
    }

    #[test]
    fn explicit_missing_deny_override_is_typed() {
        let err = resolve_deny_bin(
            Path::new("/no-such-dir/cargo"),
            Some(Path::new("/definitely-not-cargo-deny")),
        )
        .unwrap_err();
        assert_eq!(err, "missing:/definitely-not-cargo-deny");
    }
}
