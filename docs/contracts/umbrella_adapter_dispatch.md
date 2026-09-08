# Umbrella Adapter Dispatch Contract

Bead: `omp-orchestrator-jplf.7.2`

## Purpose

This contract defines the ADDRESSABILITY layer only: how one installed umbrella CLI makes every
workspace binary target reachable by name, what an unknown adapter must do, how the adapter set is
enumerated, and how the adapter roster is derived from a live runner rather than a copied integer.
It deliberately does NOT define doctor semantics (`docs/contracts/s1_l1_doctor.md` owns the probe
verdict algebra) or install semantics (`docs/contracts/s1_l0_install.md` owns artifact verification
and publication). Per `fh C47`, one identity-bearing contract gets exactly one canonical
definition, and the identity this file owns is the invocation shape `<umbrella> <verb> <adapter>`.
It is a readiness boundary and a set of known-bad leg definitions, not an implementation claim; no
adapter dispatch code exists at the revision measured below.

## Contract Artifacts

1. Canonical artifact: **EXISTS as a live command**, `ompo capabilities --json`, envelope
   `omp.umbrella/v1`, carrying `adapters`, `adapter_count`, `verbs`, `probe_ids`,
   `probe_id_count`. It is NOT yet checked in as a golden file; `capabilities_drift_is_red`
   compares the declared list against `cargo metadata` on every run, which is the stronger
   check of the two — a golden pins a snapshot, this pins the live oracle. A checked-in golden
   remains open for CI, where `cargo metadata` may not be available.
2. Smoke runner: **EXISTS** — `ompo capabilities --json`, `ompo help <adapter>`,
   `ompo doctor capabilities --json` (the plan's spelling; one implementation, two spellings),
   and `ompo init [--repo PATH] [--output PATH] [--json]`. `<umbrella>` resolved to `ompo` when
   `UAD-NAME` was ratified 2026-09-07.
3. Invariant suite: **EXISTS** at `crates/ompo-doctor/tests/umbrella_adapter_dispatch.rs`, 8
   tests, plus 10 unit tests in `crates/ompo-doctor/src/umbrella.rs`. Measured
   **18 passed / 0 failed** local darwin arm64. The Validation block below is the cheap static
   gate; the suite is the behavioural one.

## Measured premise — 2026-09-07, HEAD `7b4ac63`, pane %19

Acceptance item 1 of this bead requires the premise be re-measured before any work, because the
bead may be wrong rather than the code. It was, twice, and both defects are in the bead.

| what | measured | how |
|---|---:|---|
| workspace binary targets | **85** | `NUMBERS.toml` `figures.built_binaries`, run verbatim |
| targets reachable as an adapter | **0** | no umbrella verb parses an adapter name |
| adapter-dispatch sites, repo-wide | **0** | `crates/*/src`, matcher positive-controlled inline |
| `capabilities` verbs | **0** | neither umbrella main contains the literal |
| umbrella-shaped bin targets | **2** | `ompo` and `omp-orchestrator` both exist |
| doc files invoking `ompo <verb>` | **23** | `docs/contracts` + `docs/plan` |
| doc files invoking `omp-orchestrator <verb>` | **8** | same scan set |

So the GAP IS REAL and the bead's premise reproduces. Two corrections to the bead itself:

- **The denominator is 85, not 48.** The bead and `docs/plan/07-installability.md:115,124,556` say
  48. `NUMBERS.toml` `figures.built_binaries` is registered `expect = "LIVE"` and answers 85, and
  the plan already names it as the authority. 48 is a copied integer that went stale as the
  workspace grew, exactly as `23` did before it. This contract cites the runner, never a constant
  (`UAD-DENOMINATOR-LIVE`).
- **The bead names the wrong binary.** Its acceptance says `omp-orchestrator help <adapter>`. The
  only umbrella-SHAPED binary that exists is `ompo` (`crates/ompo-doctor/src/main.rs`, 96 LOC main
  + 313 LOC lib), and 23 doc files against 8 invoke that name. `omp-orchestrator` is the resident
  supervisor with four flag-parsed verbs (`run`, `close-readback`, `dispatch render`,
  `grade --claim`; `crates/omp-orchestrator/src/main.rs:677-679`), not an aggregator.

### What `ompo` is today

`crates/ompo-doctor/src/main.rs:21` — `if command.as_deref() != Some("doctor")` — one verb, and it
is addressed by probe FAMILY, not by adapter: `ompo doctor [--repo PATH] [--scope system] [--json]`.
Unknown command already exits 2 with a usage line (`:21-24`), which is the exit-code half of
`UAD-UNKNOWN-ADAPTER` already satisfied for an unknown VERB and absent for an unknown ADAPTER.

## The identity decision this bead must make first — `UAD-NAME`

**Two addressing models and two names are in the tree, and the plan says so.**
`docs/plan/07-installability.md:576`: *"Whether the right shape is one omp-orchestrator aggregator
with adapter subcommands, separate installed binaries, or something else remains open."* Building
against an open shape question is how a wrong thing gets built correctly (`fh C35`: run-and-diff
catches a defect in what you BUILT and cannot catch having built the WRONG THING).

| ID | axis | option A | option B | evidence weight |
|---|---|---|---|---|
| `UAD-NAME` | binary name | `ompo` | `omp-orchestrator` | A: the only aggregator-shaped bin exists, 23 doc files, all five S1 layer contracts. B: 8 doc files, and the name is taken by the resident supervisor |
| `UAD-ADDRESS` | adapter addressing | positional `<adapter>` after the verb | `--scope <family>` | A is what `07-installability` specifies and what 85 named targets need. B is implemented, and families are a COARSER axis — they are complements, not rivals |

**RATIFIED 2026-09-07, PROVISIONAL AND REVERSIBLE** (pane 1, Joshua's override invited).
`UAD-NAME = ompo`: 27 docs already invoke it including all five S1 layer contracts,
`crates/ompo-doctor` is the crate that exists, and `omp-orchestrator` stays the resident
supervisor with four flag-parsed verbs, which must not become an aggregator.
`UAD-ADDRESS = BOTH axes`: `--scope <family>` is already REQUIRED by `s1_l1_doctor.md:90`
`L1-BUILD-SCOPE`, and a positional `<adapter>` does not subsume it — a scope selects a probe
family, an adapter selects a target. Nothing is installed under either name, which is exactly
why deciding now was cheap and why reversal stays cheap.

IMPLEMENTED CONSEQUENCE, so the ratification is not just a note: the roster is generated from
the workspace at build time, and **both spellings of the adapter axis now EXECUTE.**

**SUPERSEDED 2026-09-08 by `c4fb273`, recorded rather than overwritten.** This paragraph read:
*"`ompo doctor --adapter <name>` REFUSES with `UAD_ADAPTER_SCOPED_DOCTOR_UNIMPLEMENTED` rather
than accepting a flag that does nothing — per-adapter probes are the one part of `UAD-ADDRESS`
that is named and not built."* It was true when written and is now false in both halves. A
contract asserting that a shipped capability does not exist licenses rebuilding it, which is
the `ABSENT`-vs-`INERT` misclassification in `AGENTS.md` gate rule 4a aimed at our own doc.

`crates/ompo-doctor/src/adapter_exec.rs` runs the adapter under a bounded 5 s deadline through
`subprocess-contract` (process-group kill, both pipes drained) and returns one of four typed
verdicts. **The exit code is recorded, not believed** — measured across the 38 roster adapters
resolvable on the author's host, `--help` exits `0`, `1`, `2`, `64`, `78` and `255`, and eleven
print a usage line while exiting nonzero, while `loop-queue-filter --help` exits `0` with no
output at all. Keying the verdict on the code is therefore wrong in both directions, so
**output-presence is the discriminator.**

```
ompo doctor --adapter all      executed=88 live=37 degraded=1 unmeasurable=50 exit=1
  UAD_ADAPTER_LIVE            37   spawned and answered
  UAD_ADAPTER_NO_HELP_CONTRACT 1   loop-queue-filter -- exit 0, no output
  UAD_ADAPTER_NOT_INSTALLED   50   absent from PATH: UNMEASURABLE, exit 4, never a failure
  UAD_ADAPTER_TIMED_OUT        0   a timeout is a restrictive terminal, never a pass
```

Every verdict carries the `PATH` resolution actually executed, because a roster name may
resolve to a foreign binary — measured by the executor itself: `installer` resolves to
`/usr/sbin/installer`. It also carries the child's first output line, the field whose absence
made 11 of 12 `CHECK_FAIL` rows uncausable in the 88-crate `gate-runner` roster run.

`UAD_UNKNOWN_ADAPTER reason=absent_from_roster` at exit `2` is the arm this flag never had:
`help` refused an absent name while `doctor --adapter` reported it as merely unimplemented, so
`LAW-UAD-UNKNOWN-IS-TWO` held on one axis and not the other. It now holds on both.

## Stable ID vocabulary

| ID | boundary it refuses |
|---|---|
| `UAD-NAME` | the umbrella's invocation name is defined in more than one place, or in none |
| `UAD-ADDRESS` | the adapter addressing model is ambiguous between positional and scoped forms |
| `UAD-REGISTRY` | an adapter roster is hand-maintained instead of derived from the live target runner |
| `UAD-ADDRESSABLE` | a workspace binary target has no documented invocation through the umbrella |
| `UAD-UNKNOWN-ADAPTER` | an unknown adapter name is answered with anything other than exit 2 and a message naming the unknown adapter |
| `UAD-CAPABILITIES-GOLDEN` | the declared adapter/probe list and the implemented one may drift without failing CI |
| `UAD-ENVELOPE` | a command emits a payload that is not the repo envelope shape |
| `UAD-PROBE-ID` | a probe id is not namespaced `^omp(\.[a-z][a-z0-9_-]*){2,}$`, or is accepted at review instead of at construction |
| `UAD-UPSTREAM-CLASS` | a substrate-side failure is reported as our own defect instead of carrying `class:"upstream_substrate_issue"` and `upstream_owner` |
| `UAD-DENOMINATOR-LIVE` | a target count is written as a constant rather than resolved through `NUMBERS.toml` `figures.built_binaries` |
| `UAD-NO-SECOND-COUNT` | a second inventory of targets is maintained beside the runner |

## Laws and named proof tests

Each law names the symbol the future suite must carry. No test file is created in this wave.

- `LAW-UAD-EVERY-TARGET-ADDRESSABLE` — every name the live runner reports is reachable as
  `<umbrella> help <adapter>` with a usage line. Test:
  `umbrella_adapter_dispatch.rs::every_live_target_has_a_usage_line`.
- `LAW-UAD-UNKNOWN-IS-TWO` — an unknown adapter exits 2 and the message names the rejected string.
  Test: `umbrella_adapter_dispatch.rs::unknown_adapter_exits_two_and_names_it`.
- `LAW-UAD-ROSTER-DERIVED` — the adapter roster is computed from the runner, so adding a bin target
  changes the roster with no source edit. Test:
  `umbrella_adapter_dispatch.rs::roster_tracks_the_runner_not_a_literal`.
- `LAW-UAD-CAPABILITIES-GOLDEN` — a probe id present in the implementation and absent from the
  golden capabilities artifact fails. Test:
  `umbrella_adapter_dispatch.rs::capabilities_drift_is_red`.
- `LAW-UAD-PROBE-ID-AT-CONSTRUCTION` — a bare-segment probe id is rejected where it is built, not
  where it is reviewed. Test: `umbrella_adapter_dispatch.rs::bare_probe_segment_is_a_construction_error`.
- `LAW-UAD-EMPTY-ROSTER-IS-ERROR` — a roster of zero adapters is an ERROR, never a pass. Test:
  `umbrella_adapter_dispatch.rs::empty_roster_is_an_error_not_a_green`.

## Known-bad leg definitions

The packet for this bead asks for the leg DEFINITION, not its implementation. Every leg below
asserts a MESSAGE, because a leg matching only `rc != 0` goes green on unrelated breakage — measured
in this repo: `cargo` returns 101 both for a missing test target and for a workspace that cannot
load, and a grade was one sentence from being confirmed on the wrong evidence.

| leg | input that must fail | required MESSAGE substring | why exit code alone is insufficient |
|---|---|---|---|
| KB-1 | an adapter name absent from the live roster | the rejected name, verbatim | exit 2 is also today's answer to an unknown VERB, so the code cannot distinguish the two |
| KB-2 | a bin target added to the workspace and absent from the roster | the added target's name | a stale roster is silently smaller, and a smaller roster passes a self-consistency check |
| KB-3 | a probe id with fewer than three segments | the offending id and the required pattern | a rejected-at-review id looks identical to a rejected-at-construction one in a green suite |
| KB-4 | a capabilities artifact missing an implemented probe id | the missing id and the artifact path | a drifted golden still parses, so a schema check passes |
| KB-5 | an empty roster (runner returns nothing) | `EMPTY_ROSTER` and the runner command | an empty scan reports identically to a complete one that found no problems |
| KB-6 | a substrate failure surfaced without `upstream_owner` | `upstream_substrate_issue` | a DOWN envelope is well-formed with or without attribution |

Every leg needs its known-GOOD twin in the same suite: the live roster passes KB-1/KB-2, a
three-segment id passes KB-3, a synchronized golden passes KB-4, the real runner passes KB-5. An
attack-only suite ships an over-strict gate, and an over-strict gate gets routed around.

## Anti-vacuity and the denominator

Every count this surface emits is a structured `=N` value alongside its denominator:
`workspace_targets=85 adapter_reachable=0 roster=0 capabilities_ids=0`. A bare ratio is refused.
An absent, empty, or unreadable roster is an ERROR (`LAW-UAD-EMPTY-ROSTER-IS-ERROR`); it is never
reported as clean. The matcher used by the Validation block carries an INLINE positive control,
because a pattern with no in-tree instance yet returns a structurally-guaranteed zero that is
indistinguishable from a measurement — the exact failure that made an authorized gate set look
empty in this repo one day earlier.

**This document is excluded from its own doc-name census, and it had to be.** The first run of the
Validation block below reported `docs_name_ompo=24 docs_name_omp_orchestrator=9` — one higher than
the true figures on both axes — because THIS FILE names both candidates while arguing about them.
A checker whose input contains prose about the thing it checks measures itself; that is the seventh
instance of the class in this repository, and the mitigation is the same one `close-evidence-gate`
uses: exclude the checker's own source before matching. Control for the exclusion rather than
trusting it — this file appears in both unfiltered sets exactly once, so the filter has a positive
control and the corrected 23/8 is a measurement rather than a subtraction.

## Validation

ONE pasteable command. Run from the repository root. It reported `UAD-GAP-PRESENT` (exit 1) at
`7b4ac63` when nothing existed and reports `UAD-OK` at `6e65e42+` now that the surface is built.

**THE `umbrella_bins != 1` CLAUSE IS RETIRED, and the reason matters more than the edit.** The
first version failed when *two* umbrella-shaped bin targets existed, on the theory that `ompo` and
`omp-orchestrator` competing was itself the unresolved-name defect. After `UAD-NAME = ompo` was
ratified (2026-09-07) that clause became WRONG: `omp-orchestrator` legitimately continues to exist
as the resident supervisor, so the check would refuse forever on a resolved question — a gate red
by construction, which this repo has measured three times and which always ends in the gate being
routed around. It is replaced by the check the ratification actually implies: `ompo` carries the
umbrella verbs and the supervisor does NOT grow them (`supervisor_verb_leak`).

Two further clauses were added because the first version was satisfiable by the wrong crate:
`capabilities_files` was scanned across `crates/*/src` and answered **9**, so an unrelated crate
mentioning the word would have turned the gate green. Both scans are now scoped to
`crates/ompo-doctor/src`. And `hardcoded_roster_literals` refuses the shape where someone replaces
the generated roster with a literal — the failure `LAW-UAD-ROSTER-DERIVED` exists to prevent.

```bash
cd /Users/josh/Developer/omp-orchestrator && \
PC=$(printf 'fn resolve_adapter(n: &str) {}\n"--adapter"\n' | grep -cE 'fn +[a-z_]*adapter[a-z_]*\(|"--adapter"|adapters\b' || true) && \
T=$(cargo metadata --format-version 1 --no-deps --offline 2>/dev/null | jq '[.packages[].targets[]|select(.kind[]=="bin")]|length') && \
A=$(grep -rlE 'fn +[a-z_]*adapter[a-z_]*\(|"--adapter"|adapters\b' crates/ompo-doctor/src 2>/dev/null | wc -l | tr -d ' ') && \
C=$(grep -rl '"capabilities"' crates/ompo-doctor/src 2>/dev/null | wc -l | tr -d ' ') && \
I=$(grep -rlE '"init" =>' crates/ompo-doctor/src 2>/dev/null | wc -l | tr -d ' ') && \
G=$(grep -rl 'OUT_DIR.*adapters.rs' crates/ompo-doctor/src 2>/dev/null | wc -l | tr -d ' ') && \
L=$(grep -c 'ADAPTERS: &\[&str\] = &\[' crates/ompo-doctor/src/umbrella.rs 2>/dev/null || true) && \
S=$(grep -rl '"capabilities"' crates/omp-orchestrator/src 2>/dev/null | wc -l | tr -d ' ') && \
printf 'UAD-CENSUS matcher_positive_control=%s workspace_targets=%s adapter_files=%s capabilities_files=%s init_verb_sites=%s generated_roster_includes=%s hardcoded_roster_literals=%s supervisor_verb_leak=%s\n' "$PC" "$T" "$A" "$C" "$I" "$G" "$L" "$S" && \
if [ "$PC" -eq 0 ]; then echo 'UAD-INSTRUMENT-ERROR: matcher cannot match its own needle'; exit 3; \
elif [ "$T" -eq 0 ]; then echo 'UAD-EMPTY-ROSTER: the target runner returned nothing'; exit 3; \
elif [ "$A" -eq 0 ] || [ "$C" -eq 0 ] || [ "$I" -eq 0 ]; then echo 'UAD-GAP-PRESENT'; exit 1; \
elif [ "$G" -eq 0 ] || [ "$L" -ne 0 ]; then echo 'UAD-REGISTRY-HANDROLLED: the roster is a literal, not derived'; exit 1; \
elif [ "$S" -ne 0 ]; then echo 'UAD-NAME-VIOLATED: the resident supervisor grew an umbrella verb'; exit 1; \
else echo 'UAD-OK'; fi
```

Captured 2026-09-07, post-implementation:

```text
UAD-CENSUS matcher_positive_control=2 workspace_targets=85 adapter_files=2 capabilities_files=2 init_verb_sites=1 generated_roster_includes=1 hardcoded_roster_literals=0 supervisor_verb_leak=0
UAD-OK
```

`|| true` ON THE TWO `grep -c` ASSIGNMENTS IS LOAD-BEARING, and it was caught by running this.
`grep -c` prints `0` and then exits **1**, which killed the `&&` chain before `printf` and produced
**no output at all with exit 1** — indistinguishable, to a careless reader, from a clean
`UAD-GAP-PRESENT`. `|| true` keeps the printed `0` and neutralises the exit. NOT `|| echo 0`,
which appends a SECOND zero and yields `"0\n0"`; that exact idiom is the documented defect that
made every zero-dependency crate score as having dependencies.

`UAD-INSTRUMENT-ERROR` and `UAD-EMPTY-ROSTER` exit **3**, distinct from the subject verdict's **1**,
so "the instrument failed" can never be read as "the subject is broken" — this repo has paid for
that conflation with a six-hour false diagnosis.

## Wiring

This contract is inert until something invokes it. The reachable trigger MUST be named when the
implementation bead lands: the Validation block as a CI job, or the future suite reached by
`cargo test -p ompo-doctor --test umbrella_adapter_dispatch`. A wiring proof that terminates at
another uncalled crate is not a wiring proof; walk the chain to an executor, and note that a
`.flywheel/` document naming a binary is prose, not an executor.

## Non-Coverage — what this contract DOES NOT MEAN

- It does not define doctor probe semantics, verdict arms, repair, or undo. `s1_l1_doctor.md` owns
  those, and its seven-arm `ProbeVerdict` is normative here by reference.
- It does not define install, verification, publication, or uninstall. `s1_l0_install.md` owns those.
- It does not claim any of the 85 targets is installable, healthy, or correct. Addressability is
  strictly weaker: it means a name can be reached and answered, nothing about the answer.
- It does not ratify `UAD-NAME` or `UAD-ADDRESS`. Both are open decisions and are stated as such.
- It does not authorize implementation. S1 readiness is defined solely by
  `docs/plan/flow/S1-READY.md`, and this file neither restates nor amends those criteria.

## Cross-References

- `docs/plan/07-installability.md:113-124` — the aggregator shape, envelope, and probe-id namespace
- `docs/plan/07-installability.md:576` — the shape question stated as OPEN
- `docs/contracts/s1_l1_doctor.md:11-13,90` — doctor artifacts and `L1-BUILD-SCOPE`
- `docs/contracts/s1_l0_install.md:11-13` — install artifacts and the `ompo` runner name
- `crates/ompo-doctor/src/main.rs:7-11,21-24` — the shipped usage line and the single-verb refusal
- `crates/omp-orchestrator/src/main.rs:677-679` — the resident supervisor's four verbs
- `NUMBERS.toml` `figures.built_binaries` — the live target denominator
- `docs/plan/flow/S1-READY.md` — the only definition of S1 readiness

## NO-CLAIM

This is a planning artifact. Nothing here is implemented: adapter dispatch, the capabilities golden
artifact, the probe-id constructor, and the invariant suite are all MISSING at `7b4ac63`, and the
Validation block proves their absence rather than their correctness. The premise measurement is a
static source and metadata census on one host at one revision; it does not execute either umbrella
binary, because neither is built here, so the claim is about the source surface and not about
runtime behaviour. `UAD-NAME` and `UAD-ADDRESS` carry a RECOMMENDATION, not a ruling, and the
recommendation is argued from doc-reference counts and shipped shape — neither of which is a
mandate. The 85 figure is LIVE and will move; re-run the runner before citing it.
