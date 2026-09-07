# `2sx1` grade — the finding kernel's operator surface

Grader: pane `%19` (PearlGate, claude). Filer: `%6`. Implementer: `%20`. I am a non-author of both
the bead and the code. Date 2026-09-07. **Every figure below was re-executed by me on the Contabo
lane; zero local builds.**

## VERDICT: APPROVED — all six acceptance items met, both mutations attributable, restore exact

## Tree — which tree every number comes from

```
c79524e97d9b9caa397de34bd1088d390ddb55ed        IS HEAD
git log c79524e..HEAD -- crates/finding          0 commits
git show --name-only c79524e                     Cargo.toml, src/main.rs, tests/operator_surface.rs
git status --porcelain -- src/main.rs            (clean)
shasum -a 256 src/main.rs                        08ba1dc96f45846447bb4da3adcc56880ab48bc5adda9829656badbb84756406
```

**The three subject files are clean at HEAD**, so `cargo` read the same tree the sha names. That
matters here because two OTHER files in the crate are dirty — see the `br_publisher` section.

## Acceptance, item by item, re-run not read

|item|verdict|evidence I produced|
|---|---|---|
|1 bin target|**PASS** (measurable half)|`cargo metadata` → `finding` targets = `[lib, BIN, br_publisher, finding_contract, operator_surface]`; bin count **1** ≥ 1. Derived from the resolver, not from manifest text|
|1 `command -v`|**DEFERRED-BY-RULING**|no install authorised; see "the acceptance is unsatisfiable as written" below|
|2 subcommands self-describing|**PASS**|`the_two_named_subcommands_are_reachable_and_self_describing` green|
|3 fires-on-known-bad, message|**PASS**|`an_incomplete_finding_is_refused_and_the_message_names_the_missing_field` green; M1 proves the message assertion is NOT redundant with the code assertion|
|4 known-good leg|**PASS**|`a_well_formed_finding_files_and_drains_the_pending_sweep` green; `/bin/echo` injected via `--br` at the process boundary|
|5 anti-vacuity|**PASS**|`an_unreadable_spool_is_never_silently_an_empty_sweep` green; M2 attributable|
|6 name what stays locked|**PASS**|commit body splits 9 lib-only crates into 5 libraries-by-design and 4 ACTION kernels still unreachable|

Baseline, my own run: `cargo test -j 2 -p finding --test operator_surface` — **exit=0, 6 passed /
0 failed, contabo-4**, `manifest_err=0`. Reproduces `%20`'s reported figure exactly.

## Both mutations re-run by me, and both are attributable

**M1 — collapse every cause to one exit code.** `fail()` returns `EXIT_MISSING_FIELD` regardless of
the matched variant.

```
exit=101  contabo-1   4 passed... no: 5 passed; 1 failed
RED    a_dead_publisher_is_a_distinct_cause_from_an_incomplete_finding
green  the other five
```

Its message is the load-bearing part:

```
assertion `left == right` failed: a publisher returning no id must exit 5, not the
missing-field code: FINDING_PUBLISH_FAILED detail=br create returned success without an id
  left: Some(3)   right: Some(5)
```

**The TOKEN stayed correct while the CODE collapsed.** That is a sharper result than "the mutation
bit", and it corrects a possible misreading of `AGENTS.md` rule 7. Rule 7 says a known-bad leg must
assert its MESSAGE, not just its exit code — read as *"message instead of code"*, this leg would
have stayed GREEN under M1, because `FINDING_PUBLISH_FAILED` was still printed. **The defensible
form is BOTH**, and this leg pins both. M1 is the evidence that the pairing is not belt-and-braces.

**M2 — report an unreadable spool as an empty sweep.** `report_pending`'s `Err` arm prints
`FINDING_PENDING_EMPTY` and returns SUCCESS.

```
exit=101  contabo-4   4 passed; 2 failed
RED    an_unreadable_spool_is_never_silently_an_empty_sweep
       "an absent dir must be UNREADABLE, not empty"
RED    a_well_formed_finding_files_and_drains_the_pending_sweep
green  the other four
```

Two legs, exactly as reported, and the second red is correct rather than collateral: the known-good
leg asserts `pending()` reports EMPTY *after* a successful file, so a mutation that makes EMPTY
unconditional destroys that leg's discriminating power too. **A mutation that reddens the
anti-vacuity leg AND the known-good leg is showing they share a discriminator** — which is why item
5 could not be satisfied by the known-good leg alone.

**Restore verified twice over:** `08ba1dc96f45846447bb4da3adcc56880ab48bc5adda9829656badbb84756406`
byte-identical to `%20`'s reported sha, `git status` clean against HEAD, and the suite back to
**exit=0, 6 passed** on contabo-4.

## `br_publisher` — UNMEASURABLE ON LANE, confirmed, and OUT OF SCOPE

My own run: **exit=101, 0 passed / 2 failed**, both legs identical:

```
crates/finding/tests/br_publisher.rs:42:35  br must run: Process(NotFound("br"))
```

`NotFound` is exec-level absence — the binary does not exist on `PATH`. That is not a nonzero exit
from a working `br`; it is the environment being unable to answer. `AGENTS.md` records that Contabo
workers have no `br`. **Classification confirmed: UNMEASURABLE, not failing.** `%20` diagnosed it
and correctly did not make it pass.

**And it is out of `2sx1`'s scope by measurement, not by assertion:** `c79524e` touched three files
and this is not one of them; `br_publisher.rs` was added by `ee6989e`. Its failure cannot be a gap
in this bead.

**Citation note.** `%6` cited `br_publisher.rs:42`, which is correct for the **worktree** — the file
is ` M` with a single blank-line deletion, so HEAD's `.expect("br must run")` sits at `:43`. The
`Command::new("br")` call is HEAD's own code either way, so the substance holds; the line number is
worktree-derived. Second time tonight a line citation differed between the two trees by one.

## RESIDUAL FINDING, not `%20`'s and not in scope: the test conflates absence with failure

`.expect("br must run")` **panics** on an absent binary, so `br_publisher` reports **FAILED** where
the honest verdict is **UNMEASURED**. The distinction survives only in the message, and the *verdict*
— which is what a grader or a CI gate reads — cannot tell "no `br` on this worker" from "`br` is
present and rejected the row."

**This is the same defect class I fixed earlier tonight** in
`crates/pre-delete-citation-check/tests/mirror_oracle.rs`, where an absent `.beads` mirror on
contabo-4 read as a failing oracle until a positive discriminator was added so absence reports
UNMEASURED. Two independent instances in one session, both caused by worker environments lacking
repo-local tooling and data. Recommend a separate bead; I did not edit the file.

## The `Cx` substitute is SOUND, and it was NECESSARY rather than stylistic

`main.rs:133-141` uses `RuntimeBuilder::current_thread()` + `request_cx_with_budget(Budget::INFINITE)`
+ `block_on`, which is the shape of the sanctioned exemplar at
`crates/pane-dispatch-fence/src/main.rs:193-201`.

Verified at the **pinned** rev `fa3c01a` from the local checkout, not from the bead's assertion:

```
asupersync/src/cx/cx.rs:5037   #[cfg(any(test, feature = "test-internals"))]
asupersync/src/cx/cx.rs:5039   pub fn for_request() -> Self
asupersync/src/cx/cx.rs:5026   #[cfg(any(test, feature = "test-internals"))]
asupersync/src/cx/cx.rs:5028   pub fn for_request_with_budget(budget: Budget) -> Self
```

`Cx::for_request` is genuinely unavailable to a production bin at our pinned rev — the same gating
class `jix1` proved for `messaging-fabric`'s `new_ephemeral` at `id.rs:139`/`:322`. Using it would
have been a compile failure, not a lint.

**One doctrine residual, NOT a gap in this bead:** `Budget::INFINITE` is an unbounded budget, which
sits oddly beside the asupersync contract's *"no wait in the adapter is unbounded, including
shutdown."* The exemplar makes the same choice, so this is a question about the exemplar and the
contract, for `%6` — not something to hold `2sx1` on. Flagging rather than silently accepting it.

## THE ACCEPTANCE IS UNSATISFIABLE AS WRITTEN — recommend amending item 1

`%6` invited this explicitly, and it is the `qfw0` "joined" lesson pointed at `%6`'s own bead.

Item 1 reads: *"`cargo metadata --no-deps` reports a `bin` target for `finding`, **and `command -v`
resolves it after install**."* The second clause is **DEFERRED-BY-RULING** — the same ruling forbids
the install that would make it true. So item 1 as written can never be marked satisfied, and a
future grader reading only the acceptance field will score it as an open gap. That is the failure
mode this repo has already paid for: an acceptance demanding an unreachable artifact gets routed
around, or worse, re-litigated.

**Recommend splitting item 1 into 1a (bin target declared — MET) and 1b (`command -v` after install
— BLOCKED, no install authorised), so the deferral is a recorded state rather than a permanent
apparent gap.** I did not edit the bead; that is `%6`'s call.

## Lane conditions, for whoever grades next

Six admission refusals across this grade, and **the sub-cause mix changed on every attempt** — the
acceptance's warning that a prior diagnosis does not carry is correct, measured:

```
insufficient_slots=1,active_project_exclusion=2,os_gate_excluded=1
insufficient_slots=2,active_project_exclusion=1,os_gate_excluded=1
critical_pressure=1,insufficient_slots=1,active_project_exclusion=1,os_gate_excluded=1
```

Two measured facts worth carrying:

- **`RCH_VISIBILITY=verbose` does NOT surface the per-worker discriminating sentence on the
  ADMISSION path.** The acceptance instruction to "read which sentence printed" to tell SIZING from
  OCCUPANCY is not satisfiable when admission refuses — verbose only affects the remote-exec phase,
  which never starts. The summary line is all there is.
- **`active_project_exclusion` was PEERS, not me.** `rch queue --json` showed two live
  `omp-orchestrator-38cf50d1` builds on contabo-1 and contabo-2 with heartbeats 0–4 s while I had
  nothing in flight. The correct move was to wait, and waiting worked. Pinning another worker or
  passing a flag would not have.
- One attempt logged `Selected worker:` with **no** `Remote command finished: exit=` line — a
  worker was selected and the run did not complete. That is the mid-path class the `80f8` acceptance
  distinguishes from admission, and it is not fixed by `-j 2` or by waiting.

## NO-CLAIM

- **I did not verify that `br` accepts a real row.** The known-good leg injects `/bin/echo` via
  `--br`, and `%20`'s NO-CLAIM on that is accurate. What the leg proves is stronger than "the
  plumbing runs": `operator_surface.rs:188-192` asserts the echoed id contains `BrPublisher`'s real
  argv, so it pins what WOULD reach `br`. It still does not prove `br` accepts it.
- **A reachable bin is INVOKABLE, not INVOKED.** Nothing here makes an operator use it; the commit
  says so. No wiring proof of a caller is claimed, and none is owed — this bead's product IS the
  operator surface.
- **`finding_contract.rs` is ` M` in the worktree** with a real +4/−1 change of unknown authorship.
  It is not one of the three subject files and I did not run it, so a bare `-p finding` aggregate
  would mix it in. My figures are all NAMED targets, per the `mmt4` ruling.
- **`br_publisher` remains UNMEASURED on the lane**, not passing. I did not attempt to run it
  anywhere `br` exists, so whether those two legs pass at all is unknown to me.
