#![forbid(unsafe_code)]

//! Answer, for one bead: **which agent holds this, and is that agent alive.**
//!
//! # Why this crate exists, measured
//!
//! Joshua, 2026-09-02: *"if we can't identify which agent — exactly — is connected to a bead,
//! and if they are live or active or not, we have a failing system."* Both halves failed
//! independently:
//!
//! * **The assignee field is free text.** Measured over `br list -a --json`: `in_progress`
//!   beads named `supervisor:<pid>` for processes verified DEAD, pane labels like `pane3`,
//!   and unresolvable role words.
//! * **Liveness was unanswerable.** Agent Mail's roster read `last_active = 15h ago` for an
//!   agent that had committed 20 minutes earlier, and `prog=unknown, last_active=2d` for two
//!   agents that were actively working beads. A roster timestamp is not a liveness oracle.
//!
//! # THE EVIDENCE CORRECTION THIS CRATE IS BUILT ON
//!
//! The binding that works is the ACK protocol, which writes the pane INTO the bead:
//! `ACK <token> on %<pane>`. Deriving `pane -> agent` from those rows requires one
//! correction the original survey did not make, and without it the derivation is useless:
//!
//! **`br comments add` without `--actor` attributes the row to the git/`$USER` identity.**
//! Measured 2026-09-02 over `.beads/issues.jsonl`, 295 records:
//!
//! ```text
//! naive (author as-is)          %1408 AmberGate(14), josh(3)    <- every pane AMBIGUOUS
//!                               %1409 josh(4), SilverWolf(2)
//!                               %1413 josh(6), GreenFrog(2)
//!                               %1414 BlueLantern(16), josh(4)
//!
//! excluding the default author   %1408 AmberGate    %1409 SilverWolf
//!                                %1413 GreenFrog    %1414 BlueLantern   <- UNAMBIGUOUS
//! ```
//!
//! So the pane map is clean and **the real ambiguity is at the BEAD level**: two `in_progress`
//! beads carry ACKs from two different panes by two different agents. That is the case
//! [`HolderVerdict::Ambiguous`] exists for.
//!
//! The default-author set is **derived** (`git config user.name` plus `$USER`, case-folded),
//! never hardcoded — and the case fold is load-bearing: git reported `Josh` while the comment
//! author was `josh`, so a case-sensitive exclusion silently failed and reported every pane
//! ambiguous.
//!
//! # What is derived and what is judged
//!
//! Membership is a FACT, so it is derived from ACK rows. Nothing here consults a
//! hand-maintained mapping, and there is deliberately no place to put one.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The ONLY accepted provenance for a liveness answer.
///
/// A roster's `last_active` was measured **15 hours wrong** on an agent that was committing,
/// so it is not merely discouraged here — a verdict carrying any other provenance cannot be
/// [`HolderVerdict::Bound`]. `tick-monitor` is the kernel that takes two captures ≥75s apart
/// and is the fleet's liveness authority.
pub const LIVENESS_SOURCE: &str = "tick-monitor";

/// Why an assignee string is not an agent identity.
///
/// Each variant names a shape MEASURED in the live tracker, because a refusal that cannot
/// cite the thing it refuses gets read as pedantry and switched off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityRefusal {
    /// Empty or whitespace. With `status = in_progress` this is the orphan-claim illegal
    /// state; on its own it is simply not a name.
    Empty,
    /// `supervisor:<pid>`. Seven of these were verified dead with `ps -p`. A pid is not an
    /// identity: it is reused, it dies, and it names no one after it exits.
    DeadPidShape { raw: String },
    /// `pane3`, `pane4`. **A pane label is not an agent** — indices shift and panes are
    /// recycled, measured twice in one session.
    PaneLabelShape { raw: String },
    /// `%1408`. A pane ID is a better handle than a label and is still not an agent: the
    /// same pane hosted two different agent names within one hour.
    PaneIdShape { raw: String },
    /// The git/`$USER` default, which `br` writes when `--actor` is omitted. Accepting it
    /// would attribute every unactored write to one "agent" that is really the human.
    DefaultAuthor { raw: String },
    /// A name with no evidence anywhere in the tracker. Named rather than accepted, because
    /// inventing ownership is worse than naming the gap.
    Unregistered { raw: String },
}

impl fmt::Display for IdentityRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentityRefusal::Empty => f.write_str(
                "ASSIGNEE_REFUSED reason=EMPTY next_action=claim-with-an-agent-identity",
            ),
            IdentityRefusal::DeadPidShape { raw } => write!(
                f,
                "ASSIGNEE_REFUSED reason=PID_SHAPE raw={raw} \
                 detail=\"a pid is not an identity; it is reused and names no one after it \
                 exits\" next_action=claim-with-an-agent-identity"
            ),
            IdentityRefusal::PaneLabelShape { raw } => write!(
                f,
                "ASSIGNEE_REFUSED reason=PANE_LABEL raw={raw} \
                 detail=\"pane indices shift and panes are recycled\" \
                 next_action=claim-with-an-agent-identity"
            ),
            IdentityRefusal::PaneIdShape { raw } => write!(
                f,
                "ASSIGNEE_REFUSED reason=PANE_ID raw={raw} \
                 detail=\"one pane hosted two agent names within one hour\" \
                 next_action=claim-with-an-agent-identity"
            ),
            IdentityRefusal::DefaultAuthor { raw } => write!(
                f,
                "ASSIGNEE_REFUSED reason=DEFAULT_AUTHOR raw={raw} \
                 detail=\"this is the git/$USER identity br writes when --actor is omitted, \
                 not an agent\" next_action=pass---actor"
            ),
            IdentityRefusal::Unregistered { raw } => write!(
                f,
                "ASSIGNEE_REFUSED reason=UNREGISTERED raw={raw} \
                 detail=\"no ACK row anywhere in the tracker attributes work to this name\" \
                 next_action=ack-from-a-pane-first"
            ),
        }
    }
}

/// A validated agent identity. Constructible only through [`AgentIdentity::parse`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentIdentity(String);

impl AgentIdentity {
    /// Type an assignee string, or refuse it with the reason.
    ///
    /// `roster` is the set of names with ACK evidence — DERIVED, per this bead's rule that
    /// membership is a fact. `defaults` is the case-folded git/`$USER` set.
    pub fn parse(
        raw: &str,
        roster: &BTreeSet<String>,
        defaults: &BTreeSet<String>,
    ) -> Result<Self, IdentityRefusal> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(IdentityRefusal::Empty);
        }
        if defaults.contains(&trimmed.to_lowercase()) {
            return Err(IdentityRefusal::DefaultAuthor {
                raw: trimmed.to_owned(),
            });
        }
        if let Some(rest) = trimmed.strip_prefix("supervisor:") {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return Err(IdentityRefusal::DeadPidShape {
                    raw: trimmed.to_owned(),
                });
            }
        }
        if let Some(rest) = trimmed.strip_prefix('%') {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return Err(IdentityRefusal::PaneIdShape {
                    raw: trimmed.to_owned(),
                });
            }
        }
        if let Some(rest) = trimmed.strip_prefix("pane") {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                return Err(IdentityRefusal::PaneLabelShape {
                    raw: trimmed.to_owned(),
                });
            }
        }
        if !roster.contains(trimmed) {
            return Err(IdentityRefusal::Unregistered {
                raw: trimmed.to_owned(),
            });
        }
        Ok(AgentIdentity(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One ACK row, reduced to the binding it establishes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AckBinding {
    pub bead_id: String,
    pub pane_id: String,
    pub agent: String,
}

/// Parse the ACK grammar out of one comment.
///
/// **Deliberately the LOOSE form.** `ack-stage`'s verifier requires
/// `ACK <token> on <pane> -- `, and measured over the live tracker that strict grammar
/// matches 32 of 52 ACK rows — it MISSES 20 (38%), including this crate's own ACKs, because
/// agents post `ACK <token> on %<pane>` with no trailing clause. A derivation that inherited
/// the strict grammar would silently see two thirds of the evidence.
///
/// The strict form remains correct for its own job: `ack-stage` VERIFIES a binding it already
/// knows, while this DERIVES one it does not.
#[must_use]
pub fn parse_ack(text: &str) -> Option<(String, String)> {
    let rest = text.trim_start().strip_prefix("ACK ")?;
    let (token, rest) = rest.split_once(" on ")?;
    if token.trim().is_empty() {
        return None;
    }
    let pane: String = rest
        .trim_start()
        .chars()
        .take_while(|c| *c == '%' || c.is_ascii_digit())
        .collect();
    if pane.len() < 2 || !pane.starts_with('%') {
        return None;
    }
    Some((token.trim().to_owned(), pane))
}

/// Derive every binding one bead's comments establish.
///
/// A comment whose author is the git/`$USER` default contributes NOTHING. Without that
/// exclusion the live data reports every pane as ambiguous — measured.
#[must_use]
pub fn derive_bindings(
    bead_id: &str,
    comments: &[(String, String)],
    defaults: &BTreeSet<String>,
) -> BTreeSet<AckBinding> {
    let mut out = BTreeSet::new();
    for (author, text) in comments {
        let author = author.trim();
        if author.is_empty() || defaults.contains(&author.to_lowercase()) {
            continue;
        }
        if let Some((_token, pane)) = parse_ack(text) {
            out.insert(AckBinding {
                bead_id: bead_id.to_owned(),
                pane_id: pane,
                agent: author.to_owned(),
            });
        }
    }
    out
}

/// The roster, DERIVED from every binding in the tracker. There is no setter.
#[must_use]
pub fn roster_from(bindings: &BTreeSet<AckBinding>) -> BTreeSet<String> {
    bindings.iter().map(|b| b.agent.clone()).collect()
}

/// The pane→agent map, derived. Multiple agents on one pane is reported, never resolved.
#[must_use]
pub fn pane_map(bindings: &BTreeSet<AckBinding>) -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for b in bindings {
        map.entry(b.pane_id.clone())
            .or_default()
            .insert(b.agent.clone());
    }
    map
}

/// The two illegal states of the claim machine.
///
/// Both fields must move together in BOTH directions. `--assignee X` alone makes the first;
/// `--assignee ""` alone makes the second; `--status open` alone makes the first. The
/// orchestrator created the second FIVE times while repairing the first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateFinding {
    /// `open` + assigned. Refuses every dispatch to every pane: the bead advertises
    /// availability while naming a holder.
    HalfClaim { bead_id: String, assignee: String },
    /// `in_progress` + unassigned. In-flight work no agent owns, so no follow-up detector
    /// can watch it and no operator can be asked.
    OrphanClaim { bead_id: String },
}

impl fmt::Display for StateFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateFinding::HalfClaim { bead_id, assignee } => write!(
                f,
                "ILLEGAL_STATE bead={bead_id} kind=HALF_CLAIM status=open assignee={assignee} \
                 detail=\"advertises availability while naming a holder; refuses every \
                 dispatch\" next_action=br-update---status-in_progress---assignee-{assignee}"
            ),
            StateFinding::OrphanClaim { bead_id } => write!(
                f,
                "ILLEGAL_STATE bead={bead_id} kind=ORPHAN_CLAIM status=in_progress \
                 assignee=<none> detail=\"in-flight work no agent owns; no follow-up \
                 detector can see it\" next_action=br-update---status-open"
            ),
        }
    }
}

/// Classify one bead's `(status, assignee)` pair. `None` is a legal pair.
#[must_use]
pub fn classify_state(bead_id: &str, status: &str, assignee: &str) -> Option<StateFinding> {
    let assigned = !assignee.trim().is_empty();
    match (status.trim(), assigned) {
        ("open", true) => Some(StateFinding::HalfClaim {
            bead_id: bead_id.to_owned(),
            assignee: assignee.trim().to_owned(),
        }),
        ("in_progress", false) => Some(StateFinding::OrphanClaim {
            bead_id: bead_id.to_owned(),
        }),
        _ => None,
    }
}

/// One bead as the tracker reports it, plus its comments.
#[derive(Debug, Clone)]
pub struct BeadRow {
    pub id: String,
    pub status: String,
    pub assignee: String,
    /// `(author, text)` pairs, in tracker order.
    pub comments: Vec<(String, String)>,
}

/// A pane's liveness, and WHERE the answer came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Liveness {
    /// `tick-monitor`'s verdict for the pane, verbatim.
    pub state: String,
    /// Provenance. Only [`LIVENESS_SOURCE`] is accepted; see [`HolderVerdict`].
    pub source: String,
}

/// What can be said about who holds a bead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolderVerdict {
    /// Exactly one agent, one pane, and a liveness answer from the accepted source.
    Bound {
        bead_id: String,
        agent: String,
        pane_id: String,
        liveness: Liveness,
    },
    /// The evidence names more than one `(pane, agent)`. **Both candidates are carried and
    /// neither is picked** — the orchestrator's silent pick is what produced two agents on
    /// one pane in the first place.
    Ambiguous {
        bead_id: String,
        candidates: Vec<(String, String)>,
    },
    /// The assignee resolves, but no ACK row binds the bead to a pane, so liveness is
    /// unreachable. NOT an error: the ACK protocol went live 2026-09-02 and most in-flight
    /// beads predate it. Distinct from `Bound` so it can never be counted as answered.
    NoPaneEvidence { bead_id: String, assignee: String },
    /// The assignee is not an agent identity.
    AssigneeRefused {
        bead_id: String,
        refusal: IdentityRefusal,
    },
    /// One of the two illegal states. Reported instead of a holder, because a bead in an
    /// illegal state has no well-defined holder to report.
    Illegal(StateFinding),
    /// A pane is bound but `tick-monitor` was not consulted, or answered from a source this
    /// crate refuses. **A roster's `last_active` lands here**, never in `Bound`.
    LivenessUnavailable {
        bead_id: String,
        agent: String,
        pane_id: String,
        detail: String,
    },
}

impl HolderVerdict {
    /// Does this verdict answer the question the bead asks?
    #[must_use]
    pub fn is_answered(&self) -> bool {
        matches!(self, HolderVerdict::Bound { .. })
    }

    /// The stable machine label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            HolderVerdict::Bound { .. } => "bound",
            HolderVerdict::Ambiguous { .. } => "ambiguous",
            HolderVerdict::NoPaneEvidence { .. } => "no_pane_evidence",
            HolderVerdict::AssigneeRefused { .. } => "assignee_refused",
            HolderVerdict::Illegal(_) => "illegal_state",
            HolderVerdict::LivenessUnavailable { .. } => "liveness_unavailable",
        }
    }
}

impl fmt::Display for HolderVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HolderVerdict::Bound {
                bead_id,
                agent,
                pane_id,
                liveness,
            } => write!(
                f,
                "HOLDER bead={bead_id} agent={agent} pane={pane_id} liveness={} source={}",
                liveness.state, liveness.source
            ),
            HolderVerdict::Ambiguous {
                bead_id,
                candidates,
            } => write!(
                f,
                "HOLDER_AMBIGUOUS bead={bead_id} candidates=[{}] \
                 detail=\"two bindings; naming both rather than guessing\"",
                candidates
                    .iter()
                    .map(|(pane, agent)| format!("{agent}@{pane}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            HolderVerdict::NoPaneEvidence { bead_id, assignee } => write!(
                f,
                "HOLDER_NO_PANE bead={bead_id} assignee={assignee} \
                 detail=\"no ACK row binds this bead to a pane, so liveness is unreachable\" \
                 next_action=ack-on-your-pane"
            ),
            HolderVerdict::AssigneeRefused { bead_id, refusal } => {
                write!(f, "bead={bead_id} {refusal}")
            }
            HolderVerdict::Illegal(finding) => write!(f, "{finding}"),
            HolderVerdict::LivenessUnavailable {
                bead_id,
                agent,
                pane_id,
                detail,
            } => write!(
                f,
                "HOLDER_LIVENESS_UNAVAILABLE bead={bead_id} agent={agent} pane={pane_id} \
                 detail=\"{detail}\" next_action=consult-{LIVENESS_SOURCE}"
            ),
        }
    }
}

/// Resolve one bead to its holder.
///
/// `liveness_of` is the caller's window onto `tick-monitor`; the lib stays pure so the
/// resolution is testable without tmux. A `Liveness` whose `source` is not
/// [`LIVENESS_SOURCE`] is REFUSED into [`HolderVerdict::LivenessUnavailable`] — that is how
/// "a stale `last_active` is not an acceptable source" becomes mechanical rather than
/// advisory.
pub fn resolve(
    bead: &BeadRow,
    roster: &BTreeSet<String>,
    defaults: &BTreeSet<String>,
    liveness_of: &dyn Fn(&str) -> Option<Liveness>,
) -> HolderVerdict {
    if let Some(finding) = classify_state(&bead.id, &bead.status, &bead.assignee) {
        return HolderVerdict::Illegal(finding);
    }
    let agent = match AgentIdentity::parse(&bead.assignee, roster, defaults) {
        Ok(agent) => agent,
        Err(refusal) => {
            return HolderVerdict::AssigneeRefused {
                bead_id: bead.id.clone(),
                refusal,
            }
        }
    };

    let bindings = derive_bindings(&bead.id, &bead.comments, defaults);
    let distinct: BTreeSet<(String, String)> = bindings
        .iter()
        .map(|b| (b.pane_id.clone(), b.agent.clone()))
        .collect();
    if distinct.len() > 1 {
        return HolderVerdict::Ambiguous {
            bead_id: bead.id.clone(),
            candidates: distinct.into_iter().collect(),
        };
    }
    let Some((pane_id, ack_agent)) = distinct.into_iter().next() else {
        return HolderVerdict::NoPaneEvidence {
            bead_id: bead.id.clone(),
            assignee: agent.as_str().to_owned(),
        };
    };
    if ack_agent != agent.as_str() {
        // The assignee and the ACK disagree about who holds it. Two claims, one bead — the
        // same shape as two agents on one pane, so it gets the same typed treatment.
        return HolderVerdict::Ambiguous {
            bead_id: bead.id.clone(),
            candidates: vec![
                (pane_id, ack_agent),
                ("<assignee-field>".to_owned(), agent.as_str().to_owned()),
            ],
        };
    }

    match liveness_of(&pane_id) {
        Some(liveness) if liveness.source == LIVENESS_SOURCE => HolderVerdict::Bound {
            bead_id: bead.id.clone(),
            agent: agent.as_str().to_owned(),
            pane_id,
            liveness,
        },
        Some(liveness) => HolderVerdict::LivenessUnavailable {
            bead_id: bead.id.clone(),
            agent: agent.as_str().to_owned(),
            pane_id,
            detail: format!(
                "liveness came from {:?}, and only {LIVENESS_SOURCE} is accepted; a roster \
                 last_active was measured 15h wrong on a committing agent",
                liveness.source
            ),
        },
        None => HolderVerdict::LivenessUnavailable {
            bead_id: bead.id.clone(),
            agent: agent.as_str().to_owned(),
            pane_id,
            detail: "tick-monitor reported nothing for this pane".to_owned(),
        },
    }
}

/// Why an audit could not be performed. An audit that cannot run must not report health.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// **ANTI-VACUITY.** Zero in-flight beads reads identically to a tracker whose query
    /// broke, so it is an ERROR and never a clean bill.
    EmptyScan,
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::EmptyScan => f.write_str(
                "HOLDER_AUDIT_ERROR reason=EMPTY_SCAN detail=\"zero in-flight beads is \
                 indistinguishable from a broken query, so it is not a pass\" \
                 next_action=check-br-list--a---json",
            ),
        }
    }
}

/// The audit of every in-flight bead.
#[derive(Debug, Clone)]
pub struct Audit {
    pub verdicts: Vec<HolderVerdict>,
}

impl Audit {
    /// How many beads answered the question end to end.
    #[must_use]
    pub fn answered(&self) -> usize {
        self.verdicts.iter().filter(|v| v.is_answered()).count()
    }

    /// Counts per verdict label, so the summary is derived rather than hand-tallied.
    #[must_use]
    pub fn by_label(&self) -> BTreeMap<&'static str, usize> {
        let mut out = BTreeMap::new();
        for verdict in &self.verdicts {
            *out.entry(verdict.label()).or_default() += 1;
        }
        out
    }
}

/// Audit every in-flight bead. `beads` is the FULL tracker, so `open`+assigned is visible.
pub fn audit(
    beads: &[BeadRow],
    roster: &BTreeSet<String>,
    defaults: &BTreeSet<String>,
    liveness_of: &dyn Fn(&str) -> Option<Liveness>,
) -> Result<Audit, AuditError> {
    let subject: Vec<&BeadRow> = beads
        .iter()
        .filter(|b| {
            b.status.trim() == "in_progress"
                || classify_state(&b.id, &b.status, &b.assignee).is_some()
        })
        .collect();
    if subject.is_empty() {
        return Err(AuditError::EmptyScan);
    }
    Ok(Audit {
        verdicts: subject
            .into_iter()
            .map(|bead| resolve(bead, roster, defaults, liveness_of))
            .collect(),
    })
}
