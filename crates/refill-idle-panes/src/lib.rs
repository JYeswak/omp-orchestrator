#![forbid(unsafe_code)]

//! Refill every idle pane from the bv DAG — the pure decision layer.
//!
//! # Why this crate exists
//!
//! Joshua, 2026-08-27: *"why are you and all of your agents idle? you are the one
//! building this process and you can't stay busy."* Measured at that moment: **3 panes
//! idle, 334 actionable beads, a 316-deep ready queue** — and the controller was
//! hand-writing one dispatch packet at a time. A controller that must compose prose
//! before every dispatch will always starve its own fleet.
//!
//! Then, immediately after: *"we've been migrating all .sh to rust in
//! /ntm-fleet-monitor how do you keep forgetting this"*. He was right — the first cut
//! of this shipped as 230 lines of shell into a repo whose dispatch chain is
//! **30 components, all `status = "verified"` Rust**. `bin/refill-idle-panes.sh`
//! remains as the differential oracle, the same contract every other row in
//! `registries/dispatch_chain_migration.toml` follows.
//!
//! # The decision, and why it is pure
//!
//! Everything here is a function of text already captured. No process spawning, no
//! clock, no filesystem. That is deliberate: the shell version could only be tested by
//! running it against a live fleet, which is why its riskiest rule — the two-surface
//! intersection — had no unit coverage at all. Here every rule is a fixture.
//!
//! # The safety property that is CORRECT and is preserved
//!
//! **Two surfaces disagree, and dispatching on the wrong one sends real work into a
//! void.** Measured 2026-08-27: `ntm --robot-activity` reported control-plane pane 4
//! `safe_to_dispatch: true` while `pane-dispatch-ready` reported `NO_AGENT — bare
//! shell`. The oracle was right; that pane had no agent process and could never have
//! received a packet. So a pane one surface CONFIDENTLY calls busy is never dispatched
//! on the strength of the other, and an unreadable probe yields zero candidates.
//!
//! # The defect that was fixed, and why the old rule was only half right
//!
//! MEASURED 2026-09-02 03:14:55Z, live, `ntm --robot-activity=omp-orchestrator`:
//!
//! ```text
//!   pane  agent_type  state    confidence  observation_state  safe_to_dispatch   pane-dispatch-ready
//!   2     codex       UNKNOWN  0.5         working            false              FREE
//!   3     codex       UNKNOWN  0.5         working            false              FREE
//! ```
//!
//! `ntm` cannot classify a codex pane. It says so — `state=UNKNOWN, confidence=0.5` —
//! and then **coerces that UNKNOWN into `observation_state=working,
//! safe_to_dispatch=false`**. The previous rule here read `safe_to_dispatch` as a state
//! assertion and intersected it with the oracle's `FREE`, so for a codex pane the
//! conjunction was NEVER true: **refill could not dispatch to a codex pane, by
//! construction.** Three of four workers in that session were codex. Every refill in
//! that session came from the operator by hand.
//!
//! `docs/contracts/pane_observation_contract.md` L2 states the law one direction over:
//! "Unknown is a first-class value and NEVER coerces to Idle". This is its mirror image
//! and equally fatal — **UNKNOWN must not coerce to WORKING either. An unclassifiable
//! pane is not a busy pane.**
//!
//! So the observation is three-valued ([`Observation`]), and the join
//! ([`resolve`]) is:
//!
//! * both surfaces confidently Idle -> dispatch;
//! * one CONFIRMED Idle, the other UNKNOWN -> dispatch. This is not "silently preferring
//!   a surface": UNKNOWN is not a competing claim, so there is nothing to prefer over;
//! * both UNKNOWN -> `Unknowable`, a typed nonzero refusal, never "nothing to do";
//! * both confident and disagreeing -> `Conflict`, held AND reported nonzero.
//!
//! # What routes through `oracle-compare`, and what does not
//!
//! Two comparisons EXECUTE the shared kernel rather than being handrolled beside it:
//!
//! * [`measurability_verdict`] compares the two ROSTERS with `empty_oracle_is_error`
//!   and `empty_product_is_disagreement` on. A live session cannot have an empty
//!   roster, so an empty arm is a broken probe (ntm#254), never consensus.
//! * [`conflict_verdict`] compares the two surfaces' per-pane verdicts over the
//!   confidently-observed domain with `disagree_is_finding` on.
//!
//! The three-valued JOIN itself is not routed, because `oracle-compare` has no
//! three-valued join to route it through — it compares two arms, it does not resolve
//! them. That is stated rather than papered over.

use std::collections::{BTreeMap, BTreeSet};

use oracle_compare::{compare_sets, set_delta, OracleCompareRules, OracleCompareVerdict, SetArm};

/// A three-valued observation of one pane from ONE surface.
///
/// `Unknown` is inhabited on purpose. Two-valued logic has nowhere to put "I could not
/// classify this", so it lands in whichever value is the surface's default — and that
/// default silently becomes an assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// The surface confidently reports the pane is free to receive work.
    Idle,
    /// The surface confidently reports the pane cannot receive work now — busy, or a
    /// bare shell with no agent to receive anything.
    Busy,
    /// The surface could not classify the pane. NOT evidence of either other value.
    Unknown,
}

impl Observation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Unknown => "unknown",
        }
    }
    pub const fn is_confident(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// One surface's roster: every pane it enumerated, with its three-valued observation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceView {
    pub panes: BTreeMap<String, Observation>,
}

impl SurfaceView {
    /// Every pane this surface enumerated, whatever it said about them.
    pub fn roster(&self) -> BTreeSet<String> {
        self.panes.keys().cloned().collect()
    }
    /// The panes this surface actually classified.
    pub fn confident(&self) -> BTreeSet<String> {
        self.panes
            .iter()
            .filter(|(_, o)| o.is_confident())
            .map(|(p, _)| p.clone())
            .collect()
    }
    /// A pane this surface never enumerated is UNKNOWN to it, not busy.
    pub fn get(&self, pane: &str) -> Observation {
        self.panes.get(pane).copied().unwrap_or(Observation::Unknown)
    }
}

/// `ntm` publishes this confidence when its classifier did not classify the pane.
///
/// Measured 2026-09-02: every codex pane in the live fleet carried
/// `state=UNKNOWN, confidence=0.5`, and `ntm` then rendered
/// `observation_state=working, safe_to_dispatch=false` from it.
pub const NTM_UNCLASSIFIED_CONFIDENCE: f64 = 0.5;

/// Parse `ntm --robot-activity=<session>` into a three-valued view.
///
/// Returns `None` on unparseable input or a missing `agents` array — malformed is not
/// "nothing is free". `safe_to_dispatch` is read ONLY when the classifier was
/// confident; otherwise the pane is `Unknown` and the other surface decides.
pub fn parse_activity_view(text: &str) -> Option<SurfaceView> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let agents = value.get("agents")?.as_array()?;
    let mut panes = BTreeMap::new();
    for agent in agents {
        let Some(pane) = pane_id(agent.get("pane")) else {
            continue;
        };
        panes.insert(pane, activity_observation(agent));
    }
    Some(SurfaceView { panes })
}

/// Classify one `agents[]` entry, refusing to read a coerced UNKNOWN as an assertion.
fn activity_observation(agent: &serde_json::Value) -> Observation {
    let state = agent.get("state").and_then(serde_json::Value::as_str);
    if state.is_none() || state == Some("UNKNOWN") {
        return Observation::Unknown;
    }
    if let Some(confidence) = agent.get("confidence").and_then(serde_json::Value::as_f64) {
        if confidence <= NTM_UNCLASSIFIED_CONFIDENCE {
            return Observation::Unknown;
        }
    }
    match agent
        .get("safe_to_dispatch")
        .and_then(serde_json::Value::as_bool)
    {
        Some(true) => Observation::Idle,
        Some(false) => Observation::Busy,
        None => Observation::Unknown,
    }
}

/// Parse `pane-dispatch-ready <session> --json` into a three-valued view.
///
/// The key is `state`, and that is load-bearing: a fixture written against `status`
/// parses to zero classified panes while looking plausible, and every assertion built
/// on it certifies a payload shape production never emits.
///
/// `FREE` is Idle. `BUSY` and `NO_AGENT` are both confident refusals — a bare shell
/// cannot receive a packet, and treating it as free is one of the named-forbidden
/// moves in `xtask check-product`. Any state this parser does not recognise is
/// `Unknown` rather than being folded into one of the two it does.
pub fn parse_oracle_view(text: &str) -> Option<SurfaceView> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let entries = value.get("panes")?.as_array()?;
    let mut panes = BTreeMap::new();
    for entry in entries {
        let Some(pane) = pane_id(entry.get("pane")) else {
            continue;
        };
        let observation = match entry.get("state").and_then(serde_json::Value::as_str) {
            Some("FREE") => Observation::Idle,
            Some("BUSY" | "NO_AGENT") => Observation::Busy,
            _ => Observation::Unknown,
        };
        panes.insert(pane, observation);
    }
    Some(SurfaceView { panes })
}

/// Pane ids arrive as either `"2"` or `2` depending on the surface. Accept both rather
/// than silently dropping one form — a dropped pane reads as a busy fleet.
fn pane_id(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// What the two surfaces jointly establish about one pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// At least one surface confidently says idle and neither confidently contradicts.
    Dispatch,
    /// Some surface confidently says the pane cannot receive work.
    Hold,
    /// Both surfaces are confident and they disagree. A finding, not a quiet skip.
    Conflict,
    /// Neither surface could classify the pane. We cannot tell — and cannot say "busy".
    Unknowable,
}

impl Resolution {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Hold => "hold",
            Self::Conflict => "conflict",
            Self::Unknowable => "unknowable",
        }
    }
}

/// The three-valued join. See the module header for why UNKNOWN yields to CONFIRMED.
pub const fn resolve(activity: Observation, oracle: Observation) -> Resolution {
    match (activity, oracle) {
        (Observation::Idle, Observation::Idle) => Resolution::Dispatch,
        (Observation::Idle, Observation::Unknown) | (Observation::Unknown, Observation::Idle) => {
            Resolution::Dispatch
        }
        (Observation::Idle, Observation::Busy) | (Observation::Busy, Observation::Idle) => {
            Resolution::Conflict
        }
        (Observation::Unknown, Observation::Unknown) => Resolution::Unknowable,
        _ => Resolution::Hold,
    }
}

/// Every pane either surface enumerated, with its joint resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decision {
    pub dispatchable: Vec<String>,
    pub conflicts: Vec<String>,
    pub unknowable: Vec<String>,
    pub held: Vec<String>,
}

impl Decision {
    /// Panes actually observed by at least one surface.
    pub fn observed(&self) -> usize {
        self.dispatchable.len() + self.conflicts.len() + self.unknowable.len() + self.held.len()
    }
}

/// Sort numerically where possible so a run is reproducible and diffable.
fn sort_panes(panes: &mut [String]) {
    panes.sort_by(|a, b| match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.cmp(b),
    });
}

/// Resolve every pane in the union of both rosters.
pub fn decide(activity: &SurfaceView, oracle: &SurfaceView) -> Decision {
    let mut decision = Decision::default();
    let mut union: BTreeSet<String> = activity.roster();
    union.extend(oracle.roster());
    for pane in union {
        let bucket = match resolve(activity.get(&pane), oracle.get(&pane)) {
            Resolution::Dispatch => &mut decision.dispatchable,
            Resolution::Conflict => &mut decision.conflicts,
            Resolution::Unknowable => &mut decision.unknowable,
            Resolution::Hold => &mut decision.held,
        };
        bucket.push(pane);
    }
    sort_panes(&mut decision.dispatchable);
    sort_panes(&mut decision.conflicts);
    sort_panes(&mut decision.unknowable);
    sort_panes(&mut decision.held);
    decision
}

/// Rules for the ROSTER comparison.
///
/// `empty_oracle_is_error` and `empty_product_is_disagreement` are the load-bearing
/// pair: a live session cannot have an empty pane roster, so an empty arm is a broken
/// probe and never consensus that there is no work.
///
/// `disagree_is_finding` is OFF here, and that is a deliberate scope limit rather than
/// the blinding mutation `oracle-compare`'s header warns about. The rosters legitimately
/// differ: `pane-dispatch-ready` enumerates every pane in the session including bare
/// shells, `ntm --robot-activity` enumerates only panes it identified an agent in.
/// Non-empty roster differences are therefore expected, and the CONTENT disagreement
/// question is asked separately and with the rule ON by [`conflict_verdict`].
pub fn measurability_rules() -> OracleCompareRules {
    OracleCompareRules {
        disagree_is_finding: false,
        empty_oracle_is_error: true,
        unreadable_is_error: true,
        empty_product_is_disagreement: true,
    }
}

/// Route the "can these two surfaces be compared at all?" question through the kernel.
///
/// `pane-dispatch-ready` is the oracle arm (it inspects the pane directly);
/// `ntm --robot-activity` is the product arm (it is a projection).
pub fn measurability_verdict(activity_text: &str, oracle_text: &str) -> OracleCompareVerdict {
    let product = parse_activity_view(activity_text)
        .map_or(SetArm::Unreadable, |v| SetArm::Value(v.roster()));
    let oracle =
        parse_oracle_view(oracle_text).map_or(SetArm::Unreadable, |v| SetArm::Value(v.roster()));
    compare_sets(oracle, product, &measurability_rules())
}

/// Rules for the per-pane CONTENT comparison over the confidently-observed domain.
///
/// `disagree_is_finding` ON: within that domain both surfaces have committed, so a
/// difference is a real finding. The empty rules are OFF because an empty confident
/// domain is the honest "nothing comparable yet" answer, and is reported by
/// [`Decision::unknowable`] instead.
pub fn conflict_rules() -> OracleCompareRules {
    OracleCompareRules {
        disagree_is_finding: true,
        empty_oracle_is_error: false,
        unreadable_is_error: true,
        empty_product_is_disagreement: false,
    }
}

/// `pane=verdict` labels, so an all-busy domain is still a NON-EMPTY set.
///
/// Comparing bare idle-sets would make "oracle says nothing is idle, ntm says pane 2
/// is" fall into `compare_sets`' empty-oracle branch and read as agreement. Labelling
/// each pane with its verdict keeps both arms inhabited whenever the domain is, so a
/// disagreement is a disagreement wherever it sits.
fn verdict_labels(view: &SurfaceView, domain: &BTreeSet<String>) -> BTreeSet<String> {
    domain
        .iter()
        .map(|pane| format!("{pane}={}", view.get(pane).as_str()))
        .collect()
}

/// The domain on which both surfaces have actually committed to a classification.
pub fn confident_domain(activity: &SurfaceView, oracle: &SurfaceView) -> BTreeSet<String> {
    activity
        .confident()
        .intersection(&oracle.confident())
        .cloned()
        .collect()
}

/// Route the "do the two surfaces contradict each other?" question through the kernel.
pub fn conflict_verdict(activity: &SurfaceView, oracle: &SurfaceView) -> OracleCompareVerdict {
    let domain = confident_domain(activity, oracle);
    compare_sets(
        SetArm::Value(verdict_labels(oracle, &domain)),
        SetArm::Value(verdict_labels(activity, &domain)),
        &conflict_rules(),
    )
}

/// Name the panes the two confident surfaces contradict each other on.
pub fn conflicting_panes(activity: &SurfaceView, oracle: &SurfaceView) -> Vec<String> {
    let domain = confident_domain(activity, oracle);
    let oracle_labels = verdict_labels(oracle, &domain);
    let activity_labels = verdict_labels(activity, &domain);
    let (oracle_only, _) = set_delta(&oracle_labels, &activity_labels);
    let mut panes: Vec<String> = oracle_only
        .iter()
        .filter_map(|label| label.split_once('=').map(|(pane, _)| pane.to_string()))
        .collect();
    sort_panes(&mut panes);
    panes
}

/// A named outcome the caller renders and exits on.
///
/// `code` is the process exit code. A nonzero exit the operator must answer is the only
/// signal measured to reach a human; `println!` at exit 0 is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefillOutcome {
    pub message: String,
    pub code: u8,
}

/// Turn a non-agreeing roster verdict into a typed nonzero refusal.
///
/// The message NAMES THE PROBE, because "surfaces disagree" sends the reader nowhere.
pub fn measurability_refusal(verdict: &OracleCompareVerdict) -> Option<RefillOutcome> {
    match verdict {
        OracleCompareVerdict::Agree { .. } => None,
        OracleCompareVerdict::Unmeasurable { why } => Some(RefillOutcome {
            message: format!(
                "refill: UNMEASURABLE detector=pane_roster why={why} \
                 probe=`ntm --robot-activity=<session>` vs `pane-dispatch-ready <session> --json` \
                 remedy=an empty or unreadable roster is a broken probe, never a quiet fleet"
            ),
            code: 2,
        }),
        OracleCompareVerdict::Disagree {
            oracle_n,
            product_n,
        } => Some(RefillOutcome {
            message: format!(
                "refill: SURFACE_DISAGREEMENT detector=pane_roster \
                 oracle=`pane-dispatch-ready` panes={oracle_n} \
                 product=`ntm --robot-activity` panes={product_n} \
                 remedy=the ntm projection lost the roster (ntm#254 class); refusing rather than \
                 reading an empty projection as an idle-free fleet"
            ),
            code: 1,
        }),
    }
}

/// Turn a resolved fleet into the run's outcome.
///
/// Ordering is deliberate. A conflict or an unknowable pane is reported even when other
/// panes ARE dispatchable — the fleet still gets fed, and the operator still gets a
/// nonzero exit naming what could not be established. Starving the fleet to report a
/// problem is the failure this crate exists to end; hiding the problem to feed it is the
/// failure the report exists to end.
pub fn run_outcome(decision: &Decision, conflict: &OracleCompareVerdict) -> RefillOutcome {
    if !decision.conflicts.is_empty() {
        return RefillOutcome {
            message: format!(
                "refill: SURFACE_CONFLICT detector=pane_state panes={:?} verdict={conflict:?} \
                 remedy=both surfaces are confident and contradict each other; one classifier is \
                 wrong and neither may be trusted for these panes",
                decision.conflicts
            ),
            code: 1,
        };
    }
    if decision.dispatchable.is_empty() && !decision.unknowable.is_empty() {
        return RefillOutcome {
            message: format!(
                "refill: UNMEASURABLE detector=pane_state_unknown_on_both_surfaces \
                 panes={:?} probe=`ntm --robot-activity` reported state=UNKNOWN at \
                 confidence<={NTM_UNCLASSIFIED_CONFIDENCE} and `pane-dispatch-ready` could not \
                 classify them either \
                 remedy=an unclassifiable pane is NOT a busy pane; fix the classifier or read the \
                 panes by hand — this is not 'nothing to do'",
                decision.unknowable
            ),
            code: 2,
        };
    }
    if decision.dispatchable.is_empty() {
        return RefillOutcome {
            message: format!(
                "refill: every observed pane is confidently unavailable (observed={}, held={}) \
                 — genuine no-work",
                decision.observed(),
                decision.held.len()
            ),
            code: 0,
        };
    }
    RefillOutcome {
        message: format!(
            "refill: {} dispatchable pane(s) {:?} (held={}, unknowable={})",
            decision.dispatchable.len(),
            decision.dispatchable,
            decision.held.len(),
            decision.unknowable.len()
        ),
        code: 0,
    }
}

/// Convert a non-PASS fleet reconciliation into a named refill refusal.
///
/// Refill must not turn an NTM/tmux disagreement into the healthy no-work answer.
pub fn reconciliation_failure(verdict: &fleet_reconcile::InnerVerdict) -> Option<String> {
    if verdict.verdict == "PASS" {
        return None;
    }
    Some(format!(
        "refill: SURFACE_DISAGREEMENT detector={} verdict={} tmux={} ntm={} detail={}",
        verdict.detector, verdict.verdict, verdict.tmux_count, verdict.ntm_count, verdict.detail
    ))
}

/// Parse `bv --robot-triage` recommendations into descending-score bead ids.
///
/// `quick_ref.top_picks` is a weaker summary and does not carry the ranking score
/// that dispatch needs. Recommendations are sorted explicitly so this parser does
/// not silently inherit a different envelope order.
pub fn parse_recommendations(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(recommendations) = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };

    let mut ranked = recommendations
        .iter()
        .enumerate()
        .filter_map(|(position, recommendation)| {
            let id = recommendation.get("id")?.as_str()?;
            let score = recommendation.get("score")?.as_f64()?;
            score.is_finite().then(|| (score, position, id.to_string()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    ranked.into_iter().map(|(_, _, id)| id).collect()
}

/// One pane paired with the bead it should receive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    pub pane: String,
    pub bead: String,
}

/// Pair dispatchable panes with ranked beads, capped.
///
/// Zip semantics: the shorter side bounds it. Fewer beads than panes leaves panes idle
/// (correct — there is no work); fewer panes than beads leaves beads queued (correct —
/// there is nowhere to put them). Neither is an error.
pub fn plan(panes: &[String], beads: &[String], max: usize) -> Vec<Assignment> {
    panes
        .iter()
        .zip(beads.iter())
        .take(max)
        .map(|(pane, bead)| Assignment {
            pane: pane.clone(),
            bead: bead.clone(),
        })
        .collect()
}

/// Minimum bytes for a packet to be worth sending.
///
/// A worker handed a title and no spec invents the rest, and invents it differently
/// every time. Refusing is strictly better than dispatching a stub.
pub const MIN_PACKET_BYTES: usize = 400;

/// Is this rendered packet substantial enough to send?
pub const fn packet_is_sendable(bytes: usize) -> bool {
    bytes > MIN_PACKET_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VERBATIM from `ntm --robot-activity=omp-orchestrator`, captured 2026-09-02
    /// 03:14:55Z on the live fleet. Panes 2 and 3 are codex workers. This is the
    /// known-bad: under the previous two-valued rule neither could ever be dispatched.
    const LIVE_ACTIVITY_2026_09_02: &str = r#"{
      "success": true,
      "session": "omp-orchestrator",
      "agents": [
        {"pane":"1","agent_type":"claude","state":"UNKNOWN","confidence":0.5,
         "observation_state":"idle","safe_to_dispatch":true},
        {"pane":"2","agent_type":"codex","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","safe_to_dispatch":false},
        {"pane":"3","agent_type":"codex","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","safe_to_dispatch":false},
        {"pane":"4","agent_type":"omp-glm","state":"ERROR","confidence":0.95,
         "observation_state":"working","safe_to_dispatch":false},
        {"pane":"5","agent_type":"omp-glm","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","safe_to_dispatch":false}
      ]}"#;

    /// VERBATIM from `pane-dispatch-ready omp-orchestrator --json`, same minute.
    ///
    /// The key is `state`. A fixture written against `status` parses to zero classified
    /// panes and certifies a payload shape production never emits.
    const LIVE_ORACLE_2026_09_02: &str = r#"{"schema":"zs.dispatch-ready.v1","panes":[
        {"pane":"0","state":"BUSY"},
        {"pane":"1","state":"BUSY"},
        {"pane":"2","state":"FREE"},
        {"pane":"3","state":"FREE"},
        {"pane":"4","state":"BUSY"},
        {"pane":"5","state":"NO_AGENT"}]}"#;

    fn live_views() -> (SurfaceView, SurfaceView) {
        (
            parse_activity_view(LIVE_ACTIVITY_2026_09_02).expect("fixture parses"),
            parse_oracle_view(LIVE_ORACLE_2026_09_02).expect("fixture parses"),
        )
    }

    /// The oracle fixture must actually CLASSIFY panes. A drifted key would make every
    /// assertion below vacuous while every one of them still passed.
    #[test]
    fn the_oracle_fixture_matches_the_shape_the_real_surface_emits() {
        let (_, oracle) = live_views();
        assert_eq!(oracle.get("2"), Observation::Idle);
        assert_eq!(oracle.get("5"), Observation::Busy);
        assert_eq!(
            oracle.confident().len(),
            6,
            "every pane in the capture must be classified; a `status`-keyed fixture yields 0"
        );
    }

    /// THE MEASURED DEFECT, as a test. `ntm` cannot classify codex; the oracle can.
    #[test]
    fn a_codex_pane_ntm_cannot_classify_is_dispatchable_when_the_oracle_confirms_it() {
        let (activity, oracle) = live_views();
        assert_eq!(
            activity.get("2"),
            Observation::Unknown,
            "fixture must model ntm's UNKNOWN — without it this test proves nothing"
        );
        assert_eq!(oracle.get("2"), Observation::Idle);
        let decision = decide(&activity, &oracle);
        assert_eq!(
            decision.dispatchable,
            vec!["2".to_string(), "3".to_string()],
            "a pane the oracle CONFIRMS free must dispatch even though ntm coerced its \
             UNKNOWN into safe_to_dispatch=false"
        );
    }

    /// THE KNOWN-BAD LEG. The previous rule is reproduced here exactly, and must refuse
    /// the very panes the fix dispatches. Without this the repair is unfalsifiable.
    #[test]
    fn the_old_two_valued_rule_refuses_those_same_panes_forever() {
        let value: serde_json::Value =
            serde_json::from_str(LIVE_ACTIVITY_2026_09_02).expect("fixture parses");
        let safe: BTreeSet<String> = value["agents"]
            .as_array()
            .expect("agents")
            .iter()
            .filter(|a| a["safe_to_dispatch"].as_bool() == Some(true))
            .filter_map(|a| pane_id(a.get("pane")))
            .collect();
        let oracle_value: serde_json::Value =
            serde_json::from_str(LIVE_ORACLE_2026_09_02).expect("fixture parses");
        let free: BTreeSet<String> = oracle_value["panes"]
            .as_array()
            .expect("panes")
            .iter()
            .filter(|p| p["state"].as_str() == Some("FREE"))
            .filter_map(|p| pane_id(p.get("pane")))
            .collect();
        assert!(!safe.is_empty() && !free.is_empty(), "both legs must be inhabited");
        let old: Vec<&String> = safe.intersection(&free).collect();
        assert!(
            old.is_empty(),
            "the old intersection must select NOTHING on the live fixture — that is the \
             defect being repaired"
        );
        let (activity, oracle) = live_views();
        assert!(
            !decide(&activity, &oracle).dispatchable.is_empty(),
            "and the new rule must select something, or the two rules are the same rule"
        );
    }

    /// A pane no surface can classify is UNMEASURABLE and exits nonzero. It is NOT
    /// "nothing to do" — that coercion is the whole defect.
    #[test]
    fn a_pane_unknown_on_both_surfaces_is_a_typed_nonzero_refusal() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"UNKNOWN","confidence":0.5,"safe_to_dispatch":false}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"2","state":"WHO_KNOWS"}]}"#).expect("parses");
        let decision = decide(&activity, &oracle);
        assert_eq!(decision.unknowable, vec!["2".to_string()]);
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle));
        assert_eq!(outcome.code, 2, "unknowable must exit NONZERO");
        assert!(outcome.message.contains("UNMEASURABLE"));
        assert!(
            outcome.message.contains("ntm --robot-activity"),
            "the refusal must NAME the probe: {}",
            outcome.message
        );
        assert!(
            !outcome.message.contains("no idle pane both surfaces agree on"),
            "the retired quiet-success line must never render here"
        );
    }

    /// THE PRESERVED CONSERVATISM. 2026-08-27: activity said pane 4 was free, the
    /// oracle said bare shell. A CONFIDENT disagreement is a conflict, never a dispatch.
    #[test]
    fn a_confident_disagreement_is_a_conflict_and_is_never_dispatched() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":"4","state":"IDLE","confidence":0.95,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"4","state":"NO_AGENT"}]}"#).expect("parses");
        let decision = decide(&activity, &oracle);
        assert!(
            decision.dispatchable.is_empty(),
            "a bare shell is never dispatched"
        );
        assert_eq!(decision.conflicts, vec!["4".to_string()]);
        assert_eq!(conflicting_panes(&activity, &oracle), vec!["4".to_string()]);
        let verdict = conflict_verdict(&activity, &oracle);
        assert!(
            matches!(verdict, OracleCompareVerdict::Disagree { .. }),
            "the shared kernel must see it: {verdict:?}"
        );
        let outcome = run_outcome(&decision, &verdict);
        assert_eq!(outcome.code, 1, "a conflict exits NONZERO");
        assert!(outcome.message.contains("SURFACE_CONFLICT"));
    }

    /// A conflict must not starve the panes that ARE established. Report AND feed.
    #[test]
    fn a_conflict_on_one_pane_still_reports_the_panes_that_are_established() {
        let activity = parse_activity_view(
            r#"{"agents":[
                {"pane":"2","state":"IDLE","confidence":0.95,"safe_to_dispatch":true},
                {"pane":"4","state":"IDLE","confidence":0.95,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        let oracle = parse_oracle_view(
            r#"{"panes":[{"pane":"2","state":"FREE"},{"pane":"4","state":"NO_AGENT"}]}"#,
        )
        .expect("parses");
        let decision = decide(&activity, &oracle);
        assert_eq!(decision.dispatchable, vec!["2".to_string()]);
        assert_eq!(decision.conflicts, vec!["4".to_string()]);
    }

    /// ANTI-VACUITY on the positive path: both surfaces confident and agreeing.
    #[test]
    fn a_pane_both_surfaces_confidently_call_free_is_selected() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.95,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"2","state":"FREE"}]}"#).expect("parses");
        let decision = decide(&activity, &oracle);
        assert_eq!(decision.dispatchable, vec!["2".to_string()]);
        assert!(matches!(
            conflict_verdict(&activity, &oracle),
            OracleCompareVerdict::Agree { .. }
        ));
        assert_eq!(
            run_outcome(&decision, &conflict_verdict(&activity, &oracle)).code,
            0
        );
    }

    /// The genuine no-work case survives: every pane confidently busy, exit 0.
    #[test]
    fn a_confidently_busy_fleet_is_genuine_no_work_at_exit_zero() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"THINKING","confidence":0.9,"safe_to_dispatch":false}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#).expect("parses");
        let decision = decide(&activity, &oracle);
        assert!(decision.dispatchable.is_empty());
        assert!(decision.unknowable.is_empty());
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle));
        assert_eq!(outcome.code, 0);
        assert!(outcome.message.contains("genuine no-work"));
    }

    /// An EMPTY ntm roster against a live oracle is a broken probe, routed through
    /// `oracle-compare`'s `empty_product_is_disagreement`. This is the ntm#254 case.
    #[test]
    fn an_empty_ntm_roster_against_a_live_oracle_is_a_typed_nonzero_refusal() {
        let verdict = measurability_verdict(
            r#"{"success":true,"agents":[]}"#,
            r#"{"panes":[{"pane":"2","state":"FREE"},{"pane":"3","state":"FREE"}]}"#,
        );
        let refusal = measurability_refusal(&verdict).expect("an empty projection must refuse");
        assert_eq!(refusal.code, 1);
        assert!(refusal.message.contains("ntm --robot-activity"));
        assert!(!refusal
            .message
            .contains("no idle pane both surfaces agree on"));
    }

    /// And the mirror: an empty ORACLE roster is unmeasurable, not agreement.
    #[test]
    fn an_empty_oracle_roster_is_unmeasurable() {
        let verdict = measurability_verdict(
            r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.9,"safe_to_dispatch":true}]}"#,
            r#"{"panes":[]}"#,
        );
        let refusal = measurability_refusal(&verdict).expect("an empty oracle must refuse");
        assert_eq!(refusal.code, 2);
        assert!(refusal.message.contains("UNMEASURABLE"));
    }

    #[test]
    fn an_unreadable_probe_is_unmeasurable_not_empty() {
        for (a, o) in [
            ("not json", r#"{"panes":[{"pane":"2","state":"FREE"}]}"#),
            (
                r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.9,"safe_to_dispatch":true}]}"#,
                "not json",
            ),
            ("not json", "not json"),
            ("{}", r#"{"panes":[{"pane":"2","state":"FREE"}]}"#),
        ] {
            let refusal = measurability_refusal(&measurability_verdict(a, o))
                .expect("an unreadable probe must refuse");
            assert_eq!(
                refusal.code, 2,
                "unreadable is UNMEASURABLE, not disagreement"
            );
        }
    }

    /// A roster the two surfaces enumerate differently is NOT a failure — the oracle
    /// sees bare shells ntm never reports. Over-strictness here gets the gate routed
    /// around, so it is pinned.
    #[test]
    fn differing_rosters_alone_do_not_refuse() {
        let verdict = measurability_verdict(
            r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.9,"safe_to_dispatch":true}]}"#,
            r#"{"panes":[{"pane":"0","state":"BUSY"},{"pane":"2","state":"FREE"}]}"#,
        );
        assert!(
            measurability_refusal(&verdict).is_none(),
            "a bare shell the projection never reports is not a broken probe: {verdict:?}"
        );
    }

    /// The confident domain excludes UNKNOWN panes, so ntm's coerced `working` on a
    /// codex pane cannot register as a conflict against the oracle's FREE.
    #[test]
    fn an_unknown_arm_never_registers_as_a_conflict() {
        let (activity, oracle) = live_views();
        assert!(!confident_domain(&activity, &oracle).contains("2"));
        assert!(conflicting_panes(&activity, &oracle).is_empty());
        assert!(matches!(
            conflict_verdict(&activity, &oracle),
            OracleCompareVerdict::Agree { .. }
        ));
    }

    /// The label encoding exists so an all-busy oracle arm is still non-empty. Without
    /// it `compare_sets` would take its empty-oracle branch and report agreement while
    /// the product claimed a pane was idle.
    #[test]
    fn a_conflict_is_seen_even_when_the_oracle_calls_every_pane_busy() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.95,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"2","state":"BUSY"}]}"#).expect("parses");
        assert!(matches!(
            conflict_verdict(&activity, &oracle),
            OracleCompareVerdict::Disagree { .. }
        ));
        assert_eq!(conflicting_panes(&activity, &oracle), vec!["2".to_string()]);
    }

    /// A high-confidence non-UNKNOWN state is read as the assertion it is.
    #[test]
    fn a_confident_ntm_state_is_still_believed() {
        let view = parse_activity_view(
            r#"{"agents":[
                {"pane":"2","state":"IDLE","confidence":0.95,"safe_to_dispatch":true},
                {"pane":"3","state":"THINKING","confidence":0.8,"safe_to_dispatch":false}]}"#,
        )
        .expect("parses");
        assert_eq!(view.get("2"), Observation::Idle);
        assert_eq!(view.get("3"), Observation::Busy);
    }

    /// Exactly at the measured threshold the classifier is not believed.
    #[test]
    fn confidence_at_the_unclassified_threshold_is_unknown() {
        let view = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"IDLE","confidence":0.5,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        assert_eq!(view.get("2"), Observation::Unknown);
    }

    /// Pane ids arrive as both string and number across surfaces. Dropping one form
    /// silently shrinks the candidate set, which reads as a busier fleet than exists.
    #[test]
    fn numeric_and_string_pane_ids_both_parse() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":2,"state":"IDLE","confidence":0.9,"safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        let oracle =
            parse_oracle_view(r#"{"panes":[{"pane":"2","state":"FREE"}]}"#).expect("parses");
        assert_eq!(
            decide(&activity, &oracle).dispatchable,
            vec!["2".to_string()]
        );
    }

    #[test]
    fn panes_sort_numerically_not_lexically() {
        let mut panes: Vec<String> = ["10", "2", "3"].iter().map(|s| (*s).to_string()).collect();
        sort_panes(&mut panes);
        assert_eq!(panes, vec!["2", "3", "10"]);
    }

    #[test]
    fn recommendations_parse_in_descending_score_order() {
        let text = r#"{"triage":{
            "quick_ref":{"top_picks":[{"id":"cp-wrong","unblocks":99}]},
            "recommendations":[
                {"id":"cp-low","score":0.2},
                {"id":"cp-high","score":0.9},
                {"id":"cp-no-score"},
                {"id":"cp-mid","score":0.5}
            ]}}"#;
        assert_eq!(
            parse_recommendations(text),
            vec!["cp-high", "cp-mid", "cp-low"]
        );
    }

    #[test]
    fn unparseable_triage_yields_no_recommendations() {
        assert!(parse_recommendations("{}").is_empty());
        assert!(parse_recommendations(
            r#"{"triage":{"quick_ref":{"top_picks":[{"id":"cp-stale"}]}}}"#
        )
        .is_empty());
    }

    #[test]
    fn reconcile_empty_success_with_live_tmux_is_named_failure() {
        let verdict = fleet_reconcile::reconcile_inner(
            "control-plane\n",
            "control-plane\n",
            r#"{"success":true,"summary":{"total_sessions":null},"sessions":[]}"#,
            &fleet_reconcile::FleetReconcileRules::default(),
        );
        let message = reconciliation_failure(&verdict).expect("disagreement must block refill");
        assert!(message.starts_with("refill: SURFACE_DISAGREEMENT"));
        assert!(message.contains("detector=ntm_empty_success_with_live_tmux"));
    }

    #[test]
    fn reconcile_pass_preserves_genuine_no_idle_capacity() {
        let verdict = fleet_reconcile::reconcile_inner(
            "control-plane\n",
            "control-plane\n",
            r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"control-plane"}]}"#,
            &fleet_reconcile::FleetReconcileRules::default(),
        );
        assert_eq!(verdict.verdict, "PASS");
        assert!(reconciliation_failure(&verdict).is_none());
    }

    #[test]
    fn plan_pairs_panes_with_beads_and_respects_the_cap() {
        let panes = vec!["2".to_string(), "3".to_string(), "4".to_string()];
        let beads = vec!["cp-a".to_string(), "cp-b".to_string(), "cp-c".to_string()];
        assert_eq!(plan(&panes, &beads, 8).len(), 3);
        assert_eq!(plan(&panes, &beads, 2).len(), 2);
        // Fewer beads than panes is not an error: there is simply no more work.
        assert_eq!(plan(&panes, &beads[..1], 8).len(), 1);
        // Fewer panes than beads is not an error either.
        assert_eq!(plan(&panes[..1], &beads, 8).len(), 1);
    }

    #[test]
    fn plan_never_assigns_one_bead_to_two_panes() {
        let panes = vec!["2".to_string(), "3".to_string()];
        let beads = vec!["cp-a".to_string(), "cp-b".to_string()];
        let out = plan(&panes, &beads, 8);
        assert_eq!(out[0].bead, "cp-a");
        assert_eq!(out[1].bead, "cp-b");
        assert_ne!(out[0].bead, out[1].bead, "two panes must not race one bead");
    }

    #[test]
    fn an_undersized_packet_is_refused_and_a_full_one_accepted() {
        assert!(!packet_is_sendable(0));
        assert!(!packet_is_sendable(MIN_PACKET_BYTES));
        // ANTI-VACUITY on the size rule: a real packet must pass, or the guard refuses
        // every dispatch while reporting itself healthy.
        assert!(packet_is_sendable(MIN_PACKET_BYTES + 1));
        assert!(packet_is_sendable(7_347));
    }
}
