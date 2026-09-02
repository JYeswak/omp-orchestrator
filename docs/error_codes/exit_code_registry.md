# exit_code_registry

Bead: `omp-orchestrator-exit-code-registry-rub`

## Purpose

Registers every process exit code this workspace **emits** and every foreign code it **receives**,
one row per code, with the emitting crates, what the code means, **what it does not mean**, and the
operator's next action. It reserves the number space — which bands are ours, which belong to
`sysexits.h`, which to the Rust toolchain, the `cargo` wrapper, `rch`, and POSIX shell — and it
names every code that currently carries two or more meanings. It documents what IS, at the pinned
commit below; it renumbers nothing, because callers already depend on these numbers. The registry
is enforced rather than snapshotted: `crates/no-shell-gate/tests/exit_codes.rs` re-derives the
emission set from source on every run and fails when a code is emitted that has no row here.

## Contract Artifacts

1. **Canonical artifact:** the `XC-*` tables in this document. They are the machine-read artifact —
   the invariant suite parses this file's markdown rows directly, so there is deliberately no
   sidecar `artifacts/exit_codes_v1.json`: a second copy of the table is a second source of truth,
   and the failure mode of a registry is drift, not serialization.
2. **Runner:** `cargo test -p no-shell-gate --test exit_codes -- --nocapture`
3. **Invariant suite:** `crates/no-shell-gate/tests/exit_codes.rs` — seven legs: the real scan, an
   anti-vacuity leg, a fires-on-known-bad leg with a planted undocumented code, a known-good leg, a
   column-completeness leg that refuses any row with an empty **does NOT mean**, a scan-floor leg
   seeded from this document's own measurement, and a pass-through-declaration leg.

> A contract naming no invariant suite is a DESCRIPTION. Item 3 is what stops this file rotting in
> a week, which is what every previous snapshot of these numbers did.

## 1. Why this document exists, measured

Four exit codes were misread by agents on 2026-09-01, this one included:

| code | the misread | what it actually was |
|---|---|---|
| `103` | read as a TEST RESULT, because `0 passed 0 failed` printed beside it | an `rch` refusal — a failure path emitting something SHAPED LIKE DATA |
| `75` | read as a build failure | `CARGO_MINT_CONTAINER_EXHAUSTED`, the disk-floor refusal |
| `127` | read as "`bin/check.sh` is untracked", a repo defect that did not exist | a stale worktree missing a file that HEAD has |
| `1` | read as "the gate is broken" and as "the gate correctly refused" | both, on different lines of the same file |

The common shape is not carelessness. It is that **a refusal and a failure are indistinguishable
from the outside** when the only signal is a small integer, and nothing in the repo said which was
which. Before this document, `git grep -c --no-index -E '^//[/!].*\bexit'` over `crates/*/src/*`
answered **0 files** — no crate documented its own exits — against a positive control of **102
files** carrying `//!` docs at all. The registry's load-bearing column is therefore **does NOT
mean**, not **means**.

## 2. Derivation — every figure below carries its command

Pinned: the counts in this document were derived at commit `d48615c` on 2026-09-01. Re-derive
before citing; the invariant suite re-derives on every run.

```bash
# emission sites, by mechanism
git grep -c --no-index -E 'ExitCode::from'   -- 'crates/*/src/*'   # 284 lines
git grep -c --no-index -E 'process::exit'    -- 'crates/*/src/*'   #  14 lines
git grep -c --no-index -E 'ExitCode::(SUCCESS|FAILURE)' -- 'crates/*/src/*'  # 151 lines
# distinct literal codes and their site counts
git grep -ho --no-index -E 'ExitCode::from\([0-9]+\)' -- 'crates/*/src/*' \
  | grep -oE '[0-9]+' | sort -n | uniq -c
# named constants, and any name bound to two values
git grep -n --no-index -E '^\s*(pub )?const EXIT_[A-Z_]+' -- 'crates/*/src/*'
# scan-set floor: .rs files under crates/*/src
git ls-files --others --cached --exclude-standard -- 'crates/*/src/*' | grep -c '\.rs$'  # 123
```

**INSTRUMENT NOTE, and it is the reason this section exists.** The first run of the literal-code
extraction used `grep -oE '[0-9]+$'` — anchored at end-of-line — against lines of the form
`ExitCode::from(2);`. It answered **zero distinct codes** while 245 literal sites existed. A
confident zero from a pattern that cannot match is the same defect family as every misread in §1.
The second instrument error was worse: the file set was derived with
`git grep -l -E 'ExitCode|process::exit'`, which returned 53 files and **silently omitted**
`crates/fleet-monitor/src/lib.rs`, the file that declares `EXIT_CANNOT_OBSERVE = 78` and contains
neither token. **A scan set must be at least as wide as the patterns run over it**; the corrected
set is all 123 `.rs` files under `crates/*/src`.

## 3. Codes this workspace EMITS

12 distinct codes, derived by the commands in §2. `sites` counts literal emission sites; `crates`
counts distinct emitting crates.

| ID | code | emitters | MEANS | does **NOT** mean | operator's next action |
|---|---|---|---|---|---|
| `XC-000` | 0 | all 51 bin targets | the process completed and made its claim | that work happened. `Discharged::exit_code()` (`crates/omp-orchestrator/src/lib.rs:1359`) returns 0 only for NON-EMPTY evidence, and a no-op tick, a `--dry-run`, and a real dispatch all exit 0 | read the emitted JSON, never the code alone |
| `XC-001` | 1 | 32 crates, 89 sites | **overloaded, three ways**: a gate refused (a working gate), the tool itself broke, or the caller misused the CLI | any one of the three. `crates/no-shell-gate/src/bin/pre-push-gate.rs` emits 1 for `PRE_PUSH_GATE_REFUSED` at `:312,321,366,375,384,402,416,434` AND for `PRE_PUSH_GATE_ERROR` at `:287,303,340,427` | read the marker prefix on stderr; `REFUSED` = the gate worked, `ERROR` = the gate did not run. Filed as `omp-orchestrator-exit-1-overloaded-x1o` |
| `XC-002` | 2 | 34 crates, 118 sites | dominantly a usage error (50 of 118 sites carry a `usage` message), also a runtime error, also "unmeasurable" | a usage error. `OracleCompareVerdict::Unmeasurable` maps to 2 (`crates/oracle-compare/src/lib.rs:103`) while `EXIT_USAGE = 2` in `crates/fleet-composite/src/main.rs:14`; `PLAN_ASSEMBLE_ERROR` also exits 2. **It also collides with the `cargo` wrapper's own 2** — see `XC-EXT-002` | check stderr for a `usage:` block before assuming operator error |
| `XC-003` | 3 | 11 crates, 24 sites | a dependency the tool needs is unavailable or unreadable — `ORACLE_UNAVAILABLE`, `TRACKER_ERROR`, `PRODUCT_UNASKABLE`, lint `ERROR` | that the checked property is bad. 3 is "could not check", adjacent to `XC-077` | fix the dependency, then re-run; do not record a verdict |
| `XC-004` | 4 | `loop-switch` (3), `no-shell-gate` (1) | a state write or removal failed, including "removal reported success but the switch is still set" | a policy refusal. This is a filesystem-level failure of the tool's own state | inspect the state path named on stderr |
| `XC-064` | 64 | 6 crates, 11 sites | `EX_USAGE` — command line could not be parsed | a config error, although `crates/fleet-composite/src/main.rs:16` **names it** `EXIT_CONFIG = 64`, which is `EX_CONFIG`'s job at 78. Filed as `omp-orchestrator-exit-const-collision-cas` | correct the invocation |
| `XC-070` | 70 | `omp-orchestrator` (`exit_code`) | `EX_SOFTWARE` — a discharge carrying EMPTY evidence, i.e. a no-op wearing a success | a crash. It is a deliberate refusal to let empty evidence pass as 0 | supply evidence, or accept that the decision was not discharged |
| `XC-075` | 75 | `installer` (1 site), `loop-driver` `EXIT_CONCURRENT:17`, `pane-dispatch-fence` `EXIT_BUSY:16` | `EX_TEMPFAIL` — retry later: another instance holds the lock, the pane is busy, or the installer is blocked | a defect in the work being attempted. **Three meanings in our tree plus two more from the `cargo` wrapper** — see `XC-EXT-075`. Filed as `omp-orchestrator-exit-const-collision-cas` | wait and retry; do not treat as a verdict |
| `XC-076` | 76 | `pane-dispatch-fence` `EXIT_NOT_FREE:17` | `EX_PROTOCOL` — the pane is not free capacity | the pane is dead or wedged; it may be legitimately working | select another pane |
| `XC-077` | 77 | `dispatcher-deadman` (4), `tick-dispatch` (4), `fast-dispatch` (1) | **UNPROVEN** — the checker could not establish its verdict: unreadable cwd, unset `HOME`, unwritable state, a required child unavailable | that the watched condition is bad. This is the fail-closed unknown, and it is the most useful code in the registry. Note it deviates from `sysexits.h`, where 77 is `EX_NOPERM`. Filed as `omp-orchestrator-exit-reserved-range-b7f` | fix the checker's environment; the verdict is absent, not negative |
| `XC-078` | 78 | `fleet-monitor` `EXIT_CANNOT_OBSERVE:413`, `pane-dispatch-fence` `EXIT_CONFIG:18` | `EX_CONFIG` in one crate, "cannot observe the fleet" in the other | one thing. **One value, two unrelated names** — the registry's clearest collision. Filed as `omp-orchestrator-exit-const-collision-cas` | read which binary exited, then the marker |
| `XC-124` | 124 | `loop-driver` `EXIT_DEADLINE:18` | our own deadline elapsed | that `timeout(1)` killed us — **and that is not distinguishable from outside**. Measured live while writing this document: a probe wrapped in `timeout 90` returned 124 from `timeout`, not from any of our binaries. Filed as `omp-orchestrator-exit-reserved-range-b7f` | check whether a `timeout` wrapper was in the command line before believing the binary self-limited |

## 4. Codes this workspace RECEIVES — foreign, never ours to emit

These arrive from the toolchain, the `cargo` wrapper, `rch`, or the shell. Every row was verified
against the emitting binary, not inferred from convention.

| ID | code | source | MEANS | does **NOT** mean | operator's next action |
|---|---|---|---|---|---|
| `XC-EXT-002` | 2 | `~/.local/bin/cargo` (the mint wrapper — a THIRD `cargo` on `PATH`, ahead of `~/.cargo/bin`) | a lane-identity or target-ownership refusal: `CARGO_LANE_IDENTITY_UNSAFE`, `CARGO_LANE_ISOLATED_TARGET_REQUIRED`, `CARGO_TARGET_ROOT_UNREGISTERED`, and 6 more | a compile error, and **not** our `XC-002` either | read the `CARGO_*` marker on stderr |
| `XC-EXT-006` | 6 | the same wrapper | a reclaim or contract precondition is unavailable: `CARGO_UNAVAILABLE`, `CARGO_RECLAIM_UNAVAILABLE`, `CARGO_LANE_CONTRACT_UNWRITABLE`, `CARGO_LANE_AUDIT_UNWRITABLE` | a build outcome. Nothing was compiled | fix the named precondition |
| `XC-EXT-075` | 75 | the same wrapper | the disk floor refused the build: `CARGO_MINT_CONTAINER_EXHAUSTED` or `CARGO_MINT_PEAK_WOULD_BREACH_FLOOR` | a build failure. **The wrapper is the only emitter on this machine that already ships the disambiguation in data**: its JSON carries `"exit_code":75,"is_gate_verdict":false` (`~/.local/bin/cargo:388`). That field is the precedent this whole registry generalises | reclaim disk, then re-run; `CARGO_MINT_MIN_CONTAINER_PCT=0` is a deliberate exception, not a fix |
| `XC-EXT-101` | 101 | `rustc` / `cargo` | a Rust panic — measured: a `panic!()` binary exits 101 | a test failure or a refusal | read the panic message and backtrace |
| `XC-EXT-124` | 124 | `timeout(1)` | the wrapper killed the child at its deadline | that the child decided anything. **Collides with our `XC-124`** | re-run with a longer deadline before drawing any conclusion |
| `XC-EXT-126` | 126 | POSIX shell | the file exists but is not executable | that it is missing | `chmod +x`, or check the interpreter line |
| `XC-EXT-127` | 127 | POSIX shell | command not found | that the repo is missing the file. On 2026-09-01 a 127 for `bin/check.sh` was read as "untracked" when the real cause was a **stale worktree** whose tree predated the commit that added it | verify `git ls-files` in the SAME tree that produced the 127 |
| `XC-EXT-128N` | 128+N | POSIX shell | the process died on signal N — 130 `SIGINT`, 137 `SIGKILL`/OOM, 139 `SIGSEGV` | a chosen exit. Nothing in this workspace emits above 124 | for 137, check memory and the OOM killer before the code |
| `XC-EXT-RCH` | see note | `/Users/josh/.local/bin/rch` | a remote-compilation refusal: `[RCH] remote required; refusing local fallback [RCH-E301]` | a test result — this is §1's most expensive misread, because the refusal prints alongside cargo-shaped output | read the `[RCH]` prefix and the `RCH-Exxx` code, never the integer |

**On `103` specifically, and this is a correction to the session's own account.** The number was
recorded as 103 during the incident. Two probes tonight could not reproduce it:
`rch exec -- cargo --version` under `RCH_REQUIRE_REMOTE=1` exits **1**, printing
`[RCH] remote required; refusing local fallback [RCH-E301]`; the compilation-verb probe was killed
by its own `timeout` wrapper at 124 before rch answered. `rch schema export` ships a
machine-readable catalog of **100 `RCH-Exxx` codes with ZERO numeric exit mappings**
(`grep -c exit /tmp/…/error-codes.json` finds no exit key). So: **the string is authoritative and
verified; the integer 103 is OBSERVED, not documented upstream and not reproduced here.** Recorded
that way deliberately — a registry that launders an unverified number into a fact is the artifact
this document exists to replace.

## 5. Range ownership — the reservation table

| band | owner | our use | rule |
|---|---|---|---|
| `0` | POSIX | success | `XC-000` |
| `1`–`4` | **ours**, legacy | verdict + error + usage, overloaded | do not add meanings; new semantics go to an unallocated band |
| `5`–`63` | **UNALLOCATED, ours to claim** | none | the correct home for any new distinct meaning |
| `64`–`78` | `sysexits.h` (`EX_*`) | 64, 70, 75, 76, 77, 78 | keep `EX_*` semantics; `XC-077` already deviates (`EX_NOPERM`) |
| `79`–`100` | unallocated | none | free |
| `101` | Rust toolchain | **never emit** | panic |
| `102`–`125` | toolchain and wrappers | `124` only, and it collides | **never emit in this band**; `XC-124` is a standing violation |
| `126`, `127` | POSIX shell | never | not-executable / not-found |
| `128`–`255` | POSIX shell signals | never | `128+N` |

**Collisions found, all measured, none invented:**

1. `EXIT_CONFIG` is bound to **two values** — 64 in `fleet-composite`, 78 in `pane-dispatch-fence`.
2. `78` is bound to **two names** — `EXIT_CANNOT_OBSERVE` and `EXIT_CONFIG`.
3. `75` carries **five meanings** — `EXIT_CONCURRENT`, `EXIT_BUSY`, the installer's block, and the
   wrapper's two mint refusals.
4. `2` is emitted by us (118 sites, three meanings) **and** by the `cargo` wrapper (9 markers).
5. `124` is ours and `timeout(1)`'s.
6. `77` means UNPROVEN here and `EX_NOPERM` in `sysexits.h`.

## 6. Pass-through — where a foreign code becomes ours

Seven sites forward a child process's code as their own, which is how a foreign integer arrives
wearing one of our binaries' names. Every site must be listed here or the invariant suite fails.

| ID | expression | crates |
|---|---|---|
| `XC-PT-CODE` | `code` | `loop-tick`, `omp-inventory-map` |
| `XC-PT-EXIT` | `exit` | `omp-idle-dispatch`, `tick-dispatch` |
| `XC-PT-OUT` | `out.code` | `verify-dispatch` |
| `XC-PT-OUTPUT` | `output.code` | `loop-driver`, `loop-queue-filter` |
| `XC-PT-RC` | `rc` | `fleet-monitor` |
| `XC-PT-VERDICT` | `verdict.exit` | `dispatcher-deadman` |
| `XC-PT-EXITCODE` | `v.exit_code()` | `pane-oracle-diff` |

A pass-through means **the code space at that site is not ours** — it is 0–255 of whatever ran
underneath, including `XC-EXT-101` and every `XC-EXT-*` row above. `XC-PT-VERDICT` and
`XC-PT-EXITCODE` are the two exceptions: they forward a code from a typed verdict inside this
workspace, so their range is the `XC-*` table.

## Validation

```bash
cargo test -p no-shell-gate --test exit_codes -- --nocapture
```

Expect **7 passed**. The suite re-derives the emission set from all `.rs` files under
`crates/*/src` and fails naming `file:line` for any code with no row here, refuses any row whose
**does NOT mean** cell is empty, errors on an empty scan set rather than passing, and proves it
fires by planting an undocumented code in a temporary fixture.

## Cross-References

- `crates/no-shell-gate/tests/exit_codes.rs` — the invariant suite
- `crates/no-shell-gate/src/bin/pre-push-gate.rs` — `XC-001`'s triple meaning, at the lines cited
- `crates/oracle-compare/src/lib.rs:99-104` — the *good* pattern: Agree/Disagree/Unmeasurable as 0/1/2
- `crates/omp-orchestrator/src/lib.rs:1359` — `XC-070`, empty evidence is not success
- `crates/fleet-composite/src/main.rs:14-16` — `EXIT_USAGE`/`EXIT_CONFIG`, collision 1
- `crates/pane-dispatch-fence/src/main.rs:16-18` — `EXIT_BUSY`/`EXIT_NOT_FREE`/`EXIT_CONFIG`
- `crates/loop-driver/src/lib.rs:17-18` — `EXIT_CONCURRENT`/`EXIT_DEADLINE`
- `crates/fleet-monitor/src/lib.rs:413` — `EXIT_CANNOT_OBSERVE`, the file the first scan missed
- `crates/dispatcher-deadman/src/main.rs:128-159` — `XC-077`, `emit_unproven` before every 77
- `~/.local/bin/cargo:388` — the `is_gate_verdict` precedent
- `docs/contracts/asupersync_process_grade.md` — the grade this document is scored by
- `docs/plans/plan_to_write_the_document_corpus.md` — this is document #13 of 78
- `NUMBERS.toml` — the figure-carries-its-command discipline §2 follows

## Non-Coverage

- **No renumbering.** Every collision in §5 is documented, not fixed. Renumbering breaks callers
  and belongs to the three filed beads.
- **No code is changed.** Not one `ExitCode::from` was touched.
- **stderr markers are not registered.** `PRE_PUSH_GATE_REFUSED`, `CARGO_MINT_*`, `RCH-Exxx` and
  the rest are cited where they disambiguate a code, but a marker registry is its own document.
- **Non-`crates/*/src` emitters are out of scope.** Test binaries, fixtures, and the pre-commit
  hook's own shell-free wrapper are not scanned.
- **The 51 `ExitCode::SUCCESS`/`FAILURE` sites are not individually mapped.** `FAILURE` is 1 and
  folds into `XC-001`; enumerating each site would not change a row.
- **No claim that any band is empty.** §5 says which bands we *use*; a foreign binary may emit
  anything.

## NO-CLAIM

**Documenting a code does not make any caller emit it correctly.** Every row above describes what
the source does today; nothing here constrains a future emission site, and the invariant suite
catches only *undocumented* codes — it cannot tell a correct 1 from a wrong 1, because both are 1.
That is the defect `XC-001` names, and the registry does not fix it.

The suite proves **presence of a row**, not **truth of a row**: a wrong **does NOT mean** cell
passes every leg. It also cannot see emissions it does not scan — a code produced by a shell
wrapper, a build script, or a crate outside `crates/*/src` is invisible to it, and the pass-through
sites in §6 are unbounded by construction.

`XC-EXT-RCH`'s integer is unverified, stated as such. And the four misreads in §1 were made by
agents who had this repo open; a document does not remove that failure mode, it only makes the
disambiguation cheaper than re-deriving it under pressure.
