# NEGATIVE_EVIDENCE.md

Refuted hypotheses, reverted "wins", NO-SHIP measurements, and exhausted veins.
**Every row carries a RETRY CONDITION** — the predicate that would make the row stop applying.
Read this before starting any perf, detector, or optimisation hypothesis; a vein recorded here
has already been dug.

`AGENTS.md`'s session-start line instructs every agent to read this file. **It did not exist until
2026-09-03T19:2xZ** — the instruction pointed at nothing, which is its own instance of the
declared-but-absent class (`inception.json` and `FINDINGS.jsonl` are declared in `SCHEMAS.toml`
with zero instances on disk for the same reason). First row below.

---

## NE-001 — A bare `cargo` exit of 101 from an RCH-offloaded run is uninformative

**Measured 2026-09-03. Three consecutive `101`s, three different workers, THREE DIFFERENT CAUSES, and
not one of them was a compile error.**

| run | worker | apparent verdict | actual cause |
|---|---|---|---|
`cargo check --workspace --quiet` (pane 1) | contabo-4 | `error: could not compile asupersync (lib)` | **`signal: 9, SIGKILL`** — rustc OOM-killed |
`cargo check --workspace` (AsupersyncStable) | contabo-1 | same single error line | **SIGKILL again**, `0 slots remaining after reservation` |
`cargo check --all-targets -j 1 -p <14 CI packages>` | contabo-3 | `101`, 552s | **a tracked file the worker never received** |

The third is the subtlest and the most instructive. The error was:

```
crates/omp-inventory-map/tests/coverage_mission.rs:18:21: error: couldn't read
  .../docs/plan/OMP-COVERAGE-TABLE.jsonl: No such file or directory (os error 2)
```

That file **exists and is tracked** — 15,797 bytes, `git ls-files` returns 1. It is absent on the
worker because **rch syncs only the cargo closure**; `.cargo/config.toml`'s own comment says so
(*"cargo needs ~4.8 MB (crates/ + Cargo.toml/lock + registries/) while we shipped ~1.15 GB"*), and
`docs/` is not in that closure. On `ubuntu-latest`, `actions/checkout` hands the job the whole tree
and the same `include`-time read succeeds — as it does locally.

**THE RULE: read `signal:` before you read `E`, and confirm the tree the worker actually received.**
`--quiet` suppresses the one line that distinguishes a dead process from a failed compile, so a
`--quiet` offloaded run cannot be diagnosed at all. Pane 1 filed a P0 push-hold on the first of
these before checking for `signal:`, and retracted it.

**What survives as a positive claim, and only this:** on `x86_64-unknown-linux-gnu`, serialised with
`-j 1`, `asupersync` compiles under the committed feature set with **zero `E0554`** and **zero
signals**. `channel = "stable"` is viable on the governing architecture.

**RETRY CONDITION.** This row stops applying the moment a verdict comes from a **full checkout
building one thing at a time** — i.e. a real CI run on `ubuntu-latest`, or a local run with the
complete tree present. Until then: an offloaded SIGKILL is weak evidence of red, and an offloaded
`exit=0` is weak evidence of green. The asymmetry cuts against both sides, which is why
`AsupersyncStable` raised it against its own green numbers first.

**NO-CLAIM.** This row says the offloaded instrument cannot settle the question. It does **not** say
the tree is green: two crates are independently broken (`NE-002`, `NE-003`), both channel-independent.

---

## NE-002 — `loop-driver`'s `Cx::for_request` is NOT pinned-rev drift

**Hypothesis refuted 2026-09-03.** Pane 1 filed `omp-orchestrator-loop-driver-for-request-e0599-bx50`
offering three causes: (a) the method was removed between revs, (b) it never existed, (c) it is
behind a feature `default-features = false` now excludes. **All three are wrong.** The answer is a
fourth, verified independently by pane 1 against the vendored checkout:

```
asupersync fa3c01aec  src/cx/cx.rs:5037  #[cfg(any(test, feature = "test-internals"))]
                      src/cx/cx.rs:5039  pub fn for_request() -> Self
                      src/cx/cx.rs:5026-5028  same gate on for_request_with_budget
```

The method **exists at the pinned rev** and is **test-gated**. So the defect is that **production
code calls a test-only constructor** — `crates/loop-driver/src/lib.rs:1574` and
`crates/loop-driver/src/main.rs:232` — and nothing in this workspace enables `test-internals`,
deliberately: `crates/omp-types/Cargo.toml:12-19` explains at length why enabling it would
reintroduce the production leak upstream issue #46 closed.

A feature-visibility violation at the kernel boundary, **2 sites**, and the repair is the pattern
30+ other call sites in this repo already use: `RuntimeBuilder::current_thread().build()` then
`Cx::current()`. **Not** an upstream concern and **not** caused by `channel = "stable"`.

**RETRY CONDITION.** Re-open the drift hypothesis only if the pin moves off `fa3c01aec`, or if
`test-internals` is ever enabled in this workspace — at which point the leak that issue #46 closed
becomes the live risk instead.

---

## NE-003 — `fleet-monitor`'s 33 errors have nothing to do with asupersync

**Hypothesis refuted 2026-09-03.** Pane 1 asked whether `fleet-monitor` and `loop-driver` were the
same class — both calling asupersync APIs absent at the pin — because that would be **one** finding
about the kernel boundary rather than two crate bugs. They are not.

```
git show HEAD:crates/fleet-monitor/Cargo.toml | grep -c asupersync   -> 0
git grep -c asupersync HEAD -- crates/fleet-monitor/                 -> 0 files
asupersync mentions in its 33-error log                              -> 0
```

Histogram against the `b0e000d` export: **15 × E0433, 11 × E0425, 7 × E0599** (33 total; 23 for
`--bins` alone). Missing symbols: `BoundedOutcome` ×14, `PipeReadOutcome` ×12, `IoFailed` ×4,
`Stdio` ×4, `terminate_and_reap` ×4, `join_pipes` ×3, `bounded_output` ×3, `command` ×1,
`process_group` ×1.

The decisive detail: **`NtmActivityOutcome` is fleet-monitor's OWN enum**, declared in the same file
at `src/main.rs:74-95` with variants `Completed / TimedOut / SpawnFailed / OutputLimitExceeded /
NoPanes / PaneEnumerationFailed`. There is no `IoFailed`. `Stdio` and `Command::process_group` are
std. `bounded_output` / `BoundedOutcome` are imported at `:32` yet unresolved at `:1339-1378`,
meaning those call sites sit inside a nested module the outer `use` does not reach.

**Verdict: one incomplete in-tree refactor, one crate, one owner.** Somebody restructured
fleet-monitor's process handling, committed the call sites, and did not commit the imports, helpers,
or enum variant. Pane 1 confirmed the missing half is sitting uncommitted: `git diff --stat --
crates/fleet-monitor/` shows **320 changed lines in `src/main.rs`**, 4 files, 391 insertions / 181
deletions.

**RETRY CONDITION.** Re-open the kernel-boundary hypothesis only if the errors survive after that
uncommitted diff lands. If they do, the symbols genuinely do not exist and the class changes.
