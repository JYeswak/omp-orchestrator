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
| admission-reason | Y | . | . | 0 | 0 | 0 | 3 | - |
| agent-mail-native | Y | Y | Y | 22 | 18 | 6 | 2 | - |
| asupersync-conformance | Y | . | . | 0 | 0 | 0 | 0 | - |
| bead-availability | Y | Y | Y | 2 | 2 | 2 | 1 | - |
| cargo-lane-budget | Y | . | . | 0 | 0 | 0 | 1 | - |
| commit-build-fence | Y | Y | . | 1 | 1 | 2 | 0 | - |
| composer-typed | Y | . | . | 0 | 0 | 0 | 0 | - |
| convergence-stamp | Y | . | . | 0 | 0 | 0 | 0 | - |
| crate-soundness-verify | Y | . | . | 0 | 0 | 0 | 1 | - |
| dispatch-claim-fence | Y | . | . | 0 | 0 | 0 | 0 | - |
| dispatch-silence-watch | Y | . | Y | 0 | 0 | 0 | 2 | - |
| dispatcher-deadman | Y | . | . | 0 | 0 | 0 | 3 | - |
| extraction-roster | Y | Y | Y | 2 | 0 | 0 | 1 | - |
| fast-dispatch | Y | . | Y | 0 | 0 | 0 | 32 | - |
| finding | Y | Y | Y | 1 | 0 | 1 | 0 | - |
| finding-dispatch | Y | . | . | 0 | 0 | 0 | 0 | - |
| fleet-composite | Y | . | . | 0 | 0 | 0 | 2 | - |
| fleet-monitor | Y | . | Y | 0 | 0 | 0 | 27 | - |
| fleet-reconcile | Y | . | . | 0 | 0 | 0 | 9 | - |
| fleet-truth | Y | . | . | 0 | 0 | 0 | 13 | - |
| inbox-monitor | Y | . | Y | 0 | 0 | 0 | 3 | - |
| installer | Y | . | Y | 0 | 0 | 0 | 10 | - |
| kernel-bypass-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| kernel-only-operator-hook | Y | Y | Y | 4 | 4 | 2 | 1 | - |
| loop-coverage | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-driver | Y | . | Y | 0 | 0 | 0 | 26 | - |
| loop-queue-filter | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-switch | Y | . | . | 0 | 0 | 0 | 0 | - |
| loop-tick | Y | . | Y | 0 | 0 | 0 | 1 | - |
| no-shell-gate | Y | . | Y | 0 | 0 | 0 | 11 | - |
| ntm-fleet-monitor | Y | . | . | 0 | 0 | 0 | 0 | - |
| omp-idle-dispatch | Y | . | . | 0 | 0 | 0 | 3 | - |
| omp-inventory-map | Y | Y | . | 3 | 3 | 3 | 1 | - |
| omp-orchestrator | Y | Y | Y | 18 | 18 | 3 | 7 | - |
| omp-rpc-session | Y | Y | . | 7 | 6 | 2 | 3 | - |
| omp-surface-consumption | Y | . | Y | 0 | 0 | 0 | 3 | - |
| omp-types | Y | Y | Y | 0 | 0 | 0 | 0 | - |
| oracle-compare | Y | . | . | 0 | 0 | 0 | 7 | - |
| oracle-pane-state-differential | Y | . | . | 0 | 0 | 0 | 3 | - |
| orchestration-tick-gate | Y | . | . | 0 | 0 | 0 | 0 | - |
| pane-dispatch-fence | Y | Y | Y | 1 | 0 | 0 | 2 | - |
| pane-dispatch-ready | Y | . | . | 0 | 0 | 0 | 10 | - |
| pane-oracle-diff | Y | . | . | 0 | 0 | 0 | 4 | - |
| pane-truth | Y | . | . | 0 | 0 | 0 | 4 | - |
| path-literal-guard | Y | . | . | 0 | 0 | 0 | 0 | - |
| plan-assemble | Y | Y | . | 2 | 0 | 0 | 0 | - |
| porting-gate | Y | Y | Y | 2 | 2 | 1 | 4 | - |
| pre-delete-citation-check | Y | . | . | 0 | 0 | 0 | 2 | - |
| preregistration-gate | Y | Y | Y | 4 | 2 | 1 | 1 | - |
| reap-finished-panes | Y | . | Y | 0 | 0 | 0 | 9 | - |
| receiver-receipt | Y | . | . | 0 | 0 | 0 | 1 | - |
| refill-idle-panes | Y | . | . | 0 | 0 | 0 | 2 | - |
| response-envelope-check | Y | . | . | 0 | 0 | 0 | 0 | - |
| scratch-home | Y | . | Y | 0 | 0 | 0 | 1 | - |
| silent-success-census | Y | . | . | 0 | 0 | 0 | 1 | - |
| staged-build-gate | Y | . | Y | 0 | 0 | 0 | 2 | - |
| state-wildcard-lint | Y | . | . | 0 | 0 | 0 | 0 | - |
| subprocess-contract | Y | Y | . | 2 | 2 | 1 | 14 | - |
| tick-dispatch | Y | . | . | 0 | 0 | 0 | 12 | - |
| tick-monitor | Y | . | . | 0 | 0 | 0 | 4 | - |
| undrained-pipe-lint | Y | . | . | 0 | 0 | 0 | 0 | - |
| verify-dispatch | Y | . | . | 0 | 0 | 0 | 3 | - |
| wired-but-inert-guard | Y | . | Y | 0 | 0 | 0 | 2 | - |

Crates scanned: **65**. forbid_unsafe: **65**. dep_asupersync: **16**. async_fns: **87**. cx_first: **66**. checkpoints: **28**. raw_command sites: **256**.

## Raw Command triage

Every discovered site is classified; no `UNTRIAGED` row is emitted. The lexical lint is also run over the same source set.

| crate | file | line | triage | reason |
|---|---|---:|---|---|
| ack-spine | crates/ack-spine/src/ack.rs | 69 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| ack-spine | crates/ack-spine/src/ack.rs | 87 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| admission-reason | crates/admission-reason/src/lib.rs | 315 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| admission-reason | crates/admission-reason/src/lib.rs | 889 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| admission-reason | crates/admission-reason/src/lib.rs | 908 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| agent-mail-native | crates/agent-mail-native/src/oracle.rs | 135 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| agent-mail-native | crates/agent-mail-native/src/wake.rs | 255 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| bead-availability | crates/bead-availability/src/lib.rs | 456 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| cargo-lane-budget | crates/cargo-lane-budget/src/lib.rs | 221 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| crate-soundness-verify | crates/crate-soundness-verify/src/lib.rs | 297 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatch-silence-watch | crates/dispatch-silence-watch/src/main.rs | 51 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| dispatch-silence-watch | crates/dispatch-silence-watch/src/main.rs | 65 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| dispatcher-deadman | crates/dispatcher-deadman/src/lib.rs | 285 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatcher-deadman | crates/dispatcher-deadman/src/lib.rs | 304 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| dispatcher-deadman | crates/dispatcher-deadman/src/main.rs | 181 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| extraction-roster | crates/extraction-roster/src/main.rs | 67 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 30 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 115 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fast-dispatch | crates/fast-dispatch/src/dispatch_cli_contract.rs | 127 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fast-dispatch | crates/fast-dispatch/src/lib.rs | 347 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/lib.rs | 663 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 117 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 153 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 171 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 240 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 311 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 327 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 341 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 351 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 373 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 399 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 429 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 453 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 465 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 494 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 517 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 531 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 538 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 574 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 847 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 857 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 881 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 945 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 997 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1138 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1231 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/main.rs | 1236 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fast-dispatch | crates/fast-dispatch/src/scheduled_lane_telemetry.rs | 37 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-composite | crates/fleet-composite/src/main.rs | 83 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| fleet-composite | crates/fleet-composite/src/main.rs | 167 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 31 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 114 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/dispatch_cli_contract.rs | 126 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 95 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 109 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/lock.rs | 125 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 208 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 266 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 545 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 566 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 632 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 665 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 696 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 952 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1036 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1066 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1108 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1192 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1255 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1318 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1323 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1354 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1436 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1507 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1517 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/main.rs | 1530 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-monitor | crates/fleet-monitor/src/scheduled_lane_telemetry.rs | 37 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/lib.rs | 660 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/lib.rs | 685 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 44 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 137 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 185 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 190 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 195 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 218 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-reconcile | crates/fleet-reconcile/src/main.rs | 274 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/lib.rs | 492 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/lib.rs | 517 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 37 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| fleet-truth | crates/fleet-truth/src/main.rs | 64 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 91 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 100 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 109 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 116 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 130 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 160 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 383 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 388 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| fleet-truth | crates/fleet-truth/src/main.rs | 430 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 179 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 214 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| inbox-monitor | crates/inbox-monitor/src/main.rs | 229 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 312 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 329 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 365 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 447 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 461 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| installer | crates/installer/src/lib.rs | 521 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 548 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 560 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 570 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| installer | crates/installer/src/lib.rs | 605 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| kernel-only-operator-hook | crates/kernel-only-operator-hook/src/shadow.rs | 220 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 31 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 116 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/dispatch_cli_contract.rs | 128 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 447 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 461 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 488 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 504 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 736 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 743 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 824 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 963 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1040 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1101 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1122 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1145 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1429 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1463 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1467 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/lib.rs | 1530 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1560 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| loop-driver | crates/loop-driver/src/lib.rs | 1588 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1609 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1629 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/lib.rs | 1677 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| loop-driver | crates/loop-driver/src/main.rs | 227 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-driver | crates/loop-driver/src/scheduled_lane_telemetry.rs | 37 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| loop-tick | crates/loop-tick/src/lib.rs | 104 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 266 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 426 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 456 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 477 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 523 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 633 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-commit-gate.rs | 684 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-push-gate.rs | 145 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/bin/pre-push-gate.rs | 228 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/lib.rs | 146 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| no-shell-gate | crates/no-shell-gate/src/lib.rs | 240 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-idle-dispatch | crates/omp-idle-dispatch/src/main.rs | 272 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-idle-dispatch | crates/omp-idle-dispatch/src/main.rs | 349 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-idle-dispatch | crates/omp-idle-dispatch/src/main.rs | 356 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-inventory-map | crates/omp-inventory-map/src/lib.rs | 1566 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-orchestrator | crates/omp-orchestrator/src/lib.rs | 449 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/lib.rs | 601 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 327 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 362 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-orchestrator | crates/omp-orchestrator/src/main.rs | 1538 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| omp-orchestrator | crates/omp-orchestrator/src/target_directory.rs | 318 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| omp-orchestrator | crates/omp-orchestrator/src/target_directory.rs | 341 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/lib.rs | 136 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/lib.rs | 181 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-rpc-session | crates/omp-rpc-session/src/main.rs | 136 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 37 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 152 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| omp-surface-consumption | crates/omp-surface-consumption/src/main.rs | 189 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| oracle-compare | crates/oracle-compare/src/lib.rs | 249 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 367 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 486 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 498 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 530 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 544 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-compare | crates/oracle-compare/src/lib.rs | 551 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 138 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 150 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| oracle-pane-state-differential | crates/oracle-pane-state-differential/src/main.rs | 168 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-fence | crates/pane-dispatch-fence/src/main.rs | 152 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pane-dispatch-fence | crates/pane-dispatch-fence/src/main.rs | 171 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 454 | DEADLOCK_SAFE_STDOUT_ONLY | only stdout is piped, so there is no pair of pipes to fill |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 700 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/lib.rs | 723 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 34 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 38 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 253 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 263 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 292 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 303 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-dispatch-ready | crates/pane-dispatch-ready/src/main.rs | 321 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 158 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 169 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 178 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-oracle-diff | crates/pane-oracle-diff/src/main.rs | 214 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 536 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 576 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 625 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| pane-truth | crates/pane-truth/src/lib.rs | 906 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| porting-gate | crates/porting-gate/src/lib.rs | 312 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 325 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 337 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| porting-gate | crates/porting-gate/src/lib.rs | 359 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| pre-delete-citation-check | crates/pre-delete-citation-check/src/main.rs | 13 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| pre-delete-citation-check | crates/pre-delete-citation-check/src/main.rs | 41 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| preregistration-gate | crates/preregistration-gate/src/lib.rs | 383 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 198 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 222 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 449 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 464 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 601 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/lib.rs | 624 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/main.rs | 48 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/main.rs | 339 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| reap-finished-panes | crates/reap-finished-panes/src/scheduled_lane_telemetry.rs | 37 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| receiver-receipt | crates/receiver-receipt/src/bin/receiver-receipt.rs | 29 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| refill-idle-panes | crates/refill-idle-panes/src/main.rs | 35 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| refill-idle-panes | crates/refill-idle-panes/src/main.rs | 249 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| scratch-home | crates/scratch-home/src/main.rs | 204 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| silent-success-census | crates/silent-success-census/src/lib.rs | 246 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| staged-build-gate | crates/staged-build-gate/src/main.rs | 138 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| staged-build-gate | crates/staged-build-gate/src/main.rs | 223 | ROUTED_THROUGH_SUBPROCESS_CONTRACT | the command reaches the repository drain-safe bounded runner |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 218 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 224 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 324 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 330 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 375 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 388 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 409 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 432 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 460 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 468 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 559 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 583 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 602 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| subprocess-contract | crates/subprocess-contract/src/lib.rs | 620 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 227 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 302 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 418 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 430 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 454 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 473 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 491 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 504 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 516 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 549 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 713 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-dispatch | crates/tick-dispatch/src/main.rs | 721 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 138 | DEADLOCK_SAFE_CONCURRENT_READERS | dedicated readers drain both pipes before the poll result is consumed |
| tick-monitor | crates/tick-monitor/src/lib.rs | 205 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 211 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| tick-monitor | crates/tick-monitor/src/lib.rs | 1311 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 219 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 504 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| verify-dispatch | crates/verify-dispatch/src/lib.rs | 886 | DEADLOCK_SAFE_WAIT_WITH_OUTPUT | wait_with_output drains captured output without a try_wait poll loop |
| wired-but-inert-guard | crates/wired-but-inert-guard/src/main.rs | 149 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |
| wired-but-inert-guard | crates/wired-but-inert-guard/src/main.rs | 157 | DEADLOCK_SAFE_NO_PIPES | the command site does not pipe stdout or stderr |

Raw sites triaged: **256**. undrained-pipe-lint violations: **0**.

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
