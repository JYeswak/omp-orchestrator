//! Typed mapping of `ntm --robot-snapshot` per-source freshness into L4 verdicts.
//!
//! # Contract: copy the shipped envelope, do not invent one
//!
//! Observed 2026-09-09 (`ntm --robot-snapshot | jq .sources`): `.sources.sources`
//! is a NONEMPTY object keyed by source name. Each row carries `available: bool`,
//! `fresh: bool`, `reason_code: <nonempty string>`, `age_ms: <u64>`, plus
//! passthrough extras (`name`, `updated_at`) that are preserved by ignoring,
//! never validated. The population count is MUTABLE and is never asserted.
//!
//! # Mapping into existing types
//!
//! Each row becomes a [`LayerVerdict`] with `layer = Layer::L4`, `row_count = 1`,
//! `last_reason = reason_code`, and the row's own `age_ms`/`fresh`. State is a
//! restrictive triple: available+fresh is [`LayerState::Progressing`],
//! available+stale is [`LayerState::Silent`], unavailable (whatever `fresh`
//! claims) is [`LayerState::Refusing`]. Availability dominates because a source
//! that cannot be reached must never read as merely slow.
//!
//! The aggregate [`gate_ntm_sources`] runs the existing [`gate_freshness_verdict`]
//! over the mapped verdicts first, so the L4 gate itself executes on this path,
//! then translates its layer-erasing verdict into a source-naming one: the
//! shared gate reports `l4:silent`, which cannot name the source; this module
//! must, so it keeps the names alongside.
//!
//! # Error taxonomy (all restrictive, never fresh)
//!
//! * [`NtmSourceError::SnapshotUnavailable`] — the snapshot command failed, or
//!   `.sources.sources` is absent. UNMEASURED. Exit 3.
//! * [`NtmSourceError::EmptySources`] — typed empty-scan error. Exit 2.
//! * [`NtmSourceError::MissingField`] / [`NtmSourceError::WrongType`] /
//!   [`NtmSourceError::EmptyReason`] / [`NtmSourceError::MalformedSources`] —
//!   per-row shape violations, each naming the source. Exit 1.
//! * [`NtmSourceError::StaleSources`] — aggregate NOT_LIVE, naming every
//!   non-progressing source. Exit 1.
//!
//! A new error enum rather than [`MonitorError`] reuse is deliberate:
//! `MonitorError::EmptyScan` carries a journal path and `MissingReason` a line
//! number, and neither exists for a snapshot object. Reusing them would be the
//! dishonest-label defect this crate exists to prevent.

use std::path::Path;
use std::time::Duration;

use asupersync::process::Command;
use asupersync::time::timeout;
use asupersync::Cx;
use lifecycle_event::Layer;
use serde_json::Value;

use super::{gate_freshness_verdict, LayerState, LayerVerdict};

/// Hard ceiling for the live snapshot subprocess. `ntm --robot-snapshot` was
/// measured at ~27s wall on 2026-09-09; past this deadline the run is
/// UNMEASURED rather than slow-and-trusted.
const SNAPSHOT_DEADLINE: Duration = Duration::from_secs(60);

/// One NTM source row mapped into the existing L4 verdict type. `source` is
/// the envelope's map key; `verdict` carries the preserved fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtmSourceVerdict {
    pub source: String,
    pub verdict: LayerVerdict,
}

/// Restrictive errors for the NTM source path. No variant reads as fresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NtmSourceError {
    SnapshotUnavailable {
        detail: String,
    },
    EmptySources,
    MalformedSources {
        detail: String,
    },
    MissingField {
        source: String,
        field: &'static str,
    },
    WrongType {
        source: String,
        field: &'static str,
        expected: &'static str,
    },
    EmptyReason {
        source: String,
    },
    StaleSources {
        sources: Vec<String>,
    },
    /// An EXPECTED source key is ABSENT from `.sources.sources`. UNPROVEN, never
    /// fresh: nothing was observed about that source, which is not the same as
    /// observing it healthy. Exit 3 (unmeasured), because this is a gap in the
    /// reading rather than a verdict about a source that answered.
    MissingSource {
        sources: Vec<String>,
    },
    /// The source key is PRESENT but its row is JSON `null`. A third reading,
    /// distinct from both populated and absent: the instrument named the source
    /// and then declined to describe it.
    NullSource {
        source: String,
    },
}

impl std::fmt::Display for NtmSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SnapshotUnavailable { detail } => {
                write!(
                    f,
                    "NTM_SOURCE_UNAVAILABLE {detail} — unmeasured, never fresh"
                )
            }
            Self::EmptySources => write!(f, "NTM_SOURCE_EMPTY_SCAN — empty is ERROR, never a pass"),
            Self::MalformedSources { detail } => {
                write!(f, "NTM_SOURCE_MALFORMED_SOURCES {detail}")
            }
            Self::MissingField { source, field } => {
                write!(f, "NTM_SOURCE_MISSING_FIELD source={source} field={field}")
            }
            Self::WrongType {
                source,
                field,
                expected,
            } => write!(
                f,
                "NTM_SOURCE_WRONG_TYPE source={source} field={field} expected={expected}"
            ),
            Self::EmptyReason { source } => {
                write!(f, "NTM_SOURCE_EMPTY_REASON source={source}")
            }
            Self::StaleSources { sources } => write!(
                f,
                "NTM_SOURCE_STALE sources={} — stale is ERROR, never a pass",
                sources.join(",")
            ),
            Self::MissingSource { sources } => write!(
                f,
                // The prose deliberately names NO health word. A message that
                // describes what it is not still puts those words on the wire,
                // where a downstream substring reader can pick them up.
                "NTM_SOURCE_MISSING_SOURCE sources={} — UNPROVEN: the key is ABSENT, \
                 so nothing at all was observed for it",
                sources.join(",")
            ),
            Self::NullSource { source } => write!(
                f,
                "NTM_SOURCE_NULL_ROW source={source} — UNPROVEN: the key is PRESENT \
                 but null, a third reading distinct from populated and from absent"
            ),
        }
    }
}

impl std::error::Error for NtmSourceError {}

/// Exit-code contract for the `ntm-sources` verb. Message AND code are pinned:
/// 0 all fresh, 1 stale/shape violation, 2 empty scan, 3 unmeasured.
pub fn ntm_source_exit_code(error: &NtmSourceError) -> std::process::ExitCode {
    match error {
        NtmSourceError::EmptySources => std::process::ExitCode::from(2),
        NtmSourceError::SnapshotUnavailable { .. }
        | NtmSourceError::MissingSource { .. }
        | NtmSourceError::NullSource { .. } => std::process::ExitCode::from(3),
        _ => std::process::ExitCode::from(1),
    }
}

fn get_bool(row: &Value, source: &str, field: &'static str) -> Result<bool, NtmSourceError> {
    match row.get(field) {
        None => Err(NtmSourceError::MissingField {
            source: source.to_owned(),
            field,
        }),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(NtmSourceError::WrongType {
            source: source.to_owned(),
            field,
            expected: "bool",
        }),
    }
}

fn get_reason(row: &Value, source: &str) -> Result<String, NtmSourceError> {
    match row.get("reason_code") {
        None => Err(NtmSourceError::MissingField {
            source: source.to_owned(),
            field: "reason_code",
        }),
        Some(Value::String(reason)) if !reason.is_empty() => Ok(reason.clone()),
        Some(Value::String(_)) => Err(NtmSourceError::EmptyReason {
            source: source.to_owned(),
        }),
        Some(_) => Err(NtmSourceError::WrongType {
            source: source.to_owned(),
            field: "reason_code",
            expected: "string",
        }),
    }
}

fn get_age_ms(row: &Value, source: &str) -> Result<u64, NtmSourceError> {
    match row.get("age_ms") {
        None => Err(NtmSourceError::MissingField {
            source: source.to_owned(),
            field: "age_ms",
        }),
        // `as_u64` rejects negatives and fractions: age_ms is a nonnegative integer.
        Some(value) => value.as_u64().ok_or(NtmSourceError::WrongType {
            source: source.to_owned(),
            field: "age_ms",
            expected: "u64",
        }),
    }
}

fn map_row(source: &str, row: &Value) -> Result<NtmSourceVerdict, NtmSourceError> {
    // THREE STATES, NOT TWO. A `null` row is PRESENT-BUT-NULL and gets its own
    // reading; absence is handled by `require_expected_sources`, which sees the
    // key set rather than a row. Folding these together would make it impossible
    // to say which one the instrument actually reported.
    if row.is_null() {
        return Err(NtmSourceError::NullSource {
            source: source.to_owned(),
        });
    }
    if !row.is_object() {
        return Err(NtmSourceError::WrongType {
            source: source.to_owned(),
            field: "<row>",
            expected: "object",
        });
    }
    let available = get_bool(row, source, "available")?;
    let fresh = get_bool(row, source, "fresh")?;
    let reason_code = get_reason(row, source)?;
    let age_ms = get_age_ms(row, source)?;
    let state = if !available {
        LayerState::Refusing
    } else if !fresh {
        LayerState::Silent
    } else {
        LayerState::Progressing
    };
    Ok(NtmSourceVerdict {
        source: source.to_owned(),
        verdict: LayerVerdict {
            layer: Layer::L4,
            state,
            row_count: 1,
            last_reason: reason_code,
            age_ms,
            fresh,
        },
    })
}

/// Parse `.sources.sources` out of a decoded `ntm --robot-snapshot` document.
/// Every row is mapped; the population count is never asserted.
pub fn parse_ntm_sources(snapshot: &Value) -> Result<Vec<NtmSourceVerdict>, NtmSourceError> {
    let sources = snapshot
        .get("sources")
        .and_then(|outer| outer.get("sources"))
        .ok_or_else(|| NtmSourceError::SnapshotUnavailable {
            detail: "missing .sources.sources".to_owned(),
        })?;
    let map = sources
        .as_object()
        .ok_or_else(|| NtmSourceError::MalformedSources {
            detail: "expected .sources.sources to be an object".to_owned(),
        })?;
    if map.is_empty() {
        return Err(NtmSourceError::EmptySources);
    }
    let mut out = Vec::with_capacity(map.len());
    // `serde_json::Map` preserves document order; output follows it deterministically.
    for (name, row) in map {
        out.push(map_row(name, row)?);
    }
    Ok(out)
}

/// Aggregate gate: `all_fresh` iff every mapped row is progressing. Runs the
/// existing [`gate_freshness_verdict`] over the mapped verdicts so the L4 gate
/// itself adjudicates this path, then names the offending sources (which the
/// shared gate's `l4:state` rendering cannot).
pub fn gate_ntm_sources(verdicts: &[NtmSourceVerdict]) -> Result<(), NtmSourceError> {
    if verdicts.is_empty() {
        return Err(NtmSourceError::EmptySources);
    }
    let inner: Vec<LayerVerdict> = verdicts.iter().map(|v| v.verdict.clone()).collect();
    if gate_freshness_verdict(&inner).is_ok() {
        return Ok(());
    }
    let stale: Vec<String> = verdicts
        .iter()
        .filter(|v| v.verdict.state != LayerState::Progressing)
        .map(|v| format!("{}:{}", v.source, v.verdict.state.as_str()))
        .collect();
    Err(NtmSourceError::StaleSources { sources: stale })
}

/// The source keys this repository CONSUMES and therefore must be able to refuse
/// the absence of. Measured 2026-09-11 against `ntm --robot-snapshot
/// --capability-compact` (ntm v1.31.0-4-ge4718530): only `work_coordination` is
/// emitted, so `agent_mail` and `tick_monitor` are ABSENT today. That is why an
/// expected set exists at all — without it an absent source contributes no row and
/// the aggregate reads all-fresh.
pub const EXPECTED_NTM_SOURCES: [&str; 3] = ["agent_mail", "tick_monitor", "work_coordination"];

/// CONSUME AND REFUSE, session-scoped (bead x11g): the snapshot must carry a row
/// keyed by the SESSION the consumer is asking about, not merely the global
/// `work_coordination` row.
///
/// Measured 2026-09-11, ntm v1.31.0-4-ge4718530: `.sources.sources` carries ONE
/// key, `work_coordination`, and `ntm --help` shows `--robot-snapshot` takes NO
/// session argument — so no invocation of this build produces a session-named
/// key. A consumer that asks "is session X observed?" therefore gets nothing, and
/// nothing must read as UNPROVEN rather than as the global row's health.
///
/// `session` is compared verbatim against the map key. It is the caller's session
/// name, not a pattern: a fuzzy match would let `work_coordination` satisfy a
/// question about a session, which is the collapse this refuses.
///
/// # Errors
///
/// [`NtmSourceError::EmptySources`] for an empty scan or an empty session name —
/// an unnamed session is not a satisfied one. [`NtmSourceError::MissingSource`]
/// naming the session when no row is keyed by it.
pub fn require_session_source(
    verdicts: &[NtmSourceVerdict],
    session: &str,
) -> Result<(), NtmSourceError> {
    if verdicts.is_empty() || session.is_empty() {
        return Err(NtmSourceError::EmptySources);
    }
    if verdicts.iter().any(|v| v.source == session) {
        return Ok(());
    }
    Err(NtmSourceError::MissingSource {
        sources: vec![session.to_owned()],
    })
}

/// CONSUME AND REFUSE: every expected source key must be PRESENT in the mapped
/// verdicts. An absent key is UNPROVEN — never fresh, never idle, never
/// `age_ms = 0`.
///
/// This is the predicate the population count deliberately does not provide:
/// [`parse_ntm_sources`] maps whatever rows exist, so a source that vanished from
/// the snapshot simply contributes nothing and [`gate_ntm_sources`] then reports
/// all-fresh. Absence must enter the computation explicitly or it reads as health.
///
/// # Errors
///
/// [`NtmSourceError::EmptySources`] when nothing was observed at all — an empty
/// scan is an ERROR, never "all fresh". [`NtmSourceError::MissingSource`] naming
/// every expected key that is absent.
pub fn require_expected_sources(
    verdicts: &[NtmSourceVerdict],
    expected: &[&str],
) -> Result<(), NtmSourceError> {
    if verdicts.is_empty() {
        // ANTI-VACUITY: nothing observed is not everything healthy, and an empty
        // expected set must not turn this into a pass either.
        return Err(NtmSourceError::EmptySources);
    }
    if expected.is_empty() {
        return Err(NtmSourceError::EmptySources);
    }
    let missing: Vec<String> = expected
        .iter()
        .filter(|name| !verdicts.iter().any(|v| v.source == **name))
        .map(|name| (*name).to_owned())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(NtmSourceError::MissingSource { sources: missing })
}

/// [`gate_ntm_sources`] followed by the expected-set refusal.
///
/// ORDER, and it is deliberate: the sources that DID answer are adjudicated
/// first, so a stale row keeps its own naming error (`NTM_SOURCE_STALE`, exit 1)
/// instead of being reported as an absence. Absence is adjudicated second, and
/// crucially it is NOT reachable-only-on-failure: when every row that answered is
/// fresh, the expected-set check still runs and still refuses. That is the whole
/// point — a snapshot cannot pass on the strength of the sources present.
///
/// # Errors
///
/// Propagates [`gate_ntm_sources`] and then [`require_expected_sources`].
pub fn gate_ntm_sources_requiring(
    verdicts: &[NtmSourceVerdict],
    expected: &[&str],
) -> Result<(), NtmSourceError> {
    gate_ntm_sources(verdicts)?;
    require_expected_sources(verdicts, expected)
}

/// Run the live `ntm --robot-snapshot` under the caller's cancellation context.
///
/// `&Cx` is first per the repository binding: every subprocess is cancellable
/// work with a deadline, owned by the caller's region. The 60s restrictive
/// bound is preserved through [`timeout`]: a hung daemon yields
/// [`NtmSourceError::SnapshotUnavailable`], never a hang. Any transport
/// failure — unspawnable binary, deadline, cancellation, non-JSON stdout —
/// is unmeasured, never a verdict about the sources.
pub async fn read_live_snapshot(cx: &Cx, ntm_bin: &str) -> Result<Value, NtmSourceError> {
    cx.checkpoint()
        .map_err(|_| NtmSourceError::SnapshotUnavailable {
            detail: "caller context cancelled before spawn".to_owned(),
        })?;
    let mut command = Command::new(ntm_bin);
    command.args(["--robot-snapshot"]);
    let output = match timeout(
        cx.now_for_observability(),
        SNAPSHOT_DEADLINE,
        subprocess_contract::run_output(cx, command),
    )
    .await
    {
        Ok(Ok(output)) => output,
        Ok(Err(subprocess_contract::RunError::Timeout))
        | Ok(Err(subprocess_contract::RunError::Cancelled(_))) => {
            return Err(NtmSourceError::SnapshotUnavailable {
                detail: "snapshot wait cancelled by caller context".to_owned(),
            });
        }
        Ok(Err(subprocess_contract::RunError::Process(error))) => {
            return Err(NtmSourceError::SnapshotUnavailable {
                detail: format!("cannot run {ntm_bin}: {error}"),
            });
        }
        Err(_) => {
            return Err(NtmSourceError::SnapshotUnavailable {
                detail: format!(
                    "ntm --robot-snapshot exceeded {}s",
                    SNAPSHOT_DEADLINE.as_secs()
                ),
            });
        }
    };
    if !output.status.success() {
        return Err(NtmSourceError::SnapshotUnavailable {
            detail: format!(
                "ntm --robot-snapshot exited {}",
                output.status.code().unwrap_or(-1)
            ),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|error| NtmSourceError::SnapshotUnavailable {
        detail: format!("snapshot stdout is not JSON: {error}"),
    })
}

/// Read a captured snapshot document from a file. Same envelope, alternate
/// transport for replay and for process legs that must not depend on a live
/// daemon. A missing file is unmeasured; present-but-not-JSON is malformed.
pub fn read_snapshot_file(path: &Path) -> Result<Value, NtmSourceError> {
    let bytes = std::fs::read(path).map_err(|error| NtmSourceError::SnapshotUnavailable {
        detail: format!("cannot read snapshot file {}: {error}", path.display()),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| NtmSourceError::MalformedSources {
        detail: format!("snapshot file is not JSON: {error}"),
    })
}
