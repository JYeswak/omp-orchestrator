# S1 gate residual — the 20 non-PASS crates, ENUMERATED

```
SOURCE      run 34549975939   headSha cb9d3941   completed/failure   2026-09-11T01:16Z
ORACLE      the newest run whose CONCLUSION is success|failure -- NOT the newest "completed"
SUMMARY     GATE_RUNNER crates=88 pass=68 fail=16 unmeasurable=4 short=0 no_tests=0
```

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

## FAIL — 16

| crate |
|---|
| `agent-mail-native` |
| `dispatch-silence-watch` |
| `finding-dispatch` |
| `installer` |
| `kernel-bypass-gate` |
| `kernel-only-operator-hook` |
| `no-shell-gate` |
| `omp-inventory-map` |
| `omp-orchestrator` |
| `ompo-doctor` |
| `ompo-start` |
| `pane-dispatch-ready` |
| `receiver-receipt` |
| `silent-success-census` |
| `undrained-pipe-lint` |
| `verify-dispatch` |

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
