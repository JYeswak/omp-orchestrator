#![forbid(unsafe_code)]

//! **One binding, not two** — the idle-nudge decision kernel for this repository
//! (bead `omp-orchestrator-47g0`).
//!
//! # The defect this crate exists to make unconstructible
//!
//! Measured 2026-09-07 and re-derived 2026-09-11 against
//! `control-plane/crates/fleet-monitor/src/bin/fleet-idle-monitor.rs`:
//!
//! ```text
//! :22   const DEFAULT_REPO: &str = "<an absolute path to a THIRD repository>";
//! :117  session: env::var("FLEET_SESSION").unwrap_or_else(|_| "control-plane".to_owned())
//! :118  repo:    env::var_os("FLEET_QUEUE_REPO")…unwrap_or_else(|| PathBuf::from(DEFAULT_REPO))
//! :328  .current_dir(&config.repo)          // the `br ready` child
//! :623  "NUDGE_VERIFIED session={} pane={} bead={} transition={reason}"
//! ```
//!
//! Two independent bindings. `FLEET_SESSION` selects WHICH PANES are classified;
//! `FLEET_QUEUE_REPO` selects WHICH TRACKER is read — and when unset it falls back to an
//! absolute path naming a repository that has nothing to do with the session. Because the
//! child's cwd is then SET from that fallback, a lane that prefixes its command with a `cd`
//! into the right checkout is overridden: that is why `cd` provably did not bind the queue,
//! and the answer the bead recorded as UNMEASURED. `br` itself is cwd-faithful (measured:
//! three different checkouts return three different trackers, and a non-tracker directory
//! returns `NOT_INITIALIZED`), so the tracker was never the thing that lost the binding.
//!
//! The consequence was a real dispatch of a foreign repository's bead into a live pane,
//! receipted `NUDGE_VERIFIED` — a correct payload delivered against a stale binding, whose
//! own receipt could not distinguish the two.
//!
//! # What this kernel changes
//!
//! 1. **There is no second binding to get wrong.** [`TrackerBinding`] is the only value a
//!    dispatch can be built from, and it carries the session, the repository, and that
//!    repository's tracker prefix together. Constructing one REFUSES when the repository is
//!    not the one the session names ([`BindRefusal::SessionQueueMismatch`]) — so the upstream
//!    shape, session `omp-orchestrator` against a foreign absolute default, cannot be built at
//!    all. There is no fallback repository: an unnamed one is a refusal, never a default.
//! 2. **The queue is checked against the binding it was read under.** Even if a reader hands
//!    us rows from somewhere else, every candidate must carry the bound tracker's prefix or
//!    the whole tick is [`Decision::Refused`] naming BOTH the bead and the tracker. Filtering
//!    the foreign rows out silently would hide exactly the misroute that was measured, so the
//!    tick fails closed instead.
//! 3. **`NUDGE_VERIFIED` names its binding.** [`VerifiedNudge`] has ONE constructor,
//!    [`Dispatch::verified`], so the marker cannot be printed for anything but a reconciled
//!    dispatch, and [`VerifiedNudge::receipt`] carries `repo=` and `tracker=` — the two fields
//!    whose absence made the original receipt unfalsifiable.
//! 4. **An empty queue is an outcome, not a pass.** [`Decision::NothingToDispatch`] carries
//!    the population it observed, and an unreadable queue is [`Decision::Unobservable`] —
//!    never folded into "nothing to do".
//!
//! # What this kernel deliberately is NOT
//!
//! It spawns nothing. No `br`, no `tmux`, no `ntm`. The transport, the pane classification and
//! the idle proof stay where they already live; this crate owns the one decision that was
//! wrong. That keeps the policy testable without a fleet, and keeps this crate out of the
//! kernel-bypass registry it would otherwise have to join.
//!
//! It is also STRICTER than [`session_repo`]'s one known alias case elsewhere in the fleet
//! (`fast_dispatch::session_repo_dir` maps one session to a differently-named checkout). This
//! crate deliberately holds NO alias map: a second copy of that map is a second policy, and
//! the failure direction of having none is a LOUD refusal naming both sides, against the
//! silent misroute that having a wrong one produced.

use std::fmt;
use std::path::{Path, PathBuf};

/// The reconciled binding: a session, the repository its panes work in, and that
/// repository's tracker prefix. Built only by [`TrackerBinding::reconcile`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerBinding {
    session: String,
    repo: PathBuf,
    tracker: String,
}

/// Why a binding could not be built. Every variant names both sides, because a refusal that
/// does not say what it was reconciling against cannot be acted on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindRefusal {
    /// No session was supplied. The upstream default (`"control-plane"`) is precisely the
    /// shape that let a lane run for weeks against a session nobody chose.
    UnnamedSession,
    /// The queue repository has no final path component to reconcile.
    UnnamedRepo { repo: PathBuf },
    /// The queue repository is not the one the session names. THE MEASURED DEFECT.
    SessionQueueMismatch {
        session: String,
        repo: PathBuf,
        repo_name: String,
    },
    /// The repository declares no tracker prefix, so no bead id can be checked against it.
    /// Absence is a refusal: guessing a prefix here would re-open the hole.
    TrackerUnresolved { repo: PathBuf },
}

impl fmt::Display for BindRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnnamedSession => f.write_str(
                "BIND_REFUSED_UNNAMED_SESSION: no session was named and there is no default session",
            ),
            Self::UnnamedRepo { repo } => write!(
                f,
                "BIND_REFUSED_UNNAMED_REPO repo={}: the queue repository has no name to reconcile",
                repo.display()
            ),
            Self::SessionQueueMismatch {
                session,
                repo,
                repo_name,
            } => write!(
                f,
                "BIND_REFUSED_SESSION_QUEUE_MISMATCH session={session} repo={} repo_name={repo_name}: \
                 the queue repository is not the one the session names, so a nudge would carry \
                 another project's work",
                repo.display()
            ),
            Self::TrackerUnresolved { repo } => write!(
                f,
                "BIND_REFUSED_TRACKER_UNRESOLVED repo={}: the repository declares no tracker prefix",
                repo.display()
            ),
        }
    }
}

impl TrackerBinding {
    /// Reconcile a session with the repository its queue is read from.
    ///
    /// `tracker` is the repository's OWN declared issue prefix (see [`parse_issue_prefix`]);
    /// it is an input rather than a lookup so this function stays free of I/O.
    pub fn reconcile(
        session: &str,
        repo: &Path,
        tracker: Option<&str>,
    ) -> Result<Self, BindRefusal> {
        let session = session.trim();
        if session.is_empty() {
            return Err(BindRefusal::UnnamedSession);
        }
        let Some(repo_name) = repo.file_name().and_then(|name| name.to_str()) else {
            return Err(BindRefusal::UnnamedRepo {
                repo: repo.to_path_buf(),
            });
        };
        if repo_name != session {
            return Err(BindRefusal::SessionQueueMismatch {
                session: session.to_owned(),
                repo: repo.to_path_buf(),
                repo_name: repo_name.to_owned(),
            });
        }
        let tracker = tracker.map(str::trim).filter(|value| !value.is_empty());
        let Some(tracker) = tracker else {
            return Err(BindRefusal::TrackerUnresolved {
                repo: repo.to_path_buf(),
            });
        };
        Ok(Self {
            session: session.to_owned(),
            repo: repo.to_path_buf(),
            tracker: tracker.to_owned(),
        })
    }

    pub fn session(&self) -> &str {
        &self.session
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    pub fn tracker(&self) -> &str {
        &self.tracker
    }

    /// Does this bead id belong to the bound tracker?
    ///
    /// The separator is required: tracker `omp` must not adopt `omperator-1`.
    pub fn owns(&self, bead: &str) -> bool {
        bead.len() > self.tracker.len() + 1
            && bead.starts_with(&self.tracker)
            && bead.as_bytes()[self.tracker.len()] == b'-'
    }
}

/// The repository a session's panes work in: `<developer_root>/<session>`.
///
/// Exposed so a caller never has to write a checkout path down. There is no fallback for an
/// empty session — the caller must refuse first.
pub fn session_repo(developer_root: &Path, session: &str) -> PathBuf {
    developer_root.join(session.trim())
}

/// The tracker prefix declared by a repository's tracker configuration.
///
/// The live file writes the field inside a comment (`# issue_prefix: omp-orchestrator`), so
/// both the commented and uncommented forms are accepted. Absence returns `None`; this
/// function never invents a prefix.
pub fn parse_issue_prefix(config_text: &str) -> Option<String> {
    for line in config_text.lines() {
        let line = line.trim().trim_start_matches('#').trim();
        let Some(value) = line.strip_prefix("issue_prefix:") else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    None
}

/// One row of the ready queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueEntry {
    pub id: String,
}

/// Why the queue could not be read as a queue. NEVER an empty queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueUnreadable {
    /// The payload is not JSON at all.
    NotJson { detail: String },
    /// The tracker answered with its own error envelope (`{"error":{"code":…}}`) — the shape
    /// returned by a directory that is not a checkout.
    TrackerError { code: String, message: String },
    /// JSON, but not a queue: neither a bare array nor `{"issues":[…]}`.
    NotAQueue { shape: String },
    /// A row carried no usable `id`. A queue we cannot fully name is not a queue we may
    /// dispatch from.
    RowWithoutId { index: usize },
}

impl fmt::Display for QueueUnreadable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotJson { detail } => {
                write!(f, "QUEUE_UNOBSERVABLE_NOT_JSON: {detail}")
            }
            Self::TrackerError { code, message } => {
                write!(f, "QUEUE_UNOBSERVABLE_TRACKER_ERROR code={code}: {message}")
            }
            Self::NotAQueue { shape } => {
                write!(f, "QUEUE_UNOBSERVABLE_NOT_A_QUEUE shape={shape}")
            }
            Self::RowWithoutId { index } => {
                write!(f, "QUEUE_UNOBSERVABLE_ROW_WITHOUT_ID index={index}")
            }
        }
    }
}

/// Parse a ready-queue payload.
///
/// THREE SHAPES ARE REAL and all three are handled, because reading only one of them is how a
/// queue reader prints nothing and reads as empty:
///
/// * a BARE ARRAY — what the tracker returns today for `ready --json --limit 0`;
/// * `{"issues":[…]}` — the projection other readers in this workspace parse;
/// * `{"error":{"code":…,"message":…}}` — a directory that is not a checkout.
pub fn parse_ready_queue(payload: &str) -> Result<Vec<QueueEntry>, QueueUnreadable> {
    let value: serde_json::Value =
        serde_json::from_str(payload.trim()).map_err(|error| QueueUnreadable::NotJson {
            detail: error.to_string(),
        })?;

    let rows = if let Some(rows) = value.as_array() {
        rows
    } else if let Some(error) = value.get("error") {
        let field = |name: &str| {
            error
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        return Err(QueueUnreadable::TrackerError {
            code: field("code"),
            message: field("message"),
        });
    } else if let Some(rows) = value.get("issues").and_then(serde_json::Value::as_array) {
        rows
    } else {
        let shape = match &value {
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
                keys.sort_unstable();
                format!("object({})", keys.join(","))
            }
            other => format!("{}", json_kind(other)),
        };
        return Err(QueueUnreadable::NotAQueue { shape });
    };

    let mut entries = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let id = row
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let Some(id) = id else {
            return Err(QueueUnreadable::RowWithoutId { index });
        };
        entries.push(QueueEntry { id: id.to_owned() });
    }
    Ok(entries)
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// A dispatch that has been reconciled against its binding. The only thing that can produce a
/// [`VerifiedNudge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatch {
    binding: TrackerBinding,
    bead: String,
    population: usize,
}

impl Dispatch {
    pub fn binding(&self) -> &TrackerBinding {
        &self.binding
    }

    pub fn bead(&self) -> &str {
        &self.bead
    }

    pub fn population(&self) -> usize {
        self.population
    }

    /// The proposal line. Carries the binding so a reader can check the routing without
    /// trusting the sender.
    pub fn proposal(&self) -> String {
        format!(
            "DISPATCH_BOUND session={} repo={} tracker={} bead={} population={}",
            self.binding.session,
            self.binding.repo.display(),
            self.binding.tracker,
            self.bead,
            self.population
        )
    }

    /// Record a verified pane transition. THE ONLY constructor of [`VerifiedNudge`]: a
    /// `NUDGE_VERIFIED` marker is therefore unreachable without a reconciled dispatch.
    pub fn verified(self, pane: &str, transition: &str) -> VerifiedNudge {
        VerifiedNudge {
            dispatch: self,
            pane: pane.to_owned(),
            transition: transition.to_owned(),
        }
    }
}

/// Why a reconciled dispatch was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchRefusal {
    /// A queue row does not belong to the bound tracker. The whole tick is refused rather
    /// than the row filtered: a queue holding foreign work is not the bound tracker's queue,
    /// and quietly dropping the row would hide the misroute instead of reporting it.
    ForeignTracker {
        bead: String,
        session: String,
        repo: PathBuf,
        tracker: String,
        population: usize,
    },
}

impl fmt::Display for DispatchRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignTracker {
                bead,
                session,
                repo,
                tracker,
                population,
            } => write!(
                f,
                "DISPATCH_REFUSED_FOREIGN_TRACKER bead={bead} tracker={tracker} session={session} \
                 repo={} population={population}: the bead does not belong to the bound tracker",
                repo.display()
            ),
        }
    }
}

/// The outcome of one idle-nudge tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Dispatch(Dispatch),
    Refused(DispatchRefusal),
    /// The queue was read and held nothing dispatchable. Carries the population so the line
    /// cannot be confused with a tick that dispatched.
    NothingToDispatch { population: usize },
    /// The queue could not be read. NOT empty.
    Unobservable(QueueUnreadable),
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch(dispatch) => f.write_str(&dispatch.proposal()),
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::NothingToDispatch { population } => {
                write!(f, "NOTHING_TO_DISPATCH population={population}")
            }
            Self::Unobservable(reason) => write!(f, "{reason}"),
        }
    }
}

/// Decide one tick.
///
/// Takes the PARSE RESULT, not a vector: an unreadable queue must not be expressible as an
/// empty one at this boundary.
pub fn decide(
    binding: &TrackerBinding,
    queue: Result<Vec<QueueEntry>, QueueUnreadable>,
) -> Decision {
    let entries = match queue {
        Ok(entries) => entries,
        Err(reason) => return Decision::Unobservable(reason),
    };
    let population = entries.len();
    for entry in &entries {
        if !binding.owns(&entry.id) {
            return Decision::Refused(DispatchRefusal::ForeignTracker {
                bead: entry.id.clone(),
                session: binding.session.clone(),
                repo: binding.repo.clone(),
                tracker: binding.tracker.clone(),
                population,
            });
        }
    }
    match entries.first() {
        Some(entry) => Decision::Dispatch(Dispatch {
            binding: binding.clone(),
            bead: entry.id.clone(),
            population,
        }),
        None => Decision::NothingToDispatch { population },
    }
}

/// One whole tick, from the two raw texts the CLI reads to the outcome it prints.
///
/// This exists so the CLI holds NO decision of its own. The measured defect lived in exactly
/// this glue — a session read from one place and a queue read from another, with nothing
/// between them — so leaving the glue untested would leave the defect's own habitat untested.
pub fn tick(
    session: &str,
    repo: &Path,
    tracker_config: &str,
    queue_payload: &str,
) -> Result<Decision, BindRefusal> {
    let tracker = parse_issue_prefix(tracker_config);
    let binding = TrackerBinding::reconcile(session, repo, tracker.as_deref())?;
    Ok(decide(&binding, parse_ready_queue(queue_payload)))
}

/// A `NUDGE_VERIFIED` receipt. Constructible only from a reconciled [`Dispatch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedNudge {
    dispatch: Dispatch,
    pane: String,
    transition: String,
}

impl VerifiedNudge {
    /// The receipt line. `repo=` and `tracker=` are the fields whose absence made the
    /// original marker unable to tell a correct delivery from a misrouted one.
    pub fn receipt(&self) -> String {
        format!(
            "NUDGE_VERIFIED session={} repo={} tracker={} pane={} bead={} transition={}",
            self.dispatch.binding.session,
            self.dispatch.binding.repo.display(),
            self.dispatch.binding.tracker,
            self.pane,
            self.dispatch.bead,
            self.transition
        )
    }

    pub fn dispatch(&self) -> &Dispatch {
        &self.dispatch
    }
}

/// The exit lattice, as a decision over the outcome.
///
/// Every value here is an already-documented row of `docs/error_codes/exit_code_registry.md`;
/// this crate introduces no new code, because that registry is not this crate's to edit.
///
/// * `0`   `XC-000` — a dispatch was bound and proposed.
/// * `3`   `XC-003` TRACKER_ERROR — the queue carried work from another tracker.
/// * `69`  `XC-069` EX_UNAVAILABLE — the queue could not be observed.
/// * `70`  `XC-070` EX_SOFTWARE — a tick carrying EMPTY evidence, refused rather than
///   allowed to pass as `0`. That is exactly what an empty ready-queue is, and it is why
///   NOTHING_TO_DISPATCH does not share `0` with a real dispatch.
/// * `78`  `XC-078` EX_CONFIG — the invocation bound a session to the wrong repository.
pub fn exit_code(decision: &Decision) -> u8 {
    match decision {
        Decision::Dispatch(_) => 0,
        Decision::Refused(_) => 3,
        Decision::NothingToDispatch { .. } => 70,
        Decision::Unobservable(_) => 69,
    }
}

/// The exit code for a binding that could not be built: a configuration fault, `XC-078`.
pub fn bind_exit_code(_refusal: &BindRefusal) -> u8 {
    78
}
