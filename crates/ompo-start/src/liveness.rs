#![forbid(unsafe_code)]

//! Pure L4 liveness algebra shared by \`ompo start\` and \`ompo portal\`.
//!
//! A source is live only when it is available, fresh, and carries its pane set.
//! Missing freshness is SILENT; one silent or disagreeing source makes the swarm
//! not live. Runtime adapters belong to the umbrella crate.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVerdict {
    pub name: String,
    pub available: bool,
    pub fresh: bool,
    pub reason_code: String,
    pub age_ms: Option<u64>,
    pub panes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveVerdict {
    Live { sources: Vec<SourceVerdict> },
    NotLive {
        sources: Vec<SourceVerdict>,
        reason_code: String,
    },
}

impl LiveVerdict {
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live { .. })
    }

    #[must_use]
    pub fn status(&self) -> &'static str {
        if self.is_live() { "LIVE" } else { "NOT_LIVE" }
    }

    #[must_use]
    pub fn reason_code(&self) -> &str {
        match self {
            Self::Live { .. } => "L4_LIVE",
            Self::NotLive { reason_code, .. } => reason_code,
        }
    }

    #[must_use]
    pub fn sources(&self) -> &[SourceVerdict] {
        match self {
            Self::Live { sources } | Self::NotLive { sources, .. } => sources,
        }
    }
}

/// Classify the required NTM, tick-monitor, and Agent Mail sources.
///
/// The source set is sorted by name before comparison, making pane-set agreement
/// deterministic rather than dependent on command completion order.
pub fn classify(mut sources: Vec<SourceVerdict>) -> Result<LiveVerdict, String> {
    if sources.is_empty() {
        return Err("L4_EMPTY_SOURCE_SET — no liveness source was observed".to_owned());
    }
    sources.sort_by(|left, right| left.name.cmp(&right.name));
    if sources.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err("L4_DUPLICATE_SOURCE — liveness source names must be unique".to_owned());
    }

    let required = ["agent-mail", "ntm", "tick-monitor"];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|name| !sources.iter().any(|source| source.name == *name))
        .collect();
    if !missing.is_empty() {
        return Ok(LiveVerdict::NotLive {
            sources,
            reason_code: format!("L4_SILENT_MISSING_SOURCE sources={}", missing.join(",")),
        });
    }
    let silent_source = sources
        .iter()
        .find(|source| !source.available || !source.fresh || source.age_ms.is_none())
        .map(|source| source.name.clone());
    if let Some(name) = silent_source {
        return Ok(LiveVerdict::NotLive {
            sources,
            reason_code: format!("L4_SILENT_SOURCE source={name}"),
        });
    }

    let first_panes = &sources[0].panes;
    if sources.iter().any(|source| source.panes != *first_panes) {
        return Ok(LiveVerdict::NotLive {
            sources,
            reason_code: "L4_PANE_SET_DISAGREE".to_owned(),
        });
    }

    Ok(LiveVerdict::Live { sources })
}
