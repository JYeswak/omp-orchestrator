#![forbid(unsafe_code)]

//! The ONE owner of `ntm` invocation.
//!
//! Three crates were each hand-building a `Command::new("ntm")`: ompo-doctor's
//! cass-context spawn (the single site the bypass ledger grants an amnesty to),
//! pane-dispatch-ready's agent-health probe, and fast-dispatch's dialog probe.
//! Five option-A verbs remain unadopted, so the handroll count was on a path to
//! eight. Each handroll has to re-learn the SAME FIVE PAYLOAD TRAPS, measured
//! against the live surface, and nothing checks that it did:
//!
//! 1. A not-found payload carries POPULATED, ZEROED objects, so reading a field
//!    from a failed call yields a plausible answer. Branch on exit FIRST.
//! 2. `error_code` is NOT a classifier: agent-health and interrupt emit
//!    `PANE_NOT_FOUND` while dialogs and answer-dialog emit `INVALID_FLAG` for
//!    the IDENTICAL condition. Key on the exit status.
//! 3. Motion fields are NOT monotonic. A saturated busy pane holds `lines`
//!    fixed, so a grew-the-line-count test reads it as IDLE. Compare snapshots.
//! 4. `total_panes` is the AGENT count, not the pane count: deriving a pane
//!    denominator from it silently DROPS non-agent panes (8 tmux, 7 by verb).
//! 5. `critical_count` INVERTS: a dead pane is 1, a NONEXISTENT pane is 0, so
//!    zero is not health.
//!
//! Those five live here, once, as types and functions with their own legs. The
//! spawn path delegates to `subprocess-contract`, which owns the fresh process
//! group, the concurrent drain of both pipes, and the deadline that signals the
//! GROUP rather than the leader. This crate adds no second spawn kernel.

use serde_json::Value;
use std::time::Duration;

/// The option-A robot verbs. Adding a verb here is how the sixth adoption
/// happens WITHOUT a sixth handroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtmVerb {
    AgentHealth,
    Dialogs,
    AnswerDialog,
    Interrupt,
    InspectPane,
    FleetHealth,
    Assign,
}

impl NtmVerb {
    /// The `--robot-*` flag, verbatim. One spelling, one place.
    #[must_use]
    pub fn flag(self) -> &'static str {
        match self {
            Self::AgentHealth => "--robot-agent-health",
            Self::Dialogs => "--robot-dialogs",
            Self::AnswerDialog => "--robot-answer-dialog",
            Self::Interrupt => "--robot-interrupt",
            Self::InspectPane => "--robot-inspect-pane",
            Self::FleetHealth => "--robot-fleet-health",
            Self::Assign => "--assign",
        }
    }

    /// The flag THIS verb honours for selecting a pane.
    ///
    /// Measured 2026-09-11, not assumed. `--robot-inspect-pane` documents `--inspect-index=` in
    /// `ntm --help` and SILENTLY IGNORES `--panes=`, answering `success=true, pane_index=0` for
    /// every value including a nonexistent pane. Every other verb here honours `--panes=`.
    ///
    /// One place owns both spellings, beside [`Self::flag`], because the pair is what a reader has
    /// to check: a verb whose selector differs from its neighbours is invisible at the call site.
    #[must_use]
    pub fn pane_selector_flag(self) -> &'static str {
        match self {
            Self::InspectPane => "--inspect-index",
            Self::AgentHealth
            | Self::Dialogs
            | Self::AnswerDialog
            | Self::Interrupt
            | Self::FleetHealth
            | Self::Assign => "--panes",
        }
    }
}

/// One invocation, described rather than spelled out at the call site.
#[derive(Debug, Clone)]
pub struct NtmCall {
    verb: NtmVerb,
    session: Option<String>,
    panes: Option<String>,
    /// A positional subcommand (`ntm spawn <session> …`) instead of a
    /// `--robot-*` flag. The escape hatch exists so a non-robot invocation
    /// still comes through this kernel rather than growing a fourth handroll.
    subcommand: Option<Vec<String>>,
    extra: Vec<String>,
}

impl NtmCall {
    /// A verb whose flag takes a session value (`--robot-dialogs=<session>`).
    #[must_use]
    pub fn on_session(verb: NtmVerb, session: &str) -> Self {
        Self {
            verb,
            session: Some(session.to_owned()),
            subcommand: None,
            panes: None,
            extra: Vec::new(),
        }
    }

    /// A verb with no session value (`--assign`).
    #[must_use]
    pub fn bare(verb: NtmVerb) -> Self {
        Self {
            verb,
            session: None,
            subcommand: None,
            panes: None,
            extra: Vec::new(),
        }
    }

    /// A positional subcommand, e.g. `["spawn", session]`.
    #[must_use]
    pub fn subcommand<I, S>(parts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            verb: NtmVerb::Assign,
            session: None,
            panes: None,
            subcommand: Some(
                parts
                    .into_iter()
                    .map(|part| part.as_ref().to_owned())
                    .collect(),
            ),
            extra: Vec::new(),
        }
    }

    /// The pane selector, in whatever spelling the VERB actually honours.
    ///
    /// ⛔ `--panes=` is NOT universal, and the exception fails SILENTLY. Measured 2026-09-11
    /// against a live 8-pane session: `--robot-inspect-pane --panes=<0..5|99>` returns
    /// `success=true, pane_index=0` for EVERY value -- including a nonexistent pane -- because the
    /// verb's own selector is `--inspect-index=`, documented in `ntm --help`. So a consumer that
    /// asks for pane 5 is answered about pane 0 and told it succeeded.
    ///
    /// The validator is inconsistent in the worst direction: `--pane-index=` and `--target=` are
    /// correctly refused with `INVALID_FLAG`, while `--panes=` and `--pane=` -- the two spellings a
    /// caller arrives with from the rest of this surface -- are accepted and ignored.
    ///
    /// This method therefore translates per verb. The call site keeps ONE spelling; the wire gets
    /// the one that works. Without this, the first `InspectPane` adopter would have graded eight
    /// panes on pane 0 -- the zsh pane, never an agent -- with every payload reading success.
    #[must_use]
    pub fn panes(mut self, panes: &str) -> Self {
        self.panes = Some(panes.to_owned());
        self
    }

    /// Extra flags, appended verbatim after the verb's own arguments.
    #[must_use]
    pub fn arg(mut self, arg: &str) -> Self {
        self.extra.push(arg.to_owned());
        self
    }

    /// Extra flags, appended verbatim.
    #[must_use]
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.extra
            .extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
        self
    }

    /// The argv AFTER the program name, so a caller can assert on it without
    /// spawning. This is the only place the flag grammar is assembled.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        let mut argv = Vec::new();
        if let Some(parts) = &self.subcommand {
            argv.extend(parts.iter().cloned());
        } else {
            match &self.session {
                Some(session) => argv.push(format!("{}={session}", self.verb.flag())),
                None => argv.push(self.verb.flag().to_owned()),
            }
        }
        if let Some(panes) = &self.panes {
            argv.push(format!("{}={panes}", self.verb.pane_selector_flag()));
        }
        argv.extend(self.extra.iter().cloned());
        argv
    }
}

/// What the kernel will say about a call. The payload is reachable ONLY through
/// [`NtmOutcome::Answered`], which is trap 1 enforced by the type system rather
/// than by a comment: a consumer holding a `Refused` has no field to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NtmOutcome {
    /// Exit 0 AND a parsed payload whose own `success` is true.
    Answered { payload: Value },
    /// The process ran and refused. Carries the EXIT STATUS, never an
    /// `error_code`: two verbs spell one condition two ways (trap 2).
    Refused {
        exit_code: Option<i32>,
        stderr: String,
    },
    /// Exit 0 but the bytes are not a JSON object, or the payload's own
    /// `success` is not true. Distinct from `Refused` so "the verb said no" and
    /// "the verb answered something we cannot read" never merge.
    Unparsable {
        exit_code: Option<i32>,
        detail: String,
    },
    /// The call never produced a verdict: unspawnable, or the deadline killed
    /// the process group. Restrictive by construction.
    Unanswerable {
        reason_code: &'static str,
        detail: String,
    },
}

impl NtmOutcome {
    /// The payload, or `None`. A failed call has NO payload here even when the
    /// process printed a populated zeroed object.
    #[must_use]
    pub fn payload(&self) -> Option<&Value> {
        match self {
            Self::Answered { payload } => Some(payload),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_answered(&self) -> bool {
        matches!(self, Self::Answered { .. })
    }

    /// A stable label for logs and refusal rows.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match self {
            Self::Answered { .. } => "ANSWERED",
            Self::Refused { .. } => "REFUSED",
            Self::Unparsable { .. } => "UNPARSABLE",
            Self::Unanswerable { .. } => "UNANSWERABLE",
        }
    }
}

/// Classify a completed invocation. EXIT STATUS FIRST, payload second.
///
/// `success` is the process's exit status, not any field of the payload. A
/// not-found answer from `--robot-agent-health` exits nonzero AND prints
/// `agent{process_running:false}` plus a zeroed `fleet_health`, so a consumer
/// that parses before branching reads false-and-zero as measurement.
#[must_use]
pub fn classify(success: bool, exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> NtmOutcome {
    if !success {
        return NtmOutcome::Refused {
            exit_code,
            stderr: String::from_utf8_lossy(stderr).into_owned(),
        };
    }
    let Ok(payload) = serde_json::from_slice::<Value>(stdout) else {
        return NtmOutcome::Unparsable {
            exit_code,
            detail: "stdout is not JSON".to_owned(),
        };
    };
    if payload.get("success").and_then(Value::as_bool) != Some(true) {
        return NtmOutcome::Unparsable {
            exit_code,
            detail: "payload.success is not true".to_owned(),
        };
    }
    NtmOutcome::Answered { payload }
}

/// Invoke `ntm` on the sanctioned bounded path and classify the result.
///
/// The spawn, the fresh process group, the concurrent drain of both pipes and
/// the deadline that signals the GROUP all belong to `subprocess-contract`;
/// this function owns the argv and the classification. No detached task is
/// created: the call returns only after the child is reaped or its group is
/// signalled.
#[must_use]
pub fn invoke_bounded(call: &NtmCall, deadline: Duration) -> NtmOutcome {
    let argv = call.argv();
    let mut command = std::process::Command::new("ntm");
    command.args(&argv);
    match subprocess_contract::bounded_output(&mut command, deadline) {
        subprocess_contract::BoundedOutcome::Completed(output) => classify(
            output.status.success(),
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ),
        subprocess_contract::BoundedOutcome::TimedOut => NtmOutcome::Unanswerable {
            reason_code: "NTM_TIMEOUT",
            detail: format!("ntm {} exceeded {:?}", argv.join(" "), deadline),
        },
        subprocess_contract::BoundedOutcome::Unspawned(error) => NtmOutcome::Unanswerable {
            reason_code: "NTM_UNAVAILABLE",
            detail: format!("{error}"),
        },
    }
}

/// A completed `ntm` run, for invocations whose output is NOT a robot payload
/// (`ntm spawn …`). Exit status is carried as its own field so a consumer
/// branches on it before reading either stream, exactly as [`classify`] does
/// for the JSON verbs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtmRun {
    pub argv: Vec<String>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run `ntm` and hand back the completed run, or the reason there is no run.
///
/// `Err` is the unanswerable case — unspawnable, or the deadline signalled the
/// process GROUP — so "we never asked" can never be mistaken for "it said no".
pub fn run_bounded(call: &NtmCall, deadline: Duration) -> Result<NtmRun, NtmOutcome> {
    let argv = call.argv();
    let mut command = std::process::Command::new("ntm");
    command.args(&argv);
    match subprocess_contract::bounded_output(&mut command, deadline) {
        subprocess_contract::BoundedOutcome::Completed(output) => Ok(NtmRun {
            argv,
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        subprocess_contract::BoundedOutcome::TimedOut => Err(NtmOutcome::Unanswerable {
            reason_code: "NTM_TIMEOUT",
            detail: format!("ntm {} exceeded {:?}", argv.join(" "), deadline),
        }),
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(NtmOutcome::Unanswerable {
            reason_code: "NTM_UNAVAILABLE",
            detail: format!("{error}"),
        }),
    }
}

/// The async path, for callers already inside a region. `&Cx` is FIRST per the
/// asupersync contract, cancellation is the caller's, and nothing is detached.
pub async fn invoke(cx: &asupersync::Cx, call: &NtmCall) -> NtmOutcome {
    let argv = call.argv();
    let mut command = asupersync::process::Command::new("ntm");
    command.args(&argv);
    match subprocess_contract::run_output(cx, command).await {
        Ok(output) => classify(
            output.status.success(),
            output.status.code(),
            &output.stdout,
            &output.stderr,
        ),
        Err(error) => NtmOutcome::Unanswerable {
            reason_code: "NTM_UNAVAILABLE",
            detail: format!("{error}"),
        },
    }
}

/// One capture of a pane, digest included. Trap 3: motion is decided by
/// COMPARING SNAPSHOTS, never by assuming a counter grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSnapshot {
    pub pane: String,
    pub lines: u64,
    pub chars: u64,
    pub digest: String,
}

/// What two captures prove about a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// The captures differ: the pane moved.
    Moved,
    /// The captures are identical: the pane did not move.
    Still,
    /// The captures cannot be compared (different panes, or a capture we never
    /// got). NOT idleness — a pane we could not observe twice is unproven.
    Indeterminate,
}

/// Decide motion from two captures of the same pane.
///
/// `lines` and `chars` are deliberately NOT consulted. A saturated busy pane
/// holds `lines` fixed while its content churns, so a grew-the-line-count test
/// calls the busiest pane in the fleet idle; and a scrollback-capped pane can
/// hold `chars` fixed or move it either direction. The digest is the only field
/// that answers the question asked.
#[must_use]
pub fn motion(first: &PaneSnapshot, second: &PaneSnapshot) -> Motion {
    if first.pane != second.pane || first.digest.is_empty() || second.digest.is_empty() {
        return Motion::Indeterminate;
    }
    if first.digest == second.digest {
        Motion::Still
    } else {
        Motion::Moved
    }
}

/// The pane denominator: how many PANES the payload describes.
///
/// Trap 4: `total_panes` is the AGENT count. Measured 2026-09-11, a session
/// with 8 tmux panes reported `total_panes: 7`, so a ratio built on it silently
/// drops every non-agent pane and reads as a complete census.
///
/// An absent or non-object `panes` map is an ERROR, never zero: "we could not
/// see the panes" and "there are no panes" are different facts.
pub fn pane_denominator(payload: &Value) -> Result<usize, &'static str> {
    payload
        .get("panes")
        .and_then(Value::as_object)
        .map(serde_json::Map::len)
        .ok_or("NTM_NO_PANE_MAP")
}

/// Whether a named pane is present, and whether it is critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The pane is in the payload and not critical.
    Healthy,
    /// The pane is in the payload and critical.
    Critical,
    /// The pane is NOT in the payload. `critical_count` is 0 for this case and
    /// 1 for a dead-but-present pane, so zero must never read as health.
    Absent,
}

/// Resolve a pane's presence WITHOUT letting `critical_count` stand in for it.
///
/// Trap 5: `critical_count` INVERTS at the boundary — a dead pane contributes
/// 1, a pane that does not exist contributes 0. Gating on `critical_count == 0`
/// therefore treats the one pane nobody can reach as the healthiest in the
/// fleet. Presence is resolved from the pane MAP; the count is only ever used
/// once presence is established.
#[must_use]
pub fn presence(payload: &Value, pane: &str) -> Presence {
    let Some(row) = payload
        .get("panes")
        .and_then(Value::as_object)
        .and_then(|panes| panes.get(pane))
    else {
        return Presence::Absent;
    };
    let critical = row
        .get("critical_count")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        > 0;
    if critical {
        Presence::Critical
    } else {
        Presence::Healthy
    }
}

/// Per-pane rate-limit facts, with ABSENCE OF EVIDENCE kept distinct from EVIDENCE OF ABSENCE.
///
/// The shape this replaces returned an empty map for BOTH "the verb answered and nobody is
/// limited" and "the verb refused, so I know nothing" -- and the second read as the first at every
/// call site. That conflation is the whole reason a rate-limited pane can be offered as capacity:
/// a consumer asks "is this pane limited", gets `false` from a map that was never populated, and
/// dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitCensus {
    /// The verb answered. The map is authoritative for exactly the panes it names.
    Known(std::collections::BTreeMap<String, bool>),
    /// The verb refused, timed out, or shipped no `panes` object. **NOT** "nobody is limited".
    /// Carries the outcome word so a consumer can disclose which bound it is reporting under.
    Unknown(String),
}

impl RateLimitCensus {
    /// `Some(true|false)` when measured for this pane; `None` when UNMEASURED.
    ///
    /// Three-valued on purpose: a consumer that wants to fail open must say so by matching `None`
    /// explicitly, rather than receiving a `false` it cannot distinguish from a measurement.
    #[must_use]
    pub fn is_rate_limited(&self, pane: &str) -> Option<bool> {
        match self {
            Self::Known(map) => map.get(pane).copied(),
            Self::Unknown(_) => None,
        }
    }

    /// The bound this census is reporting under, for a consumer's own disclosure.
    #[must_use]
    pub fn bound(&self) -> String {
        match self {
            Self::Known(map) => format!("KNOWN panes={}", map.len()),
            Self::Unknown(reason) => format!("UNKNOWN {reason}"),
        }
    }
}

/// Parse the `--robot-agent-health` payload's per-pane rate-limit flags.
///
/// PURE, so the parse is testable without a live fleet -- the reason the previous copy had no leg.
#[must_use]
pub fn rate_limited_from_payload(document: &Value) -> RateLimitCensus {
    let Some(panes) = document.get("panes").and_then(Value::as_object) else {
        return RateLimitCensus::Unknown("payload carried no `panes` object".to_owned());
    };
    RateLimitCensus::Known(
        panes
            .iter()
            .filter_map(|(index, pane)| {
                pane.get("local_state")?
                    .get("is_rate_limited")?
                    .as_bool()
                    .map(|limited| (index.clone(), limited))
            })
            .collect(),
    )
}

/// ONE agent-health call per session, keyed by pane index.
///
/// One implementation, two consumers (`fleet-monitor`, `pane-dispatch-ready`): two copies of this
/// parse is two policies, and they drift where a monitor cannot see it.
#[must_use]
pub fn rate_limited_panes(session: &str, deadline: Duration) -> RateLimitCensus {
    let outcome = invoke_bounded(
        &NtmCall::on_session(NtmVerb::AgentHealth, session).arg("--no-caut"),
        deadline,
    );
    // A non-`Answered` outcome has NO payload to read, so "branch on success first" is enforced by
    // the type rather than by a comment.
    match outcome.payload() {
        Some(document) => rate_limited_from_payload(document),
        None => RateLimitCensus::Unknown(format!("agent-health {}", outcome.state())),
    }
}
