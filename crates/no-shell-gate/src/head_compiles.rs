//! Clean-tree compilation gate.
//!
//! ENFORCES: `git archive HEAD` is extracted into a private directory and
//! `cargo check --workspace --all-targets` runs from that export, not from the
//! caller's worktree.
//! STILL PASSES: this raises the floor for committed-tree compilation only; it
//! does not certify tests, CI cache/network parity, or worktree cleanliness.
//! PROVENANCE: the export and build are performed by this module at invocation
//! time, and the receipt records the committed HEAD.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use subprocess_contract::{BoundedOutcome, bounded_output, bounded_output_stdin};

pub const ARCHIVE_DEADLINE: Duration = Duration::from_secs(30);
pub const EXTRACT_DEADLINE: Duration = Duration::from_secs(30);
pub const BUILD_DEADLINE: Duration = Duration::from_secs(1_800);
const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum GateError {
    GitHeadFailed(String),
    ArchiveFailed(String),
    ArchiveTimedOut,
    ArchiveEmpty,
    ExportCreateFailed { path: PathBuf, detail: String },
    ExportEmpty { path: PathBuf, detail: String },
    ExportExtractFailed(String),
    ExportExtractTimedOut,
    ExportArchiveTooLarge(usize),
    DependencyResolutionFailed(String),
    CompileFailed(String),
    BuildTimedOut,
    ReceiptFailed { path: PathBuf, detail: String },
}

impl GateError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::DependencyResolutionFailed(_) | Self::CompileFailed(_) | Self::BuildTimedOut => {
                101
            }
            _ => 2,
        }
    }
}

impl fmt::Display for GateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitHeadFailed(detail) => write!(formatter, "ERROR GIT_HEAD_FAILED {detail}"),
            Self::ArchiveFailed(detail) => write!(formatter, "ERROR HEAD_ARCHIVE_FAILED {detail}"),
            Self::ArchiveTimedOut => formatter.write_str(
                "ERROR HEAD_ARCHIVE_TIMEOUT git archive HEAD exceeded its bounded deadline",
            ),
            Self::ArchiveEmpty => formatter.write_str(
                "ERROR HEAD_ARCHIVE_EMPTY git archive HEAD produced zero bytes; no committed tree was measured",
            ),
            Self::ExportCreateFailed { path, detail } => {
                write!(formatter, "ERROR EXPORT_CREATE_FAILED path={} detail={detail}", path.display())
            }
            Self::ExportEmpty { path, detail } => {
                write!(formatter, "ERROR EXPORT_EMPTY path={} detail={detail}", path.display())
            }
            Self::ExportExtractFailed(detail) => {
                write!(formatter, "ERROR EXPORT_EXTRACT_FAILED {detail}")
            }
            Self::ExportExtractTimedOut => formatter.write_str(
                "ERROR EXPORT_EXTRACT_TIMEOUT tar extraction exceeded its bounded deadline",
            ),
            Self::ExportArchiveTooLarge(bytes) => write!(
                formatter,
                "ERROR HEAD_ARCHIVE_TOO_LARGE bytes={bytes} max_bytes={MAX_ARCHIVE_BYTES}"
            ),
            Self::DependencyResolutionFailed(detail) => {
                write!(formatter, "ERROR HEAD_COMPILE_DEPENDENCY_ERROR {detail}")
            }
            Self::CompileFailed(detail) => write!(formatter, "ERROR HEAD_COMPILE_FAILED {detail}"),
            Self::BuildTimedOut => formatter.write_str(
                "ERROR HEAD_COMPILE_TIMEOUT cargo check exceeded its bounded deadline",
            ),
            Self::ReceiptFailed { path, detail } => {
                write!(formatter, "ERROR RECEIPT_WRITE_FAILED path={} detail={detail}", path.display())
            }
        }
    }
}

impl std::error::Error for GateError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateConfig {
    pub repo: PathBuf,
    pub receipt: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateReceipt {
    pub text: String,
}

fn command_detail(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = if stderr.trim().is_empty() {
        stdout.into_owned()
    } else if stdout.trim().is_empty() {
        stderr.into_owned()
    } else {
        format!("stdout={stdout} stderr={stderr}")
    };
    detail.trim().to_owned()
}

fn current_head(repo: &Path) -> Result<String, GateError> {
    let mut command = Command::new("git");
    command.args(["rev-parse", "HEAD"]).current_dir(repo);
    match bounded_output(&mut command, ARCHIVE_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        }
        BoundedOutcome::Completed(output) => Err(GateError::GitHeadFailed(command_detail(&output))),
        BoundedOutcome::TimedOut => Err(GateError::GitHeadFailed(
            "rev-parse HEAD exceeded its bounded deadline".to_owned(),
        )),
        BoundedOutcome::Unspawned(error) => Err(GateError::GitHeadFailed(error.to_string())),
    }
}

fn archive_head(repo: &Path) -> Result<Vec<u8>, GateError> {
    let mut command = Command::new("git");
    command.args(["archive", "HEAD"]).current_dir(repo);
    match bounded_output(&mut command, ARCHIVE_DEADLINE) {
        BoundedOutcome::Completed(output) if !output.status.success() => {
            Err(GateError::ArchiveFailed(command_detail(&output)))
        }
        BoundedOutcome::Completed(output) if output.stdout.is_empty() => {
            Err(GateError::ArchiveEmpty)
        }
        BoundedOutcome::Completed(output) if output.stdout.len() > MAX_ARCHIVE_BYTES => {
            Err(GateError::ExportArchiveTooLarge(output.stdout.len()))
        }
        BoundedOutcome::Completed(output) => Ok(output.stdout),
        BoundedOutcome::TimedOut => Err(GateError::ArchiveTimedOut),
        BoundedOutcome::Unspawned(error) => Err(GateError::ArchiveFailed(error.to_string())),
    }
}

fn export_directory() -> Result<ExportDirectory, GateError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("head-compiles-gate-{}-{now}", std::process::id()));
    fs::create_dir(&path).map_err(|error| GateError::ExportCreateFailed {
        path: path.clone(),
        detail: error.to_string(),
    })?;
    Ok(ExportDirectory { path })
}

#[derive(Debug)]
struct ExportDirectory {
    path: PathBuf,
}

impl Drop for ExportDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn validate_export_root(path: &Path) -> Result<(), GateError> {
    if !path.is_dir() {
        return Err(GateError::ExportEmpty {
            path: path.to_owned(),
            detail: "export directory is absent".to_owned(),
        });
    }
    if !path.join("Cargo.toml").is_file() {
        return Err(GateError::ExportEmpty {
            path: path.to_owned(),
            detail: "Cargo.toml is absent".to_owned(),
        });
    }
    let crates = path.join("crates");
    let entries = fs::read_dir(&crates).map_err(|error| GateError::ExportEmpty {
        path: path.to_owned(),
        detail: format!("crates directory unreadable: {error}"),
    })?;
    let member_count = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .count();
    if member_count == 0 {
        return Err(GateError::ExportEmpty {
            path: path.to_owned(),
            detail: "crates directory contains no Cargo.toml members".to_owned(),
        });
    }
    Ok(())
}

fn extract_archive(export: &Path, archive: &[u8]) -> Result<(), GateError> {
    let mut command = Command::new("tar");
    command.args(["-x", "-C", &export.display().to_string()]);
    match bounded_output_stdin(&mut command, EXTRACT_DEADLINE, archive) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(()),
        BoundedOutcome::Completed(output) => {
            Err(GateError::ExportExtractFailed(command_detail(&output)))
        }
        BoundedOutcome::TimedOut => Err(GateError::ExportExtractTimedOut),
        BoundedOutcome::Unspawned(error) => Err(GateError::ExportExtractFailed(error.to_string())),
    }
}

fn dependency_failure(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    [
        "failed to select a version",
        "failed to get",
        "failed to resolve",
        "no matching package",
        "unable to update",
        "could not resolve",
        "lock file",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn diagnostic_excerpt(status: Option<i32>, combined: &str) -> String {
    let start = [
        "error[E",
        "error:",
        "failed to select a version",
        "failed to get",
        "failed to resolve",
        "no matching package",
        "unable to update",
        "could not resolve",
        "lock file",
    ]
    .iter()
    .find_map(|needle| combined.find(needle))
    .unwrap_or(0);
    let excerpt: String = combined[start..].chars().take(12_000).collect();
    format!(
        "status={} diagnostic={}",
        status.unwrap_or(1),
        excerpt.trim()
    )
}

pub fn classify_build_failure(status: Option<i32>, stdout: &str, stderr: &str) -> GateError {
    let combined = format!("stdout={stdout} stderr={stderr}");
    let detail = diagnostic_excerpt(status, &combined);
    if dependency_failure(&combined) {
        GateError::DependencyResolutionFailed(detail)
    } else {
        GateError::CompileFailed(detail)
    }
}
fn build_export(export: &Path) -> Result<(), GateError> {
    let cargo_program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = export.join("Cargo.toml");
    let target = export.join(".head-compiles-target");
    let mut command = Command::new(cargo_program);
    command
        .args([
            "check",
            "--workspace",
            "--all-targets",
            "--manifest-path",
            &manifest.display().to_string(),
        ])
        .current_dir(export)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_TERM_COLOR", "never");
    match bounded_output(&mut command, BUILD_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(()),
        BoundedOutcome::Completed(output) => Err(classify_build_failure(
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
        )),
        BoundedOutcome::TimedOut => Err(GateError::BuildTimedOut),
        BoundedOutcome::Unspawned(error) => Err(GateError::DependencyResolutionFailed(format!(
            "cargo spawn_error={error}"
        ))),
    }
}

fn write_receipt(path: &Path, text: &str) -> Result<(), GateError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| GateError::ReceiptFailed {
            path: path.to_owned(),
            detail: error.to_string(),
        })?;
    }
    fs::write(path, format!("{text}\n")).map_err(|error| GateError::ReceiptFailed {
        path: path.to_owned(),
        detail: error.to_string(),
    })
}

pub fn run(config: &GateConfig) -> Result<GateReceipt, GateError> {
    let head = current_head(&config.repo)?;
    let archive = archive_head(&config.repo)?;
    let export = export_directory()?;
    extract_archive(&export.path, &archive)?;
    validate_export_root(&export.path)?;
    build_export(&export.path)?;
    let receipt_path = config
        .receipt
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "stdout-only".to_owned());
    let text = format!(
        "HEAD_COMPILES_GATE PASS head={head} source=git-archive-HEAD-piped-to-tar export=private build=cargo-check-workspace-all-targets receipt={receipt_path}\nCLAIM: HEAD compiles from a clean archive export.\nDOES_NOT_CLAIM: HEAD tests pass; CI cache or dependency-resolution parity; worktree cleanliness.\nTRIGGER: .github/workflows/gate.yml job=head-compiles-as-committed on push, pull_request, or workflow_dispatch.\nCONSUMER: .github/workflows/gate.yml step=Read committed-tree verdict.\nLIMIT: .git/hooks are per-clone; this CI job is the repository-wide trigger."
    );
    if let Some(path) = &config.receipt {
        write_receipt(path, &text)?;
    }
    Ok(GateReceipt { text })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "head-compiles-gate-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("crates/member")).expect("fixture root");
        fs::write(
            path.join("Cargo.toml"),
            "[workspace]\nmembers=[\"crates/*\"]\n",
        )
        .expect("fixture manifest");
        fs::write(
            path.join("crates/member/Cargo.toml"),
            "[package]\nname=\"member\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
        )
        .expect("fixture member manifest");
        path
    }

    #[test]
    fn clean_export_is_nonempty() {
        let root = temp_root("clean");
        validate_export_root(&root).expect("clean export must be accepted");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn archive_failure_is_named_error() {
        let root =
            std::env::temp_dir().join(format!("head-compiles-gate-no-repo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("non-repository fixture");
        let error = archive_head(&root).expect_err("non-repository must refuse archive");
        assert!(error.to_string().contains("ERROR HEAD_ARCHIVE_FAILED"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn empty_export_is_a_named_error() {
        let root =
            std::env::temp_dir().join(format!("head-compiles-gate-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("empty export");
        let error = validate_export_root(&root).expect_err("empty export must refuse");
        assert!(error.to_string().contains("ERROR EXPORT_EMPTY"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn compiler_error_message_is_preserved() {
        let error = classify_build_failure(
            Some(101),
            "",
            "error[E0432]: unresolved import `missing_crate`",
        );
        assert!(error.to_string().contains("E0432"));
        assert!(error.to_string().contains("HEAD_COMPILE_FAILED"));
    }

    #[test]
    fn dependency_resolution_error_is_typed() {
        let error = classify_build_failure(Some(101), "", "failed to select a version for `x`");
        assert!(error.to_string().contains("HEAD_COMPILE_DEPENDENCY_ERROR"));
        assert_eq!(error.exit_code(), 101);
    }

    #[test]
    fn receipt_states_floor_raise_and_limits() {
        let receipt = "HEAD_COMPILES_GATE PASS head=abc source=git-archive-HEAD-piped-to-tar\nCLAIM: HEAD compiles from a clean archive export.\nDOES_NOT_CLAIM: HEAD tests pass; CI cache or dependency-resolution parity; worktree cleanliness.\nTRIGGER: .github/workflows/gate.yml job=head-compiles-as-committed\nCONSUMER: .github/workflows/gate.yml step=Read committed-tree verdict\nLIMIT: .git/hooks are per-clone";
        assert!(receipt.contains("CLAIM: HEAD compiles"));
        assert!(receipt.contains("DOES_NOT_CLAIM"));
        assert!(receipt.contains("TRIGGER:"));
        assert!(receipt.contains("CONSUMER:"));
        assert!(receipt.contains("LIMIT: .git/hooks are per-clone"));
    }
}
