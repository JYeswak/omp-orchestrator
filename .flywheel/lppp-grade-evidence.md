# `lppp` grade — platform-scoping the Darwin group-kill/reap tests

Grader: pane `%19` (PearlGate, claude). Implementer: `%7` (WildStone), commit `0622849`. Authorship
per `%6`'s routing statement, since `created_by` carries no usable provenance. Date 2026-09-07.
All `br` writes for this grade pass `--actor PearlGate`.

## VERDICT: APPROVED — all nine items met exactly as written.
## AND THE ACCEPTANCE'S OWN PREMISE IS FALSE AT HEAD. That is a residual on the RULING, not on `%7`.

## Tree

```
0622849                            3 files: Cargo.toml, src/lib.rs, tests/platform_scope.rs
git diff --check                   clean
git log --full-history 0622849..HEAD -- <paths>    2bd4e99 only
  2bd4e99 = "drop the exec bit from 84 Rust source files"
  content diff 0622849..HEAD for both subject files: EMPTY  -> MODE ONLY
mode now                           100644 both (was 100755)
worktree                           clean
```

`2bd4e99` is the fix for the `100755` write-tool artifact I flagged while grading `f2sh`. It is
mode-only for these paths, so every figure below describes `0622849`'s content.

Pre-mutation sha of `src/lib.rs`: `a9664cb72269b3266b7f9911`.

## Items 1–9

|item|verdict|how I established it|
|---|---|---|
|1 title/authority|**MET**|title is test-scoping, no P0/JOSHUA prefix; no control-plane worker config touched (item 9 file list)|
|2 four tests remain real|**MET**|all four present as `#[test]` fns at `lib.rs:524/623/695/735`|
|3 shared typed helper, no cfg deletion|**MET**|see "leg 1" below — this is the load-bearing one|
|4 harness-free reporter|**MET**|`harness = false` at `Cargo.toml:21`; on-lane `exit=20` with the full marker|
|5 macOS must not short-circuit|**MET (static)**|guard is an early `return`; the real body sits *below* it, untouched. Grandchild message preserved at `:754`|
|6 positive control retained|**MET**|`fast_child_completes_with_output_and_status … ok` in the on-lane run|
|7 the two verification commands|**MET**|both re-run by me on-lane, figures below|
|8 retry condition|**MET as written**|but the token is inert — see "leg 5"|
|9 no unrelated changes|**MET**|3 files, `git diff --check` clean|

## LEG 1 — the `cfg!` vs `#[cfg]` claim, which is the whole design

```
#[cfg(target_os   occurrences in crates/subprocess-contract/{src,tests}   0
cfg!(target_os    lib.rs:452, tests/platform_scope.rs:13                  2
#[ignore]         occurrences in the crate                                0
```

**Confirmed: nothing is deleted at compile time and nothing is ignore-only.** `cfg!` is a macro that
evaluates to a `bool`, so **both arms are compiled and type-checked on Linux** — which is exactly the
coverage property `#[cfg]` would have destroyed invisibly.

**And I did not take that on structure alone — I proved the bodies are live code by executing
them.** See the mutation: flipping the gate made all four bodies run on Linux, and the suite's
runtime went `0.76s → 3.15s`. Dead code cannot take three seconds. That is the strongest available
evidence that they are compiled, reachable, and intact.

`%6`'s grep for `#[cfg(target_os` returning **0** and reading as "no platform routing" is corrected:
the zero is real, and the macro form is the answer.

All four tests use one identical guard:

```rust
if group_kill_platform_verdict("<test name>") == GroupKillPlatformVerdict::UnmeasuredOnThisPlatform {
    return;
}
// real body follows, unchanged
```

One shared helper, one early return, body below. That is what makes item 5 true by construction on
macOS without a macOS run: the guard goes false there, and nothing between it and the body changed.

## LEG 3 + the two verification commands, re-run on-lane

```
RCH_WORKER=contabo-3 … RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p subprocess-contract --test platform_scope
  REMOTE exit=20  contabo-3  bypass=0
  UNMEASURED_ON_THIS_PLATFORM property=process_group_kill_and_reap platform=linux tests=4 \
    exit_code=20 retry_if=darwin-production-group-kill-reap-defect

… rch exec -- cargo test -j 2 -p subprocess-contract --lib -- --nocapture
  REMOTE exit=0   contabo-3  bypass=0     test result: ok. 12 passed; 0 failed
  UNMEASURED lines: 4   named tests: exactly the four in item 2, no fifth, none missing
  positive control fast_child_completes_with_output_and_status … ok
```

**Rule 7b satisfied: message AND code.** `verdict_code=20` / `exit_code=20` is distinct from every
other exit this crate produces, and the marker text names the property, the platform, the test, the
count and the retry label — so a reader discriminates `UNMEASURED` from `FAILED` from the exit alone
*and* from the text. Neither pinned alone.

**`harness = false` is load-bearing and correct.** libtest captures stdout for a **passing** test, so
a printed marker is invisible in a green run without `--nocapture`. That is the finding I recorded
in `mirror_oracle.rs:147-153` while grading `pre-delete-citation-check`, and `%7` applied it
prospectively. The custom harness is what makes the operator-facing verdict **unconditionally
visible**, and it also supplies the process-level exit code a captured `println!` cannot.

## LEG 4 — the mutation, designed to REACH the assertions

Per the refinement I contributed on `f2sh` — a mutation that makes the guarded call *succeed* never
reaches the message assertions, so it proves the guard exists, not that the message bites. So I
mutated the **platform gate** rather than the marker, forcing Linux down the *verified* path so the
four real bodies execute:

```
lib.rs:452   if cfg!(target_os = "macos")  ->  if cfg!(target_os = "macos") || cfg!(target_os = "linux")

MUTATED    REMOTE exit=0  12 passed; 0 failed   UNMEASURED lines: 0   runtime 3.15s
RESTORED   sha a9664cb72269b3266b7f9911 byte-identical, git status clean
           REMOTE exit=0  12 passed; 0 failed   UNMEASURED lines: 4   runtime 0.63s
```

The typed line **vanished 4 → 0**, which is the required "must change or vanish", and the mutation
reached and executed the real bodies. **Both halves of leg 4 satisfied.**

## THE FINDING — the premise is stale, and the mutation is how I know

The mutation answered a second question I did not expect to ask. **With the gate flipped, all four
group-kill/reap bodies PASS on Linux.** Not error, not skip — pass, in 3.15s of real child-spawning
work.

The helper's own justification, `lib.rs:447-450`:

> *"Darwin is the shipped runtime for this process-group contract, but the reachable Rust lane is
> Linux and **its group-kill primitive is known to be broken**."*

**That sentence was true and is now false.** It describes the `o3eb` defect — six `/bin/kill` sites
passing a negative pgid with no `--` separator, a silent no-op under procps-ng — which I graded and
closed myself. Dating it:

```
e4c9138  2026-09-06T17:31:06-06:00  fix(process): pass separator before negative pgid
0622849  2026-09-07T12:14:59-06:00  test(subprocess-contract): scope group-kill checks by platform
"--" separator occurrences in lib.rs at HEAD: 4
```

**The fix predates the scoping by nineteen hours.** So the stated reason for declaring these four
UNMEASURED on Linux no longer holds at the commit under grade.

### And there is stronger evidence that needs no mutation at all

`process_count_returns_to_baseline_after_contract_runs` (`lib.rs:795`) is **unscoped**, runs on
Linux, and is green in the 12. What it does:

```
:816-821  spawn `sleep 30` with a 250ms deadline, assert BoundedOutcome::TimedOut   <- invokes the
                                                                                       timeout KILL
:823-838  poll descendants; panic "subprocess contract left descendants" if any leak <- the REAP half
```

**It exercises the same `process_group_kill_and_reap` property, on Linux, unscoped, and it passes.**
So at HEAD the crate simultaneously emits four typed lines saying *this property is unmeasured on
this platform* and runs a fifth test that depends on that property and measures it green. That is an
internal inconsistency in the shipped state, and it required no mutation to see.

### Why this is APPROVED anyway, and where the residual belongs

Item 3 **mandates** this behaviour, and `HD-0045` is named as the authority. `%7` implemented the
ruling faithfully and precisely — the mechanism is better than the acceptance demanded (`cfg!` over
`#[cfg]`, a real harness-free reporter, one shared helper). **The staleness is in the premise the
ruling rests on, not in the implementation of it.** Grading `%7` down for it would be grading the
implementer for a decision made above them.

**There is also a still-valid reason for the same behaviour, and it is a different one:** Darwin is
the shipped runtime, and a Linux pass does not verify the Darwin property. That argument survives
`o3eb` intact. Item 7 already says *"Neither is evidence that group kill/reap works on Linux"* — which
my mutation shows is now understated in the interesting direction: Linux **can** supply that
evidence, and the suite already does, via the fifth test.

**Recommended follow-up (not filed by me — `%6`'s routing call):** correct the helper's doc comment
to the surviving justification, and decide whether the four Linux legs should be promoted from typed
UNMEASURED to a genuine platform-scoped PASS, given that they pass and a sibling test already
depends on their property. That is a decision, not a defect — which is why I am naming it rather
than refusing the bead.

## LEG 5 — `retry_if` is an inert string

```
git grep -l "darwin-production-group-kill-reap-defect"
  .beads/issues.jsonl                                 <- the bead text
  crates/subprocess-contract/src/lib.rs               <- EMITS it
  crates/subprocess-contract/tests/platform_scope.rs  <- EMITS it
```

**Nothing consults it.** No workflow, hook, crontab, gate or crate reads the token; the only
non-bead occurrences are the two sites that print it.

**Item 8 is nonetheless MET as written**, and I want the distinction exact: item 8 states a **human**
retry condition — *"revisit … if a defect is observed in production on Darwin, or when a schedulable
Darwin Rust lane is intentionally established by a separate decision."* Both triggers are human
judgements. Items 3 and 4 require the marker to **carry the field**, which it does, and nothing in
the acceptance requires a runner to consume it.

**The residual is the shape, not the compliance:** `retry_if=<token>` looks machine-consumed. A
reader may reasonably infer an automated retrigger exists. It does not — the token's real value is
that it makes the condition greppable when the human condition fires. Worth one sentence in the
doc comment; not a gap in this bead.

## Lane

Every figure ON-LANE: `Remote command finished: exit=` present and the `RCH BYPASS` banner absent in
each log, checked per run. **Verdict work only — no triple passed**, per the corrected binding.

`RCH_REQUIRE_REMOTE=1` on every invocation. I also implemented `%7`'s own clause: on an
`[RCH-I005] project_excluded` refusal, query `rch queue --json` for a live row **on my project** and
unpin if there is none, because that is stale state rather than contention. It did not fire this
grade — contabo-3 admitted on the first attempt for all four runs.

## NO-CLAIM

- **NOTHING HERE IS EVIDENCE ABOUT DARWIN.** Every run is Linux on contabo-3. Item 5 — that the real
  bodies execute unchanged on macOS — is established **statically** (the guard is an early return
  above an untouched body) and is **not** executed. A passing macOS leg is not claimed, per the
  acceptance's own prohibition.
- **The mutation shows the four bodies pass on Linux at HEAD. It does not show the Darwin property
  holds**, and it does not show they passed before `o3eb` — I did not run them on a pre-`e4c9138`
  tree.
- **`retry_if` reachability is a static scan** of tracked files plus the repo's workflow/hook
  surfaces. I did not enumerate every executor on this machine, so "nothing consults it" is scoped
  to what a repo-wide grep can see.
- **`created_by` was not used to establish authorship.** `%7` is named as implementer by `%6`'s
  routing statement; 32.9% of this repo's `created_by` values read `josh` and are not evidence.
- The bead was released to `pane=%19;agent=PearlGate` before I closed it; the author held it until
  then, and two panes correctly refused to force a second holder.
