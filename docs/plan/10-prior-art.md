# 10 — What would Jeffrey do: prior art mined from the mirror

**Requirement served: R7.** *"use fh — mine the dicklesworthstone projects along the way — anywhere we
find a gap — we should ask — what would jeffrey do in one of his projects."*

Nine gaps. Each carries the gap, the exact search, a verbatim quote citing **the named construct** (not
a bare line number) **or** an explicit not-found naming the pattern *and the search space*, and an
**ADOPT / ADAPT / REJECT** verdict. **Every citation was verified by opening the cited file.** This
section was seeded from an earlier read-only pass whose document was never produced, so its findings
arrived as *leads*: **five of nine seeded verdicts did not survive**, four of them `no prior art found`
verdicts that were searches stopped too early. That correction rate is the most useful number here.

---

## 0. Denominators, tooling state, and four ways to manufacture a false zero

```
ls -1 | wc -l                                      -> 218   # visible entries
find . -maxdepth 1 -mindepth 1 -type d | wc -l     -> 217   # directories
find . -maxdepth 2 -mindepth 2 -name .git | wc -l  -> 210   # actual git work-trees
ls -1 | grep -c corrupt                            -> 1     # ntm.corrupt-20260819
```

**This section uses 210** — a directory without a work-tree is not a project. Brief §3.7's "216 repos"
is a fourth count, not re-derivable from the above; recorded, not resolved.

**`fh` fails closed in both surfaces with two different typed codes** — MCP refuses
`SERVE_INPUT_STALE` (mirror HEAD moved `5dec4212…` → `ecdea397…`), the CLI refuses
`SEARCH_INDEX_STALE` at `exit_code:3`:

```
"message":"SEARCH_INDEX_STALE: published key f2845efff917afd4 differs from current b1acb6e7b011b1f5",
"hint":"run `fh technical-manifest` to rebuild the standing search index","retryable":false
```

Two refusals, two `failure_kind`s, both naming the drifted key, one naming its own repair command.
**That is the model, not the defect**, and it is itself prior art for Gap 4. All nine searches below ran
against the filesystem, unassisted by any semantic index.

**Seven `no prior art found` verdicts were refuted this session, by four distinct mechanisms — all
indistinguishable from real absence by inspecting the output:**

1. **`--include=` returning empty at exit 0** in this harness. Hit once here, on an installer search.
2. **An extension filter aimed at the wrong language** — `--include='*.rs'` over `ntm`, which is a **Go**
   repo. Hit three times, and **this section committed one of them** in its first Gap 8 pass.
3. **A search space too narrow to contain the answer** — the original Gap 6 pass searched three
   documentation files, expecting doctrine to live in docs. It lives in test code, production telemetry,
   a shell gate and a type alias.
4. **A not-found published with no recorded search at all.** The seeds for Gaps 3 and 5 asserted absence
   without naming a pattern or a space, so neither could be audited — and both are refuted below. This
   is the worst of the four, because the other three at least leave a reproducible command behind.

**Two rules adopted here.** *A not-found is publishable only if it names the exact command **and**
argues the search space could have contained the answer.* And *a citation must name the construct*:
`doctor.rs:924` is unverifiable without knowing whether 924 is the function, the gate or the predicate.
Line numbers drift; a named construct survives a reformat.

---

## Gap 1 — A publish that returns no receipt

**Gap.** Dispatch emits no typed acknowledgement, so "sent" and "accepted" are one observable.
**Search.** `grep 'pub (struct|enum) (PublishReceipt|AckKind|DeliveryClass|PublishPermit)'` over the
whole `asupersync/src/messaging` module, not a file list.

**Found.** `messaging/fabric.rs:1913` (`struct PublishReceipt`) carries `subject`, `payload_len`,
`ack_kind`, `delivery_class`. The obligation is in the type system at `fabric.rs:1944`:
`#[must_use = "a PublishPermit must be sent or explicitly aborted"]`.

**Lead corrected.** The seed placed all four types in `fabric.rs`; only two are there. `AckKind`
(`Accepted`, `Committed`, `Recoverable`, `Served`, `Received`) is `class.rs:83`, `DeliveryClass` is
`class.rs:17`. The seeded `#[must_use]` on `cost_vector` verified exactly at `class.rs:43` — one of a
pair; `minimum_ack` carries it at `class.rs:56`.

**Verdict: ADAPT**, not ADOPT. `crates/omp-types/Cargo.toml:10-18` records why in our own tree:
`messaging-fabric` transitively needs `test-internals`, which upstream issue #46 removed from defaults.
**Prior art existing upstream is not prior art we can call.** Adopt the shape — a receipt carrying an
ack boundary, a permit that cannot be dropped silently — and define it locally.

---

## Gap 2 — An allowance list that outlives the defect it records

**Gap.** Declare "this lane exists but nothing runs it" without the declaration becoming permanent.
**Search.** `grep -n 'UNWIRED_LANE_ALLOWANCE' franken_lean/crates/fln-conformance/tests/contract_roots.rs`

**Found.** `contract_roots.rs:284-288` — and the list is **empty**, so every lane is wired:

```rust
/// Checked in BOTH directions ... an undeclared unwired lane fails, and a declared lane
/// that has since been wired ALSO fails. So the allowance shrinks as lanes land and
/// cannot quietly outlive the defect it records.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[];
```

The seeded test name verified at `contract_roots.rs:777`
(`fn allowance_verdict_fails_in_both_directions`), which exercises all four combinations directly
because the filesystem path could not reach one branch. The refusal text at `:757-761`: *"an allowance
that outlives its defect is how a repaired gap keeps reading as broken, and it is what stops this list
from shrinking."*

**Verdict: ADOPT verbatim, as the shape of every allowance list here.** We already have one needing it:
`crates/omp-inventory-map/src/types_inventory.rs:176-178` excludes `Observation` from an allowance
list, with no dual-direction test.

---

## Gap 3 — A binary that cannot say which source built it

**Gap.** An installed binary cannot prove it matches the tree it claims, so install drift is invisible.
**Search.** `grep -rl vergen --include=Cargo.toml .` then `grep -rn 'binary_identity|build_id|running_binary'`.
The corpus is Rust here so the filter was defensible; both re-confirmed with the harness grep.

**Found — the seeded verdict was WRONG.** The seed said `no prior art found`. **18 of 210 repos** build
identity in via `vergen`; `beads_rust/build.rs:41-45` (in `fn emit_git_metadata`) emits the drift signal
specifically: `emit_env("VERGEN_GIT_DIRTY", if status.is_empty() { "false" } else { "true" })`. The
strongest statement is the doc comment on `fn running_binary_identity`,
`frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846`:

```rust
/// Fails closed: a gate that cannot name the exact binary it measured is not
/// admissible evidence, so an unresolvable path or any read error panics
/// rather than degrading to an unidentified run.
```

**Two honest negatives inside the positive.** The best identity string in the toolset is `fh`'s —
`franken-harvest 0.1.0+tree.<64-hex>.src.<40-hex>` — but its source is **not in the mirror**
(`ls -1 | grep -i harv` → no match), so it is a measured artifact, not mirror prior art. And
`beads_rust` embeds identity without exposing it: `br --version` prints `br 0.4.1`; the SHA is read only
at `src/cli/commands/version.rs:55`. Embedding and exposing are two decisions.

**Verdict: ADOPT** the tree-digest-plus-dirty-flag shape with the fail-closed rule quoted above.

---

## Gap 4 — The canonical doctor shape

**Gap.** We have no `doctor`, and what we do have is undiscoverable — `omp-inventory-map --help` returns
`CONFIG_ERROR unknown argument --help` (brief §3.6). **Search.** `grep -rn 'DoctorExitCode' beads_rust/src`;
`grep -n 'Commands::Doctor' beads_rust/src/main.rs`; `find pi_agent_rust -name doctor.rs`.

**Found.** The richest vein in the mirror; it feeds `07-installability.md` directly.

**(a) A typed exit-code dictionary — lead right, badly incomplete.** The seed named four variants;
`beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:51` (`enum DoctorExitCode`) declares
**eleven**, under a stability promise at `:45-49` (*"agent scripts that mask `match c { 0 => .., _ =>
bail }` cope safely"*):

```rust
Healthy = 0,  FindingsPresent = 1,  FixPartial = 2,  FixFailedRolledBack = 3,
RefusedUnsafe = 4,  ConcurrencyLost = 5,  OnlineRequired = 6,
UsageError = 64,  NoInput = 66,  CannotCreateOutput = 73,  IoError = 74,
```

The four seeded values confirm exactly. The seed missed the **two-band design**: 0–6 are doctor-domain
verdicts, 64/66/73/74 are `<sysexits.h>`, so a caller that knows nothing about doctors still gets
meaning. `FixFailedRolledBack = 3` is *"rolled back from the verbatim backup. Workspace state is
unchanged"* (`:21-24`) — a repair that fails cleanly is a distinct verdict from one that half-succeeded.

**(b) The doctor publishes its own contract.** `capabilities_doctor.rs:1-15` (module doc for
`br.doctor.capabilities.v1`) declares `write_scopes` (`.beads/`, `.doctor/`), `env_vars`, `fixers`,
`detectors`, and *"`exit_codes` — derived from `DoctorExitCode::all`"*. This answers §3.6's ADDRESSABLE
property: the binary enumerates its own detectors and fixers, so a surface cannot be
wired-but-unaddressable — and because the code list is **derived**, that drift is impossible.

**(c) An error naming its own repair command.** `eidetic_engine_cli/src/cache/hotset.rs:1504` (the
`"repair"` field of the degraded-class JSON), repeated at `:1519`:
`"repair": "Run \`ee doctor --workspace . --json\` if the store schema looks incomplete."`

**(d) Doctor scope declared in prose.** `pi_agent_rust/src/doctor.rs:1-5` (module doc): *"checks config,
directories, auth, shell tools, and sessions… With `--fix`, automatically repairs safe issues."*

**(e) `Doctor` is exempted from the preconditions every other command must satisfy.**
`beads_rust/src/main.rs:104` and `:297`, both `&& !matches!(cli.command, Commands::Doctor(_))`. The
sharpest idea in the vein: **the tool you run when the workspace is broken must not require the
workspace to be intact.** A doctor gated behind the checks it exists to diagnose is not a doctor.

**Verdict: ADOPT, as the spine of `07-installability.md`** — a `#[repr(i32)]` two-band exit enum; a
`capabilities.v1` document deriving its own codes and naming write scopes, detectors and fixers; every
error carrying a runnable `repair` string; `doctor` exempted from preflight.

---

## Gap 5 — Mutation through a real hook, not a fixture

**Gap.** Our gate suites mutate fixtures; a fixture cannot tell us the *installed* hook still refuses.
**Search.** `grep -rln 'hooks/pre-commit'` across `beads_rust`, `franken_lean`,
`destructive_command_guard` with **no extension filter** — deliberately, because a git hook is
extensionless and the answer was in shell and Python, not Rust.

**Found — the seeded verdict was WRONG.** `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362`
copies the **real** hook into a lab repo and chmods it executable; `scripts/git-hooks/test_projection_guard.sh`
drives real `git commit` against it, including case 8 at `:202-212`, asserting the guard **chains** to a
pre-existing `.git/hooks/pre-commit` rather than shadowing it. The reason is at
`ci/VERIFICATION_MANIFEST.jsonl:93`:

> *"CELL C IS WHAT MAKES CELL B MEAN ANYTHING: a successful hook prints NOTHING, so a green commit and a
> hook that never ran are indistinguishable from outside. C re-plants the empty row in B's OWN
> repository after B's success and requires a refusal…"*

Fixture *size* is asserted because the defect is a race, not a threshold (`test_projection_guard.sh:520-524`):
the broken form refuses 5% of the time at 50 627 B, 92% at 72 725 B, 100% only from ~98 KB. The
counter-example is instructive: `asupersync/src/subsystem_mutation_testing.rs:9` is gated
`#![cfg(all(test, feature = "real-service-e2e"))]` but builds a `LabRuntime` over a `TempDir` — a
fixture. Both patterns exist; only franken_lean's reaches the installed artifact.

**Verdict: ADOPT.** The rule: *a successful gate prints nothing, so a green run and a gate that never
ran are indistinguishable* — every gate needs a planted-defect cell that **requires** a refusal.

---

## Gap 6 — Refusing an empty scan set

**Gap.** Brief §3.3: all 183 census rows carry exactly **1 distinct** `must_be_true`. Our own inventory
satisfies the four-field discipline vacuously. Did Jeffrey solve this?

**Search — the longest here, because the first pass was a false zero by mechanism 3.** The original
searched three documentation files (`asupersync/docs`, two `AGENTS.md`) expecting doctrine to live in
docs. Re-derived with no extension filter across whole repos:

```
grep -rli 'vacuous' asupersync             -> 37 files    # docs held almost none of it
grep -rlEi 'vacuit'       --include=*.rs . -> 236 files
grep -rlEi 'anti.vacuity' --include=*.rs . ->  63 files
```

Also tried: `scanned zero`, `empty scan set`, `no files were scanned`, `scan set is empty`,
`would pass vacuously`, `zero (files|candidates) (scanned|examined)`.

**Found — the seeded verdict was WRONG, and it was the row flagged CRITICAL.** Anti-vacuity is a
pervasive *named* discipline across `frankenmermaid` (51 files), `franken_lean` (50), `frankensim` (35),
`frankengit` (21) and eleven more. **Five distinct shapes, strongest first:**

**1 — Vacuity as a typed state, not a failure. Better than anything we designed.**
`asupersync/src/messaging/jetstream.rs:2460` serialises into **production telemetry**:
`waiter_fairness_mode: "vacuous_zero_wait_refusal".to_string(),` and
`scripts/run_jetstream_publish_backpressure_smoke.sh:181-186` gates on that exact string. The system
neither claims fairness nor fails — it **names its own vacuity as a first-class value**. That splits the
concept: **structural** vacuity (trivially satisfied by construction, legitimate, must be named) versus
**accidental** vacuity (the scan found nothing, must fail). Our design had only the second. The irony is
exact: our census's universal `must_be_true` reads *"The source probe is non-empty before a known verdict
is emitted"* — the right rule, stated identically 183 times.

**2 — Anti-vacuity guards inside the metamorphic relation.**
`asupersync/src/runtime/scheduler/metamorphic_tests.rs:438-442` (the MR4 `prop_assert!`):

```rust
prop_assert!(cancel_dispatches >= 1,
    "MR4 VIOLATION: zero cancel dispatches across {} injected cancel tasks — \
     streak-bound assertion would be vacuous", cancel_tasks,
```

Repeated for MR5 (`:517-522`, an *"ABSOLUTE-CORRECTNESS ANCHOR"*) and MR7 (`:661-662`). The relation does
not merely hold — the test first proves the workload was exercised.

**3 — The anti-vacuity floor as an explicit rule.**
`franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115`: *"ANTI-VACUITY FLOOR.
An empty or implausibly small scan is a BROKEN SCAN, not a clean tree — … most sharply when a derived
scope returned zero and read as 'nothing to report'"*, enforced by `assert!(workflows.len() >= 2, …)`.
Note `>= 2`, not `> 0`: *implausibly small* is also broken. The one-liner form is
`franken_lean/tribunal/epoch-lab/tests/derived_input_provenance.rs:538` —
`assert!(p.item_count > 0, "{} scanned nothing", p.rule)` — and `VACUOUS PASS` exists as a named verdict
at `build_gate_governed_sets.rs:549`, asserted at `:613`.

**4 — Positive control justified as anti-vacuity**, fusing two of our separate gate properties into one
mechanism. `asupersync/tests/atp_rq_observability_metrics.rs:134-135`: *"Positive control: the manifest
DOES carry its content-descriptor fields, so the negative assertions above are meaningful (not vacuous on
an empty blob)."*

**5 — Pushed into the type system and return values.** `asupersync/src/trace/tla_export.rs:111-114`
declares `pub type EntityKey = (u32, u32)` precisely so slot reuse cannot alias two entities onto one key
and let `QuiescenceOnClose` *"pass vacuously"*; `combinator/map_reduce.rs:140-144` has `all_succeeded()`
return **false** on empty input *"even though the aggregate decision is `AllOk` (vacuously true)"*.

**Measured yield, which makes the verdict unarguable.** `asupersync/CHANGELOG.md:1077-1078` records six
RFC 9112 tests *"that previously passed vacuously when codec validation was missing"*, and
`audit_index.jsonl:3251` records `MR2 cancellation_state_consistency` in
`tests/metamorphic_task_inspector.rs` as *"vacuous because it toggled an unrelated testing Cx"* —
`verdict: FIXED`, found by cross-review on 2026-04-19. Our exact defect, in his repo, caught by his own
audit and closed.

**Verdict: ADOPT — the highest-priority adoption in this file.** Take all five shapes, above all shape 1:
a gate must report *structurally vacuous* as a named state distinct from both pass and fail. Applied to
§3.3, our census would refuse itself today — 183 rows over 1 distinct `must_be_true` is accidental
vacuity, and nothing in our tree can say so.

---

## Gap 7 — A worker that cannot say it is done

**Gap.** Brief §4's `complete` row: *"worker says done — DOES NOT EXIST — every completion this session
was found by a human looking."* Our largest architectural hole. **Search.** `grep 'pub enum Outcome'
asupersync/src`, then `grep 'pub enum (ChildExit|ExitReason|ChildOutcome|SupervisionEvent|ChildStatus)'`
over `supervision.rs`, `gen_server.rs`, `spork.rs` — the three supervision surfaces, enumerated from
`ls src` rather than guessed.

**Found, half.** `asupersync/src/types/outcome.rs:213-227` (`enum Outcome<T, E>`) — seeded variants
verified exactly, and the lattice is the part worth having:

```rust
/// Forms a severity lattice where worse outcomes dominate: `Ok < Err < Cancelled < Panicked`
pub enum Outcome<T, E> { Ok(T), Err(E), Cancelled(CancelReason), Panicked(PanicPayload) }
```

`Cancelled` and `Panicked` are not collapsed into `Err` — a timeout is not a verdict and a panic is not
an error, our async contract (brief §3.7) already stated as a type.

**Not found, and this is the finding.** `asupersync/src/supervision.rs:3122` (`enum SupervisionEvent`)
has **eight variants**: `ActorFailed`, `DecisionMade`, `RestartBeginning`, `RestartComplete`,
`RestartFailed`, `BudgetExhausted`, `Escalating`, `BudgetRefusedRestart`. **Every one is failure,
decision, restart or escalation; there is no completion variant.** The nearest, `RestartComplete`,
reports a *restart* finishing, not work. `supervision.rs:3098` (`enum StopReason`) is the same across
six: `ExplicitStop`, `RestartBudgetExhausted`, `BudgetRefused`, `Cancelled`, `Panicked`, `RegionClosing`.
None means *"the worker finished what it was asked to do"*; `ExplicitStop` is the supervisor stopping the
child — the opposite direction of travel.

**Stated plainly, because this is the most consequential result in the section: across 210 work-trees,
`SupervisionEvent` carries 8 variants and `StopReason` carries 6, and not one of those 14 means "the
worker finished." The mirror provides a severity lattice and an evidence ledger and no completion
protocol.** So the supervisor learns that a child DIED, never that it FINISHED.

That independently confirms brief §4's dead `complete` row — the layer where every completion this
session was found by a human looking — and it upgrades that row from *"we have not built it"* to
**"it is precedent-free across the entire corpus we mine for precedent."** It cuts both ways and the
plan must carry both halves. It is the strongest evidence the gap is real and worth building: eighteen
mature repositories converge on failure-only supervision, so the absence is structural rather than an
oversight. And it is the strongest warning attached to the work: **nobody in this lineage has solved
it, so our estimate for it has no anchor** — no reference implementation to diff against, no prior
schedule to calibrate from, and no way to discover we designed it wrong except by running it.

One adjacent mechanism is worth taking. `supervision.rs:3208-3213` opens an **Evidence Ledger**:
*"Structured, deterministic, test-assertable record of why each supervision decision was made. Every call
to `Supervisor::on_failure_with_budget` appends exactly one `EvidenceEntry` whose `binding_constraint`
field…"* — one entry per decision, naming the constraint that bound.

**Verdict: ADAPT, and own the remainder.** Adopt `Outcome<T,E>` with its lattice (already in `omp-types`)
and the one-entry-per-decision ledger. **Design** the completion signal ourselves: a `WorkerReport`
carrying `Outcome`, the claim it discharges, and the evidence discharging it — pushed by the worker, not
polled by a human. **This is the one place where the mirror gives us a lattice and a ledger but no
protocol.**

---

## Gap 8 — Per-adapter scoping and typed missing dependencies

**Gap.** A CLI run inside someone else's repo must scope what it touches and degrade per adapter when a
dependency is absent.

**Search, first pass — wrong.** `grep '(adapter|Adapter)\w*(registry|Registry|scope|Scope)|per-adapter'`
over `beads_rust/src`, `eidetic_engine_cli/src` → no matches. **That space was Rust-only** — mechanism 2,
committed here. **Second pass:** `grep -rn 'ErrNotInstalled|DEPENDENCY_MISSING' ntm` with no extension
filter, because ntm is the wrapped binary written in Go and the one that shells out to foreign deps.

**First pass still yielded something.** `asupersync/src/adapter_certification.rs:1-6` (module doc) — *"the
source-owned declaration surface that keeps adapter identities and fail-closed status from drifting into
hand-maintained prose"* — with `enum AdapterCategory` (`:10`), `enum AdapterCertificationStatus` (`:39`,
`CertifiedLive` = *"live implementation and reference coverage are wired"*), `enum AdapterRenderedStatus`
(`:65`, `Pass` vs an *"expected fail-closed status"*) and `struct AdapterCertificationDeclaration
{ adapter_id, category, … }` (`:88`).

**The second pass is the better half.** `ntm` ships the typed missing-dependency vocabulary at four
layers. A **per-dependency typed sentinel** — `internal/bv/bv.go:31` (`var ErrNotInstalled`), identically
at `internal/cass/client.go:13` and `internal/caut/client.go:14`. A **shared wire taxonomy** —
`docs/robot-action-handoff-contract.md:379`, `ErrCodeDependencyMissing = "DEPENDENCY_MISSING"`. The
**remediation carried inside the envelope**, not printed beside it — `internal/cli/bugs.go:85-89`:

```go
response := robot.NewErrorResponse(cause, robot.ErrCodeDependencyMissing,
    "Install UBS from https://github.com/nightowlai/ubs, then rerun 'ntm bugs list --json'")
```

An **explicit per-call-site degradation policy** — `internal/alerts/generator.go:383-385`: the same
missing binary is fatal in `bugs.go` and silent here, and the difference is a *decision*
(`// Silently skip when bv is not installed; only warn on real errors.`, guarded by
`!errors.Is(err, bv.ErrNotInstalled)`). And a **conformance test pinning the exit code**, which makes the
rest binding — `internal/cli/robot_registry_conformance_test.go:15`
(`fn TestRobotProcessExitContractReservesUnavailableForNotImplemented`): the envelope *asks for* exit 2
via `WithExitCode(2)` and the contract *gives it 1*, because 2 is reserved for `NOT_IMPLEMENTED`. A
missing dependency and an unimplemented action may not collide, and a response's declared exit code does
not override the taxonomy.

**Three of these five citations reached this section second-hand and carried off-by-one errors** — the
sentinels sit one line below the comment cited (`bv.go` 30→31, `cass` 12→13, `caut` 13→14) and the
degradation comment is `generator.go:383`, not `:384`. Quotes otherwise exact; numbers above corrected.

**Verdict: ADOPT** — upgraded from ADAPT on the second pass. Compose ntm's four pieces with
`adapter_certification.rs`'s fail-closed per-adapter status and `capabilities_doctor.rs`'s declared
`write_scopes`, enforced by `DoctorExitCode::RefusedUnsafe = 4` (*"write outside safety_envelope §2
scopes"*).

---

## Gap 9 — Probing tool presence without trusting exit codes

**Gap.** We wrap nine binaries and no single version flag covers them (`--version` answers 8/9, `-V`
answers 5/9), so a naive probe records a present binary as absent. **Search.**
`grep -n 'PresenceOnly|ProbeExecution|fn check_tool|fn probe_failure_is_known_nonfatal|fn which_tool|status.success()'`
over `pi_agent_rust/src/doctor.rs`, then read each construct.

**Found — and the precedent ships its own remediation and its own tests.** Cited by construct, because
three agents gave three different line numbers for this precedent and each was naming a different
construct on adjacent lines: `doctor.rs:924` is `fn check_tool`; `:950` is the naive arm
`Ok(output) if output.status.success()`; `:967-968` is the **two-signal arm**,
`Ok(output) if discovered_path.is_some() && probe_failure_is_known_nonfatal(...)`; `:1052` is
`fn probe_failure_is_known_nonfatal`; `:1057` is its one-tool allowlist,
`if tool.ne("sh") || args.ne(&["--version"]) { return false; }`; `:1066` is `fn which_tool`, the
independent presence signal.

The design is **two independent signals**: presence comes from `which_tool`, never from the version
probe, and a failed probe is forgiven only via a *named predicate* — `doctor.rs:970-971`: *"Some shells
(e.g. dash as /bin/sh) do not support --version. If this is the known non-fatal probe case, treat tool as
present."* **Both arms are pinned by tests**: `fn check_tool_falls_back_when_probe_args_are_unsupported`
(`:13948`) and `fn check_tool_reports_invocation_failure_for_broken_executable` (`:13964`). The second is
the one we would skip — it proves the fallback did not become a blanket amnesty. **That is the known-good
leg for a presence probe, precisely the leg `path-literal-guard` is missing in our tree.**

**A brief fact does not survive verification, and the correction inverts it.** Brief §3.1 states
`tmux --version` *"prints an error AND EXITS 0 — it fails while reporting success."* Measured four ways:

```
tmux --version                          -> exit 1, banner on STDERR, stdout EMPTY (0 bytes)
env -i /opt/homebrew/bin/tmux --version -> exit 1
v=$(tmux --version 2>&1)                -> exit 1
tmux --version 2>&1 | head -1           -> exit 0     <-- source of the claim; PIPESTATUS=(1 0)
tmux -V                                 -> exit 0, "tmux 3.6a"
```

**tmux is honest** — it fails, says so on stderr, returns 1. The `exit 0` came from a pipeline, whose
status is `head`'s; the exit code that lied belonged to *the measurement harness*, same family as the
brief's `installer` example. The stated attack ("captures a usage banner as the version string") cannot
occur, since stdout is empty; the real hazard runs the other way — **a probe treating non-zero as ABSENT
records tmux, present at 3.6a, as MISSING**, a false negative on the one binary through which we read
pane truth.

**Verdict: ADOPT WITH A NAMED GAP.** Take the two-signal structure and **both** tests. The gap is
`doctor.rs:1057`: the predicate is an allowlist of exactly one tool, so tmux — whose stderr matches the
spirit (`unknown option`) but not the allowlist — falls to the failure arm and is marked MISSING today.
Ours: presence from `which`, a per-binary flag table (`-V` for tmux, `--version` for eight), absence
reported in Gap 8's `DEPENDENCY_MISSING` vocabulary.

---

## Summary

| # | gap | verdict | citation, or not-found |
|---|---|---|---|
| 1 | delivery receipts | **ADAPT** | `fabric.rs:1913,1944`; `class.rs:43,83` — unreachable at our pinned rev |
| 2 | unwired-lane allowance | **ADOPT** | `contract_roots.rs:288` (the const), test at `:777` |
| 3 | binary identity / install drift | **ADOPT** | `bd_wsw3p_concurrent_write_showcase.rs:846`; `beads_rust/build.rs:41`; 18/210 repos |
| 4 | canonical doctor shape | **ADOPT** | `exit_codes.rs:51`; `capabilities_doctor.rs:1`; `main.rs:104`; `hotset.rs:1504`; `pi_agent_rust/src/doctor.rs:1` |
| 5 | mutation through a real hook | **ADOPT** | `evidence_finalization.rs:360`; `test_projection_guard.sh:202`; `VERIFICATION_MANIFEST.jsonl:93` |
| 6 | anti-vacuity / empty scan set | **ADOPT** | `jetstream.rs:2460`; `metamorphic_tests.rs:438`; `marrow_sanitizer_dispatch.rs:105`; `tla_export.rs:111`; `CHANGELOG.md:1077` |
| 7 | typed worker→supervisor done signal | **ADAPT** | `outcome.rs:218` exists; `supervision.rs:3122` — **8 variants, none a completion** |
| 8 | per-adapter scoping / typed missing-dependency | **ADOPT** | `ntm/internal/bv/bv.go:31`; `robot-action-handoff-contract.md:379`; `bugs.go:85`; `generator.go:383`; `robot_registry_conformance_test.go:15` |
| 9 | tool probe not trusting exit codes | **ADOPT + NAMED GAP** | `doctor.rs:967` (two-signal arm), `:13948`/`:13964` (both tests); gap at `:1057`; brief §3.1 tmux measurement refuted |

**Seeded-verdict correction rate: 6 of 9 (the earlier "5 of 9" excluded tmux and is retired) — more than half the verdicts handed to this section were
wrong, and Gaps 6, 8 and 9 were each corrected twice.** Gaps 3, 5, 6 and 8 were seeded
`no prior art found` and all four have prior art — Gap 6 emphatically, and it was the row flagged
CRITICAL. Gap 1 was seeded ADOPT and is ADAPT. Gap 9's measurement was inverted, and its verdict then
rose to ADOPT once the precedent's own tests were found.

**Seven refuted not-founds across four mechanisms, from four independent authors, is the strongest
argument here for the writing contract's rule 7.** The count is a property of the session, not an
attribution: it does not matter whether a given false zero was one author's to retract or another's to
find. Two of the seven were this section catching its own earlier output, which is the strongest kind —
a checker refuting itself rather than someone else. Every one was caught by **re-deriving**, never by
**reading**, and none was distinguishable from genuine absence by inspecting output. Hence §0's two
rules — name the search space, not just the pattern; name the construct, not just the line. Gap 6 is the
proof: the corpus refuting its not-found lives in production telemetry, a `prop_assert!`, a shell gate
and a type alias, and none of those is where the first search looked.

### What has no precedent, and what that means

**No whole gap is precedent-free.** Every seeded absence dissolved under a second search. One specific
shape genuinely is:

> **A typed success-completion event in a supervision protocol.** Across 210 work-trees, supervision
> vocabularies enumerate failure, cancellation, panic, restart, budget exhaustion and escalation. None
> enumerates *finished*.

That is the centre of Gap 7 and of brief §4's dead `complete` row. **The risk:** we would design the
worker→conductor protocol with no in-house precedent and nothing to diff against — in a repo whose own
census (§3.3) proves we can satisfy a discipline vacuously. **Why it is worth building:** the absence is
not an oversight across eighteen mature repositories. Erlang-lineage supervision is a *failure* protocol
by construction — a supervisor restarts what died, and a child that finishes merely stops. Our conductor
is not a supervisor but a work dispatcher, and a dispatcher that cannot observe completion cannot
dispatch. The piece is missing because nobody in the lineage needed it, and it is load-bearing for us
exactly there. Mitigation is named in Gap 7: build on the `Outcome` lattice, so a cancelled worker is
never laundered into a failed one, and the one-entry-per-decision ledger, so a completion claim arrives
with its evidence. New protocol, borrowed vocabulary, borrowed audit trail.

---

**NO-CLAIM.** This section establishes that a pattern exists in the mirror at the cited construct, and
nothing more — not that it is correct, is the best solution, will transfer here, or will close the gap it
is cited against. An ADOPT verdict is a decision to try, not evidence of a result. It does not claim the
searches were exhaustive: they covered 210 git work-trees at one filesystem snapshot, using the patterns
named per gap, with `fh` refusing in both surfaces so no semantic index assisted them. **Seven refuted
not-founds establish that incomplete search is this document's live failure mode, not a hypothetical
one**, and nothing here proves an eighth is absent. It does not claim the mirror is the whole of the
prior art, nor that absence here implies absence anywhere. The tmux correction is measured on this
workstation only, against `/opt/homebrew/bin/tmux` 3.6a; it refutes one sentence of brief §3.1 and
leaves the flag-coverage table beside it intact. Every zero was re-derived against all four known
mechanisms, and the second-hand Gap 6, Gap 8 and Gap 9 citations were re-opened construct by construct
— which is how three off-by-one numbers were corrected. That re-verification is itself unaudited: no
one has re-derived these
searches independently of this author, which on this session's record is precisely the condition under
which a false absence survives.

---

## Gap 7 is REFUTED — the completion signal ships in the tool we wrap

This section's headline result was that a typed worker→supervisor completion signal is
**precedent-free across 210 mirror work-trees** — `SupervisionEvent` has 8 variants, `StopReason`
has 6, and none of the 14 means *"the worker finished."* That was stated as the single most
consequential finding in the plan, and as the strongest argument the gap was real.

**It is wrong, and it is wrong for the reason this document keeps recording: the search space
excluded the obvious place.** `%1408`, probing `type_root:extensibility` with the per-kind
instrument, found:

```typescript
// dist/types/extensibility/shared-events.d.ts:154
export interface AgentEndEvent {
    type: "agent_end";
    messages: AgentMessage[];
    /**
     * When true, the session has already scheduled an automatic continuation
     * (auto-retry, empty/unexpected-stop retry, etc.). Subscribers must not
     * treat this as a user-visible terminal settle.
     */
    willContinue?: boolean;
}
```

That is a **typed worker-completion event**, and it is better than the one we would have designed.
`willContinue` distinguishes *"terminal settle"* from *"a continuation is already scheduled"* —
which is precisely the `NewlyIdle` versus `ConfirmedIdle` distinction §00 §4 records as broken in
our own `idle_panes` filter. The tool we wrap solved our hardest problem and typed the edge case we
have not yet handled.

### The receipt gap is a design choice, not an impossibility

Same pass, same instrument:

```
dist/types/tools/hub/types.d.ts:8    import type { IrcDeliveryReceipt }
dist/types/tools/hub/types.d.ts:84   receipts?: IrcDeliveryReceipt[]
```

`cp-z42vu` — transport reporting `success:[N]` with no packet delivered — is recorded throughout
this plan as a missing receipt *type*. The type exists. As `%1408` put it: **"the receipt and
cross-session mechanisms EXIST in the substrate on planes we do not ride; the receipt gap is a
design choice, not an impossibility."**

### And `muxPing` was never broken

`%1414`, Task A: **six mux workers, all with live broker parents, stable pong on all 18 correct
socket probes.** The null that retired `omp/muxPing` was an **endpoint/protocol mismatch**, not
evidence about the surface and not explained by the mux count either. Third instance in one wave of
*the probe was wrong, not the surface*.

### Why the mirror search missed it

Gap 7 searched **210 mirror work-trees** for prior art and never searched **the binary we depend
on**. The search was real, the command was recorded, the result was honest — and the space was
chosen to exclude the single most likely location. That is the fourth distinct false-zero mechanism
this session, and the most expensive: it declared our largest architectural gap unprecedented while
the precedent shipped in the tool named on line one of the plan.

**What survives.** The mirror finding stands as stated — *Jeffrey's repos* contain no typed
completion protocol, and that remains true and interesting. What does not survive is the
**conclusion** drawn from it: that we would be building without precedent. We would be **adopting**,
from OMP, on a plane we do not currently ride.

**NO-CLAIM.** `AgentEndEvent` exists as a declared type in the installed `dist/types`. It is
**not established** that it reaches the `--mode=rpc` frame plane, which is the only place our
orchestrator could consume it — `%1408` specified the test (one live-session frame capture) and it
is **unrun**. Until it runs, this refutes *"no precedent exists"* and does **not** establish *"we
can consume it today."* Those are different claims and only the first is measured.

---

## Every named gap in this plan has an existing typed mechanism upstream

`%1408`'s signal sweep — same symbol-level instrument, applied to the whole `dist/types` tree —
found that **the refutation of Gap 7 was not an isolated miss**. It was the first of seven.

| gap this plan names | upstream type that closes it |
|---|---|
| worker completion (S6→S7) | `AgentEndEvent.willContinue` + `SessionStopEvent.settle`, carried on `RpcSessionEventFrame` |
| dispatch receipts (`cp-z42vu`) | `IrcDeliveryReceipt` + `AsyncJobDeliverySink` |
| claim / ownership | `Stage1Claim` / `GlobalClaim` with `ownershipToken` + `inputWatermark` |
| idle vs newly-idle | `GuestIdleReconcilerCtx` — the settle-vs-continuation split, done upstream |
| roster | `HubRosterCounts`, a five-tally schema |
| cost measurement (Q2) | `PerplexityCost` / `SearchUsage` / `ContextUsage` |
| **context compaction loss** | `SessionBefore` / `CompactEvent` pair |

Verified at the file level: `modes/rpc/rpc-types.d.ts:589` declares
`RpcSessionEventFrame = AgentSessionEvent | RpcSubagentFrame`;
`memories/storage.d.ts:20-27` carries `ownershipToken` and `inputWatermark`;
`extensibility/shared-events.d.ts:27,43,54` carries the session/compact event family.

### This changes the architecture from build to adopt

`RpcSessionEventFrame` is the decisive one. **The `--mode=rpc` wire has a dedicated event-frame
channel** — so the completion signal has a defined path to a consumer, and the open question
narrows from *"can we get a completion signal"* to *"does `agent_end` appear on a channel that
already exists for exactly this."* `%1408`'s read: **adoption, not bridge.**

The last row is the one to sit with. `SessionBefore` / `CompactEvent` is **a typed hook at exactly
the boundary where `%1413` lost its entire grading task tonight** — context hit 85 %, compacted, the
task vanished, and the recovery was a hand-written re-dispatch plus an invented file-output rule.
A typed pre-compaction event was available the whole time.

### The pattern, stated plainly

This plan documented seven architectural gaps, searched 210 repositories for prior art on one of
them, and **never searched the tool it wraps**. All seven have upstream types. The plan's central
technical claim — that we must build a completion protocol, a receipt type, a claim vocabulary and
an idle discriminator — is now **"we must adopt seven types from a dependency we already ship
against."** That is a materially different project: smaller, faster, and lower-risk.

**NO-CLAIM.** Seven types are *declared* in the installed `dist/types`. For **one** of them
(`AgentEndEvent`) a consuming channel is identified but the frame capture proving it crosses the
wire is **still unrun** — `%1414` has it now. For the other six, nothing establishes they are
reachable from `--mode=rpc`, that their semantics match our need, or that adopting them is cheaper
than building. What is established is narrower and still decisive: **"no precedent exists" is false
for all seven**, and every design decision this plan made on that premise needs re-examination.
