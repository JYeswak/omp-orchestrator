# S1 gate residual — the 16 non-PASS crates, ENUMERATED

**Extracted 2026-09-11 by pane 1 from the authoritative CI run. This is the list `pd5ua` was
credited with delivering and did not** — a post-close audit measured 0 of 11 failing crate names
in its comments and 2 of 11 in its body, both as prose examples rather than a classified list.

```
SOURCE      gh run view 34289493517 --log      job "gate" (conclusion: failure)
HEAD        475c70288bc66feedf97c621675d5539d95a23f5
SUMMARY     GATE_RUNNER crates=88 pass=72 fail=12 unmeasurable=4 short=0 no_tests=0
DERIVED     88 unique rows: 72 PASS · 12 FAIL · 4 UNMEASURABLE   <- sums to 88 AND matches
```

## Why nobody had this list

⛔ **THE PER-CRATE ROWS DO NOT CARRY THE `GATE_RUNNER` TOKEN.** They are bare
`PASS crate=… / FAIL crate=… / UNMEASURABLE crate=…` lines. `grep -c 'GATE_RUNNER'` over the whole
run log returns **4** — the two `_PLAN` lines, `_CHECKS`, and the summary. **The instrument every
reader was told to use cannot reach the data it is pointed at**, and the data has shipped in every
run since `568f2dfe`.

⛔ **AND `gh run view --log` EMITS EVERY ROW TWICE.** A naive tally reads
`24 FAIL / 144 PASS / 8 UNMEASURABLE`, sum **176** — a clean doubling that looks like a plausible
failure count rather than an instrument artifact. **`sort -u` first; the sum-to-88 control is what
catches it.** Without that control this document would have claimed 24 failing crates.

## FAIL — 12, each with the target that failed

| crate | failing test |
|---|---|
| `agent-mail-native` | `first_resume_from_origin_succeeds_for_recipient_with_later_first_event` |
| `dispatch-silence-watch` | `dispatch_silence_watch_is_in_crontab` |
| `kernel-bypass-gate` | `real_workspace_ledger_balances` |
| `no-shell-gate` | `gate_checker_scan_is_nonempty_and_clean` |
| `omp-idle-dispatch` | `complete_environment_passes_startup` |
| `omp-inventory-map` | `tests::allowance_integrity_passes_on_named_decisions` |
| `omp-orchestrator` | `unowned_target_is_rewritten_with_typed_reason,known_good_registered_target_routes_inside_registered_root,unowned_dir_receives_artifacts_only_without_the_wrapper,fleet_wrapper_matches_measured_revision` |
| `pane-dispatch-ready` | `mutation_busy_markers_load_bearing` |
| `reap-finished-panes` | `mutation_lock_names_holder` |
| `silent-success-census` | `positive_controls_refind_named_live_rows` |
| `undrained-pipe-lint` | `wired_into_ci_workflow` |
| `verify-dispatch` | `tests::disabling_named_beads_all_closed_false_passes_open_set,tests::closed_bead_is_verified,tests::disabling_only_closed_status_counts_false_passes_open_bead,tests::empty_beads_list_is_legacy_and_no_evidence,tests::duplicate_bead_ids_are_deduped,tests::malformed_json_is_skipped,tests::open_bead_is_no_evidence,tests::partial_close_is_no_evidence` |

## UNMEASURABLE — 4, and the two reasons are NOT the same defect

| crate | reason | remedy |
|---|---|---|
| `admission-reason` | `POLICY_UNAVAILABLE` | oracle absent at its declared path -- WIRE it |
| `finding` | `MISSING_EXECUTABLE` | the binary does not exist -- BUILD it |
| `loop-driver` | `POLICY_UNAVAILABLE` | oracle absent at its declared path -- WIRE it |
| `loop-queue-filter` | `MISSING_EXECUTABLE` | the binary does not exist -- BUILD it |

**Per gate rule 4a these are different verdicts with different remedies.** `MISSING_EXECUTABLE`
says *build it*; `POLICY_UNAVAILABLE` says the oracle is absent at its declared path, so the crate
is `INERT`, not `ABSENT`. Collapsing them into one "unmeasurable" bucket sends the reader to the
wrong repair — which is the exact failure 4a was written for.

## NO-CLAIM

This enumerates **one run at one head**. It does not claim the 12 are still failing at `HEAD`
today, that any one is a real defect rather than a stale test, or that the 72 PASS rows are
correct. **Re-run the extraction against the newest run CARRYING A VERDICT before treating this as
a work list** — "completed" and "carrying a verdict" are different populations, and several of
these crates already have open beads whose premises must be re-derived before dispatch.
