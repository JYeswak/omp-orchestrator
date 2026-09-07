# DEFINE — `gate_runner_full_roster`

Phase 1 artifact of `/skill:profiling-software-performance`. **Nothing may be optimized until the
hotspot table downstream of this file exists** — the skill's One Rule is *ranked evidence before any
optimization*, and its Quick Trigger for *"optimize X"* is **refuse until a profile identifies X as
top-5**.

Written 2026-09-07 by `pane1-omp-claude`. **This file measures nothing.** It declares what would
count as an answer.

---

## Scenario

`gate_runner_full_roster` — one invocation of `gate-runner --run` over the full derived roster,
executed on the Contabo remote lane through `rch exec` (never locally; zero local Rust builds is
binding).

```
entry point   crates/gate-runner/src/main.rs
loop          `for entry in &roster` at main.rs:185   -- SERIAL, measured
per iteration Command::new("cargo") .args(["test", ...]) at main.rs:340
              through subprocess_contract::bounded_output, PER_CRATE_DEADLINE = 600s
roster        88 crates, DERIVED from cargo metadata `[package.metadata.gate]`, not hand-maintained
```

## Metric

**Per-crate wall time, ranked** — plus the run total as a secondary.

**The primary metric is per-crate because the total cannot be baselined to this skill's standard on
this lane, and saying so is part of the definition.** The skill requires **≥ 20 runs** for a
baseline. One run is **21m21s**, so twenty runs is **≈ 7.1 hours** of exclusive lane time, on a
shared four-worker fleet where `rch` already refuses concurrent same-project builds
(`active_project_exclusion`). **A 20-run total-time baseline is not affordable and will not be
faked.**

**The resolution is that one instrumented run yields 88 samples.** Per-crate ranking — which is what
a hotspot table is — reaches n=88 in a single invocation and satisfies the sample-count requirement
for the ranking itself. The run **total** stays at **n=1** and is labelled `conservative
worst-observed`, never a p95.

```
per-crate      n=88 in one run   -> ranked hotspot table, this is the deliverable
run total      n=1               -> single observation, NOT a percentile, flagged as such
p99.9 / p99.99 NOT REPORTED      -> the skill forbids pretending to stats you do not have
```

## Budget

**645 s** total wall — the skill's default of *current × 0.5* — **offered for override, not
asserted as a requirement.**

Two reasons it is provisional. First, the input to that arithmetic is a **single observation**, and
half of one sample is not a defensible target. Second, the real question is not the number but the
**class**: at 21m the gate fits CI's 6h ceiling with 17× of headroom and is fine as a per-push job,
but it is far too slow for a pre-commit hook. **So the budget that matters is whichever use is
intended, and that is Joshua's call rather than a derivation.**

```
measured       1281 s   (21:20:17Z -> 21:41:38Z, one run, 2026-09-07)
default budget  645 s   (measured x 0.5, per the skill's rule when no number is committed)
CI ceiling     21600 s  (GitHub Actions 6h) -- currently 17x of headroom
```

**Explicitly NOT a budget: the 14.7 h worst case.** `600 s × 88` is reachable if crates hang and no
aggregate deadline exists (`omp-orchestrator-9gta3`), but a **bound is not a forecast** — that
conflation was a real error tonight, made by `%19` and then amplified by me into a P0 title before
`%19` refuted it with a measurement.

## Golden output

```
docs/plan/perf/gate-runner-full-roster/golden-88rows-2026-09-07.txt   (preserved copy)
sha256  85aea04b7ff59220b9fa9ea99617fb96...
88 verdict-shaped rows: crate=<name> plus PASS/FAIL and failing_targets
crates=88 pass=67 fail=21 unmeasurable=0 short=0 no_tests=0   exit=1
```

**Any optimization must reproduce this crate→verdict mapping exactly.** Not the count — the
**mapping**. A change that keeps `67/21` while moving which crates fail has changed behaviour, and a
count-only check cannot see it.

**The corpus was rescued from `/tmp`.** `%19` banked it there and `/tmp` is reaped; it is now under
`ZS_SCRATCH` and in-tree. **This is the only full-roster run in this repository's history** — losing
it would have cost 21 minutes of exclusive lane time to recreate, and per `C38` a fixture drifted
from production certifies nothing, so a real artifact was worth preserving over a synthesised one.

## Scope boundary

```
IN     per-crate wall time; the serial-vs-parallel schedule; the roster derivation cost
OUT    the 21 failing crates' CAUSES        -- %7 owns classification, %8 owns UNMEASURABLE
OUT    local execution                      -- zero local Rust builds, binding
OUT    optimizing anything                  -- this skill hands off; it does not change code
```

## THE BLOCKER, and it is why no profiler runs yet

**`gate-runner` measures no durations.** Verified at HEAD:

```
grep -c 'Instant::now' crates/gate-runner/src/main.rs   ->  0
grep -c '\.elapsed()'  crates/gate-runner/src/main.rs   ->  0
rayon / thread::spawn over the roster                   ->  0
```

The 1281 s figure exists **only as a total**, from two `rch` transport timestamps bracketing the run.
The five duration-bearing lines in the banked output are `rch`'s own transfer logs — *sync complete*,
*wrapping command with external timeout*, *remote command finished* — **not crate timings.**

**So "which crate is slow" is unanswerable today, and `extreme-software-optimization` has nothing to
score.** Phase 4 (Instrument) is missing entirely, and it is the whole gap between here and a
hotspot table.

## THE HYPOTHESIS THAT MUST BE KILLED FIRST — and it decides whether this is a code problem at all

```
H1  the 1281 s is ~100% child `cargo test` time, and gate-runner's own Rust is noise
```

**If H1 SUPPORTS, then profiling `gate-runner`'s code is optimizing ~1% of the run**, and the only
levers are scheduling: bounded parallelism over the serial loop at `main.rs:185`, skip-unchanged, or
a shared build. **If H1 REJECTS**, something in our own code — roster derivation, output parsing,
the manifest walk — is material and a CPU profile is warranted.

**H1 is cheap to settle and nothing else should be measured before it.**

**THE DENOMINATOR IS NAMED, AND MY FIRST VERSION OF THIS LINE NAMED THE WRONG ONE.** `%19` flagged
it: *"if the ratio comes back below 0.95 I would suspect the instrument before the code, since the
denominator is two `rch` transport timestamps and the numerator will be 88 in-process spans."*
Measured against the banked log — sync completes at `21:20:23` and the command is wrapped at
`21:20:28`, against a first-to-last span of `1281 s`, so **~11 s ≈ 0.9 % of the total is `rch`
transport that no crate spent.**

**0.9 % sounds ignorable and it is not, because it straddles the threshold.** A true ratio of `0.955`
is pushed to `0.946` by transport alone and would REJECT on an instrument artifact — the exact class
this repository keeps paying for. So:

```
numerator     sum of the 88 per-crate wall_ms spans
denominator   gate-runner's OWN total, emitted by the same instrumentation
              NOT the rch first-to-last timestamp span, which includes sync and setup
verdict       >= 0.95 supports;  <= 0.80 rejects and names a real code hotspot
              0.80 .. 0.95 is INDETERMINATE, not a weak support
```

**Stated prior, so the measurement can embarrass it:** I expect H1 to SUPPORT at ≥ 0.97, because 88
serial `cargo test` invocations at a 14.6 s mean is almost entirely compile-and-test work. **`%19`
independently predicted SUPPORT and by more than 0.97**, on the sharper ground that `run_crate` does
one `Command::new("cargo")` per crate while everything else in the loop is a `format!`, a `print!`
and an fsync — **88 fsyncs against 88 `cargo test` invocations.**

**Two agreeing priors are a reason to be more careful, not less.** They make H1 the comfortable
answer, so the indeterminate band above exists to stop a near-miss being read as confirmation.
**Writing the prior down is the point** — an unfalsifiable expectation is not a hypothesis, and `%19`
filed a prediction tonight that its own run refuted, which is the behaviour worth copying.

## What a completed hand-off looks like

```
DEFINE.md              this file
fingerprint.json       lane identity: worker, rustc, git sha, build profile, cargo flags
span_summary.json      88 rows: crate, wall_ms  -- Phase 4's output, the missing piece
hotspot_table.md       ranked by wall_ms, every row citing span_summary.json
scaling_law.md         serial 88x1 vs bounded-parallel; the SHAPE is the finding
hypothesis.md          H1 plus whatever the data raises, each supports/rejects with evidence
```

**Only then does `extreme-software-optimization` get invoked**, with targets scored
Impact × Confidence / Effort ≥ 2.0 and one lever per run.

## NO-CLAIM

This file establishes **no** performance fact. The one number in it — 1281 s — is a **single
observation** derived from two transport timestamps in a log, not a benchmark, and it is not a
percentile of anything. No fingerprint has been captured, no sampler has run, and the per-crate
distribution is **entirely unmeasured**: it is currently consistent with 88 crates at 14.6 s each and
equally consistent with 3 crates at 400 s and 85 at 1 s. **Those two worlds have completely different
optimizations and nothing here distinguishes them.**
