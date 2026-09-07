# ASUPERSYNC-CONFORMANCE — generated syntactic shape and spawn triage

Generated, never drawn. The command beside this output is:

```text
cargo run --quiet -p asupersync-conformance -- --repo . --write ASUPERSYNC-CONFORMANCE.md
```

Measured source revision: `working-tree`
Pinned asupersync revision: `fa3c01aec` (version 0.4.9)

## Schema (syntactic only)

| property | measurement | contract meaning |
|---|---|---|
| forbid_unsafe | unsafe_code = "forbid" in Cargo.toml | memory-safety lint is present |
| dep_asupersync | asupersync in Cargo.toml | Cx-based cancellation surface exists |
| dep_subprocess_contract | subprocess-contract in Cargo.toml | drain-safe runner dependency exists |
| async_fns | async fn count in src | denominator for cx_first |
| cx_first | async fn first parameter is cx | declared cancellation parameter shape |
| checkpoints | .checkpoint() call sites | declared cancellation checkpoints |
| raw_command | Command::new call sites | lower-bound spawn census |
| forbidden_deps | forbidden runtime dependency names | dependency contract remains explicit |


|---|:-:|:-:|:-:|---:|---:|---:|---:|---|
| ack-spine | Y | Y | Y | 16 | 8 | 4 | 2 | - |
| ack-stage | Y | . | . | 0 | 0 | 0 | 0 | - |
| admission-reason | Y | . | Y | 0 | 0 | 0 | 3 | - |
| agent-mail-native | Y | Y | Y | 23 | 19 | 6 | 2 | - |
| asupersync-conformance | Y | . | . | 0 | 0 | 0 | 0 | - |
| bead-availability | Y | Y | Y | 4 | 4 | 2 | 1 | - |
| bead-holder | Y | . | Y | 0 | 0 | 0 | 3 | - |
| blocker-taxonomy | . | . | . | 0 | 0 | 0 | 0 | - |
| cargo-lane-budget | Y | . | Y | 0 | 0 | 0 | 1 | - |
| commit-build-fence | Y | Y | . | 1 | 1 | 2 | 0 | - |
| composer-typed | Y | . | . | 0 | 0 | 0 | 0 | - |
| convergence-stamp | Y | . | . | 0 | 0 | 0 | 0 | - |
| crate-atom-gate | Y | . | Y | 0 | 0 | 0 | 4 | - |
| crate-soundness-verify | Y | . | Y | 0 | 0 | 0 | 1 | - |
| decision-ledger | Y | . | . | 0 | 0 | 0 | 0 | - |
| dispatch-claim-fence | Y | . | . | 0 | 0 | 0 | 0 | - |
| dispatch-saga | Y | . | Y | 0 | 0 | 0 | 1 | - |
| dispatch-silence-watch | Y | . | Y | 0 | 0 | 0 | 2 | - |
| dispatcher-deadman | Y | . | Y | 0 | 0 | 0 | 3 | - |
| extraction-roster | Y | Y | Y | 2 | 0 | 0 | 1 | - |
| fast-dispatch | Y | . | Y | 0 | 0 | 0 | 30 | - |
| finding | . | Y | Y | 4 | 2 | 2 | 1 | - |
| finding-dispatch | . | . | . | 0 | 0 | 0 | 0 | - |
| fleet-composite | Y | . | Y | 0 | 0 | 0 | 1 | - |
| fleet-monitor | Y | . | Y | 0 | 0 | 0 | 27 | - |
| fleet-reconcile | Y | . | Y | 0 | 0 | 0 | 9 | - |
| fleet-truth | Y | . | Y | 0 | 0 | 0 | 13 | - |
| fuzz-build-gate | Y | . | Y | 0 | 0 | 0 | 1 | - |
| gate-runner | . | . | Y | 0 | 0 | 0 | 2 | - |
| grader-attribution-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| inbox-monitor | Y | Y | Y | 4 | 4 | 3 | 4 | - |
| input-manifest | . | . | . | 0 | 0 | 0 | 0 | - |
| installer | Y | . | Y | 0 | 0 | 0 | 10 | - |
| kernel-bypass-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| kernel-only-operator-hook | Y | Y | Y | 5 | 5 | 2 | 1 | - |
| lifecycle-event | . | Y | . | 2 | 2 | 2 | 0 | - |
| lifecycle-monitor | . | . | . | 0 | 0 | 0 | 0 | - |
| loop-coverage | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-driver | Y | Y | Y | 0 | 0 | 1 | 28 | - |
| loop-queue-filter | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-switch | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-tick | Y | . | Y | 0 | 0 | 0 | 1 | - |
| m2-grading-lane | . | . | . | 0 | 0 | 0 | 0 | - |
| named-test-filter-gate | . | . | . | 0 | 0 | 0 | 0 | - |
| no-shell-gate | Y | . | Y | 0 | 0 | 0 | 18 | - |
| ntm-fleet-monitor | Y | Y | . | 1 | 1 | 2 | 0 | - |
| omp-idle-dispatch | Y | Y | Y | 0 | 0 | 0 | 2 | - |
| omp-inventory-map | Y | Y | Y | 3 | 3 | 3 | 1 | - |
| omp-orchestrator | Y | Y | Y | 31 | 31 | 4 | 8 | - |
| omp-rpc-session | Y | Y | . | 7 | 6 | 2 | 3 | - |
| omp-surface-consumption | Y | . | Y | 0 | 0 | 0 | 3 | - |
| omp-types | Y | Y | Y | 0 | 0 | 0 | 0 | - |
| ompo-doctor | . | . | Y | 0 | 0 | 0 | 1 | - |
| ompo-start | . | . | . | 0 | 0 | 0 | 0 | - |
| oracle-compare | Y | . | Y | 0 | 0 | 0 | 6 | - |
| oracle-pane-state-differential | Y | . | Y | 0 | 0 | 0 | 3 | - |
| orchestration-tick-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| pane-dispatch-fence | Y | Y | Y | 1 | 1 | 0 | 2 | - |
| pane-dispatch-ready | Y | . | Y | 0 | 0 | 0 | 10 | - |
| pane-oracle-diff | Y | . | Y | 0 | 0 | 0 | 4 | - |
| pane-truth | Y | . | Y | 0 | 0 | 0 | 4 | - |
| path-literal-guard | Y | . | . | 0 | 0 | 0 | 0 | - |
| plan-assemble | Y | Y | . | 2 | 0 | 0 | 0 | - |
| porting-gate | Y | Y | Y | 2 | 2 | 1 | 4 | - |
| pre-delete-citation-check | Y | . | Y | 0 | 0 | 0 | 2 | - |
| preregistration-gate | Y | Y | Y | 4 | 2 | 1 | 1 | - |
| r1-breadth-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| reap-finished-panes | Y | . | Y | 0 | 0 | 0 | 8 | - |
| receiver-receipt | Y | . | . | 0 | 0 | 0 | 1 | - |
| refill-idle-panes | Y | Y | Y | 0 | 0 | 0 | 3 | - |
| response-envelope-check | Y | . | . | 0 | 0 | 0 | 0 | - |
| s1-coverage | Y | Y | Y | 4 | 4 | 1 | 1 | - |
| s2-gate | . | . | . | 0 | 0 | 0 | 0 | - |
| salvage-taxonomy | . | Y | . | 0 | 0 | 2 | 0 | - |
| scratch-home | Y | . | Y | 0 | 0 | 0 | 1 | - |
| sender-identity | Y | . | . | 0 | 0 | 0 | 0 | - |
| silent-success-census | Y | . | Y | 0 | 0 | 0 | 1 | - |
| staged-build-gate | Y | . | Y | 0 | 0 | 0 | 2 | - |
| state-wildcard-lint | Y | . | . | 0 | 0 | 0 | 0 | - |
| subprocess-contract | Y | Y | . | 2 | 2 | 1 | 19 | - |
| text-structure | Y | . | . | 0 | 0 | 0 | 0 | - |
| tick-dispatch | Y | . | Y | 0 | 0 | 0 | 12 | - |
| tick-monitor | Y | . | . | 0 | 0 | 0 | 4 | - |
| undrained-pipe-lint | Y | . | . | 0 | 0 | 0 | 0 | - |
| verify-dispatch | Y | . | Y | 0 | 0 | 0 | 3 | - |
| wired-but-inert-guard | Y | . | Y | 0 | 0 | 0 | 2 | - |
| worker-oracle-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| worker-tag-gate | Y | . | . | 0 | 0 | 0 | 0 | - |

Crates scanned: **88**. forbid_unsafe: **75**. dep_asupersync: **24**. async_fns: **118**. cx_first: **97**. checkpoints: **41**. raw_command sites: **281**.

## Raw Command triage

Every discovered site is classified; no `UNTRIAGED` row is emitted. The lexical lint is also run over the same source set.

| crate | file | line | triage | reason |
|---|---|---:|---|---|
| ack-spine | crates/ack-spine/src/ack.rs | 69 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| ack-spine | crates/ack-spine/src/ack.rs | 87 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| admission-reason | crates/admission-reason/src/lib.rs | 323 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| admission-reason | crates/admission-reason/src/lib.rs | 905 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| admission-reason | crates/admission-reason/src/lib.rs | 924 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| agent-mail-native | crates/agent-mail-native/src/oracle.rs | 135 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| agent-mail-native | crates/agent-mail-native/src/wake.rs | 255 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| bead-availability | crates/bead-availability/src/lib.rs | 755 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| bead-holder | crates/bead-holder/src/main.rs | 144 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| bead-holder | crates/bead-holder/src/main.rs | 158 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| bead-holder | crates/bead-holder/src/main.rs | 241 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| cargo-lane-budget | crates/cargo-lane-budget/src/lib.rs | 221 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| crate-atom-gate | crates/crate-atom-gate/src/main.rs | 209 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| crate-atom-gate | crates/crate-atom-gate/src/main.rs | 447 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| crate-atom-gate | crates/crate-atom-gate/src/main.rs | 462 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| crate-atom-gate | crates/crate-atom-gate/src/main.rs | 508 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| crate-soundness-verify | crates/crate-soundness-verify/src/lib.rs | 269 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatch-saga | crates/dispatch-saga/src/main.rs | 69 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatch-silence-watch | crates/dispatch-silence-watch/src/main.rs | 50 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| dispatch-silence-watch | crates/dispatch-silence-watch/src/main.rs | 61 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| dispatcher-deadman | crates/dispatcher-deadman/src/lib.rs | 239 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatcher-deadman | crates/dispatcher-deadman/src/lib.rs | 258 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatcher-deadman | crates/dispatcher-deadman/src/main.rs | 195 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| extraction-roster | crates/extraction-roster/src/main.rs | 67 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 30 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 115 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 130 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/lib.rs | 348 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/lib.rs | 663 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 119 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 190 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 261 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 277 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 291 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 301 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 323 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 349 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 379 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 403 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 415 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 425 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 448 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 462 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 468 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 501 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 773 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 783 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 807 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 871 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 923 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1064 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1157 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1162 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/scheduled_lane_telemetry.rs | 38 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| finding | crates/finding/src/lib.rs | 463 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fleet-composite | crates/fleet-composite/src/main.rs | 81 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 31 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 114 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 129 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 97 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 113 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 127 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 203 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 267 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 560 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 584 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 619 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 650 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 695 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 887 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 976 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1007 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1051 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1135 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1198 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1261 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1266 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1292 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1376 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1507 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1520 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1536 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/scheduled_lane_telemetry.rs | 38 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/lib.rs | 614 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/lib.rs | 639 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 58 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 162 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 210 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 215 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 220 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 243 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 299 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/lib.rs | 486 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/lib.rs | 508 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 50 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 96 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 123 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 132 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 141 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 148 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 162 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 192 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 415 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 421 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 461 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fuzz-build-gate | crates/fuzz-build-gate/src/lib.rs | 401 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| gate-runner | crates/gate-runner/src/main.rs | 189 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| gate-runner | crates/gate-runner/src/main.rs | 271 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 327 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 362 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 377 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/wake.rs | 161 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| installer | crates/installer/src/lib.rs | 605 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 622 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 658 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 744 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 758 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 818 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 845 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 857 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 867 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 902 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| kernel-only-operator-hook | crates/kernel-only-operator-hook/src/shadow.rs | 266 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 31 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 116 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 131 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/lib.rs | 464 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 482 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 508 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 523 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 755 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 759 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 837 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 958 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1035 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1096 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1117 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1140 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1424 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1458 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1462 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1526 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1557 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1587 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1607 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1626 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1675 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1802 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1822 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/main.rs | 244 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/scheduled_lane_telemetry.rs | 39 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-tick | crates/loop-tick/src/lib.rs | 104 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 470 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 591 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 621 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 642 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 688 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 798 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 849 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 998 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 1097 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-push-gate.rs | 149 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-push-gate.rs | 232 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/commit_serialization.rs | 406 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| no-shell-gate | crates/no-shell-gate/src/head_compiles.rs | 125 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| no-shell-gate | crates/no-shell-gate/src/head_compiles.rs | 140 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| no-shell-gate | crates/no-shell-gate/src/head_compiles.rs | 215 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| no-shell-gate | crates/no-shell-gate/src/head_compiles.rs | 278 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| no-shell-gate | crates/no-shell-gate/src/lib.rs | 148 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/lib.rs | 242 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-idle-dispatch | crates/omp-idle-dispatch/src/main.rs | 314 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-idle-dispatch | crates/omp-idle-dispatch/src/main.rs | 321 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-inventory-map | crates/omp-inventory-map/src/lib.rs | 1583 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/lib.rs | 700 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 690 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 725 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 4072 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 8653 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/resident_tick.rs | 466 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| omp-orchestrator | crates/omp-orchestrator/src/target_directory.rs | 349 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/target_directory.rs | 400 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/lib.rs | 136 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/lib.rs | 181 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/main.rs | 136 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 37 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 152 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 189 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| ompo-doctor | crates/ompo-doctor/src/lib.rs | 171 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 239 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 386 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 398 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 430 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 444 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 451 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 151 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 163 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 181 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-fence | crates/pane-dispatch-fence/src/main.rs | 153 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pane-dispatch-fence | crates/pane-dispatch-fence/src/main.rs | 172 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 443 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 672 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 695 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 43 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 47 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 255 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 273 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 303 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 314 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 335 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 173 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 184 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 193 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 229 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 485 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 525 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 578 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 859 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| porting-gate | crates/porting-gate/src/lib.rs | 312 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 325 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 337 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 359 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pre-delete-citation-check | crates/pre-delete-citation-check/src/main.rs | 20 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pre-delete-citation-check | crates/pre-delete-citation-check/src/main.rs | 73 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| preregistration-gate | crates/preregistration-gate/src/lib.rs | 383 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 236 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 262 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 501 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 516 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 653 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 676 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/main.rs | 49 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/main.rs | 384 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| receiver-receipt | crates/receiver-receipt/src/bin/receiver-receipt.rs | 31 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| refill-idle-panes | crates/refill-idle-panes/src/main.rs | 61 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| refill-idle-panes | crates/refill-idle-panes/src/main.rs | 127 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| refill-idle-panes | crates/refill-idle-panes/src/main.rs | 1073 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| s1-coverage | crates/s1-coverage/src/main.rs | 113 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| scratch-home | crates/scratch-home/src/main.rs | 236 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| silent-success-census | crates/silent-success-census/src/lib.rs | 250 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| staged-build-gate | crates/staged-build-gate/src/main.rs | 138 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| staged-build-gate | crates/staged-build-gate/src/main.rs | 237 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 268 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 274 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 388 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 394 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 844 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 855 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 876 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 890 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 918 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 946 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 954 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1052 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1083 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1102 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1127 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1145 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1200 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1207 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 1217 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 236 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 313 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 431 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 450 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 481 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 508 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 530 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 545 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 565 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 606 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 783 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 793 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 98 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| tick-monitor | crates/tick-monitor/src/lib.rs | 185 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 191 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 1539 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 220 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 506 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 888 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| wired-but-inert-guard | crates/wired-but-inert-guard/src/main.rs | 149 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| wired-but-inert-guard | crates/wired-but-inert-guard/src/main.rs | 157 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |

Raw sites triaged: **281**. undrained-pipe-lint violations: **0**.

## Scope and limits

The table measures only the eight greppable schema properties: `forbid_unsafe`, `dep_asupersync`, `dep_subprocess_contract`, `async_fns`, `cx_first`, `checkpoints`, `raw_command`, and `forbidden_deps`.

> **NO-CLAIM 1.** Every column is a **syntactic** fact. `cx_first` counts a parameter name, not
> that cancellation is honoured; `checkpoints` counts call sites, not that they sit in the loops
> that matter. A crate can score perfectly and still leak a detached task.
>
> **NO-CLAIM 2.** `raw_command` counts `Command::new` textually. A spawn built through a helper or
> a dynamically-constructed name is **invisible** to it, so the count is a **lower bound**.
>
> **NO-CLAIM 3.** Absent from this schema entirely, because they are not greppable: **region
> ownership** (no detached tasks), **kill the process GROUP not the pid**, **a timeout is not a
> verdict**, `Budget`/`Outcome`/capability narrowing, two-phase effects, and deterministic
> `LabRuntime` tests. Those need a semantic pass. Naming them here so their absence from the table
> is not read as their absence from the contract.
>
> **NO-CLAIM 4.** This document is generated from the current source, but generation does not prove
> the behavior of any process. The values below are a syntactic census, not a behavioral certificate.
>
> **NO-CLAIM 5.** The asupersync fabric is a messaging plane between components that both use it.
> Whether its permit/ack machinery can wrap a tmux pane that has never heard of asupersync is
> **UNMEASURED** — possibly **NOT APPLICABLE**. The finding is that we invented vocabulary that
> already exists, not that adoption is proven.
>
> The measured dependency contract is pinned to asupersync revision `fa3c01aec` (version 0.4.9).
> The local skill text naming v0.4.4 is stale relative to this pinned source; the pinned source wins.
