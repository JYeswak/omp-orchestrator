# finding_contract

Bead: `omp-orchestrator-finding-contract-gbok`

## Purpose

Defines the obligation lifecycle for `crates/finding`: a named gap becomes `Filed` (a bead) or
`Waived` (a written reason), and there is no third exit. It fixes the five types
(`Finding`, `SpooledFinding`, `Filed`, `Waived`, `FindingError`), the spool-then-publish ordering
that makes a cancelled file a DEFERRED finding rather than a lost one, and five laws —
`FC-L1` filed-or-waived, `FC-L2` `Filed` names a bead id, `FC-L3` the spool row outlives the
observer, `FC-L4` a finding reporting an absence names its replacement, `FC-L5` a waiver carries an
expiry. **Three of the five are stated here and NOT enforced by the code**, each named with the
bead that will change it, because a contract that claims a guarantee its type does not carry is
worse than no contract. Documents what IS at the commit below; changes no crate source.

## Contract Artifacts

1. **Canonical artifact:** `crates/finding/src/lib.rs` — the five types and the two disposal
   methods. There is deliberately no `artifacts/finding_v1.json`: the durable artifact of this
   concern is a **spool row on disk** (`finding-<fnv1a64>.pending`, write-then-rename, retired to
   `.filed-<id>`), and its format is asserted by the suite rather than duplicated in a fixture.
2. **Runner:** `cargo test -p finding --test finding_contract`
3. **Invariant suite:** `crates/finding/tests/finding_contract.rs` — 12 legs, and it is **the
   first caller of `Finding::file` in the workspace**. Legs are pinned defects that go RED when a
   law becomes enforced, forcing this document to be updated in the same commit. **One has now
   fired:** `l1_must_use_is_only_a_warning_and_does_not_survive_option` failed with
   `unused_must_use is now denied in 1 manifest(s)` and was REPLACED by
   `l1_unused_must_use_is_denied_and_the_producer_does_not_return_option`, which asserts the same
   two facts in the direction that now holds. A pin left in place after its defect is fixed becomes
   a test that fails forever and gets `#[ignore]`d — which is how a suite goes vacuously green.
   Two pinned defects remain (`FC-L2`, `FC-L5`).

> A contract naming no invariant suite is a DESCRIPTION. Item 3 is what makes the pinned defects
> load-bearing instead of a to-do list.

## 1. The measured state of the crate

Measured 2026-09-01 with `cargo metadata` (authoritative) and literal greps:

| question | answer |
|---|---|
| `Finding::file` call sites, whole workspace including tests | **0** |
| `Publisher` implementors | **0** — the trait had none, so `file()` was unreachable |
| `Finding::waive` call sites outside the crate's own tests | **0** |
| `pending()` production callers (the recovery sweep) | **0** |
| crates depending on `finding` | **2** — `ack-spine`, `finding-dispatch` |
| …of those, crates that IMPORT it | **1** — `finding-dispatch/src/lib.rs:10` |
| `finding-dispatch` dependents | **0** |
| callers of `finding_dispatch::finding_for` | **0** |

**A correction to the standing account.** The repo says "ZERO production callers". That is wrong in
one direction and understated in another: there ARE two dependency edges, and the law-bearing
method had **zero exercises of any kind, including tests**, until this suite landed.
`ack-spine/Cargo.toml:22` declares `finding = { path = "../finding" }` and never imports it — the
edge that made the crate look wired is an unused dep. Owned by
`omp-orchestrator-finding-crate-unrouted-ca9q`.

```bash
# re-derive; do not cite this table without running it
cargo metadata --no-deps --format-version 1 | python3 -c "import json,sys; d=json.load(sys.stdin); print([p['name'] for p in d['packages'] for x in p['dependencies'] if x['name']=='finding'])"
git grep -n --no-index -E '\.file\(|\.waive\(' -- 'crates/*/src/*'   # 0 outside crates/finding
git grep -n --no-index -E 'impl .*Publisher' -- 'crates/*/src/*'     # 0
```

**INSTRUMENT NOTE.** The first pass at this table used `git grep -E 'finding\s*=\s*\{\s*path'` and
answered **0 dependents**, which agreed with the standing account and was wrong. `\s` is not POSIX
ERE, so the pattern could not match; the positive control on the same reader
(`subprocess-contract`, 14 real matches) also answered 0 and exposed it. `cargo metadata` is the
authority for dependency questions; `grep -F` for literal text.

## 2. The types

| ID | type | what it is | invariant at construction |
|---|---|---|---|
| `FC-T-FINDING` | `Finding` | a named gap that must be disposed of | `#[must_use]`; WHAT, WHY, ACCEPTANCE and ≥1 non-blank label all required |
| `FC-T-SPOOLED` | `SpooledFinding` | the durable row; its existence is the recovery guarantee | content-derived name, write-then-rename |
| `FC-T-FILED` | `Filed` | proof the gap became a bead | carries an id — **unvalidated today**, see `FC-L2` |
| `FC-T-WAIVED` | `Waived` | proof the gap was deliberately not filed | a non-blank reason is required |
| `FC-T-ERROR` | `FindingError` | why construction or filing failed | five named variants, no catch-all |

`FindingError::Cancelled { spool_path }` is the one that matters: it is a **DEFERRED** verdict
naming a recoverable row, not a failure. That is "a timeout is not a verdict" applied to
durability.

## 3. The laws

Each law states its enforcement level as measured, and names the test that proves it.

| ID | law | enforced today? | evidence |
|---|---|---|---|
| `FC-L1` | a gap observed is a gap FILED — no path to discard without a `Waived` carrying a reason | **YES for a dropped VALUE** | `unused_must_use = "deny"` at the workspace root; the producer returns `#[must_use] MaybeFinding`, not `Option<Finding>`. Dropping it is a **compile error**. Still not proof a gap was filed — §3.1 |
| `FC-L2` | `Filed` must name the bead id; a Finding that cannot produce one is unconstructible | **NO** | `Filed { id }` takes the publisher's string verbatim, empty included — `finding-l2-unvalidated-id-3j8` |
| `FC-L3` | `SpooledFinding` survives the process that saw it — durable before the observer exits | **YES** | proved by ordering, §3.2 |
| `FC-L4` | a Finding names its REPLACEMENT when it reports something missing | **NO** | no such field exists — `finding-l4-no-replacement-jz2` |
| `FC-L5` | WAIVED IS NOT CLOSED: a waiver carries an expiry or it is a silent permanent exemption | **NO** | `Waived` has `body` + `reason` only — `finding-l5-waiver-no-expiry-jex` |

*Tests:* `crates/finding/tests/finding_contract.rs` — `l1_the_only_disposals_…`,
`l1_must_use_is_only_a_warning_…`, `l2_filed_carries_whatever_…`, `l2_a_real_bead_shaped_id_…`,
`l3_the_durable_row_is_on_disk_before_the_publisher_runs`, `l3_a_publisher_failure_…`,
`l3_respooling_…`, `l3_an_unreadable_spool_dir_…`, `l4_a_finding_cannot_name_a_replacement_today`,
`l5_a_waiver_carries_no_expiry_so_it_is_permanent_today`.

### 3.1 `FC-L1` is now a build error for a dropped value, and still not proof a gap was filed

**CLOSED 2026-09-02 by `omp-orchestrator-finding-l1-bypassable-py3`.** Two measured holes, both
fixed; the honest statement of what is and is not guaranteed follows.

**Hole 1 — `#[must_use]` did not survive the producer's wrapper.** Reproduced on
`rustc 1.100.0-nightly` before changing anything:

```
direct();            -> warning: unused `Finding` that must be used
returns_option();    -> NO WARNING            (fn returns Option<Finding>)
returns_result();    -> warning: unused `Result` that must be used
returns_enum();      -> warning: unused `MaybeFinding` that must be used
```

`#[must_use]` does not propagate through `Option`, and `Option<Finding>` was the signature of the
crate's only producer — so the one mechanism protecting this law was bypassed by the one function
that creates the obligation. `finding_for` now returns
`#[must_use] MaybeFinding { Owed(Finding), NotYet(NotYet) }`. An enum rather than
`Result<Finding, NotYet>`: `Result` is `#[must_use]` and the probe confirms it would work, but "no
finding is owed" is not an error, and putting a healthy fleet in `Err` invites `unwrap()`,
`?`-propagation out of a correct path, and an operator reading `Err` as a failure.

`NotYet` also splits three conditions the old `None` collapsed — `BelowThreshold`,
`AlreadyEmitted`, `NotAFindableDecision` — so a caller logging "nothing to file" can say WHICH
nothing it saw.

**Hole 2 — `unused_must_use` was denied nowhere.** No `[workspace.lints]` table existed and no
manifest set it, against a positive control of 51 manifests carrying `unsafe_code = "forbid"`.
`[workspace.lints.rust] unused_must_use = "deny"` now exists at the root, inherited by
`crates/finding` and `crates/finding-dispatch` via `[lints] workspace = true`.
`unsafe_code = "forbid"` is repeated in that table because a package cannot carry both
`lints.workspace = true` and its own `[lints]` table — inheriting without it would silently RELAX
the strongest lint in the repository while appearing to tighten one.

**Proven to bite.** Planting `finding_for(decision, FINDING_THRESHOLD);` in `finding-dispatch`
produces `error: unused MaybeFinding that must be used` and the crate does not compile. And the
negative direction holds: a caller that disposes — `into_owed()` or `expect_nothing_owed()` —
produces **no** warning, because an over-strict lint that fires on correct code gets `#[allow]`-ed
and dies.

**WHAT IS NOW GUARANTEED, precisely:** in this workspace, a `MaybeFinding` VALUE cannot be dropped
without a compile error. **What is still NOT guaranteed:** `let _ = …` silences it, which is a
deliberate language escape hatch; a downstream consumer outside this workspace can drop it freely;
and **a warning-free build does not prove any gap was filed** — it proves no `Finding` value was
dropped, which is strictly weaker than `FC-L1` as stated. Routing production code through this
crate at all remains the unrouted-crate bead.

### 3.2 `FC-L3` is proved by ordering, not asserted by inspection

`Finding::file` is spool-then-publish:

1. write the durable row — pure, synchronous, no cancellable effect;
2. `cx.checkpoint()` — the ONLY cancellation point;
3. publish;
4. mark the row published only after a confirmed exit.

The suite proves step 1 precedes step 3 by having the test `Publisher` read the spool directory
**at the moment it is invoked** and assert it sees exactly one pending row. Inspection of the
source cannot distinguish the correct order from the wrong one; a publisher that observes the row
can. The mutation leg confirms attribution: swapping publish ahead of spool makes the publisher see
**0** rows and the leg goes RED.

A publisher failure leaves the row `.pending`, so the sweep can finish it — asserted, not implied.

## Validation

```bash
cargo test -p finding --test finding_contract
```

Expect **12 passed**. Three of those legs (`l2_filed_carries_whatever_…`,
`l4_a_finding_cannot_name_a_replacement_today`,
`l5_a_waiver_carries_no_expiry_so_it_is_permanent_today`) are **pinned defects**: they pass because
the law is unenforced, and they go RED the moment it is enforced. Their failure is the signal that
this document must be updated, not that the code broke.

## Cross-References

- `crates/finding/src/lib.rs` — the implementation; `:104` the `#[must_use]`, `:205-225` `file()`,
  `:251-264` the `pending()` sweep with its anti-vacuity rule
- `crates/finding/tests/finding_contract.rs` — the invariant suite
- `crates/finding-dispatch/src/lib.rs:22` — the only producer, and the `Option<Finding>` boundary
  where `FC-L1`'s enforcement stops
- `crates/ack-spine/Cargo.toml:22` — the unused dependency edge
- `crates/no-shell-gate/tests/wired_lanes.rs` — `UNWIRED_LANE_ALLOWANCE`, the measured precedent
  behind `FC-L5`: "an allowance that outlives its reason is worse than no allowance"
- `crates/subprocess-contract/src/lib.rs` — what a `Publisher` must use, both pipes drained
- `docs/error_codes/exit_code_registry.md` — the adjacent concern: how a failure ANNOUNCES itself
- `docs/contracts/asupersync_process_grade.md` — the grade this document is scored by
- `docs/plans/plan_to_write_the_document_corpus.md` — this is document #10 of 78
- `AGENTS.md` §KERNEL-ONLY — the self-indictment this crate exists to answer

## Non-Coverage

- **This contract does not make any caller route through the crate.** `Finding::file` had zero
  callers and `Publisher` zero implementors before this suite; production code still files gaps by
  running `br create` directly. Owned by `omp-orchestrator-finding-crate-unrouted-ca9q`.
- **Three laws are documented as UNENFORCED**, not fixed: `FC-L2` (`-3j8`), `FC-L4` (`-jz2`),
  `FC-L5` (`-jex`). `FC-L1`'s two enforcement holes are `-py3`.
- **No crate source was changed.** The only manifest change is one dev-dependency (`asupersync`,
  same pin) because an integration test cannot see a crate's normal dependencies and `file()` takes
  `&Cx`.
- **Cancellation is not exercised.** `FindingError::Cancelled` is reachable only from a cancelled
  `Cx`, which this suite does not construct; the deferred path is proved via a *failing publisher*
  instead, which exercises the same recovery but not the checkpoint itself.
- **No waiver durability.** `Waived` is an in-memory value nothing writes to disk, so there is no
  waiver sweep to test. Part of `-jex`.
- **The publisher contract is unverified.** `Publisher` implementors MUST use
  `subprocess_contract::run_output`; nothing enforces that, and the suite's test publisher touches
  no subprocess at all.

## NO-CLAIM

**A contract does not wire a crate.** Everything above describes a mechanism with one importing
dependent and zero live callers; the laws are proved for the TYPES, never for consumers, because
there are none.

Three of five laws are **stated and unenforced**. A reader who takes `FC-L2`, `FC-L4` or `FC-L5` as
a guarantee has misread this document — each carries its bead id inline for exactly that reason.
`FC-L1` is a floor-raise: dropping a `Finding` is a warning nothing denies, and it is not even a
warning through the `Option` the only producer returns.

`FC-L3` is the one law with a real proof, and its scope is narrow: it proves the durable write
precedes the publish **in the tested path**. It does not prove the row is ever collected — the
sweep has no operator — nor that the filesystem's rename is atomic on every volume this runs on.
