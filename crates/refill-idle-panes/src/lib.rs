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
//! # The defect that was fixed, and the wrong diagnosis that nearly shipped
//!
//! MEASURED 2026-09-02 03:14:55Z, live, `ntm --robot-activity=omp-orchestrator`
//! alongside `pane-dispatch-ready omp-orchestrator --json`:
//!
//! ```text
//!   pane  agent_type  state     conf  observation_state  obs_conf  safe   pane-dispatch-ready
//!   1     claude      UNKNOWN   0.5   idle               0.95      true   BUSY
//!   2     codex       UNKNOWN   0.5   working            0.95      false  FREE
//!   3     codex       UNKNOWN   0.5   working            0.95      false  FREE
//!   4     omp-glm     ERROR     0.95  working            0.95      false  BUSY
//!   5     omp-glm     UNKNOWN   0.5   working            0.95      false  NO_AGENT
//! ```
//!
//! Panes 2 and 3 are the whole story: `pane-dispatch-ready` CONFIRMS them free while
//! `ntm` reports them working at `observation_confidence: 0.95`. Two surfaces, both
//! confident, flatly contradicting. The old rule intersected `safe_to_dispatch == true`
//! with `FREE`, got the empty set, and printed
//! `refill: no idle pane both surfaces agree on — nothing to do` at **exit 0**. It was
//! refusing CORRECTLY and reporting the refusal as a healthy quiet fleet, so nobody
//! learned that two authorities disagreed about three panes. The operator hand-dispatched
//! all night.
//!
//! **A WRONG DIAGNOSIS ALMOST SHIPPED HERE, and the trap is worth recording because the
//! payload invites it.** The reading was: "ntm cannot classify a codex pane —
//! `state=UNKNOWN, confidence=0.5` — and coerces that UNKNOWN into
//! `safe_to_dispatch=false`". Every row above is consistent with it. It is false.
//! `safe_to_dispatch` never consults `state`. Measured across two captures 23 minutes
//! apart, 10 rows:
//!
//! ```text
//!   safe_to_dispatch == (observation_state == "idle")   10/10
//!   safe_to_dispatch derivable from state                0/10
//! ```
//!
//! and the 03:37:51Z capture settles it in the opposite direction: panes 4 and 5 carried
//! `state=UNKNOWN, confidence=0.5` with `safe_to_dispatch=TRUE`, while panes 2 and 3
//! carried `state=THINKING, confidence=0.8` with `safe_to_dispatch=FALSE`. On that
//! capture `state` is ANTI-correlated with dispatch. A rule gating on `state` would have
//! refused the only panes that were dispatchable.
//!
//! `docs/contracts/pane_observation_contract.md` names the distinction as a law —
//! **PO-L5-DISPATCH-SEPARATE**: dispatch admissibility is a separate field, and "no
//! conversion from `bool`, `safe_to_dispatch`, or a liveness variant is part of this
//! contract". `state` is the last-status-line liveness classifier;
//! `observation_state` is what dispatch reads. This crate reads `observation_state` and
//! deliberately does not read `state`.
//!
//! So the observation is three-valued ([`Observation`]), and the join ([`resolve`]) is:
//!
//! * both surfaces confidently Idle -> dispatch;
//! * one Idle, the other UNKNOWN -> `Unconfirmed`. NEVER dispatched. A lone positive is
//!   not a confirmed free pane, per `pane_readiness_contract` 2.2.1 (609a97b), which was
//!   measured: ntm reported pane 1 `observation_state: "idle"` at
//!   `observation_confidence: 0.95` while two status-line captures 95 seconds apart both
//!   carried a spinner with the timer advancing 26m -> 28m. A confident idle can be
//!   FALSE, and unlike `state=UNKNOWN` at 0.5 it does not announce its own weakness.
//!   UNKNOWN therefore yields to CONFIRMED only in the NEGATIVE direction — a confident
//!   busy on either arm holds the pane. The ntm arm is UNKNOWN when the pane is absent
//!   from its roster, when the evidence is not
//!   `(capture_provenance=live, observation_freshness=fresh)`, or when
//!   `observation_state` is unrecognised — NEVER because `state` said UNKNOWN;
//! * both UNKNOWN -> `Unknowable`, a typed nonzero refusal, never "nothing to do";
//! * both confident and disagreeing -> `Conflict`, held AND reported nonzero. This is
//!   the branch that fires on the capture above, and it is the product change: the
//!   contradiction is now named per pane at a nonzero exit instead of rendering as a
//!   quiet fleet.
//!
//! **WHAT THIS DOES NOT DO:** it does not make panes 2 and 3 dispatchable. While the two
//! surfaces contradict, refusing is right. Unblocking them requires fixing whichever
//! surface is wrong, which is upstream of this crate and unresolved — see
//! `omp-orchestrator-readiness-l3-motion-window-7523` for the leading candidate.
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
        self.panes
            .get(pane)
            .copied()
            .unwrap_or(Observation::Unknown)
    }
}

/// Parse `ntm --robot-activity=<session>` into a three-valued view.
///
/// Returns `None` on unparseable input or a missing `agents` array — malformed is not
/// "nothing is free".
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

/// Is this row's evidence a LIVE, FRESH capture?
///
/// Same pair `ntm-fleet-monitor::freshness` requires (`src/ntm.rs:242`): both
/// `capture_provenance == "live"` and `observation_freshness == "fresh"`. Reusing that
/// shape rather than inventing a confidence floor is deliberate — every row measured on
/// this fleet carried `observation_confidence: 0.95`, so a numeric floor would be a
/// threshold with no observation behind it and a branch that never fires.
///
/// A row that is not live-and-fresh is UNKNOWN, not busy: a stale capture is not
/// evidence of work, it is absence of evidence.
fn evidence_is_live(agent: &serde_json::Value) -> bool {
    let field = |key: &str| {
        agent
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_ascii_lowercase)
    };
    matches!(
        (
            field("capture_provenance").as_deref(),
            field("observation_freshness").as_deref()
        ),
        (Some("live"), Some("fresh"))
    )
}

/// Classify one `agents[]` entry from the field dispatch actually reads.
///
/// **`state` and `confidence` are deliberately NOT consulted.** They are the
/// last-status-line liveness classifier, and `safe_to_dispatch` does not derive from
/// them — measured 10/10 rows across two captures, with the 03:37:51Z capture showing
/// `state=UNKNOWN` panes dispatchable and `state=THINKING` panes not. Gating on `state`
/// refuses the only panes that can receive work. `pane_observation_contract`
/// PO-L5-DISPATCH-SEPARATE is the same rule stated as law.
///
/// `observation_state` is read directly rather than `safe_to_dispatch`, because the
/// boolean cannot express the third value: `safe_to_dispatch=false` renders identically
/// for "this pane is working" and "I could not observe this pane".
fn activity_observation(agent: &serde_json::Value) -> Observation {
    if !evidence_is_live(agent) {
        return Observation::Unknown;
    }
    match agent
        .get("observation_state")
        .and_then(serde_json::Value::as_str)
    {
        Some("idle") => Observation::Idle,
        Some("working") => Observation::Busy,
        _ => Observation::Unknown,
    }
}

/// Is a completed probe's stdout usable as a roster payload?
///
/// # THE MEASURED ROOT CAUSE, 2026-09-03T21:10Z
///
/// ```text
/// $ refill-idle-panes --plan
/// refill-idle-panes: ~/.local/bin/pane-dispatch-ready exited=1
/// refill: UNMEASURABLE detector=pane_roster why=unreadable_arm …
///
/// $ pane-dispatch-ready control-plane --json ; echo "exit=$?"
/// {"schema":"zs.dispatch-ready.v1","panes":[{"pane":"0",…},…,{"pane":"4",…}],"free_count":0}
/// exit=1
/// ```
///
/// A COMPLETE five-pane roster, and exit 1, because `pane-dispatch-ready` exits 1 to mean
/// **zero FREE panes** — `crates/pane-dispatch-ready/src/main.rs:395`:
/// `if free_count > 0 { ExitCode::SUCCESS } else { ExitCode::from(1) }`. The caller
/// discarded stdout on any nonzero exit, so that roster arrived as the empty string and
/// the measurability gate refused. **A semantic exit code was read as an I/O verdict**,
/// and the selector could not run at all whenever the fleet was busy — the exact state it
/// exists to wait out.
///
/// # Why an exit code is not a parameter here
///
/// It is not ignored; it is UNAVAILABLE. A function that cannot see the exit code cannot
/// regress into gating on it, and the next reader cannot reintroduce the conflation by
/// adding one condition.
///
/// Fail-closed is preserved, because readability is still decided downstream by the
/// parsers: `pane-dispatch-ready`'s genuinely-unreadable paths print NOTHING on stdout
/// (`main.rs:270` `tmux not available` exit 2, `main.rs:280` `no tmux sessions` exit 1),
/// so they arrive empty, return `None` here, and still refuse.
pub fn roster_readability(stdout: &str) -> Option<&str> {
    if stdout.trim().is_empty() {
        None
    } else {
        Some(stdout)
    }
}

/// WHETHER THE ORACLE'S `FREE` WAS CONFIRMED ON THE RATE-LIMIT AXIS.
///
/// `pane-dispatch-ready` 8557c07 began emitting `rate_limit` on every row —
/// `measured_free` | `measured_limited` | `unmeasured` — because its census
/// (`ntm --robot-agent-health`) can answer three ways and its silence used to read as
/// "not rate-limited". THIS PARSER KEYED ON `state` ALONE, so the field landed in the
/// payload and was DROPPED ON THE FLOOR: an `unmeasured` FREE still resolved `Idle` and
/// still dispatched. BUILT ≠ WIRED at FIELD granularity — a produced field whose consumer
/// runs constantly and never reads it is invisible to any crate-level wiring census.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreeConfirmation {
    /// The oracle measured this pane's rate-limit axis.
    Measured,
    /// The oracle ran and did NOT measure this pane. Its `FREE` is unconfirmed in exactly
    /// the dimension the census exists to cover.
    Unmeasured,
    /// The oracle does not emit the field at all: a binary older than 8557c07.
    NotSpoken,
}

/// Classify the oracle row's `rate_limit` token.
///
/// ⛔ THE THREE ANSWERS ARE NOT TWO, AND `NotSpoken` IS NOT `Unmeasured`. An installed
/// `~/.local/bin/pane-dispatch-ready` predating 8557c07 emits no field at all. Folding that
/// into `Unmeasured` would turn EVERY `FREE` row into `Unknown` the moment this crate is
/// deployed ahead of the oracle — a fleet-wide capacity zero produced by DEPLOYMENT LAG
/// rather than by any measurement, which is the false-zero class this repo has paid for.
/// So an absent field preserves the status quo ante exactly: `FREE` stays `Idle`.
///
/// ⭐ AN UNRECOGNISED TOKEN IS `Unmeasured`, NOT `NotSpoken`, AND THE ASYMMETRY IS
/// DELIBERATE. Absent is an IDENTIFIED state — the pre-8557c07 schema, which never claimed
/// coverage. A token this parser does not know is an UNIDENTIFIED state: the oracle claimed
/// coverage in a dialect we cannot read, and unreadable coverage is not coverage. This crate
/// is a DISPATCHER (it plans and sends), so by the same asymmetry its own `resolve` already
/// encodes — "a false hold costs a tick of throughput, a false dispatch interrupts a pane 28
/// minutes into a turn" — it withholds rather than guesses.
#[must_use]
pub fn free_confirmation(coverage: Option<&str>) -> FreeConfirmation {
    match coverage {
        None => FreeConfirmation::NotSpoken,
        Some("measured_free" | "measured_limited") => FreeConfirmation::Measured,
        Some(_) => FreeConfirmation::Unmeasured,
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
///
/// ⛔ AND `FREE` ALONE IS NO LONGER ENOUGH. The oracle's own `rate_limit` field says whether
/// its FREE was confirmed on the rate-limit axis; see [`free_confirmation`]. An `unmeasured`
/// FREE becomes `Unknown` HERE rather than `Idle`, and the existing join does the rest with no
/// new policy: `resolve(Idle, Unknown) => Unconfirmed`, which is reported and never dispatched.
/// A lone positive still never dispatches — this just stops a FREE the oracle declined to
/// confirm from counting as the second confirming read.
pub fn parse_oracle_view(text: &str) -> Option<SurfaceView> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let entries = value.get("panes")?.as_array()?;
    let mut panes = BTreeMap::new();
    for entry in entries {
        let Some(pane) = pane_id(entry.get("pane")) else {
            continue;
        };
        let observation = match entry.get("state").and_then(serde_json::Value::as_str) {
            Some("FREE") => {
                match free_confirmation(entry.get("rate_limit").and_then(serde_json::Value::as_str))
                {
                    FreeConfirmation::Measured | FreeConfirmation::NotSpoken => Observation::Idle,
                    FreeConfirmation::Unmeasured => Observation::Unknown,
                }
            }
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
    /// BOTH surfaces confidently say idle.
    Dispatch,
    /// Some surface confidently says the pane cannot receive work.
    Hold,
    /// Both surfaces are confident and they disagree. A finding, not a quiet skip.
    Conflict,
    /// Exactly one surface says idle and the other cannot see the pane. A positive read
    /// no second reader confirmed — never dispatched, always reported.
    Unconfirmed,
    /// Neither surface could classify the pane. We cannot tell — and cannot say "busy".
    Unknowable,
}

impl Resolution {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Hold => "hold",
            Self::Conflict => "conflict",
            Self::Unconfirmed => "unconfirmed",
            Self::Unknowable => "unknowable",
        }
    }
}

/// The three-valued join.
///
/// **A LONE POSITIVE NEVER DISPATCHES**, and that clause is the one this crate got wrong
/// first. `docs/contracts/pane_readiness_contract.md` §2.2.1, landed 2026-09-01 21:47 in
/// 609a97b: "No single ntm field is sufficient. A positive free read must be confirmed
/// against the last status line at the two-capture grade." It was measured — pane 1 read
/// `observation_state: "idle", observation_confidence: 0.95, safe_to_dispatch: true`
/// while two `--robot-tail` captures 95 seconds apart both carried a braille spinner with
/// the timer advancing 26m -> 28m. **A confident idle can be false, and unlike
/// `state=UNKNOWN` at 0.5 it does not announce its own weakness.** Bead
/// `omp-orchestrator-observation-state-false-idle-riqd`.
///
/// So UNKNOWN yields to CONFIRMED only in the NEGATIVE direction — a confident busy on
/// either arm holds the pane. In the positive direction both arms must agree. The
/// asymmetry is deliberate: a false hold costs a tick of throughput, a false dispatch
/// interrupts a pane 28 minutes into a turn.
pub const fn resolve(activity: Observation, oracle: Observation) -> Resolution {
    match (activity, oracle) {
        (Observation::Idle, Observation::Idle) => Resolution::Dispatch,
        (Observation::Idle, Observation::Busy) | (Observation::Busy, Observation::Idle) => {
            Resolution::Conflict
        }
        (Observation::Idle, Observation::Unknown) | (Observation::Unknown, Observation::Idle) => {
            Resolution::Unconfirmed
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
    pub unconfirmed: Vec<String>,
    pub unknowable: Vec<String>,
    pub held: Vec<String>,
    /// Every pane index the exclusion set names, whether or not it was dispatchable.
    ///
    /// Echoed on every run. The orchestrator pane is BUSY almost all the time, so a
    /// report that only listed exclusions when they bit would leave "is the conductor
    /// excluded?" unanswerable until the exact moment the answer stops mattering.
    pub excluded: Vec<String>,
    /// Panes that WERE dispatchable and were removed from capacity by the exclusion set.
    pub withheld: Vec<String>,
}

impl Decision {
    /// Panes actually observed by at least one surface.
    pub fn observed(&self) -> usize {
        self.dispatchable.len()
            + self.conflicts.len()
            + self.unconfirmed.len()
            + self.unknowable.len()
            + self.held.len()
            + self.withheld.len()
    }

    /// Remove the excluded panes from CAPACITY, and from capacity only.
    ///
    /// THE MEASURED DEFECT (`AGENTS.md`): "`refill-idle-panes --plan` proposes `pane=1`,
    /// the ORCHESTRATOR pane." Pane index 1 of session `omp-orchestrator` is `%6`, the
    /// conductor — dispatching a bead into it interrupts the process that does the
    /// dispatching.
    ///
    /// An excluded pane KEEPS its place in `conflicts` / `unconfirmed` / `unknowable`.
    /// `crates/tick-monitor/src/main.rs:284` states the same rule for the same fleet:
    /// "NOT excluded by `--exclude-pane`: the conductor's own pane can be obscured or
    /// prompting too, and 'it is not worker capacity' is not 'I need not look at it'."
    /// Exclusion is a capacity decision, never a blindfold.
    pub fn withhold(&mut self, exclusions: &ExclusionSet) {
        self.excluded = exclusions.indices.iter().cloned().collect();
        sort_panes(&mut self.excluded);
        let mut withheld = Vec::new();
        self.dispatchable.retain(|pane| {
            if exclusions.indices.contains(pane) {
                withheld.push(pane.clone());
                false
            } else {
                true
            }
        });
        sort_panes(&mut withheld);
        self.withheld = withheld;
    }
}

/// One row of `tmux list-panes -a -F '#{pane_id} #{session_name} #{pane_index}'`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRow {
    pub pane_id: String,
    pub session: String,
    pub index: String,
}

/// Parse the tmux pane table.
///
/// **Two namespaces have to be joined by tmux itself.** Both roster surfaces key panes by
/// INDEX (`ntm --robot-activity` emits `"pane":"1"`, `pane-dispatch-ready` emits
/// `"pane":"0"`), while every exclusion in this fleet is written as a tmux pane ID: the
/// live watcher for this very session runs
/// `tick-monitor watch --session omp-orchestrator … --exclude-pane %6 --exclude-pane %5`,
/// and `crates/omp-orchestrator/src/resident.rs` pushes `$TMUX_PANE` onto that same list.
/// Assuming `%6` means index 6 is how an exclusion silently stops excluding — measured
/// live 2026-09-03: `%5 %6 %7 %8 %9` are indices `0 1 2 3 4`.
///
/// `None` on an empty or unparseable listing. A live server cannot have zero panes, so an
/// empty table is a broken probe, never "no panes to exclude".
pub fn parse_pane_table(text: &str) -> Option<Vec<PaneRow>> {
    let rows: Vec<PaneRow> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pane_id = fields.next()?;
            let session = fields.next()?;
            let index = fields.next()?;
            if !pane_id.starts_with('%') || index.parse::<u64>().is_err() {
                return None;
            }
            Some(PaneRow {
                pane_id: pane_id.to_owned(),
                session: session.to_owned(),
                index: index.to_owned(),
            })
        })
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

/// Which session holds this pane id, if the table knows.
pub fn session_of_pane(rows: &[PaneRow], pane_id: &str) -> Option<String> {
    rows.iter()
        .find(|row| row.pane_id == pane_id)
        .map(|row| row.session.clone())
}

/// Is this session present in the table at all?
pub fn session_is_live(rows: &[PaneRow], session: &str) -> bool {
    rows.iter().any(|row| row.session == session)
}

/// `pane_id -> pane_index` for ONE session.
///
/// `None` when the session contributed no rows — an absent session is not a session with
/// nothing to exclude.
pub fn pane_index_map(rows: &[PaneRow], session: &str) -> Option<BTreeMap<String, String>> {
    let map: BTreeMap<String, String> = rows
        .iter()
        .filter(|row| row.session == session)
        .map(|row| (row.pane_id.clone(), row.index.clone()))
        .collect();
    if map.is_empty() {
        None
    } else {
        Some(map)
    }
}

/// The panes refill may never propose, resolved into the index namespace the rosters use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExclusionSet {
    /// Pane INDICES excluded from capacity.
    pub indices: BTreeSet<String>,
    /// `%`-form specs that named no pane in the target session. Reported rather than
    /// dropped in silence: an exclusion aimed at another fleet is worth seeing, and an
    /// exclusion aimed at a pane that has since died is worth seeing too.
    pub unmatched: Vec<String>,
}

/// Resolve `--exclude-pane` specs against the target session's pane table.
///
/// Accepts both namespaces, because both are in live use: `%6` (the shape
/// `tick-monitor watch --session omp-orchestrator … --exclude-pane %6` passes, and the
/// shape `$TMUX_PANE` carries) and a bare index `1` (the shape the rosters speak).
///
/// **A `%`-spec that cannot be resolved is a REFUSAL, not an empty exclusion.** Silently
/// dropping `%6` because tmux was unreadable proposes the conductor's own pane — the
/// precise defect this function exists to prevent. A bare index needs no table and is
/// taken at face value; it can only ever match fewer panes than intended, never more.
pub fn resolve_exclusions(
    specs: &[String],
    pane_map: Option<&BTreeMap<String, String>>,
) -> Result<ExclusionSet, String> {
    let mut set = ExclusionSet::default();
    for raw in specs {
        let spec = raw.trim();
        if spec.is_empty() {
            continue;
        }
        if spec.starts_with('%') {
            let Some(map) = pane_map else {
                return Err(format!(
                    "refill: UNMEASURABLE detector=pane_index_map spec={spec} \
                     why=an exclusion named a tmux pane id and the pane table was unreadable \
                     probe=`tmux list-panes -a -F '#{{pane_id}} #{{session_name}} #{{pane_index}}'` \
                     remedy=an unresolvable exclusion is not an absent exclusion; refusing rather \
                     than proposing the pane the exclusion exists to protect"
                ));
            };
            match map.get(spec) {
                Some(index) => {
                    set.indices.insert(index.clone());
                }
                None => set.unmatched.push(spec.to_owned()),
            }
            continue;
        }
        if spec.parse::<u64>().is_ok() {
            set.indices.insert(spec.to_owned());
            continue;
        }
        return Err(format!(
            "refill: usage detector=exclude_pane spec={spec} \
             why=neither a tmux pane id (`%6`) nor a pane index (`1`) \
             remedy=an unparsed exclusion would silently exclude nothing; pass the shape \
             `tick-monitor watch --exclude-pane %6` uses, or the index the rosters key on"
        ));
    }
    Ok(set)
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
            Resolution::Unconfirmed => &mut decision.unconfirmed,
            Resolution::Unknowable => &mut decision.unknowable,
            Resolution::Hold => &mut decision.held,
        };
        bucket.push(pane);
    }
    sort_panes(&mut decision.dispatchable);
    sort_panes(&mut decision.conflicts);
    sort_panes(&mut decision.unconfirmed);
    sort_panes(&mut decision.unknowable);
    sort_panes(&mut decision.held);
    decision
}

/// Resolve capacity from both surfaces AND the exclusion set, in ONE call.
///
/// The production path uses this rather than `decide` followed by
/// [`Decision::withhold`], because two calls is how the exclusion set came to be resolved
/// and PRINTED while never being applied. Measured live on 2026-09-03, from the run that
/// caught it: the header said `exclude_panes={"1"}` and the same run's verdict said
/// `excluded=[]`. A resolved-but-unapplied guard is worse than an absent one — it reads,
/// to a log reader and to the next author, exactly like a working guard.
pub fn decide_capacity(
    activity: &SurfaceView,
    oracle: &SurfaceView,
    exclusions: &ExclusionSet,
) -> Decision {
    let mut decision = decide(activity, oracle);
    decision.withhold(exclusions);
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
    if decision.dispatchable.is_empty()
        && (!decision.unknowable.is_empty() || !decision.unconfirmed.is_empty())
    {
        return RefillOutcome {
            message: format!(
                "refill: UNMEASURABLE detector=pane_state_not_established \
                 unknown_on_both={:?} unconfirmed_positive={:?} \
                 probe=`ntm --robot-activity` vs `pane-dispatch-ready <session> --json` \
                 remedy=an unobservable pane is NOT a busy pane, and a positive read only \
                 one surface can see is NOT a confirmed free pane \
                 (pane_readiness_contract 2.2.1); fix the probe or read the panes by hand \
                 — this is not a quiet fleet",
                decision.unknowable, decision.unconfirmed
            ),
            code: 2,
        };
    }
    // ORDERING: this sits AFTER the unmeasurable branch on purpose. If capacity was
    // withheld AND some pane could not be established, the unresolved observation is the
    // louder fact and keeps its nonzero exit. Exclusion may empty the plan; it may never
    // convert a refusal into a pass.
    if decision.dispatchable.is_empty() && !decision.withheld.is_empty() {
        return RefillOutcome {
            message: format!(
                "refill: every dispatchable pane is EXCLUDED from capacity \
                 (withheld={:?}, excluded={:?}, held={}) — the fleet has no WORKER \
                 capacity, and an excluded pane is not capacity",
                decision.withheld,
                decision.excluded,
                decision.held.len()
            ),
            code: 0,
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
            "refill: {} dispatchable pane(s) {:?} \
             (held={}, unconfirmed={}, unknowable={}, excluded={:?}, withheld={:?})",
            decision.dispatchable.len(),
            decision.dispatchable,
            decision.held.len(),
            decision.unconfirmed.len(),
            decision.unknowable.len(),
            decision.excluded,
            decision.withheld
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

/// A recommendation the selector refused, with the reason a reader can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPick {
    pub bead: String,
    pub reason: String,
}

/// Bead statuses that may receive a fresh packet. Everything else is owned by someone:
/// `in_progress` by its worker, `grading` by its grader, `blocked` by its blocker.
pub const DISPATCHABLE_STATUSES: [&str; 2] = ["open", "ready"];

/// Why a recommendation must not be dispatched, or `None` if it may be.
///
/// The bv envelope carries `type` and `status` on every recommendation (measured
/// 2026-09-02, keys: action, breakdown, id, labels, priority, reasons, score, status,
/// title, type, unblocks_ids). A field that is ABSENT is not a disqualifier — an older
/// envelope or a fixture without it must not silently empty the queue — but a field that
/// is PRESENT and disqualifying is refused by name.
///
/// MEASURED 2026-09-02T20:21Z, the first live `--apply` after the cron environment was
/// repaired: the two top picks were `cp-epic-fleet-work-quality-08l6` (type=epic) and
/// `cp-...08l6.74.2` (status=grading, owned by its grader). Both panes accepted the packet
/// and went WORKING; one spent its cycle proving an epic is not a unit of work. bv ranks
/// by graph centrality, and an epic is the most central node there is — so without this
/// filter the top pick is an epic whenever one is open.
pub fn dispatch_refusal(recommendation: &serde_json::Value) -> Option<String> {
    if let Some(kind) = recommendation.get("type").and_then(serde_json::Value::as_str) {
        if kind.eq_ignore_ascii_case("epic") {
            return Some("type=epic".into());
        }
    }
    if let Some(status) = recommendation.get("status").and_then(serde_json::Value::as_str) {
        if !DISPATCHABLE_STATUSES
            .iter()
            .any(|ok| ok.eq_ignore_ascii_case(status))
        {
            return Some(format!("status={status}"));
        }
    }
    None
}

/// Parse `bv --robot-triage` recommendations into descending-score bead ids.
///
/// `quick_ref.top_picks` is a weaker summary and does not carry the ranking score
/// that dispatch needs. Recommendations are sorted explicitly so this parser does
/// not silently inherit a different envelope order. Epics and non-dispatchable
/// statuses are refused (see `dispatch_refusal`); use `parse_recommendations_with_skips`
/// when the refusals must be reported.
pub fn parse_recommendations(text: &str) -> Vec<String> {
    parse_recommendations_with_skips(text).0
}

/// `parse_recommendations`, plus every refused recommendation with its reason, so a
/// `--plan` reader can see WHY a bead was skipped instead of inferring it from absence.
pub fn parse_recommendations_with_skips(text: &str) -> (Vec<String>, Vec<SkippedPick>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return (Vec::new(), Vec::new());
    };
    let Some(recommendations) = value
        .get("triage")
        .and_then(|t| t.get("recommendations"))
        .and_then(serde_json::Value::as_array)
    else {
        return (Vec::new(), Vec::new());
    };

    let mut skipped = Vec::new();
    let mut ranked = recommendations
        .iter()
        .enumerate()
        .filter_map(|(position, recommendation)| {
            let id = recommendation.get("id")?.as_str()?;
            let score = recommendation.get("score")?.as_f64()?;
            if !score.is_finite() {
                return None;
            }
            if let Some(reason) = dispatch_refusal(recommendation) {
                skipped.push(SkippedPick {
                    bead: id.to_string(),
                    reason,
                });
                return None;
            }
            Some((score, position, id.to_string()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.1.cmp(&right.1))
    });
    (ranked.into_iter().map(|(_, _, id)| id).collect(), skipped)
}

/// Fallback picks from the ready-queue JSON when bv's top-N recommendations all refuse.
///
/// MEASURED 2026-09-02T21:23Z, the first `--apply` with `dispatch_refusal` installed: bv
/// returned exactly 10 recommendations (`--robot-max-results` does not raise it), every one
/// an epic, a grading bead, or a blocked bead, so the lane reported "NO picks — queue empty"
/// while the ready queue listed 28 dispatchable beads. bv ranks by centrality; the DAG's most
/// central nodes are precisely the ones a worker must not receive. The ready list is the
/// second oracle: rows ordered by priority (P0 first), same refusal applied to
/// `issue_type`/`status`, epics never.
pub fn parse_ready_fallback(text: &str) -> (Vec<String>, Vec<SkippedPick>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return (Vec::new(), Vec::new());
    };
    let rows = match &value {
        serde_json::Value::Array(rows) => rows.clone(),
        serde_json::Value::Object(map) => map
            .get("issues")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut skipped = Vec::new();
    let mut ranked = rows
        .iter()
        .enumerate()
        .filter_map(|(position, row)| {
            let id = row.get("id")?.as_str()?;
            // ready-queue rows spell the type `issue_type`; bv spells it `type`. Normalise so
            // one refusal function governs both oracles.
            let mut probe = serde_json::Map::new();
            if let Some(kind) = row.get("issue_type").or_else(|| row.get("type")) {
                probe.insert("type".into(), kind.clone());
            }
            if let Some(status) = row.get("status") {
                probe.insert("status".into(), status.clone());
            }
            if let Some(reason) = dispatch_refusal(&serde_json::Value::Object(probe)) {
                skipped.push(SkippedPick {
                    bead: id.to_string(),
                    reason,
                });
                return None;
            }
            if row.get("assignee").and_then(serde_json::Value::as_str).map(|a| !a.is_empty()).unwrap_or(false) {
                skipped.push(SkippedPick {
                    bead: id.to_string(),
                    reason: "assigned".into(),
                });
                return None;
            }
            let priority = row.get("priority").and_then(serde_json::Value::as_i64).unwrap_or(i64::MAX);
            Some((priority, position, id.to_string()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    (ranked.into_iter().map(|(_, _, id)| id).collect(), skipped)
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

// -------------------------------------------------------------------------------------
// A PLAN IS NOT AN AUTHORIZATION
// -------------------------------------------------------------------------------------
//
// STANDING CONSTRAINT, 2026-09-03, from the repository owner, verbatim: "we dont want to
// automatically refill panes with work without being approved - we have to reap every
// update and get docs updated - we dont want auto refill until our process is proven."
//
// The distinction is NOT less automation. Notification is wanted — a pane finishing must
// reach the conductor as a loud event. ACTUATION is not: what the conductor does next is
// a judgement that gets reaped, documented and approved first.
//
// This crate is the SELECTION half and it stops at a proposal. The failure mode that
// makes that a code problem rather than a policy one: a plan line and a dispatch order
// are the same bytes, so any downstream reader that finds a `PLAN pane=… bead=…` row can
// treat it as an order and nobody can tell from the row that it was not one. The marker
// below is what makes them different bytes.

/// The token every planned assignment carries so a plan can never read as an order.
pub const APPROVAL_MARKER: &str = "APPROVAL_REQUIRED";

/// Why an actuator may not act on a refill plan on the strength of the plan alone.
pub const APPROVAL_REASON: &str = "reap-and-document-before-refill";

/// Render one planned assignment as a PROPOSAL.
///
/// The marker is in the line itself, not in a header the reader may never have seen and
/// not in a sidecar an actuator can skip. One line, self-describing, greppable.
pub fn plan_line(assignment: &Assignment, bytes: usize) -> String {
    format!(
        "PLAN  pane={} bead={} bytes={bytes} {APPROVAL_MARKER}={APPROVAL_REASON} \
         note=refill-idle-panes PROPOSES this pairing; it does not authorize it",
        assignment.pane, assignment.bead
    )
}

/// May a consumer ACT on this plan line?
///
/// Two ways to be refused, and both are defects a plan-as-order reader would have:
/// * the line carries no [`APPROVAL_MARKER`], so its provenance cannot be established —
///   and an unestablished provenance is not a permissive one;
/// * the line carries the marker and the consumer brought no approval token of its own.
///
/// The token is deliberately opaque here. This function decides only that SOMETHING
/// approved outside refill; who may issue one is the approving system's question, and
/// answering it here would put the approver and the requester in the same binary.
pub fn authorize_plan_line(line: &str, approval: Option<&str>) -> Result<(), String> {
    if !line.contains(APPROVAL_MARKER) {
        return Err(format!(
            "refill: PLAN_UNMARKED detector=plan_provenance why=a plan line without \
             `{APPROVAL_MARKER}` cannot be shown to have come from a refill proposal \
             remedy=an unestablished provenance is not a permissive one; refusing"
        ));
    }
    match approval.map(str::trim).filter(|token| !token.is_empty()) {
        Some(_) => Ok(()),
        None => Err(format!(
            "refill: {APPROVAL_MARKER} detector=plan_consumed_unapproved \
             reason={APPROVAL_REASON} why=this row is a PROPOSAL and carries no approval \
             remedy=reap the finished work, update the docs, then approve; a plan is not \
             an authorization"
        )),
    }
}

/// The refusal `--apply` returns while actuation is withheld.
///
/// **This crate's dispatch path is INERT BY DECISION, not by breakage.** It was inert by
/// breakage until 2026-09-03: every verb, `--apply` included, died on
/// `UNMEASURABLE detector=pane_roster` before reaching a send. Repairing that arm — the
/// whole point of the fix — would have moved `--apply` from dead to LIVE as a side effect,
/// which is precisely the actuation the standing constraint withholds. So the verb refuses
/// HERE, at the top of `run`, ahead of every probe: a broken instrument and a withheld
/// capability must not be the same state, because repairing the instrument then silently
/// grants the capability.
pub fn actuation_refusal() -> RefillOutcome {
    RefillOutcome {
        message: format!(
            "refill: {APPROVAL_MARKER} detector=actuation_withheld verb=--apply \
             reason={APPROVAL_REASON} why=the selection half proposes and stops; \
             auto-refill is withheld until the reap-and-document process is proven \
             remedy=run `--plan`, reap and document the finished work, then dispatch \
             under an approval that is not this plan"
        ),
        code: 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VERBATIM from `ntm --robot-activity=omp-orchestrator`, captured 2026-09-02
    /// 03:14:55Z on the live fleet, trimmed to the fields either rule reads.
    ///
    /// Panes 2 and 3 are codex workers `pane-dispatch-ready` CONFIRMS free while `ntm`
    /// reports them working at `observation_confidence: 0.95`. Note `state=UNKNOWN,
    /// confidence=0.5` on panes 1, 2, 3 and 5 with OPPOSITE `safe_to_dispatch` values —
    /// that detail refutes the unknown-coercion reading, and it is retained here so the
    /// refutation stays visible.
    const LIVE_ACTIVITY_0314: &str = r#"{
      "success": true,
      "session": "omp-orchestrator",
      "agents": [
        {"pane":"1","agent_type":"claude","state":"UNKNOWN","confidence":0.5,
         "observation_state":"idle","observation_confidence":0.95,
         "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
        {"pane":"2","agent_type":"codex","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","observation_confidence":0.95,
         "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"3","agent_type":"codex","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","observation_confidence":0.95,
         "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"4","agent_type":"omp-glm","state":"ERROR","confidence":0.95,
         "observation_state":"working","observation_confidence":0.95,
         "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"5","agent_type":"omp-glm","state":"UNKNOWN","confidence":0.5,
         "observation_state":"working","observation_confidence":0.95,
         "capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false}
      ]}"#;

    /// VERBATIM `ntm --robot-activity=omp-orchestrator` at 2026-09-02 03:37:51Z, the
    /// capture that settles which field dispatch reads. `state=UNKNOWN` panes are
    /// dispatchable here and `state=THINKING` panes are not.
    const LIVE_ACTIVITY_0337: &str = r#"{"agents":[
        {"pane":"1","state":"THINKING","confidence":0.8,"observation_state":"idle",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
        {"pane":"2","state":"THINKING","confidence":0.8,"observation_state":"working",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"3","state":"THINKING","confidence":0.8,"observation_state":"working",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":false},
        {"pane":"4","state":"UNKNOWN","confidence":0.5,"observation_state":"idle",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true},
        {"pane":"5","state":"UNKNOWN","confidence":0.5,"observation_state":"idle",
         "observation_confidence":0.95,"capture_provenance":"live","observation_freshness":"fresh","safe_to_dispatch":true}]}"#;

    /// VERBATIM from `pane-dispatch-ready omp-orchestrator --json`, 03:14:55Z.
    ///
    /// The key is `state`. A fixture written against `status` parses to zero classified
    /// panes and certifies a payload shape production never emits.
    const LIVE_ORACLE_0314: &str = r#"{"schema":"zs.dispatch-ready.v1","panes":[
        {"pane":"0","state":"BUSY"},
        {"pane":"1","state":"BUSY"},
        {"pane":"2","state":"FREE"},
        {"pane":"3","state":"FREE"},
        {"pane":"4","state":"BUSY"},
        {"pane":"5","state":"NO_AGENT"}]}"#;

    /// A live, fresh ntm row. Every synthetic fixture goes through this so no test can
    /// accidentally assert on a row the parser treats as UNKNOWN for a reason the test
    /// did not intend.
    fn ntm_row(pane: &str, observation_state: &str) -> String {
        format!(
            r#"{{"pane":"{pane}","observation_state":"{observation_state}",
                "observation_confidence":0.95,"capture_provenance":"live",
                "observation_freshness":"fresh"}}"#
        )
    }

    fn ntm(rows: &[(&str, &str)]) -> SurfaceView {
        let body: Vec<String> = rows.iter().map(|(p, s)| ntm_row(p, s)).collect();
        parse_activity_view(&format!(r#"{{"agents":[{}]}}"#, body.join(",")))
            .expect("fixture parses")
    }

    fn oracle(rows: &[(&str, &str)]) -> SurfaceView {
        let body: Vec<String> = rows
            .iter()
            .map(|(p, s)| format!(r#"{{"pane":"{p}","state":"{s}"}}"#))
            .collect();
        parse_oracle_view(&format!(r#"{{"panes":[{}]}}"#, body.join(","))).expect("fixture parses")
    }

    fn live_views() -> (SurfaceView, SurfaceView) {
        (
            parse_activity_view(LIVE_ACTIVITY_0314).expect("fixture parses"),
            parse_oracle_view(LIVE_ORACLE_0314).expect("fixture parses"),
        )
    }

    /// THE REFUTATION, pinned, with the numbers it actually measures.
    ///
    /// `safe_to_dispatch == (observation_state == "idle")` holds on 10/10 rows across
    /// both captures. A rule deriving it from `state` scores 5/10 overall — a coin flip
    /// — and 1/5 on the 03:37 capture, WORSE than chance, because there
    /// `state=UNKNOWN` panes are dispatchable and `state=THINKING` panes are not.
    ///
    /// The overall 5/10 is why the wrong diagnosis was so easy to believe: on the
    /// 03:14 capture alone a `state` rule scores 4/5 and looks like the mechanism. Only
    /// the second capture separates them. If this test ever fails, the field this crate
    /// reads is the wrong one and the header's reasoning is void.
    #[test]
    fn dispatch_admissibility_tracks_observation_state_and_not_state() {
        let mut agreement = Vec::new();
        for capture in [LIVE_ACTIVITY_0314, LIVE_ACTIVITY_0337] {
            let value: serde_json::Value = serde_json::from_str(capture).expect("parses");
            let agents = value["agents"].as_array().expect("agents");
            let mut state_agrees = 0usize;
            for agent in agents {
                let safe = agent["safe_to_dispatch"]
                    .as_bool()
                    .expect("safe_to_dispatch");
                assert_eq!(
                    safe,
                    agent["observation_state"].as_str() == Some("idle"),
                    "pane {} breaks safe_to_dispatch == (observation_state == idle)",
                    agent["pane"]
                );
                let state_unclassified =
                    matches!(agent["state"].as_str(), Some("UNKNOWN" | "ERROR"));
                if safe == (state_unclassified == false) {
                    state_agrees += 1;
                }
            }
            agreement.push((agents.len(), state_agrees));
        }
        assert_eq!(
            agreement,
            vec![(5usize, 4usize), (5usize, 1usize)],
            "measured agreement of a `state`-derived rule with safe_to_dispatch, per \
             capture. 4/5 on 03:14 is why the wrong mechanism was believable; 1/5 on \
             03:37 is what refutes it, and a single capture could never have."
        );
    }

    /// The gating fields are the observation ones. A row whose `state` says UNKNOWN is
    /// still a CONFIDENT observation, and vice versa.
    #[test]
    fn state_is_never_consulted_by_the_parser() {
        let unknown_state_idle_observation = parse_activity_view(
            r#"{"agents":[{"pane":"4","state":"UNKNOWN","confidence":0.5,
                "observation_state":"idle","capture_provenance":"live",
                "observation_freshness":"fresh","safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        assert_eq!(
            unknown_state_idle_observation.get("4"),
            Observation::Idle,
            "state=UNKNOWN must NOT make a live idle observation unknown — this is the \
             03:37 capture's pane 4, the only kind of pane that was dispatchable"
        );
        let thinking_state_working_observation = parse_activity_view(
            r#"{"agents":[{"pane":"2","state":"THINKING","confidence":0.8,
                "observation_state":"working","capture_provenance":"live",
                "observation_freshness":"fresh","safe_to_dispatch":false}]}"#,
        )
        .expect("parses");
        assert_eq!(
            thinking_state_working_observation.get("2"),
            Observation::Busy
        );
    }

    /// The oracle fixture must actually CLASSIFY panes. A drifted key would make every
    /// assertion below vacuous while every one of them still passed.
    #[test]
    fn the_oracle_fixture_matches_the_shape_the_real_surface_emits() {
        let (_, oracle_view) = live_views();
        assert_eq!(oracle_view.get("2"), Observation::Idle);
        assert_eq!(oracle_view.get("5"), Observation::Busy);
        assert_eq!(
            oracle_view.confident().len(),
            6,
            "every pane in the capture must be classified; a `status`-keyed fixture yields 0"
        );
    }

    // ── THE ORACLE'S RATE-LIMIT COVERAGE (BUILT ≠ WIRED, AT FIELD GRANULARITY) ─────────
    // `pane-dispatch-ready` 8557c07 EMITS `rate_limit`; this parser keyed on `state` alone,
    // so the field was produced on every row and read by nobody. These are the legs that
    // make the produced field load-bearing.
    fn oracle_row(state: &str, coverage: Option<&str>) -> SurfaceView {
        let row = match coverage {
            Some(c) => format!(r#"{{"pane":"2","state":"{state}","rate_limit":"{c}"}}"#),
            None => format!(r#"{{"pane":"2","state":"{state}"}}"#),
        };
        parse_oracle_view(&format!(r#"{{"panes":[{row}]}}"#)).expect("fixture parses")
    }

    #[test]
    fn rule_free_coverage_matrix_reaches_the_observation() {
        let matrix = [
            (Some("measured_free"), Observation::Idle),
            (Some("unmeasured"), Observation::Unknown),
            (None, Observation::Idle),
        ];
        assert!(
            !matrix.is_empty(),
            "RULE coverage_matrix_non_vacuous: an empty matrix must never read as a pass"
        );
        for (coverage, expected) in matrix {
            assert_eq!(
                oracle_row("FREE", coverage).get("2"),
                expected,
                "RULE oracle_coverage_wired: a FREE row with rate_limit={coverage:?} observes \
                 {expected:?}; keying on `state` alone drops the field on the floor"
            );
        }
    }

    #[test]
    fn rule_an_unmeasured_free_is_never_dispatched() {
        // The join does the withholding with NO new policy: a lone positive never dispatches.
        let activity = ntm(&[("2", "idle")]);
        let confirmed = decide(&activity, &oracle_row("FREE", Some("measured_free")));
        assert_eq!(
            confirmed.dispatchable,
            vec!["2".to_string()],
            "RULE measured_free_still_dispatches: this must not withhold healthy capacity"
        );
        let unmeasured = decide(&activity, &oracle_row("FREE", Some("unmeasured")));
        assert!(
            unmeasured.dispatchable.is_empty(),
            "RULE unmeasured_free_withheld: an unconfirmed FREE must never be capacity, got {:?}",
            unmeasured.dispatchable
        );
        assert_eq!(
            unmeasured.unconfirmed,
            vec!["2".to_string()],
            "RULE unmeasured_free_reported: it is UNCONFIRMED and reported, never a quiet skip"
        );
    }

    #[test]
    fn rule_an_absent_field_is_not_an_unmeasured_field() {
        // ANTI-VACUITY IN THE OTHER DIRECTION. An oracle older than 8557c07 emits no field.
        // Folding that into `unmeasured` would zero the fleet from DEPLOYMENT LAG rather than
        // from any measurement.
        assert_eq!(
            free_confirmation(None),
            FreeConfirmation::NotSpoken,
            "RULE absent_is_not_unmeasured: no field is a schema fact, not a coverage fact"
        );
        assert_ne!(
            free_confirmation(None),
            free_confirmation(Some("unmeasured")),
            "RULE absent_is_not_unmeasured: collapsing them makes a stale binary zero the fleet"
        );
        let activity = ntm(&[("2", "idle")]);
        assert_eq!(
            decide(&activity, &oracle_row("FREE", None)).dispatchable,
            vec!["2".to_string()],
            "RULE absent_preserves_status_quo: a pre-8557c07 oracle must behave exactly as before"
        );
    }

    #[test]
    fn rule_an_unreadable_coverage_token_withholds() {
        // An UNIDENTIFIED token is not the IDENTIFIED absent case: the oracle claimed coverage
        // in a dialect this parser cannot read, and unreadable coverage is not coverage.
        assert_eq!(
            free_confirmation(Some("measured_at_dawn")),
            FreeConfirmation::Unmeasured,
            "RULE unreadable_coverage_withholds: an unknown token must not be read as confirmation"
        );
        assert_eq!(
            oracle_row("FREE", Some("measured_at_dawn")).get("2"),
            Observation::Unknown
        );
    }

    #[test]
    fn rule_coverage_never_promotes_a_non_free_row() {
        // The field narrows FREE; it must never widen anything else.
        for state in ["BUSY", "NO_AGENT"] {
            assert_eq!(
                oracle_row(state, Some("measured_free")).get("2"),
                Observation::Busy,
                "RULE coverage_scope: {state} stays a confident refusal whatever the coverage says"
            );
        }
    }

    /// THE MEASURED DEFECT. On the 03:14:55Z capture the two surfaces are BOTH confident
    /// and contradict on three panes. The old rule rendered that as "nothing to do" at
    /// exit 0; it must now be a named, per-pane, nonzero refusal.
    #[test]
    fn the_live_capture_is_a_named_nonzero_conflict_not_a_quiet_fleet() {
        let (activity, oracle_view) = live_views();
        let decision = decide(&activity, &oracle_view);
        assert!(
            decision.dispatchable.is_empty(),
            "refusing was CORRECT while the surfaces contradict; this fix does not \
             invent dispatchability"
        );
        assert_eq!(
            decision.conflicts,
            vec!["1".to_string(), "2".to_string(), "3".to_string()],
            "pane 1: ntm idle vs oracle BUSY; panes 2,3: ntm working vs oracle FREE"
        );
        let verdict = conflict_verdict(&activity, &oracle_view);
        assert!(
            matches!(verdict, OracleCompareVerdict::Disagree { .. }),
            "the shared kernel must see it: {verdict:?}"
        );
        let outcome = run_outcome(&decision, &verdict);
        assert_eq!(outcome.code, 1, "a confident contradiction exits NONZERO");
        assert!(outcome.message.contains("SURFACE_CONFLICT"));
        let renders_retired_line = outcome
            .message
            .contains("no idle pane both surfaces agree on");
        assert_eq!(
            renders_retired_line, false,
            "the retired quiet-success line must never render here"
        );
    }

    /// THE KNOWN-BAD LEG. The previous rule is reproduced exactly and must produce the
    /// reassuring empty answer on the same capture. Without this the repair is
    /// unfalsifiable.
    #[test]
    fn the_old_two_valued_rule_reports_the_conflict_as_an_empty_intersection() {
        let value: serde_json::Value = serde_json::from_str(LIVE_ACTIVITY_0314).expect("parses");
        let safe: BTreeSet<String> = value["agents"]
            .as_array()
            .expect("agents")
            .iter()
            .filter(|a| a["safe_to_dispatch"].as_bool() == Some(true))
            .filter_map(|a| pane_id(a.get("pane")))
            .collect();
        let oracle_value: serde_json::Value =
            serde_json::from_str(LIVE_ORACLE_0314).expect("parses");
        let free: BTreeSet<String> = oracle_value["panes"]
            .as_array()
            .expect("panes")
            .iter()
            .filter(|p| p["state"].as_str() == Some("FREE"))
            .filter_map(|p| pane_id(p.get("pane")))
            .collect();
        assert_eq!(
            safe.is_empty() || free.is_empty(),
            false,
            "both legs must be inhabited or the empty intersection proves nothing"
        );
        assert!(
            safe.intersection(&free).next().is_none(),
            "the old rule must select NOTHING here — that empty set is what it printed \
             as 'nothing to do' at exit 0"
        );
        let (activity, oracle_view) = live_views();
        assert_eq!(
            decide(&activity, &oracle_view).conflicts.is_empty(),
            false,
            "and the new rule must NAME what the old one silently swallowed"
        );
    }

    /// A LONE POSITIVE IS NEVER DISPATCHED. `pane_readiness_contract` 2.2.1: a positive
    /// free read must be confirmed by a second reader, because a confident idle was
    /// MEASURED false on a pane 28 minutes into a turn (609a97b).
    #[test]
    fn a_positive_only_one_surface_can_see_is_unconfirmed_and_never_dispatched() {
        let activity = ntm(&[("2", "idle")]);
        let oracle_view = oracle(&[("2", "FREE"), ("7", "FREE")]);
        assert_eq!(
            activity.get("7"),
            Observation::Unknown,
            "a pane ntm never enumerated is UNKNOWN to it, not busy"
        );
        let decision = decide(&activity, &oracle_view);
        assert_eq!(
            decision.dispatchable,
            vec!["2".to_string()],
            "only the pane BOTH surfaces confirm is dispatched"
        );
        assert_eq!(
            decision.unconfirmed,
            vec!["7".to_string()],
            "the oracle-only positive is reported, not acted on"
        );
    }

    /// The asymmetry is deliberate: a lone CONFIDENT BUSY still holds the pane, because
    /// a false hold costs a tick and a false dispatch interrupts a working agent.
    #[test]
    fn a_lone_confident_busy_still_holds_the_pane() {
        let activity = ntm(&[("2", "working")]);
        let oracle_view = oracle(&[]);
        let decision = decide(&activity, &oracle_view);
        assert_eq!(decision.held, vec!["2".to_string()]);
        assert!(decision.dispatchable.is_empty());
        assert!(decision.unconfirmed.is_empty());
    }

    /// A run with nothing dispatchable but an unconfirmed positive must NOT read as a
    /// quiet fleet — that is the whole defect, one class over.
    #[test]
    fn an_unconfirmed_positive_alone_is_a_typed_nonzero_refusal() {
        let activity = ntm(&[("7", "idle")]);
        let oracle_view = oracle(&[("2", "BUSY")]);
        let decision = decide(&activity, &oracle_view);
        assert_eq!(decision.unconfirmed, vec!["7".to_string()]);
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 2);
        assert!(outcome.message.contains("unconfirmed_positive"));
        assert!(outcome.message.contains("pane_readiness_contract 2.2.1"));
    }

    /// A capture that is not live-and-fresh is UNKNOWN, not busy. A stale reading is
    /// absence of evidence, and absence of evidence must not refuse a pane forever.
    #[test]
    fn a_stale_or_replayed_capture_is_unknown_not_busy() {
        for (provenance, freshness) in [("stale", "fresh"), ("live", "stale"), ("replay", "fresh")]
        {
            let view = parse_activity_view(&format!(
                r#"{{"agents":[{{"pane":"2","observation_state":"working",
                    "capture_provenance":"{provenance}","observation_freshness":"{freshness}",
                    "safe_to_dispatch":false}}]}}"#
            ))
            .expect("parses");
            assert_eq!(
                view.get("2"),
                Observation::Unknown,
                "provenance={provenance} freshness={freshness} must not assert working"
            );
        }
        let missing = parse_activity_view(
            r#"{"agents":[{"pane":"2","observation_state":"idle","safe_to_dispatch":true}]}"#,
        )
        .expect("parses");
        assert_eq!(
            missing.get("2"),
            Observation::Unknown,
            "a row with no provenance fields at all has not established liveness"
        );
    }

    /// An `observation_state` this parser does not recognise is UNKNOWN, never folded
    /// into one of the two it does.
    #[test]
    fn an_unrecognised_observation_state_is_unknown() {
        assert_eq!(ntm(&[("2", "wedged")]).get("2"), Observation::Unknown);
    }

    /// A pane no surface can classify is UNMEASURABLE and exits nonzero. It is NOT
    /// "nothing to do" — that coercion is the whole defect.
    #[test]
    fn a_pane_unknown_on_both_surfaces_is_a_typed_nonzero_refusal() {
        let activity = ntm(&[("2", "wedged")]);
        let oracle_view = oracle(&[("2", "WHO_KNOWS")]);
        let decision = decide(&activity, &oracle_view);
        assert_eq!(decision.unknowable, vec!["2".to_string()]);
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 2, "unknowable must exit NONZERO");
        assert!(outcome.message.contains("UNMEASURABLE"));
        assert!(
            outcome.message.contains("ntm --robot-activity"),
            "the refusal must NAME the probe: {}",
            outcome.message
        );
        assert!(!outcome
            .message
            .contains("no idle pane both surfaces agree on"));
    }

    /// THE PRESERVED CONSERVATISM. 2026-08-27: activity said pane 4 was free, the
    /// oracle said bare shell. A CONFIDENT disagreement is a conflict, never a dispatch.
    #[test]
    fn a_confident_disagreement_is_a_conflict_and_is_never_dispatched() {
        let activity = ntm(&[("4", "idle")]);
        let oracle_view = oracle(&[("4", "NO_AGENT")]);
        let decision = decide(&activity, &oracle_view);
        assert!(
            decision.dispatchable.is_empty(),
            "a bare shell is never dispatched"
        );
        assert_eq!(decision.conflicts, vec!["4".to_string()]);
        assert_eq!(
            conflicting_panes(&activity, &oracle_view),
            vec!["4".to_string()]
        );
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 1);
        assert!(outcome.message.contains("SURFACE_CONFLICT"));
    }

    /// A conflict must not starve the panes that ARE established. Report AND feed.
    #[test]
    fn a_conflict_on_one_pane_still_dispatches_the_panes_that_are_established() {
        let activity = ntm(&[("2", "idle"), ("4", "idle")]);
        let oracle_view = oracle(&[("2", "FREE"), ("4", "NO_AGENT")]);
        let decision = decide(&activity, &oracle_view);
        assert_eq!(decision.dispatchable, vec!["2".to_string()]);
        assert_eq!(decision.conflicts, vec!["4".to_string()]);
        assert_eq!(
            run_outcome(&decision, &conflict_verdict(&activity, &oracle_view)).code,
            1,
            "the fleet is fed AND the exit code still carries the unresolved observation"
        );
    }

    /// ANTI-VACUITY on the positive path: both surfaces confident and agreeing.
    #[test]
    fn a_pane_both_surfaces_confidently_call_free_is_selected() {
        let activity = ntm(&[("2", "idle")]);
        let oracle_view = oracle(&[("2", "FREE")]);
        let decision = decide(&activity, &oracle_view);
        assert_eq!(decision.dispatchable, vec!["2".to_string()]);
        assert!(matches!(
            conflict_verdict(&activity, &oracle_view),
            OracleCompareVerdict::Agree { .. }
        ));
        assert_eq!(
            run_outcome(&decision, &conflict_verdict(&activity, &oracle_view)).code,
            0
        );
    }

    /// The genuine no-work case survives: every pane confidently busy, exit 0.
    #[test]
    fn a_confidently_busy_fleet_is_genuine_no_work_at_exit_zero() {
        let activity = ntm(&[("2", "working")]);
        let oracle_view = oracle(&[("2", "BUSY")]);
        let decision = decide(&activity, &oracle_view);
        assert!(decision.dispatchable.is_empty());
        assert!(decision.unknowable.is_empty());
        assert!(decision.conflicts.is_empty());
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
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
            &format!(r#"{{"agents":[{}]}}"#, ntm_row("2", "idle")),
            r#"{"panes":[]}"#,
        );
        let refusal = measurability_refusal(&verdict).expect("an empty oracle must refuse");
        assert_eq!(refusal.code, 2);
        assert!(refusal.message.contains("UNMEASURABLE"));
    }

    #[test]
    fn an_unreadable_probe_is_unmeasurable_not_empty() {
        let good = format!(r#"{{"agents":[{}]}}"#, ntm_row("2", "idle"));
        for (a, o) in [
            ("not json", r#"{"panes":[{"pane":"2","state":"FREE"}]}"#),
            (good.as_str(), "not json"),
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
    /// sees bare shells ntm never reports. Over-strictness here gets the lane routed
    /// around, so it is pinned.
    #[test]
    fn differing_rosters_alone_do_not_refuse() {
        let verdict = measurability_verdict(
            &format!(r#"{{"agents":[{}]}}"#, ntm_row("2", "idle")),
            r#"{"panes":[{"pane":"0","state":"BUSY"},{"pane":"2","state":"FREE"}]}"#,
        );
        assert!(
            measurability_refusal(&verdict).is_none(),
            "a bare shell the projection never reports is not a broken probe: {verdict:?}"
        );
    }

    /// The confident domain excludes UNKNOWN panes, so a pane only one surface can see
    /// cannot register as a conflict.
    #[test]
    fn an_unknown_arm_never_registers_as_a_conflict() {
        let activity = ntm(&[("2", "idle")]);
        let oracle_view = oracle(&[("2", "FREE"), ("7", "BUSY")]);
        assert!(!confident_domain(&activity, &oracle_view).contains("7"));
        assert!(conflicting_panes(&activity, &oracle_view).is_empty());
        assert!(matches!(
            conflict_verdict(&activity, &oracle_view),
            OracleCompareVerdict::Agree { .. }
        ));
    }

    /// The label encoding exists so an all-busy oracle arm is still non-empty. Without
    /// it `compare_sets` would take its empty-oracle branch and report agreement while
    /// the product claimed a pane was idle.
    #[test]
    fn a_conflict_is_seen_even_when_the_oracle_calls_every_pane_busy() {
        let activity = ntm(&[("2", "idle")]);
        let oracle_view = oracle(&[("2", "BUSY")]);
        assert!(matches!(
            conflict_verdict(&activity, &oracle_view),
            OracleCompareVerdict::Disagree { .. }
        ));
        assert_eq!(
            conflicting_panes(&activity, &oracle_view),
            vec!["2".to_string()]
        );
    }

    /// Pane ids arrive as both string and number across surfaces. Dropping one form
    /// silently shrinks the candidate set, which reads as a busier fleet than exists.
    #[test]
    fn numeric_and_string_pane_ids_both_parse() {
        let activity = parse_activity_view(
            r#"{"agents":[{"pane":2,"observation_state":"idle","capture_provenance":"live",
                "observation_freshness":"fresh"}]}"#,
        )
        .expect("parses");
        let oracle_view = oracle(&[("2", "FREE")]);
        assert_eq!(
            decide(&activity, &oracle_view).dispatchable,
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

    /// PLANTED KNOWN-BAD, verbatim shape of the live envelope that dispatched an epic and a
    /// grading bead on 2026-09-02T20:21Z. The epic outranks everything (0.58) and the
    /// grading bead outranks the honest pick; both must be refused BY NAME and the honest
    /// pick must survive. Removing either arm of `dispatch_refusal` turns this RED.
    #[test]
    fn epics_and_owned_statuses_are_refused_by_name() {
        let text = r#"{"triage":{"recommendations":[
            {"id":"cp-epic-fleet-work-quality-08l6","score":0.58,"type":"epic","status":"open"},
            {"id":"cp-epic-fleet-work-quality-08l6.74.2","score":0.28,"type":"feature","status":"grading"},
            {"id":"cp-busy","score":0.27,"type":"bug","status":"in_progress"},
            {"id":"cp-honest","score":0.20,"type":"task","status":"open"},
            {"id":"cp-ready","score":0.10,"type":"bug","status":"ready"}
        ]}}"#;
        let (picks, skipped) = parse_recommendations_with_skips(text);
        assert_eq!(picks, vec!["cp-honest", "cp-ready"]);
        assert_eq!(
            skipped,
            vec![
                SkippedPick { bead: "cp-epic-fleet-work-quality-08l6".into(), reason: "type=epic".into() },
                SkippedPick { bead: "cp-epic-fleet-work-quality-08l6.74.2".into(), reason: "status=grading".into() },
                SkippedPick { bead: "cp-busy".into(), reason: "status=in_progress".into() },
            ]
        );
        assert_eq!(parse_recommendations(text), vec!["cp-honest", "cp-ready"]);
    }

    /// PLANTED: the ready list carries an epic, a grading bead, an assigned bead and two honest
    /// rows; priority decides the order (P0 before P2), never list position.
    #[test]
    fn ready_fallback_orders_by_priority_and_refuses_by_name() {
        let text = r#"[
            {"id":"cp-epic","issue_type":"epic","status":"open","priority":0,"assignee":null},
            {"id":"cp-grading","issue_type":"task","status":"grading","priority":0,"assignee":"Someone"},
            {"id":"cp-owned","issue_type":"task","status":"open","priority":0,"assignee":"Someone"},
            {"id":"cp-p2","issue_type":"bug","status":"open","priority":2,"assignee":null},
            {"id":"cp-p0","issue_type":"task","status":"open","priority":0,"assignee":null}
        ]"#;
        let (picks, skipped) = parse_ready_fallback(text);
        assert_eq!(picks, vec!["cp-p0", "cp-p2"]);
        assert_eq!(skipped.len(), 3);
        assert_eq!(skipped[0], SkippedPick { bead: "cp-epic".into(), reason: "type=epic".into() });
        assert_eq!(skipped[1].reason, "status=grading");
        assert_eq!(skipped[2].reason, "assigned");
    }

    /// An ABSENT field is not a disqualifier: an envelope without `type`/`status` (older bv,
    /// or the fixture above) must still rank, or a schema drift would silently idle the fleet.
    #[test]
    fn absent_type_and_status_fields_do_not_refuse() {
        let text = r#"{"triage":{"recommendations":[{"id":"cp-bare","score":0.4}]}}"#;
        let (picks, skipped) = parse_recommendations_with_skips(text);
        assert_eq!(picks, vec!["cp-bare"]);
        assert!(skipped.is_empty());
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

    /// THE CLOSED CODE SPACE `XC-PT-OUTCOME` CLAIMS.
    ///
    /// `main.rs` forwards `outcome.code` at four sites, which the exit-code registry gate
    /// records as pass-through sites. §6 of the registry declares their code space CLOSED
    /// at {0,1,2} rather than the 0..=255-of-a-child default, and this test is what makes
    /// that a checked claim instead of a sentence.
    ///
    /// The enumeration is over the SHAPES `run_outcome` branches on, not over sampled
    /// fixtures: each of conflicts / unconfirmed / unknowable / held / dispatchable /
    /// withheld is varied empty-vs-nonempty, all 64 combinations, against all three
    /// kernel verdicts. A sampled test would pass while a new branch returned 3 — which
    /// is exactly the risk the `withheld` branch introduced, so the slot count grew with
    /// the branch rather than after it.
    #[test]
    fn run_outcome_only_ever_yields_a_documented_exit_code() {
        const DOCUMENTED: [u8; 3] = [0, 1, 2];
        let verdicts = [
            OracleCompareVerdict::Agree { n: 0 },
            OracleCompareVerdict::Disagree {
                oracle_n: 2,
                product_n: 1,
            },
            OracleCompareVerdict::Unmeasurable { why: "probe" },
        ];
        let mut seen: BTreeSet<u8> = BTreeSet::new();
        let mut cases = 0usize;
        for bits in 0u8..64 {
            let pick = |slot: u8, name: &str| -> Vec<String> {
                if bits & (1 << slot) == 0 {
                    Vec::new()
                } else {
                    vec![name.to_string()]
                }
            };
            let mut decision = Decision {
                dispatchable: pick(0, "2"),
                conflicts: pick(1, "3"),
                unconfirmed: pick(2, "4"),
                unknowable: pick(3, "5"),
                held: pick(4, "6"),
                excluded: Vec::new(),
                withheld: Vec::new(),
            };
            // Reached through `withhold` rather than by writing the field, so the
            // enumeration exercises the real production path into the branch.
            if bits & (1 << 5) != 0 {
                decision.dispatchable.push("7".to_owned());
                decision.withhold(&ExclusionSet {
                    indices: BTreeSet::from(["7".to_owned()]),
                    unmatched: Vec::new(),
                });
            }
            for verdict in &verdicts {
                let outcome = run_outcome(&decision, verdict);
                cases += 1;
                assert!(
                    DOCUMENTED.contains(&outcome.code),
                    "run_outcome yielded UNDOCUMENTED code {} for {decision:?} — every code \
                     forwarded through outcome.code must have an XC-* row, and \
                     XC-PT-OUTCOME declares the space closed at {DOCUMENTED:?}",
                    outcome.code
                );
                seen.insert(outcome.code);
            }
        }
        assert_eq!(
            cases, 192,
            "the enumeration must actually run all 64x3 shapes"
        );
        assert_eq!(
            seen.iter().copied().collect::<Vec<u8>>(),
            DOCUMENTED.to_vec(),
            "ANTI-VACUITY: all three documented codes must be REACHABLE. A range test that \
             only ever observes 0 would pass against a function that can no longer refuse."
        );
    }

    /// The property `omp-orchestrator-oe2` established must survive the registry
    /// declaration: a CONFIDENT two-surface contradiction exits NONZERO and names the
    /// panes. A declaration that flattened this into a generic code would undo the fix it
    /// was written to document.
    #[test]
    fn a_declared_pass_through_still_carries_the_typed_refusal() {
        let (activity, oracle_view) = live_views();
        let decision = decide(&activity, &oracle_view);
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_ne!(
            outcome.code, 0,
            "a confident contradiction must exit NONZERO"
        );
        assert_eq!(outcome.code, 1);
        for pane in ["1", "2", "3"] {
            assert!(
                outcome.message.contains(pane),
                "the refusal must NAME pane {pane}: {}",
                outcome.message
            );
        }
        assert!(outcome.message.contains("SURFACE_CONFLICT"));
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

    // -----------------------------------------------------------------------------------
    // The orchestrator exclusion
    // -----------------------------------------------------------------------------------

    /// VERBATIM `tmux list-panes -a -F '#{pane_id} #{session_name} #{pane_index}'`,
    /// captured 2026-09-03T21:10Z on the live server, `omp-orchestrator` rows only.
    ///
    /// `%6` is INDEX 1 — the number the rosters key on and the number `AGENTS.md` measured
    /// refill proposing. The two namespaces do not coincide anywhere in this table, which
    /// is why the mapping cannot be guessed.
    const LIVE_PANE_TABLE: &str = "%5 omp-orchestrator 0\n\
                                   %6 omp-orchestrator 1\n\
                                   %7 omp-orchestrator 2\n\
                                   %8 omp-orchestrator 3\n\
                                   %9 omp-orchestrator 4\n\
                                   %0 control-plane 0\n\
                                   %1 control-plane 1\n";

    fn live_table() -> Vec<PaneRow> {
        parse_pane_table(LIVE_PANE_TABLE).expect("the live table parses")
    }

    #[test]
    fn the_pane_table_joins_the_id_and_index_namespaces_per_session() {
        let rows = live_table();
        let map = pane_index_map(&rows, "omp-orchestrator").expect("session is live");
        assert_eq!(map.get("%6").map(String::as_str), Some("1"));
        assert_eq!(map.get("%9").map(String::as_str), Some("4"));
        assert_eq!(
            map.len(),
            5,
            "the map must be SCOPED to one session — control-plane also has a %1 and an \
             index 1, and mixing the two excludes a stranger's pane: {map:?}"
        );
        assert_eq!(
            session_of_pane(&rows, "%6").as_deref(),
            Some("omp-orchestrator"),
            "$TMUX_PANE=%6 must resolve to the session that holds it"
        );
        assert!(session_is_live(&rows, "control-plane"));
        assert!(!session_is_live(&rows, "no-such-session"));
    }

    /// KNOWN-BAD LEG: an unreadable table is not an empty table.
    #[test]
    fn an_empty_or_unparseable_pane_table_is_none_not_an_empty_map() {
        assert_eq!(parse_pane_table(""), None);
        assert_eq!(parse_pane_table("\n\n"), None);
        // A tmux error on stdout must not parse as a roster of zero panes.
        assert_eq!(parse_pane_table("no server running on /tmp/x"), None);
        assert_eq!(pane_index_map(&live_table(), "gone"), None);
    }

    /// THE DEFECT `AGENTS.md` MEASURED: a plan naming pane 1, the orchestrator.
    ///
    /// Both surfaces confidently report the conductor idle — which they do, between its
    /// turns — and the join dispatches it. With `%6` excluded the same input plans only
    /// worker panes.
    #[test]
    fn the_orchestrator_pane_is_never_proposed_once_its_pane_id_is_excluded() {
        let activity = ntm(&[("1", "idle"), ("2", "idle"), ("3", "working")]);
        let oracle_view = oracle(&[("1", "FREE"), ("2", "FREE"), ("3", "BUSY")]);
        let mut decision = decide(&activity, &oracle_view);
        assert_eq!(
            decision.dispatchable,
            vec!["1".to_string(), "2".to_string()],
            "PRE-FIX BEHAVIOUR, pinned: without exclusion the orchestrator pane IS \
             proposed. If this ever stops holding, the exclusion below is being proved \
             against an input that never had the defect."
        );

        let map = pane_index_map(&live_table(), "omp-orchestrator").expect("session is live");
        let exclusions = resolve_exclusions(&["%6".to_owned()], Some(&map)).expect("resolves");
        assert_eq!(exclusions.indices, BTreeSet::from(["1".to_owned()]));
        decision.withhold(&exclusions);

        assert_eq!(decision.dispatchable, vec!["2".to_string()]);
        assert_eq!(decision.withheld, vec!["1".to_string()]);
        assert_eq!(decision.excluded, vec!["1".to_string()]);
        let plan = plan(&decision.dispatchable, &["bead-a".to_owned()], 8);
        assert!(
            plan.iter().all(|assignment| assignment.pane != "1"),
            "the ORCHESTRATOR pane reached a plan: {plan:?}"
        );
    }

    /// Exclusion is a CAPACITY decision, not a blindfold — the rule
    /// `crates/tick-monitor/src/main.rs:284` states for this same fleet.
    #[test]
    fn an_excluded_pane_keeps_its_place_in_the_findings() {
        let activity = ntm(&[("1", "idle")]);
        let oracle_view = oracle(&[("1", "BUSY")]);
        let mut decision = decide(&activity, &oracle_view);
        assert_eq!(decision.conflicts, vec!["1".to_string()]);
        decision.withhold(&ExclusionSet {
            indices: BTreeSet::from(["1".to_owned()]),
            unmatched: Vec::new(),
        });
        assert_eq!(
            decision.conflicts,
            vec!["1".to_string()],
            "a two-surface contradiction on the conductor's pane is still a finding"
        );
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 1, "{}", outcome.message);
        assert!(outcome.message.contains("SURFACE_CONFLICT"));
    }

    /// KNOWN-BAD LEG: a `%`-spec with no table REFUSES. Dropping it would propose the
    /// pane the exclusion exists to protect.
    #[test]
    fn an_unresolvable_pane_id_exclusion_refuses_rather_than_excluding_nothing() {
        let error = resolve_exclusions(&["%6".to_owned()], None)
            .expect_err("an unresolvable exclusion must refuse");
        assert!(error.contains("UNMEASURABLE"), "{error}");
        assert!(error.contains("detector=pane_index_map"), "{error}");
        assert!(error.contains("%6"), "{error}");

        // A spec in neither namespace is a usage refusal, not a silent no-op.
        let map = pane_index_map(&live_table(), "omp-orchestrator").expect("session is live");
        let bad = resolve_exclusions(&["orchestrator".to_owned()], Some(&map))
            .expect_err("an unparsed spec must refuse");
        assert!(bad.contains("detector=exclude_pane"), "{bad}");

        // KNOWN-GOOD on the same path: no specs at all needs no table and refuses nothing.
        assert_eq!(
            resolve_exclusions(&[], None).expect("no specs, no refusal"),
            ExclusionSet::default()
        );
        // And a bare index needs no table either — it can only ever match fewer panes.
        assert_eq!(
            resolve_exclusions(&["1".to_owned()], None)
                .expect("a bare index needs no table")
                .indices,
            BTreeSet::from(["1".to_owned()])
        );
    }

    /// A `%`-spec naming another fleet's pane is REPORTED, not silently swallowed.
    #[test]
    fn an_exclusion_aimed_at_another_session_is_reported_as_unmatched() {
        let map = pane_index_map(&live_table(), "omp-orchestrator").expect("session is live");
        let set = resolve_exclusions(&["%1".to_owned(), "%6".to_owned()], Some(&map))
            .expect("resolves");
        assert_eq!(
            set.indices,
            BTreeSet::from(["1".to_owned()]),
            "%1 belongs to control-plane and must NOT resolve to omp-orchestrator index 1"
        );
        assert_eq!(set.unmatched, vec!["%1".to_string()]);
    }

    /// Exclusion may empty the plan. It may NEVER turn a refusal into a pass, and the
    /// zero it does report must say WHY it is zero.
    #[test]
    fn withholding_the_last_dispatchable_pane_reports_exclusion_and_not_a_quiet_fleet() {
        let activity = ntm(&[("1", "idle"), ("2", "working")]);
        let oracle_view = oracle(&[("1", "FREE"), ("2", "BUSY")]);
        let mut decision = decide(&activity, &oracle_view);
        decision.withhold(&ExclusionSet {
            indices: BTreeSet::from(["1".to_owned()]),
            unmatched: Vec::new(),
        });
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 0, "{}", outcome.message);
        assert!(outcome.message.contains("EXCLUDED"), "{}", outcome.message);
        assert!(
            outcome.message.contains("withheld=[\"1\"]"),
            "the zero must name the pane it withheld: {}",
            outcome.message
        );
        assert!(
            !outcome.message.contains("genuine no-work"),
            "an excluded fleet is not a quiet fleet: {}",
            outcome.message
        );
        assert_eq!(
            decision.observed(),
            2,
            "a withheld pane was still OBSERVED and must stay in the denominator"
        );
    }

    /// KNOWN-GOOD LEG. An exclusion set that matches nothing must leave every worker pane
    /// dispatchable, and the run must still ECHO the set — the orchestrator is busy almost
    /// always, so a report that only spoke when exclusion bit would answer "is the
    /// conductor excluded?" only when it no longer matters.
    #[test]
    fn an_exclusion_that_bites_nothing_leaves_the_workers_dispatchable_and_still_echoes() {
        let activity = ntm(&[("1", "working"), ("4", "idle")]);
        let oracle_view = oracle(&[("0", "NO_AGENT"), ("1", "BUSY"), ("4", "FREE")]);
        let mut decision = decide(&activity, &oracle_view);
        let map = pane_index_map(&live_table(), "omp-orchestrator").expect("session is live");
        decision.withhold(
            &resolve_exclusions(&["%6".to_owned(), "%5".to_owned()], Some(&map)).expect("resolves"),
        );
        assert_eq!(
            decision.dispatchable,
            vec!["4".to_string()],
            "the worker pane must survive exclusion: {decision:?}"
        );
        assert!(decision.withheld.is_empty());
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 0, "{}", outcome.message);
        assert!(
            outcome.message.contains("excluded=[\"0\", \"1\"]"),
            "the echo must name the excluded indices even when none bit: {}",
            outcome.message
        );
    }

    // -----------------------------------------------------------------------------------
    // The root cause: a semantic exit code read as an I/O verdict
    // -----------------------------------------------------------------------------------

    /// VERBATIM `pane-dispatch-ready control-plane --json`, 2026-09-03T21:11Z, the run
    /// that **exited 1** while emitting this complete roster. Trimmed only in `reason`.
    const ZERO_FREE_ORACLE: &str = r#"{"schema":"zs.dispatch-ready.v1","panes":[
        {"pane":"0","reason":"no prompt marker in the live region","session":"control-plane","state":"BUSY"},
        {"pane":"1","reason":"pane buffer changed over 10s","session":"control-plane","state":"BUSY"},
        {"pane":"2","reason":"no prompt marker in the live region","session":"control-plane","state":"BUSY"},
        {"pane":"3","reason":"no prompt marker in the live region","session":"control-plane","state":"BUSY"},
        {"pane":"4","reason":"no agent process rendering in this pane (bare shell)","session":"control-plane","state":"NO_AGENT"}],
        "free_count":0}"#;

    /// KNOWN-GOOD LEG on the exact payload that used to be discarded.
    ///
    /// `free_count: 0` with exit 1 is a fully READ session in which nothing is free. That
    /// is the healthy busy-fleet answer, and the kernel could not reach it.
    #[test]
    fn a_zero_free_roster_is_readable_and_resolves_to_a_confidently_busy_fleet() {
        let payload = roster_readability(ZERO_FREE_ORACLE).expect("a full roster is readable");
        let oracle_view = parse_oracle_view(payload).expect("the verbatim payload parses");
        assert_eq!(
            oracle_view.panes.len(),
            5,
            "the payload the old caller threw away held five classified panes"
        );
        let activity = ntm(&[("1", "working"), ("2", "working")]);
        assert!(
            measurability_refusal(&measurability_verdict(
                &format!(
                    r#"{{"agents":[{},{}]}}"#,
                    ntm_row("1", "working"),
                    ntm_row("2", "working")
                ),
                ZERO_FREE_ORACLE
            ))
            .is_none(),
            "with the payload kept, the roster comparison is MEASURABLE — this is the \
             whole fix, and before it every busy fleet reported UNMEASURABLE"
        );
        let decision = decide(&activity, &oracle_view);
        assert!(decision.dispatchable.is_empty());
        let outcome = run_outcome(&decision, &conflict_verdict(&activity, &oracle_view));
        assert_eq!(outcome.code, 0, "{}", outcome.message);
    }

    /// KNOWN-BAD LEG. The fix must not widen into a pass: the paths that genuinely cannot
    /// read a session print nothing on stdout, and those must still refuse.
    #[test]
    fn an_empty_or_unparseable_roster_still_refuses_however_the_probe_exited() {
        for empty in ["", "   ", "\n\t\n"] {
            assert_eq!(
                roster_readability(empty),
                None,
                "an empty payload is unreadable no matter what the exit code said"
            );
        }
        // `tmux not available` / `no tmux sessions` go to stderr; stdout is empty, so the
        // arm arrives here as the empty string and the gate must still refuse.
        let refusal = measurability_refusal(&measurability_verdict(
            &format!(r#"{{"agents":[{}]}}"#, ntm_row("1", "working")),
            "",
        ))
        .expect("an unreadable oracle arm must refuse");
        assert_eq!(refusal.code, 2, "{}", refusal.message);
        assert!(refusal.message.contains("UNMEASURABLE"), "{}", refusal.message);
        assert!(
            refusal.message.contains("detector=pane_roster"),
            "{}",
            refusal.message
        );
        // Readable-but-not-JSON is unreadable too: a payload is not a roster.
        assert!(parse_oracle_view("no server running on /tmp/tmux-501/default").is_none());
    }

    // -----------------------------------------------------------------------------------
    // A plan is not an authorization
    // -----------------------------------------------------------------------------------

    /// KNOWN-BAD LEG: a plan row consumed with no approval is REFUSABLE, and an unmarked
    /// row cannot be laundered into one.
    #[test]
    fn a_plan_line_is_refused_without_an_approval_and_cannot_be_forged() {
        let proposal = plan_line(
            &Assignment {
                pane: "4".into(),
                bead: "omp-orchestrator-example".into(),
            },
            7_347,
        );
        assert!(proposal.contains(APPROVAL_MARKER), "{proposal}");
        assert!(proposal.contains(APPROVAL_REASON), "{proposal}");

        let unapproved =
            authorize_plan_line(&proposal, None).expect_err("a plan is not an authorization");
        assert!(unapproved.contains(APPROVAL_MARKER), "{unapproved}");
        assert!(
            unapproved.contains("detector=plan_consumed_unapproved"),
            "{unapproved}"
        );
        for blank in ["", "   "] {
            assert!(
                authorize_plan_line(&proposal, Some(blank)).is_err(),
                "a blank token is not an approval"
            );
        }
        // The OLD line shape — the bytes a dispatcher would have found before the marker
        // existed — must not pass even WITH a token, because its provenance is unknown.
        let legacy = "PLAN  pane=4 bead=omp-orchestrator-example bytes=7347";
        let forged = authorize_plan_line(legacy, Some("approval-2026-09-03"))
            .expect_err("an unmarked row has no establishable provenance");
        assert!(forged.contains("PLAN_UNMARKED"), "{forged}");

        // KNOWN-GOOD LEG: the gate is satisfiable. An unsatisfiable gate gets deleted
        // rather than obeyed, which is a slower death than no gate at all.
        assert!(authorize_plan_line(&proposal, Some("approval-2026-09-03")).is_ok());
    }

    /// `--apply` is withheld by DECISION and says so, at a code no reader can mistake for
    /// success. Before 2026-09-03 it was withheld by BREAKAGE, and repairing the roster
    /// arm would have granted it silently.
    #[test]
    fn actuation_is_a_named_refusal_and_not_a_silent_zero() {
        let refusal = actuation_refusal();
        assert_ne!(refusal.code, 0, "{}", refusal.message);
        assert_eq!(refusal.code, 2);
        assert!(refusal.message.contains(APPROVAL_MARKER), "{}", refusal.message);
        assert!(
            refusal.message.contains("detector=actuation_withheld"),
            "{}",
            refusal.message
        );
        assert!(
            refusal.message.contains("verb=--apply"),
            "the refusal must name the verb it refused: {}",
            refusal.message
        );
    }

    /// KNOWN-BAD LEG for the RESOLVED-BUT-UNAPPLIED class.
    ///
    /// The first live run of this fix printed `exclude_panes={"1"}` in its header and
    /// `excluded=[]` in its verdict, because the resolution and the application were two
    /// calls and only the first was made. This leg fires on that exact shape: `decide`
    /// ALONE must still propose the excluded pane (proving the input has the defect), and
    /// `decide_capacity` must not.
    #[test]
    fn decide_capacity_applies_the_exclusion_set_and_decide_alone_does_not() {
        let activity = ntm(&[("1", "idle"), ("4", "idle")]);
        let oracle_view = oracle(&[("1", "FREE"), ("4", "FREE")]);
        let exclusions = ExclusionSet {
            indices: BTreeSet::from(["1".to_owned()]),
            unmatched: Vec::new(),
        };

        let unapplied = decide(&activity, &oracle_view);
        assert_eq!(
            unapplied.dispatchable,
            vec!["1".to_string(), "4".to_string()],
            "the KNOWN-BAD path: classification alone proposes the excluded pane"
        );
        assert!(
            unapplied.excluded.is_empty() && unapplied.withheld.is_empty(),
            "a decision that never saw the exclusion set must not CLAIM one: {unapplied:?}"
        );

        let applied = decide_capacity(&activity, &oracle_view, &exclusions);
        assert_eq!(applied.dispatchable, vec!["4".to_string()]);
        assert_eq!(applied.withheld, vec!["1".to_string()]);
        assert_eq!(
            applied.excluded,
            vec!["1".to_string()],
            "the echo and the effect must come from the SAME call, or a log reader cannot \
             tell a working guard from a printed one"
        );

        // KNOWN-GOOD: an empty set through the same entry point changes nothing.
        assert_eq!(
            decide_capacity(&activity, &oracle_view, &ExclusionSet::default()).dispatchable,
            unapplied.dispatchable
        );
    }
}
