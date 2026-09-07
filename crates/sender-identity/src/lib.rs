#![forbid(unsafe_code)]

//! **Who this process signs mail as, and whether that identity is its own to use.**
//!
//! # The defect, measured 2026-09-02
//!
//! The supervisor's Agent Mail identity was AMBIENT: it read `AGENT_NAME` from launchd's
//! global environment, which carried whatever agent last ran `launchctl setenv AGENT_NAME`
//! on this Mac. Today that was `WildStone`, registered in `~/Developer/fsw`, so the daemon
//! correctly refused every send:
//!
//! ```text
//! DISPATCH_RESULT_MAIL_DEGRADED     70
//! DISPATCH_RESULT_NOTIFY_DEGRADED    2
//! DISPATCH_RESULT_RECORDED         101
//!   of the 70:  "Agent 'WildStone' not found in project '…/omp-orchestrator'"   64
//!
//! falsifier: messages with subject 'dispatch result:%' from a real dispatch  ->  0
//! launchctl getenv AGENT_NAME  ->  AzureCrane      (a THIRD foreign identity)
//! plist EnvironmentVariables   ->  13 keys, none of them AGENT_MAIL_AGENT
//! ```
//!
//! The bead recorded 23 refusals and 21 `WildStone`; by the time it was worked the counts
//! were **70 and 64**. The defect was live and accumulating, and every surface reported the
//! tick healthy (`SUPERVISED_WORKING working=3 ready=63`).
//!
//! # THE RULE, and why it needs no launchd detection
//!
//! `7n5b` covered the UNSET case and got it. It did not anticipate SET-BUT-FOREIGN, which
//! is the case that actually ran, because the resolution only tested for emptiness.
//!
//! An earlier design here tried to detect "am I launchd-managed" so it could ignore
//! `AGENT_NAME` only in that case. That is the wrong axis. **A variable set by
//! `launchctl setenv` is machine-global; a project-scoped identity is repo-local. An
//! ambient variable therefore can NEVER be a valid repo-scoped identity, whoever spawned
//! the process.** So the variable list is TYPED — [`IdentitySource::Owned`] vs
//! [`IdentitySource::Ambient`] — and an ambient value is refused by NAME, with the value
//! quoted, before any send is attempted. No launchd probe, no `XPC_SERVICE_NAME` guess, and
//! the refusal is identical under launchd, tmux and a bare shell.
//!
//! # What this crate does NOT do
//!
//! It does not register anything: registration is I/O and belongs to the caller. This crate
//! decides, from a value plus a registration answer, whether the identity may be used — so
//! the decision is testable without a daemon.

use std::collections::BTreeSet;
use std::fmt;

/// How many consecutive refused sends make the durable half DEAD rather than degraded.
///
/// **N = 3, declared here as the bead requires (`<= 3`).** Two is a coincidence a retry can
/// explain; three consecutive refusals for the SAME sender is a configuration fact. The
/// measured run produced 64 in a row, so any N in range would have fired — the value is
/// chosen for the smallest honest sample, not to be reachable.
pub const DEAD_AFTER_CONSECUTIVE_REFUSALS: u32 = 3;

const _: () = assert!(
    DEAD_AFTER_CONSECUTIVE_REFUSALS <= 3 && DEAD_AFTER_CONSECUTIVE_REFUSALS >= 2,
    "N must be <= 3 per the bead, and >= 2 so one transient refusal is not a verdict"
);

/// Where an identity value came from, and whether that origin can be trusted to be
/// repo-scoped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// Set deliberately for THIS process by whoever configured it — a plist
    /// `EnvironmentVariables` entry, a wrapper, or an operator's shell. Repo-scoped by
    /// construction: nobody else's `setenv` can reach it.
    Owned(&'static str),
    /// Machine-global, set by `launchctl setenv` or a login shell and inherited by every
    /// process on the Mac. **Never a valid project identity**, because its value is
    /// whatever another project's agent last wrote.
    Ambient(&'static str),
}

impl IdentitySource {
    #[must_use]
    pub fn var(self) -> &'static str {
        match self {
            IdentitySource::Owned(name) | IdentitySource::Ambient(name) => name,
        }
    }

    #[must_use]
    pub fn is_ambient(self) -> bool {
        matches!(self, IdentitySource::Ambient(_))
    }
}

/// The variables consulted, in order, with their trust class.
///
/// `AGENT_NAME` stays in the list DELIBERATELY rather than being deleted. Removing it would
/// make an ambient value invisible — the process would report `sender_identity_unset` while
/// a stale `WildStone` sat in the environment, and the next reader would have no idea why.
/// Keeping it typed makes the leak NAMED.
pub const MAIL_IDENTITY_VARS: [IdentitySource; 3] = [
    IdentitySource::Owned("AGENT_MAIL_AGENT"),
    IdentitySource::Owned("OMP_MAIL_AGENT"),
    IdentitySource::Ambient("AGENT_NAME"),
];

/// What the registry says about a candidate identity, for THIS project.
///
/// Supplied by the caller, because asking is I/O. [`Registration::Unverified`] exists so a
/// caller that could not ask cannot accidentally present that as `Registered` — an
/// unverifiable identity is refused, not assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registration {
    /// Registered in this project. The only value that permits a send.
    Registered,
    /// Registered, but in a DIFFERENT project — the measured `WildStone` case.
    ForeignProject(String),
    /// Not registered anywhere the caller can see.
    Absent,
    /// The caller could not ask. Fail closed.
    Unverified(String),
}

/// Why this process may not sign mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderRefusal {
    /// No identity variable held a value.
    Unset { searched: Vec<&'static str> },
    /// The only value found came from an ambient variable. **The measured defect.**
    AmbientSource {
        var: &'static str,
        value: String,
        project: String,
    },
    /// An owned variable named an agent registered to another project.
    ForeignRegistration {
        var: &'static str,
        value: String,
        project: String,
        registered_in: String,
    },
    /// An owned variable named an agent nobody has registered.
    Unregistered {
        var: &'static str,
        value: String,
        project: String,
    },
    /// Registration could not be checked, so use is not permitted.
    Unverifiable {
        var: &'static str,
        value: String,
        project: String,
        detail: String,
    },
}

impl SenderRefusal {
    /// The stable machine label. Every one is a `SUPERVISOR_REFUSED` class.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            SenderRefusal::Unset { .. } => "SENDER_IDENTITY_UNSET",
            SenderRefusal::AmbientSource { .. } => "SENDER_IDENTITY_AMBIENT",
            SenderRefusal::ForeignRegistration { .. } => "SENDER_IDENTITY_FOREIGN",
            SenderRefusal::Unregistered { .. } => "SENDER_IDENTITY_UNREGISTERED",
            SenderRefusal::Unverifiable { .. } => "SENDER_IDENTITY_UNVERIFIABLE",
        }
    }
}

impl fmt::Display for SenderRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SenderRefusal::Unset { searched } => write!(
                f,
                "SUPERVISOR_REFUSED {} searched={} \
                 next_action=set-AGENT_MAIL_AGENT-in-the-plist-and-register-it",
                self.label(),
                searched.join(",")
            ),
            SenderRefusal::AmbientSource {
                var,
                value,
                project,
            } => write!(
                f,
                "SUPERVISOR_REFUSED {} var={var} value={value} project={project} \
                 detail=\"{var} is machine-global: its value is whatever agent last ran \
                 `launchctl setenv {var}`, so it cannot be a project-scoped identity. \
                 Measured 2026-09-02: it carried WildStone (project ~/Developer/fsw) and 64 \
                 sends were refused before anything noticed.\" \
                 next_action=set-AGENT_MAIL_AGENT-in-the-plist-and-register-it",
                self.label()
            ),
            SenderRefusal::ForeignRegistration {
                var,
                value,
                project,
                registered_in,
            } => write!(
                f,
                "SUPERVISOR_REFUSED {} var={var} sender={value} project={project} \
                 registered_in={registered_in} \
                 detail=\"this agent belongs to another project; the daemon will refuse every \
                 send and the refusal arrives one dispatch too late\" \
                 next_action=register-this-sender-in-this-project-or-name-a-different-one",
                self.label()
            ),
            SenderRefusal::Unregistered {
                var,
                value,
                project,
            } => write!(
                f,
                "SUPERVISOR_REFUSED {} var={var} sender={value} project={project} \
                 detail=\"no such agent in any project the registry can see\" \
                 next_action=register_agent-then-whois-read-back",
                self.label()
            ),
            SenderRefusal::Unverifiable {
                var,
                value,
                project,
                detail,
            } => write!(
                f,
                "SUPERVISOR_REFUSED {} var={var} sender={value} project={project} \
                 detail=\"{detail}; an unverifiable identity is refused, never assumed\" \
                 next_action=repair-the-registry-then-retry",
                self.label()
            ),
        }
    }
}

/// One candidate: a variable and the value it held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub source: IdentitySource,
    pub value: String,
}

/// Pick the candidate the resolution would use, from a lookup over the typed var list.
///
/// The FIRST non-empty value wins, in list order — owned before ambient — so an ambient
/// value is only ever reached when no owned variable was set. That ordering is what makes
/// the ambient refusal a diagnosis of a MISSING configuration rather than a veto of a
/// present one.
#[must_use]
pub fn first_candidate(lookup: &dyn Fn(&str) -> Option<String>) -> Option<Candidate> {
    for source in MAIL_IDENTITY_VARS {
        if let Some(value) = lookup(source.var()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(Candidate {
                    source,
                    value: trimmed.to_owned(),
                });
            }
        }
    }
    None
}

/// Decide whether this process may sign mail, and as whom.
///
/// `registration_of` is the caller's window onto the registry — `whois`, or a direct read.
/// It is only consulted for an OWNED candidate: an ambient value is refused before any
/// lookup, because asking whether a machine-global name happens to be registered here
/// invites accepting it when it accidentally is.
pub fn resolve_sender(
    project: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
    registration_of: &dyn Fn(&str) -> Registration,
) -> Result<String, SenderRefusal> {
    let Some(candidate) = first_candidate(lookup) else {
        return Err(SenderRefusal::Unset {
            searched: MAIL_IDENTITY_VARS.iter().map(|s| s.var()).collect(),
        });
    };
    let var = candidate.source.var();
    if candidate.source.is_ambient() {
        return Err(SenderRefusal::AmbientSource {
            var,
            value: candidate.value,
            project: project.to_owned(),
        });
    }
    match registration_of(&candidate.value) {
        Registration::Registered => Ok(candidate.value),
        Registration::ForeignProject(registered_in) => Err(SenderRefusal::ForeignRegistration {
            var,
            value: candidate.value,
            project: project.to_owned(),
            registered_in,
        }),
        Registration::Absent => Err(SenderRefusal::Unregistered {
            var,
            value: candidate.value,
            project: project.to_owned(),
        }),
        Registration::Unverified(detail) => Err(SenderRefusal::Unverifiable {
            var,
            value: candidate.value,
            project: project.to_owned(),
            detail,
        }),
    }
}

/// The consumer for `DISPATCH_RESULT_MAIL_DEGRADED` rows.
///
/// # Why a consumer is the third acceptance and not a nicety
///
/// The mechanism reported itself honestly — the DEGRADED row is typed and names the sender —
/// and **nothing read it.** 21 lines in the log, then 64, with no bead, no refusal to
/// dispatch, and no repair. Meanwhile the ledger's own status line still said
/// `SUPERVISED_WORKING working=3 ready=63`, so a reader of the ledger had to grep to learn
/// that the durable half of S5 logging was dead.
///
/// So after [`DEAD_AFTER_CONSECUTIVE_REFUSALS`] consecutive refusals for one sender the
/// supervisor STOPS CLAIMING the durable half exists: the status becomes a named degraded
/// value carrying the sender, visible without grepping.
#[derive(Debug, Clone, Default)]
pub struct MailRefusalTally {
    sender: Option<String>,
    consecutive: u32,
}

impl MailRefusalTally {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one refused send for `sender`. A DIFFERENT sender resets the run: the claim
    /// is about one identity being wrong, and a new identity is a new question.
    pub fn refused(&mut self, sender: &str) {
        if self.sender.as_deref() == Some(sender) {
            self.consecutive = self.consecutive.saturating_add(1);
        } else {
            self.sender = Some(sender.to_owned());
            self.consecutive = 1;
        }
    }

    /// Record one send that landed. Clears the run — the durable half demonstrably works.
    pub fn landed(&mut self) {
        self.sender = None;
        self.consecutive = 0;
    }

    #[must_use]
    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }

    /// The status this tick must report INSTEAD of `SUPERVISED_WORKING`, if any.
    ///
    /// `None` means the healthy status stands. A caller that ignores `Some` is back to the
    /// measured state, so the return is `#[must_use]`.
    #[must_use]
    pub fn degraded_status(&self) -> Option<String> {
        let sender = self.sender.as_deref()?;
        if self.consecutive < DEAD_AFTER_CONSECUTIVE_REFUSALS {
            return None;
        }
        Some(format!(
            "SUPERVISED_WORKING_MAIL_DEAD sender={sender} consecutive_refusals={} \
             detail=\"the durable dispatch-result half of S5 logging is NOT working; this \
             tick does not claim it does\" next_action=repair-the-sender-identity",
            self.consecutive
        ))
    }
}

/// Every distinct sender named in a set of refusal details, for a report that does not
/// require the reader to grep.
///
/// Keyed on the daemon's own message shape, `Agent 'NAME' not found`, which is the string
/// the 64 measured rows carried.
#[must_use]
pub fn senders_in_refusals<S: AsRef<str>>(details: &[S]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for detail in details {
        let text = detail.as_ref();
        let mut rest = text;
        while let Some(at) = rest.find("Agent '") {
            let after = &rest[at + "Agent '".len()..];
            if let Some(end) = after.find('\'') {
                let name = &after[..end];
                if !name.is_empty() && after[end..].contains("not found") {
                    out.insert(name.to_owned());
                }
                rest = &after[end..];
            } else {
                break;
            }
        }
    }
    out
}
