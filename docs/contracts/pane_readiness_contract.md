# pane_readiness_contract

Bead: `omp-orchestrator-pane-readiness-contract-n8c0`

## Purpose

Defines dispatch readiness for one terminal pane: the six states pane-dispatch-ready can return
(FREE, BUSY, WEDGED, QUOTA_BLOCKED, NO_AGENT, UNREADABLE), which of them are claims
and which are fail-closed non-answers, and the five laws that separate readiness from liveness.
— PR-L1 safe_to_dispatch is not liveness, PR-L2 an UNKNOWN classification is not a busy
claim, PR-L3 a positive readiness read needs two captures ≥75s apart, PR-L4 a confident busy
claim beats a stale free read while an unknown one does not, PR-L5 NO_AGENT is a bare shell
and never dispatchable. **PR-L1 and PR-L3 are enforced by the current source.** PR-L1 delegates
wedge recognition to tick-monitor; PR-L3 delegates two-capture interval and motion evidence to
PaneObservation. Documents what IS at the commits below; changes no crate source.

## Contract Artifacts

1. **Canonical artifact:** crates/pane-dispatch-ready/src/lib.rs — PaneDispatchReadyState,
   classify, confirm_free, apply_composer_rc, and the marker authority edge. There is deliberately
   no artifacts/readiness_v1.json: the input is a terminal capture, and the suite carries its
   fixtures inline so a reader sees the exact bytes each verdict was derived from rather than a
   filename.
2. **Runner:** cargo test -p pane-dispatch-ready --test readiness_contract
3. **Invariant suite:** crates/pane-dispatch-ready/tests/readiness_contract.rs — 13 legs. The
   original 11-leg characterization suite now includes the post-landing wedge authority and
   state-registry legs. Former PR-L1 and PR-L3 pinned-defect assertions were replaced when the
   corresponding source paths became enforced.

> A contract naming no invariant suite is a DESCRIPTION. Item 3 is what makes the evidence
> load-bearing instead of a to-do list.


## 1. The state model

`PaneDispatchReadyState` is an exhaustive enum with no catch-all. The column that matters is the
last one: **only `FREE` is a positive claim.** Every other state is either a negative claim or an
admission that the question was not answered, and conflating those two is this document's subject.

| ID | state | meaning | kind |
|---|---|---|---|
| PR-S-FREE | FREE | agent present, no busy marker, prompt marker present, composer holds no typed text | **positive claim** |
| PR-S-BUSY | BUSY | a busy marker in the 6-line tail, or motion between captures, or no prompt marker in the live region | negative claim, and partly fail-closed |
| PR-S-WEDGED | WEDGED | a packet arrived and is parked unsubmitted; an operator must submit or clear it | negative claim, operator action required |
| PR-S-QUOTA | QUOTA_BLOCKED | provider quota exhausted — not busy, not free; needs spend, not a dispatch | negative claim, distinct action |
| PR-S-NOAGENT | NO_AGENT | no agent process rendering; a bare shell | negative claim, never dispatchable |
| PR-S-UNREADABLE | UNREADABLE | empty capture; pane blank or capture-pane failed | **non-answer**, fail closed |

`BUSY` is doing double duty: it carries both "the agent is working: `esc to interrupt`" (a real
observation) and "no prompt marker in the live region — free-prompt not PROVEN (fail closed)" (an
absence of evidence). The reason string is the only thing distinguishing them, which is the same
overloading `docs/error_codes/exit_code_registry.md` records for exit 1.

## 2. The laws

| ID | law | enforced by this crate? | evidence |
|---|---|---|---|
| PR-L1 | safe_to_dispatch is NOT liveness — a wedged pane accepts a packet and parks it forever | **YES for the named wedge path; not general liveness** | the classifier consults tick-monitor and returns WEDGED, not FREE; §2.1 |
| PR-L2 | an UNKNOWN classification is not a busy claim | **YES**, and the framing needed correcting | §2.2 |
| PR-L3 | a positive readiness read needs two captures ≥75s apart (pane_observation_contract PO-L1) | **YES** | PaneObservation enforces the canonical interval and motion evidence; §2.3 |
| PR-L4 | a CONFIDENT busy claim beats a stale free read; an UNKNOWN one does not | **YES** | §2.4 |
| PR-L5 | NO_AGENT is a bare shell and never dispatchable | **YES** | §2.5 |

*Tests:* crates/pane-dispatch-ready/tests/readiness_contract.rs —
 l1_a_wedged_pane_is_distinguishable_from_an_idle_one,
 l1_the_wedge_authority_is_consulted_not_reimplemented,
 l1_state_registry_round_trips_and_covers_every_state_the_classifier_emits,
 l2_a_codex_pane_can_be_unclassifiable_in_the_state_field,
 l2_safe_to_dispatch_tracks_observation_state_not_state,
 l2_the_observation_channel_was_confidently_wrong_about_a_working_pane,
 l3_this_crates_motion_window_meets_the_75_second_floor,
 l4_a_changed_second_capture_overturns_a_provisional_free,
 l4_an_unreadable_second_capture_does_not_become_free_or_a_busy_claim,
 l5_a_bare_shell_is_no_agent_even_with_a_perfect_prompt,
 l5_an_empty_capture_is_unreadable_not_no_agent,
 a_planted_busy_marker_is_caught_in_the_tail_and_ignored_above_it,
 a_quota_exhausted_pane_is_neither_busy_nor_free.

### 2.1 `PR-L1` — the readiness authority now CONSULTS the wedge authority instead of guessing

A wedged pane **accepted** the packet. It sits at `Press up to edit queued messages` and never
submits. Every surface `classify` read still said free — an agent is rendering, the marker is not
in `BUSY_RE`, the buffer is unchanged, and the `π` prompt is on the last status line — so the
wedged and idle captures produced the **same verdict line**.

The detection already existed three times over, which is why the fix is a **dependency edge and
not a fourth regex**:

| crate | detects | how |
|---|---|---|
| `fast-dispatch` | `src/lib.rs:325` | own `contains` |
| `fleet-monitor` | `src/lib.rs:170` | own `contains` |
| `tick-monitor` | `src/lib.rs:391` | own `contains` — **the authority** |
| `pane-dispatch-ready` | `src/lib.rs` | **consults `tick_monitor::classify`** |

`classify` now returns `PaneDispatchReadyState::Wedged` when
`tick_monitor::classify(text) == PaneState::Wedged`. Three properties of that choice are
load-bearing:

1. **`Wedged` is its own state, not folded into `Busy`.** `Busy` means *come back later*; a parked
   packet never clears without an operator. The reason string names the action — *"an operator must
   submit or clear the queued message; waiting will not clear it"* — so a caller reading only the
   line still learns the difference.
2. **Delegation is strictly richer than the anchor the bead proposed.** `tick_monitor::classify`
   recognises **two** parked-packet footers (`Press up to edit queued messages` and `Messages to be
   submitted after next tool call`); adding the single marker to `BUSY_RE` would have caught one.
   Neither string appears in this crate. `receiver-receipt`, whose own contract forbids I/O,
   already consumes `tick_monitor::classify`, so this is a precedented edge onto a pure classifier.
3. **Order: after quota, before busy.** `tick-monitor` checks `Wedged` before its own spinner
   branch because a wedged pane can still render a live spinner; a busy-first order here would
   score it `BUSY` and hide it behind *come back later*.

**The second pinned leg did NOT fire, and that is the sharper finding.** It asserted
`!own.contains("Press up to edit queued messages")` — a search of this crate's own source text.
The correct fix leaves that string absent, so the pin stayed green through the very change it
existed to catch. **A pin keyed on source text cannot see a fix implemented by delegation.** It has
been replaced by `l1_the_wedge_authority_is_consulted_not_reimplemented`, which asserts the
behaviour, the dependency edge, and the second footer.

**Differential.** Acceptance 4 asked that the shell gain the same clause or the divergence be
declared with a reason. The shell cannot gain it: `bin/` no longer exists in this repository and
`AGENTS.md`'s first rule forbids re-adding a `.sh` file, so every differential test already skips
with `reason=missing_script` and compares **0 cases**. The divergence is declared in
`tests/differential.rs::DECLARED_DIVERGENCES` and checked in two halves —
`declared_divergences_are_real` runs the row through the **Rust binary** with no shell involved and
fails if the declaration stops describing this binary, and the *"the shell still disagrees"* half
announces loudly that it did not run.

**INSTRUMENT NOTE.** The scan that found the original three printed a hardcoded
`"(empty above = no crate detects it)"` label beneath non-empty output. The label was written
before the result and contradicted it. A label is not a measurement; the rows above are. The line
numbers in that table had also drifted by the time this fix landed (321/157/357 → 325/170/391),
which is why they were re-derived by text search rather than trusted.

### 2.2 `PR-L2` — UNKNOWN is not a busy claim, and the derivation is not what it looks like

Measured live, `ntm --robot-activity=omp-orchestrator` at `2026-09-02T03:28:05Z`, five panes, one
`captured_at`:

| pane | agent | `state` | conf | `observation_state` | obs conf | `safe_to_dispatch` |
|---|---|---|---|---|---|---|
| 1 | claude | `THINKING` | 0.8 | `idle` | 0.95 | **true** |
| 2 | codex | `ERROR` | 0.95 | `working` | 0.95 | false |
| 3 | codex | `UNKNOWN` | **0.5** | `working` | 0.95 | false |
| 4 | omp-glm | `THINKING` | 0.8 | `working` | 0.95 | false |
| 5 | omp-glm | `THINKING` | 0.8 | `working` | 0.95 | false |

`state=UNKNOWN, confidence=0.5` on a codex pane is confirmed: a coin flip, not a claim. And the
two codex panes **disagree with each other** in `state` (`ERROR` vs `UNKNOWN`) while agreeing in
`observation_state` — that disagreement is the field's unreliability, measured within one
snapshot.

**A CORRECTION.** The standing account says ntm *derives* `safe_to_dispatch:false` from the
UNKNOWN non-answer. It does not. Pane 1 is the counter-example in the same snapshot:
`state=THINKING` — a busy-sounding word — with `observation_state=idle` at **0.95** and
`safe_to_dispatch=true`. Across all five rows `safe_to_dispatch == (observation_state == "idle")`,
and the observation channel is 0.95-confident on every pane. **The derivation follows
`observation_state`, never `state`.**

#### 2.2.1 AMENDMENT — and it retracts this section's own advice

That correction concluded "gate on `observation_state`". **Six minutes after this document
landed, a differently-shaped reader refuted it.** Measured 2026-09-02T03:42–03:44Z on pane 1,
two `--robot-tail` captures ~95 seconds apart — above the 75-second floor:

```
A   ⠏ 26m  · ◕ Opus 5 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *8 ?5 · ◫ 77.4%/1M
B   ⠼ 28m  · ◕ Opus 5 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *9 ?5 · ◫ 77.9%/1M
```

Both lines carry a braille spinner AND an elapsed timer — the v18 WORKING signature. The timer
advanced 26m → 28m, the spinner changed, and the dirty-file count moved `*8` → `*9`. That
satisfies `PO-L1` on both clauses, so **WORKING is proven, not inferred.** At those same two
timestamps `ntm` reported `observation_state: "idle"` at `observation_confidence: 0.95` and
`safe_to_dispatch: true`; `--robot-tail` independently reported `state: "idle"`. Both surfaces
were confidently wrong about a pane 28 minutes into a turn.

**This is worse than the `UNKNOWN` case above.** An UNKNOWN at 0.5 announces its own weakness;
an `idle` at 0.95 does not. So the rule is NOT "gate on `observation_state`" — it is:

> No single ntm field is sufficient. A positive free read must be confirmed against the last
> status line at the two-capture grade, which is what `pane-truth` already does.

Both measurements stand and they are consistent: in the five-pane snapshot the derivation
faithfully followed `observation_state`, and here `observation_state` itself was false. Together
they say the derivation is faithful to a field that can lie. Owned by
`omp-orchestrator-observation-state-false-idle-riqd`; pinned by
`l2_the_observation_channel_was_confidently_wrong_about_a_working_pane`.

The advice retracted here survived **six minutes** in a landed contract. That is the argument
for the pinned-defect pattern: the claim was written down precisely enough to be refuted, and the
refutation is now a test rather than a memory.

#### 2.2.2 AMENDMENT 2 — it reproduces, staleness is refuted, and the payload already contradicted itself

`§2.2.1` retracted the advice. This bounds the defect, answers which side is broken, and replaces
the retraction with a cheaper rule. Owned by `omp-orchestrator-observation-state-false-idle-riqd`.

**It reproduces on the first attempt.** 2026-09-02T23:38:21Z / 23:41:59Z, same pane, twenty hours
later, a **218-second** gap:

```
A   ⠋ 17m  · ◕ Fable 5.1 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *144 ?12 · ◫ 83.0%/1M
B   ⠼ 20m  · ◕ Fable 5.1 · ⏸ Goal 878K · 📁 ~/Developer/omp-orchestrator · ⑂ main *133 ?11 · ◫ 83.2%/1M
```

`observation_state: "idle"`, `observation_confidence: 0.95`, `safe_to_dispatch: true` at both
timestamps, from both `--robot-activity` and `--robot-tail`.

**Not a freshness bug.** The observation row's own `capture_provenance` is `"live"`, its
`capture_collected_at` is `2026-09-02T23:41:59Z` — equal to the tail's `captured_at` **to the
second** — `observation_freshness: "fresh"`, and `source_health.tmux.freshness_sec: 0`. The
observation and the spinner come from the same capture at the same instant.

**Not a missed read either, which is sharper than the two options the bead offered.**
`detected_patterns` for the false-idle row is
`["failed_text", "claude_unicode_prompt", "braille_spinner"]`. **The spinner was DETECTED and an
idle-side signal outranked it.** That makes this a PRECEDENCE defect upstream, not a footer the
classifier cannot see — a different report, and a much easier one to fix.

**The bound.** 22 rows across five live sessions at 23:39Z: a bare `braille_spinner` implied
`observation_state == "working"` on **12 of 13** rows carrying it, and `omp-orchestrator` pane 1
was the only violation. Four features were perfectly correlated with the single idle row inside one
session, so none could be blamed from it; widening the sample **refutes three** —
`agent_type=claude` (zeststream-cast pane 1 is claude and reads working),
`claude_unicode_prompt` (same row carries it), and `failed_text` (a clutterfreespaces row carries
it and works). The surviving candidate has **n=1 and is not claimed.** Pinned by
`l2_three_candidate_triggers_for_the_false_idle_are_refuted_by_the_wider_sample`.

**A CHEAPER RULE THAN `§2.2.1`'s.** That amendment concluded a positive free read must be confirmed
against the last status line at the two-capture grade. True, and it costs 75 seconds. But the
false-idle payload **already contradicted itself**: `state: "THINKING"` and
`observation_state: "idle"` in the same row. `ntm-fleet-monitor::readiness()` scores that
`Conflicting`, so `dispatchable()` and `capture_eligible()` both refused the live row with **no
second capture at all**. Verified with `agent_type` substituted to `"omp"` so `is_omp()` could not
be the reason — `readiness == Conflicting`, `freshness == Live`, `safe_to_dispatch == true`, both
predicates false. So:

> **Both ntm channels must AGREE, and a disagreement is a refusal.** That is available inside a
> single payload. The two-capture confirmation of `§2.2.1` is still required for a positive free
> read, because agreement is not proof — but disagreement is sufficient to refuse, and it is free.

This also says the retracted advice was worse than merely incomplete: *"gate on `observation_state`,
never on `state`"* would have **discarded the only channel that caught this**.

**Site audit.** `git grep -n --no-index -F 'observation_state' -- 'crates/*/src/*'`: the three
kernels that decide dispatch — `pane-truth`, `tick-monitor`, `pane-dispatch-ready` — carry **0**
sites each (positive control: the same reader finds `fn ` in 2, 3 and 2 files respectively). Of the
remaining non-fixture sites, `ntm-fleet-monitor::capture_eligible` reads the field and names why in
its own doc, guarded by the conflict check and pinned by
`l2_a_confidently_false_idle_is_refused_by_the_conflict_check`; `fleet-composite` reads it for a
health SCORE and never dispatches; `refill-idle-panes` reads it to select panes and is owned by a
separate bead.

**NO-CLAIM.** Two reproductions on one machine. Enough to say the defect re-triggers and that
staleness is not its cause; not enough to name its trigger. The 12-of-13 bound is a sample, not a
rate.

### 2.3 PR-L3 — the canonical two-capture floor is enforced

| authority | constant | value |
|---|---|---|
| pane-truth | TWO_CAPTURE_MIN_SECS: i64 | **75** |
| tick-monitor | MIN_GAP_SECS: u64 | **75** |
| pane_observation_contract PO-L1 | two captures | **≥75s** |
| pane-dispatch-ready | TWO_CAPTURE_MIN_SECS = omp_types::MIN_TWO_CAPTURE_INTERVAL_SECS | **75** |

The floor is not an idle proof. tick-monitor records why 75 seconds is required: a lane deep in a
long tool call can have a STATIC timer. The canonical PaneObservation constructor rejects an
under-floor pair before confirm_free can classify motion as evidence for BUSY or FREE.

The former 10-second DEFAULT_MOTION_SECS path is no longer present. The previous short-window
finding is retained as history in readiness-l3-motion-window-7523; its mutation changed 10 to 75
and made the pinned leg RED. The current l3 leg asserts the canonical 75-second value and source
use. A short or zero interval is UNREADABLE and fail-closed, not FREE.

### 2.4 `PR-L4` — the confident/unknown asymmetry, and it holds

A changed spinner-stripped hash between captures overturns a provisional `FREE`, and the reason
names the motion rather than merely asserting `BUSY`. An **empty** second capture does not become
`FREE` and does not become a busy claim either: it is `UNREADABLE`, fail closed. That is
`pane_observation_contract` `PO-L2` — "unknown never becomes idle" — with the addition that it
also never becomes *working*. Conflating "I could not look" with "it is busy" is the same defect
class as an empty scan set reporting as a pass.

The known-good arm is asserted alongside: an **unchanged** second capture leaves `FREE` standing.
Without it the crate could be over-strict and still look green, and an over-strict readiness
authority gets routed around.

### 2.5 `PR-L5` — `NO_AGENT` is checked before the prompt, and that ordering is the law

A bare shell has a *perfect* prompt marker (`$ `), so the agent test must run first or a login
shell classifies `FREE`. In `classify` the agent check is second, after only the empty-capture
guard, and it returns early. The suite proves attributability: the same text with an agent marker
appended is no longer `NO_AGENT`, so the verdict is caused by the missing agent and not by
anything else in the fixture.

`NO_AGENT` and `UNREADABLE` are two different absences with two different operator actions — a
bare shell needs an agent started, a failed capture needs the pane or tmux investigated. Merging
them would make a `capture-pane` failure look like an idle terminal.

## Validation

```bash
cargo test -p pane-dispatch-ready --test readiness_contract
```

Expect 13 passed in the current workspace. The original 11-leg characterization suite has two
additional post-landing legs for the delegated wedge authority and exhaustive state registry.
PR-L1 and PR-L3 are no longer pinned defects: the former now returns WEDGED for the parked-packet
fixture, and the latter now uses the canonical 75-second two-capture floor.

Historical mutation evidence remains reproducible. Mutation A changed the former
DEFAULT_MOTION_SECS value from 10 to 75 at its single source site and produced 10 passed / 1
failed. Mutation B appended the wedge marker to BUSY_RE and produced 9 passed / 2 failed. Both
mutations were restored byte-identically; the recorded source hash is
1409f2de20f241b4282544870633be5f57ff5fd643bc37921a47b07774ce8837.

## Cross-References

- `crates/pane-dispatch-ready/src/lib.rs` — the implementation; `:121` the state enum, `:269`
  `classify` (the wedge clause at `:299`), `:396` `confirm_free`. Line numbers re-derived by
  symbol search 2026-09-02; the previous four had all drifted.
- `crates/pane-dispatch-ready/src/main.rs:332` — where the motion window is slept
- `crates/tick-monitor/src/lib.rs:387` — `classify`, the consulted wedge authority
- `crates/pane-dispatch-ready/tests/differential.rs` — `DECLARED_DIVERGENCES` and its two-half check
- `crates/pane-dispatch-ready/tests/readiness_contract.rs` — the invariant suite
- `crates/pane-dispatch-ready/tests/differential.rs` — the shell original as differential oracle
- `crates/pane-truth/src/lib.rs:39` — `TWO_CAPTURE_MIN_SECS = 75`, the floor
- `crates/tick-monitor/src/lib.rs:486-490` — `MIN_GAP_SECS = 75` and the measured reason
- `crates/fast-dispatch/src/lib.rs:321`, `crates/fleet-monitor/src/lib.rs:157`,
  `crates/tick-monitor/src/lib.rs:357` — the three crates that DO detect the wedge marker
- `docs/contracts/pane_observation_contract.md` — `PO-L1` two-capture dominance, `PO-L2` unknown
  never becomes idle, `PO-L5` dispatch admissibility is independent of liveness
- `docs/contracts/subprocess_contract.md` — the bounded-subprocess boundary this crate's
  `spawn_timeout` predates
- `docs/error_codes/exit_code_registry.md` — `XC-001`, the same overloading defect in exit codes
- `docs/plans/plan_to_write_the_document_corpus.md` — this is document #17 of 78
- `AGENTS.md:419` — the wedged-pane failure, recorded before this contract
- `docs/contracts/asupersync_process_grade.md` — the grade this document is scored by

## Non-Coverage

- This contract does not claim general pane liveness. PR-L1 covers the named parked-packet
  wedge path, and PR-L3 covers the canonical two-capture evidence floor; neither proves that a
  pane will continue processing work when those observations are absent.
- No src/ file was changed for this contract update. The fixes described here are existing source
  state, not implementation work in this bead.
- crates/refill-idle-panes is untouched — another agent owns it — and so is crates/omp-types.
- The ntm classifier is not under test. PR-L2 and PR-L4 use a dated verbatim robot-activity
  snapshot, so the suite tests the RULE rather than the live fleet. A live-ntm leg would be a flake
  that measures whatever the fleet happens to be doing.
- The composer discriminator is out of scope here; composer_rc.rs already owns its fail-closed
  behavior.
- No transport, receipt, queue admission, bead selection, or lifecycle-transition claim belongs
  here.
- BUSY still carries two meanings. Splitting them changes an observable contract and is separate
  work from the named wedge and two-capture laws.

## NO-CLAIM

Readiness is not liveness, and this contract does not make it so. FREE means every surface this
crate can read says free at the moment of capture; it does not mean the pane will process work.
PR-L1 and PR-L3 are bounded named laws, not universal guarantees: a wedge can render a different
footer, and two captures can remain static for minutes.

The PR-L2 five-pane table is one snapshot on one machine at one timestamp, and the amendment
records a second reader that refuted the first advice. Treat every ntm field as advisory and
confirm a positive free read against the last status line at the two-capture grade. The suite proves
classify/confirm_free when called directly; it does not prove any caller consults them, waits for a
second capture, or gates on observation_state — three separate adoption questions with no test here.
