# o3eb / w21v packet grade

Packet delivery evidence: one ACK, `ACK o3eb-grade on %8 --`, covers both beads. No second ACK was requested or posted.

## o3eb

Target commit: `e4c9138d46556ddf4bdc7a0d6c196aed32be6cae`, `fix(process): pass separator before negative pgid [test]`.

Source-tree comparison using `git show`:

```text
REF=e4c9138^
subprocess-contract: 2 no-separator TERM/KILL sites for group
 tick-monitor:        1 no-separator TERM/KILL site for negated pgid
REF=e4c9138
subprocess-contract: 2 TERM + 2 KILL sites with [signal, --, &group]
 tick-monitor:        1 TERM + 1 KILL site with [signal, --, &neg]
```

The commit changes exactly the six negative-PGID invocations. `loop-driver`
uses positive PID arguments and is outside scope. The tick-monitor comment now
states the procps-ng requirement for `--`.

Remote current-tree targeted acceptance checks on Contabo 4, with the source
path clean for `subprocess-contract`, all returned exit 0:

```text
bounded_status_signals_the_group_so_grandchildren_die_too  1 passed
sleep_child_past_deadline_is_timedout_never_completed       1 passed
deadline_killed_child_is_reaped_not_orphaned                1 passed
bounded_status_kills_a_hung_child_and_refuses_to_call_it_completed 1 passed
process_count_returns_to_baseline_after_contract_runs       1 passed
```

The full parent-tree remote run was attempted from a `git archive e4c9138^`
export. Contabo 3 first refused with RCH-I001 worker admission; Contabo 4
compiled the tree but the process-source lock connection closed during the
process-kill tests. The parent aggregate terminal receipt is therefore
`UNKNOWN`, not treated as a test result. The parent source nevertheless has
all six pre-fix no-separator sites and the exact known-bad assertion messages
at `subprocess-contract/src/lib.rs:457,529,706,787`.

The Darwin shell leg was safe: candidate PGID `99991` was absent (`0` matches;
absence-check rc `1`) and `/bin/kill -KILL -- -99991` returned rc `1`,
`kill: -99991: No such process`.

Wiring is non-vacuous: 30+ non-own Cargo manifests depend on
`subprocess-contract`, and production source callers use
`subprocess_contract::bounded_output`, `bounded_status`, or `run_output`.
The crate itself terminates in real `Command::new` process probes.

## w21v

Authoritative `br show` read: status `grading`, assignee
`pane=%6;incarnation=1;agent=pane1-omp-claude`, description length `235`,
`acceptance_criteria` length `0`.

The bead is structurally ungradeable under the repository contract. Its
read-first description is not an acceptance specification. No acceptance was
invented, no source or test was accepted in its place, and it must remain in
grading until its author writes measurable `run X, expect Y` criteria.

## Decision

- o3eb: implementation evidence passes the source delta, current targeted
  behavior, Darwin leg, and wiring checks; parent dynamic aggregate remains
  explicitly UNKNOWN because RCH lost the terminal process-test receipt.
- w21v: REFUSED / STRUCTURALLY UNDISPATCHABLE due empty acceptance criteria.

NO-CLAIM: o3eb source and targeted behavior do not prove every external runtime
consumer is compatible. w21v has no closeable contract until acceptance exists.
