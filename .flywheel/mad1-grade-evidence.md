# `mad1` grade — sender identity on the dispatch renderers

Grader: pane `%19` (PearlGate, claude). Implementer: `%8` (WildStone, codex). Non-author of bead and
code, different model lineage. Commit `c294b7f`. Date 2026-09-07.

## VERDICT: APPROVED — all four legs met. Leg 2's on-lane green is UNMEASURABLE (`ln7k`), and I
## substantiated it with an INDEPENDENT ORACLE instead of re-running the implementer's fixture.

## Tree

```
c294b7f            8 files exactly, as claimed
  crates/omp-orchestrator/src/main.rs in commit        0   (came via 14b34f1)
  crates/refill-idle-panes/tests/differential.rs       0   (peer file, excluded)
5 commits c294b7f..HEAD, and NONE touched mad1's 8 paths (empty --name-only list)
git status on the 8 subject paths                      clean
```

**Provenance caveat, stated because it changes what a `cargo` figure means:** the *crate*
`agent-mail-native` has **7 dirty peer files** (`client.rs`, `endpoint.rs`, `error.rs`, `journey.rs`,
`oracle.rs`, `tests/live_journey.rs`, `tests/packet.rs`). `src/identity.rs` and `tests/identity.rs`
— the files this bead is about — are clean, but any `-p agent-mail-native` build compiles the peer
edits. My identity-target figures are therefore **WORKTREE** figures, not tree figures; the static
oracle below is the one that speaks about `HEAD`.

## Leg 2 — the substitution `%8` made is CORRECT, and the acceptance's prediction was stale

The acceptance predicted the allowance would shrink to `kernel-only-operator-hook` as the sole row.
`%8` replaced it with `dispatch-saga/src/m2.rs`. **Verified at HEAD, with a positive control:**

```
crates/kernel-only-operator-hook/src/lib.rs      0 dispatch-flag occurrences
crates/kernel-only-operator-hook/src/main.rs     0
crates/kernel-only-operator-hook/src/shadow.rs   0
POSITIVE CONTROL crates/omp-idle-dispatch/src/main.rs   1
```

My grep was **looser** than the gate's rule (any occurrence, not just inside a string literal), so a
zero here is the strong direction: zero anywhere implies zero in literals. `kernel-only-operator-hook`
is no longer a dispatch site at all, so a row naming it would be a **stale row**, and the gate fails
in both directions. **Keeping it would not have been conservative; it would have been a defect.**
`%8`'s reasoning holds and the acceptance's prediction, written earlier, is what went stale.

**The surviving row is structural, not a TODO** — verified rather than taken on trust:

```
crates/dispatch-saga/src/m2.rs   --robot-send 1 | --msg-file 0 | --msg 0 | FROM/reply 0
  :144-145  format!("--robot-send=omp-orchestrator"), format!("--panes={}", …)
```

It builds a send argv with **no message payload at all**, so there is genuinely no packet boundary
for a FROM line to occupy. The reason as written is accurate.

## THE INDEPENDENT ORACLE — leg 2's substance confirmed without the lane

Re-running `%8`'s compiled binary would have re-measured `%8`'s fixture. Instead I reimplemented the
gate's predicate against `git ls-tree HEAD` and compared verdicts. It took **three iterations, and
the first two were my own defects** — recorded because the corrections are the useful part:

|iteration|unidentified files found|my error|
|---|---|---|
|v1|**7**|swept `tests/` — the gate is `src/**` only (`:57-58`, `:384`)|
|v2|**2**|marker set inferred from prose, not read from the constant|
|v3|**1**|comments not stripped|

- **v1 → v2.** `:57-58` states *"`crates/<name>/src/` only — never `tests/`"*, with its own leg
  `the_scan_swept…must cover src/ only`. My v1 counted `kernel-only-operator-hook/tests/hook.rs`
  with **15** sites, which the gate deliberately never sees.
- **v2 → v3.** I read the doc comment's four markers and missed four more. The real constant
  `SENDER_IDENTITY_MARKERS` (`:99-108`) has **eight**, including `"sender"`, `"from"`,
  `"from_agent"`, `"reply_to"` — quoted, because *`"sender"` does not match `"sender_ok"`*. Reading
  the constant instead of the prose is the same lesson as `close-evidence-gate`'s `CITED_PATH`.
- **v3.** v2's extra row was `crates/response-envelope-check/src/lib.rs`, whose only hit is
  **inside a doc comment**: `//! Fail-closed validation for the response from "ntm --robot-send".`
  That is precisely the defect `AGENTS.md` records — *"a doc comment warning about a needle contained
  the needle"* — and this instance is **mine**. The gate itself strips comments while keeping string
  literals (`:121`, and its leg `tokenizing_removes_prose_but_keeps_argv_literals`, which even covers
  the `'"'` char-literal edge). So the gate was right and my instrument was wrong.

**Final independent result at HEAD, mirroring the gate's predicate (comments stripped, literals
kept, `src/**` only, own source excluded):**

```
dispatch-site files 6, site occurrences 12
UNIDENTIFIED  ['crates/dispatch-saga/src/m2.rs']
ALLOWANCE     ['crates/dispatch-saga/src/m2.rs']
MATCH         True

per file:  2 agent-mail-native/src/identity.rs      1 dispatch-saga/src/m2.rs
           3 fast-dispatch/src/main.rs              2 loop-tick/src/lib.rs
           1 omp-idle-dispatch/src/main.rs          3 tick-dispatch/src/main.rs
```

**Exactly one unidentified file, equal to the allowance.** This is a differential oracle, not a
re-run, and it is stronger evidence for leg 2 than the lane could have produced.

## Leg 2's on-lane run — UNMEASURABLE, and the gate is ENVIRONMENT-HONEST

```
RCH_WORKER=contabo-3 … RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p no-shell-gate --test sender_identity
exit=101  contabo-3  bypass=0   5 passed; 3 failed
```

The split is perfectly clean:

|passed — logic and planted fixtures|failed — every leg that reads the REAL repo|
|---|---|
|`the_marker_reader_distinguishes_a_packet_field_from_a_receipt_field`|`an_empty_dispatch_site_set_is_an_error`|
|`tokenizing_removes_prose_but_keeps_argv_literals`|`every_allowance_row_carries_a_reason_and_names_a_real_site`|
|`the_scan_does_not_include_its_own_source`|`every_dispatch_site_renders_a_from_line`|
|`a_planted_dispatch_site_with_a_from_line_passes`||
|`a_planted_dispatch_site_without_a_from_line_is_caught`||

All three failures share one root cause:

```
SENDER_SCAN_EMPTY crates=0 files=0 sites=0 — zero dispatch sites found.
An empty scan set is an ERROR, never a pass: the most likely cause …
```

That is **`ln7k`**: `rch`'s transfer excludes `.git`, so `git ls-files` returns nothing on a worker.
**Environment, not subject.** And note what the gate did with it: its own anti-vacuity leg
`an_empty_dispatch_site_set_is_an_error` **fired**, refusing to report a vacuous green. The failure
message names the likely cause itself.

**This contrasts instructively with `br_publisher`**, which I graded an hour earlier. Both report
FAILED where the honest verdict is UNMEASURABLE. `br_publisher` conveys that only through a panic
string from `.expect("br must run")`; this gate conveys it through a **named, designed error whose
text tells the reader what broke**. Same verdict-level ambiguity, very different quality of
disclosure — and the gate's version is what the `mcbt` template should look like.

**`%8`'s 8/8 figure was a compiled test binary plus a hand-made roster.** That is a **fixture**, C38
applies, and `%8` disclosed it. I hold it to exactly that scope: evidence the LOGIC works, not that
the gate runs on this lane. **Not a fault of the implementer** — and my static oracle covers what the
fixture could not.

## Legs 1 and 3 — on-lane, with a mutation

```
cargo test -j 2 -p agent-mail-native --test identity   exit=0  contabo-3  bypass=0  10 passed / 0 failed
```

Leg 1's rendered header, asserted verbatim by
`sender_header_names_the_verified_agent_and_both_reply_routes`:

```
FROM: AmberGate pane_index=4 pane_id=%1408 binding=verified-live
REPLY-VIA: ntm --robot-send=omp-orchestrator --panes=4 --msg-file <path>; Agent Mail to AmberGate project=…
```

Agent name **first**, pane index **and** pane_id, both routes. Leg 1 satisfied as specified, with a
fires-on-known-bad sibling (`sender_header_refuses_missing_pane_index`).

Leg 3 satisfied and the trap is closed at source:

```
identity.rs:342-350  tmux_identity_argv  ["display-message","-t",pane_id,"-p","#{pane_id} …"]
identity.rs:353-354  from_env reads TMUX_PANE, refuses IdentityError::MissingTmuxPane
omp-idle-dispatch/src/main.rs:501-511    TMUX_PANE, else SENDER_IDENTITY_REFUSED reason=TMUX_PANE_missing
refill-idle-panes/src/main.rs:192-206    same, PLUS `if identity.pane_id != pane_id` cross-check
```

No unqualified `display-message` exists in any `crates/*/src/*` — every hit carries `-t`, and the
remaining textual matches are the warning comments explaining why.

**The negative control leg 3 demanded exists** (`tests/identity.rs:110`): `active_pane = "%1396"`
with `assert_ne!(args[2], active_pane)`, so it proves the query targets the CALLER's pane and not the
focused one.

**MUTATION, on production source:** emptied the `-t` argument in `tmux_identity_argv`
(`pane_id` → `""`).

```
exit=101  9 passed; 1 failed   RED = tmux_identity_targets_the_calling_pane ONLY
  left:  ["display-message","-t","","-p", …]
  right: ["display-message","-t","%1413","-p", …]
RESTORED  sha 31083314ae4b008ed2c2 byte-identical, clean vs HEAD, back to 10 passed / 0 failed
```

Attributable, and the failure output is self-describing rather than a bare code.

## Leg 4 — disclosure, satisfied

All three renderers are now identified, confirmed by the oracle at HEAD (`FROM:` present in each):

```
crates/omp-idle-dispatch/src/main.rs     sites=1   FROM:   landed in c294b7f
crates/refill-idle-panes/src/main.rs     sites=0   FROM:   landed in c294b7f
crates/omp-orchestrator/src/main.rs      sites=4   FROM:   landed in 14b34f1 (correctly not in this commit)
```

`refill-idle-panes/src/main.rs` shows **0** dispatch sites after comment-stripping yet renders a FROM
line anyway — belt beyond the gate's requirement, not a defect.

## NO-CLAIM

- **This gate proves presence of a marker, never correctness of the value** — its own module docs say
  so (`:44-48`): a file rendering `FROM: {}` with an empty string passes every leg. Per-site dataflow
  is unbuilt. My oracle inherits that limit exactly.
- **My oracle is a differential, not a proof of the gate.** It agrees with the allowance at HEAD; it
  does not establish that the gate's compiled scan agrees on a machine where `git ls-files` works.
  That remains blocked on `ln7k`.
- **Leg 2's "stays GREEN" is UNSATISFIED ON THIS LANE and I am not scoring it as a gap** — the cause
  is `ln7k` (P0, filed), not this commit. A grader who reads only `5 passed; 3 failed` would score
  `mad1` as failing; that reading is wrong, and this paragraph exists so nobody makes it.
- **My identity-target figures are WORKTREE figures**, because 7 peer files in `agent-mail-native`
  are dirty. The subject files are clean, but the compiled crate is not purely `HEAD`.
- **The bead title says "19 of 37 sites unidentified"; I measure 12 sites in 6 files today**, one
  unidentified. I did not reconcile 37 → 12; the earlier figure predates the fix and may have used a
  wider scope (`tests/` included would add 15 from one file alone). Cite neither number as current
  without re-deriving.
- Every figure above is ON-LANE: `Remote command finished: exit=` present and the `RCH BYPASS`
  banner absent in each log, checked per run.
