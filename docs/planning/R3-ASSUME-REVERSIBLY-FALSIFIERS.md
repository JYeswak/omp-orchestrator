# R3 ASSUME_REVERSIBLY falsifier audit

Bead: `omp-orchestrator-r3-assume-reversibly-no-falsifier-tlae`

This audit is deliberately separate from the pre-existing Atlas Arc report so a shared-checkout report edit is not swept into this commit. Scope is `docs/plan/flow/unknowns/**`, `docs/plan/flow/boxes/**`, and `docs/planning/**`. `docs/plan/00-brief.md` was not edited.


## Audit result

This is the follow-up to bead `omp-orchestrator-r3-assume-reversibly-no-falsifier-tlae`. Scope is limited to `docs/plan/flow/unknowns/**`, `docs/plan/flow/boxes/**`, and `docs/planning/**`; `docs/plan/00-brief.md` was not edited. No build, crate, or install ran.

### 7.1 Re-derivation and the count attack

The population command is:

```text
git show HEAD^:docs/plan/flow/unknowns/DISPOSITIONS.toml | grep -c '^disposition = "ASSUME_REVERSIBLY"'
git show HEAD^:docs/plan/flow/unknowns/DISPOSITIONS.toml | grep -c '^falsifier = '
git show HEAD^:docs/plan/flow/unknowns/DISPOSITIONS.toml | grep -c 'falsifier='
```

On the pre-audit tree this is `45`, `0`, and `4`. The register had no structured `falsifier =` field. Four reasons contained the prose token `falsifier=`; only one was an executable-shaped command, and none carried a checked expected result. Therefore the supplied `42 of 45` is not reproducible under the command-grade definition. This pass audits all 45 rather than manufacturing a 42-row worklist.

### 7.2 Outcome distribution

| outcome | count | treatment |
|---|---:|---|
| FALSIFIER | 36 | Exact runnable command plus explicit `REFUTES_IF` condition in the row's `falsifier` field. |
| RECLASSIFIED | 2 | `GAP-S3-06` and `GAP-S4-05` become `HUMAN_DECISION`, bound to `HD-0002`. |
| DECLARED_NOT_WIRED | 7 | S6a/S6b/S6c/S7/S8/S9 hook boundaries and S5b projection; each carries a `dies_when` and follow-up bead `omp-orchestrator-ys6b`. |
| UNRESOLVED | 0 | Every row has one of the three outcomes. |

The two human rows are not silently treated as implementation work: HD-0002 already decides that Joshua is the buyer and the external validation loop. The seven declared rows remain explicit residuals because presence of prose or a source symbol cannot prove a live certified hook or a live typed projection.

### 7.3 Enumeration of all 45

Each source location is the exact `file:line` for the disagreement or box-gap object. FALSIFIER rows carry their complete command in `docs/plan/flow/unknowns/DISPOSITIONS.toml`; declared and reclassified rows carry their landing condition in the same record.

| # | id | source | outcome |
|---:|---|---|---|
| 1 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#001-S5a-all` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:9` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 2 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#002-S5a-kernel_output` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:18` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 3 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#003-S5b-all` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:29` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 4 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#006-S7-branches` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:58` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 5 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#007-S8-all` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:69` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 6 | `WAVE-wave-1/AmberGate-S5a-S5b-S7-S8.toml#008-S8-crate_status` | `docs/plan/flow/waves/wave-1/AmberGate-S5a-S5b-S7-S8.toml:78` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 7 | `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#009-S6a-all` | `docs/plan/flow/waves/wave-1/AmberGate-S6a-S6b-S6c.toml:8` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 8 | `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#010-S6a-measurement` | `docs/plan/flow/waves/wave-1/AmberGate-S6a-S6b-S6c.toml:17` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 9 | `WAVE-wave-1/AmberGate-S6a-S6b-S6c.toml#013-S6c-all` | `docs/plan/flow/waves/wave-1/AmberGate-S6a-S6b-S6c.toml:48` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 10 | `WAVE-wave-1/AmberGate-S9.toml#017-S9-crate_status` | `docs/plan/flow/waves/wave-1/AmberGate-S9.toml:9` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 11 | `WAVE-wave-1/AmberGate-S9.toml#018-S9-measurement` | `docs/plan/flow/waves/wave-1/AmberGate-S9.toml:27` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 12 | `WAVE-wave-1/AmberGate-S9.toml#019-S9-branches` | `docs/plan/flow/waves/wave-1/AmberGate-S9.toml:36` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 13 | `WAVE-wave-1/BlueLantern-S2.toml#021-S2-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S2.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 14 | `WAVE-wave-1/BlueLantern-S3.toml#024-S3-gap_class_coverage` | `docs/plan/flow/waves/wave-1/BlueLantern-S3.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 15 | `WAVE-wave-1/BlueLantern-S3.toml#025-S3-hook_schema` | `docs/plan/flow/waves/wave-1/BlueLantern-S3.toml:21` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 16 | `WAVE-wave-1/BlueLantern-S4.toml#029-S4-hook_schema` | `docs/plan/flow/waves/wave-1/BlueLantern-S4.toml:21` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 17 | `WAVE-wave-1/BlueLantern-S6a.toml#031-S6a-branches` | `docs/plan/flow/waves/wave-1/BlueLantern-S6a.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 18 | `WAVE-wave-1/BlueLantern-S6a.toml#032-S6a-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S6a.toml:12` | DECLARED_NOT_WIRED: dies_when=a certified S6a hook row and a real hook-invocation test exist for the ACK/read-back trigger |
| 19 | `WAVE-wave-1/BlueLantern-S6b.toml#036-S6b-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S6b.toml:21` | DECLARED_NOT_WIRED: dies_when=a certified S6b hook row and a real hook-invocation test exist for the follow-up trigger |
| 20 | `WAVE-wave-1/BlueLantern-S6c.toml#037-S6c-branches` | `docs/plan/flow/waves/wave-1/BlueLantern-S6c.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 21 | `WAVE-wave-1/BlueLantern-S6c.toml#039-S6c-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S6c.toml:21` | DECLARED_NOT_WIRED: dies_when=a certified S6c hook row and a real hook-invocation test exist for the verify-grade-close trigger |
| 22 | `WAVE-wave-1/BlueLantern-S7.toml#040-S7-gap_class_schema` | `docs/plan/flow/waves/wave-1/BlueLantern-S7.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 23 | `WAVE-wave-1/BlueLantern-S7.toml#041-S7-hook_schema` | `docs/plan/flow/waves/wave-1/BlueLantern-S7.toml:12` | DECLARED_NOT_WIRED: dies_when=a certified S7 hook row names the installed binary, policy, stage, and six-stage certification result, with a real hook-interception test |
| 24 | `WAVE-wave-1/BlueLantern-S8.toml#045-S8-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S8.toml:21` | DECLARED_NOT_WIRED: dies_when=a certified S8 hook row and a real hook-invocation test exist for the install trigger |
| 25 | `WAVE-wave-1/BlueLantern-S9.toml#046-S9-gap_class_schema` | `docs/plan/flow/waves/wave-1/BlueLantern-S9.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 26 | `WAVE-wave-1/BlueLantern-S9.toml#048-S9-hook` | `docs/plan/flow/waves/wave-1/BlueLantern-S9.toml:21` | DECLARED_NOT_WIRED: dies_when=a certified S9 hook row and a real hook-invocation test exist for the decision-ledger trigger |
| 27 | `WAVE-wave-1/QuietRidge-S2.toml#049-S2-gap_inventory` | `docs/plan/flow/waves/wave-1/QuietRidge-S2.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 28 | `WAVE-wave-1/QuietRidge-S2.toml#050-S2-agreement.approval` | `docs/plan/flow/waves/wave-1/QuietRidge-S2.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 29 | `WAVE-wave-1/QuietRidge-S3.toml#051-S3-gap_inventory` | `docs/plan/flow/waves/wave-1/QuietRidge-S3.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 30 | `WAVE-wave-1/QuietRidge-S4.toml#053-S4-gap_inventory` | `docs/plan/flow/waves/wave-1/QuietRidge-S4.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 31 | `WAVE-wave-1/QuietRidge-S5a.toml#056-S5a-branches` | `docs/plan/flow/waves/wave-1/QuietRidge-S5a.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 32 | `WAVE-wave-1/QuietRidge-S5b.toml#057-S5b-gap_inventory` | `docs/plan/flow/waves/wave-1/QuietRidge-S5b.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 33 | `WAVE-wave-1/QuietRidge-S5b.toml#058-S5b-kernel_output` | `docs/plan/flow/waves/wave-1/QuietRidge-S5b.toml:12` | DECLARED_NOT_WIRED: dies_when=a typed dispatch projection has a live consumer that distinguishes authorization, packet validation, pane admission, and transport receipt; current S5b has no such wired boundary |
| 34 | `WAVE-wave-1/QuietRidge-S6a.toml#059-S6a-branches` | `docs/plan/flow/waves/wave-1/QuietRidge-S6a.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 35 | `WAVE-wave-1/QuietRidge-S6a.toml#060-S6a-agreement.approval` | `docs/plan/flow/waves/wave-1/QuietRidge-S6a.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 36 | `WAVE-wave-1/QuietRidge-S6b.toml#062-S6b-agreement.approval` | `docs/plan/flow/waves/wave-1/QuietRidge-S6b.toml:12` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 37 | `WAVE-wave-1/SilverWolf-S1.toml#067-S1-wave1_sections` | `docs/plan/flow/waves/wave-1/SilverWolf-S1.toml:3` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 38 | `WAVE-wave-2/pane4-jt1i-citations.toml#081-S1-cite.mirror-truncated-ellipsis` | `docs/plan/flow/waves/wave-2/pane4-jt1i-citations.toml:36` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 39 | `WAVE-wave-2/pane4-jt1i-citations.toml#083-S1-figure.unlabelled-in-owner-files` | `docs/plan/flow/waves/wave-2/pane4-jt1i-citations.toml:54` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 40 | `WAVE-wave-2/pane4-qibn-hook-count.toml#084-S1-box.hook.certified_false_vs_unattempted` | `docs/plan/flow/waves/wave-2/pane4-qibn-hook-count.toml:14` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 41 | `GAP-S1-02` | `docs/plan/flow/boxes/S1.toml:293` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 42 | `GAP-S1-03` | `docs/plan/flow/boxes/S1.toml:297` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 43 | `GAP-S1-04` | `docs/plan/flow/boxes/S1.toml:301` | FALSIFIER: exact command in `DISPOSITIONS.toml` |
| 44 | `GAP-S3-06` | `docs/plan/flow/boxes/S3.toml:118` | RECLASSIFIED: HUMAN_DECISION, gate=HD-0002 |
| 45 | `GAP-S4-05` | `docs/plan/flow/boxes/S4.toml:115` | RECLASSIFIED: HUMAN_DECISION, gate=HD-0002 |

### 7.4 Fires-on-known-bad proofs

Three falsifiers fired, not merely existed:

1. `WAVE-wave-1/BlueLantern-S3.toml#024-S3-gap_class_coverage`: the exact count command observed `12` gaps where the falsifier requires `16`; it emitted `FALSIFIER_FIRED`.
2. `WAVE-wave-1/QuietRidge-S2.toml#050-S2-agreement.approval`: the exact grep found the blank `approval = ""`; it emitted `FALSIFIER_FIRED`.
3. `WAVE-wave-2/pane4-qibn-hook-count.toml#084-S1-box.hook.certified_false_vs_unattempted`: a temporary copy mutated `UNATTEMPTED` to `false`; the exact grep emitted `FALSIFIER_FIRED`, then the temporary file was removed.

No falsifier was invented to make the aggregate equal 42.

### 7.5 Cheaper discipline and residual claim

The pressure is real. Requiring a full production test for every low-stakes wording or inventory assumption would incentivise `OUT_OF_SCOPE` laundering. Cheaper rule: a build-gating assumption must carry a runnable falsifier and a known-good leg; a non-build assumption may use a static command-grade falsifier, but if its decisive state is not constructible it must be `DECLARED_NOT_WIRED` with a concrete `dies_when`. Never accept bare `ASSUME_REVERSIBLY` prose.

NO-CLAIM: these rows now carry executable predicates or explicit residual boundaries, but this pass does not prove the future hooks/projection live, does not wire any crate, and does not resolve the separate maturity0 register.
