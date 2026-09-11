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

/// The short jq-facing key for a canonical source name.
///
/// The acceptance readbacks are `.liveness.sources.ntm`, `.liveness.sources.tick`
/// and `.liveness.sources.mail`, while the algebra's canonical names are `ntm`,
/// `tick-monitor` and `agent-mail`. Both keys are emitted and carry the SAME
/// object, so neither the short readback nor a consumer pinned to the canonical
/// name can silently read `null` — a null there is indistinguishable from a
/// source that was never probed.
#[must_use]
pub fn short_key(name: &str) -> &str {
    match name {
        "tick-monitor" => "tick",
        "agent-mail" => "mail",
        other => other,
    }
}

/// A source with no observable age.
///
/// `observed_at` absent is SILENT, not zero and not fresh: a missing timestamp
/// once read as "age 0 ms", which is the freshest possible answer for the source
/// that answered least.
#[must_use]
pub fn is_silent(source: &SourceVerdict) -> bool {
    !source.available || !source.fresh || source.age_ms.is_none()
}

/// WHY a source is silent, or `None` when it is not.
///
/// A bare `silent: true` says the mail row had no `_meta.timestamp` and a row
/// whose probe never answered are the same fact; they are not. The reason is the
/// field a reader routes on, so it is emitted beside the boolean and is `null`
/// exactly when `silent` is false.
#[must_use]
pub fn silent_reason(source: &SourceVerdict) -> Option<String> {
    if !source.available {
        return Some("L4_SILENT_UNAVAILABLE".to_owned());
    }
    if !source.fresh {
        return Some("L4_SILENT_STALE".to_owned());
    }
    if source.age_ms.is_none() {
        return Some("L4_SILENT_NO_TIMESTAMP".to_owned());
    }
    None
}

/// One source's census row.
///
/// `names` is the pane census: WHICH panes the source saw, not how many. An
/// earlier portal emitted only `work_coordination` for ntm, so a source that saw
/// zero panes and a source that saw three were the same row from outside.
/// `pane_count` is emitted beside it so a renderer that truncates the list shows
/// up as a count mismatch instead of a silent drop.
///
/// `gap_secs` is the tick-monitor gap in whole seconds, derived from `age_ms` and
/// `null` exactly when `age_ms` is `null`. It is never defaulted to 0: a gap of
/// zero and an unmeasured gap are different facts, and `silent` is the field that
/// distinguishes them. A reader gets EITHER an age or `silent: true`, never
/// neither.
#[must_use]
pub fn source_json(source: &SourceVerdict) -> serde_json::Value {
    serde_json::json!({
        "name": source.name,
        "available": source.available,
        "fresh": source.fresh,
        "reason_code": source.reason_code,
        "age_ms": source.age_ms,
        "gap_secs": source.age_ms.map(|age| age / 1000),
        "silent": is_silent(source),
        "silent_reason": silent_reason(source),
        "names": source.panes,
        "panes": source.panes,
        "pane_count": source.panes.len(),
    })
}

/// The per-source census map, keyed by BOTH canonical and short name.
#[must_use]
pub fn sources_json(sources: &[SourceVerdict]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for source in sources {
        let row = source_json(source);
        let short = short_key(&source.name).to_owned();
        map.insert(source.name.clone(), row.clone());
        if short != source.name {
            map.insert(short, row);
        }
    }
    serde_json::Value::Object(map)
}

/// True only when EVERY observed source is available, fresh, and carries an age.
///
/// An empty source set is `false`, never `true`: "nothing was observed" must not
/// read as "everything is fresh".
#[must_use]
pub fn all_fresh(sources: &[SourceVerdict]) -> bool {
    !sources.is_empty()
        && sources
            .iter()
            .all(|source| source.available && source.fresh && source.age_ms.is_some())
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

/// A source's pane set, normalized: sorted and deduplicated.
///
/// Agreement is a SET question. Comparing the raw vectors made agreement depend
/// on each probe's output order, so two sources that saw the same panes in a
/// different order read as disagreeing.
#[must_use]
pub fn pane_set(source: &SourceVerdict) -> Vec<String> {
    let mut panes = source.panes.clone();
    panes.sort();
    panes.dedup();
    panes
}

/// The pane-set equality writer: the verdict AND the sets it was computed from.
///
/// `agree` is false when ANY set is empty, even though empty sets are trivially
/// equal to each other. Three sources that each saw nothing agree about nothing,
/// and reporting that as agreement is how a dead swarm reads as live. The three
/// sets are emitted beside the boolean so the verdict is checkable here rather
/// than trusted.
#[must_use]
pub fn pane_set_agreement(sources: &[SourceVerdict]) -> serde_json::Value {
    let sets: serde_json::Map<String, serde_json::Value> = sources
        .iter()
        .map(|source| {
            (
                short_key(&source.name).to_owned(),
                serde_json::json!(pane_set(source)),
            )
        })
        .collect();
    let normalized: Vec<Vec<String>> = sources.iter().map(pane_set).collect();
    let any_empty = normalized.iter().any(Vec::is_empty);
    let equal = normalized
        .windows(2)
        .all(|pair| pair[0] == pair[1]);
    let agree = !normalized.is_empty() && !any_empty && equal;
    let reason_code = if normalized.is_empty() {
        "L4_PANE_SET_NO_SOURCE"
    } else if any_empty {
        "L4_PANE_SET_VACUOUS"
    } else if !equal {
        "L4_PANE_SET_DISAGREE"
    } else {
        "L4_PANE_SET_AGREE"
    };
    serde_json::json!({
        "agree": agree,
        "reason_code": reason_code,
        "source_count": normalized.len(),
        "pane_sets": sets,
    })
}

/// Classify the required NTM, tick-monitor, and Agent Mail sources.
///
/// The source set is sorted by name before comparison, making pane-set agreement
/// deterministic rather than dependent on command completion order. Agreement is
/// computed by [`pane_set_agreement`], so an all-empty census is NOT agreement.
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
    // ONE AUTHORITY. This arm re-implemented `is_silent`'s condition inline, so
    // the row writer and the swarm verdict held two copies of the silence law
    // with DISJOINT consumers: gutting `is_silent` left every verdict leg green
    // (measured by two graders from two sites), while a test doc comment claimed
    // the metric and the verdict "cannot disagree about what silent means". They
    // could. Now there is nothing to disagree with.
    let silent_source = sources
        .iter()
        .find(|source| is_silent(source))
        .map(|source| source.name.clone());
    if let Some(name) = silent_source {
        return Ok(LiveVerdict::NotLive {
            sources,
            reason_code: format!("L4_SILENT_SOURCE source={name}"),
        });
    }

    let agreement = pane_set_agreement(&sources);
    if agreement["agree"] != serde_json::Value::Bool(true) {
        let reason_code = agreement["reason_code"]
            .as_str()
            .unwrap_or("L4_PANE_SET_DISAGREE")
            .to_owned();
        return Ok(LiveVerdict::NotLive {
            sources,
            reason_code,
        });
    }

    Ok(LiveVerdict::Live { sources })
}
