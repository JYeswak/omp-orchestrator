# 03 — Every crate: contract, schema, types, dependencies

*Serves **R4** — "every crate - every schema input / output, every typed interface, how everything
is interacting". Answerable to `00-brief.md`; measured facts are taken from its §3 unless this
section explicitly disagrees, and every disagreement is named in "Where this section disagrees with
the brief" below.*

## How to read this section

The workspace ships a built census, `omp-inventory-map`, that emits 183 rows in a versioned
envelope. Every row carries the four mandatory fields — `inputs`, `outputs`, `must_be_true`,
`negative_evidence` — with zero missing. That census is **not** the source for this section, and
the reason is the sharpest measured finding of the session.

**MEASURED.** Across all 183 rows there is exactly **one distinct** `must_be_true` value and
exactly **one distinct** `negative_evidence` value:

```
python3 -c "…collections.Counter(json.dumps(r.get('must_be_true')) …)"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

The universal `must_be_true` is `["The source probe is non-empty before a known verdict is
emitted.","A versioned inventory envelope carries the probe state."]`. That sentence is true of the
*scanner*, for every row, regardless of what the row describes. The four-field discipline is
satisfied **syntactically and vacuously**: the fields are present, the schema validates, and the
content distinguishes nothing.

The same defect runs through the per-crate rows specifically. For a crate row, `inputs` names
`cargo metadata --format-version 1 --no-deps` and the crate's own `Cargo.toml`; `outputs` names the
crate's build targets. Those describe **the scanner's provenance**, not the crate's contract.
`what_it_provides` is boilerplate — "Workspace crate X from cargo metadata" — 26 distinct strings
only because the name varies.

So: **every contract below is derived from the crate's own source**, with the derivation command
inline. Where the source does not state a contract, the row says `UNDECLARED`. We do not fill a
cell by inference. An empty cell that admits it is empty is worth more to an investor than a
plausible cell that was invented, and the census is the standing proof of what the second kind
costs.

**NO-CLAIM:** deriving contracts from public signatures does not prove those contracts are
*honoured* at runtime. A `pub fn assess(&AckStageInput) -> AckStageResult` states a shape, not a
semantics. Behavioural conformance is the gate section's problem, not this one's.

---

## The crate table

**MEASURED**, derived by three commands over `crates/*`:

```
grep -E '^(name|description)' crates/*/Cargo.toml            # role
grep -hoE '^pub (enum|struct|trait|fn|type) [A-Za-z_0-9]+' crates/*/src/*.rs   # key types
sed -n '/^\[dependencies\]/,/^\[[a-z]/p' crates/*/Cargo.toml # path-deps
grep -c 'unsafe_code = "forbid"' crates/*/Cargo.toml         # manifest lint
grep -rlF 'forbid(unsafe_code)' --include=lib.rs --include=main.rs crates/  # inner attr
```

Twenty-six crates. `no unsafe` is **yes** for all 26, but by two different mechanisms — see the
note below the table.

| crate | role | inputs (real) | outputs (real) | key public types | path-deps | no unsafe |
|---|---|---|---|---|---|---|
| `ack-spine` | separate transport, delivery and ack authorities | `bead_id: &str`, `marker: &str`, capture text | `AckVerdict`, `AckSummary`, `StepLedger` rows | `AckVerdict`, `TransportAuthority`, `DeliveryAuthority`, `AckAuthority`, `AckEvidence`, `FollowUpVerdict`, `StepKind`, `StepRecord`, `StepLedger`, `HeartbeatRow` | `finding` | yes |
| `ack-stage` | typed follow-up actions for receiver receipts | `&AckStageInput` (transport receipt + ack readback) | `AckStageResult`, `AckAction` | `TransportKind`, `TransportReceipt`, `AckReadback`, `AckStageInput`, `AckStageResult`, `AckAction` | `receiver-receipt`, `tick-monitor` | yes |
| `commit-build-fence` | fail-closed pre-commit fence for in-flight builds | `RegistrationStore` loaded from `&Path`; `BuildRegistration` / `ReleaseEvent` | `FenceVerdict`; atomically saved store | `BuildRegistration`, `ReleaseEvent`, `RegistrationStore`, `FenceVerdict`, `StoreError` | — | yes |
| `composer-typed` | does a pane composer hold real typed text | `data: &str`, `&Rules` | `bool` | `Rule`, `Rules` | — | yes |
| `dispatch-claim-fence` | fail-closed claim authorization for dispatch packets | `br show --json` text → `BeadSnapshot`; `DispatchIntent` | `DispatchPermit` | `BeadStatus`, `BeadSnapshot`, `DispatchIntent`, `DispatchPermit`, `ClaimFenceError`, `SnapshotParseError` | — | yes |
| `dispatch-silence-watch` | typed verdict after every dispatch, read from the tracker | bead comment text, assignee text | `SilenceVerdict` | `SilenceVerdict` | — | yes |
| `finding` | a named gap is an obligation, filed or waived, never dropped | spool dir `&Path`; a constructed `Finding` | `SpooledFinding`, `Filed`, `Waived`, `Vec<PathBuf>` of pending | `Finding`, `SpooledFinding`, `Filed`, `Waived`, `Publisher` (trait), `FindingError` | `subprocess-contract` | yes |
| `finding-dispatch` | turn recurring supervisor decisions into named findings | `&SupervisorDecision`, `recurrence_count: u32` | `Option<Finding>` | — (single free fn `finding_for`) | `finding`, `omp-orchestrator` | yes |
| `fleet-composite` | geometric fleet health composite | `&BTreeMap<String, f64>` of raw factors | `CompositeReport`, `SelftestReport`, JSON | `FactorSpec`, `InputError`, `CompositeReport`, `SelftestCheck`, `SelftestReport` | — | yes |
| `installer` | one-touch build, four-way identity verify, install | repo `&Path`, binary name | `RepoOwnership`, `IdentityCheck`, installed binary | `InstallError`, `InstallTarget`, `RepoOwnership`, `IdentityCheck` | — | yes |
| `kernel-bypass-gate` | detect raw invocations duplicating an existing kernel | file path + source text; workspace root | `Vec<Bypass>`, `GateReport` | `Bypass`, `GateReport` | — | yes |
| `kernel-only-operator-hook` | fail-closed PreToolUse hook blocking kernel bypasses | hook JSON `&[u8]` → `HookInput` | `Decision` (rendered to stdout) | `HookInput`, `Permission`, `Decision`, `ParseError` | `subprocess-contract` | yes |
| `loop-queue-filter` | fail-closed port of the control-plane queue selector | queue JSON `&str`, argv `&[String]`, `&Runtime` | `RunOutput` | `Runtime`, `RunOutput` | — | yes |
| `no-shell-gate` | refuse tracked `.sh`/`.py`; exemption list empty by design | `git ls-files` output; repo root `&Path` | `Verdict`, `Vec<Violation>`, `WorkspaceLoad` | `Violation`, `Verdict`, `GateError`, `WorkspaceLoad` | `path-literal-guard`, `state-wildcard-lint`, `pre-delete-citation-check`, `undrained-pipe-lint` | yes |
| `omp-inventory-map` | the OMP v18 surface + workspace crate census | `cargo metadata`, OMP CLI probes, source scrape | `InventoryMap` envelope (184 nodes, 207 edges, 183 rows) | `ProbeState`, `SurfaceMap`, `SurfaceMapAudit`, `InventoryRow`, `InventoryNode`, `InventoryEdge`, `InventoryMap`, `ProbeEvidence`, `InventoryCounts` | — | yes |
| `omp-orchestrator` | resident supervisor: observe → dispatch → receipt → escalate | `&Observation`, `&IdleAuthorization`, repo `&Path` | `SupervisorDecision`, `GateCensus`, `Duty`/`Discharged` | `Observation`, `PaneObservation`, `QueueState`, `SupervisorDecision`, `IdleAuthorization`, `GateCensus`, `GateReachability`, `Duty`, `Discharged`, `Census` | `ack-stage`, `dispatch-claim-fence`, `omp-rpc-session`, `receiver-receipt`, `subprocess-contract` | yes |
| `omp-rpc-session` | typed bounded single-session adapter for `omp --mode=rpc` | `OmpCommand` + `RpcSessionConfig`; raw stdout frames | `RpcFrame` (`Ready`/`Response`/`Unknown`/`Malformed`), `RpcSessionReport` | `Deadlines`, `TimeoutPhase`, `OmpCommand`, `RpcSessionConfig`, `RequestId`, `RpcRequest`, `RpcFrame`, `MalformedReason`, `Lifecycle`, `ProtocolError`, `RpcError` | — | yes |
| `omp-types` | the canonical vocabulary, derived from asupersync, never authored | none (re-export only) | `pub use` of asupersync types | `Outcome`, `OutcomeError`, `PanicPayload`, `Severity`, `join_outcomes`, `Budget`, `CapabilityBudget*`, `RemainingBudget`, `ObligationId`, `RegionId`, `TaskId`, `Time` | — | yes |
| `pane-dispatch-fence` | cross-process per-pane admission fence | `UNDECLARED` — binary only, no `src/lib.rs`, zero `pub` items | `UNDECLARED` — exit status only | none exported | `subprocess-contract` | yes |
| `path-literal-guard` | zero hardcoded home-path literals across `crates/*/src` | repo root `&Path` | `ScanReport` (`Vec<Hit>`) | `Hit`, `ScanReport` | — | yes |
| `pre-delete-citation-check` | refuse deleting a file a closed bead cites as evidence | staged-deletion git output, `br` closed-bead JSON | `Vec<CitationConflict>` | `CitationConflict`, `ClosedBead` | — | yes |
| `receiver-receipt` | receiver-side dispatch receipt classifier | `pane_id`, pane capture `&str`, timestamp | `Observation` → `ReceiptVerdict` | `PanePresence`, `PostSendObservation`, `ReceiptReason`, `ReceiptVerdict` | `tick-monitor` | yes |
| `state-wildcard-lint` | reject wildcard arms on locally resolvable state enums | source text; workspace root | `LintReport` (`Vec<Finding>`) | `FindingKind`, `Finding`, `LintReport` | — | yes |
| `subprocess-contract` | the cancel-correct process-group and dual-pipe boundary | `&Cx`, `asupersync::process::Command` | `Output` / `ExitStatus`, or `RunError` | `RunError`; `run_output`, `run_status` (both `async`, `&Cx` first) | — | yes |
| `tick-monitor` | three monitors behind one loop-enforcement choke point | pane capture, git state, `Tick` | `Outcome`, `PaneState`, `Liveness`, `CapacityEscalationReceipt` | `Outcome`, `PaneState`, `Liveness`, `Observation`, `Reject`, `Tick`, `State`, `CapacityAlarm`, `CapacityAlarmEvent`, `CapacityEscalationReceipt`, `RepoError` | — | yes |
| `undrained-pipe-lint` | fail on both-pipes-piped + `try_wait` poll + no concurrent drain | source text; workspace root | `LintReport` (`Vec<Violation>`) | `Violation`, `LintReport` | — | yes |

**The unsafe-forbid split, MEASURED.** Twenty of 26 crates forbid unsafe in the manifest
(`grep -c 'unsafe_code = "forbid"' crates/*/Cargo.toml` → 20). Twenty-five of 26 forbid it with an
inner `#![forbid(unsafe_code)]` attribute (`grep -rlF 'forbid(unsafe_code)' --include=lib.rs
--include=main.rs crates/ | cut -d/ -f2 | sort -u | wc -l` → 25). Nineteen do both. The union is
26 — every crate is covered — but by **two mechanisms with no single enforcement point**:
`tick-monitor` is manifest-only; `composer-typed`, `dispatch-silence-watch`, `loop-queue-filter`,
`no-shell-gate`, `pane-dispatch-fence` and `subprocess-contract` are attribute-only. An investor
should read that as: the property holds today by coincidence of two habits, and nothing fails the
build if crate 27 adopts neither.

**PROJECTED.** A one-file lint asserting *manifest-and-attribute for every workspace member* turns
that coincidence into an invariant. It is the cheapest gate in the plan and it is not written yet.

---

## The dependency graph in prose

**MEASURED** — 18 `path-depends-on` edges, complete, from the census:

```
ack-spine -> finding                     finding -> subprocess-contract
ack-stage -> receiver-receipt            ack-stage -> tick-monitor
receiver-receipt -> tick-monitor         finding-dispatch -> finding
finding-dispatch -> omp-orchestrator     omp-orchestrator -> ack-stage
omp-orchestrator -> dispatch-claim-fence omp-orchestrator -> omp-rpc-session
omp-orchestrator -> receiver-receipt     omp-orchestrator -> subprocess-contract
kernel-only-operator-hook -> subprocess-contract
no-shell-gate -> path-literal-guard      no-shell-gate -> state-wildcard-lint
no-shell-gate -> pre-delete-citation-check
no-shell-gate -> undrained-pipe-lint     pane-dispatch-fence -> subprocess-contract
```

**The leaves.** Nine crates emit an outgoing `path-depends-on` edge (`ack-spine`, `ack-stage`,
`finding`, `finding-dispatch`, `kernel-only-operator-hook`, `no-shell-gate`, `omp-orchestrator`,
`pane-dispatch-fence`, `receiver-receipt`), so **17 of 26 are leaves**: `commit-build-fence`,
`composer-typed`, `dispatch-claim-fence`, `dispatch-silence-watch`, `fleet-composite`, `installer`,
`kernel-bypass-gate`, `loop-queue-filter`, `omp-inventory-map`, `omp-rpc-session`, `omp-types`,
`path-literal-guard`, `pre-delete-citation-check`, `state-wildcard-lint`, `subprocess-contract`,
`tick-monitor`, `undrained-pipe-lint`. Two of those leaves are the interesting ones:
`omp-rpc-session` and `omp-inventory-map` both spawn processes and neither routes through
`subprocess-contract`. A leaf that spawns is a leaf that reimplemented the boundary.

**The deepest chain** is five nodes:
`finding-dispatch → omp-orchestrator → ack-stage → receiver-receipt → tick-monitor`.
Everything the supervisor decides is ultimately grounded in a `tick-monitor` pane observation, and
`finding-dispatch` converts a repeated decision at the top of that chain into a `Finding`
obligation. That is a coherent spine.

**The two hubs.** `omp-orchestrator` is the *fan-out* hub — five outgoing edges, the only crate
that composes ack, claim-fence, rpc-session, receipt and subprocess into one supervisor.
`no-shell-gate` is the *aggregation* hub — four outgoing edges, all to sibling gates, making it the
single build-time entry point for the repo's lint family.

**`subprocess-contract` is the most-depended-on crate: four dependents** — `finding`,
`omp-orchestrator`, `kernel-only-operator-hook`, `pane-dispatch-fence`. That is the right shape,
and the reason is structural rather than stylistic. Its entire public surface is two async
functions and one error enum:

```
pub async fn run_output(cx: &Cx, mut command: Command) -> Result<Output, RunError>
pub async fn run_status(cx: &Cx, command: Command) -> Result<ExitStatus, RunError>
pub enum RunError { … }
```

`&Cx` comes first, as the asupersync contract requires. The doc comment states the three properties
that make it load-bearing: a fresh process group per child with group-targeted cancellation, both
pipes drained concurrently by `output_async`, and — the one that matters most — *"a timeout or
deadline cancellation is surfaced as `RunError::Timeout`, never as a child failure or an invented
output verdict."* That is the repo's hardest-won rule, **a timeout is not a verdict**, encoded once
as a type instead of remembered separately in every call site. Anything that spawns should route
through it precisely so that rule cannot be re-litigated per crate.

**MEASURED: 22 of 26 crates do not route through it.** Four depend on it. Independently measured
across 29 raw spawn sites, 4 crates use `subprocess-contract` and 12 of 14 async fns take `cx`
first. The gap is not an abstraction preference — it means at least 25 spawn sites can still leak a
process group, deadlock on an undrained pipe, or convert a timeout into a fabricated verdict, and
the only thing standing between us and that is `undrained-pipe-lint`, which catches one of the
three failure modes by pattern-matching source text.

**What Jeffrey would do.** Searched the mirror at `/Volumes/ZestData/dicklesworthstone-mirror` for
this exact shape; `asupersync` itself is the prior art and it is already our dependency —
`process::Command` with `ProcessGroupMode`, `ProcessSignalTarget` and `output_async` exists
upstream precisely so a caller never hand-rolls the group/drain pair. The correct move is not to
invent a wrapper policy but to make the existing wrapper the only reachable door.

**PROJECTED.** A gate that fails any crate constructing `std::process::Command` or
`asupersync::process::Command` outside `subprocess-contract` raises the floor: the 22 crates that
currently *may* hand-roll a spawn would have to be edited deliberately, in the open, to do so. It
does not guarantee correct spawning — a crate could route through `subprocess-contract` and still
mishandle the result — it removes accidental divergence, which is the failure mode we measured.
That gate does not exist today.

---

## The typed-interface problem, measured

**MEASURED**, by direct grep over `crates/`:

```
grep -rhoE '^pub enum [A-Za-z_0-9]+'   --include=*.rs crates/ | wc -l   -> 59
grep -rhoE '^pub struct [A-Za-z_0-9]+' --include=*.rs crates/ | wc -l   -> 91
```

An earlier scan scoped to library surfaces reported **51 public enums (excluding test+bin sources; 59 including them — publish the pair) and 79 public structs across
22 of 24 crates**. Both numbers are real; they differ because the grep above includes test modules
and binary sources. We publish both rather than pick the flattering one — the delta *is* the
measurement's error bar.

**Four colliding type names**, exact:

```
grep -rhoE '^pub (enum|struct) [A-Za-z_0-9]+' --include=*.rs crates/ \
  | awk '{print $3}' | sort | uniq -d
  -> Finding  LintReport  Observation  Violation
```

One of those four is structural rather than cosmetic, and it is the direct cause of the single
worst row in the brief's five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (§4). `tick-monitor` produces the `Observation`
that `omp-orchestrator` consumes, and **each declares its own incompatible struct**. That is the
seam where `free_capacity` was derived from the same `is_dispatchable` filter that requires a
*Confirmed* Idle pane, so a pane at `t=0` fell out of the `idle_panes` list **and** out of the
capacity count — the `actionable: BROKEN` row. Had one type crossed the boundary rather than being
re-declared on each side, the mismatch would have been a compile error instead of a silent
arithmetic error observed only after 162 refused ticks.

**Six Verdict-shaped types with no shared trait**, exact:

```
grep -rhoE '^pub enum [A-Za-z_0-9]*Verdict' --include=*.rs crates/ | sort -u
  -> AckVerdict  FenceVerdict  FollowUpVerdict  ReceiptVerdict  SilenceVerdict  Verdict
```

Six independent answers to "what happened". No trait, no `From`, no common discriminant. The
consequence is precise and it is not aesthetic: **you cannot compose them and you cannot count
them.** There is no function `fn grade(&[impl Verdictlike]) -> Grade`, because there is no
`Verdictlike`. A supervisor holding an `AckVerdict`, a `ReceiptVerdict` and a `FenceVerdict` cannot
reduce them to one number without hand-writing a match per pair. That is *why grading is prose* —
not because nobody wrote the grader, but because there is no type a grade could be.

**Seventeen ack/receipt types in three incompatible dialects.** `ack-spine` speaks *Authority*
(`TransportAuthority`, `DeliveryAuthority`, `AckAuthority`). `ack-stage` speaks *Receipt*
(`TransportReceipt`, `AckReadback`, `AckStageResult`). `receiver-receipt` speaks a third
(`PanePresence`, `PostSendObservation`, `ReceiptReason`, `ReceiptVerdict`). All three are groping
toward the same distinction — *did the transport accept it, did it arrive, did the receiver
acknowledge it* — and none can be passed to another without translation.

**`omp-types` exists and has ZERO dependents.** No crate lists it as a path-dep; it appears in none
of the 18 edges. Its own doc comment states the design rule verbatim: *"No crate invents a type
that already exists here… its contents are derived — re-exported from `asupersync` at the exact rev
we pin (`fa3c01aec`, v0.4.9) — never authored here."* The vocabulary is shipped and unadopted.

**One honest correction to the brief.** The brief lists `omp-types` as re-exporting `AckKind`,
`DeliveryClass`, `ObligationLedger`, `Budget` and `Outcome`. **MEASURED** by reading
`crates/omp-types/src/lib.rs` (129 lines), the actual `pub use` set is three lines:

```
pub use asupersync::types::{Outcome, OutcomeError, PanicPayload, Severity, join_outcomes};
pub use asupersync::types::{Budget, CapabilityBudget, CapabilityBudgetDimension,
    CapabilityBudgetRefusal, CapabilityBudgetRequirements, RemainingBudget};
pub use asupersync::types::{ObligationId, RegionId, TaskId, Time};
```

`AckKind`, `DeliveryClass` and `ObligationLedger` are **not** re-exported. The crate documents why:
`messaging-fabric` requires the `test-internals` feature at rev `fa3c01aec`
(`consumer.rs:1299` default impl), and that feature was correctly removed from upstream defaults —
so the ack vocabulary is *unreachable at our pinned rev*. The crate names that absence in a test
(`ack_vocabulary_is_documented_as_unreachable`) rather than silently omitting it. The crate also
records a second trap worth quoting to anyone who thinks re-export is mechanical: asupersync
declares **two** `AckKind`s, and `obligation/graded.rs:790` is an **uninhabited marker** — matching
the name yields a type with no values.

This matters to the plan because it means the single most valuable half of the vocabulary — the
half that would collapse the three ack dialects — is **blocked on an upstream feature boundary**,
not merely unadopted. Any migration schedule that assumes `AckKind` is available today is wrong.

---

## Design spec: the adoption path

**PROJECTED** throughout this subsection. Nothing below is measured; it is what we will build.

**Collapsing the six Verdicts.** We will define one trait in `omp-types`, over the asupersync
`Outcome` shape rather than a new enum:

```
pub trait Verdictlike {
    fn outcome(&self) -> Outcome<(), VerdictError>;
    fn subject(&self) -> &str;
}
```

`Outcome` is chosen over `Result` for one reason and the reason is the repo's own scar tissue: its
variants are `Ok` / `Err` / `Cancelled(CancelReason)` / `Panicked(PanicPayload)`. **Cancellation is
a first-class outcome, not an error.** That is the type-level form of *a timeout is not a verdict*,
which the repo has already violated once by parsing an empty buffer, defaulting the verdict field
to FAIL, and manufacturing a fleet-wide claim out of nothing. With `Outcome`, a killed child maps
to `Cancelled` and **cannot** be confused with `Err`. Each of the six existing enums keeps its own
variants and gains one impl. Once all six implement it, `fn grade(&[&dyn Verdictlike]) -> Grade`
becomes writable, and grading stops being prose.

**Collapsing the three ack dialects.** The target is `AckKind` — `Accepted` (packet plane accepted
custody), `Committed` (authority plane committed), `Recoverable` (durability class met), `Served`
(service obligation completed by callee). Those four map cleanly onto what our three dialects were
approximating: `Accepted` is transport success from `ntm` JSON, `Served` is the bead-comment ack
read back, and `Committed`/`Recoverable` are distinctions we currently cannot express at all. The
migration is gated on the upstream feature boundary described above; the sequenced form is
(1) advance the pin past the `messaging-fabric`/`test-internals` constraint, (2) re-export
`AckKind` and `DeliveryClass`, (3) convert `ack-spine`'s Authority triple, then `ack-stage`'s
Receipt set, then `receiver-receipt`'s — one crate per change so a bad conversion is bisectable.
Until step (1) lands, the ack collapse is **blocked, and named as blocked**, not scheduled.

**The gate that enforces it.** A crate declaring a public type whose name duplicates one exported
by `omp-types` will fail the build. This is not a new mechanism — `omp-inventory-map` already
implements the pattern, and its `types_inventory.rs:176-178` deliberately excludes `Observation`
from the allowance list *so that the collision demands convergence rather than being tolerated*.
Extending that from one allowance list to the full `omp-types` export set is an incremental change
to a gate that already exists and already passes 13 tests. The same gate acquires the
`subprocess-contract` rule described earlier: constructing a `Command` outside that crate is a
duplicate-of-canonical violation of exactly the same kind. **This raises a floor, it does not
guarantee coherence** — the gate matches names, and two crates can still hold semantically
divergent types under distinct names. It removes the collision class we measured; it does not
remove divergence.

**Why this framework and not an invented one.** Three reasons, in order of force. First,
asupersync **already compiles in our tree** at a pinned rev — it is a dependency of seven crates
today, not a proposal. Second, it **already models the three things we keep re-deriving badly**:
obligation (reserve → commit-or-abort, with `ObligationId`), outcome (including cancellation as a
peer of success), and capability narrowing (`Budget`). Our `Duty` type carries `#[must_use]` and
**no ledger**, which is why a dropped `Duty` leaked an obligation that survived 162 refused ticks
before a human noticed; `ObligationLedger` is the upstream answer to precisely that. Third, and
decisively: **inventing a parallel vocabulary is the exact error the repo's rules forbid.** The
census we are correcting in this section is what happens when a shape is satisfied without being
derived. Authoring a seventh Verdict enum to unify six would be the same mistake with better
intentions.

---

## Where this section disagrees with the brief

Per the parent instruction, a disagreement between a section's own measurement and `00-brief.md`
§3 is reported rather than reconciled silently. Three, in descending order of consequence.

**1. `omp-types` does not re-export the ack vocabulary.** The brief §3.7 states it re-exports
`AckKind`, `DeliveryClass`, `ObligationLedger`, `Budget`, `Outcome`. Command:
`grep -nE '^pub use' crates/omp-types/src/lib.rs`. Result: three lines, exporting
`Outcome`/`OutcomeError`/`PanicPayload`/`Severity`/`join_outcomes`, the `Budget` family, and
`ObligationId`/`RegionId`/`TaskId`/`Time`. `AckKind`, `DeliveryClass` and `ObligationLedger` are
**absent**, and the crate documents why — `messaging-fabric` requires the `test-internals` feature
at rev `fa3c01aec` (`consumer.rs:1299`), which upstream correctly removed from defaults. This is
the consequential disagreement: the half of the vocabulary that would collapse the three ack
dialects is **blocked on an upstream feature boundary**, not merely unadopted, so any schedule that
assumes it is available today is wrong.

**2. The unsafe-forbid denominator.** The brief §3.7 states "16 of 22 forbid unsafe". Commands:
`grep -c 'unsafe_code = "forbid"' crates/*/Cargo.toml` → **20 of 26**;
`grep -rlF 'forbid(unsafe_code)' --include=lib.rs --include=main.rs crates/ | cut -d/ -f2 | sort -u | wc -l`
→ **25 of 26**; union → **26 of 26**. The brief's figure appears to come from the 22-crate spawn-site
scan and counts one mechanism only. Both denominators (22 vs 26) and both mechanisms need stating —
this is exactly writing-contract rule 4. The *substantive* finding survives either way and is
arguably worse than the brief's: coverage is complete today, but by two habits with no single
enforcement point.

**3. The type-inventory scope.** The brief §3.7 gives 51 enums / 79 structs across 22 of 24 crates.
`grep -rhoE '^pub enum …' --include=*.rs crates/ | wc -l` → **59**, and the struct form → **91**.
Not a contradiction — a scope difference (the greps above include test modules and binary sources).
Published as an error bar rather than resolved to the flattering number. The collision count (4) and
the Verdict count (6) are **identical** under both scopes, which is the part that matters.

**NO-CLAIM:** these three are the disagreements this section found while deriving crate contracts
from source. They are not an audit of the brief. Facts in §3 that this section did not need — the
census row kinds, the classification split, the gate leg table — were used as given and are
unverified here.

---

## Constraints this section discovered, written down (R11)

R11 exists because a requirement living only in chat dies with the conversation. Three constraints
surfaced while deriving the table above that were not previously written anywhere:

1. **Unsafe-forbid must be single-mechanism.** Today the property holds across all 26 crates by the
   union of a manifest lint and an inner attribute, with 19 crates carrying both, 1 manifest-only
   and 6 attribute-only. Nothing fails if crate 27 adopts neither. A one-file lint asserting
   *manifest **and** attribute for every workspace member* is the cheapest gate in the plan.
2. **A crate that spawns must depend on `subprocess-contract`.** Two leaves — `omp-rpc-session` and
   `omp-inventory-map` — spawn processes today with no path-dep on the boundary crate. The rule is
   stated here so it is checkable, not remembered.
3. **`pane-dispatch-fence` has no library surface.** It is the only workspace member with no
   `src/lib.rs` and zero `pub` items (`ls crates/pane-dispatch-fence/src` → `main.rs`). Its contract
   is `UNDECLARED` and therefore untestable from outside the binary. Any crate whose behaviour other
   crates must rely on needs a library surface; that is a constraint, and it is now written down.

**NO-CLAIM.** This section states each crate's contract as its source declares it, and states the
adoption path we intend. It does **not** claim any of the following: that the derived contracts are
honoured at runtime; that the 22 crates outside `subprocess-contract` are actually leaking process
groups (only that nothing prevents them from doing so); that the `Verdictlike` trait above compiles
— it has not been written; that the ack migration is schedulable — it is blocked on an upstream
feature boundary we have not yet cleared; or that a tmux pane which has never heard of asupersync
can produce an `Accepted`. That last one is **UNMEASURED**, and because `--mode=rpc` is
single-session and cannot address a third-party pane, the receipt gap may survive the vocabulary
entirely.
