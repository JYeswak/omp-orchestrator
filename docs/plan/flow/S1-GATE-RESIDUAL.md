# S1 gate residual — the non-PASS crates, ENUMERATED

```
⛔ THE TITLE ONCE SAID "the 20 non-PASS crates". THE RESIDUAL IS NOW 6. Superseded THREE times
   on 2026-09-11/12, each against a NEWER verdict-bearing run. Old figures are kept as
   SUPERSEDED rather than deleted, because a deleted number cannot be checked against the
   run that produced it.

AUTHORITATIVE  run 34666855450   headSha 5d3ef562   conclusion=failure
  GATE_RUNNER_FAILING      count=2  no-shell-gate, omp-orchestrator
  GATE_RUNNER_UNMEASURABLE count=4  admission-reason:POLICY_UNAVAILABLE,
                                    finding:MISSING_EXECUTABLE,
                                    loop-driver:POLICY_UNAVAILABLE,
                                    loop-queue-filter:MISSING_EXECUTABLE
  non-PASS = 6.
  GATE_RUNNER_LEDGER_DRIFT — ZERO lines, from a grep whose PLAN alternative MATCHED, so the
  zero is a measured negative rather than a pattern that could never hit.

  ✅ THE SUM CONTROL IS AVAILABLE ON THIS RUN, which the entry below explicitly could not say:
     pass=88 fail=2 unmeasurable=4 = 94, reconciled against GATE_RUNNER_PLAN crates=94.
     The pass count here is MEASURED, not derived by subtraction.

  ⭐ THE DENOMINATOR GREW WHILE THE FAILURE COUNT FELL — 88 -> 94 crates against 16 -> 2
     failures. THAT ORDER MATTERS: a falling failure count is precisely what a coverage
     collapse looks like, and the only thing separating the two readings is the PLAN line.
     Every crate is measured and none was dropped, so this is a repair on a LARGER
     population. Without PLAN crates=94 this entry would be indistinguishable from a
     gate that quietly stopped looking.

  ⭐ ONE CRATE LEFT THE FAILING SET AND THE DEPARTURE IS NOT ATTRIBUTED:
    omp-inventory-map  left somewhere between 4911489 and 5d3ef562. That window is 137
                       commits and NINE touch the crate: 5f749df 6fbdd42 c26cd3e 8aa76e2
                       6f0dfbe 4d257e1 19a5c4b a14edb7 df0d4a3. Nobody has shown WHICH one
                       moved the gate verdict, so this file names the candidates and claims
                       none of them. The roster repair (`git ls-files` -> `ls-tree HEAD`) is
                       the plausible single cause and is recorded as a HYPOTHESIS, not a fix
                       credit — same bar the s1-coverage row below is held to.

  CORROBORATION, not a second measurement: run 34666287039 (3fe73133) carries the IDENTICAL
  failing and unmeasurable sets, so count=2 is not a one-run artifact. Two runs agreeing does
  not make either of them the right oracle; the run id and headSha remain the claim.

SUPERSEDED     run 34601341490   headSha 4911489   conclusion=failure   2026-09-11 12:54Z
  GATE_RUNNER_FAILING      count=3  no-shell-gate, omp-inventory-map, omp-orchestrator
  GATE_RUNNER_UNMEASURABLE count=4  admission-reason:POLICY_UNAVAILABLE,
                                    finding:MISSING_EXECUTABLE,
                                    loop-driver:POLICY_UNAVAILABLE,
                                    loop-queue-filter:MISSING_EXECUTABLE
  non-PASS = 7.
  GATE_RUNNER_LEDGER_DRIFT — ZERO lines. `build-stamp`'s drift cleared by ac8b4eb.

  ⭐ TWO CRATES LEFT THE FAILING SET. ONE DEPARTURE IS ATTRIBUTABLE, ONE IS NOT:
    ompo-doctor   fixed by d0c50f2 (repo-backed fixtures for the inception repair legs).
                  ⚠️ The grade for that work explicitly said it proved `cargo test` green and
                  did NOT prove the crate passes the GATE RUNNER, "a separate oracle". THIS
                  RUN DISCHARGES THAT NO-CLAIM — the separate oracle has now spoken. A
                  NO-CLAIM is a debt to be paid by later evidence, not a permanent hedge.
    s1-coverage   left the set without a targeted fix; its `cargo test` was already green when
                  measured, so its FAIL was upstream of the crate. NOT attributed to any
                  commit here, because nobody has shown the mechanism.

SUPERSEDED     run 34592771605   headSha 39b52fee   2026-09-11 11:11Z   non-PASS 9
                 FAILING 5: the three above plus ompo-doctor, s1-coverage
SUPERSEDED     run 34587961695   headSha df49750a   2026-09-11          non-PASS 20

  ⚠️ THE SUM CONTROL WAS NOT AVAILABLE ON RUN 34601341490 and its absence was stated rather
  than papered over: an earlier entry reconciled 81 + 5 + 4 = 90 against GATE_RUNNER_PLAN
  crates=90. No PLAN line was captured for 34601341490, so its pass count is DERIVED by
  subtraction rather than measured, and is deliberately not written down. The authoritative
  entry above restores the control.

  ⚠️ THE SUM CONTROL VALIDATES THE TALLY, NEVER THE ORACLE. This file has twice published a
  correct tally of the WRONG run, so the control above is necessary and not sufficient: it
  would read equally true at the stale head. The run id and headSha are the claim.

  ⚠️ AND UNMEASURABLE IS NOT A SMALLER KIND OF PASS. Four crates produce NO VERDICT —
  two for a missing executable, two for an unavailable policy — and an absent verdict is
  indistinguishable from a pass at the exit code. They are residual, not resolved.

SUPERSEDED   run 34587961695   headSha df49750a   completed/failure   2026-09-11   non-PASS 20
ORACLE      the newest run whose CONCLUSION is success|failure -- NOT the newest "completed"

  ELEVEN CRATES LEFT THE NON-PASS SET BETWEEN THESE TWO RUNS. Anyone dispatched against the
  superseded enumeration would have worked eleven crates that are already green — the exact
  cost this file was written to prevent, now incurred by the file itself. RE-DERIVE BEFORE
  DISPATCHING, with the two commands the newest runs make sufficient:
    gh run list --limit 12 --json databaseId,conclusion,headSha
    gh run view <id> --log | grep -aoE 'GATE_RUNNER_(FAILING|UNMEASURABLE) count=[0-9]+ names=.*'
  The newest runs ship the NAMES inline, so no per-crate scraping and no `sort -u` is needed.

⛔ THE FOUR UNMEASURABLE ARE ENVIRONMENT, NOT CRATE DEFECTS — DO NOT DISPATCH THEM AS WORK.
  Measured 2026-09-11 from run 34592771605's own detail strings, which the runner prints and
  which nobody had read:

    finding            MISSING_EXECUTABLE  executable=br  detail=br is unavailable on PATH
    loop-queue-filter  MISSING_EXECUTABLE  executable=bv  detail=bv is unavailable on PATH
    admission-reason   POLICY_UNAVAILABLE  policy=admission-reason-differential
                       detail=oracle missing at <runner>/../control-plane/bin
    loop-driver        POLICY_UNAVAILABLE  policy=lockf-shell-oracle
                       detail=the loop-driver differential oracle requires /usr/bin/lockf

  NOT ONE IS A BUG IN THE CRATE. `br` and `bv` are local operator tools absent from a GitHub
  runner; `../control-plane` is a SIBLING REPOSITORY that is never checked out there; and
  `/usr/bin/lockf` is a BSD/macOS utility that does not exist on Linux at all — its Linux
  counterpart is `flock`, a different tool with a different interface.

  SO TWO OF THE FOUR ARE STRUCTURALLY UNMEASURABLE ON THIS CI, not merely unmeasured.
  `loop-driver` cannot be measured on a Linux runner while its oracle is named `lockf`, and
  `admission-reason` cannot be measured without coupling this repo's CI to a second one.
  `finding` and `loop-queue-filter` are the addressable pair, and only if installing `br`/`bv`
  into CI is judged worth the coupling — that is a decision, not a chore.

  ⚠️ THE RUNNER'S TYPING IS THE THING THAT WORKS HERE, and it deserves saying: it emits
  MISSING_EXECUTABLE where the remedy is *reach the tool* and POLICY_UNAVAILABLE where the
  remedy is *supply the oracle*, and `lib.rs:156-157` states in its own words that neither
  means *fix the crate*. The gate got the verdict class right; every reader downstream — this
  file included — flattened four typed non-verdicts into a residual count and implied work
  that does not exist.

  ⚠️ AND AN UNMEASURABLE IS STILL NOT A PASS. Four crates have NO verdict. At the exit code an
  absent verdict and a green are the same observation, which is why they stay enumerated here
  rather than being netted out of the residual. The honest reading is: 5 failing, 4 unknown.

✅ THE CAUSE-CAPTURE ARC IS CLOSED ON THIS RUN. Four rungs cleared before a row was read:
  GATE_RUNNER ' ' 2 · could-not-compile 0 · GATE_RUNNER_FAILURE_CAUSE 59 lines
  assert_eq OPERANDS: details containing "left:" 10, against a denominator of 10 details
  announcing `assertion left == right failed` -- measured in the SAME run, same command.
  f60845e location-only (59/59) -> 9176b51 message (55/55, header-only 0) -> 51990cc operands
  5/10 (adjacency-bounded) -> d342505 block-bounded 10/10. Every step a ratio, never a count.

⛔ AND THE TALLY DID NOT MOVE WITH IT. crates 89 -> 90 (build-stamp landed and PASSES),
  pass 81 -> 82, fail 4, unmeasurable 4, SAME FOUR NAMES. Readable causes are progress on
  SIZING the failing set, never on shrinking it. Do not read this arc as a crate getting fixed.

⛔⛔ RETRACTED WITHIN THE HOUR BY ITS OWN AUTHOR -- DO NOT ACT ON THE DELETED TEXT. This block
  read "seven of no-shell-gate's legs emit ONE byte-identical cause, so one behaviour closes
  seven legs." IT IS UNFOUNDED. The shared string is `empty_staged: CLEAN staged_files=1` and
  it is a PROGRESS TRACE, not a verdict: pre-commit-gate.rs:180 emits it UNCONDITIONALLY on any
  invocation with staged files, BEFORE the refusal vector and before every validator. It means
  "there are staged files, proceeding". 10 of 55 cause lines carry it, so it identifies an
  INVOCATION SHAPE, not a defect. A needle that fires on every member of a class cannot
  discriminate within it -- and "only one producer" was the evidence AGAINST, not corroboration:
  one unconditional producer is exactly what a non-discriminating string looks like.

⛔ AND THE REAL RESIDUAL, found by chasing that error: THE assert_eq! OPERANDS ARE ABSENT FROM
  THE ARTIFACT ENTIRELY. 10 of 55 details announce `assertion 'left == right' failed`; details
  containing "left:" -> 0; "right:" -> 0; and "left:"/"right:" anywhere in the 493,235-byte raw
  log -> 0/0 (positive control). Rust prints an assert_eq message on the HEADER line and the
  OPERANDS on the two lines beneath; the emitter captures header + the NEXT non-empty line, so
  it takes the human label and drops observed-vs-expected. AN assert_eq PAYLOAD IS THREE LINES,
  NOT TWO. For those 10 the deciding values are unreadable by anyone. The message-capture fix
  is real (55/55 carry a message where 59/59 carried only a location) and INCOMPLETE.

  ✅ RESOLVED on the SOURCE run above -- 10 of 10 operands captured. Two commits: 51990cc
  collected them adjacent to the message and scored 5/10, because an assert_eq MESSAGE can be
  MULTI-LINE (one embeds captured gate stderr, so continuation text sits between the assertion
  and its operands). d342505 bounds the scan by the TEST BLOCK instead -- skip continuation,
  stop at '---- ' or a new 'panicked at', stop once both operands are held. The 5/10 was
  published as a MISS with the same prominence as the hits, and its mechanism was named as an
  UNVERIFIED candidate plus the one command that would decide it; that command closed it.

SUMMARY     GATE_RUNNER crates=90 · pass=82 · fail=4 · unmeasurable=4   (82+4+4=90 ✓)
FAILING     no-shell-gate · omp-inventory-map · omp-orchestrator · ompo-doctor

⛔ THE TALLY IS BYTE-IDENTICAL TO THE SUPERSEDED RUN (89/81/4/4, same four names). NOTHING
  ABOUT THE VERDICTS CHANGED -- only their LEGIBILITY, and the first attempt to convert that
  legibility into a SIZING was retracted above within the hour. The honest current state: the
  causes are readable, one class (10 of 55 assert_eq) is still undecidable for lack of its
  operands, and NO leg has yet been shown to share a defect with another.

UNMEASURABLE admission-reason:POLICY_UNAVAILABLE · finding:MISSING_EXECUTABLE
             loop-driver:POLICY_UNAVAILABLE · loop-queue-filter:MISSING_EXECUTABLE

PER-CRATE LEG COUNTS, from the RAW LOG and with the parse rule stated, because three
independent parses of the same line disagreed until each declared its own:
  no-shell-gate      45 names / 20 target groups   2951 B   owner sc0h5
  omp-orchestrator    9 names                       633 B   owners bz2na (4) + u3f6q (2) + uldvu (1)
  ompo-doctor         6 names                       444 B   owner ue29h; 2 already fixed by 6a0a4ed
  omp-inventory-map   1 name                        132 B   parked by design (poumg.3)
  parse rule: grep -F 'FAIL crate=<name>' on `gh run view --log`, split on ',', sort -u

SUPERSEDED  34569451324 / 3093989  06:19Z  FAILING count=4
SUPERSEDED  34566431257 / 98e44cc  06:01Z  FAILING count=7
SUPERSEDED  34549975939 / cb9d3941 01:16Z  FAILING count=16
SUPERSEDED  34570440318  f2dbf3eb  06:33Z  FAILING count=4  (same four names)
```

⛔ BEFORE YOU READ ANY ROW BELOW, CLEAR FOUR RUNGS. Three of them were skipped tonight and
each produced a confident wrong reading:
  1. PUSHED     git ls-remote origin refs/heads/main      <- NEVER a tracking ref; origin/main is a CACHE
  2. CONTAINED  git merge-base --is-ancestor <fix> <headSha>
  3. GATE RAN   grep -c 'GATE_RUNNER ' <log>   MUST be nonzero
                grep -c 'could not compile'    MUST be zero
  4. LEG'S ROW  your crate's row present in THAT output

⛔⛔ RUNG 3 EXISTS BECAUSE OF A LIVE SPECIMEN: run 34577201755 CONTAINS the cause-capture fix
and produced ZERO GATE_RUNNER lines, because that same commit's unpaired consumer broke
gate-runner's build (E0432 unresolved import). ITS SILENCE ABOUT A LEG IS ABSENT-BY-BUILD,
NOT A VERDICT -- and "0 cause lines" there reads exactly like "the feature regressed".
THE FEATURE'S OWN COMMIT PREVENTED THE BUILD THAT WOULD HAVE MEASURED IT. Repaired by
454328b. A zero on rung 3 at a head containing 454328b is a NEW defect, not this one.

⭐ The rung-3 needle is a FINISHED marker, not a started one: the census prints at main.rs:374
from a report built at :372, downstream of the run loop at :264. The trailing space is
load-bearing -- it excludes GATE_RUNNER_FAILURE_CAUSE (using the feature under test to prove
the gate ran would be circular) AND the early GATE_RUNNER_PLAN lines. Do not widen it, do not
substitute GATE_RUNNER_PLAN_TOTAL (that one is emitted at :219, BEFORE the sweep), and note a
healthy log scores 2: one emission plus one ci-citation step quoting it verbatim. It is a
liveness check, never a count of gate invocations, and it is not portable to a cargo-test log
of gate-runner itself, where ci_citation.rs self-matches nine times.

⛔⛔ **THE "SIX LEGS" FIGURE THIS DOCUMENT'S DISPATCHES ONCE CARRIED FOR `no-shell-gate` WAS
SELF-INFLICTED: a `grep -oE 'failing_tests[^|]{0,400}'` clipped a 2951-byte line at 400
characters.** The producer was complete throughout; **no CI line carries an ellipsis and every
one terminates with `)`.** ⭐ **Truncation is ONE-DIRECTIONAL — it can hide a name, never invent
one — so PRESENCE claims here are immune and only ABSENCE claims ever needed re-checking.** The
one absence that mattered (`u3f6q`'s two legs missing from CI) was re-checked on the raw log and
**holds**.

⭐⭐ **FAILING WENT 16 → 7 → 4 IN ONE SESSION, AND THE TRAJECTORY IS ONLY VISIBLE BECAUSE THE
GATE CAN PRODUCE VERDICTS AGAIN.** Before `2d25494`, `cancel-in-progress: true` on a
`workflow+ref` group meant every push killed the running gate: **14 of 15 cancelled, last real
verdict four hours stale.** After the sha-keyed fix, **six verdict-bearing runs in forty
minutes.** ⛔ **The 16 → 7 → 4 improvement was happening the whole time and NOTHING COULD SEE
IT** — the crates were being repaired while the only instrument that compiles the committed tree
was being cancelled before it could say so.

⛔ **RE-DERIVE BEFORE CITING, per this document's own history: it named a superseded run
authoritative TWICE.** One command:
`gh run list --limit 25 --json databaseId,conclusion,headSha` → newest with conclusion
`success|failure`. **`completed` and `carrying a verdict` are different populations.**

⚠️ **THIS DOCUMENT WAS WRONG ON ITS FIRST PUBLICATION AND THE CORRECTION IS THE POINT.** `17c632e`
enumerated run `34289493517` / `475c702` — **two days and 83 commits stale** — and called it
authoritative. It was authoritative only against the even-older run it replaced. Caught
independently by two agents within minutes, **against a rule I had adopted as binding one message
earlier**: *"the newest run whose CONCLUSION is success or failure."* `completed` and `carrying a
verdict` are different populations — of the 8 most recent runs, **6 are `cancelled`**.

**The stale list was not a rounding drift — EIGHT crates moved, and it would have misrouted work
in both directions:**

```
in the OLD 12, NOT failing now :  omp-idle-dispatch · reap-finished-panes
failing now, ABSENT from OLD 12:  finding-dispatch · installer · kernel-only-operator-hook
                                  ompo-doctor · ompo-start · receiver-receipt
in both                        :  10
```

⛔ **AND `omp-idle-dispatch` DATES THE STALE RUN WITHOUT A TIMESTAMP.** `Cargo.toml:7` reads
`exclude = ["crates/omp-idle-dispatch"]`; it is on disk and **`ABSENT` from `cargo metadata`**,
which is where gate-runner's roster comes from. So today it cannot be a FAIL — the newest run
classifies it `GATE_RUNNER_LEDGER_DRIFT reason=in_ledger_absent_from_workspace`. **A FAIL row for
that crate PROVES the run predates the exclusion.** `reap-finished-panes` is the second,
independent tell, repaired by `924e3c7`.

## FAIL — 16 at `cb9d3941`. **THREE ARE NOW VERIFIED GREEN AT HEAD; FOUR ARE OWNED.**

**Re-derive before dispatching any row.** Verified-green rows were each re-run on Contabo with
both proof lines; they are NOT predictions.

| crate | state at HEAD | evidence / owner |
|---|---|---|
| `agent-mail-native` | FAIL | unowned |
| `dispatch-silence-watch` | repaired | `poumg.1` `f686ed3` — grading |
| `finding-dispatch` | ✅ **VERIFIED GREEN** | `25b8af4` · `2 passed` · `exit=0` |
| `installer` | FAIL | owned — background job `InstallerClobber` |
| `kernel-bypass-gate` | FAIL | owned — muse `%26` via `9ub39` |
| `kernel-only-operator-hook` | FAIL | unowned |
| `no-shell-gate` | FAIL | unowned — 2 named causes, neither from `nar5l` |
| `omp-inventory-map` | FAIL | unowned — see `poumg.3`, **PARKED on purpose** |
| `omp-orchestrator` | FAIL | unowned — 5 pre-existing `resident::tests` failures |
| `ompo-doctor` | FAIL | owned — muse `%26` via `t0ixj`; `8vflj` blocked behind it |
| `ompo-start` | ✅ **VERIFIED GREEN** | `063e67f` · `13 passed` · `exit=0` |
| `pane-dispatch-ready` | ✅ **FIXED** | `1c311ed` · `56 tests, 0 failed` · `exit=0` — and the ON arm was VACUOUS |
| `receiver-receipt` | ✅ **VERIFIED GREEN** | `6f3953b` · `7 passed` · `exit=0` |
| `silent-success-census` | repaired | `poumg.5` `7b3f78d` — grading |
| `undrained-pipe-lint` | ✅ **ALREADY-FIXED** | `bfced67` stale test DELETED · `19 passed` · `exit=0` |
| `verify-dispatch` | ✅ **FIXED** | `a4468ad` · `17 passed` · `exit=0` — one env degree of freedom, and 3 dead mutation legs |

⛔ **SEVEN CRATES CANNOT CROSS-BUILD TO macOS THROUGH THE SANCTIONED FORM, AND EVERY TEST WE RUN
IS BLIND TO IT** — `omp-orchestrator-8jlpp`, open. `chrono`'s `clock` feature pulls
`iana_time_zone` → CoreFoundation, and `zigcc-aarch64-darwin` resolves no `_CF*` symbols. **It
fails at LINK after a clean compile**, so `cargo check` and every `-p <crate>` test run return
green. Affected, all shipping a `[[bin]]`: `fast-dispatch`, `fleet-truth`, `loop-driver`,
`loop-switch`, `loop-tick`, `pane-truth`, `verify-dispatch`.
**Positive control:** `receiver-receipt`, same flags, no `chrono` → `Finished` / `exit=0`, so
the darwin path itself is sound. ✅ **`ompo` is NOT affected** — verified independently:
`grep -c chrono crates/ompo-doctor/Cargo.toml` → **0**, against `verify-dispatch` → **1**.

⚠️ **AND A SECOND PREMISE I GOT WRONG.** I briefed an agent *"there is NO bead yet, file one"*
for `verify-dispatch`. **`poumg.7` already existed**; it claimed that instead of filing a
duplicate. **Two dispatcher premise errors on this page, both caught by the worker** — which is
why the standing rule is to re-derive a bead's premise before sending it, and why a worker
correcting the packet is a success rather than friction.

⚠️ **A PREMISE I GOT WRONG, corrected by measurement.** I briefed an agent that `25b8af4` and
`c5519dc` repaired **both** `finding-dispatch` and `receiver-receipt`. Measured: `25b8af4` fixed
**only** `finding-dispatch`; its `receiver-receipt` hunk touched **comments only**
(`src/lib.rs:616,965`). **`receiver-receipt`'s actual CI failure lived in contract MARKDOWN, not
in Rust** — `docs/contracts/receiver_receipt_contract.md:155` — and was fixed by the separate,
later `6f3953b`. **Two crates on the same list, repaired by different commits in different
languages, and one confident sentence merged them.**

**AND BOTH GREENS WERE CHECKED FOR THE DELETED-PROPERTY FAILURE, not just for passing.**
`receipt_contract.rs:280-292` still hard-asserts `root.join(candidate).is_file()` for every
`crates/`|`docs/` token **and** still carries the anti-vacuity floor `assert!(checked >= 8)`, so
a relaxed extractor could not satisfy it. `finding-dispatch` still asserts both original needles
and its companion positive control passed in the same run. Anchored control on each:
`git show <sha> -- <paths> | grep -c '^-.*assert'` → **0 removed assertion lines**.

## UNMEASURABLE — 4, and the two reasons are DIFFERENT DEFECTS

| crate | reason | remedy |
|---|---|---|
| `admission-reason` | `POLICY_UNAVAILABLE` | oracle absent at its declared path — `INERT`, **WIRE it** |
| `finding` | `MISSING_EXECUTABLE` | the binary does not exist — **BUILD it** |
| `loop-driver` | `POLICY_UNAVAILABLE` | oracle absent at its declared path — `INERT`, **WIRE it** |
| `loop-queue-filter` | `MISSING_EXECUTABLE` | the binary does not exist — **BUILD it** |

**Per gate rule 4a, collapsing these into one bucket sends the reader to the wrong repair.**

## LEDGER DRIFT — 2, neither is a FAIL and both need a decision

| crate | reason | remedy emitted by the runner |
|---|---|---|
| `contabo-reclaim` | `in_workspace_absent_from_ledger` | `add_the_row` — **the crate WAS still run** |
| `omp-idle-dispatch` | `in_ledger_absent_from_workspace` | `delete_the_row_or_restore_the_crate` |

## Instrument findings — these survive the re-derivation

**1. `grep -c 'GATE_RUNNER'` IS A MOVING TARGET, WHICH IS WORSE THAN A WRONG ONE.** It returns
**4** on `475c702` and **13** on `cb9d3941`, because the newest run added `_FAILING`, `_UNMEASURABLE`
and `_LEDGER_DRIFT` summary lines. On the older run the per-crate verdicts are bare
`PASS crate=… / FAIL crate=…` lines carrying **no `GATE_RUNNER` token at all**, so the instrument
everyone was pointed at could not reach the data it was aimed at. **On `cb9d3941` the names ship
in the log directly** — `GATE_RUNNER_FAILING count=16 names=…` — so re-deriving needs no scraping.
`86zjl`'s defect, now confirmed from three runs.

**2. `gh run view --log` EMITS EVERY PER-CRATE ROW TWICE** — streamed as each crate lands, then
again in the report, byte-identically and by design. A naive tally on the older run reads
`24 FAIL / 144 PASS / 8 UNMEASURABLE`, **sum 176**: internally coherent, plausible, and double.
**The sum-to-88 control catches it; a count does not.**

**3. ⛔ A SOURCE THAT CONTAINS THE WHOLE POPULATION CANNOT EVIDENCE A SUBSET OF IT.** `pd5ua` was
credited with enumerating the failing crates. Grepping its cited `docs/gate-roster.txt` for them
returns **11 of 11** — because it is the **FULL 88-crate roster**, containing `tick-monitor`,
`pane-truth` and `bead-availability`, all of which PASSED. Every failing crate matched for the
identical reason every passing one did. Same class as `grep -c ompo` → 62 counting substrings.

**4. AND A CORRECT TALLY OF THE WRONG RUN IS THE MOST CONVINCING KIND OF WRONG FIGURE.** The
sum-to-88 control proves one run's internal consistency and says **nothing about which run**.
`72+12+4 = 88` is equally true at `475c702` and irrelevant at `cb9d3941`. **The control validates
the tally, not the oracle** — that is what defeated the first publication of this file.

## NO-CLAIM

One run at one head. It does **not** claim the 16 still fail at today's `HEAD`, that any one is a
real defect rather than a stale test, or that the 68 PASS rows are correct. **Re-derive before
dispatching any of them** — `dispatch-silence-watch`, `silent-success-census`, `undrained-pipe-lint`
and `omp-inventory-map` are `poumg` children whose premises are already under re-derivation.
