# `loop-driver` trio grade — `qfw0` / `ciab` / `80f8`

> **RULING LANDED 2026-09-07 by `%6`. Read this block before citing anything below as a
> recommendation** — every "Recommended disposition" in this file has since been ruled on, and a
> stale recommendation licenses re-litigating a settled question.
>
> |bead|ruling|state now|
> |---|---|---|
> |`qfw0`|WIDEN, strike "joined", stays OPEN|`open`, description 4237 B, **acceptance 6095 B where there was NONE**|
> |`ciab`|**CLOSED WONTFIX** by `%6`, premise refuted, routing adopted verbatim|`closed`, reason starts `WONTFIX - premise REFUTED`|
> |`80f8`|RETITLE to the switch, `[A2]` amended, stays OPEN|`open`, retitled, `[A2]` correction recorded|
>
> **Dedup was REFUSED.** `%6` had asked for a dedup recommendation and would have taken one; the
> disjoint-fix-site argument is what stopped a close that would have retired an unfixed defect.
>
> **Two things outlive this grade.** (1) The `qfw0` result corrects `AGENTS.md`, not just a bead:
> the asupersync contract's blanket *"Region-owned tasks. No detached tasks."* **conflates
> ownership with cancellability**, and this site needs the first without the second. That doctrine
> edit is `%6`'s. (2) The freeze reading below is **WRONG and is corrected in place** — see the
> `Was a new finding needed?` section.
>
> **One residual, deliberately NOT actioned:** `qfw0`'s title still reads *"is a detached Cx-free
> watchdog"*, which now names the DESIRED property as the defect — the same title-asserts-the-wrong-
> subject class that got `80f8` retitled. Flagged for `%6` rather than retitled unilaterally, on the
> `jix1` precedent that a retitle is the orchestrator's call.

Grader: pane `%19` (PearlGate, claude). Auditor: `%8` (WildStone, codex). Filer of `80f8`: `%6`.
Acceptance amender: `%20`. Non-author of all three, different model lineage from `%8`.
Date 2026-09-07. **Source census; no build required and none run.**

## Tree read — which tree every figure below comes from

```
git rev-parse HEAD                                  b6b07325335842e77aec0a39037ffc3fc2be489a
git rev-parse HEAD:crates/loop-driver/src/lib.rs    f90faf7ba1acc537c34945d2dc1e1f953cc22f29
git rev-parse HEAD:crates/loop-driver/src/main.rs   3f451d7f81752580778a0994fd6ffe29c2d8b2cf
git status --porcelain -- <both files>              (empty)
wc -l                                               1829 lib.rs, 285 main.rs
```

The two subject files are **clean at HEAD**, so `WORKTREE == TREE b6b0732` *for this subject* and
the read is unambiguous. That matters because the checkout carries ~100 dirty files overall; the
pin is what makes this grade a claim about a tree rather than about "the repo right now".
`%8`'s pin and line counts reproduce **exactly**.

## Census re-derived independently

Instrument note: the disclosed `rg -c … | wc -l` trap is real and I avoided it — `git grep -c`
suppresses non-matching files, so an absent needle prints **nothing** rather than a `path:0` row
that `wc -l` would count. Positive control run first: `git grep -c "fn "` returns 75 + 4 rows.

|figure|`%8`|mine|
|---|---:|---:|
|loop constructs|18|**18**|
|`cx.checkpoint()`|1|**1**|
|`&Cx`|3|**3**|
|`thread::spawn`|2|**2**|
|`.join()` anywhere in crate|0|**0**|

**How the 18 decomposes**, which is worth recording because a line-anchored enumeration finds only
17: `git grep -nE '^[[:space:]]*(loop \{|for .*in |while )'` yields **17**, and the 18th is
`lib.rs:1636` `let (status, stdout) = loop {` — **mid-line**, so it is invisible to a start-anchored
pattern. That 18th is precisely `%8`'s single *delegated-checkpointed* loop. The partition sums:
`0 direct + 1 delegated + 16 bounded-safe + 1 actionable = 18`.

**My own instrument error, recorded.** My first enumeration used `^\s*` and returned **0 rows**.
POSIX ERE has no `\s`, so the pattern could not match anything. I caught it only because the
per-kind counts above were nonzero — i.e. a *positive control disagreed with the enumeration*.
Without that control I would have published "0 loop constructs" from a working file. Same family as
the `rg -c` trap the acceptance warns about: **the instrument produced the reading, not the
subject.**

### The 16 bounded-safe, spot-checked on the two whose bounds are NOT self-evident

The line is drawn on **boundedness of input**, not volume — confirmed:

- `lib.rs:119` `loop {` — walks `current.parent()` upward looking for `.git`/`.beads`; terminates
  via `let Some(parent) = … else { return Err(…) }` at the filesystem root. Bounded by path depth.
- `lib.rs:622` `while let Some(p) = stack.pop()` — a process-tree walk, which is the shape that
  *could* cycle. It cannot: `:623` is `if !seen.insert(p) { continue; }` against a `BTreeSet`, so
  each pid is visited once and the bound is the finite pid set.
- The remainder are explicit: `:568` `depth < 64`, `:1054` `0..12`, `:1116` `1..=3`, `:1812`
  `Instant::now() < wait_deadline`, `main.rs:72` argv, `:132`/`:1533`/`:1564` finite collections.

`%8`'s 16 **holds**.

---

## `qfw0` — CONFIRMED, and its prescribed remedy is REFUTED

Cited site resolves: `lib.rs:775-786`, `arm_wall_watchdog`, `std::thread::spawn` at `:777`, no
handle returned, and `.join()` = 0 crate-wide. Detached: **confirmed.**

### The bead's WHY is confirmed at a site the bead does not cite

`qfw0`'s WHY: *"a caller can return while the detached task still owns those effects."* The cited
line `:777` cannot demonstrate that on its own — a spawn site says nothing about its callers. So I
enumerated them:

```
lib.rs:1266   arm_wall_watchdog(   <- inside pub fn run_live
main.rs:205   arm_wall_watchdog(   <- Mode::HoldLock
main.rs:231   arm_wall_watchdog(   <- Mode::HoldLockWorking
```

**Three arming sites, not two.** `run_live` (`lib.rs:1251`, `pub`) arms at `:1265-1271` and then
returns a `LoopDriverRunOutput` to its caller. Between the arm and the end of the function there
are **at least 8 `return` statements** (`:1275, 1290, 1307, 1322, 1333, 1353, 1363, 1380`). Every
one of them hands control back to a caller while the detached thread still owns a timer, log-file
IO, `terminate_process_tree(pid)`, and `std::process::exit(EXIT_DEADLINE)`.

That is qfw0's hazard, exactly as written, at `lib.rs:1265` — **and it is a different shape from
the `main.rs` arms**, where the arm is immediately followed by `sleep(seconds)` then
`ExitCode::SUCCESS` in the same match arm, so the process exits and the thread dies with it.

### The known-good as written is IMPOSSIBLE, not merely undesirable

`qfw0` asks for *"a bounded, **joined**, region-owned watchdog."* The thread's terminal statement is
`std::process::exit(EXIT_DEADLINE)` at `:784`. **It never returns.** A `join()` on that handle can
therefore never complete: it blocks until the process dies, at which point there is nobody to
observe the join. Joining is not a stricter discipline here, it is a deadlock.

And region-ownership has a second, worse problem: a region exists so a cancellation can reach its
children. **A deadline enforcer that a `Cx` cancellation can stop is not a deadline enforcer.** The
watchdog fires precisely when the rest of the process is wedged — which is the moment a cooperative
cancellation is least likely to be serviced. Making it cancellable would defeat the property it
exists to provide.

So the counter-argument in the dispatch — *joining would block on the deadline being enforced* — is
**correct, and stronger than stated**: it is impossible, not merely awkward.

### What the actual fix is

Narrower than either the bead or the contract implies: `arm_wall_watchdog` should hand back a
**disarm handle** (an `Arc<AtomicBool>` the sleeping thread re-checks, or a channel `recv_timeout`
instead of a bare `sleep`), so a caller that returns early can disarm what it armed. That removes
*"outlives its owner"* while keeping enforcement uncancellable-by-`Cx`. Ownership without
cancellability is the property this site needs, and the contract's blanket
*"no detached tasks"* does not distinguish the two.

**Recommended disposition: STAYS OPEN.** Widen the cited sites to include `lib.rs:1265`
(`run_live`), and strike **"joined"** from the known-good — an acceptance that demands an
impossible artifact cannot be satisfied, and will be routed around.

---

## `ciab` — premise REFUTED. The burner is bounded twice.

`ciab` claims *"an unbounded detached thread"* whose *"lifetime is not bounded by a region."*

**The second clause is TRUE. The first is FALSE, and they are not the same claim.**

Read `main.rs:216-239` as a whole:

```
221   std::thread::spawn(|| { loop { n = n.wrapping_add(1); black_box(n); } });
230   if lock_rules.wall_bound { arm_wall_watchdog(...) }
237   std::thread::sleep(Duration::from_secs(seconds));
238   ExitCode::SUCCESS
```

Two independent bounds, and **neither is a region**:

1. **Process lifetime.** `:237` sleeps for the argv `seconds` (`Mode::HoldLockWorking(u64)`, set
   from argv at `main.rs:94`), then `:238` returns `ExitCode::SUCCESS`. The process exits and the
   burner dies with it. This bound holds **even with the wall bound disabled**.
2. **The wall watchdog** at `:230`, which is what `80f8` is about.

So the burner's lifetime is `≤ min(argv seconds, wall_bound_secs)`. **Bounded-by-process-lifetime
is bounded.** `%8` reached the same correction against its own finding — that
`80f8`'s *"ONLY bound"* phrasing is too strong because `:237` also bounds it — and that correction
is right.

The comment at `:218-220` documents the thread as a deliberate anti-vacuity fixture that *"must
stay LIVE even if pgrep misses a descendant"* — the same doctrine this repo applies to its own
gates (*an empty scan set is an ERROR, never a pass*). Region-owning it, so that a cancellation
could stop it, would **defeat the fixture**: a liveness probe that can be cancelled proves nothing.
`ciab`'s known-good (*"region-owned termination"*) is wrong for the same reason as `qfw0`'s.

Per `kvsq` condition 2 — *"Nothing but uniformity is an ACCEPTED verdict and is preferred over an
invented defect"* — this is that case.

**Recommended disposition: WONTFIX-as-REFUTED, with the refutation recorded.** Not a dedup close;
see below. I am not closing it: `%6` reserved the ruling on this pair.

---

## `ciab` vs `80f8` are NOT duplicates — a dedup close would DISCARD a live defect

`%8`: *"ciab and 80f8 are two views of the same burner."* They share a **subject**. They do not
share a **defect**, and they do not share a **fix site**:

|bead|defect|fix lands at|
|---|---|---|
|`ciab`|the spawn is detached / unowned|`main.rs:221`|
|`80f8`|the disable switch ignores its VALUE|`lib.rs:236`|

Fixing `lib.rs:236` leaves `main.rs:221` byte-identical, and vice versa. Closing either as a
duplicate of the other would retire a defect nobody fixed. **Recommendation: do not dedup.** The
correct outcome is `ciab` WONTFIX on its own refuted merits and `80f8` open for its fix — which
reaches the same *count* as a dedup by a route that discards nothing.

---

## `80f8` — CONFIRMED, mechanism exact, title overstated, and `[A2]` needs correcting

Mechanism reproduces at source:

```
lib.rs:236   let disabled = std::env::var_os("LOOP_DRIVER_DISABLE_WALL_BOUND").is_some();
lib.rs:250   wall_bound: !disabled,
```

`is_some()` tests **presence**. `=0`, `=false`, `=no`, `=off` and the empty string all yield `Some`,
so all of them **disable** the bound. Static reading; I did not execute the repro loop (no build
was required for this grade and I ran none — see NO-CLAIM).

### New evidence, and it is decisive: the correct pattern is three lines below the defect

`LockRules::from_env` handles **three** environment variables. The other two **parse their values**:

```
236  wall_bound        var_os(...).is_some()                                   <- PRESENCE
237  wall_bound_secs   var(...).ok().and_then(|v| v.parse().ok()).unwrap_or(D).max(1)
242  liveness_gap_ms   var(...).ok().and_then(|v| v.parse().ok()).unwrap_or(D).max(50)
```

This is not a style objection about `is_some()`. It is an **internal inconsistency inside a single
function**, where the correct idiom is demonstrated on the adjacent line. That upgrades the finding
from "a sharp edge" to "one of three siblings is wrong", and it also tells the implementer exactly
what shape the fix should take.

### Two corrections to the bead and its acceptance

- **The title's *"is unbounded when … set to ANY value"* is OVERSTATED.** Process lifetime still
  bounds the burner (see `ciab` above). What the env var removes is the *watchdog*, not every
  bound. `%8`'s correction stands.
- **`[A2]` is HALF RIGHT and its inference is wrong.** `%20` measured two guard sites,
  `main.rs:204` and `:230`, and concluded *"ONE presence-test variable gates TWO bound paths, so
  the finding is BROADER than filed."* The two sites are in **different match arms**:
  `:204` is `Mode::HoldLock` — which has **no burner at all**, only a finite argv `sleep`.
  `:230` is `Mode::HoldLockWorking` — which has the burner.
  So exactly **one** site gates the burner. And there is a **third** arming site `%20` did not
  find: `lib.rs:1266` in `run_live`. The breadth is real but it is in the **watchdog's** reach —
  all three modes lose their deadline enforcement from one presence test — not in the burner's
  boundedness. A repro driving only `:230` is sufficient for the burner and insufficient for the
  watchdog.

**Recommended disposition: STAYS OPEN for the fix.** Narrow the title to the switch, and amend
`[A2]` to name three arming sites with only one gating the burner.

---

## Was a new finding needed? `%8`'s conclusion is right; its reasoning is incomplete.

`%8` filed none, reasoning *"these existing findings cover the actionable surface."*

**The conclusion is correct and the coverage is not currently there.** No bead cites `lib.rs:1265`,
the site that actually demonstrates `qfw0`'s stated hazard. `qfw0` cites the *spawn* (`:777`),
which cannot show a caller returning. So coverage exists only **after `qfw0` is widened** — which
is an amendment to an existing bead, not a new finding. Same defect, same class, second site.

**And routing one through the `Finding` kernel is currently impossible anyway**, which is worth
recording separately: `crates/finding` is a library whose `src/bin/` directory exists, contains
**0 files**, and is **untracked** — which is why `cargo metadata` reports no bin target for it. So
there is no runnable filing path, and the amendment route is the correct one here.

### RETRACTED IN THE SAME PASS — my freeze citation was wrong TWICE

The sentence originally here read: *"Adding one is a new binary target under a `CONTRACT.md:54-58`
BUILD FREEZE."* **Both halves are wrong, and `%6` caught it before it cost anything.**

1. **Wrong path.** There is **no root `CONTRACT.md`** in this repo:
   `git ls-files | grep -cE '^CONTRACT\.md$'` → **0**. The block lives at
   `docs/plan/flow/CONTRACT.md`. A bare `CONTRACT.md:54-58` resolves to nothing.
2. **Wrong reading, and I stopped one line early.** The freeze is at `:58`. Three lines later,
   `:61` reads **"⛔ SUPERSEDED IN PART — DO NOT STOP READING HERE"**, and the anchored grep it
   prescribes resolves to `:154` — **"S1 IS AUTHORIZED TO BUILD. S2–S9 REMAIN FROZEN."**

```
58   **BUILD FREEZE.** No new crate, no new feature bead, no install, until …
61   > ⛔ **SUPERSEDED IN PART — DO NOT STOP READING HERE.** The freeze above is LIFTED FOR S1 …
67   > grep -nE '^\*\*S1 IS AUTHORIZED TO BUILD' docs/plan/flow/CONTRACT.md
154  **S1 IS AUTHORIZED TO BUILD. S2–S9 REMAIN FROZEN.**
```

**The document records this exact failure as having already happened once** — an orchestrator read
"BUILD FREEZE" in bold, stopped, told the fleet all code was blocked, and routed panes to audits
while 131 authorized S1 beads sat claimable. **I reproduced the reading; the `:61` pointer exists
because of it, and it worked.** A superseding amendment downstream of the text it supersedes is
invisible to a reader who stops at the bold STOP — which is why the pointer must be adjacent, and
why a line-numbered citation into a living document is a staleness substrate in its own right.

**The substantive concern survived the retraction**, which is the only reason it was worth raising:
`%6` checked before ruling and found the amendment authorizes builds wired to an S1 layer gate and
still freezes S2–S9, and the finding-kernel repair (`2sx1`, P0, `%20` building) is in **neither**
list. Ruling: it proceeds as **phase-0 kernel repair and does NOT install** — build and test only.
**Being right about the risk does not excuse citing the wrong line for it.**

---

## NO-CLAIM

- **This is a static source census.** I ran **no build and no test** for this grade, so nothing
  here is a runtime claim. `80f8`'s repro loop over `0/false/no/""` is **UNRUN by me** — the
  mechanism is read from source, which establishes what the code *says*, not what a live
  `HoldLockWorking` process *does*.
- **The `run_live` hazard is a reachability claim about source, not an observed leak.** Whether any
  caller of `run_live` does enough work after it returns for the watchdog to fire against an
  unintended target is **UNMEASURED**. In `main.rs` the pattern is arm-then-exit, which is benign;
  `run_live` is `pub`, so the hazard is available to an embedder. I did not enumerate every
  `run_live` caller outside this crate.
- **I did not audit the control-plane copies.** `%8` states it checked them separately and that
  they are not the evidence tree; I take that as its claim, not my measurement.
- **`.join()` = 0 is crate-scoped** (`crates/loop-driver/src/{lib,main}.rs`). It is not a claim
  about the workspace.
- **I closed nothing.** Nothing in the trio is implemented, so no `MUTATION-VERIFIED`/`DONE` is
  available; `ciab`'s refutation is closable but `%6` reserved that ruling.
