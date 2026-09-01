# 01 — The idea

*Serves R10 (idea → why → binaries → SOTA bar) and R3 (investor-attackable). Written to the writing
contract in `00-brief.md` §6. Numbers in this section are either (a) a dated, command-backed receipt,
(b) a source citation explicitly marked as reported, or (c) historical/non-authoritative. A corrected
value in `00-brief.md` or `NUMBERS.toml` wins; stale values may remain only when labelled as such.*

---

## 1.1 The one sentence

**Falsifiable thesis (HYPOTHESIS):** a technical lead or founder who runs a multi-agent coding fleet
(economic buyer and budget owner) will pay a proposed **$500/month** for a narrow supervisor that lets
the daily fleet operator turn one completed OMP session into a receipt-backed bead close. The trigger
is a worker ending while the board, pane, and receipt disagree; the current workaround is manually
watching panes, copying evidence into a board, and asking a human to retry dispatch. The local session
below demonstrates that failure mode, but not its prevalence or value outside this checkout. The first
outcome is deliberately low-trust: **one OMP completion → durable receipt → one typed close/refusal**,
with no autonomous dispatch and no claim of broad fleet accounting.

The load-bearing word is *provable*. “An accountant for the ones already running” is a hypothesis, not
a category fact. **FACT:** this checkout recorded a local reconciliation failure. **UNKNOWN:** whether
other operators pay to solve it, what their baseline loss is, and whether $500/month is acceptable.
The cheapest discovery test is five interviews with fleet owners followed by a concierge replay of the
one-session wedge; ask each owner to provide a redacted completion/board mismatch and to sign a
non-binding $500/month letter of intent only if the receipt removes a manual close. No paid pilot or
production commitment is requested before the gates below pass.

---

## 1.2 Why this exists

The problem hypothesis is not that an agent fleet fails to work. It is that a fleet can complete work
while its board and evidence cannot tell the operator what finished. The **economic buyer** is the
technical lead/founder accountable for throughput and auditability; the **daily operator** is the
person tending panes, dispatch, and bead state. The observable trigger is a terminal worker event or
stand-down followed by a board state that remains open/in-progress or a receipt that cannot be bound
to the worker. The current workflow is pane watching plus manual board updates; its cost, frequency,
and willingness-to-pay are **UNKNOWN** outside this repository.

That is not yet a market claim; it is a local reap. From one stand-down session on this repository
(FACT about this checkout only):

 - **Historical, non-authoritative recollection:** 6 beads landed awaiting grade and ZERO landed *and*
  closed on the day. Work reached the tree; the ledger never learned it had. This is not used as market
  evidence because the detailed reap artifact and derivation command were not retained here.
 - **Historical, non-authoritative recollection:** 7 conditions were live in the repository and belonged
  to no bead; two were pre-existing red tests in a suite expected to be red. The detailed reap artifact
  and derivation command were not retained here, so this is not a measured prevalence claim.
 - **UNVERIFIED REPORTED VALUE:** 162 refused dispatch ticks across a reported 4.2 hours, with a human as the only
   actuator (00-brief.md §3.7, line 546). The brief's command receipt is not preserved in this section;
   treat this as a cited report, not an independently reproducible measurement or market-frequency claim.
 - **Historical, non-authoritative recollection:** a 23-commit drift between the installed supervisor
  binary and HEAD. No immutable receipt or derivation command is retained here.
 - Board snapshot (**reported FACT, as of 2026-08-31; authoritative reference 00-brief.md §3.3,
  lines 498-502; command receipt not preserved here**): **28 closed, 25 in_progress, 19 open,
  2 blocked (74 total)**. The snapshot is not the live board.

### The mechanism behind the 162

The reported 162 is a local symptom with a named cause, not proof of market recurrence. The control loop has
**five stages and seven measured rows** (00-brief.md §4); the denominator is seven rows, and there
are **zero unqualified WORKS rows**:

| stage | row/mechanism | measured state |
|---|---|---|
| observe | tick-monitor | **WORKS, qualified** — the observed monitor path has an asymmetry defect |
| actionable | idle_panes | **BROKEN** — discards NewlyIdle; free_capacity uses the same is_dispatchable filter and excludes a pane at t=0 |
| consume | selection | **UNVERIFIED** — no durable receipt here proves the selected work was consumed |
| consume | transport | **UNVERIFIED** — no durable receipt here proves delivery to the intended worker |
| consume | admission (decide()) | **FENCED** — the cited report is DISPATCH_RETRY_BLOCKED |
| actuate | dispatch | **DOES NOT EXIST** — a human types into panes |
| complete | worker says done | **AVAILABLE, NOT WIRED** — OMP exposes AgentEndEvent.willContinue on RpcSessionEventFrame; the local loop does not consume it |

A single shared predicate, used to answer two different questions, produced a coherent but wrong local
world model. **INFERENCE (not yet proven):** typed separation may outperform logging-only
instrumentation; logging could faithfully record 162 refusals while leaving the overloaded question
unchanged. The falsification test compares both approaches on false completion/idle classification,
operator minutes, and adoption effort.

**INFERENCE:** four symptoms may share one shape: **no typed answer to a question the supervisor must
answer in one call**. This is an experiment hypothesis, not a conclusion. The completion signal exists
on the wire, but the supervisor does not attach to it; the other six gap types remain DECLARED ONLY.

**NO-CLAIM:** these observations come from a single session's reap on one machine and one checkout.
The 6/7/23 figures are historical recollections without retained command receipts and are not market
evidence; the 162 figure is reported by the brief but is not independently reproducible from this
section. They establish only that these failures occurred in that local report, not how often they
occur.

### 1.2.1 The seven gap claims — what the upstream types actually change

The upstream sweep changes the strength of the absence claims without pretending that a declared type is a consumed contract. One gap is WIRE-PROVEN; the other six are DECLARED ONLY.

| gap | upstream type and source | true strength | effect on the idea
|---|---|---|
| completion | AgentEndEvent.willContinue + SessionStopEvent (extensibility/shared-events.d.ts:83-93,154-162), on RpcSessionEventFrame (modes/rpc/rpc-types.d.ts:589) | **WIRE-PROVEN for one observed frame only** — exact raw receipt at /tmp/grade/agent-end-raw-frame.json; capture command /Users/josh/.local/bin/omp --mode=rpc --no-session --no-tools --no-lsp --max-time=30; artifact mtime/retrieval observed 2026-08-31T19:52:26-0600; SHA-256 d8bd80c6949b2ec48af1639b5b5e241bd90b4dce1e769483dd1690ed2be8f644 | the frame's session-specific isTerminal=true was observed; shared willContinue was absent; repeatability, semantic fit, and supervisor consumption remain UNKNOWN
| receipts | IrcDeliveryReceipt + AsyncJobDeliverySink (tools/hub/types.d.ts:8,84) | DECLARED ONLY — no wire path measured | the cp-z42vu transport/receipt gap remains; type existence does not replace receiver proof
| claims | Stage1Claim / GlobalClaim with ownershipToken + inputWatermark (memories/storage.d.ts:20-27) | DECLARED ONLY — no wire path measured | local claim/ownership gap remains until reachability and semantics are proven
| idle | GuestIdleReconcilerCtx (dist/types/collab/guest.d.ts:9-30) | DECLARED ONLY — no wire path measured | the local NewlyIdle/ConfirmedIdle defect remains; the upstream split is corroboration, not a fix
| roster | HubRosterCounts (dist/types/tools/hub/types.d.ts:33-90) | DECLARED ONLY — no wire path measured | hand-derived roster remains an unclosed observation gap
| cost | SearchUsage (dist/types/web/search/types.d.ts:232-254), PerplexityCost (:510-527), ContextUsage (dist/types/extensibility/extensions/types.d.ts:238-240) | DECLARED ONLY — no wire path measured | cost telemetry remains unmeasured
| compaction | SessionBeforeCompactEvent / SessionCompactEvent (dist/types/extensibility/shared-events.d.ts:54-75) | DECLARED ONLY — no wire path measured | context-loss recovery remains unproven; the type narrows adoption work but does not close it

**NO-CLAIM:** the durable receipt proves one raw wire frame, not supervisor consumption. In the declaration,
willContinue is the shared AgentEndEvent field, while isTerminal is supplied by the session-specific
frame shape. The receipt does not establish repeatability, semantic equivalence of isTerminal to
willContinue=false, non-terminal settle behavior, crashes, killed panes, rate limits, compaction,
semantic fit, or adoption cost. The other six entries refute “no upstream type exists” only.

---

## 1.2.2 Substitution map and residual wedge (HYPOTHESIS)

The job is “bind a worker completion to a durable, reviewable board outcome.” Current substitutes are
not yet evaluated; the table is a discovery inventory, not a claim that this product wins:

| substitute | job outcome | trust/evidence | price/switching cost | strongest contrary evidence |
|---|---|---|---|---|
| OMP/ntm/br/bv native workflow | runs agents, panes, triage, and bead state | partial; no single completion-to-close receipt | existing tools are sunk cost; switching cost low to try | may already be sufficient for disciplined operators |
| shell/Python scripts | bespoke polling, parsing, and board updates | varies; scripts can be local and inspectable | low cash cost, high maintenance cost | a script may solve this exact narrow path faster |
| human review | watches panes and reconciles board/evidence | high judgment, low repeatability | operator time is the cost | may be cheaper than buying software at low volume |
| managed service/consultant | outsourced fleet tending and reporting | depends on provider and handoff | recurring labor cost; high switching cost | service can absorb exceptions without new software |
| doing nothing | accepts stale/ambiguous state | no additional assurance | zero cash; hidden rework/opportunity cost UNKNOWN | pain may be tolerable and non-recurring |

**Residual wedge (HYPOTHESIS):** a typed, low-trust completion → receipt → close/refusal path may be
worth adopting only when it saves more operator time/rework than its price and switching cost. The
cheapest hostile test is to ask five current operators to replay one redacted mismatch using their
native workflow and a concierge receipt; record outcome, trust, minutes saved, willingness to pay, and
why they reject it. Until that test, differentiation, prevalence, and value are UNKNOWN.

## 1.2.3 Narrow first-value boundary

**IN SCOPE:** one OMP RPC terminal completion, one durable evidence receipt, and one typed bead
close/refusal that an operator can review. **OUT OF SCOPE:** autonomous dispatch, all seven gap
families, fleet-wide cost accounting, multi-tenant service, generalized integrations, and production
security/compliance claims. The nine binaries, scanner, census, and gates below are research or
internal enforcement instrumentation unless and until this wedge is wired end to end. A materially
narrower thesis needs a new viability packet; this section intentionally preserves the fresh grader's
no-dispatchable-stage scope.

## 1.3 The binaries we are wrapping

We are building an **enforcement-substrate prototype**, not claiming a customer-ready runtime. The
future product promise is a typed supervisor over binaries that already exist; the local implementation
currently lacks the OMP supervisory integration. The bet that accountability is the residual wedge is a
HYPOTHESIS, not a universal category claim.

Measured 2026-08-31 by `<bin> --version` (MEASURED; reproduced from `00-brief.md` §3.1, not
re-derived):

| binary | version | path |
|---|---|---|
| `omp` | `omp/18.0.11` | `/Users/josh/.local/bin/omp` |
| `ntm` | `ntm version v1.30.0-1-gda270719` | `/Users/josh/.local/bin/ntm` |
| `br` | `br 0.4.1` | `/Users/josh/.local/bin/br` |
| `bv` | `bv v0.20.0` | `/opt/homebrew/bin/bv` |
| `git` | `git version 2.50.1 (Apple Git-155)` | `/usr/bin/git` |
| `cargo` | `cargo 1.100.0-nightly (e8cb624d5 2026-08-2…)` | `/Users/josh/.rch/shims/cargo` (a shim; real cargo at `~/.cargo/bin/cargo`) |
| `fh` | `franken-harvest 0.1.0+tree.7b0fc50c3e5a29d…` | `/Users/josh/.local/bin/fh` |
| `jsm` | `jsm 0.1.4` | `/usr/local/bin/jsm` |
| `tmux` | rejects `--version` (`tmux: unknown option -- -`) | `/opt/homebrew/bin/tmux` |

**1.3.1 `omp` — the agent surface.** We call it for the surface itself: CLI subcommands, the
JSON-RPC transport reached through `--mode=rpc`, and the type roots defining what a session can be
asked to do. *Contract depended on:* the versioned envelope and the stability of the `--mode=rpc`
selector. *If it drifts:* every consumer re-derives a different graph from the same repository and
our stored inventory silently describes a surface that no longer exists — a stale map that still
renders. Deepest dependency; per §1.5, least exercised.

**1.3.2 `ntm` — pane truth.** We call it for which panes exist, what is in them, and which are live
agents rather than dead shells. *Contract depended on:* pane identity is stable across the window in
which we dispatch to a pane and then read back from it. *If it drifts:* a receipt binds to the wrong
actor and the ledger records a delivery that never happened — a **false positive on completion**,
precisely the class §1.2 exists to eliminate, arriving through the mechanism meant to eliminate it.

**1.3.3 `br` — the bead board.** We call it for the durable record of what work exists, who holds
it, and what state it is in. *Contract depended on:* `br`'s typed close-policy refusal — it refuses
to close a bead that has not satisfied policy, and refuses *with a type*. That is load-bearing: it
is the upstream mechanism making "landed awaiting grade" a representable state rather than untracked
limbo. *If it drifts* into a warning, the 6-landed-zero-closed measurement stops being *unresolved*
and becomes *invisible*, which is strictly worse.

**1.3.4 `bv` — triage.** We call it for ranking what to work next. *Contract depended on:*
`--robot-triage` returning multiple ranked slices in one call rather than N round-trips. That is not
ergonomics; it is what makes a supervisor tick cheap enough to run on a loop. *If it drifts* to
one-slice-per-call, tick cost multiplies by slice count and cadence collapses. Jeffrey's own agent
handbook states the same as a safety rule — `beads_rust/src/cli/commands/robot_docs.rs:59`: *"Avoid
bare `bv` in automated sessions; use `bv --robot-*` flags."* The robot surface is the contract; the
human surface is not.

**1.3.5 `git` — tree ground truth.** We call it for what is committed, what is dirty, and what
`git ls-files` says is tracked. *Contract depended on:* `git ls-files` is the authoritative
tracked-file set — our one hard rule (no `.sh`, no `.py`, exemption list empty) is enforced by a Rust
gate walking exactly that set. *If it drifts,* or if we ever substitute a filesystem walk, an
untracked scratch script passes and a hard rule silently becomes advisory.

**1.3.6 `cargo` — build and metadata, through a shim.** We call it for build and for
`cargo metadata --format-version 1 --no-deps`, the flagged versioned form our crate census reads.
Note the measured path: `cargo` resolves to `/Users/josh/.rch/shims/cargo`, **a shim**, with real
cargo at `~/.cargo/bin/cargo`. A shim in the dependency path is a supply-chain surface — it can
rewrite arguments, change the effective toolchain, or add latency — and the census attributes results
to "cargo" without qualification. **Every crate-metadata claim inherits whatever the shim does, and
we have not measured what it does.** Open item, recorded here rather than left in chat (R11).

**1.3.7 `fh` — evidence, failing closed.** We call it for retrieval against the dicklesworthstone
mirror — 210 git work-trees at `/Volumes/ZestData/dicklesworthstone-mirror` — which is how R7 gets
answered with a citation instead of a memory. *Contract depended on:* `fh` **fails closed with a
type**. As of this session its MCP surface returns `SERVE_INPUT_STALE` because the mirror HEAD moved
(`5dec4212…` → `ecdea397…`); direct grep still works, and this section's citations were taken that
way. **That failure is the good outcome** — it refused to serve a stale index rather than answering
confidently from data it knew was behind. *If it drifted* to best-effort, every prior-art citation in
this plan would become unfalsifiable, which is worse than having none, because unfalsifiable
citations survive review.

**1.3.8 `jsm` — skill resolution.** We call it for skill resolution and installation topology.
*Contract depended on:* the single-store-plus-symlinks invariant; the session-start gate reports
`PASS (one store, N symlinks)`. *If it drifts,* two divergent copies of a skill become simultaneously
resolvable and behaviour starts depending on resolution order rather than content — a nondeterminism
that reproduces only on the machine holding both.

### 1.3.9 tmux — historical correction to the brief

We call tmux as the substrate under ntm; pane truth is ultimately read through it. An earlier brief
draft incorrectly said that tmux had no machine-readable version handshake. The current brief is
correct: tmux has a machine-readable handshake on its short flag, not the GNU long flag.

 - **FACT, measured on this host:** command tmux -V; echo "exit=$?" returned tmux 3.6a and exit=0.
 - **FACT, measured on this host:** command tmux --version; echo "exit=$?" returned an unknown-option
   usage block and exit=1.
 - **Historical correction:** the old “no machine-readable handshake” interpretation and its old line
   citation are non-authoritative and must not be copied forward. The table's -V result is the current
   source of truth.

The real risk is narrower: a uniform probe loop written against --version can record tmux as absent
or broken rather than present at 3.6a. The dependency probe must use -V and document that asymmetry;
nonzero exit from the long flag is not evidence of absence.

**NO-CLAIM:** the table records what resolved on `PATH` at one moment on one host (Apple M3 Ultra,
darwin 25.5.0). It does not claim these versions are pinned, reproducible elsewhere, free of the
`cargo` shim's effects, or that the stated contract per binary exhaustively describes that tool's
interface. "If it drifts" clauses describe consequences we reason about; none has been observed and
none is a prediction of likelihood.

---

## 1.4 Dependency-contract/SLO floor (not product SOTA)

R10, verbatim: “every aspect of this needs to be on par or greater than SOTA - same as the binaries
we are wrapping.” The table below operationalizes a **technical dependency floor**: properties already
shipped by wrapped tools that our implementation must match. It is not evidence that the product wins
the customer job. Product-level SOTA remains OPEN until the substitution map is measured against
end-to-end latency, false completion/idle rates, operator-time reduction, reliability, adoption effort,
and cost. Any expected superiority is **PROJECTED**, not measured.
Product benchmark definition for later discovery: compare the wedge with each named substitute using
p95 completion-to-receipt latency, false completion and false-idle rates, operator minutes per close,
replay reliability, onboarding effort, and fully loaded monthly cost. Record each baseline and result
with the same cohort and a retained receipt; no product SOTA conclusion exists until that comparison
runs.
| wrapped binary | property it already ships | our obligation |
|---|---|---|
| `bv` | `--robot-triage` returns **multiple ranked slices in one call** | a tick answers multi-part questions in one invocation, never N |
| `br` | **typed close-policy refusal** — refuses to close, with a type | gates refuse with a typed reason, never a bare nonzero exit |
| `fh` | **fails closed** with typed `SERVE_INPUT_STALE` + remediation hint | every surface prefers typed refusal over a confident stale answer |
| `omp` | **versioned envelope** on output | every artifact carries `schema_version` |
| `git` | `ls-files` is an **authoritative machine-parseable set** | gates enumerate from an authority, never a filesystem walk |
| `jsm` | **single-store invariant, checked at session start** | installation topology is checked, not assumed |
| `tmux` | machine-readable version at `-V`, exit 0 | our binaries answer a version probe **on the conventional flag** — we do not ship tmux's asymmetry ourselves |

**Where the enforcement substrate meets the dependency floor (reported receipt, as of 2026-08-31).**
The built scanner at /Volumes/BuildShared/cargo-targets/debug/omp-inventory-map reportedly emits
schema_version omp-inventory-map/v1 with command doctor and status UNKNOWN, exits 2 on UNKNOWN, and
produces 544,697 bytes (00-brief.md §3.4; the exact command receipt is not retained in this section).
This demonstrates a versioned envelope and distinct uncertainty exit in the cited report; it does not
establish product superiority or customer value.

**Where the enforcement substrate is below the dependency floor — a named defect (reported receipt, as of 2026-08-31).** omp-inventory-map --help returns:

```json
{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR",
 "data":null,"error":"CONFIG_ERROR unknown argument --help"}
```

The gate is **built, correct, and undiscoverable.** **23 test functions are present in source** (13 in types_inventory.rs and 10 in tests/inventory.rs; source-count FACT as of 2026-08-31, not a test-pass claim).
`crates/omp-inventory-map/src/types_inventory.rs:176-178` deliberately excludes `Observation` from
the allowance list so a name collision *demands* convergence rather than being waved through — and
the running binary's 544 KB output contains **zero** occurrences of `Observation`, `CONVERGE`, or
`Verdict`. This is not built-versus-wired. It is **wired-but-unaddressable**, a worse diagnosis,
because it survives every check that asks "does the code exist and pass."

Note the shape, because it recurs in §1.5: `--help` did not return a generic error. It returned a
*typed* error (`CONFIG_ERROR`) with an accurate message. **The typing discipline was applied
perfectly, to the wrong outcome.** Rigor pointed at the wrong target is this codebase's
characteristic defect. Hence a sixth required gate property, now first-class across the plan:

> **ADDRESSABLE** — one documented command runs the gate, and `--help` names that command.

### What would Jeffrey do — prior art for ADDRESSABLE (R7)

Searched the mirror for self-describing CLI surfaces (`robot-docs|robot_docs` and
`CONFIG_ERROR|unknown argument` across `/Volumes/ZestData/dicklesworthstone-mirror/beads_rust` and
`.../ntm`). **Prior art found**, and it is stronger than the property we named:

1. **Discoverability is a shipped command, not a flag.** `br robot-docs guide` is a top-level
   subcommand whose entire job is machine-readable self-documentation, carrying its own contract
   version: `const CONTRACT_VERSION: &str = "br.robot_docs.v1";`
   (`beads_rust/src/cli/commands/robot_docs.rs:11`). The doctor subsystem ships its own,
   `br.doctor.robot_docs.v1` (`.../doctor_subsystems/surface.rs:108`). Discoverability is versioned
   like any other output.
2. **CI enforces that discoverability keeps working.** `beads_rust/.github/workflows/doctor.yml:88-93`
   runs `br doctor robot-docs --format json` and asserts both
   `jq -e '.schema_version == "br.doctor.robot_docs.v1"'` and `jq -e '.line_count > 20'` — the second
   an anti-vacuity check on the docs themselves: present *and non-trivial*.
3. **The help surface must not fail for the same reason the tool fails.**
   `beads_rust/src/main.rs:3744-3747` asserts `!needs_write_lock(&robot_docs)` with the message
   *"robot-docs is a pure help surface and must not depend on workspace lock health."* The deepest of
   the three, and we had not stated it: a discoverability surface that degrades when the workspace
   degrades is unavailable exactly when it is needed.
4. **Contract drift is a P0 bug filed against the docs, not the code.** Bead `beads_rust-mbpq`
   (`beads_rust/.beads/issues.jsonl:697`) was raised *because* `capabilities` and `robot-docs` claimed
   a concurrent `--repair` exits 5 on lock contention while the binary silently waited 30s and exited
   0 — filed *"P0 because it contradicts a documented contract."* The documented contract is normative
   over the implementation.
5. **Exit codes are a dictionary, not a convention.** Bead `beads_rust-rw4u`
   (`.../issues.jsonl:843`) specifies eleven variants: `0` healthy, `1` findings present, `2` fix
   partial, `3` fix failed and rolled back, `4` refused unsafe, `5` concurrency lost, `6` online
   required, `64` usage error, `66` no input, `73` cannot create output, `74` I/O error — and CI
   asserts `(.exit_codes | length) >= 11` (`doctor.yml:85`). Stated rationale: returning 0 with
   warn-level findings forces CI to parse JSON to gate.

**This raises our bar, and we adopt it (PROJECTED).** ADDRESSABLE as first stated — "`--help` names
the command" — is the weakest form. The mirror's form is a *versioned, CI-asserted, lock-independent*
self-documentation surface plus an exit-code dictionary rich enough that a caller branches without
parsing a payload. Our single distinction (`2` = UNKNOWN) is directionally right and
dictionary-poor. §06 owns the resulting spec; §10 owns the full prior-art pass.

**NO-CLAIM:** this names properties observed in these tools' documented interfaces and, where cited,
in mirror source at a specific file and line. It does not claim those properties exhaustively
describe those tools, that they hold across versions, that the mirror checkout matches upstream HEAD
(it does not — see `SERVE_INPUT_STALE`, §1.3.7), or that matching them is *sufficient* for quality.
Matching a bar is a floor-raise, never a guarantee.

---

## 1.5 The honest position

**What is genuinely built (reported snapshot, as of 2026-08-31 ~21:45).** The workspace contains 26
crates. The current registry value is **413 test functions across 31 integration test files**
(NUMBERS.toml keys test_functions/test_files); **406 and 407 are historical, non-authoritative values**
from earlier rounds and must not be copied forward. The exact registry derivation command is owned by the
number registry, not reproduced here. The same cited snapshot reports **184 nodes, 207 edges, 183 rows,
544,697 bytes, 18 dependency edges, and 4 dependents** for subprocess-contract; these are reported
inventory values, not product completion evidence, and the command receipt is not retained in this
section. The no-.sh/no-.py rule is enforced over the authoritative git ls-files set with an empty
exemption list.

The omp-types crate re-exports Outcome, OutcomeError, PanicPayload, Severity, and join_outcomes; Budget,
CapabilityBudget, CapabilityBudgetDimension, CapabilityBudgetRefusal, CapabilityBudgetRequirements, and
RemainingBudget; plus ObligationId, RegionId, TaskId, and Time from the pinned asupersync revision
fa3c01aec. AckKind and DeliveryClass are **present upstream but blocked at the pinned feature boundary**.
ObligationLedger is **absent everywhere** in the inspected upstream inventory; its semantics require a
decision. This is an **enforcement-substrate prototype**, not a customer-ready supervisor.

**The sentence that undercuts all of it (reported inventory snapshot, as of 2026-08-31).** Of the 26 workspace crates, **25 consume zero
OMP surface.** All 7 `consumes` edges in the entire 207-edge graph originate from one crate —
`omp-inventory-map`, the scanner — each carrying the evidence string *"direct process probe produced
this row."* The scanner consumes OMP because the scanner's job is to look at OMP. Nothing else does.

**The thing named `omp-orchestrator` does not yet speak to OMP.**

### The three objections an investor should raise

**Objection 1 — "You have built 26 crates of scaffolding around a hole. The one integration that justifies the name does not exist."** *Partly conceded, with a narrower truth.* The completion signal is now WIRE-PROVEN upstream, but the supervisory integration that would consume it still does not exist. The 25-of-26 measurement is ours, not a reviewer's. The partial answer is that the layer census shows `observe` WORKS and failure is
concentrated in actionable/consume/actuate — and the seven-row table above records **zero unqualified WORKS rows**. We will not use the qualified observe result as a rebuttal. What we do not concede is that the crates are therefore waste: the
gates operate on the repository and the process boundary, not on OMP, and they run today. The
accurate statement is that the *supervisory* path is unbuilt while the *enforcement* path is built
and running.

**Objection 2 — "Your own evidence discipline is theatre."** *Conceded outright, and we found it
ourselves.* All 183 census rows carry the four mandatory fields — `inputs`, `outputs`, `must_be_true`,
`negative_evidence` — with **zero missing**, and exactly **one distinct value** of `must_be_true` and
**one distinct** `negative_evidence` across the entire census (`00-brief.md` §3.3;
`python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"` → crate rows n=26,
distinct=1; non-crate rows n=157, distinct=1). For the 26 crate rows, `inputs`/`outputs` describe
*the scanner's own provenance* rather than the crate's contract, and `what_it_provides` is
boilerplate — "Workspace crate X from cargo metadata" — distinct 26 ways only because the name varies.
The four-field discipline this orchestrator demanded of every worker was satisfied **syntactically
and vacuously**, and the indictment lands on the conductor, not the workers. The answer is
structural: every gate must ship an **anti-vacuity** leg, and the success criterion is not "the
fields are present" but "**the field values discriminate**." A schema fully populated with one value
carries exactly zero bits.

**Objection 3 — “Your gates are unevenly built, so your floor is the weakest one.”** *Accepted as
stated.* The corrected typed classification is: **no gate currently mutates production source through
the real hook**. Of 8 gates, undrained-pipe-lint has the four named legs (1/1/1/3), but its mutation
leg is **FIXTURE**, not AUTOMATED (00-brief.md §3.5); no production-source mutation was tested. The
no-shell-gate mutation is an **AFFORDANCE**—a switch named to be flipped, nothing flips it. Four of 8
have no mutation leg, 2 of 8 have no known-bad leg, and path-literal-guard has no known-good leg.
Those counts are a reported, dated snapshot, not proof of semantic coverage. Earlier all-four and
mutation-AUTOMATED headlines are retracted and non-authoritative.

The highest-risk gate is path-literal-guard: **INFERENCE**, not FACT, says an attack-only suite could
ship an over-strict gate, routing could then bypass it, and the gate could still report green. The
falsification test is a known-good/known-bad fixture pair against the real hook, with a recorded result.

### The corollary connecting all three

omp-types — the crate holding the canonical vocabulary — has **zero dependents** in the reported
snapshot. The vocabulary is defined and unused. **INFERENCE:** this may contribute to the inventory's
fragmentation; “direct cause” is not established. The measured type snapshot reports **51 public enums
(excluding test+bin sources; 59 including them) and 79 structs** across library surfaces; including
test modules and binaries it reports 59/91 over 26 crates. It also reports **6 distinct Verdict-shaped
types sharing no trait, 17 ack/receipt types in 3 incompatible dialects, and 4 colliding type names**.
These are inventory observations, not proof that vocabulary adoption alone solves completion accounting.
The falsification test is the same typed-separation versus logging-only experiment named above, with a
precommitted threshold for fewer false classifications and fewer operator minutes.

**NO-CLAIM:** this reports one checkout on 2026-08-31. It does not claim the census is complete, that
183 rows enumerate every OMP surface, that grep-derived leg counts classify test intent, or that type
collision counts are exhaustive. A name match is not semantic coverage; re-derivation under a stricter
method has not run.


---

## 1.6 Viability gates, risk posture, and stage disposition

This is a **DISCOVER** decision with a narrowed first-value wedge. It is not KILL, PILOT, or BUILD.
The local technical evidence is a FACT about one checkout; buyer prevalence, willingness to pay,
reachable population, distribution, recurrence, rights, and defensibility are UNKNOWN. The authoritative
brief still marks Q1–Q3 as OPEN/PARTIAL; gates 1–4 and 12 are therefore not passed. The fresh grade imposed no
“dispatchable-stage” contract, so this section does not add one.

### Gate status (8–17)

| gate | test | status now | precommitted pass / kill rule |
|---|---|---|---|
| 8 | reachable population, sourced from named fleet operators | **OPEN / UNKNOWN** — no external population sample | pass if 5 qualified operators provide a redacted mismatch; kill/narrow if fewer than 2 do |
| 9 | bottom-up reachable economics | **OPEN / UNKNOWN** — no baseline minutes, volume, or ACV | pass if measured annualized avoided operator cost is at least 3× proposed annual price for 3 of 5 operators; otherwise narrow or kill |
| 10 | distribution access | **OPEN / UNKNOWN** — no acquisition route or funnel | pass if 3 of 5 qualified buyers name an reachable channel and accept a concierge introduction; otherwise do not promote |
| 11 | first-value path | **PARTIAL** — local enforcement exists, OMP supervisor integration does not | pass if 4 of 5 redacted replays produce a reviewable completion → receipt → typed close/refusal; kill this wedge if fewer than 2 do |
| 12 | paid commitment at the proposed 500/month | **OPEN / UNKNOWN** — price is a test, not a validated fact | pass if 3 of 5 qualified economic buyers sign a non-binding letter of intent after replay; zero is a kill signal |
| 13 | unit economics | **OPEN / UNKNOWN** — support, onboarding, compute, and acquisition cost unmeasured | pass if fully loaded recurring cost is below one third of 500/month for the measured cohort; otherwise reprice or kill |
| 14 | recurrence and retention reason | **OPEN / UNKNOWN** — one stand-down is not recurrence | pass if 4 of 5 operators report the mismatch at least monthly over a 30-day diary; otherwise narrow |
| 15 | rights, security, and licensing | **OPEN / UNKNOWN** — data-use rights, secrets, permissions, and licenses unreviewed | pass only with written rights and license review, no unresolved secret/permission blocker, and a fail-closed access test |
| 16 | defensibility / compounding asset | **OPEN / UNKNOWN** — no measured moat against scripts, labor, or native tools | pass only if 3 of 5 buyers choose the receipt contract over their strongest substitute and retained evidence improves the next replay; otherwise treat as commodity |
| 17 | proportionality against substitutes | **OPEN / UNKNOWN** — no 3× incremental comparison exists | pass if measured time/rework reduction is at least 3× incremental adoption cost for 3 of 5 operators; otherwise do not build |

No gate row above is evidence of a pass merely because a technical type or binary exists. A gate is
passed only by its stated external or end-to-end test and a retained receipt.

### Owned risk register

| risk | impact / uncertainty | mitigation | go/no-go test |
|---|---|---|---|
| dispatch blast radius | a false positive could close the wrong work or trigger unwanted action | keep the wedge reviewable and non-autonomous; typed refusal by default | known-good/known-bad replay cannot close without a matching receipt |
| secrets and tokens | OMP/ntm access could expose credentials or cross a permission boundary | least privilege, redacted fixtures, fail closed on missing scope | permission-denied and secret-bearing fixtures produce refusal with no secret emission |
| compatibility and upstream drift | OMP event shape or binary flags can change | pin and record versions; probe tmux with -V; treat stale input as UNKNOWN | one version-drift fixture preserves typed UNKNOWN rather than acting |
| licensing and data-use rights | upstream types, mirror data, or customer frames may be unusable | rights and license review before any paid pilot | written approval covers every shipped dependency and retained frame |
| access and distribution | no buyer channel or install path has been proven | concierge discovery only; no broad rollout claim | 3 of 5 buyers identify an accessible channel and complete install/replay |
| operational failure | adapter may miss asynchronous events or hang after terminal frame | bounded capture, explicit unknown state, no autonomous close | crash, killed pane, rate-limit, and compaction fixtures remain non-success |

### Explicit disposition

**DISCOVER / NARROW — owner:** omp-orchestrator product owner; **reviewer:** an independent viability
reviewer appointed by the project lead. The next test is the five-operator interview plus concierge
replay described in the thesis. Promotion to PILOT requires gates 8–17 plus gates 1–4 and 12 to pass,
three paid commitments at the proposed price, rights/security approval, and a retained end-to-end
receipt. Promotion to BUILD requires a second independent attack and the same conjunction; there is no
implicit promotion by crate count or green internal gates. KILL the wedge if fewer than 2 of 5 operators
show the mismatch, fewer than 2 of 5 complete the first-value replay, or no qualified buyer signs after
five replays. Revisit DISCOVER after 30 days or when the population, recurrence, or price evidence
changes; until then, PILOT and BUILD are prohibited.

### Number provenance and stale-data rule

The following is the section-wide provenance ledger. Values marked “reported” are not fresh command
receipts; they are retained only to explain the local argument and are non-authoritative for promotion.
The current number registry and corrected brief win over every copied value.

| figures | provenance and as-of |
|---|---|
| 6, 7, 23 | historical recollections from one stand-down; no retained artifact or derivation command; non-authoritative |
| 162 and 4.2 hours | UNVERIFIED REPORTED VALUE in 00-brief.md §3.7 line 546; command receipt not retained in this section; cited as of 2026-08-31, not independently reproducible or a market-frequency claim |
| 28/25/19/2 and 74 total | board snapshot reported in 00-brief.md §3.3 lines 498–502, as of 2026-08-31; command receipt not retained; corrected arithmetic, not live-board truth |
| 23 scanner tests | source-count FACT as of 2026-08-31: 13 markers in types_inventory.rs and 10 in tests/inventory.rs; not a pass count |
| 26 crates, 413 tests, 31 test files | current registry-backed snapshot as of 2026-08-31; 406/407 are historical and retracted; NUMBERS.toml is authoritative |
| 184/207/183, 544,697, 18, 4 | reported inventory snapshot from 00-brief.md §3.3 as of 2026-08-31; command receipt not retained here; not product completion evidence |
| 8 gates and gate-leg counts | reported corrected snapshot from 00-brief.md §3.5 as of 2026-08-31; grep naming counts are not semantic coverage |
| 51/59 enums, 79/91 structs, 6/17/4 collisions | reported type-inventory snapshot as of 2026-08-31; library-only versus all-source scopes are intentionally distinct |
| tmux 3.6a | captured by tmux -V; echo "exit=$?" on the host, as of 2026-08-31; the long-flag failure is not absence |
| raw agent_end receipt | /tmp/grade/agent-end-raw-frame.json; capture command and SHA-256 are recorded in section 1.2.1; artifact mtime/retrieval 2026-08-31T19:52:26-0600 |

Any later correction must update this ledger or remove the figure. Historical/retracted values are never
silently promoted back to current facts.

## 1.7 Recorded under R11

Two constraints surfaced while writing this section and are recorded here rather than left in
conversation:

1. **The cargo shim is an unmeasured supply-chain surface** in the metadata path (§1.3.6). We know it
   is a shim; we have not measured its effect on arguments or toolchain selection.
2. **Tmux's old “no machine-readable handshake” interpretation was wrong.** The current correction is
   the short -V handshake and a false-negative-on-presence risk for loops that use --version (§1.3.9).

**NO-CLAIM:** nothing here commits to a schedule, a cost, or an architecture validated by execution.
This section establishes a scoped hypothesis, local observations, dependency-contract floor, risks, and
explicit discovery gates. It does not imply product viability, PILOT, or BUILD.
