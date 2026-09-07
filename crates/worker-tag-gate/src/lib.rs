//! Refuses an rch `workers.toml` in which a **non-darwin** worker carries the `os:darwin` tag.
//!
//! # Why this gate exists
//!
//! Joshua, 2026-09-07, verbatim: *"we should never be able to tag any of our boxes darwin, it
//! breaks our systems."*
//!
//! In rch, **a worker declaring an OS is RESTRICTED to commands targeting that OS.** So an
//! `os:darwin` tag on a Linux box does not add cross-compile capability — it makes that box
//! **refuse every native build**. Measured the same day on `contabo-3`, omp-orchestrator's own
//! assigned lane:
//!
//! ```text
//! [RCH-I001] requested worker set [contabo-3] refused (unavailable);
//! 'contabo-3' failed worker admission (worker declares os=darwin,
//!  so it takes only commands targeting that OS)
//! ```
//!
//! The observable consequence was that this repo's verdict builds scattered onto **other repos'**
//! assigned lanes (`contabo-1` is zeststream-cast's, `contabo-2` is control-plane's) — the lane
//! guard reported `OBSERVED-FOREIGN` — while two Darwin cross-compiles sat wedged with frozen CPU
//! time, holding slots.
//!
//! This also encodes Joshua's standing decision `HD-0013`: *"NO macOS SDK on the Linux lane. Split
//! lanes by TARGET instead of cross-linking: contabo for Rust/Linux compilation, the LOCAL MAC for
//! darwin builds."*
//!
//! # What a green run does NOT establish
//!
//! * **Tag hygiene, never lane health.** A correctly-tagged fleet can still be saturated, wedged,
//!   or out of disk. This gate reads one file; it never contacts a worker.
//! * **Nothing about `--config build.target`.** That form deliberately hides the target from rch
//!   (`required_os=none`), so it is invisible here. A gate on the *command* is a separate,
//!   unbuilt check.
//! * **The file on disk, not the daemon's belief.** `rchd` caches worker config; a clean file does
//!   not prove the running daemon agrees. Re-read after any edit.

/// A worker row as this gate understands it: an id, its tags, and whether it is enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRow {
    /// The worker id, e.g. `contabo-3`.
    pub id: String,
    /// Every tag on the row, verbatim and in file order.
    pub tags: Vec<String>,
    /// `enabled = true`? Absent is treated as `false`.
    pub enabled: bool,
}

impl WorkerRow {
    /// The tag that restricts a worker to darwin-targeted commands.
    pub const OS_DARWIN: &'static str = "os:darwin";

    /// Tags that mark a row as a genuine macOS host. Only these may carry [`Self::OS_DARWIN`].
    pub const DARWIN_HOST_TAGS: &'static [&'static str] = &["macos", "darwin", "aarch64-apple"];

    /// Does this row declare `os:darwin`?
    #[must_use]
    pub fn declares_os_darwin(&self) -> bool {
        self.tags.iter().any(|t| t == Self::OS_DARWIN)
    }

    /// Is this row a real macOS host, and therefore allowed to declare `os:darwin`?
    #[must_use]
    pub fn is_darwin_host(&self) -> bool {
        self.tags
            .iter()
            .any(|t| Self::DARWIN_HOST_TAGS.contains(&t.as_str()))
    }

    /// Is this row a Linux host?
    #[must_use]
    pub fn is_linux_host(&self) -> bool {
        self.tags.iter().any(|t| t == "linux")
    }
}

/// Why a scan refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateError {
    /// The workers file could not be read. **Not** a pass — the subject was never examined.
    Unreadable {
        /// The path we tried.
        path: String,
        /// The OS error text.
        detail: String,
    },
    /// The file parsed but yielded zero worker rows. Anti-vacuity: an empty scan set is an ERROR.
    NoWorkers {
        /// The path we read.
        path: String,
    },
    /// One or more non-darwin workers declare `os:darwin`.
    DarwinTagOnNonDarwinHost {
        /// Every offending row, in file order.
        offenders: Vec<WorkerRow>,
    },
}

impl GateError {
    /// A stable machine token, distinct per cause.
    ///
    /// Distinct codes matter: `AGENTS.md` rule 7 requires a known-bad leg to pin the **message and
    /// the exit code**, because pinning either alone is defeasible in one direction.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unreadable { .. } => "WORKERS_TOML_UNREADABLE",
            Self::NoWorkers { .. } => "WORKERS_TOML_EMPTY",
            Self::DarwinTagOnNonDarwinHost { .. } => "OS_DARWIN_ON_NON_DARWIN_HOST",
        }
    }

    /// The process exit code for this cause. Each cause gets its own, never a shared `1`.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::DarwinTagOnNonDarwinHost { .. } => 2,
            Self::Unreadable { .. } => 3,
            Self::NoWorkers { .. } => 4,
        }
    }
}

impl core::fmt::Display for GateError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unreadable { path, detail } => write!(
                f,
                "{}: cannot read {path}: {detail} — this is UNKNOWN, not a pass; the file was never examined",
                self.code()
            ),
            Self::NoWorkers { path } => write!(
                f,
                "{}: {path} parsed but declared ZERO workers — an empty scan set is an ERROR, never a pass",
                self.code()
            ),
            Self::DarwinTagOnNonDarwinHost { offenders } => {
                writeln!(
                    f,
                    "{}: {} non-darwin worker(s) declare `{}`, which makes them REFUSE EVERY NATIVE BUILD",
                    self.code(),
                    offenders.len(),
                    WorkerRow::OS_DARWIN
                )?;
                for row in offenders {
                    writeln!(
                        f,
                        "  {} tags={:?} enabled={}",
                        row.id, row.tags, row.enabled
                    )?;
                }
                write!(
                    f,
                    "  rch restricts a worker declaring an OS to commands targeting that OS, so the tag \
                     subtracts capability rather than adding it. Measured 2026-09-07 on contabo-3: \
                     `RCH-I001 ... worker declares os=darwin, so it takes only commands targeting that OS`. \
                     Remove the tag; darwin artifacts belong on the local Mac per HD-0013."
                )
            }
        }
    }
}

/// Parse the worker rows out of an rch `workers.toml` body.
///
/// Deliberately a small hand parser rather than a toml dependency: the gate must run in a
/// pre-commit hook with no network and the shape it needs is three fields.
///
/// **Comments are stripped before matching.** `AGENTS.md` records a census going green because its
/// needle matched inside a doc comment two functions above the code — over-stripping can only
/// report LESS, and the failure this prevents is a false GREEN.
#[must_use]
pub fn parse_workers(body: &str) -> Vec<WorkerRow> {
    let mut rows: Vec<WorkerRow> = Vec::new();
    let mut current: Option<WorkerRow> = None;

    for raw in body.lines() {
        let line = match raw.find('#') {
            Some(idx) => &raw[..idx],
            None => raw,
        }
        .trim();

        if line.starts_with("[[") {
            if let Some(row) = current.take() {
                if !row.id.is_empty() {
                    rows.push(row);
                }
            }
            current = Some(WorkerRow {
                id: String::new(),
                tags: Vec::new(),
                enabled: false,
            });
            continue;
        }

        let Some(row) = current.as_mut() else {
            continue;
        };

        if let Some(value) = field(line, "id").or_else(|| field(line, "name")) {
            if row.id.is_empty() {
                row.id = value;
            }
        } else if let Some(list) = line.strip_prefix("tags") {
            if let Some(open) = list.find('[') {
                if let Some(close) = list.find(']') {
                    if close > open {
                        row.tags = list[open + 1..close]
                            .split(',')
                            .map(|t| t.trim().trim_matches('"').to_owned())
                            .filter(|t| !t.is_empty())
                            .collect();
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("enabled") {
            row.enabled = rest.trim_start_matches(['=', ' ']).starts_with("true");
        }
    }

    if let Some(row) = current.take() {
        if !row.id.is_empty() {
            rows.push(row);
        }
    }

    rows
}

fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let inner = rest.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_owned())
}

/// Refuse any non-darwin worker declaring `os:darwin`.
///
/// Returns the rows that were scanned on success, so a caller can report a NONZERO denominator —
/// a gate that scanned nothing and a gate that passed must never read identically.
pub fn check(body: &str, path: &str) -> Result<Vec<WorkerRow>, GateError> {
    let rows = parse_workers(body);
    if rows.is_empty() {
        return Err(GateError::NoWorkers {
            path: path.to_owned(),
        });
    }

    let offenders: Vec<WorkerRow> = rows
        .iter()
        .filter(|row| row.declares_os_darwin() && !row.is_darwin_host())
        .cloned()
        .collect();

    if offenders.is_empty() {
        Ok(rows)
    } else {
        Err(GateError::DarwinTagOnNonDarwinHost { offenders })
    }
}
