# S1 gate residual — the 20 non-PASS crates, ENUMERATED

```
SOURCE      run 34569451324   headSha 3093989    completed/failure   2026-09-11T06:19Z
ORACLE      the newest run whose CONCLUSION is success|failure -- NOT the newest "completed"
SUMMARY     GATE_RUNNER_PLAN crates=89 · FAILING count=4 · UNMEASURABLE count=4
FAILING     no-shell-gate · omp-inventory-map · omp-orchestrator · ompo-doctor
UNMEASURABLE admission-reason:POLICY_UNAVAILABLE · finding:MISSING_EXECUTABLE
             loop-driver:POLICY_UNAVAILABLE · loop-queue-filter:MISSING_EXECUTABLE

SUPERSEDED  34566431257 / 98e44cc  06:01Z  FAILING count=7
SUPERSEDED  34549975939 / cb9d3941 01:16Z  FAILING count=16
```

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
