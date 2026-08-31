# 10 — What would Jeffrey do: prior art mined from the mirror

**Requirement served: R7.** *"use fh — mine the dicklesworthstone projects along the way — anywhere we
find a gap — we should ask — what would jeffrey do in one of his projects."*

Nine gaps. Each carries the gap in one sentence, the exact search, a verbatim quote with `file:line`
**or** an explicit not-found naming the pattern *and the search space*, and an **ADOPT / ADAPT /
REJECT** verdict.

**Every citation was verified by opening the cited file and reading the cited line.** That is
load-bearing: this section was seeded from an earlier read-only pass whose document was never
produced, so its findings arrived as *leads*. **Five of nine seeded verdicts did not survive.** Four
were `no prior art found` verdicts that were searches stopped too early. The correction rate is the
most useful number here.

---

## 0. Denominators, tooling state, and two ways to manufacture a false zero

```
ls -1 | wc -l                                      -> 218   # visible entries
find . -maxdepth 1 -mindepth 1 -type d | wc -l     -> 217   # directories
find . -maxdepth 2 -mindepth 2 -name .git | wc -l  -> 210   # actual git work-trees
ls -1 | grep -c corrupt                            -> 1     # ntm.corrupt-20260819
```

**This section uses 210** — a directory without a work-tree is not a project. The brief §3.7 figure of
"216 repos" is a fourth count, not re-derivable from any of the above; recorded, not resolved.

**`fh` fails closed in both surfaces, with two different typed codes.** MCP refuses with
`SERVE_INPUT_STALE` (mirror HEAD moved `5dec4212…` → `ecdea397…`). The CLI refuses differently:

```
fh search 'delivery receipt' --json
{"success":false,"code":"STALE","exit_code":3,"failure_kind":"SEARCH_INDEX_STALE",
 "error":{"message":"SEARCH_INDEX_STALE: published key f2845efff917afd4 differs from current
  b1acb6e7b011b1f5; rebuild it",
  "hint":"run `fh technical-manifest` to rebuild the standing search index","retryable":false}}
```

Two refusals, two distinct `failure_kind`s, both naming the drifted key, one naming its own repair
command. **This is the model, not the defect** — and it is itself prior art for Gap 4. All nine
searches below therefore ran against the filesystem, unassisted by any semantic index.

**Two hazards, both disclosed, because both manufacture a `no prior art found` that is
indistinguishable from a real one:**

1. **`--include=` returning empty at exit 0.** Shell `grep -r … --include=` intermittently returns
   nothing rather than erroring in this harness. One derivation below came back a false zero and was
   re-derived with the harness grep, which found matches.
2. **The extension filter aimed at the wrong language.** A sibling's `--include='*.rs'` over `ntm`
   found nothing — **ntm is a Go repo**. Structural absence read as semantic absence; re-derived
   without the filter, 93+ files. **This section committed the same error**: the first Gap 8 search
   was Rust-only. Gap 8 is corrected below and its verdict changed as a result.

**Operational rule, adopted here: a not-found is publishable only if it names the exact command AND
why the search space was the right one.** "I grepped and got nothing" is not a finding; "I grepped
`*.rs` across a Go repo and got nothing" is a bug.

---

## Gap 1 — A publish that returns no receipt

**Gap.** Our dispatch path emits no typed acknowledgement, so "sent" and "accepted" are one observable.

**Search.** `grep 'pub (struct|enum) (PublishReceipt|AckKind|DeliveryClass|PublishPermit)'` over
`asupersync/src/messaging`.

**Found.** `asupersync/src/messaging/fabric.rs:1911-1921`, and the obligation carried by the type
system at `:1943-1945`:

```rust
/// Packet-plane publish acknowledgement.
pub struct PublishReceipt { pub subject: Subject, pub payload_len: usize,
                            pub ack_kind: AckKind, pub delivery_class: DeliveryClass }

#[must_use = "a PublishPermit must be sent or explicitly aborted"]
pub struct PublishPermit<'ledger> {
```

**Lead corrected.** The seed placed all four types in `fabric.rs`; only two are there. `AckKind`
(`Accepted`, `Committed`, `Recoverable`, `Served`, `Received`) and `DeliveryClass` live at
`class.rs:83` and `:17`. The seeded `#[must_use]` on `cost_vector` verified exactly at `class.rs:43`,
and it is one of a pair — `minimum_ack` carries it at `:56`.

**Verdict: ADAPT**, not ADOPT. `crates/omp-types/Cargo.toml:10-18` records why in our own tree:
`messaging-fabric` transitively requires `test-internals`, which upstream issue #46 removed from
defaults. **Prior art existing upstream is not prior art we can call.** Adopt the shape — a receipt
carrying an ack boundary, a permit that cannot be dropped silently — and define it locally.

---

## Gap 2 — An allowance list that outlives the defect it records

**Gap.** We need to declare "this lane exists but nothing runs it" without the declaration becoming
permanent.

**Search.** `grep -n 'UNWIRED_LANE_ALLOWANCE' franken_lean/crates/fln-conformance/tests/contract_roots.rs`

**Found.** `contract_roots.rs:284-288`, verbatim — and the list is **empty**, so every lane is wired:

```rust
/// Checked in BOTH directions by
/// [`the_lane_this_suite_delegates_to_is_present_and_invoked`]: an undeclared unwired
/// lane fails, and a declared lane that has since been wired ALSO fails. So the
/// allowance shrinks as lanes land and cannot quietly outlive the defect it records.
const UNWIRED_LANE_ALLOWANCE: &[(&str, &str)] = &[];
```

The seeded test name verified exactly at `:775-777` — *"Both failure directions fire, and both passing
directions stay quiet"*, `fn allowance_verdict_fails_in_both_directions()`. The refusal text at
`:757-761` is the idea in one sentence: *"an allowance that outlives its defect is how a repaired gap
keeps reading as broken, and it is what stops this list from shrinking."*

**Verdict: ADOPT verbatim, as the shape of every allowance list here.** We already have one needing
it: `crates/omp-inventory-map/src/types_inventory.rs:176-178` excludes `Observation` from an allowance
list, with no dual-direction test today.

---

## Gap 3 — A binary that cannot say which source built it

**Gap.** An installed binary cannot prove it matches the tree it claims, so install drift is invisible.

**Search.** `grep -rl vergen --include=Cargo.toml .`, then
`grep -rn 'binary_identity|build_id|running_binary' --include=*.rs .` (both re-confirmed with the
harness grep; the corpus is Rust, so the filter was correct here).

**Found — the seeded verdict was WRONG.** The seed said `no prior art found`. **18 of 210 repos** build
identity in via `vergen`. `beads_rust/build.rs:41-45` emits the drift signal specifically:

```rust
if let Some(status) = git_output(&["status", "--porcelain"]) {
    emit_env("VERGEN_GIT_DIRTY", if status.is_empty() { "false" } else { "true" });
}
```

The strongest statement is `frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846`:

```rust
/// Fails closed: a gate that cannot name the exact binary it measured is not
/// admissible evidence, so an unresolvable path or any read error panics
/// rather than degrading to an unidentified run.
fn running_binary_identity() -> (PathBuf, String) {
```

**Two honest negatives inside the positive.** The best identity string in the toolset is `fh`'s —
`franken-harvest 0.1.0+tree.<64-hex>.src.<40-hex>` — but its source is **not in the mirror**
(`ls -1 | grep -i harv` → no match), so it is a measured artifact, not mirror prior art. And
`beads_rust` embeds identity without exposing it: `br --version` prints `br 0.4.1` and nothing else;
the SHA is read only at `src/cli/commands/version.rs:55`. Embedding and exposing are two decisions.

**Verdict: ADOPT** the tree-digest-plus-dirty-flag shape with the fail-closed rule quoted above.

---

## Gap 4 — The canonical doctor shape

**Gap.** We have no `doctor`, and what we do have is undiscoverable — `omp-inventory-map --help`
returns `CONFIG_ERROR unknown argument --help` (brief §3.6).

**Search.** `grep -rn 'DoctorExitCode' beads_rust/src`; `grep -n 'Commands::Doctor' beads_rust/src/main.rs`;
`find pi_agent_rust -name doctor.rs`.

**Found.** The richest vein in the mirror; it feeds `07-installability.md` directly.

**(a) A typed exit-code dictionary — lead right, badly incomplete.** The seed named four variants.
`beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:51-74` declares **eleven**:

```rust
/// Numeric values are stable contract; do **not** change them. Adding
/// new variants is fine if a fresh number is chosen — agent scripts
/// that mask `match c { 0 => .., _ => bail }` cope safely.
#[repr(i32)]
pub enum DoctorExitCode {
    Healthy = 0,        FindingsPresent = 1,  FixPartial = 2,  FixFailedRolledBack = 3,
    RefusedUnsafe = 4,  ConcurrencyLost = 5,  OnlineRequired = 6,
    UsageError = 64,    NoInput = 66,         CannotCreateOutput = 73,  IoError = 74,
}
```

The four seeded values confirm exactly. What the seed missed is the **two-band design**: 0–6 are
doctor-domain verdicts, 64/66/73/74 are `<sysexits.h>`, so a caller that knows nothing about doctors
still gets meaning. Note `FixFailedRolledBack = 3` — *"the run was rolled back from the verbatim
backup. Workspace state is unchanged"* (`:21-24`). A repair that fails cleanly is a distinct verdict
from one that half-succeeded.

**(b) The doctor publishes its own contract.** `doctor_subsystems/capabilities_doctor.rs:1-15`:

```rust
//! `br.doctor.capabilities.v1` — machine-readable doctor contract.
//! - `exit_codes` — derived from [`super::exit_codes::DoctorExitCode::all`]
//! - `write_scopes` — `.beads/`, `.doctor/`
//! - `fixers` — currently wired repair/refuse paths
//! - `detectors` — currently wired flat-doctor check IDs
//! Stability: the JSON shape is stable contract. New fields are
//! purely additive; agents must tolerate unknown keys.
```

This answers §3.6's ADDRESSABLE property: the binary enumerates its own detectors and fixers, so a
surface cannot be wired-but-unaddressable. The exit-code list is **derived** from the enum, not
transcribed — that drift is structurally impossible.

**(c) An error naming its own repair command.** `eidetic_engine_cli/src/cache/hotset.rs:1504` (seeded
lead, verbatim), and not a one-off — the same file repeats it at `:1519`:

```rust
"repair": "Run `ee doctor --workspace . --json` if the store schema looks incomplete.",
```

**(d) Doctor scope declared in prose.** `pi_agent_rust/src/doctor.rs:1-5`, seeded lead verbatim:
*"When invoked without a path, checks config, directories, auth, shell tools, and sessions… With
`--fix`, automatically repairs safe issues (missing dirs, permissions)."*

**(e) `Doctor` is exempted from the preconditions every other command must satisfy.**
`beads_rust/src/main.rs:104` and `:297`, both `&& !matches!(cli.command, Commands::Doctor(_))`. This is
the sharpest single idea in the vein: **the tool you run when the workspace is broken must not require
the workspace to be intact.** A doctor gated behind the checks it exists to diagnose is not a doctor.

**Verdict: ADOPT, as the spine of `07-installability.md`** — a `#[repr(i32)]` two-band exit enum; a
`capabilities.v1` document deriving its own codes and naming write scopes, detectors and fixers; every
error carrying a runnable `repair` string; and `doctor` exempted from preflight.

---

## Gap 5 — Mutation through a real hook, not a fixture

**Gap.** Our gate suites mutate fixtures. A fixture cannot tell us the *installed* hook still refuses.

**Search.** `grep -rln 'hooks/pre-commit' --include=*.rs .` plus the harness grep over `beads_rust`,
`franken_lean`, `destructive_command_guard` (no extension filter, so the shell scripts were in scope —
which is where the answer was).

**Found — the seeded verdict was WRONG.** `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362`
copies the **real** hook into a lab repo:

```python
for source, dest in ((evidence, "scripts/evidence.py"), (hook, "scripts/git-hooks/pre-commit")):
    (repo / dest).write_bytes(source.read_bytes())
os.chmod(repo / "scripts/git-hooks/pre-commit", 0o755)
```

`scripts/git-hooks/test_projection_guard.sh` then drives real `git commit` against it — including case
8 at `:202-212`, asserting the guard **chains** to a pre-existing `.git/hooks/pre-commit` rather than
shadowing it. The reason is stated exactly at `ci/VERIFICATION_MANIFEST.jsonl:93`:

> *"CELL C IS WHAT MAKES CELL B MEAN ANYTHING: a successful hook prints NOTHING, so a green commit and
> a hook that never ran are indistinguishable from outside. C re-plants the empty row in B's OWN
> repository after B's success and requires a refusal…"*

Fixture *size* is asserted because the defect is a race, not a threshold (`test_projection_guard.sh:520-524`):
the broken form refuses 5% of the time at 50 627 B, 92% at 72 725 B, 100% only from ~98 KB.

**The counter-example is instructive.** `asupersync/src/subsystem_mutation_testing.rs:9` is gated
`#![cfg(all(test, feature = "real-service-e2e"))]` but builds a `LabRuntime` over a `TempDir` — a
fixture. Both patterns exist; only franken_lean's reaches the installed artifact.

**Verdict: ADOPT.** The rule to carry: *a successful gate prints nothing, so a green run and a gate
that never ran are indistinguishable* — therefore every gate needs a planted-defect cell that
**requires** a refusal.

---

## Gap 6 — Refusing an empty scan set

**Gap.** Brief §3.3: all 183 census rows carry exactly **1 distinct** `must_be_true`. Our own inventory
satisfies the four-field discipline vacuously. Did Jeffrey solve this?

**Search — hard, because a false absence here would be expensive.** Patterns across all 210
work-trees: `vacuit`, `vacuous`, `anti.vacuity`, `scanned zero`, `empty scan set`,
`no files were scanned`, `scan set is empty`, `vacuous(ly) (pass|green|true)`, `would pass vacuously`,
`empty (input|scan|corpus|candidate) set`, `zero (files|candidates) (scanned|examined)`.

```
grep -rlEi 'vacuit'       --include=*.rs .  ->  236 files
grep -rlEi 'vacuous'      --include=*.rs .  ->  838 files
grep -rlEi 'anti.vacuity' --include=*.rs .  ->   63 files
```

**Found — the seeded verdict was WRONG, and it was the one flagged CRITICAL.** Anti-vacuity is a
pervasive *named* discipline: 63 files carry the literal term, across `frankenmermaid` (51),
`franken_lean` (50), `frankensim` (35), `frankengit` (21) and eleven more repos.

The floor as a rule — `franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115`:

```rust
// ANTI-VACUITY FLOOR. An empty or implausibly small scan is a BROKEN SCAN, not a clean
// tree — the failure this repository has recorded repeatedly, most sharply when a
// derived scope returned zero and read as "nothing to report". Without this, deleting
// `.github/workflows/` entirely would make every assertion below pass vacuously.
assert!(workflows.len() >= 2, "…a scan this small is broken, not clean.", …);
```

The one-liner — `franken_lean/tribunal/epoch-lab/tests/derived_input_provenance.rs:538`:

```rust
assert!(p.item_count > 0, "{} scanned nothing", p.rule);
```

And a first-class verdict *name* — `crates/fln-conformance/tests/build_gate_governed_sets.rs:541-552`,
asserted at `:613`: *"a repaired population's live guard is unkillable unless it refuses its own
emptiness… its absence is a VACUOUS PASS and not a clean one: reinstate the clause, or retire this
check deliberately rather than by rewording."*

**Verdict: ADOPT — the highest-priority adoption in this file.** Take all three strengths: a
**cardinality floor** (`>= 2`, not `> 0`, because *implausibly small* is also broken), a **per-rule
`item_count > 0`**, and **`VACUOUS PASS` as a named verdict** distinct from failure. Applied to §3.3,
the census would refuse itself today: 183 rows over 1 distinct `must_be_true` covers nothing.

---

## Gap 7 — A worker that cannot say it is done

**Gap.** Brief §4, the `complete` row: *"worker says done — DOES NOT EXIST — every completion this
session was found by a human looking."* Our largest architectural hole.

**Search.** `grep 'pub enum Outcome' asupersync/src`; then
`grep 'pub enum (ChildExit|ExitReason|ChildOutcome|SupervisionEvent|ChildStatus)'` over
`supervision.rs`, `gen_server.rs`, `spork.rs`.

**Found, half.** `asupersync/src/types/outcome.rs:213-227` — seeded variants verified exactly, and the
lattice is the part worth having:

```rust
/// Forms a severity lattice where worse outcomes dominate:
/// `Ok < Err < Cancelled < Panicked`
pub enum Outcome<T, E> { Ok(T), Err(E), Cancelled(CancelReason), Panicked(PanicPayload) }
```

`Cancelled` and `Panicked` are not collapsed into `Err`. A timeout is not a verdict and a panic is not
an error — our async contract (brief §3.7), already stated as a type.

**Not found, and this is the finding.** `asupersync/src/supervision.rs:3122-3206` declares
`SupervisionEvent` with **eight variants**: `ActorFailed`, `DecisionMade`, `RestartBeginning`,
`RestartComplete`, `RestartFailed`, `BudgetExhausted`, `Escalating`, `BudgetRefusedRestart`.

**Every one is failure, decision, restart or escalation. There is no completion variant.** The nearest,
`RestartComplete`, reports a *restart* finishing — not work. `StopReason` (`:3098-3116`) is the same
shape across six: `ExplicitStop`, `RestartBudgetExhausted`, `BudgetRefused`, `Cancelled`, `Panicked`,
`RegionClosing`. None means *"the worker finished what it was asked to do"*; `ExplicitStop` is the
supervisor stopping the child, the opposite direction of travel.

**So the supervisor learns that a child DIED, never that it FINISHED** — precisely our gap, and across
210 work-trees no supervision protocol closes it.

One adjacent mechanism is worth taking. `supervision.rs:3208-3213` opens an **Evidence Ledger**:
*"Structured, deterministic, test-assertable record of why each supervision decision was made. Every
call to `Supervisor::on_failure_with_budget` appends exactly one `EvidenceEntry` whose
`binding_constraint` field…"* — one entry per decision, naming the constraint that bound.

**Verdict: ADAPT, and own the remainder.** Adopt `Outcome<T,E>` with its lattice (already in
`omp-types`, brief §3.7) and the one-entry-per-decision ledger. **Design** the completion signal
ourselves: a `WorkerReport` carrying `Outcome`, the claim it discharges, and the evidence discharging
it — pushed by the worker, not polled by a human. **This is the one place in this document where the
mirror gives us a lattice and a ledger but no protocol.**

---

## Gap 8 — Per-adapter scoping and typed missing dependencies

**Gap.** A CLI run inside someone else's repo must scope what it touches, and degrade per adapter when
a dependency is absent.

**Search, first pass — wrong.** `grep '(adapter|Adapter)\w*(registry|Registry|scope|Scope)|per-adapter|upstream_report'`
over `beads_rust/src`, `eidetic_engine_cli/src` → no matches. **That space was Rust-only** — hazard 2
from §0, committed here. **Second pass:** the same question asked of `ntm` with no extension filter
(`grep -rn 'ErrNotInstalled|DEPENDENCY_MISSING' ntm`), because ntm is the wrapped binary written in Go
and the one that actually shells out to foreign dependencies.

**First pass still yielded something.** `asupersync/src/adapter_certification.rs:1-6` — *"the
source-owned declaration surface that keeps adapter identities and fail-closed status from drifting
into hand-maintained prose"* — with `AdapterCategory` (`:10`), `AdapterCertificationStatus` (`:39`,
`CertifiedLive` = *"live implementation and reference coverage are wired"*), `AdapterRenderedStatus`
(`:65`, `Pass` vs an *"expected fail-closed status"*) and `AdapterCertificationDeclaration
{ adapter_id, category, … }` (`:88`).

**The second pass is the better half.** `ntm` ships the whole typed missing-dependency vocabulary at
four layers. A **per-dependency typed sentinel**, one per adapter — `internal/bv/bv.go:31`, identically
at `internal/cass/client.go:13` and `internal/caut/client.go:14`:

```go
var ErrNotInstalled = errors.New("bv is not installed")
```

A **shared wire taxonomy** — `docs/robot-action-handoff-contract.md:379`:
`ErrCodeDependencyMissing = "DEPENDENCY_MISSING"`.

The **remediation carried inside the envelope**, not printed beside it — `internal/cli/bugs.go:85-89`:

```go
response := robot.NewErrorResponse(cause, robot.ErrCodeDependencyMissing,
    "Install UBS from https://github.com/nightowlai/ubs, then rerun 'ntm bugs list --json'")
```

An **explicit per-call-site degradation policy** — `internal/alerts/generator.go:383-385`. The same
missing binary is fatal in `bugs.go` and silent here, and the difference is a *decision*:

```go
// Silently skip when bv is not installed; only warn on real errors.
if !errors.Is(err, bv.ErrNotInstalled) && !strings.Contains(err.Error(), "executable file not found") {
```

And a **conformance test pinning the exit code**, which makes the rest binding —
`internal/cli/robot_registry_conformance_test.go:15-19`. Read it closely: the envelope *asks for* exit
2 via `WithExitCode(2)` and the contract *gives it 1*, because 2 is reserved for `NOT_IMPLEMENTED`.

```go
func TestRobotProcessExitContractReservesUnavailableForNotImplemented(t *testing.T) {
	dependency.Meta = robot.NewResponseMeta("robot-test").WithExitCode(2)
	if got := ExitCode(robot.ExitResultForResponse(dependency, …)); got != 1 {
```

A missing dependency and an unimplemented action may not collide, and a response's declared exit code
does not override the taxonomy. **Three of these five citations reached this section second-hand and
carried off-by-one errors** — the sentinels sit one line below the comment cited (`bv.go` 30→31,
`cass` 12→13, `caut` 13→14) and the degradation comment is at `generator.go:383`, not `:384`. Quotes
otherwise exact; line numbers above are corrected.

**Verdict: ADOPT** — upgraded from ADAPT on the second pass. Compose ntm's four pieces with
`adapter_certification.rs`'s fail-closed per-adapter status and `capabilities_doctor.rs`'s declared
`write_scopes` (enforced by `DoctorExitCode::RefusedUnsafe = 4`, *"write outside safety_envelope §2
scopes"*), and the foreign-repo story is complete enough to build without inventing anything.

---

## Gap 9 — Probing tool presence without trusting exit codes

**Gap.** We wrap nine binaries and no single version flag covers them (`--version` answers 8/9, `-V`
answers 5/9), so a naive probe records a present binary as absent.

**Search.** `sed -n '920,1000p' pi_agent_rust/src/doctor.rs` (seeded lead, ~944) and
`grep -n 'fn probe_failure_is_known_nonfatal' -A 30`.

**Found.** Lead verified. `pi_agent_rust/src/doctor.rs:918-924` splits the question in two —
`ToolCheckMode::{PresenceOnly, ProbeExecution}`, then `fn check_tool(…)`. **Does it trust exit status?
Yes** — the success arm is `Ok(output) if output.status.success()`, taking the first stdout line as the
version — which is why it needs an escape hatch, at `:1052-1064`:

```rust
fn probe_failure_is_known_nonfatal(tool: &str, args: &[&str], output: &std::process::Output) -> bool {
    if tool.ne("sh") || args.ne(&["--version"]) { return false; }
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    stderr.contains("illegal option") || stderr.contains("unknown option")
        || stderr.contains("invalid option")
}
```

**That allowance is hard-coded to `sh`.** `tmux` falls straight through to `"{tool}: invocation
failed"` — recorded broken while present and working.

**A brief fact does not survive verification, and the correction inverts it.** Brief §3.1 states
`tmux --version` *"prints an error AND EXITS 0 — it fails while reporting success."* Measured four ways
on `/opt/homebrew/bin/tmux`:

```
tmux --version                          -> exit 1, banner on STDERR, stdout EMPTY (0 bytes)
env -i /opt/homebrew/bin/tmux --version -> exit 1
v=$(tmux --version 2>&1)                -> exit 1
tmux --version 2>&1 | head -1           -> exit 0     <-- source of the claim; PIPESTATUS=(1 0)
tmux -V                                 -> exit 0, "tmux 3.6a"
```

**tmux is honest.** It fails, says so on stderr, and returns 1. The `exit 0` came from a pipeline,
whose status is `head`'s. The exit code that lied belonged to *the measurement harness* — the same
defect family as the brief's `installer` example, relocated. Two consequences: the stated attack
("captures a usage banner as the version string") cannot occur, since stdout is empty; and the real
hazard runs the other way — **a probe treating non-zero as ABSENT records tmux, present at 3.6a, as
MISSING**, a false negative on the one binary through which we read pane truth. That is exactly what
`check_tool` would do.

**Verdict: ADAPT.** Take the two-mode split and the named-nonfatal allowance; reject the hard-coded
`tool.ne("sh")` for a per-binary flag table. Presence must be decided by `which`, never by a version
probe's exit status — and when a probe does conclude absence, it must say so in Gap 8's
`DEPENDENCY_MISSING` vocabulary rather than inventing a ninth spelling of "not there."

---

## Summary

| # | gap | verdict | citation, or not-found |
|---|---|---|---|
| 1 | delivery receipts | **ADAPT** | `asupersync/…/fabric.rs:1911,1944`; `class.rs:43,83` — unreachable at our pinned rev |
| 2 | unwired-lane allowance | **ADOPT** | `franken_lean/…/contract_roots.rs:288`, test at `:777` |
| 3 | binary identity / install drift | **ADOPT** | `frankensqlite/…/bd_wsw3p_concurrent_write_showcase.rs:846`; `beads_rust/build.rs:41`; 18/210 repos |
| 4 | canonical doctor shape | **ADOPT** | `beads_rust/…/exit_codes.rs:51`; `capabilities_doctor.rs:1`; `main.rs:104`; `hotset.rs:1504`; `pi_agent_rust/src/doctor.rs:1` |
| 5 | mutation through a real hook | **ADOPT** | `franken_lean/…/evidence_finalization.rs:360`; `test_projection_guard.sh:202`; `VERIFICATION_MANIFEST.jsonl:93` |
| 6 | anti-vacuity / empty scan set | **ADOPT** | `marrow_sanitizer_dispatch.rs:105`; `derived_input_provenance.rs:538`; `build_gate_governed_sets.rs:549` |
| 7 | typed worker→supervisor done signal | **ADAPT** | `types/outcome.rs:218` exists; `supervision.rs:3122` — **8 variants, none a completion** |
| 8 | per-adapter scoping / typed missing-dependency | **ADOPT** | `ntm/internal/bv/bv.go:31`; `robot-action-handoff-contract.md:379`; `bugs.go:85-89`; `generator.go:383`; `robot_registry_conformance_test.go:15`; `adapter_certification.rs:1,39,65,88` |
| 9 | tool probe not trusting exit codes | **ADAPT** | `pi_agent_rust/src/doctor.rs:922,1052` — allowance hard-coded to `sh`; brief §3.1 tmux measurement refuted |

**Seeded-verdict correction rate: 5 of 9, and Gap 8 was corrected twice.** Gaps 3, 5, 6 and 8 were
seeded `no prior art found`; all four have prior art, Gap 6 emphatically, and it was the one flagged
CRITICAL. Gap 1 was seeded ADOPT and is ADAPT. Gap 9's supporting measurement was inverted, and the
refutation propagated back into the brief. Gap 8 moved `not-found → ADAPT → ADOPT` across two passes,
the second only because a sibling named the language-filter mechanism behind the first zero.

**Four false absences and two inverted findings in nine rows is the strongest argument in this file for
the writing contract's rule 7** — and the failure has a shape, because it recurred across three
independent authors. Every instance was caught by someone **re-deriving**, never by someone
**reading**. Two mechanisms produced identical-looking zeros: `--include=` returning empty at exit 0,
and an extension filter aimed at another language. Neither is distinguishable from genuine absence by
inspecting output. **So a not-found must name its search space, not just its pattern** — the
operational upgrade this section makes to rule 7, and the only reason Gap 8 carries its best citation.

### What has no precedent, and what that means

**No whole gap is precedent-free.** Every seeded absence dissolved under a second search. One specific
shape genuinely is:

> **A typed success-completion event in a supervision protocol.** Across 210 work-trees, supervision
> vocabularies enumerate failure, cancellation, panic, restart, budget exhaustion and escalation. None
> enumerates *finished*.

That is the centre of Gap 7 and of brief §4's dead `complete` row. It cuts both ways and the plan
should say both. **The risk:** we would design the worker→conductor completion protocol with no
in-house precedent and no reference to diff against — in a repo whose own census (§3.3) proves we can
satisfy a discipline vacuously. **Why it is worth building:** the absence is not an oversight across
eighteen mature repositories. Erlang-lineage supervision is a *failure* protocol by construction — a
supervisor's job is restarting what died, and a child that finishes merely stops. Our conductor is not
a supervisor; it is a work dispatcher, and a dispatcher that cannot observe completion cannot
dispatch. The missing piece is missing because nobody in the lineage needed it, and it is load-bearing
for us exactly there. That is the shape of something worth building rather than importing.

The mitigation is named in Gap 7: build on the two mechanisms the mirror *does* supply — the `Outcome`
severity lattice, so a cancelled worker is never laundered into a failed one, and the
one-entry-per-decision evidence ledger, so a completion claim arrives with the evidence discharging
it. New protocol, borrowed vocabulary, borrowed audit trail.

---

**NO-CLAIM.** This section establishes that a pattern exists in the mirror at the cited line, and
nothing more. It does not establish that a cited pattern is correct, is the best available solution,
will transfer here, or will close the gap it is cited against — an ADOPT verdict is a decision to try,
not evidence of a result. It does not claim the searches were exhaustive: they covered 210 git
work-trees at one filesystem snapshot, using the patterns named per gap, with `fh` refusing in both
surfaces so no semantic index assisted them; a pattern in vocabulary none of those greps used would
have been missed, and four such misses are documented above as evidence that this is a live failure
mode, not a hypothetical one. It does not claim the mirror is the whole of the prior art, nor that
absence here implies absence anywhere. The tmux correction in Gap 9 is measured on this workstation
only, against `/opt/homebrew/bin/tmux` 3.6a; it refutes one sentence of brief §3.1 and does not
disturb the flag-coverage table beside it. **No zero in this file survives from a single search** —
each was re-derived against both known false-zero mechanisms, and the Gap 8 citations were re-opened
line by line, which is how three off-by-one numbers were found. That re-verification is itself
unaudited: no one has re-derived these searches independently of this author, and on this session's
measured record that is precisely the condition under which a false absence survives.
