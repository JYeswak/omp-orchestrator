#![forbid(unsafe_code)]

//! Profile-scoped OMP pane-to-session resolution.
//!
//! The terminal-session index is an input to pane observation, not proof of liveness.  A
//! profiled pane is resolved through its own `omp --profile` argv, both historical roots are
//! probed, and only a session path under the selected profile's `agent/sessions` root is handed
//! to `ompo_doctor::omp_state::read_state`.  Unprofiled or foreign panes remain explicit routes;
//! they must not be mistaken for a missing OMP installation.

use asupersync::process::Command;
use asupersync::Cx;
use ompo_doctor::omp_state::{read_state, StateOutcome};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use subprocess_contract::{run_output, RunError};

const OMP_DIR: &str = ".omp";
const AGENT_DIR: &str = "agent";
const SESSIONS_DIR: &str = "sessions";
const TERMINAL_SESSIONS_DIR: &str = "terminal-sessions";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileSelection {
    Profiled(String),
    Unprofiled,
}

impl ProfileSelection {
    #[must_use]
    pub fn profile(&self) -> Option<&str> {
        match self {
            Self::Profiled(profile) => Some(profile),
            Self::Unprofiled => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileSelectionError {
    MissingValue,
    EmptyValue,
    UnsafeValue(String),
}

impl fmt::Display for ProfileSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue => formatter.write_str("PANE_ORACLE_PROFILE_MISSING_VALUE"),
            Self::EmptyValue => formatter.write_str("PANE_ORACLE_PROFILE_EMPTY_VALUE"),
            Self::UnsafeValue(value) => {
                write!(
                    formatter,
                    "PANE_ORACLE_PROFILE_UNSAFE_VALUE value={value:?}"
                )
            }
        }
    }
}

impl std::error::Error for ProfileSelectionError {}

/// Parse the child command printed by `ps -p <child> -o command=`.
///
/// The exact `--profile <name>` form is the contract.  `--profile=<name>` is accepted because
/// the command-line parser emits the same semantic value; an empty or missing value refuses
/// rather than silently selecting the unprofiled root.
#[must_use]
pub fn profile_from_command(command: &str) -> Result<ProfileSelection, ProfileSelectionError> {
    let args: Vec<&str> = command.split_whitespace().collect();
    for (index, arg) in args.iter().enumerate() {
        if *arg == "--profile" {
            let Some(value) = args.get(index + 1) else {
                return Err(ProfileSelectionError::MissingValue);
            };
            if value.starts_with('-') {
                return Err(ProfileSelectionError::MissingValue);
            }
            return validate_profile(value);
        }
        if let Some(value) = arg.strip_prefix("--profile=") {
            if value.is_empty() {
                return Err(ProfileSelectionError::EmptyValue);
            }
            return validate_profile(value);
        }
    }
    Ok(ProfileSelection::Unprofiled)
}

fn validate_profile(value: &str) -> Result<ProfileSelection, ProfileSelectionError> {
    if value.is_empty() {
        return Err(ProfileSelectionError::EmptyValue);
    }
    if value == "."
        || value == ".."
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(char::is_whitespace)
    {
        return Err(ProfileSelectionError::UnsafeValue(value.to_owned()));
    }
    Ok(ProfileSelection::Profiled(value.to_owned()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryShape {
    TwoLines,
    ThreeLines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateToken {
    Fresh,
    Missing,
    Unknown(String),
}

impl StateToken {
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self {
            Self::Fresh => "Fresh",
            Self::Missing => "Unknown/MissingStateToken",
            Self::Unknown(_) => "Unknown/UnrecognizedStateToken",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSessionEntry {
    pub cwd: PathBuf,
    pub session_path: PathBuf,
    pub state_token: StateToken,
    pub shape: EntryShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootProbe {
    Missing,
    EntryPresent,
    NonEmptyWithoutEntry,
    Empty,
    Unreadable(String),
}

impl RootProbe {
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self {
            Self::Missing => "ABSENT",
            Self::EntryPresent => "PRESENT",
            Self::NonEmptyWithoutEntry => "ABSENT_NONEMPTY_ROOT",
            Self::Empty => "EMPTY_STORE",
            Self::Unreadable(_) => "UNKNOWN_UNREADABLE",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootObservations {
    pub unprofiled: RootProbe,
    pub profiled: Option<RootProbe>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedSessionEntry {
    pub profile: ProfileSelection,
    pub terminal_entry_path: PathBuf,
    pub sessions_root: PathBuf,
    pub entry: TerminalSessionEntry,
    pub roots: RootObservations,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileStoreError {
    InvalidPane(String),
    EmptyStore {
        root: PathBuf,
    },
    Absent {
        expected: PathBuf,
        roots: RootObservations,
    },
    Unreadable {
        path: PathBuf,
        detail: String,
    },
    Malformed {
        path: PathBuf,
        line_count: usize,
    },
    ForeignSessionPath {
        observed: PathBuf,
        expected_root: PathBuf,
    },
}

impl fmt::Display for ProfileStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPane(pane) => write!(formatter, "PANE_ORACLE_INVALID_PANE pane={pane:?}"),
            Self::EmptyStore { root } => {
                write!(formatter, "PANE_ORACLE_EMPTY_STORE root={}", root.display())
            }
            Self::Absent { expected, roots } => write!(
                formatter,
                "PANE_ORACLE_ABSENT expected={} unprofiled={} profiled={}",
                expected.display(),
                roots.unprofiled.status(),
                roots
                    .profiled
                    .as_ref()
                    .map_or("NOT_SELECTED", RootProbe::status)
            ),
            Self::Unreadable { path, detail } => write!(
                formatter,
                "PANE_ORACLE_UNKNOWN_UNREADABLE path={} detail={detail}",
                path.display()
            ),
            Self::Malformed { path, line_count } => write!(
                formatter,
                "PANE_ORACLE_MALFORMED_ENTRY path={} line_count={line_count}",
                path.display()
            ),
            Self::ForeignSessionPath {
                observed,
                expected_root,
            } => write!(
                formatter,
                "PANE_ORACLE_UNKNOWN_FOREIGN_SESSION_PATH observed={} expected_root={}",
                observed.display(),
                expected_root.display()
            ),
        }
    }
}

impl std::error::Error for ProfileStoreError {}

#[derive(Debug)]
pub enum PaneOracleError {
    Command { program: String, detail: String },
    Profile(ProfileSelectionError),
    Store(ProfileStoreError),
    Runtime(String),
}

impl fmt::Display for PaneOracleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { program, detail } => {
                write!(
                    formatter,
                    "PANE_ORACLE_COMMAND_FAILED program={program} detail={detail}"
                )
            }
            Self::Profile(error) => error.fmt(formatter),
            Self::Store(error) => error.fmt(formatter),
            Self::Runtime(detail) => {
                write!(formatter, "PANE_ORACLE_RUNTIME_FAILED detail={detail}")
            }
        }
    }
}

impl PaneOracleError {
    /// Discovery failures are unmeasured, never subject-state failures.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        4
    }
}
impl std::error::Error for PaneOracleError {}

impl From<ProfileSelectionError> for PaneOracleError {
    fn from(error: ProfileSelectionError) -> Self {
        Self::Profile(error)
    }
}

impl From<ProfileStoreError> for PaneOracleError {
    fn from(error: ProfileStoreError) -> Self {
        Self::Store(error)
    }
}

/// Parse the observed two-line or three-line terminal-session entry.
#[must_use]
pub fn parse_entry(text: &str) -> Result<TerminalSessionEntry, ProfileStoreError> {
    let without_one_terminal_newline = text.strip_suffix('\n').unwrap_or(text);
    let lines: Vec<&str> = without_one_terminal_newline.split('\n').collect();
    if !matches!(lines.len(), 2 | 3) {
        return Err(ProfileStoreError::Malformed {
            path: PathBuf::from("<memory>"),
            line_count: lines.len(),
        });
    }
    let cwd = lines[0].trim().trim_end_matches('\r');
    let session_path = lines[1].trim().trim_end_matches('\r');
    if cwd.is_empty() || session_path.is_empty() {
        return Err(ProfileStoreError::Malformed {
            path: PathBuf::from("<memory>"),
            line_count: lines.len(),
        });
    }
    let state_token = match lines.get(2).map(|line| line.trim().trim_end_matches('\r')) {
        None => StateToken::Missing,
        Some("fresh") => StateToken::Fresh,
        Some(value) if value.is_empty() => {
            return Err(ProfileStoreError::Malformed {
                path: PathBuf::from("<memory>"),
                line_count: lines.len(),
            });
        }
        Some(value) => StateToken::Unknown(value.to_owned()),
    };
    Ok(TerminalSessionEntry {
        cwd: PathBuf::from(cwd),
        session_path: PathBuf::from(session_path),
        state_token,
        shape: if lines.len() == 2 {
            EntryShape::TwoLines
        } else {
            EntryShape::ThreeLines
        },
    })
}

fn pane_suffix(pane_id: &str) -> Result<&str, ProfileStoreError> {
    let Some(number) = pane_id.strip_prefix('%') else {
        return Err(ProfileStoreError::InvalidPane(pane_id.to_owned()));
    };
    if number.is_empty() || !number.chars().all(|character| character.is_ascii_digit()) {
        return Err(ProfileStoreError::InvalidPane(pane_id.to_owned()));
    }
    Ok(number)
}

fn terminal_entry_path(
    home: &Path,
    profile: &ProfileSelection,
    pane_id: &str,
) -> Result<PathBuf, ProfileStoreError> {
    let number = pane_suffix(pane_id)?;
    let root = home.join(OMP_DIR);
    Ok(match profile {
        ProfileSelection::Unprofiled => root.join(AGENT_DIR).join(TERMINAL_SESSIONS_DIR),
        ProfileSelection::Profiled(name) => root
            .join("profiles")
            .join(name)
            .join(AGENT_DIR)
            .join(TERMINAL_SESSIONS_DIR),
    }
    .join(format!("tmux-%{number}")))
}

fn session_root(home: &Path, profile: &ProfileSelection) -> PathBuf {
    let root = home.join(OMP_DIR);
    match profile {
        ProfileSelection::Unprofiled => root.join(AGENT_DIR).join(SESSIONS_DIR),
        ProfileSelection::Profiled(name) => root
            .join("profiles")
            .join(name)
            .join(AGENT_DIR)
            .join(SESSIONS_DIR),
    }
}

fn probe_missing_entry(path: &Path) -> RootProbe {
    let Some(root) = path.parent() else {
        return RootProbe::Missing;
    };
    match fs::read_dir(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => RootProbe::Missing,
        Err(error) => RootProbe::Unreadable(error.to_string()),
        Ok(mut entries) => match entries.next() {
            None => RootProbe::Empty,
            Some(Ok(_)) => RootProbe::NonEmptyWithoutEntry,
            Some(Err(error)) => RootProbe::Unreadable(error.to_string()),
        },
    }
}

fn read_probe(path: &Path) -> Result<(RootProbe, Option<String>), ProfileStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok((RootProbe::EntryPresent, Some(text))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((probe_missing_entry(path), None))
        }
        Err(error) => Err(ProfileStoreError::Unreadable {
            path: path.to_owned(),
            detail: error.to_string(),
        }),
    }
}

fn parse_file(path: &Path, text: &str) -> Result<TerminalSessionEntry, ProfileStoreError> {
    parse_entry(text).map_err(|error| match error {
        ProfileStoreError::Malformed { line_count, .. } => ProfileStoreError::Malformed {
            path: path.to_owned(),
            line_count,
        },
        other => other,
    })
}

/// Probe both roots for a profiled pane and validate the candidate session path.
///
/// The unprofiled root is never used as a fallback once an `--profile` argument was observed.
/// A present candidate outside the selected `agent/sessions` root is an explicit UNKNOWN.
#[must_use]
pub fn read_store_entry(
    home: &Path,
    pane_id: &str,
    profile: &ProfileSelection,
) -> Result<ValidatedSessionEntry, ProfileStoreError> {
    let selected_path = terminal_entry_path(home, profile, pane_id)?;
    let unprofiled_profile = ProfileSelection::Unprofiled;
    let unprofiled_path = terminal_entry_path(home, &unprofiled_profile, pane_id)?;
    let (unprofiled_probe, _) = read_probe(&unprofiled_path)?;
    let (selected_probe, selected_text) = read_probe(&selected_path)?;
    let roots = RootObservations {
        unprofiled: unprofiled_probe,
        profiled: match profile {
            ProfileSelection::Profiled(_) => Some(selected_probe.clone()),
            ProfileSelection::Unprofiled => None,
        },
    };

    let Some(text) = selected_text else {
        return match selected_probe {
            RootProbe::Empty => Err(ProfileStoreError::EmptyStore {
                root: selected_path
                    .parent()
                    .map_or_else(|| selected_path.clone(), Path::to_owned),
            }),
            RootProbe::Unreadable(detail) => Err(ProfileStoreError::Unreadable {
                path: selected_path,
                detail,
            }),
            _ => Err(ProfileStoreError::Absent {
                expected: selected_path,
                roots,
            }),
        };
    };

    let entry = parse_file(&selected_path, &text)?;
    let expected_root = session_root(home, profile);
    if !entry.session_path.is_absolute() || !entry.session_path.starts_with(&expected_root) {
        return Err(ProfileStoreError::ForeignSessionPath {
            observed: entry.session_path,
            expected_root,
        });
    }
    Ok(ValidatedSessionEntry {
        profile: profile.clone(),
        terminal_entry_path: selected_path,
        sessions_root: session_root(home, profile),
        entry,
        roots,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaneRoute {
    Unprofiled {
        command: String,
    },
    Profiled {
        command: String,
        profile: String,
        entry: ValidatedSessionEntry,
        outcome: StateOutcome,
    },
}

impl PaneRoute {
    #[must_use]
    pub fn is_profiled(&self) -> bool {
        matches!(self, Self::Profiled { .. })
    }

    #[must_use]
    pub fn outcome(&self) -> Option<&StateOutcome> {
        match self {
            Self::Unprofiled { .. } => None,
            Self::Profiled { outcome, .. } => Some(outcome),
        }
    }
}

/// Project the existing six-outcome OMP state vocabulary onto dispatch readiness.
///
/// No seventh state is invented: `None` means the existing state outcome did not establish the
/// two fields needed for an idle decision, or the outcome was not an answered state.
#[must_use]
pub fn idle_from_state(outcome: &StateOutcome) -> Option<bool> {
    let StateOutcome::Answered(state) = outcome else {
        return None;
    };
    match (state.is_streaming, state.queued_message_count) {
        (Some(false), Some(0)) => Some(true),
        (Some(true), _) => Some(false),
        (_, Some(count)) if count > 0 => Some(false),
        _ => None,
    }
}

fn map_run_error(program: &str, error: RunError) -> PaneOracleError {
    match error {
        RunError::Timeout => PaneOracleError::Command {
            program: program.to_owned(),
            detail: "TIMEOUT_UNMEASURED".to_owned(),
        },
        RunError::Cancelled(kind) => PaneOracleError::Command {
            program: program.to_owned(),
            detail: format!("CANCELLED kind={kind:?}"),
        },
        RunError::Process(error) => PaneOracleError::Command {
            program: program.to_owned(),
            detail: error.to_string(),
        },
    }
}

async fn command_output(
    cx: &Cx,
    program: &str,
    args: &[String],
) -> Result<asupersync::process::Output, PaneOracleError> {
    let mut command = Command::new(program);
    for arg in args {
        command.arg(arg);
    }
    run_output(cx, command)
        .await
        .map_err(|error| map_run_error(program, error))
}

async fn command_text(cx: &Cx, program: &str, args: &[String]) -> Result<String, PaneOracleError> {
    let output = command_output(cx, program, args).await?;
    if !output.status.success() {
        return Err(PaneOracleError::Command {
            program: program.to_owned(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn first_nonempty_line(output: &str, program: &str) -> Result<String, PaneOracleError> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| PaneOracleError::Command {
            program: program.to_owned(),
            detail: "EMPTY_OUTPUT".to_owned(),
        })
}

async fn child_pid(cx: &Cx, pane_pid: &str) -> Result<Option<String>, PaneOracleError> {
    let output = command_output(cx, "pgrep", &["-P".to_owned(), pane_pid.to_owned()]).await?;
    if output.status.success() {
        return first_nonempty_line(&String::from_utf8_lossy(&output.stdout), "pgrep").map(Some);
    }
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return Ok(None);
    }
    Err(PaneOracleError::Command {
        program: "pgrep".to_owned(),
        detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

/// Resolve one pane's profile and, only for a validated profile entry, read OMP state.
///
/// The caller owns `Cx`; every subprocess observes its cancellation and is group-killed by
/// `subprocess-contract` on timeout. `~/.omp` is read only.
pub async fn read_pane_state(
    cx: &Cx,
    pane_id: &str,
    home: &Path,
    binary: &str,
) -> Result<PaneRoute, PaneOracleError> {
    let pane_pid = command_text(
        cx,
        "tmux",
        &[
            "display-message".to_owned(),
            "-p".to_owned(),
            "-t".to_owned(),
            pane_id.to_owned(),
            "#{pane_pid}".to_owned(),
        ],
    )
    .await
    .and_then(|output| first_nonempty_line(&output, "tmux"))?;
    let Some(child_pid) = child_pid(cx, &pane_pid).await? else {
        return Ok(PaneRoute::Unprofiled {
            command: "<no-child>".to_owned(),
        });
    };
    let command = command_text(
        cx,
        "ps",
        &[
            "-p".to_owned(),
            child_pid,
            "-o".to_owned(),
            "command=".to_owned(),
        ],
    )
    .await
    .and_then(|output| first_nonempty_line(&output, "ps"))?;
    let selection = profile_from_command(&command)?;
    match selection {
        ProfileSelection::Unprofiled => Ok(PaneRoute::Unprofiled { command }),
        ProfileSelection::Profiled(profile) => {
            let profile_selection = ProfileSelection::Profiled(profile.clone());
            let validated = read_store_entry(home, pane_id, &profile_selection)?;
            let session_path = validated.entry.session_path.to_string_lossy().into_owned();
            let outcome = read_state(
                cx,
                binary,
                Some(&session_path),
                Some(&validated.sessions_root),
            )
            .await;
            Ok(PaneRoute::Profiled {
                command,
                profile,
                entry: validated,
                outcome,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_home(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omp-pane-oracle-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn profile_argument_and_unprofiled_root_are_distinct() {
        assert_eq!(
            profile_from_command("/usr/local/bin/omp --profile claude --resume x").unwrap(),
            ProfileSelection::Profiled("claude".to_owned())
        );
        assert_eq!(
            profile_from_command("/bin/zsh -l").unwrap(),
            ProfileSelection::Unprofiled
        );
        assert_eq!(
            profile_from_command("omp --profile").unwrap_err(),
            ProfileSelectionError::MissingValue
        );
    }

    #[test]
    fn variable_length_entries_preserve_missing_and_unknown_tokens() {
        let two = parse_entry("/repo\n/host/.omp/agent/sessions/a.jsonl\n").unwrap();
        assert_eq!(two.shape, EntryShape::TwoLines);
        assert_eq!(two.state_token, StateToken::Missing);
        assert_eq!(two.state_token.status(), "Unknown/MissingStateToken");

        let three = parse_entry("/repo\n/host/.omp/agent/sessions/a.jsonl\nfresh\n").unwrap();
        assert_eq!(three.shape, EntryShape::ThreeLines);
        assert_eq!(three.state_token, StateToken::Fresh);

        let unknown = parse_entry("/repo\n/host/.omp/agent/sessions/a.jsonl\nfuture\n").unwrap();
        assert_eq!(
            unknown.state_token,
            StateToken::Unknown("future".to_owned())
        );
    }

    #[test]
    fn one_and_four_line_entries_are_typed_malformed() {
        assert_eq!(
            parse_entry("only-one-line\n").unwrap_err(),
            ProfileStoreError::Malformed {
                path: PathBuf::from("<memory>"),
                line_count: 1
            }
        );
        assert_eq!(
            parse_entry("a\nb\nc\nd\n").unwrap_err(),
            ProfileStoreError::Malformed {
                path: PathBuf::from("<memory>"),
                line_count: 4
            }
        );
    }

    #[test]
    fn foreign_fixture_path_is_unknown_not_trusted() {
        let home = test_home("foreign");
        let terminal = home.join(".omp/profiles/claude/agent/terminal-sessions");
        fs::create_dir_all(&terminal).unwrap();
        fs::write(
            terminal.join("tmux-%8"),
            "/repo\n/tmp/scratch/session.jsonl\n",
        )
        .unwrap();
        let error = read_store_entry(
            &home,
            "%8",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProfileStoreError::ForeignSessionPath { .. }
        ));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn empty_selected_store_is_an_error_not_no_panes() {
        let home = test_home("empty");
        let root = home.join(".omp/profiles/claude/agent/terminal-sessions");
        fs::create_dir_all(&root).unwrap();
        let error = read_store_entry(
            &home,
            "%8",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .unwrap_err();
        assert_eq!(error, ProfileStoreError::EmptyStore { root: root.clone() });
        assert!(error.to_string().starts_with("PANE_ORACLE_EMPTY_STORE"));
        assert_eq!(PaneOracleError::Store(error.clone()).exit_code(), 4);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn missing_entry_is_absent_even_when_the_selected_root_is_nonempty() {
        let home = test_home("absent");
        let terminal = home.join(".omp/profiles/claude/agent/terminal-sessions");
        fs::create_dir_all(&terminal).unwrap();
        fs::write(terminal.join("tmux-%7"), "/repo\n/session.jsonl\n").unwrap();
        let error = read_store_entry(
            &home,
            "%8",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .unwrap_err();
        assert!(matches!(error, ProfileStoreError::Absent { .. }));
        assert!(error.to_string().contains("PANE_ORACLE_ABSENT"));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn mutation_is_rejected_and_the_original_entry_restores_byte_identically() {
        let home = test_home("mutation");
        let terminal = home.join(".omp/profiles/claude/agent/terminal-sessions");
        let sessions = home.join(".omp/profiles/claude/agent/sessions");
        fs::create_dir_all(&terminal).unwrap();
        fs::create_dir_all(&sessions).unwrap();
        let session = sessions.join("session.jsonl");
        fs::write(&session, "{}\n").unwrap();
        let entry_path = terminal.join("tmux-%8");
        let original = format!("/repo\n{}\nfresh\n", session.display());
        fs::write(&entry_path, &original).unwrap();
        assert!(read_store_entry(
            &home,
            "%8",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .is_ok());

        fs::write(&entry_path, "/repo\n/tmp/foreign.jsonl\nfresh\n").unwrap();
        assert!(matches!(
            read_store_entry(
                &home,
                "%8",
                &ProfileSelection::Profiled("claude".to_owned())
            ),
            Err(ProfileStoreError::ForeignSessionPath { .. })
        ));
        fs::write(&entry_path, &original).unwrap();
        assert_eq!(fs::read(&entry_path).unwrap(), original.as_bytes());
        assert!(read_store_entry(
            &home,
            "%8",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .is_ok());
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn valid_profile_path_is_selected_even_when_unprofiled_root_is_missing() {
        let home = test_home("valid");
        let terminal = home.join(".omp/profiles/claude/agent/terminal-sessions");
        let sessions = home.join(".omp/profiles/claude/agent/sessions");
        fs::create_dir_all(&terminal).unwrap();
        fs::create_dir_all(&sessions).unwrap();
        let session = sessions.join("session.jsonl");
        fs::write(&session, "{}\n").unwrap();
        fs::write(
            terminal.join("tmux-%6"),
            format!("/repo\n{}\n", session.display()),
        )
        .unwrap();
        let entry = read_store_entry(
            &home,
            "%6",
            &ProfileSelection::Profiled("claude".to_owned()),
        )
        .unwrap();
        assert_eq!(entry.entry.session_path, session);
        assert_eq!(entry.roots.unprofiled, RootProbe::Missing);
        assert_eq!(entry.roots.profiled, Some(RootProbe::EntryPresent));
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn idle_projection_reuses_state_outcomes_without_a_new_state_variant() {
        let answered = StateOutcome::Answered(Box::new(ompo_doctor::omp_state::OmpState {
            session_id: Some("s".to_owned()),
            requested_session: None,
            model: None,
            is_streaming: Some(false),
            queued_message_count: Some(0),
            message_count: Some(1),
            lifecycle: "running".to_owned(),
            protocol_negotiated: 2,
            raw: serde_json::json!({}),
        }));
        assert_eq!(idle_from_state(&answered), Some(true));
        assert_eq!(idle_from_state(&StateOutcome::NoPayload), None);
    }
}
