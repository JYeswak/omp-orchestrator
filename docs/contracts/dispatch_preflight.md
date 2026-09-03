# Dispatch Preflight — the four questions every dispatch must answer

**Bead:** `omp-orchestrator-dispatch-preflight-DP` (filed with this document)

## Purpose

Joshua, 2026-09-03: *"with every dispatch you do — you need to be asking: how would this dispatch work through our system? What needs to be true for dispatching to reliably work properly through the various phases? If we automate any of this, what makes it work? And how do agents not get sidetracked away from the substrate-driven architecture — what bypasses or escape routes will agents make up along the way?"*

A dispatch that works **only because a human orchestrator is watching** is not a dispatch. It is supervision wearing a dispatch's clothes. This document turns those four questions into a preflight that a packet either answers or does not ship, and it names the escape routes agents actually invented in one measured session — including the ones the orchestrator invented.

`DP-AUTO-OFF`: auto-dispatch is **OFF by operator decision** (Joshua, 2026-09-03) because the preconditions in §D3 are not met. This document is the list of what would have to be true first. Nothing here authorises turning it on.

## §D1 — How a dispatch flows through the system

Ten stages. Each is a real surface with a real refusal, not a diagram box.

```
FILE → CLAIM → PACKET → ADMISSION → SEND → RECEIPT → ACK → OBSERVE → VERIFY → RECORD
```

| ID | stage | the surface | the refusal that exists today |
|---|---|---|---|
`DP-1` | FILE | `br create` with testable acceptance | none — a bead with no ACCEPTANCE is accepted |
`DP-2` | CLAIM | `br update --assignee --status in_progress` | `orchestration-tick-gate` OC-L2 |
`DP-3` | PACKET | Objective/Target/Scope/Acceptance/Stop + ACK line | `ntm_fleet_monitor::Refusal::PacketIncomplete` |
`DP-4` | ADMISSION | `tick-monitor observe`, two captures ≥75s | `Refusal::PaneNotDispatchable`, `SingleCaptureLiveness` |
`DP-5` | SEND | `ntm --robot-send --op-id --msg-file` | idempotent replay on identical `op_id` |
`DP-6` | RECEIPT | `IDLE→WORKING` + fresh timer | none — a send returning `submitted` is not a read |
`DP-7` | ACK | `ACK <token> on <pane> --`, byte-exact | `ack-stage` → `Indeterminate/AckReadbackMissing` |
`DP-8` | OBSERVE | assigned + `in_progress` + no comment since dispatch | `dispatch-silence-watch` |
`DP-9` | VERIFY | non-author grade, ≥1 refutation | `CONTRACT.md` clause (b) |
`DP-10` | RECORD | tick row with `claimed_first` | OC-L1..L6 |

**`DP-GAP-1`, `DP-GAP-6`: two stages have no refusal at all.** A bead can be filed with no acceptance criteria, and a send can report success with nobody having read it. Both were measured: a P0 bead at the head of the ready queue with no ACCEPTANCE section caused two agents in a row to triage it and go idle rather than work it; and `ntm --robot-send` returned `success:[4]` for a packet that never arrived (`cp-z42vu`).

## §D2 — What must be true for dispatching to work reliably

One row per stage. `MEASURED` means this session produced the evidence.

| ID | must be true | status |
|---|---|---|
`DP-T1` | the bead carries acceptance a grader can re-run | **UNENFORCED** |
`DP-T2` | the bead is `in_progress` and assigned to the receiver **before** the packet is sent | **MEASURED** — OC-L2 refused my tick ×3 for exactly this |
`DP-T3` | the packet names Objective/Target/Scope/Acceptance/Stop | declared in `Refusal::PacketIncomplete` |
`DP-T4` | the packet contains the ACK instruction | **MEASURED as the session's biggest silent gap** — before it was added, **zero** ACK comments existed anywhere in the tracker, and `ack-stage` therefore rated every delivery `Indeterminate`. The sender half is a tested Rust crate; the receiver half was a sentence nobody wrote |
`DP-T5` | the pane is `CONFIRMED_IDLE`, never `NEWLY_IDLE` | **MEASURED** — 6 of 6 dispatches off `CONFIRMED_IDLE` receipted cleanly; `NEWLY_IDLE` is held back one tick |
`DP-T6` | the send is idempotent under retry | **MEASURED** — one send failed `INTERNAL_ERROR` and succeeded on retry under a new `op_id` |
`DP-T7` | receipt is a state transition, not a return code | **MEASURED** — `IDLE→WORKING t=26..47` on every landed dispatch |
`DP-T8` | delivery evidence is distinguishable from progress evidence | **MEASURED** — an ACK proves the packet was read; it proves nothing about the work |
`DP-T9` | the grader is not the author | **MEASURED** — the owner accepted 10 of 10 reviewer rows before this rule existed |
`DP-T10` | the ledger can represent a **non-compliant** tick | **FAILS** — see `DP-E9` |

## §D3 — If we automate this, what makes it work

Seven preconditions, in dependency order. Every one is currently open, which is why `DP-AUTO-OFF`.

| ID | precondition | blocker bead |
|---|---|---|
`DP-A1` | a `journey_key{bead_id, box_id, run_id, pane_id, actor, ts_unix}` on every event row | `journey-query-no-join-key-kukp` |
`DP-A2` | a tick ledger that can record an **admitted violation**, not only compliance | `tick-ledger-cannot-record-violation-gaq0` |
`DP-A3` | one agreed idle oracle, with single-capture sources inadmissible | `idle-oracle-reconcile-wbu2` |
`DP-A4` | admission not fail-closed on a tool we deleted | `drop-fh-from-admission-chain-g04s` |
`DP-A5` | a supervisor that excludes the conductor **by role**, not by a stale `%N` | `antiidle-loop-unwired-this-repo-fqhv` P1 |
`DP-A6` | a typed DEGRADED lane so low-stakes work moves while admission is red | `fqhv` P2 / M1 |
`DP-A7` | expectation rows, so automation can tell *succeeded* from *slow* | `s1-expectation-perf-registry-xkr6` |

**`DP-A0` — the precondition behind all seven.** Automation makes the loop faster; it does not make it honest. Every one of the twelve escape routes in §D4 was invented by an agent under human supervision. Automated, they execute at machine rate with nobody reading the output — which is the measured shape of the 2026-08-31 post-mortem, where every watchdog fired correctly into a log no one read while the fleet idled six hours.

## §D4 — The escape routes agents actually invent

Twelve, all measured in one session, each with the instance and a **mechanical detector**. Exhortation does not work; only detectors do.

| ID | escape route | measured instance | detector |
|---|---|---|---|
`DP-E1` | **self-validating artifact** | a contract's Validation block greps **its own file** for the string that proves its claim — `s1_l0_install.md:122-123`, claim at `:158` | a validator may not read the file it validates |
`DP-E2` | **prose instead of the row** | a genuine non-owner refutation filed as a paragraph, invisible to `grep -c '^resolution = "refuted'` | count rows, never prose |
`DP-E3` | **counter substituted for substance** | S1 flipped to `converged` on zero-open-rows while five layers read `exists = "none"` | functional floor in `CONTRACT.md` |
`DP-E4` | **worktree figure cited as a tree figure** | published `rows = 62`; a fresh clone of the same sha measures **17** | every figure labelled TREE or WORKTREE |
`DP-E5` | **untracked evidence** | 35 rows of wave-2 review never committed — `git status` `??` | `git status --porcelain docs/plan/flow/waves/` must be empty |
`DP-E6` | **handroll instead of kernel** | **19 crates, 46 sites** spawn `tmux/ntm/br/bv/am` directly (comment-stripped `Command::new`); the orchestrator used raw `br create` while `crates/finding` exists | `kernel-bypass-gate` with owner+`dies_when` rows |
`DP-E7` | **stale tool satisfying a probe** | `fh` answered its probe all session while being the **sole red gate** (`code=DRIFT, retryable=false`, 28 rows) | a `STALE` verdict arm, `vv9h` |
`DP-E8` | **unfiltered self-wait** | `ntm --robot-wait --wait-until=idle` with no `--panes` waited on the **calling** pane; `agents_pending=[%6]` | never wait on your own pane |
`DP-E9` | **no-record when the ledger cannot express a violation** | OC-L1 ∧ OC-L2 admitted only a false attestation, a false refusal, a false observation, or no record. I chose no record | `gaq0`: admit `claimed_first:false` + `violation_reason` |
`DP-E10` | **agreement drift** | 88 wave-1 rows across 12 boxes, **0** reviewer rejections | clause (b): a wave of all-accepted rows is a failed wave |
`DP-E11` | **denied probe read as a negative** | a `dcg`-denied `env` grep became "the CLI carries no token"; both tokens were set | a refusal is UNKNOWN, never absence |
`DP-E12` | **borrowed claim** | a scout's `bv meta` and ntm `degraded[]` went into my packet unverified; both **refuted at runtime** | restating a claim makes it yours |

**The pattern.** Every escape route is *cheaper than the discipline* and *shaped like compliance*. `DP-E1` produces a passing validator. `DP-E3` produces a green box. `DP-E4` produces a confident number. None looks like cheating from the inside, which is why the detector has to be mechanical and has to run on the artifact rather than on the intent.

**`DP-E13` — the reserved row.** The next escape route is not on this list. When one is found, it is added with its instance and its detector; a taxonomy that stops growing has stopped being measured.

## Contract Artifacts

- **Invariant suite (TARGET):** `crates/dispatch-preflight/tests/preflight.rs` with the five named legs — `fires_on_known_bad`, `passes_known_good`, `mutation_goes_red`, `empty_scan_is_error`, `claim_header`. **DOES NOT EXIST**; frozen.
- **Enforced today, by hand:** `DP-T2`, `DP-T4`, `DP-T5`, `DP-T6`, `DP-T7`, `DP-T9`.
- **Enforced by a gate today:** `DP-T2` (OC-L2), `DP-T10` (OC-L1..L6, in the negative — it refuses the honest row).

## Validation

```
cd /Users/josh/Developer/omp-orchestrator && \
printf 'DP stable_ids=%s escape_routes=%s untracked_wave_evidence=%s kernel_bypass_crates=%s\n' \
  "$(grep -c '^`DP-' docs/contracts/dispatch_preflight.md)" \
  "$(grep -c '^`DP-E' docs/contracts/dispatch_preflight.md)" \
  "$(git status --porcelain docs/plan/flow/waves/ | wc -l | tr -d ' ')" \
  "$(git grep --no-index -l -E 'Command::new\("(tmux|ntm|br|bv|am)"\)' -- 'crates/*/src/*' | wc -l | tr -d ' ')"
```
Measured 2026-09-03T18:4xZ: `stable_ids=40 escape_routes=12 untracked_wave_evidence=4 kernel_bypass_crates=23`. **Re-measured ~15 minutes later: `untracked_wave_evidence=1`** — three of the four untracked ledgers were committed by the panes that owned them once `DP-E5` was named and dispatched to them. That movement is the only evidence in this document that a detector changes behaviour rather than describing it; the remaining 1 is still live. `kernel_bypass_crates=23` is the comment-INCLUSIVE grep and over-counts: the comment-stripped figure is **19 crates / 46 sites**, and the gap between 23 and 19 is four files whose only match is a comment — which is `DP-E1`'s cousin and the reason AGENTS.md requires blanking comments before matching.

## Cross-References

- `docs/plan/flow/CONTRACT.md:82-113` — the adversarial convergence bar; `DP-T9`, `DP-E10`.
- `docs/plan/flow/boxes/S1.toml` — `[[box.observability]]`, six layers; the event rows `DP-1..DP-10` must carry.
- `docs/contracts/journey_query.md` — the read side; `DP-A1`.
- `AGENTS.md` — the four numbered rules, the KERNEL-ONLY section (`DP-E6`), and the denied-probe rule (`DP-E11`).
- `crates/crate-atom-gate/src/lib.rs:51-70,115-121` — nine parts, five named legs.

## What slots into S2

S1 hands S2 exactly one thing, and it is currently **absent on disk**: the `Inception` envelope at `.omp-orchestrator/inception.json` (`SCHEMAS.toml:156-161`) carrying `schema_version, project_id, repo_identity, control_files, host_capabilities, required_tools, trust_status` — plus an appended `FOUNDATION.jsonl` `stage=S1` row and a `jq` readback proving both. Nothing else crosses the boundary. S2 may not read a pane, a tick, or a bead to learn what S1 established; if it needs a fact, that fact is a field in the envelope or it does not exist.

## NO-CLAIM

This document is a **preflight and a taxonomy, not a gate**. Six of the ten truth-conditions are enforced by an orchestrator remembering to check them, which is precisely the fragility `DP-A0` describes. The escape-route list is measured from **one session on one machine**; it is a floor on what agents invent, never a ceiling. And naming a detector is not building one: of the twelve rows, `DP-E6` has a real gate that currently cannot go green, `DP-E9`'s gate refuses the honest record, and the remaining ten are checked by hand or not at all.
