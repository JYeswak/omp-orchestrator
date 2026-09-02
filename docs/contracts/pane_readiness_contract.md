# pane_readiness_contract

Bead: `omp-orchestrator-pane-readiness-contract-n8c0`

## Purpose

Defines dispatch readiness for one terminal pane: the five states `pane-dispatch-ready` can
return (`FREE`, `BUSY`, `QUOTA_BLOCKED`, `NO_AGENT`, `UNREADABLE`), which of them are *claims*
and which are *fail-closed non-answers*, and the five laws that separate readiness from liveness
— `PR-L1` `safe_to_dispatch` is not liveness, `PR-L2` an UNKNOWN classification is not a busy
claim, `PR-L3` a positive readiness read needs two captures ≥75s apart, `PR-L4` a confident busy
claim beats a stale free read while an unknown one does not, `PR-L5` `NO_AGENT` is a bare shell
and never dispatchable. **`PR-L1` and `PR-L3` are stated here and NOT enforced by this crate**,
each named with the bead that will change it. Documents what IS at the commit below; changes no
crate source.

## Contract Artifacts

1. **Canonical artifact:** `crates/pane-dispatch-ready/src/lib.rs` — `PaneDispatchReadyState`,
   `classify`, `confirm_free`, `apply_composer_rc`, and the three marker regexes. There is
   deliberately no `artifacts/readiness_v1.json`: the input is a **terminal capture**, and the
   suite carries its fixtures inline so a reader sees the exact bytes each verdict was derived
   from rather than a filename.
2. **Runner:** `cargo test -p pane-dispatch-ready --test readiness_contract`
3. **Invariant suite:** `crates/pane-dispatch-ready/tests/readiness_contract.rs` — 11 legs. Two
   are pinned defects that go RED when a law becomes enforced, forcing this document to be
   updated in the same commit; both were proven to fire by mutation at authoring time.

> A contract naming no invariant suite is a DESCRIPTION. Item 3 is what makes the pinned defects
> load-bearing instead of a to-do list.

## 1. The state model

`PaneDispatchReadyState` is an exhaustive enum with no catch-all. The column that matters is the
last one: **only `FREE` is a positive claim.** Every other state is either a negative claim or an
admission that the question was not answered, and conflating those two is this document's subject.

| ID | state | meaning | kind |
|---|---|---|---|
| `PR-S-FREE` | `FREE` | agent present, no busy marker, prompt marker present, composer holds no typed text | **positive claim** |
| `PR-S-BUSY` | `BUSY` | a busy marker in the 6-line tail, or motion between captures, or no prompt marker in the live region | negative claim, and partly fail-closed |
| `PR-S-QUOTA` | `QUOTA_BLOCKED` | provider quota exhausted — "not busy, not free; needs spend, not a dispatch" | negative claim, distinct action |
| `PR-S-NOAGENT` | `NO_AGENT` | no agent process rendering; a bare shell | negative claim, never dispatchable |
| `PR-S-UNREADABLE` | `UNREADABLE` | empty capture; pane blank or `capture-pane` failed | **non-answer**, fail closed |

`BUSY` is doing double duty: it carries both "the agent is working: `esc to interrupt`" (a real
observation) and "no prompt marker in the live region — free-prompt not PROVEN (fail closed)" (an
absence of evidence). The reason string is the only thing distinguishing them, which is the same
overloading `docs/error_codes/exit_code_registry.md` records for exit 1.

## 2. The laws

| ID | law | enforced by this crate? | evidence |
|---|---|---|---|
| `PR-L1` | `safe_to_dispatch` is NOT liveness — a wedged pane accepts a packet and parks it forever | **NO** | wedged and idle verdicts are byte-identical, §2.1 — `readiness-l1-wedge-blind-46y7` |
| `PR-L2` | an UNKNOWN classification is not a busy claim | **YES**, and the framing needed correcting | §2.2 |
| `PR-L3` | a positive readiness read needs two captures ≥75s apart (`pane_observation_contract` `PO-L1`) | **NO** | 10s window against a 75s floor, §2.3 — `readiness-l3-motion-window-7523` |
| `PR-L4` | a CONFIDENT busy claim beats a stale free read; an UNKNOWN one does not | **YES** | §2.4 |
| `PR-L5` | `NO_AGENT` is a bare shell and never dispatchable | **YES** | §2.5 |

*Tests:* `crates/pane-dispatch-ready/tests/readiness_contract.rs` —
`l1_a_wedged_pane_still_classifies_free_in_this_crate`,
`l1_the_wedge_marker_is_detected_by_three_other_crates`,
`l2_a_codex_pane_can_be_unclassifiable_in_the_state_field`,
`l2_safe_to_dispatch_tracks_observation_state_not_state`,
`l3_this_crates_motion_window_is_below_the_75_second_floor`,
`l4_a_changed_second_capture_overturns_a_provisional_free`,
`l4_an_unreadable_second_capture_does_not_become_free_or_a_busy_claim`,
`l5_a_bare_shell_is_no_agent_even_with_a_perfect_prompt`,
`l5_an_empty_capture_is_unreadable_not_no_agent`.

### 2.1 `PR-L1` — the readiness authority is blind to the wedge, and three other crates are not

A wedged pane **accepted** the packet. It sits at `Press up to edit queued messages` and never
submits. Every surface `classify` reads still says free: an agent is rendering, the marker is not
in `BUSY_RE`, the buffer is unchanged, and the `π` prompt is on the last status line. So the
wedged and idle captures produce the **same verdict line**, asserted equal by the pinned leg.

The detection exists — three times, and naming where matters because a wrong reading of this
finding sends someone to build a fourth detector:

| crate | site |
|---|---|
| `fast-dispatch` | `src/lib.rs:321` |
| `fleet-monitor` | `src/lib.rs:157` |
| `tick-monitor` | `src/lib.rs:357` |

**INSTRUMENT NOTE.** The scan that found those three printed a hardcoded
`"(empty above = no crate detects it)"` label beneath non-empty output. The label was written
before the result and contradicted it. A label is not a measurement; the rows above are.

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

### 2.3 `PR-L3` — this crate's motion window is 1/7.5 of the floor

| authority | constant | value |
|---|---|---|
| `pane-truth` | `TWO_CAPTURE_MIN_SECS: i64` (`src/lib.rs:39`) | **75** |
| `tick-monitor` | `MIN_GAP_SECS: u64` (`src/lib.rs:490`) | **75** |
| `pane_observation_contract` `PO-L1` | two captures | **≥75s** |
| `pane-dispatch-ready` | `DEFAULT_MOTION_SECS: u64` (`src/lib.rs:36`, slept at `src/main.rs:304`) | **10** |

`tick-monitor/src/lib.rs:486` records why the floor is 75: *"measured, a lane deep in a long tool
call has a STATIC timer"*. A pane inside one long tool call renders nothing for far longer than
10 seconds, so its buffer is unchanged and `confirm_free` keeps the free read.

The asymmetry decides the severity: `buffer_changed == true` yields `BUSY`, which is fail-closed
and safe at any window. The unsafe direction is **unchanged-over-10s reading as idle** — a short
window cannot manufacture a false `BUSY`, only a false `FREE`.

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

Expect **12 passed**. Two legs (`l1_a_wedged_pane_still_classifies_free_in_this_crate`,
`l3_this_crates_motion_window_is_below_the_75_second_floor`) are **pinned defects**: they pass
because the law is unenforced and go RED the moment it is enforced. Their failure is the signal
that this document must be updated, not that the code broke. Both were proven to fire by mutation
— `DEFAULT_MOTION_SECS` 10→75 turned leg 3 RED (10 passed / 1 failed), and appending the wedge
marker to `BUSY_RE` turned both `l1_` legs RED (9 passed / 2 failed), each restored
byte-identically to sha256
`1409f2de20f241b4282544870633be5f57ff5fd643bc37921a47b07774ce8837`.

## Cross-References

- `crates/pane-dispatch-ready/src/lib.rs` — the implementation; `:36` the motion window, `:114`
  the state enum, `:243` `classify`, `:343` `confirm_free`
- `crates/pane-dispatch-ready/src/main.rs:304` — where the 10-second window is slept
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

- **Two laws are documented as UNENFORCED**, not fixed: `PR-L1` (`readiness-l1-wedge-blind-46y7`)
  and `PR-L3` (`readiness-l3-motion-window-7523`). No `src/` file was changed.
- **`crates/refill-idle-panes` is untouched** — another agent owns it — and so is
  `crates/omp-types`.
- **The `ntm` classifier is not under test.** `PR-L2` and `PR-L4` are asserted against a dated
  verbatim `--robot-activity` snapshot, so the suite tests the RULE rather than the live fleet. A
  live-`ntm` leg would be a flake that measures whatever the fleet happens to be doing.
- **The composer discriminator is out of scope here.** `apply_composer_rc` is fail-closed on any
  rc other than 0 or 1, and `crates/pane-dispatch-ready/tests/composer_rc.rs` already owns it.
- **No transport or receipt claim.** Whether a dispatched packet ARRIVED is
  `receiver-receipt`'s question, not readiness's.
- **No queue admission, bead selection, or lifecycle transition.**
- **`BUSY`'s two meanings are documented, not split.** Splitting them changes an observable
  contract and belongs with `PR-L1`'s fix, where a new state is already required.

## NO-CLAIM

**Readiness is not liveness, and this contract does not make it so.** `FREE` means every surface
this crate can read says free at the moment of capture; it does not mean the pane will process
work. Two of the five laws are stated and unenforced — a reader who takes `PR-L1` or `PR-L3` as a
guarantee has misread the document, which is why each carries its bead id inline.

`PR-L2`'s five-pane table is **one snapshot on one machine at one timestamp**, and §2.2.1
records what happened when a second reader was pointed at the same field two minutes later: it
refuted the conclusion. Neither measurement is retracted; the *advice* drawn from the first one
is. Treat every ntm field as advisory and confirm a positive free read against the last status
line at the two-capture grade. `omp-orchestrator-observation-state-false-idle-riqd` owns
characterising the failure — one pane over two minutes refutes a universal, and it does not
establish when `observation_state` lies or how often.

The suite proves the laws for `classify`/`confirm_free` as called directly. It does not prove any
caller consults them, waits for the second capture, or gates on `observation_state` — three
separate adoption questions with no test here.
