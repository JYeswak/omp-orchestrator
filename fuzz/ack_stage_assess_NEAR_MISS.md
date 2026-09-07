# 20 live near-miss ACK first-lines (NOMATCH)

Expectation: strict grammar `ACK <last-hyphen-segment> on <pane> -- ` does **not** match.
Source: `.beads/beads.db` comments, 2026-09-07. Live unmatched 126/441; 20 unique first-lines below.
Also compiled into `crates/ack-stage/tests/assess_invariants.rs` and `fuzz/fuzz_targets/ack_stage_assess.rs`.

| # | expect | issue_id | first line |
|---|---|---|---|
| 01 | NOMATCH | omp-orchestrator-ack-spine-oj6.2 | ACK DETECTOR LANDED (SilverWolf, pane 5, %1409) — … |
| 02 | NOMATCH | omp-orchestrator-gate-no-runner-j58 | ACK j58 received on %1408 -- … |
| 03 | NOMATCH | omp-orchestrator-kernel-gate-census-69i | ACK 69i on %1408 — … (em-dash, not ` -- `) |
| 04 | NOMATCH | omp-orchestrator-crate-reachability-census-44g | ACK 44g on %1409 — … |
| 05 | NOMATCH | omp-orchestrator-crate-reachability-census-44g | ACK 44g on %1408 — … |
| 06 | NOMATCH | omp-orchestrator-followup-stage-180 | ACK 180 on %1409 — … |
| 07 | NOMATCH | omp-orchestrator-commit-msg-backtick-injection-232 | ACK from AmberGate (%1408): … |
| 08 | NOMATCH | omp-orchestrator-leht | ACK leht-grade on %1413 … |
| 09 | NOMATCH | omp-orchestrator-eg0m | ACK eg0m on %1408 |
| 10 | NOMATCH | omp-orchestrator-kxe.1 | ACK kxe.1 on %1414 |
| 11 | NOMATCH | omp-orchestrator-typed-degraded-dispatch-policy-i0kp | ACK i0kp on %1413 |
| 12 | NOMATCH | omp-orchestrator-ack-spine-oj6.3 | ACK oj6.3 on %1414 |
| 13 | NOMATCH | omp-orchestrator-omp-coverage-mission-ipg.19 | ACK ipg.19 on %1408 |
| 14 | NOMATCH | omp-orchestrator-receiver-receipt-contract-pwm | ACK pwm on %1408 |
| 15 | NOMATCH | omp-orchestrator-remove-unattributed-scratch-fallback-7bp | ACK 7bp on %1408 |
| 16 | NOMATCH | omp-orchestrator-installer-dep-uncommitted-txl | ACK txl on %1408 |
| 17 | NOMATCH | omp-orchestrator-finding-l1-bypassable-py3 | ACK py3 on %1408 |
| 18 | NOMATCH | omp-orchestrator-readiness-l1-wedge-blind-46y7 | ACK 46y7 on %1408 |
| 19 | NOMATCH | omp-orchestrator-observation-state-false-idle-riqd | ACK riqd on %1408 |
| 20 | NOMATCH | omp-orchestrator-monitor-reads-oracle-y256 | ACK y256 on %1408 |

Contrast MATCH (not in this 20): `ACK qhl on %1413 -- read existing ACK stage…`
