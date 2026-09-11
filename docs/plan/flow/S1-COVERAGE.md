# S1 Coverage Matrix

Generated 2026-09-06T03:44Z by the Validation command below. **Never hand-edit the table.**

`S1_REQUIREMENTS=476 COVERED=111 MISSING=365 DOC_ONLY=0`

TREE (git show HEAD of six contracts): stable-id occurrences summed=212 named-tests summed=59. Pane 1's census was 63 ids / 15 tests against TREE; this run's TREE tests=59. WORKTREE ids/tests are larger because L0/L1/L2 contracts are dirty in this worktree.
A sha names a TREE. This command reads the worktree. Re-run after checkout of a clean tree to get TREE counts.

## Headline (WORKTREE)

`S1_REQUIREMENTS=476 COVERED=111 MISSING=365 DOC_ONLY=0`

PX-P0 exit predicate is `MISSING = 0`. DOC-ONLY is not a way to zero MISSING; every DOC-ONLY row carries a reason.

## Source counts (producing command = this generator)

| source | n | COVERED | MISSING | DOC_ONLY |
|---|---:|---:|---:|---:|
| `contract.stable_id` | 212 | 69 | 143 | 0 |
| `contract.named_test` | 59 | 37 | 22 | 0 |
| `box.gap` | 10 | 0 | 10 | 0 |
| `box.observability` | 36 | 1 | 35 | 0 |
| `box.hook` | 96 | 0 | 96 | 0 |
| `box.branch.diagram` | 20 | 0 | 20 | 0 |
| `layer.exists` | 6 | 0 | 6 | 0 |
| `box.branch.test` | 20 | 0 | 20 | 0 |
| `decisions.HD` | 8 | 4 | 4 | 0 |
| `crate-atom.L0` | 9 | 0 | 9 | 0 |

## Seventh source (the six-source denominator is incomplete)

Added because pane 1 asked for an attack, not adoption:

1. `layer.exists` — 6 rows. `exists = "none"` is a MISSING crate, not a documented skip.
2. `box.branch.test` — 20 rows. The box listed 20 branches 'each needing a diagram arm AND a test' as one count; the test half is a distinct requirement. Counting them as one makes MISSING=0 a lie.
3. `decisions.HD` — HD-0009..0012. S1 cannot leave L3/L4/hooks without them.
4. `crate-atom.L0` — nine parts on the one named S1 crate (`installer`). L1–L5 have no crate so they already fail `layer.exists`; applying 9 parts there would double-count the missing crate.

Not counted (cannot be gated): whether a branch string is a mermaid edge; whether a bead *implements* an ID. Join is now a word-boundary token in bead **title or acceptance_criteria**, not description prose.

## Join delta (substring → token in title/acceptance)

COVERED_before (loose substring on this same worktree pass) = 201
COVERED_after (token in title or acceptance) = 111
FALSE_COVERED = 90

Examples of false COVERED (loose hit, tight miss):

- `contract.stable_id` `L2-TEST-REMOTE-A` was joined to `omp-orchestrator-l2-test-remote-a-996z` by substring
- `contract.stable_id` `L1-BUILD-DOCTOR` was joined to `omp-orchestrator-l1-build-doctor-25u5` by substring
- `contract.stable_id` `L1-BUILD-SCOPE` was joined to `omp-orchestrator-l1-build-scope-myw3` by substring
- `contract.stable_id` `L1-BUILD-PROBE-TMUX` was joined to `omp-orchestrator-l1-build-probe-tmux-pdpb` by substring
- `contract.stable_id` `L1-BUILD-PROBE-NTM` was joined to `omp-orchestrator-l1-build-probe-ntm-mqag` by substring
- `contract.stable_id` `L1-BUILD-PROBE-BR` was joined to `omp-orchestrator-l1-build-probe-br-g4pg` by substring
- `contract.stable_id` `L1-BUILD-PROBE-BV` was joined to `omp-orchestrator-l1-build-probe-bv-lebe` by substring
- `contract.stable_id` `L1-BUILD-PROBE-AGENT-MAIL` was joined to `omp-orchestrator-l1-build-probe-agent-mail-pev8` by substring

## Matrix

| source | stable_id | requirement | bead_id | state |
|---|---|---|---|---|
| `contract.stable_id` | `L0-PLATFORM-TRIPLE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b01-3vro` | COVERED |
| `contract.stable_id` | `L0-VERIFY-SHA256` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b03-pp2o` | COVERED |
| `contract.stable_id` | `L0-VERIFY-MINISIGN` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b04-a6nx` | COVERED |
| `contract.stable_id` | `L0-VERIFY-SIGSTORE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b05-qsyg` | COVERED |
| `contract.stable_id` | `L0-ATOMIC-RENAME` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b06-vz1p` | COVERED |
| `contract.stable_id` | `L0-DURABILITY-PARENT` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b07-h6pu` | COVERED |
| `contract.stable_id` | `L0-DURABILITY-FULLFSYNC` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b07-h6pu` | COVERED |
| `contract.stable_id` | `L0-PATH-COLLISION` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b08-u7tm` | COVERED |
| `contract.stable_id` | `L0-HOOK-MERGE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b10-3r1g` | COVERED |
| `contract.stable_id` | `L0-SKILLS` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b11-uegf` | COVERED |
| `contract.stable_id` | `L0-UNINSTALL` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b14-f2jh` | COVERED |
| `contract.stable_id` | `L0-EVENT` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b09-x282` | COVERED |
| `contract.stable_id` | `L0-REPORT` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b12-rmr2` | COVERED |
| `contract.stable_id` | `L0-MONITOR` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b09-x282` | COVERED |
| `contract.stable_id` | `L0-GATE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b15-ucvv` | COVERED |
| `contract.stable_id` | `L0-METRIC` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b07-h6pu` | COVERED |
| `contract.stable_id` | `LAW-L0-FAIL-CLOSED` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-law-fail-closed-d8v5` | COVERED |
| `contract.stable_id` | `LAW-L0-ATOMIC-DURABLE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-law-atomic-durable-rfs8` | COVERED |
| `contract.stable_id` | `LAW-L0-RESTORE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-law-restore-ylsu` | COVERED |
| `contract.stable_id` | `LAW-L0-REPORT-BEFORE-SUCCESS` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-law-report-before-success-lxdb` | COVERED |
| `contract.stable_id` | `LAW-L0-OBSERVABLE-REFUSAL` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-t21-f6si` | COVERED |
| `contract.stable_id` | `LAW-L0-IDENTITY-READBACK` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-t18-sk8h` | COVERED |
| `contract.stable_id` | `OBS-L0-EVENT` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b15-ucvv` | COVERED |
| `contract.stable_id` | `OBS-L0-REPORT` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b15-ucvv` | COVERED |
| `contract.stable_id` | `OBS-L0-MONITOR` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b15-ucvv` | COVERED |
| `contract.stable_id` | `OBS-L0-GATE` | stable id in s1_l0_install.md | `omp-orchestrator-s1-l0-b15-ucvv` | COVERED |
| `contract.stable_id` | `L2-TEST-REMOTE-A` | stable id in s1_l0_install.md | `—` | MISSING |
| `contract.stable_id` | `L1-INPUT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-VERDICT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-REPAIR` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-UNDO` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-IDEMPOTENCE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-DRIFT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-SCOPE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-EXHAUSTIVE-ARMS` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-WRONG-VERSION-STALE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-UNRUN-NOT-REFUSED` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-PAUSED-DISTINCT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-HD-NAMED` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-VERDICT-EXHAUSTIVE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-UNRUN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-WRONG-VERSION` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-PAUSED` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-UNKNOWN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-TWO-SIGNALS` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-UNKNOWN-LOUD` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-ONE-MUTATOR` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-BEFORE-HASH` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-CONDITIONAL-IDEMPOTENCE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1P-REPAIR-THEN-REPROBE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-SCOPED-PROBE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-TWO-SIGNALS` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-MUTATE-AUDIT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-CONDITIONAL-IDEMPOTENCE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-UNDO` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L1-REPROBE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-DOCTOR` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-SCOPE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-TMUX` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-NTM` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-BR` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-BV` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-AGENT-MAIL` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-SOCRATICODE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-RCH` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-GIT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-DISK` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-FRANKENMERMAID` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-PROBE-TOOLCHAIN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-TWO-SIGNALS` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-VERDICT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-REMEDIATION` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-EXIT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-MUTATE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-UNDO` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-BUILD-REPROBE-IDEMPOTENCE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-TMUX` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-NTM` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-BR` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-BV` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-AGENT-MAIL` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-SOCRATICODE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-RCH` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-GIT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-DISK` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-FRANKENMERMAID` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-PROBE-TOOLCHAIN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-TWO-SIGNAL` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-VERDICT-ARMS` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-TIMEOUT-UNRUN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-ABSENT-REMEDIATION` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-EXIT-LATTICE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-MUTATE-BACKUP` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-UNDO` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-IDEMPOTENT-SAME` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-IDEMPOTENT-DRIFT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-REPROBE-HALT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L1-TEST-SCOPE-UNKNOWN` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L1-PROBE` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L1-VERDICT` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L1-MUTATION` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L1-UNDO` | stable id in s1_l1_doctor.md | `—` | MISSING |
| `contract.stable_id` | `L2-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TRUST` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-SNAPSHOT` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-INCEPTION` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-FOUNDATION` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-REPROBE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-SCOPE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-IDEMPOTENCE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-IDENTITY-BEFORE-WRITE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-NO-FOREIGN-OVERWRITE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-BACKUP-BEFORE-HASH` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-INCEPTION-COMPLETE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-FOUNDATION-LINK` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-REPROBE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-CONDITIONAL-IDEMPOTENCE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2P-SCOPE-UNKNOWN` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-TRUSTED-INIT` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-BACKUP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-INCEPTION` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-REPROBE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-CONDITIONAL-IDEMPOTENCE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `LAW-L2-SCOPE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-GIT-REPO` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-REMOTE-PERSONA-A` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-REMOTE-PERSONA-BC` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-AGENTS-STAMP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-CLAUDE-STAMP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-BEADS` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-RUST-TOOLCHAIN` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-HOOK-HEAD` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-CARGO-MEMBERS` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-AGENT-MAIL` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-RCH-LANE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-TRUSTED-INIT-OPTIN` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-TEMPLATE-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-BACKUP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-INCEPTION-FOUNDATION` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-REPROBE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-BUILD-HALT-NOT-TAKEN` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-GIT-REPO` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-REMOTE-A` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-REMOTE-BC` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-AGENTS-STAMP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-CLAUDE-STAMP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-BEADS` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-RUST-TOOLCHAIN` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-HOOK-HEAD` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-CARGO-MEMBERS` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-AGENT-MAIL` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-RCH-LANE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-TRUSTED-INIT` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-TEMPLATE-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-BACKUP` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-INCEPTION-FOUNDATION` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-REPROBE-SUCCESS` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-REPROBE-FAIL` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-IDEMPOTENT-SAME` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-IDEMPOTENT-DRIFT` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-EPISTEMIC-COMPLETE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L2-TEST-ATOMIC-ROLLBACK` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L2-IDENTITY` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L2-TRUST` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L2-WRITE` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `OBS-L2-READBACK` | stable id in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.stable_id` | `L3-ARRAY` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-array-zr1g` | COVERED |
| `contract.stable_id` | `L3-STEP` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-step-kymv` | COVERED |
| `contract.stable_id` | `L3-TUI` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-tui-l7k8` | COVERED |
| `contract.stable_id` | `L3-JSON` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-json-my9i` | COVERED |
| `contract.stable_id` | `L3-HD0009` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-hd0009-lo3g` | COVERED |
| `contract.stable_id` | `L3-SKIPPED` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-skipped-t4k9` | COVERED |
| `contract.stable_id` | `L3-NEXT` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-next-0ne8` | COVERED |
| `contract.stable_id` | `L3-OBS-COUNT` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-obs-count-st8w` | COVERED |
| `contract.stable_id` | `L3-OBS-PARITY` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-obs-parity-xar6` | COVERED |
| `contract.stable_id` | `L3-OBS-CURSOR` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-obs-cursor-jb5m` | COVERED |
| `contract.stable_id` | `L3-OBS-HD0009` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-obs-hd0009-n5tt` | COVERED |
| `contract.stable_id` | `L3-METRIC-DIVERGENT-IDS` | stable id in s1_l3_walkthrough.md | `omp-orchestrator-s1-l3-metric-divergent-hli2` | COVERED |
| `contract.stable_id` | `L4-SRC-NTM` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-src-ntm-2yrg` | COVERED |
| `contract.stable_id` | `L4-SRC-TICK` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-src-tick-1u9f` | COVERED |
| `contract.stable_id` | `L4-SRC-MAIL` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-src-mail-fols` | COVERED |
| `contract.stable_id` | `L4-SILENT` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-silent-gbyo` | COVERED |
| `contract.stable_id` | `L4-LIVE` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-live-hw99` | COVERED |
| `contract.stable_id` | `L4-NOT-LIVE` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-not-live-8r0r` | COVERED |
| `contract.stable_id` | `L4-SPAWN` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-spawn-mail-reg-b8z3` | COVERED |
| `contract.stable_id` | `L4-CX` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-cx-i0mv` | COVERED |
| `contract.stable_id` | `LAW-L4-THREE-FRESH` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-test-three-fresh-3feu` | COVERED |
| `contract.stable_id` | `LAW-L4-SILENT-NOT-LIVE` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-test-silent-third-8qd7` | COVERED |
| `contract.stable_id` | `LAW-L4-NO-TMUX-RAW` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-test-no-tmux-raw-d9d7` | COVERED |
| `contract.stable_id` | `LAW-L4-MAP-TICK` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-test-missing-gap-z7dj` | COVERED |
| `contract.stable_id` | `LAW-L4-SPAWN-CX` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-test-cancelled-qoac` | COVERED |
| `contract.stable_id` | `L4-OBS-NTM` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-obs-ntm-xl56` | COVERED |
| `contract.stable_id` | `L4-OBS-TICK` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-obs-tick-17nw` | COVERED |
| `contract.stable_id` | `L4-OBS-MAIL` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-obs-mail-l2de` | COVERED |
| `contract.stable_id` | `L4-OBS-AGREE` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-obs-agree-l7ve` | COVERED |
| `contract.stable_id` | `L4-METRIC-SILENT-COUNT` | stable id in s1_l4_liveness.md | `omp-orchestrator-s1-l4-metric-silent-vdxb` | COVERED |
| `contract.stable_id` | `L5-SCHEMA` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-schema-ciay` | COVERED |
| `contract.stable_id` | `L5-HASH` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-hash-jbmy` | COVERED |
| `contract.stable_id` | `L5-SOURCE` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-source-van0` | COVERED |
| `contract.stable_id` | `L5-ALERT` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-alert-cqwo` | COVERED |
| `contract.stable_id` | `L5-ONE-NEXT` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-one-next-qcev` | COVERED |
| `contract.stable_id` | `L5-INCEPTION` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-inception-y80i` | COVERED |
| `contract.stable_id` | `L5-READBACK` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-readback-3tek` | COVERED |
| `contract.stable_id` | `L5-CURSOR` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-cursor-enjg` | COVERED |
| `contract.stable_id` | `L5-OBS-SCHEMA` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-obs-schema-bbc8` | COVERED |
| `contract.stable_id` | `L5-OBS-HASH` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-obs-hash-ub2l` | COVERED |
| `contract.stable_id` | `L5-OBS-SOURCES` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-obs-sources-1o28` | COVERED |
| `contract.stable_id` | `L5-OBS-READBACK` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-obs-readback-zi3x` | COVERED |
| `contract.stable_id` | `L5-METRIC-READBACK-OK` | stable id in s1_l5_portal.md | `omp-orchestrator-s1-l5-metric-readback-fqgi` | COVERED |
| `contract.named_test` | `l0_install.rs::platform_triple_matrix` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t01-o34w` | COVERED |
| `contract.named_test` | `l0_install.rs::tampered_sha256_refuses` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t03-26ou` | COVERED |
| `contract.named_test` | `l0_install.rs::required_minisign_missing_refuses` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t04-kqne` | COVERED |
| `contract.named_test` | `l0_install.rs::cosign_below_cve_floor_refuses` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t05-xe16` | COVERED |
| `contract.named_test` | `l0_install.rs::sigstore_identity_mismatch_refuses` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t06-3ga8` | COVERED |
| `contract.named_test` | `l0_install.rs::bare_installer_path_collision_refuses` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t11-andf` | COVERED |
| `contract.named_test` | `l0_install.rs::staging_failure_leaves_no_destination` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t07-ab3a` | COVERED |
| `contract.named_test` | `l0_install.rs::atomic_publish_renames_complete_artifact` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t08-aqao` | COVERED |
| `contract.named_test` | `l0_install.rs::parent_directory_fsync_is_required` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t09-jx83` | COVERED |
| `contract.named_test` | `l0_install.rs::fullfsync_failure_is_restrictive` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t10-6idp` | COVERED |
| `contract.named_test` | `l0_install.rs::durability_metric_counts_missing_parent_sync` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t22-bsfy` | COVERED |
| `contract.named_test` | `l0_install.rs::hook_merge_failure_restores_before_hash` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t15-tzul` | COVERED |
| `contract.named_test` | `l0_install.rs::installed_identity_must_match_readback` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t18-sk8h` | COVERED |
| `contract.named_test` | `l0_install.rs::event_and_report_precede_success` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t20-i6ki` | COVERED |
| `contract.named_test` | `l0_install.rs::known_bad_legs_name_reason` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t21-f6si` | COVERED |
| `contract.named_test` | `l0_install.rs::per_agent_summary_is_complete` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t17-sado` | COVERED |
| `contract.named_test` | `l0_install.rs::failed_precondition_is_restrictive` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-law-fail-closed-d8v5` | COVERED |
| `contract.named_test` | `l0_install.rs::publication_requires_both_sync_boundaries` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-law-atomic-durable-rfs8` | COVERED |
| `contract.named_test` | `l0_install.rs::merge_failure_restores_before_hash` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-law-restore-ylsu` | COVERED |
| `contract.named_test` | `l0_install.rs::success_requires_event_and_report` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-law-report-before-success-lxdb` | COVERED |
| `contract.named_test` | `l0_install.rs::musl_fallback_requires_gnu_artifact` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t02-mauc` | COVERED |
| `contract.named_test` | `l0_install.rs::ten_agent_detection_is_complete` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t12-6t0k` | COVERED |
| `contract.named_test` | `l0_install.rs::zero_agents_is_error_not_clean` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t13-kvw6` | COVERED |
| `contract.named_test` | `l0_install.rs::hook_backup_matches_mutations` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t14-pccb` | COVERED |
| `contract.named_test` | `l0_install.rs::skill_install_covers_detected_agents` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t16-m61c` | COVERED |
| `contract.named_test` | `l0_install.rs::uninstall_leaves_no_owned_paths` | named test in s1_l0_install.md | `omp-orchestrator-s1-l0-t19-shc0` | COVERED |
| `contract.named_test` | `file.rs::function` | named test in s1_l0_install.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::unknown_arm_is_instrument_error` | named test in s1_l1_doctor.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::timeout_is_unrun` | named test in s1_l1_doctor.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::wrong_version_is_stale` | named test in s1_l1_doctor.md | `omp-orchestrator-l1-test-two-signal-wrbx` | COVERED |
| `contract.named_test` | `l1_doctor.rs::paused_is_not_unprobeable` | named test in s1_l1_doctor.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::unknown_record_is_unmeasured` | named test in s1_l1_doctor.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::scope_does_not_vacuously_pass_skipped_probes` | named test in s1_l1_doctor.md | `—` | MISSING |
| `contract.named_test` | `l1_doctor.rs::repair_records_before_hash_backup_after_hash` | named test in s1_l1_doctor.md | `omp-orchestrator-l1-test-mutate-backup-kyng` | COVERED |
| `contract.named_test` | `l1_doctor.rs::second_repair_is_zero_actions_given_identical_hashes` | named test in s1_l1_doctor.md | `omp-orchestrator-l1-test-idempotent-same-s12x` | COVERED |
| `contract.named_test` | `l1_doctor.rs::undo_restores_before_hash` | named test in s1_l1_doctor.md | `omp-orchestrator-l1-test-undo-3oq5` | COVERED |
| `contract.named_test` | `l1_doctor.rs::failed_reprobe_blocks_l2` | named test in s1_l1_doctor.md | `omp-orchestrator-l1-test-reprobe-halt-jgat` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::identity_includes_root_and_revision` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-identity-1d7i` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::foreign_policy_without_opt_in_is_read_only` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-trusted-init-l0ku` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::init_backup_precedes_write` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-backup-6au3` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::inception_and_foundation_are_linked` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-inception-foundation-mumf` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::failed_reprobe_halts_l3` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-reprobe-fail-0oqo` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::second_init_is_zero_writes_given_identical_hashes` | named test in s1_l2_ecosystem.md | `omp-orchestrator-l2-test-idempotent-same-yi8a` | COVERED |
| `contract.named_test` | `l2_ecosystem.rs::scope_preserves_unknowns` | named test in s1_l2_ecosystem.md | `—` | MISSING |
| `contract.named_test` | `l3_step_parity.rs::both_renderers_borrow_the_same_array` | named test in s1_l3_walkthrough.md | `—` | MISSING |
| `contract.named_test` | `l3_step_parity.rs::ordered_ids_tui_json_steps` | named test in s1_l3_walkthrough.md | `—` | MISSING |
| `contract.named_test` | `l3_step_parity.rs::skipped_steps_remain_in_both_renders` | named test in s1_l3_walkthrough.md | `—` | MISSING |
| `contract.named_test` | `l3_step_parity.rs::undecided_hd0009_blocks_tail` | named test in s1_l3_walkthrough.md | `—` | MISSING |
| `contract.named_test` | `l3_step_parity.rs::second_start_same_ids` | named test in s1_l3_walkthrough.md | `—` | MISSING |
| `contract.named_test` | `l4_liveness.rs::live_requires_three_fresh_agreeing` | named test in s1_l4_liveness.md | `—` | MISSING |
| `contract.named_test` | `l4_liveness.rs::silent_third_is_not_live` | named test in s1_l4_liveness.md | `—` | MISSING |
| `contract.named_test` | `l4_liveness.rs::no_bool_tmux_source` | named test in s1_l4_liveness.md | `—` | MISSING |
| `contract.named_test` | `l4_liveness.rs::missing_gap_is_silent` | named test in s1_l4_liveness.md | `—` | MISSING |
| `contract.named_test` | `l4_liveness.rs::spawn_cancelled_is_named` | named test in s1_l4_liveness.md | `—` | MISSING |
| `contract.named_test` | `l5_portal.rs::missing_envelope_field_is_not_a_row` | named test in s1_l5_portal.md | `—` | MISSING |
| `contract.named_test` | `l5_portal.rs::one_next_action_is_object` | named test in s1_l5_portal.md | `—` | MISSING |
| `contract.named_test` | `l5_portal.rs::write_zero_readback_fail_refuses` | named test in s1_l5_portal.md | `—` | MISSING |
| `contract.named_test` | `l5_portal.rs::inception_fsyncs_parent_dir` | named test in s1_l5_portal.md | `—` | MISSING |
| `contract.named_test` | `l5_portal.rs::hash_stable_under_hash_field` | named test in s1_l5_portal.md | `—` | MISSING |
| `box.gap` | `GAP-1` | L1-L5 crates do not exist (ompo doctor/init/start/portal) | `—` | MISSING |
| `box.gap` | `GAP-2` | installer crate name collides with /usr/sbin/installer | `—` | MISSING |
| `box.gap` | `GAP-3` | Persona A (08-end-users solo, gates-only) had no skip-swarm branch | `—` | MISSING |
| `box.gap` | `GAP-4` | three S1 names: 12-journey Inception vs boxes/S1 human-start vs spine.mmd S1 inception | `—` | MISSING |
| `box.gap` | `GAP-5` | no LifecycleEvent writer for L0-L5 | `—` | MISSING |
| `box.gap` | `GAP-6` | L1 doctor SOTA is br doctor mutate()+undo by before_hash; ompo doctor is none | `—` | MISSING |
| `box.gap` | `GAP-7` | SessionStart hook is 0; launchd paused; slash start does not exist | `—` | MISSING |
| `box.gap` | `GAP-8` | crate field was unparseable composite; umbrella still MISSING | `—` | MISSING |
| `box.gap` | `GAP-9` | the eight box-hook rows carry surface/hook/status, not the twelve certified fields (id, event, matcher, class, fail_mode | `—` | MISSING |
| `box.gap` | `GAP-10` | S1 has no registered refusal type: Code::Refused has 0 call sites and no enum Code exists in crates/*/src, while 31 file | `—` | MISSING |
| `box.observability` | `L0.event_row` | L0 observability event_row | `—` | MISSING |
| `box.observability` | `L0.artifact` | L0 observability artifact | `—` | MISSING |
| `box.observability` | `L0.monitor` | L0 observability monitor | `—` | MISSING |
| `box.observability` | `L0.gate` | L0 observability gate | `—` | MISSING |
| `box.observability` | `L0.known_bad` | L0 observability known-bad | `—` | MISSING |
| `box.observability` | `L0.metric` | L0 observability metric | `omp-orchestrator-s1-l0-b07-h6pu` | COVERED |
| `box.observability` | `L1.event_row` | L1 observability event_row | `—` | MISSING |
| `box.observability` | `L1.artifact` | L1 observability artifact | `—` | MISSING |
| `box.observability` | `L1.monitor` | L1 observability monitor | `—` | MISSING |
| `box.observability` | `L1.gate` | L1 observability gate | `—` | MISSING |
| `box.observability` | `L1.known_bad` | L1 observability known-bad | `—` | MISSING |
| `box.observability` | `L1.metric` | L1 observability metric | `—` | MISSING |
| `box.observability` | `L2.event_row` | L2 observability event_row | `—` | MISSING |
| `box.observability` | `L2.artifact` | L2 observability artifact | `—` | MISSING |
| `box.observability` | `L2.monitor` | L2 observability monitor | `—` | MISSING |
| `box.observability` | `L2.gate` | L2 observability gate | `—` | MISSING |
| `box.observability` | `L2.known_bad` | L2 observability known-bad | `—` | MISSING |
| `box.observability` | `L2.metric` | L2 observability metric | `—` | MISSING |
| `box.observability` | `L3.event_row` | L3 observability event_row | `—` | MISSING |
| `box.observability` | `L3.artifact` | L3 observability artifact | `—` | MISSING |
| `box.observability` | `L3.monitor` | L3 observability monitor | `—` | MISSING |
| `box.observability` | `L3.gate` | L3 observability gate | `—` | MISSING |
| `box.observability` | `L3.known_bad` | L3 observability known-bad | `—` | MISSING |
| `box.observability` | `L3.metric` | L3 observability metric | `—` | MISSING |
| `box.observability` | `L4.event_row` | L4 observability event_row | `—` | MISSING |
| `box.observability` | `L4.artifact` | L4 observability artifact | `—` | MISSING |
| `box.observability` | `L4.monitor` | L4 observability monitor | `—` | MISSING |
| `box.observability` | `L4.gate` | L4 observability gate | `—` | MISSING |
| `box.observability` | `L4.known_bad` | L4 observability known-bad | `—` | MISSING |
| `box.observability` | `L4.metric` | L4 observability metric | `—` | MISSING |
| `box.observability` | `L5.event_row` | L5 observability event_row | `—` | MISSING |
| `box.observability` | `L5.artifact` | L5 observability artifact | `—` | MISSING |
| `box.observability` | `L5.monitor` | L5 observability monitor | `—` | MISSING |
| `box.observability` | `L5.gate` | L5 observability gate | `—` | MISSING |
| `box.observability` | `L5.known_bad` | L5 observability known-bad | `—` | MISSING |
| `box.observability` | `L5.metric` | L5 observability metric | `—` | MISSING |
| `box.hook` | `HOOK-1-id` | hook-1 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-1-event` | hook-1 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-1-matcher` | hook-1 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-1-class` | hook-1 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-1-fail_mode` | hook-1 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-1-binary` | hook-1 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-1-policy_file` | hook-1 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-1-source_commit` | hook-1 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-1-language` | hook-1 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-1-stage` | hook-1 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-1-certified` | hook-1 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-1-harm_class` | hook-1 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-2-id` | hook-2 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-2-event` | hook-2 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-2-matcher` | hook-2 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-2-class` | hook-2 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-2-fail_mode` | hook-2 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-2-binary` | hook-2 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-2-policy_file` | hook-2 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-2-source_commit` | hook-2 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-2-language` | hook-2 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-2-stage` | hook-2 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-2-certified` | hook-2 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-2-harm_class` | hook-2 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-3-id` | hook-3 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-3-event` | hook-3 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-3-matcher` | hook-3 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-3-class` | hook-3 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-3-fail_mode` | hook-3 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-3-binary` | hook-3 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-3-policy_file` | hook-3 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-3-source_commit` | hook-3 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-3-language` | hook-3 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-3-stage` | hook-3 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-3-certified` | hook-3 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-3-harm_class` | hook-3 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-4-id` | hook-4 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-4-event` | hook-4 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-4-matcher` | hook-4 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-4-class` | hook-4 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-4-fail_mode` | hook-4 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-4-binary` | hook-4 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-4-policy_file` | hook-4 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-4-source_commit` | hook-4 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-4-language` | hook-4 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-4-stage` | hook-4 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-4-certified` | hook-4 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-4-harm_class` | hook-4 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-5-id` | hook-5 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-5-event` | hook-5 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-5-matcher` | hook-5 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-5-class` | hook-5 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-5-fail_mode` | hook-5 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-5-binary` | hook-5 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-5-policy_file` | hook-5 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-5-source_commit` | hook-5 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-5-language` | hook-5 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-5-stage` | hook-5 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-5-certified` | hook-5 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-5-harm_class` | hook-5 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-6-id` | hook-6 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-6-event` | hook-6 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-6-matcher` | hook-6 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-6-class` | hook-6 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-6-fail_mode` | hook-6 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-6-binary` | hook-6 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-6-policy_file` | hook-6 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-6-source_commit` | hook-6 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-6-language` | hook-6 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-6-stage` | hook-6 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-6-certified` | hook-6 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-6-harm_class` | hook-6 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-7-id` | hook-7 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-7-event` | hook-7 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-7-matcher` | hook-7 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-7-class` | hook-7 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-7-fail_mode` | hook-7 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-7-binary` | hook-7 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-7-policy_file` | hook-7 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-7-source_commit` | hook-7 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-7-language` | hook-7 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-7-stage` | hook-7 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-7-certified` | hook-7 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-7-harm_class` | hook-7 certified field harm_class | `—` | MISSING |
| `box.hook` | `HOOK-8-id` | hook-8 certified field id | `—` | MISSING |
| `box.hook` | `HOOK-8-event` | hook-8 certified field event | `—` | MISSING |
| `box.hook` | `HOOK-8-matcher` | hook-8 certified field matcher | `—` | MISSING |
| `box.hook` | `HOOK-8-class` | hook-8 certified field class | `—` | MISSING |
| `box.hook` | `HOOK-8-fail_mode` | hook-8 certified field fail_mode | `—` | MISSING |
| `box.hook` | `HOOK-8-binary` | hook-8 certified field binary | `—` | MISSING |
| `box.hook` | `HOOK-8-policy_file` | hook-8 certified field policy_file | `—` | MISSING |
| `box.hook` | `HOOK-8-source_commit` | hook-8 certified field source_commit | `—` | MISSING |
| `box.hook` | `HOOK-8-language` | hook-8 certified field language | `—` | MISSING |
| `box.hook` | `HOOK-8-stage` | hook-8 certified field stage | `—` | MISSING |
| `box.hook` | `HOOK-8-certified` | hook-8 certified field certified | `—` | MISSING |
| `box.hook` | `HOOK-8-harm_class` | hook-8 certified field harm_class | `—` | MISSING |
| `box.branch.diagram` | `BR-1-diagram` | L0 checksum/cosign FAIL or missing checksum -> refuse supply chain | `—` | MISSING |
| `box.branch.diagram` | `BR-2-diagram` | L0 command -v ompo first hit != dest -> refuse name collision with PATH hits listed | `—` | MISSING |
| `box.branch.diagram` | `BR-3-diagram` | L0 agent merge created/merged/already/skipped -> L1 | `—` | MISSING |
| `box.branch.diagram` | `BR-4-diagram` | L0 agent merge failed -> refuse or degraded, not L1 healthy | `—` | MISSING |
| `box.branch.diagram` | `BR-5-diagram` | L1 ABSENT family -> halt with named remediation | `—` | MISSING |
| `box.branch.diagram` | `BR-6-diagram` | L1 ABSENT specific -> family probe required before L2 | `—` | MISSING |
| `box.branch.diagram` | `BR-7-diagram` | L1 UNPROBEABLE -> halt unless named HD degraded-continue | `—` | MISSING |
| `box.branch.diagram` | `BR-8-diagram` | L2 no git repository -> halt | `—` | MISSING |
| `box.branch.diagram` | `BR-9-diagram` | L2 repo without remote is allowed for persona A local-only | `—` | MISSING |
| `box.branch.diagram` | `BR-10-diagram` | L2 missing AGENTS.md/CLAUDE.md without trusted-init opt-in -> halt do not overwrite | `—` | MISSING |
| `box.branch.diagram` | `BR-11-diagram` | L2 trusted-init -> template with identity plus backup then re-probe; re-probe fail -> halt | `—` | MISSING |
| `box.branch.diagram` | `BR-12-diagram` | L2 hook identity != HEAD -> rebuild reinstall then re-probe; still mismatch -> halt | `—` | MISSING |
| `box.branch.diagram` | `BR-13-diagram` | L3 HD-0009 empty -> HUMAN HALT S9 | `—` | MISSING |
| `box.branch.diagram` | `BR-14-diagram` | L4 sources disagree -> HALT name the source | `—` | MISSING |
| `box.branch.diagram` | `BR-15-diagram` | L4 live -> L5 | `—` | MISSING |
| `box.branch.diagram` | `BR-16-diagram` | L4 not live persona A -> portal gates-only no spawn | `—` | MISSING |
| `box.branch.diagram` | `BR-17-diagram` | L4 not live persona B or C -> HD-0010 then ntm spawn --assign --cass-context via Cx | `—` | MISSING |
| `box.branch.diagram` | `BR-18-diagram` | L4 spawn partial -> refuse; post-spawn not live -> halt | `—` | MISSING |
| `box.branch.diagram` | `BR-19-diagram` | L5 decision owed -> HUMAN HALT S9 | `—` | MISSING |
| `box.branch.diagram` | `BR-20-diagram` | L5 else write inception.json required fields AND append FOUNDATION.jsonl stage=S1 AND jq readback, then S2 | `—` | MISSING |
| `layer.exists` | `L0.exists` | L0 exists=crates/installer: four verbs --check/--install <target>/--version/-h/--help plus | `—` | MISSING |
| `layer.exists` | `L1.exists` | L1 exists=none | `—` | MISSING |
| `layer.exists` | `L2.exists` | L2 exists=template exists in foundry; crate none | `—` | MISSING |
| `layer.exists` | `L3.exists` | L3 exists=none | `—` | MISSING |
| `layer.exists` | `L4.exists` | L4 exists=ntm spawn/--assign/--cass-context/templates exist; wrapper none | `—` | MISSING |
| `layer.exists` | `L5.exists` | L5 exists=none | `—` | MISSING |
| `box.branch.test` | `BR-1-test` | test for branch: L0 checksum/cosign FAIL or missing checksum -> refuse supply chain | `—` | MISSING |
| `box.branch.test` | `BR-2-test` | test for branch: L0 command -v ompo first hit != dest -> refuse name collision with PATH hits listed | `—` | MISSING |
| `box.branch.test` | `BR-3-test` | test for branch: L0 agent merge created/merged/already/skipped -> L1 | `—` | MISSING |
| `box.branch.test` | `BR-4-test` | test for branch: L0 agent merge failed -> refuse or degraded, not L1 healthy | `—` | MISSING |
| `box.branch.test` | `BR-5-test` | test for branch: L1 ABSENT family -> halt with named remediation | `—` | MISSING |
| `box.branch.test` | `BR-6-test` | test for branch: L1 ABSENT specific -> family probe required before L2 | `—` | MISSING |
| `box.branch.test` | `BR-7-test` | test for branch: L1 UNPROBEABLE -> halt unless named HD degraded-continue | `—` | MISSING |
| `box.branch.test` | `BR-8-test` | test for branch: L2 no git repository -> halt | `—` | MISSING |
| `box.branch.test` | `BR-9-test` | test for branch: L2 repo without remote is allowed for persona A local-only | `—` | MISSING |
| `box.branch.test` | `BR-10-test` | test for branch: L2 missing AGENTS.md/CLAUDE.md without trusted-init opt-in -> halt do not overwrite | `—` | MISSING |
| `box.branch.test` | `BR-11-test` | test for branch: L2 trusted-init -> template with identity plus backup then re-probe; re-probe fail -> halt | `—` | MISSING |
| `box.branch.test` | `BR-12-test` | test for branch: L2 hook identity != HEAD -> rebuild reinstall then re-probe; still mismatch -> halt | `—` | MISSING |
| `box.branch.test` | `BR-13-test` | test for branch: L3 HD-0009 empty -> HUMAN HALT S9 | `—` | MISSING |
| `box.branch.test` | `BR-14-test` | test for branch: L4 sources disagree -> HALT name the source | `—` | MISSING |
| `box.branch.test` | `BR-15-test` | test for branch: L4 live -> L5 | `—` | MISSING |
| `box.branch.test` | `BR-16-test` | test for branch: L4 not live persona A -> portal gates-only no spawn | `—` | MISSING |
| `box.branch.test` | `BR-17-test` | test for branch: L4 not live persona B or C -> HD-0010 then ntm spawn --assign --cass-context via Cx | `—` | MISSING |
| `box.branch.test` | `BR-18-test` | test for branch: L4 spawn partial -> refuse; post-spawn not live -> halt | `—` | MISSING |
| `box.branch.test` | `BR-19-test` | test for branch: L5 decision owed -> HUMAN HALT S9 | `—` | MISSING |
| `box.branch.test` | `BR-20-test` | test for branch: L5 else write inception.json required fields AND append FOUNDATION.jsonl stage=S1 AND jq readback, t | `—` | MISSING |
| `decisions.HD` | `HD-0009` | HD decision recorded; decided=False | `—` | MISSING |
| `decisions.HD` | `HD-0010` | HD decision recorded; decided=False | `—` | MISSING |
| `decisions.HD` | `HD-0011` | HD decision recorded; decided=False | `—` | MISSING |
| `decisions.HD` | `HD-0012` | HD decision recorded; decided=False | `—` | MISSING |
| `decisions.HD` | `HD-0009` | HD decision recorded; decided=True | `omp-orchestrator-cbp4` | COVERED |
| `decisions.HD` | `HD-0010` | HD decision recorded; decided=True | `omp-orchestrator-s1-l4-spawn-ol44` | COVERED |
| `decisions.HD` | `HD-0011` | HD decision recorded; decided=True | `omp-orchestrator-cbp4` | COVERED |
| `decisions.HD` | `HD-0012` | HD decision recorded; decided=True | `omp-orchestrator-hook-surface-probe-ex7e` | COVERED |
| `crate-atom.L0` | `ATOM-L0-ALL` | crate-atom-gate part ALL for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Lib` | crate-atom-gate part Lib for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Bin` | crate-atom-gate part Bin for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Verdict` | crate-atom-gate part Verdict for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Tests` | crate-atom-gate part Tests for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Fuzz` | crate-atom-gate part Fuzz for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Claim` | crate-atom-gate part Claim for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Slo` | crate-atom-gate part Slo for crates/installer | `—` | MISSING |
| `crate-atom.L0` | `ATOM-L0-Oracle` | crate-atom-gate part Oracle for crates/installer | `—` | MISSING |

## Validation

The generator is a tracked Rust crate, `crates/s1-coverage` (bin target `s1-coverage`). The
previously documented worktree-local Python runner is **retired**: a program a human pastes into an
interpreter is the same program as a committed `.py` file, and the no-shell gate keys on
`git ls-files` extensions. Regenerate with the sanctioned remote form:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run -j 2 -p s1-coverage -- --repo . --head HEAD --json
```

Compare two commits for the relation-based convergence decision:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run -j 2 -p s1-coverage -- --repo . --compare HEAD~1 HEAD --json
```

### Provenance states — the four the crate distinguishes

|state|instrument|what a count from it means|
|---|---|---|
|`TREE`|`git ls-tree -r HEAD`|a commit. The only clone-reproducible denominator.|
|`INDEX`|`git ls-files --cached`|what is staged. Agrees with TREE only on a clean index.|
|`WORKTREE`|`git diff --name-only HEAD`|local modification. Invisible to any clone.|
|`WORKTREE_ONLY`|`git ls-files --others --exclude-standard`|untracked input. Evidence of state divergence, **never** evidence about a commit.|

A non-empty `WORKTREE_ONLY` set is a refusal (`DENOMINATOR_WORKTREE_ONLY`, exit 2), not a smaller
denominator. An empty scan set is an error (`DENOMINATOR_EMPTY_SCAN_SET`, exit 3), never zero
missing and never `CONVERGING`. `decision` is `CONVERGING` exactly when closure exceeds growth and
`NON_CONVERGING` when growth is greater than or equal to closure; no absolute count is a target.
Wired executor: `.github/workflows/gate.yml`, step "S1 coverage provenance".

### When the CHECKOUT cannot answer — exit 4, and it is not a coverage defect

A revision the checkout cannot resolve refuses as `S1_COVERAGE_CHECKOUT_UNUSABLE revision=<rev>
cause=CHECKOUT_CANNOT_RESOLVE_REVISION`, **exit 4**, and the denominator is `UNKNOWN` — never zero
and never `CONVERGING`. Exit 4 means the checkout is unusable for that revision; exit 2 and 3 mean
this crate refused. A grader must not score them the same.

This is live under `rch exec`: the remote snapshot carries no resolvable history, so any two-tree
command (`--compare`, `git ls-tree <old>`) is unexecutable there **by construction** and the honest
verdict is UNRUN, not FAIL. Run those in a full-history checkout (CI with `fetch-depth: 0`).

A worktree fallback stands in only for `HEAD`, the one revision a dirty worktree can honestly
approximate. For any other revision the fallback is refused rather than silently substituted —
substituting it is the exact defect this document exists to retire.

`git show HEAD:<path>` reads the TREE; bare `git grep` reads the WORKTREE. Label which one any
number came from.

## NO-CLAIM

COVERED means a bead title or acceptance carries the id as a whole token. It does not mean the crate is wired. MISSING=0 would still not mean S1 is done.

