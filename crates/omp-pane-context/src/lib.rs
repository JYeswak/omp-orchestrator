#![forbid(unsafe_code)]

//! Session-file context sensor for OMP panes.
//!
//! `ntm --robot-context` estimates context from pane SCROLLBACK against an assumed 128k window
//! and labels every row `source=scrollback_estimate confidence=low`. Measured 2026-09-25 against
//! the OMP session files it was 2.7x to 11.7x off (bead cp-4yz11). OMP already writes the
//! provider-reported prompt size to disk on every assistant turn; this crate reads it.
//!
//! # The chain, and the trap at each link
//!
//! ```text
//! tmux pane  -> pane_pid, pane_tty
//!            -> the interactive `omp` process under pane_pid (argv, start time)
//!            -> profile from argv (`--profile P`), else the unprofiled root
//!            -> <agent root>/terminal-sessions/<tty basename>      (the breadcrumb)
//!            -> line 2 = session JSONL, validated under <agent root>/sessions
//!            -> newest usage anchor after the newest compaction
//!            -> contextSnapshot.promptTokens / model contextWindow
//! ```
//!
//! - **The breadcrumb key is the TTY, not `tmux-%N`.** OMP's `getTerminalId()`
//!   (`pi-tui/src/ttyid.ts:41-63`, omp 18.3.1) returns `ttyname(0)` minus `/dev/` when stdin is a
//!   TTY and only falls back to `tmux-$TMUX_PANE` when it is not. An interactive pane always has
//!   a TTY, so a `tmux-%N` breadcrumb was written by a NON-interactive omp (a tool-spawned child,
//!   a conformance probe) and points at the wrong session. It is never read here.
//! - **A breadcrumb outlives its process, and TTYs are reused.** A breadcrumb older than the
//!   live omp process was written by a previous process on that TTY: `STALE_POINTER`, UNKNOWN.
//! - **A breadcrumb can point outside the profile's store** (`--session <fixture>`, a
//!   `sessions/../..` path, or a symlink inside `sessions/` to a file elsewhere). Containment
//!   is checked on the CANONICAL paths and the resolved file is the one read:
//!   `FOREIGN_SESSION_PATH`, UNKNOWN.
//! - **A zombie `omp` row is not an agent** (`ps` STAT `Z`): it is skipped, so its pane reads
//!   `NO_OMP_PROCESS` rather than a number from the breadcrumb it left.
//! - **No session file is UNKNOWN, never 0.** A fresh session whose JSONL is not materialised
//!   has no usage record; reporting 0% would read as an empty context.
//!
//! # What the number is, exactly
//!
//! OMP's own figure (`session-stats.ts:getContextBreakdown`) is `correctedPromptTokens(anchor)`
//! plus a tokenizer estimate of every message appended after the anchor. This crate reports the
//! first term only — the provider-reported prompt at the newest anchor, minus any recorded
//! `historyRewriteTokensRemoved`. It is therefore a FLOOR that trails OMP's live figure by at
//! most the messages since the last assistant turn, and `read_at` says how old it is. After a
//! compaction with no newer anchor it reports the compaction entry's `tokensAfter`. Only the
//! CURRENT BRANCH is read — the parent chain from the newest complete entry, which is the leaf
//! OMP itself selects on load — so an anchor on an abandoned `/tree` branch never counts.
//!
//! # NO-CLAIM
//!
//! This reads files. It proves the omp process exists and is not a zombie at the `ps` instant,
//! not that it is making progress, and it does not tokenise the tail. `~/.omp` is read only.
//! A live process that navigated `/tree` without appending yet has an in-memory leaf the file
//! does not show; this reads the leaf OMP would restore on reload.

use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subprocess_contract::{bounded_output, BoundedOutcome};

/// The literal every consumer prints when no reading exists. Never `0`.
pub const UNKNOWN: &str = "UNKNOWN";

/// `ps` reports start time at one-second resolution; allow that much skew plus one.
const POINTER_SLACK_SECS: u64 = 2;
const TAIL_CHUNK: usize = 64 * 1024;

// ------------------------------------------------------------------------------------------
// Profiles and paths
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    Profiled(String),
    Unprofiled,
}

impl Profile {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Profiled(name) => name,
            Self::Unprofiled => "-",
        }
    }
}

/// Parse `--profile <name>` / `--profile=<name>` from an omp argv line.
///
/// A missing, empty, or path-like value refuses: it must never silently select the unprofiled
/// root, which is TRAP ZERO of `skill://omp-integration`.
pub fn profile_from_argv(argv: &str) -> Result<Profile, String> {
    let args: Vec<&str> = argv.split_whitespace().collect();
    for (index, arg) in args.iter().enumerate() {
        let value = if *arg == "--profile" {
            match args.get(index + 1) {
                Some(value) if !value.starts_with('-') => *value,
                _ => return Err("PROFILE_MISSING_VALUE".to_owned()),
            }
        } else if let Some(value) = arg.strip_prefix("--profile=") {
            value
        } else {
            continue;
        };
        if value.is_empty() || value == "." || value == ".." || value.contains('/') {
            return Err(format!("PROFILE_UNSAFE_VALUE value={value:?}"));
        }
        return Ok(Profile::Profiled(value.to_owned()));
    }
    Ok(Profile::Unprofiled)
}

/// `~/.omp/agent` or `~/.omp/profiles/<name>/agent`.
#[must_use]
pub fn agent_root(home: &Path, profile: &Profile) -> PathBuf {
    let omp = home.join(".omp");
    match profile {
        Profile::Unprofiled => omp.join("agent"),
        Profile::Profiled(name) => omp.join("profiles").join(name).join("agent"),
    }
}

/// OMP's terminal id for a TTY path: `/dev/ttys004` -> `ttys004`, `/dev/pts/3` -> `pts-3`
/// (mirrors `ttyPath.slice(5).replace(/\//g, "-")`).
#[must_use]
pub fn terminal_id(tty: &str) -> Option<String> {
    let rest = tty.strip_prefix("/dev/")?;
    if rest.is_empty() {
        return None;
    }
    Some(rest.replace('/', "-"))
}

/// Whether an argv line is an interactive `omp` launch (`omp …` or `bun …/omp …`).
#[must_use]
pub fn is_omp_argv(argv: &str) -> bool {
    let is_omp = |token: &str| {
        Path::new(token).file_name().is_some_and(|name| name == "omp")
            || token.ends_with("pi-coding-agent/dist/cli.js")
    };
    let mut tokens = argv.split_whitespace();
    let Some(first) = tokens.next() else {
        return false;
    };
    if is_omp(first) {
        return true;
    }
    // `bun|node [runtime flags] <omp entry>`: the entry is the first non-flag argument.
    let runtime = Path::new(first)
        .file_name()
        .is_some_and(|name| name == "bun" || name == "node");
    runtime
        && tokens
            .find(|token| !token.starts_with('-'))
            .is_some_and(is_omp)
}

/// Parse `ps` ELAPSED time: `mm:ss`, `hh:mm:ss`, or `d-hh:mm:ss`.
#[must_use]
pub fn parse_etime(text: &str) -> Option<u64> {
    let text = text.trim();
    let (days, clock) = match text.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, text),
    };
    let parts: Vec<u64> = clock
        .split(':')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    if minutes >= 60 || seconds >= 60 {
        return None;
    }
    Some(((days * 24 + hours) * 60 + minutes) * 60 + seconds)
}

// ------------------------------------------------------------------------------------------
// The breadcrumb
// ------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breadcrumb {
    pub cwd: String,
    pub session_file: PathBuf,
    pub fresh: bool,
}

/// Parse a terminal breadcrumb exactly as `session-paths.ts:readTerminalBreadcrumbEntry` does:
/// line 1 cwd, line 2 session file, then any extras (`fresh`, `cwdstat <dev> <ino>`). OMP 18.3.1
/// writes FOUR lines; a reader that requires two or three refuses every current breadcrumb.
#[must_use]
pub fn parse_breadcrumb(text: &str) -> Option<Breadcrumb> {
    let lines: Vec<&str> = text.trim().split('\n').map(str::trim).collect();
    if lines.len() < 2 || lines[0].is_empty() || lines[1].is_empty() {
        return None;
    }
    Some(Breadcrumb {
        cwd: lines[0].to_owned(),
        session_file: PathBuf::from(lines[1]),
        fresh: lines[2..].iter().any(|extra| *extra == "fresh"),
    })
}

// ------------------------------------------------------------------------------------------
// The session file
// ------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingSource {
    /// `contextSnapshot.promptTokens` (or the usage fallback) of the newest usage anchor.
    UsageAnchor,
    /// `tokensAfter` of a compaction newer than every usage anchor.
    CompactionTokensAfter,
}

impl ReadingSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UsageAnchor => "usage_anchor",
            Self::CompactionTokensAfter => "compaction_tokens_after",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageRecord {
    pub tokens: u64,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The session entry's own ISO-8601 `timestamp`: when the reading was taken.
    pub read_at: String,
    pub source: ReadingSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TailScan {
    Record(UsageRecord),
    /// No assistant turn with usage and no compaction: nothing was ever measured.
    NoUsageRecord,
    /// A `reset_boundary` is newer than every anchor: the window restarted unmeasured.
    ResetWithoutUsage { at: String },
    /// A compaction is newer than every anchor and carries no `tokensAfter`.
    CompactionWithoutTokens { at: String },
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// `pi-agent-core/src/compaction/compaction.ts:calculateContextTokens`.
fn context_tokens(usage: &Value) -> u64 {
    if let Some(tokens) = usage.get("contextTokens").and_then(Value::as_u64) {
        return tokens;
    }
    let orchestration = usage.get("orchestration").map_or(0, |o| {
        u64_field(o, "input") + u64_field(o, "output") + u64_field(o, "cacheRead")
    });
    let total = u64_field(usage, "totalTokens");
    let raw = if total > 0 {
        total
    } else {
        u64_field(usage, "input")
            + u64_field(usage, "output")
            + u64_field(usage, "cacheRead")
            + u64_field(usage, "cacheWrite")
    };
    raw.saturating_sub(orchestration)
}

/// `compaction.ts:calculatePromptTokens`.
fn prompt_tokens(usage: &Value) -> u64 {
    if let Some(tokens) = usage.get("contextTokens").and_then(Value::as_u64) {
        return tokens;
    }
    let prompt = u64_field(usage, "input") + u64_field(usage, "cacheRead") + u64_field(usage, "cacheWrite");
    if prompt > 0 {
        prompt
    } else {
        context_tokens(usage)
    }
}

/// `compaction.ts:hasContextTokenUsage`.
fn has_context_usage(usage: &Value) -> bool {
    usage.get("contextTokens").and_then(Value::as_u64).unwrap_or(0) > 0
        || u64_field(usage, "input") + u64_field(usage, "cacheRead") + u64_field(usage, "cacheWrite") > 0
        || context_tokens(usage) > u64_field(usage, "output")
}

/// `transcript-tokens.ts:isTranscriptUsageAnchor` + `session-stats.ts:correctedPromptTokens`.
/// Returns the corrected prompt tokens when `message` may anchor, else `None`.
fn anchor_tokens(message: &Value) -> Option<u64> {
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    if matches!(
        message.get("stopReason").and_then(Value::as_str),
        Some("aborted" | "error")
    ) {
        return None;
    }
    let usage = message.get("usage")?;
    if !has_context_usage(usage) {
        return None;
    }
    let snapshot = message.get("contextSnapshot");
    let provider_prompt = snapshot
        .and_then(|s| s.get("promptTokens"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| prompt_tokens(usage));
    let removed = snapshot
        .and_then(|s| s.get("historyRewriteTokensRemoved"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(provider_prompt.saturating_sub(removed))
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// `model_change.model` is `<provider>/<model id>`; provider ids carry no `/`, model ids may.
fn split_model_ref(model_ref: &str) -> Option<(String, String)> {
    let (provider, model) = model_ref.split_once('/')?;
    (!provider.is_empty() && !model.is_empty()).then(|| (provider.to_owned(), model.to_owned()))
}

/// Visit complete lines from the END of the file, newest first, without reading the whole
/// file: session files reach hundreds of MB. `visit` returns `false` to stop.
pub fn for_each_line_rev(
    path: &Path,
    mut visit: impl FnMut(&[u8]) -> bool,
) -> std::io::Result<()> {
    let mut file = File::open(path)?;
    let mut pos = file.metadata()?.len();
    let mut chunk = vec![0u8; TAIL_CHUNK];
    // Segments of the line currently being assembled, NEWEST segment first.
    let mut pending: Vec<Vec<u8>> = Vec::new();
    let emit = |head: &[u8], pending: &mut Vec<Vec<u8>>, visit: &mut dyn FnMut(&[u8]) -> bool| {
        if pending.is_empty() {
            return visit(head);
        }
        let mut line = head.to_vec();
        for segment in pending.iter().rev() {
            line.extend_from_slice(segment);
        }
        pending.clear();
        visit(&line)
    };
    while pos > 0 {
        let start = pos.saturating_sub(TAIL_CHUNK as u64);
        let len = usize::try_from(pos - start).unwrap_or(TAIL_CHUNK);
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut chunk[..len])?;
        pos = start;
        let buf = &chunk[..len];
        let mut end = len;
        while let Some(newline) = buf[..end].iter().rposition(|byte| *byte == b'\n') {
            if !emit(&buf[newline + 1..end], &mut pending, &mut visit) {
                return Ok(());
            }
            end = newline;
        }
        if end > 0 {
            pending.push(buf[..end].to_vec());
        }
    }
    if !pending.is_empty() {
        emit(&[], &mut pending, &mut visit);
    }
    Ok(())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

/// Scan a session JSONL backwards for the reading OMP would anchor on.
///
/// # Only the CURRENT BRANCH counts
///
/// A session file is a tree: every entry names its `parentId`, and `/tree` navigation or a
/// `branch_summary` starts a new branch from an older entry while the abandoned branch's lines
/// stay in the file. On load OMP's `SessionIndex.add` makes the LAST entry the leaf
/// (`session-manager.ts:462`) and `getBranch()` walks `parentId` from it. So the newest line
/// in the file is the leaf, and an entry is on the branch only if the parent chain from that
/// leaf reaches it. Parents always precede children in an append-only file, so one reverse
/// pass following a single `wanted` id visits the branch newest-first; every other line is
/// skipped, including a newer anchor on an abandoned branch.
///
/// A trailing line that does not parse (an append in progress) is skipped, so the leaf is the
/// newest COMPLETE entry.
pub fn last_usage_record(path: &Path) -> std::io::Result<TailScan> {
    let mut outcome = TailScan::NoUsageRecord;
    // The next entry id on the current branch; `None` until the leaf is seen.
    let mut wanted: Option<String> = None;
    // The newest `model_change` seen so far: the model whose window applies NOW.
    let mut latest_model: Option<(String, String)> = None;
    // A compaction newer than every anchor, waiting for a model to be attributed.
    let mut compaction: Option<(u64, String)> = None;
    for_each_line_rev(path, |raw| {
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if line.is_empty() || !contains(line, b"\"id\":\"") {
            return true;
        }
        let Ok(entry) = serde_json::from_slice::<Value>(line) else {
            return true;
        };
        let Some(id) = entry.get("id").and_then(Value::as_str) else {
            return true;
        };
        if wanted.is_some() && wanted.as_deref() != Some(id) {
            // Off the current branch (or the session header).
            return true;
        }
        let parent = entry.get("parentId").and_then(Value::as_str).map(str::to_owned);
        let at = str_field(&entry, "timestamp").unwrap_or_default();
        let keep_going = match entry.get("type").and_then(Value::as_str) {
            Some("model_change") => {
                let model = entry.get("model").and_then(Value::as_str).and_then(split_model_ref);
                if let Some((tokens, at)) = compaction.take() {
                    let (provider, model) = latest_model.clone().or(model).unzip();
                    outcome = TailScan::Record(UsageRecord {
                        tokens,
                        provider,
                        model,
                        read_at: at,
                        source: ReadingSource::CompactionTokensAfter,
                    });
                    return false;
                }
                if latest_model.is_none() {
                    latest_model = model;
                }
                true
            }
            Some("message") => {
                let Some(message) = entry
                    .get("message")
                    .filter(|m| m.get("role").and_then(Value::as_str) == Some("assistant"))
                else {
                    // A user/tool turn: still on the branch, carries no usage.
                    wanted = parent;
                    return wanted.is_some();
                };
                let own = str_field(message, "provider").zip(str_field(message, "model"));
                if let Some((tokens, at)) = compaction.take() {
                    // Any assistant turn names the model in force when the compaction ran.
                    let (provider, model) = latest_model.clone().or(own).unzip();
                    outcome = TailScan::Record(UsageRecord {
                        tokens,
                        provider,
                        model,
                        read_at: at,
                        source: ReadingSource::CompactionTokensAfter,
                    });
                    return false;
                }
                if let Some(tokens) = anchor_tokens(message) {
                    let (provider, model) = latest_model.clone().or(own).unzip();
                    outcome = TailScan::Record(UsageRecord {
                        tokens,
                        provider,
                        model,
                        read_at: at,
                        source: ReadingSource::UsageAnchor,
                    });
                    false
                } else {
                    true
                }
            }
            Some("compaction") if compaction.is_none() => {
                match entry.get("tokensAfter").and_then(Value::as_u64) {
                    Some(tokens) => {
                        compaction = Some((tokens, at));
                        true
                    }
                    None => {
                        outcome = TailScan::CompactionWithoutTokens { at };
                        false
                    }
                }
            }
            Some("reset_boundary") if compaction.is_none() => {
                outcome = TailScan::ResetWithoutUsage { at };
                false
            }
            _ => true,
        };
        wanted = parent;
        // A missing `parentId` is the branch root: nothing older is on this branch.
        keep_going && wanted.is_some()
    })?;
    if let Some((tokens, at)) = compaction {
        // A compaction with no model anywhere before it.
        let (provider, model) = latest_model.unzip();
        outcome = TailScan::Record(UsageRecord {
            tokens,
            provider,
            model,
            read_at: at,
            source: ReadingSource::CompactionTokensAfter,
        });
    }
    Ok(outcome)
}

// ------------------------------------------------------------------------------------------
// Context windows
// ------------------------------------------------------------------------------------------

/// Resolves a model's context window for a profile.
pub trait WindowCatalog: Sync {
    /// `Ok(None)`: the catalog answered and does not know the model.
    /// `Err`: the catalog itself could not be read.
    fn context_window(&self, profile: &Profile, provider: &str, model: &str)
        -> Result<Option<u64>, String>;
}

/// Find `contextWindow` for `(provider, id)` in parsed `omp models --json` output.
#[must_use]
pub fn window_in_models(models: &Value, provider: &str, model: &str) -> Option<u64> {
    models
        .get("models")?
        .as_array()?
        .iter()
        .find(|row| {
            row.get("provider").and_then(Value::as_str) == Some(provider)
                && row.get("id").and_then(Value::as_str) == Some(model)
        })?
        .get("contextWindow")?
        .as_u64()
        .filter(|window| *window > 0)
}

/// Parse `omp models --json`; `None` unless it carries a `models` array.
#[must_use]
pub fn parse_models(text: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    value.get("models")?.as_array()?;
    Some(value)
}

type CatalogSlot = Arc<OnceLock<Result<Value, String>>>;

/// The model registry OMP itself uses, read through `omp [--profile P] models --json` once per
/// profile per process (single-flight across threads). It runs with `cwd=$HOME`: never inside a
/// project checkout, whose `.omp/` would be imported at startup (`skill://omp-integration`,
/// PROJECT TRUST). About 9s per profile on this host.
pub struct OmpModelsCatalog {
    program: String,
    home: PathBuf,
    timeout: Duration,
    slots: Mutex<HashMap<Profile, CatalogSlot>>,
}

impl OmpModelsCatalog {
    #[must_use]
    pub fn new(program: impl Into<String>, home: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            home: home.into(),
            timeout,
            slots: Mutex::new(HashMap::new()),
        }
    }

    fn fetch(&self, profile: &Profile) -> Result<Value, String> {
        let mut command = Command::new(&self.program);
        if let Profile::Profiled(name) = profile {
            command.args(["--profile", name]);
        }
        command.args(["models", "--json"]).current_dir(&self.home);
        match bounded_output(&mut command, self.timeout) {
            BoundedOutcome::Completed(output) if output.status.success() => {
                parse_models(&String::from_utf8_lossy(&output.stdout))
                    .ok_or_else(|| "OMP_MODELS_UNPARSEABLE".to_owned())
            }
            BoundedOutcome::Completed(output) => Err(format!(
                "OMP_MODELS_FAILED status={} stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            BoundedOutcome::TimedOut => Err("OMP_MODELS_TIMEOUT_UNMEASURED".to_owned()),
            BoundedOutcome::Unspawned(error) => Err(format!("OMP_MODELS_UNSPAWNED detail={error}")),
        }
    }
}

impl WindowCatalog for OmpModelsCatalog {
    fn context_window(
        &self,
        profile: &Profile,
        provider: &str,
        model: &str,
    ) -> Result<Option<u64>, String> {
        let slot = {
            let mut slots = self
                .slots
                .lock()
                .map_err(|_| "OMP_MODELS_CACHE_POISONED".to_owned())?;
            Arc::clone(slots.entry(profile.clone()).or_default())
        };
        let models = slot
            .get_or_init(|| self.fetch(profile))
            .as_ref()
            .map_err(Clone::clone)?;
        Ok(window_in_models(models, provider, model))
    }
}

// ------------------------------------------------------------------------------------------
// One pane
// ------------------------------------------------------------------------------------------

/// The interactive omp process found under a pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmpProcess {
    pub pid: u32,
    pub argv: String,
    /// Unix seconds, from `now - etime`.
    pub started_epoch: u64,
}

/// What the OS says about one pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneObservation {
    pub pane_id: String,
    /// `session:window.pane`.
    pub label: String,
    pub tty: String,
    pub omp: Option<OmpProcess>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    NoOmpProcess,
    ProfileInvalid,
    NoTerminalId,
    NoPointer,
    PointerUnreadable,
    StalePointer,
    MalformedPointer,
    ForeignSessionPath,
    NoSessionFile,
    SessionUnreadable,
    NoUsageRecord,
    ResetWithoutUsage,
    CompactionWithoutTokens,
    ModelUnknown,
    CatalogUnavailable,
    WindowUnknown,
}

impl UnknownReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoOmpProcess => "NO_OMP_PROCESS",
            Self::ProfileInvalid => "PROFILE_INVALID",
            Self::NoTerminalId => "NO_TERMINAL_ID",
            Self::NoPointer => "NO_POINTER",
            Self::PointerUnreadable => "POINTER_UNREADABLE",
            Self::StalePointer => "STALE_POINTER",
            Self::MalformedPointer => "MALFORMED_POINTER",
            Self::ForeignSessionPath => "FOREIGN_SESSION_PATH",
            Self::NoSessionFile => "NO_SESSION_FILE",
            Self::SessionUnreadable => "SESSION_UNREADABLE",
            Self::NoUsageRecord => "NO_USAGE_RECORD",
            Self::ResetWithoutUsage => "RESET_WITHOUT_USAGE",
            Self::CompactionWithoutTokens => "COMPACTION_WITHOUT_TOKENS",
            Self::ModelUnknown => "MODEL_UNKNOWN",
            Self::CatalogUnavailable => "CATALOG_UNAVAILABLE",
            Self::WindowUnknown => "WINDOW_UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub percent: f64,
    pub tokens: u64,
    pub context_window: u64,
    pub provider: String,
    pub model: String,
    pub read_at: String,
    pub source: ReadingSource,
    pub profile: Profile,
    pub session_file: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaneContext {
    Measured(Measured),
    Unknown { reason: UnknownReason, detail: String },
}

impl PaneContext {
    fn unknown(reason: UnknownReason, detail: impl Into<String>) -> Self {
        Self::Unknown {
            reason,
            detail: detail.into(),
        }
    }

    /// The percentage as text, or [`UNKNOWN`]. There is no path to `"0"` without a reading.
    #[must_use]
    pub fn percent_text(&self) -> String {
        match self {
            Self::Measured(m) => format!("{:.1}", m.percent),
            Self::Unknown { .. } => UNKNOWN.to_owned(),
        }
    }
}

fn mtime_epoch(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Resolve one pane's context from files. Pure apart from reads under `home` and the catalog.
pub fn read_pane(pane: &PaneObservation, home: &Path, catalog: &dyn WindowCatalog) -> PaneContext {
    let Some(omp) = &pane.omp else {
        return PaneContext::unknown(UnknownReason::NoOmpProcess, "no omp argv under pane_pid");
    };
    let profile = match profile_from_argv(&omp.argv) {
        Ok(profile) => profile,
        Err(detail) => return PaneContext::unknown(UnknownReason::ProfileInvalid, detail),
    };
    let Some(terminal) = terminal_id(&pane.tty) else {
        return PaneContext::unknown(UnknownReason::NoTerminalId, format!("tty={:?}", pane.tty));
    };
    let root = agent_root(home, &profile);
    let pointer = root.join("terminal-sessions").join(&terminal);
    let text = match std::fs::read_to_string(&pointer) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PaneContext::unknown(UnknownReason::NoPointer, pointer.display().to_string());
        }
        Err(error) => {
            return PaneContext::unknown(
                UnknownReason::PointerUnreadable,
                format!("{} {error}", pointer.display()),
            );
        }
    };
    let written = mtime_epoch(&pointer).unwrap_or(0);
    if written.saturating_add(POINTER_SLACK_SECS) < omp.started_epoch {
        return PaneContext::unknown(
            UnknownReason::StalePointer,
            format!(
                "pointer_mtime={written} omp_started={} pid={}",
                omp.started_epoch, omp.pid
            ),
        );
    }
    let Some(crumb) = parse_breadcrumb(&text) else {
        return PaneContext::unknown(UnknownReason::MalformedPointer, pointer.display().to_string());
    };
    let sessions = root.join("sessions");
    let foreign = |detail: String| PaneContext::unknown(UnknownReason::ForeignSessionPath, detail);
    if !crumb.session_file.is_absolute() {
        return foreign(format!("observed={} (relative)", crumb.session_file.display()));
    }
    // Containment is decided on CANONICAL paths only. A lexical `Path::starts_with` both
    // accepts escapes (`sessions/../../x`, `sessions/f -> /tmp/x`) and rejects equivalent
    // spellings of an in-store path (a case-folded APFS component, `/var` vs `/private/var`),
    // so it is used only to classify a path that does not exist at all.
    match std::fs::symlink_metadata(&crumb.session_file) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !crumb.session_file.starts_with(&sessions) {
                return foreign(format!(
                    "observed={} expected_root={} (absent)",
                    crumb.session_file.display(),
                    sessions.display()
                ));
            }
            return PaneContext::unknown(
                UnknownReason::NoSessionFile,
                format!("{} fresh={}", crumb.session_file.display(), crumb.fresh),
            );
        }
        Err(error) => {
            return PaneContext::unknown(
                UnknownReason::SessionUnreadable,
                format!("{} stat: {error}", crumb.session_file.display()),
            );
        }
        Ok(_) => {}
    }
    // The path exists as a directory entry. A dangling symlink or a symlink loop fails here
    // and is SESSION_UNREADABLE: the breadcrumb names something, and it cannot be resolved.
    let resolved = match (
        std::fs::canonicalize(&crumb.session_file),
        std::fs::canonicalize(&sessions),
    ) {
        (Ok(file), Ok(root)) if file.starts_with(&root) => file,
        (Ok(file), Ok(root)) => {
            return foreign(format!(
                "observed={} resolves_to={} expected_root={}",
                crumb.session_file.display(),
                file.display(),
                root.display()
            ));
        }
        (Err(error), _) => {
            return PaneContext::unknown(
                UnknownReason::SessionUnreadable,
                format!("{} canonicalize: {error}", crumb.session_file.display()),
            );
        }
        // The file resolves but the profile's store does not exist: it cannot be inside it.
        (Ok(file), Err(error)) => {
            return foreign(format!(
                "observed={} resolves_to={} expected_root={} ({error})",
                crumb.session_file.display(),
                file.display(),
                sessions.display()
            ));
        }
    };
    if !resolved.is_file() {
        return PaneContext::unknown(
            UnknownReason::SessionUnreadable,
            format!("{} is not a regular file", resolved.display()),
        );
    }
    let record = match last_usage_record(&resolved) {
        Ok(TailScan::Record(record)) => record,
        Ok(TailScan::NoUsageRecord) => {
            return PaneContext::unknown(
                UnknownReason::NoUsageRecord,
                crumb.session_file.display().to_string(),
            );
        }
        Ok(TailScan::ResetWithoutUsage { at }) => {
            return PaneContext::unknown(UnknownReason::ResetWithoutUsage, format!("at={at}"));
        }
        Ok(TailScan::CompactionWithoutTokens { at }) => {
            return PaneContext::unknown(UnknownReason::CompactionWithoutTokens, format!("at={at}"));
        }
        Err(error) => {
            return PaneContext::unknown(
                UnknownReason::SessionUnreadable,
                format!("{} {error}", crumb.session_file.display()),
            );
        }
    };
    let (Some(provider), Some(model)) = (record.provider.clone(), record.model.clone()) else {
        return PaneContext::unknown(UnknownReason::ModelUnknown, format!("tokens={}", record.tokens));
    };
    let window = match catalog.context_window(&profile, &provider, &model) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return PaneContext::unknown(
                UnknownReason::WindowUnknown,
                format!("model={provider}/{model} tokens={}", record.tokens),
            );
        }
        Err(detail) => return PaneContext::unknown(UnknownReason::CatalogUnavailable, detail),
    };
    // `window` is > 0 by construction: `window_in_models` filters 0.
    let percent = record.tokens as f64 / window as f64 * 100.0;
    PaneContext::Measured(Measured {
        percent,
        tokens: record.tokens,
        context_window: window,
        provider,
        model,
        read_at: record.read_at,
        source: record.source,
        profile,
        session_file: crumb.session_file,
    })
}

// ------------------------------------------------------------------------------------------
// OS observation (tmux + ps), bounded
// ------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum ObserveError {
    Tmux(String),
    Ps(String),
    NoPanes(String),
}

impl fmt::Display for ObserveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tmux(detail) => write!(formatter, "TMUX_UNREACHABLE detail={detail}"),
            Self::Ps(detail) => write!(formatter, "PS_UNREACHABLE detail={detail}"),
            Self::NoPanes(target) => write!(formatter, "NO_PANES target={target}"),
        }
    }
}

impl std::error::Error for ObserveError {}

fn run_text(mut command: Command, timeout: Duration) -> Result<String, String> {
    match bounded_output(&mut command, timeout) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        BoundedOutcome::Completed(output) => Err(format!(
            "status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        BoundedOutcome::TimedOut => Err("TIMEOUT_UNMEASURED".to_owned()),
        BoundedOutcome::Unspawned(error) => Err(format!("UNSPAWNED {error}")),
    }
}

const PANE_FORMAT: &str =
    "#{pane_id}\t#{session_name}:#{window_index}.#{pane_index}\t#{pane_pid}\t#{pane_tty}";

/// One `ps` row: pid, ppid, state, elapsed seconds, argv.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    /// `ps` STAT, e.g. `S+`, `Ss`, `Z`.
    pub state: String,
    pub elapsed_secs: u64,
    pub argv: String,
}

impl ProcessRow {
    /// A zombie has exited: its argv is still listed but no agent owns the breadcrumb.
    #[must_use]
    pub fn is_zombie(&self) -> bool {
        self.state.starts_with('Z')
    }
}

/// The `ps` column list [`parse_ps`] reads.
pub const PS_COLUMNS: &str = "pid=,ppid=,stat=,etime=,command=";

/// Parse `ps -A -ww -o pid=,ppid=,stat=,etime=,command=`.
#[must_use]
pub fn parse_ps(text: &str) -> Vec<ProcessRow> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse().ok()?;
            let ppid = fields.next()?.parse().ok()?;
            let state = fields.next()?.to_owned();
            let elapsed_secs = parse_etime(fields.next()?)?;
            let argv = fields.collect::<Vec<_>>().join(" ");
            Some(ProcessRow {
                pid,
                ppid,
                state,
                elapsed_secs,
                argv,
            })
        })
        .collect()
}

/// The shallowest omp process at or below `root_pid` (breadth-first, depth 4). A tool-spawned
/// `omp` child of the interactive agent is deeper and therefore never chosen over it.
#[must_use]
pub fn find_omp(rows: &[ProcessRow], root_pid: u32, now_epoch: u64) -> Option<OmpProcess> {
    let mut frontier = vec![root_pid];
    for _ in 0..=4 {
        let mut hits: Vec<&ProcessRow> = rows
            .iter()
            .filter(|row| {
                frontier.contains(&row.pid) && !row.is_zombie() && is_omp_argv(&row.argv)
            })
            .collect();
        hits.sort_by_key(|row| row.pid);
        if let Some(row) = hits.first() {
            return Some(OmpProcess {
                pid: row.pid,
                argv: row.argv.clone(),
                started_epoch: now_epoch.saturating_sub(row.elapsed_secs),
            });
        }
        frontier = rows
            .iter()
            .filter(|row| frontier.contains(&row.ppid))
            .map(|row| row.pid)
            .collect();
        if frontier.is_empty() {
            return None;
        }
    }
    None
}

#[must_use]
pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Observe a pane (`%N`) or every pane of a tmux session (`name`).
pub fn observe(target: &str, timeout: Duration) -> Result<Vec<PaneObservation>, ObserveError> {
    let mut tmux = Command::new(tick_monitor::TMUX);
    if target.starts_with('%') {
        tmux.args(["display-message", "-p", "-t", target, PANE_FORMAT]);
    } else {
        tmux.args(["list-panes", "-s", "-t", target, "-F", PANE_FORMAT]);
    }
    let panes = run_text(tmux, timeout).map_err(ObserveError::Tmux)?;
    let mut ps = Command::new("ps");
    ps.args(["-A", "-ww", "-o", PS_COLUMNS]);
    let rows = parse_ps(&run_text(ps, timeout).map_err(ObserveError::Ps)?);
    let now = now_epoch();
    let observed: Vec<PaneObservation> = panes
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let pane_id = fields.next()?.trim().to_owned();
            let label = fields.next()?.trim().to_owned();
            let pane_pid: u32 = fields.next()?.trim().parse().ok()?;
            let tty = fields.next()?.trim().to_owned();
            Some(PaneObservation {
                omp: find_omp(&rows, pane_pid, now),
                pane_id,
                label,
                tty,
            })
        })
        .collect();
    if observed.is_empty() {
        return Err(ObserveError::NoPanes(target.to_owned()));
    }
    Ok(observed)
}

/// Every pane of `target`, each with its reading.
pub fn read_target(
    target: &str,
    home: &Path,
    catalog: &dyn WindowCatalog,
    timeout: Duration,
) -> Result<Vec<(PaneObservation, PaneContext)>, ObserveError> {
    Ok(observe(target, timeout)?
        .into_iter()
        .map(|pane| {
            let context = read_pane(&pane, home, catalog);
            (pane, context)
        })
        .collect())
}

/// The highest measured percentage across `readings`, or [`UNKNOWN`] when none was measured.
///
/// Panes with no reading are EXCLUDED, not counted as zero: a session whose only reading is
/// UNKNOWN must print UNKNOWN. When some panes are unknown, the maximum is over the measured
/// panes only and is a lower bound for the session.
#[must_use]
pub fn max_percent_text(readings: &[(PaneObservation, PaneContext)]) -> String {
    readings
        .iter()
        .filter_map(|(_, context)| match context {
            PaneContext::Measured(m) => Some(m.percent),
            PaneContext::Unknown { .. } => None,
        })
        .fold(None, |max: Option<f64>, p| Some(max.map_or(p, |m| m.max(p))))
        .map_or_else(|| UNKNOWN.to_owned(), |p| format!("{p:.1}"))
}
