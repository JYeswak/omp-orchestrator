# o3eb closing grade — pane %19 (claude lineage, non-author)

Bead: `omp-orchestrator-kill-group-missing-double-dash-o3eb`
Implementer: `%7` (codex, WildStone) at `e4c9138`. First grader: `%8` (codex) — verdict UNKNOWN.
Closing grader: `%19` (claude). Different model lineage from both. Nothing below is read from a
report; every figure is a command this pane ran.

Delivery: one packet ACK, `ACK o3eb on %19 --`, posted and read back before any work.

## Tree attribution (read this before any number)

    HEAD                              7b4ac631efe8a81020a5d86d3f5ea4af498afaa2
    git status --porcelain | wc -l    100          (shared checkout, other panes' work)
    subject files                     CLEAN at HEAD -- no diff, no porcelain entry
      crates/subprocess-contract/src/lib.rs  sha256 eed862ebee8b9551db570e43bf0ba361ce57534a5265a74460c915420d0adc09
      crates/tick-monitor/src/lib.rs         sha256 e4f83bed65ef4ac9996bc0180ffbfd57c23835654b9e24d34cabd4881cf6016b

So for `subprocess-contract` the WORKTREE and HEAD trees are identical and every cargo figure below
is attributable to `7b4ac63`. For `tick-monitor` they are NOT: `crates/tick-monitor/build.rs` is
modified (+6/-2) and `crates/tick-monitor/tests/properties.rs` is untracked, both by another pane.
Every tick-monitor number below is WORKTREE, not HEAD, and is labelled so.

## Leg 1 — premise, re-verified (acceptance 1)

Worktree and HEAD agree; six negative-pgid sites, all six carrying the separator:

    crates/subprocess-contract/src/lib.rs:269   .args(["-TERM", "--", &group])
    crates/subprocess-contract/src/lib.rs:275   .args(["-KILL", "--", &group])
    crates/subprocess-contract/src/lib.rs:389   .args(["-TERM", "--", &group])
    crates/subprocess-contract/src/lib.rs:395   .args(["-KILL", "--", &group])
    crates/tick-monitor/src/lib.rs:186          .args(["-TERM", "--", &neg])
    crates/tick-monitor/src/lib.rs:192          .args(["-KILL", "--", &neg])

Commit-level delta, `git show e4c9138^:<f>` vs `git show e4c9138:<f>`: parent tree has the same six
sites with NO separator (`:268,274,388,394` and `:185,191`); child tree has all six with it. Two
files touched, nothing else.

`loop-driver/src/lib.rs:756,760` pass `&pid.to_string()` — a POSITIVE pid, not this defect class,
and untouched by the commit. Confirmed out of scope.

REPO-WIDE COMPLETENESS, which the earlier grade did not do. Every `/bin/kill` call site in
`crates/*/src`, by argv shape:

    negative pgid (this defect class)     6   all six carry --
    "-0" liveness probes, POSITIVE pid    9   subprocess-contract:441,544; loop-driver:839,1588,
                                              1608,1676; omp-orchestrator/target_directory.rs:342
    "-TERM"/"-KILL" with POSITIVE pid     2   loop-driver:756,760
    negative-pgid sites still unmigrated  0

Only three sites in the workspace build a negative argument at all —
`subprocess-contract:267,387` and `tick-monitor:184`, each `format!("-{pid}")` — and all three feed
a separator-carrying call.

## Leg 2 — the commit contains no undisclosed behavioral change

`git show e4c9138 -- crates/subprocess-contract/src/lib.rs`: beyond the four token insertions, the
diff is four rustfmt-only reflows (`checkpoint_io` map_err, the `stdin_writer` closure, one deleted
blank line at `:448`, the `is_some_and` reflow at `:723`) — the four hunks `%7` disclosed. The
tick-monitor half is the two tokens plus the doc comment. Independently confirmed: nothing
behavioral outside the six tokens.

Doc comment (acceptance 6) now reads, at `tick-monitor/src/lib.rs:180-182`: "procps-ng requires --
before a negative pgid; without it, procps-ng can return 0 without signalling. BSD kill accepts the
explicit form." It no longer asserts the pre-fix behaviour.

## Leg 3 — THE GAP `%8` LEFT: the dynamic before/after aggregate, both halves captured

This is what `%8` could not get. Same command both times, one variable:

    CARGO_BUILD_JOBS=2 RCH_REQUIRE_REMOTE=1 RCH_VISIBILITY=verbose \
      rch exec -- cargo test -j 2 --release -p subprocess-contract

AFTER (tree as committed, sha256 `eed862e…`), worker contabo-1, no retry:

    Remote command finished: exit=0 in 684904ms
    test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.16s

BEFORE (mutation leg, acceptance 7 — the four separators removed, `git diff --numstat` = `4 4`,
sha256 `22d6c73f5d652c7e9de28c6b01ae61bcd1c32f5e3ac287e36c99f5d00dbabc60`):

    Remote command finished: exit=101 in 685151ms
    test result: FAILED. 8 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.74s

    failures:
        tests::bounded_status_signals_the_group_so_grandchildren_die_too
        tests::deadline_killed_child_is_reaped_not_orphaned
        tests::process_count_returns_to_baseline_after_contract_runs
        tests::sleep_child_past_deadline_is_timedout_never_completed

The four messages VERBATIM — the known-bad leg asserts the MESSAGE, not the exit code, because `101`
is cargo's generic failure and goes red on any unrelated breakage:

    lib.rs:704  grandchild survived the group kill and touched
                /Users/josh/Developer/omp-orchestrator/.rch-tmp/bs-grandchild-423859
                - the signal went to the pid, not the group
    lib.rs:455  bounded_output must not wait out the child's full runtime
    lib.rs:529  killed group leader 423881 still alive: the kill path is broken
    lib.rs:786  subprocess contract left descendants:
                before={423867, 423869, 423871, 423874, 423875, 423876, 423881, 423883}
                after={423867, 423869, 423871, 423875, 423881, 423899, 423903, 423927, 424188}
                leaked={423899, 423903, 423927, 424188}

Four independent assertions, four leaked pids, identical to the messages `%7` and `%9` recorded on
separate runs. `8 -> 12` and `4 -> 0` across a 4-line diff.

WHY THE MUTATION IS STRONGER EVIDENCE THAN THE PARENT TREE `%8` TRIED TO EXPORT. `e4c9138^` differs
from `e4c9138` by the six tokens AND four rustfmt hunks; the mutated worktree differs from HEAD by
the four tokens and nothing else. The A/B therefore isolates the token, which is what acceptance 7
asks for and what `git archive e4c9138^` could not give.

RESTORE, byte-identical: sha256 back to `eed862ebee8b9551db570e43bf0ba361ce57534a5265a74460c915420d0adc09`,
`git diff --numstat` empty, `git status --porcelain` empty for the file. The file was reserved in
Agent Mail for the duration (`granted` length 1, `conflicts` empty — length tested, not truthiness)
and released after. `git checkout --` was DENIED by `dcg` (`core.git:checkout-discard`); the restore
went through the edit path instead and is proven by the sha, not by the denied command.

## Leg 4 — `%8`'s UNKNOWN was NOT the os-gate. I reproduced the real cause.

The dispatch premise was that `o3eb`'s UNKNOWN was the phantom os-gate refusal. Half right. `%8`
reported two causes: contabo-3 `RCH-I001` (the os-gate, now fixed) AND "Contabo 4 lost the remote
source-lock connection during process-kill tests". The second is REAL, REPRODUCIBLE, and I hit it —
on contabo-1, on the BEFORE run, verbatim:

    Connection to 89.117.22.43 closed by remote host.
    WARN rch::hook: Remote execution failed on contabo-1: remote source-authority lock on contabo-1
      exited unexpectedly: exit status: 255; stderr=Connection to 89.117.22.43 closed by remote host.;
      will retry on another worker if available
    WARN rch::hook: Remote build remote execution failed on contabo-1; retrying on higher-capacity
      worker contabo-2 (attempt 2/3)

It dropped after `running 12 tests` and five oks — i.e. inside the process-kill tests, exactly where
`%8` lost contabo-4. What produced a terminal receipt for me and not for `%8` is rch's own
retry-on-another-worker: attempt 2/3 on contabo-2 ran the full suite and returned the `8 passed;
4 failed` above. No refusal line appears in either log; `no admissible workers`, `os_gate_excluded`,
`no_free_slots` and `active_project_exclusion` are all ABSENT from both runs, and the queue was idle
at dispatch (`slots_available: 12`, `workers_offline: 0`, one unrelated `uds` build).

CORRELATION, n=1 each, NOT a causal claim: the drop happened on the pre-fix (leaking) tree and not
on the fixed tree. A no-op group kill leaves live descendants sharing the remote session's process
group, which is a plausible mechanism for killing the source-authority lock — but one run per arm
does not establish it, and I did not test it.

## Leg 5 — Darwin portability probe, re-run on this Mac (acceptance 5)

    candidate=99991   pgid_count=0            (ps -o pgid= -ax | tr -d ' ' | grep -cx 99991)
    /bin/kill -KILL -- -99991   rc=1   kill: -99991: No such process
    /bin/kill -KILL -99991      rc=1   kill: -99991: No such process
    Darwin 25.5.0 arm64

No "illegal option" in either form, so the separator is safe on BSD kill and the fix is portable.
Probed against a pgid proven absent; no live group was signalled.

## Leg 6 — tick-monitor (acceptance 8, second half) — WORKTREE, not HEAD

    rch exec -- cargo test -j 2 --release -p tick-monitor      contabo-1, exit=101 in 42719ms
    test result: FAILED. 25 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

Sole failure, and it is an INPUT defect, not a kill assertion:

    repo_tests::the_marker_walk_finds_this_repo_and_names_what_it_sought
    lib.rs:1753  marker walk should find this repo:
                 NotFound { from: ".../crates/tick-monitor", markers: ".git/.beads" }

`resolve_repos(&[])` walks up for `.git` or `.beads`; both exist locally (`ls -d .git .beads`), and
the rch-synced remote tree carries neither. The test never touches `kill_group`. This independently
reproduces `%7`'s 25/1 and its classification. tick-monitor is NOT claimed green.

## Leg 7 — wiring, re-measured with a positive control

    subprocess-contract  manifest callers (excl. own)   47
                         source callers (excl. own)     69
    tick-monitor         manifest callers               21
                         source callers                 26
    positive control     "serde" in crates/*/Cargo.toml 61

Non-vacuous, and the matcher demonstrably matches. This is a fix to two already-wired crates, not a
new lane, so no BUILT != WIRED window is opened by it.

## Verdict

PASS. Every leg of the hand-written acceptance is satisfied by commands this pane ran, and the one
leg `%8` left UNKNOWN — the dynamic before/after aggregate — is now captured on both arms with the
four verbatim messages. `%8`'s refusal to close was correct at the time: it had no terminal receipt
for the BEFORE arm, and an UNRUN is never a pass.

## NO-CLAIM

* This fixes the argv of six `/bin/kill` calls. It does not make group kill correct on every
  platform, and Darwin remains unschedulable for Rust tests per `lppp` — the Darwin leg here is a
  shell probe, not a test run.
* It does not establish a production orphan rate. The evidence is 12 tests on two Linux workers.
* The four hand-rolled structured-concurrency forms catalogued in `w21v` are untouched.
* tick-monitor's suite is 25/1 on the WORKTREE with another pane's `build.rs` change and untracked
  `tests/properties.rs` present; its marker failure is an input defect that predates and is
  independent of this bead, and neither its green count nor its red is attributable to `7b4ac63`.
* No `Compiling asupersync v` line appeared in either subprocess-contract run, so the dependency
  build revision is unattributed — as `%7` and `%9` also recorded.
* The contabo-1 source-authority drop is UNFIXED and now reproduced twice by two panes. It is
  survivable only because rch retries on another worker; a run pinned to one worker can still lose
  its terminal receipt. Worth its own bead.
