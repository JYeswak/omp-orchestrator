# exit_code_registry

Bead: `omp-orchestrator-exit-code-registry-rub`

## Purpose

Registers every process exit code this workspace **emits** and every foreign code it **receives**,
one row per code, with the emitting crates, what the code means, **what it does not mean**, and the
operator's next action. It reserves the number space — which bands are ours, which belong to
`sysexits.h`, which to the Rust toolchain, the `cargo` wrapper, `rch`, and POSIX shell — and it
names every code that currently carries two or more meanings. It documents what IS, at the pinned
commit below; it renumbers nothing, because callers already depend on these numbers. The registry
is enforced rather than snapshotted: `crates/no-shell-gate/tests/exit_codes.rs` re-derives the
emission set from source on every run and fails when a code is emitted that has no row here.

## Contract Artifacts

1. **Canonical artifact:** the `XC-*` tables in this document. They are the machine-read artifact —
   the invariant suite parses this file's markdown rows directly, so there is deliberately no
   sidecar `artifacts/exit_codes_v1.json`: a second copy of the table is a second source of truth,
   and the failure mode of a registry is drift, not serialization.
2. **Runner:** `cargo test -p no-shell-gate --test exit_codes -- --nocapture`
3. **Invariant suite:** `crates/no-shell-gate/tests/exit_codes.rs` — seven legs: the real scan, an
   anti-vacuity leg, a fires-on-known-bad leg with a planted undocumented code, a known-good leg, a
   column-completeness leg that refuses any row with an empty **does NOT mean**, a scan-floor leg
   seeded from this document's own measurement, and a pass-through-declaration leg.

> A contract naming no invariant suite is a DESCRIPTION. Item 3 is what stops this file rotting in
> a week, which is what every previous snapshot of these numbers did.

## 1. Why this document exists, measured

Four exit codes were misread by agents on 2026-09-01, this one included:

| code | the misread | what it actually was |
|---|---|---|
| `103` | read as a TEST RESULT, because `0 passed 0 failed` printed beside it | an `rch` refusal — a failure path emitting something SHAPED LIKE DATA |
| `75` | read as a build failure | `CARGO_MINT_CONTAINER_EXHAUSTED`, the disk-floor refusal |
| `127` | read as "`bin/check.sh` is untracked", a repo defect that did not exist | a stale worktree missing a file that HEAD has |
| `1` | read as "the gate is broken" and as "the gate correctly refused" | both, on different lines of the same file |

The common shape is not carelessness. It is that **a refusal and a failure are indistinguishable
from the outside** when the only signal is a small integer, and nothing in the repo said which was
which. Before this document, `git grep -c --no-index -E '^//[/!].*\bexit'` over `crates/*/src/*`
answered **0 files** — no crate documented its own exits — against a positive control of **102
files** carrying `//!` docs at all. The registry's load-bearing column is therefore **does NOT
mean**, not **means**.

## 2. Derivation — every figure below carries its command

Pinned, and deliberately MIXED VINTAGE rather than uniformly restated: the bulk of the counts below
were derived at commit `d48615c` on 2026-09-01. Re-derived at commit `b403e3a` on 2026-09-02: every
figure in this section's command block, `XC-003`'s emitters cell, and §6's `XC-PT-SELFTEST` and
`XC-PT-OUTCOME-CODE` rows. Four of the six §2 figures had DRIFTED in one day — `ExitCode::from`
284 -> 328, `.rs` files 123 -> 145, `ExitCode::(SUCCESS|FAILURE)` 151 -> 160, and `XC-003` 11
crates/24 sites -> 14/26 — while `process::exit` held at 14, which is the positive control proving
the re-reading instrument was the same shape as the original. Re-derive before citing. The invariant
suite re-derives the EMISSION SET on every run and asserts only lower bounds nearby — `FILE_FLOOR`
100, `CODE_FLOOR` 10, `PASSTHROUGH_FLOOR` 5 (`crates/no-shell-gate/tests/exit_codes.rs:37-39`) — so
it catches a collapse to nothing and passes every one of the four drifts above.

```bash
# emission sites, by mechanism
git grep -c --no-index -E 'ExitCode::from'   -- 'crates/*/src/*'   # 328 lines (was 284 at d48615c)
git grep -c --no-index -E 'process::exit'    -- 'crates/*/src/*'   #  14 lines
git grep -c --no-index -E 'ExitCode::(SUCCESS|FAILURE)' -- 'crates/*/src/*'  # 160 lines (was 151)
# distinct literal codes and their site counts
git grep -ho --no-index -E 'ExitCode::from\([0-9]+\)' -- 'crates/*/src/*' \
  | grep -oE '[0-9]+' | sort -n | uniq -c
# named constants, and any name bound to two values
git grep -n --no-index -E '^\s*(pub )?const EXIT_[A-Z_]+' -- 'crates/*/src/*'
# scan-set floor: .rs files under crates/*/src
git ls-files --others --cached --exclude-standard -- 'crates/*/src/*' | grep -c '\.rs$'  # 145 (was 123)
```

**INSTRUMENT NOTE, and it is the reason this section exists.** The first run of the literal-code
extraction used `grep -oE '[0-9]+$'` — anchored at end-of-line — against lines of the form
`ExitCode::from(2);`. It answered **zero distinct codes** while 245 literal sites existed. A
confident zero from a pattern that cannot match is the same defect family as every misread in §1.
The second instrument error was worse: the file set was derived with
`git grep -l -E 'ExitCode|process::exit'`, which returned 53 files and **silently omitted**
`crates/fleet-monitor/src/lib.rs`, the file that declares `EXIT_CANNOT_OBSERVE = 78` and contains
neither token. **A scan set must be at least as wide as the patterns run over it**; the corrected
set is all 123 `.rs` files under `crates/*/src`.

## 3. Codes this workspace EMITS

34 rows, one per code this workspace can emit. **Re-derived 2026-09-11 by this document's own
gate** — `cargo test -j 2 -p no-shell-gate --test exit_codes` on the rch lane, reading the
WORKTREE and not a sha (rule 8) — over 288 `.rs` files under `crates/*/src`: the union of literal
emission sites, `fn exit_code` bodies and `const EXIT_*` declarations is **34 distinct codes**, and
every one now has a row. Fourteen rows (5–11, 20–22, 30–32, 101) were added that day. They had been
missing while the same gate ALSO demanded four fictions — 138, 142, 306 and 307, read out of a
comment inside `crates/ompo-doctor/src/health_repair.rs`'s `fn exit_code` body that cites
`adapter_exec.rs:138-142` and `:306-307`, and 306/307 are not representable in the `u8` that body
returns. The honest total could not be stated until the scanner stopped reading prose as code; the
14 real codes and the 4 fictions had been failing the same leg together. `sites` counts literal
emission sites; `crates` counts distinct emitting crates.

| ID | code | emitters | MEANS | does **NOT** mean | operator's next action |
|---|---|---|---|---|---|
| `XC-000` | 0 | all 51 bin targets | the process completed and made its claim | that work happened. `Discharged::exit_code()` (`crates/omp-orchestrator/src/lib.rs:1359`) returns 0 only for NON-EMPTY evidence, and a no-op tick, a `--dry-run`, and a real dispatch all exit 0 | read the emitted JSON, never the code alone |
| `XC-001` | 1 | 32 crates, 89 sites | **overloaded, three ways**: a gate refused (a working gate), the tool itself broke, or the caller misused the CLI | any one of the three. `crates/no-shell-gate/src/bin/pre-push-gate.rs` emits 1 for `PRE_PUSH_GATE_REFUSED` at `:312,321,366,375,384,402,416,434` AND for `PRE_PUSH_GATE_ERROR` at `:287,303,340,427` | read the marker prefix on stderr; `REFUSED` = the gate worked, `ERROR` = the gate did not run. Filed as `omp-orchestrator-exit-1-overloaded-x1o` |
| `XC-002` | 2 | 34 crates, 118 sites | dominantly a usage error (50 of 118 sites carry a `usage` message), also a runtime error, also "unmeasurable" | a usage error. `OracleCompareVerdict::Unmeasurable` maps to 2 (`crates/oracle-compare/src/lib.rs:103`) while `EXIT_USAGE = 2` in `crates/finding/src/main.rs:44` and `EXIT_CLI = 2` in `crates/fleet-composite/src/main.rs:14`; `PLAN_ASSEMBLE_ERROR` also exits 2. **It also collides with the `cargo` wrapper's own 2** — see `XC-EXT-002`. **Citation corrected 2026-09-11**: this cell read "`EXIT_USAGE = 2` in `crates/fleet-composite/src/main.rs:14`", which was wrong on both halves — that line declares `EXIT_CLI`, and the only `EXIT_USAGE = 2` in the tree is `finding`'s | check stderr for a `usage:` block before assuming operator error |
| `XC-003` | 3 | **14 crates, 26 sites** — was "11 crates, 24 sites", which had DRIFTED. Re-derived 2026-09-02 by three independently-shaped readers that agree: `git grep -c --no-index -F 'ExitCode::from(3)' -- 'crates/*/src/*'` summed to 26, a `git grep -l` roll-up to 14 distinct crates, and a python recursive walk of `crates/*/src/**.rs` to the same 14/26. No const-declared path exists for this code, so the literal scan is complete for it | **two meanings, and they are not interchangeable.** Dominantly a dependency the tool needs is unavailable or unreadable — `ORACLE_UNAVAILABLE`, `TRACKER_ERROR`, `PRODUCT_UNASKABLE`, lint `ERROR`. **AND, in the five-gate pre-commit family, `NOTHING_TO_CHECK` — the scan set was empty**: `crates/no-shell-gate/src/bin/pre-commit-gate.rs:28`, `crates/path-literal-guard/src/main.rs:77`, `crates/state-wildcard-lint/src/main.rs:82`, all three `Verdict::NothingToCheck => ExitCode::from(3)` | that the checked property is bad. 3 is "could not check", adjacent to `XC-077`. **And it does not mean one thing**: "a dependency is missing" is a fault to repair, while "nothing was eligible" is a healthy tree plus an empty scan set — the condition `omp-orchestrator-calr` exists to stop being read as a pass. Not renumbered, per §5. The `exit_codes` suite cannot catch either the stale count or the double meaning: it keys on the code cell alone and proves PRESENCE OF A ROW, never truth of one — its own disclosure at `crates/no-shell-gate/tests/exit_codes.rs:19-21` | read the marker prefix on stderr — `NOTHING_TO_CHECK` means nothing was scanned, anything else means fix the dependency and re-run. Record no verdict either way |
| `XC-004` | 4 | `loop-switch` (3), `no-shell-gate` (1) | a state write or removal failed, including "removal reported success but the switch is still set" | a policy refusal. This is a filesystem-level failure of the tool's own state | inspect the state path named on stderr |
| `XC-005` | 5 | `dispatch-saga` `EXIT_TRANSITION_NOT_APPLIED` (`crates/dispatch-saga/src/main.rs:18`), `finding` `EXIT_PUBLISH` (`crates/finding/src/main.rs:47`), `gate-runner` `EXIT_RUN_UNAVAILABLE` (`crates/gate-runner/src/ci_citation.rs:81`) and `EXIT_LEDGER_DRIFT` (`crates/gate-runner/src/lib.rs:70`) | **four meanings under one integer**: a `br` transition RAN and refused (or could not be spawned); a finding could not be published; the cited CI run could not be fetched; the committed ledger and the derived roster disagree | that 5 discriminates anything. §5 hands 5–63 to NEW distinct meanings and four crates each claimed 5 independently, so the integer cannot tell a refused transition from a ledger disagreement. This is `XC-001`'s overloading one band over, arrived at by four crates that never saw each other | read the marker token on stderr — `TRANSITION_REFUSED`/`TRANSITION_UNSPAWNED`, `FINDING_PUBLISH_FAILED`, `RUN_UNAVAILABLE`, `LEDGER_DRIFT` — never the integer |
| `XC-006` | 6 | `finding` `EXIT_CANCELLED` (`crates/finding/src/main.rs:48`), `gate-runner` `EXIT_LOG_LINE_MISSING` (`crates/gate-runner/src/ci_citation.rs:82`) and `EXIT_METADATA_UNREADABLE` (`crates/gate-runner/src/lib.rs:72`) | cancellation observed at a checkpoint; or the cited run's log did not carry the expected line; or `cargo metadata` itself could not be read | that the work failed. Cancellation is not a verdict on the work, and an unreadable `cargo metadata` is the instrument failing rather than a gate. **It also collides with the `cargo` wrapper's own 6** — see `XC-EXT-006`, whose remedy is a precondition fix, not a retry | read the marker token; for `XC-EXT-006` look for the `CARGO_*` prefix first |
| `XC-007` | 7 | `finding` `EXIT_SPOOL_UNREADABLE` (`crates/finding/src/main.rs:49`), `gate-runner` `EXIT_AGGREGATE_MALFORMED` (`crates/gate-runner/src/ci_citation.rs:83`) and `EXIT_LEDGER_UNREADABLE` (`crates/gate-runner/src/lib.rs:80`) | an input this process must READ was absent, unreadable, or present and carrying zero rows — the spool, the run aggregate, or the committed ledger | that the comparison was made and came out negative. `EXIT_LEDGER_UNREADABLE` is deliberately separate from `EXIT_LEDGER_DRIFT` (5): drift means two readable things disagree, 7 means the comparison could not be made at all, and collapsing them reports a DELETED ledger as a disagreement with it | fix the named path, then re-run; record no verdict for this run |
| `XC-008` | 8 | `gate-runner` `EXIT_RUN_METADATA_MALFORMED` (`crates/gate-runner/src/ci_citation.rs:84`) | the cited run's METADATA parsed but is not shaped like run metadata | that the run failed, and not that it is unreachable (5). The run answered; its envelope is wrong | re-fetch the run, then treat a repeat as a `gh` output-format change |
| `XC-009` | 9 | `gate-runner` `EXIT_SELF_REFERENCE` (`crates/gate-runner/src/ci_citation.rs:86`) | the selector names the run this process is executing inside — unsatisfiable by construction, not by outage | that CI is broken. It is a refusal to cite yourself, which would make a run its own evidence | cite a concrete run id, or use the `local` selector |
| `XC-010` | 10 | `gate-runner` `EXIT_RUN_IN_PROGRESS` (`crates/gate-runner/src/ci_citation.rs:88`), `salvage-taxonomy` `TurnOutcome::DeadlineExceeded` (`crates/salvage-taxonomy/src/lib.rs:179`) | the run exists but has not finished, so its logs do not exist yet; or an agent turn exceeded its deadline | that anything failed. Both meanings are "not yet", and `XC-010`'s two emitters are unrelated crates that both needed a not-yet code. Distinct from `gh` failing, which is 4 | wait and re-cite the run; for `SALVAGE_DEADLINE` the work is usually MOSTLY done — read the landed paths before relaunching |
| `XC-011` | 11 | `gate-runner` `EXIT_LOCAL_AGGREGATE_UNAVAILABLE` (`crates/gate-runner/src/ci_citation.rs:90`), `salvage-taxonomy` `TurnOutcome::ProviderError` (`crates/salvage-taxonomy/src/lib.rs:180`) | the `local` selector was asked for and its inputs were absent; or the upstream provider errored mid-turn | that the aggregate said nothing interesting. An absent aggregate is an ERROR, never a pass — that is the same silent-false-zero this whole registry is written against | supply the aggregate the `local` selector reads, or for `SALVAGE_PROVIDER_ERROR` retry upstream |
| `XC-012` | 12 | `inbox-monitor`; and `salvage-taxonomy` `TurnOutcome::SilentProcessDeath` (`crates/salvage-taxonomy/src/lib.rs:181`) | **MAIL WAITING** — a human owes an answer to a named sender. Reachable only after BOTH surfaces read successfully, so it is a positive observation, not a silence. In `salvage-taxonomy` the same integer means `SALVAGE_SILENT_DEATH`: no exit was observed and the session is stale | that the message is urgent — `importance` is a separate field on the row — and not that anyone has READ it. Folding this into 1 was refused: 1 already carries gate-refused, tool-broke and caller-misused, which would make the one actionable outcome indistinguishable from a crash. **And it does not mean one thing across crates**: the second emitter was found 2026-09-11 when the scanner stopped reading prose, so "mail waiting" and "the agent died silently" share 12 | read the marker token — `SALVAGE_SILENT_DEATH` or the mail row — then answer the named sender or mark read |
| `XC-013` | 13 | `inbox-monitor`; and `salvage-taxonomy` `TurnOutcome::EmptyCompletion` (`crates/salvage-taxonomy/src/lib.rs:182`) | **UNREACHABLE** — the monitor could not observe, so the verdict is ABSENT rather than negative. `subprocess-contract`'s `TimedOut`/`Unspawned` both map here. In `salvage-taxonomy` the same integer is `SALVAGE_EMPTY_COMPLETION`: the provider returned nothing and nothing landed | that there is no mail. A DIFFERENT number from 12 deliberately: "I could not look" is not the same fact as "you have mail". This is the exact false negative measured on 2026-09-02, when `am agent start` claimed no listener while `/health` returned `status: ready`. Second emitter found 2026-09-11, same scan | read the marker token first; for the monitor, fix reachability and re-run, recording no verdict |
| `XC-014` | 14 | `inbox-monitor` | **CURSOR REGRESSED** — the persisted delivery cursor is AHEAD of the durable tail, so our state is wrong rather than the mailbox | that mail is waiting. A third number because the remedy is a state repair, and every further run would silently SKIP events — the failure is invisible without its own code | repair or delete the cursor file named on stderr |
| `XC-015` | 15 | `inbox-monitor` | **CURSOR BELOW FLOOR** — the persisted delivery cursor is strictly below `oldest_available_cursor`, this recipient's oldest retained event, so the position we would resume from cannot be shown to be continuous with what we have already consumed. A fourth number because it is the only fault in the crate that is silent by construction: 14 means our state is AHEAD of the tail and further runs SKIP, this means the position is unprovable and further runs report healthy forever. Measured how-we-get-here: the orchestrator circulated cursor 5105 fleet-wide, which is below SnowyCanyon's floor of 5147 and perfectly resumable for GreenFrog (floor 2108) — the sequence is GLOBAL while the floor is PER RECIPIENT, so cursors are not portable between mailboxes | that mail is waiting, and **not** that the mailbox lost anything — nothing is evicted. A recipient's events are sparse and non-contiguous inside one global monotonic sequence (GreenFrog holds 2108, 2109, 2126), so a position below the floor is indistinguishable from a recipient that simply started receiving later; `oldest_available_cursor` equals a recently-registered mailbox's FIRST event. Continuity is unprovable in both cases, which is why the guard needs no cause. And it does **not** mean the daemon will tell you: it documents `CURSOR_EXPIRED` for such a read and does not emit it — measured 2026-09-02, `am inbox-events --agent GreenFrog --after 1 --limit 3 --json` returned rc=0, served its first event from cursor 2108, and carried NO error/code/status key, so 2107 positions were silently skipped behind a normal success shape. A documented fail-closed refusal that in fact CLAMPS, which is why this check is client-side and runs BEFORE the mail check | repair or re-baseline the cursor file named on stderr (`--position-now` establishes a fresh position with no notification). The run did **not** advance the cursor — `MonitorVerdict::advances_cursor()` abstains for both cursor faults, so the unresumable position survives as the repair target rather than being silently re-based to whatever the server clamped to. Read `persisted`, `oldest_available` and `tail_cursor` off the ledger row; a "below floor" row without all three is undiagnosable |
| `XC-016` | 16 | `inbox-monitor` | **AUTHORITIES DISAGREE** — the two mailbox authorities both answered and gave DIFFERENT unread counts, so no unread count is reported. A fifth number because nothing failed: the primary (authenticated MCP `fetch_inbox`) and the differential oracle (`am inbox`, which opens `storage.sqlite3` directly) contradict each other, and the remedy is to reconcile the surfaces rather than to restart or credential anything. Both numbers and both source labels are carried on the verdict and in the emitted row (`daemon_unread`, `cli_unread`, `primary_authority`, `oracle_authority`). Measured live 2026-09-02 on AmberGate: daemon **96** unread of 128 rows (32 with a non-null `read_ts`) against the CLI's **20**. | that anything is unreachable — both reads SUCCEEDED, which is why this is not 13. And NOT that mail is waiting, nor that it is not: with the arms in conflict there is no unread count, and reporting one arm's number would be picking the convenient side silently. It does NOT say which number is true — that is a separate question with a separate owner. | reconcile the two surfaces: re-run the primary with `mark_read:false` and the oracle with `am inbox --unread --json`, and compare `daemon_unread` against `cli_unread` in the emitted row. Do NOT restart the daemon and do NOT supply a credential — both reads worked. The cursor IS advanced (`advances_cursor()` returns true) because the delivery POSITION was proven by the two cursor guards that ran before this check; only the mailbox is unresolved, so the verdict repeats every run until the surfaces agree. |
| `XC-017` | 17 | `inbox-monitor` (`EXIT_WATCH_TIMED_OUT`) | **WATCH TIMED OUT** — the watch bound elapsed after one or more successful polls without a wake. The mailbox was observable; nothing arrived. Lives in the unallocated 5–63 band on purpose | that the mailbox is unreachable (that is 13) or that it is clear (that is 0). A timeout is not a verdict that there is no mail forever | re-arm the watch; do not treat 17 as empty-inbox |
| `XC-018` | 18 | `inbox-monitor` (`EXIT_CURSOR_ADVANCED`) | **CURSOR ADVANCED** — this recipient's next cursor moved past the persisted baseline and the page contained events, but unread flags were clear; a cursor-keyed wake is required | that the message is urgent or still unread; this is weaker than `MAIL WAITING` and does not claim every event was addressed to this recipient | process the event page and persist `next_cursor`; do not run the mail-only drain path |
| `XC-020` | 20 | `salvage-taxonomy` `TurnOutcome::Unknown` (`crates/salvage-taxonomy/src/lib.rs:185`) | **UNCLASSIFIED** — the taxonomy read the evidence and matched no row, so the turn's outcome is unknown | that the turn finished. The number is deliberately distinct from every classified outcome so that "could not classify" can never be read as "finished" by a caller reading the code alone — the crate's own comment at `:183-184` says so | HOLD: do not relaunch an unclassified turn; read `SALVAGE_UNKNOWN`'s reason text |
| `XC-021` | 21 | `salvage-taxonomy` `TaxonomyError::NonzeroExitWithoutText` (`crates/salvage-taxonomy/src/lib.rs:220`) | **REFUSED, INSTRUMENT SIDE** — a nonzero exit arrived with no accompanying text, so the classifier refused to guess | an outcome. It is the classifier declining, kept in a separate `TaxonomyError` space from `TurnOutcome` precisely so a broken instrument cannot masquerade as a verdict. A deadline, a provider error and a workspace-load outage all produce a bare nonzero | supply the captured text and re-classify; `SALVAGE_EXIT_WITHOUT_TEXT` names the missing input |
| `XC-022` | 22 | `salvage-taxonomy` `TaxonomyError::Cancelled` (`crates/salvage-taxonomy/src/lib.rs:221`) | **CANCELLED AT A CHECKPOINT** — the classification was abandoned cooperatively | a failure, and not `XC-006`'s cancellation either: 6 is `finding`'s cancelled publish, this is the taxonomy's own checkpoint | re-run when not cancelling; record no classification |
| `XC-030` | 30 | `loop-queue-filter` `GateError::ArcCensusZero` (`crates/loop-queue-filter/src/phase_gate.rs:143`) | **ARC CENSUS ZERO** — the arc has no members, so every withhold decision the gate could make would be vacuous | that nothing should be dispatched. An empty census is an ERROR, never a permissive pass; the crate's own message names the positive control it expected | fix the arc census input named by `PHASE_GATE_ERROR arc_census_zero`, then re-run |
| `XC-031` | 31 | `loop-queue-filter` `GateError::EmptyCandidateSet` (`crates/loop-queue-filter/src/phase_gate.rs:144`) | **EMPTY CANDIDATE SET** — there were no candidates at all to filter | that the queue was read and found clear. "Nothing to dispatch" and "I could not read the queue" are opposite conditions with opposite remedies, which is why this is not folded into 30 or into 0 | read `PHASE_GATE_ERROR empty_candidate_set`; establish whether the queue is genuinely empty before treating it as idle |
| `XC-032` | 32 | `loop-queue-filter` `GateError::ExceptionSetIncomplete` (`crates/loop-queue-filter/src/phase_gate.rs:145`) | **EXCEPTION SET INCOMPLETE** — the declared exception set is empty or malformed, so the gate cannot be trusted to admit the paths that make the phase completable | that the gate refused a bead. The gate itself is unusable here; a run that kept going would withhold work it has no basis to withhold | repair the declared exception set, then re-run the gate |
| `XC-064` | 64 | 6 crates, 11 sites including `fleet-composite` `EXIT_EX_USAGE` (`crates/fleet-composite/src/main.rs:24`) | `EX_USAGE` — command line / invocation could not be resolved | a config error (`EX_CONFIG` is 78, `pane-dispatch-fence` `EXIT_CONFIG`). cas: fleet-composite no longer names 64 `EXIT_CONFIG`. **Renamed again 2026-09-11, `EXIT_USAGE` -> `EXIT_EX_USAGE`**: the cas rename moved the collision rather than ending it, because `crates/finding/src/main.rs:44` already declared `const EXIT_USAGE: u8 = 2`, and one name bound to 2 and 64 makes a reader who learned it in one crate confidently wrong in the next. Neither cause was deleted; the `EX_` prefix now says which vocabulary this 64 claims | correct the invocation |
| `XC-069` | 69 | `fleet-monitor` `EXIT_CANNOT_OBSERVE` | `EX_UNAVAILABLE` — the fleet could not be observed (`ntm list` empty/unreadable) | a configuration error (that is 78 `EXIT_CONFIG`) or a busy pane (that is 75) | fix observability inputs; do not treat as EX_CONFIG |
| `XC-070` | 70 | `omp-orchestrator` (`exit_code`) | `EX_SOFTWARE` — a discharge carrying EMPTY evidence, i.e. a no-op wearing a success | a crash. It is a deliberate refusal to let empty evidence pass as 0 | supply evidence, or accept that the decision was not discharged |
| `XC-075` | 75 | `installer` (1 site), `loop-driver` `EXIT_CONCURRENT:17`, `pane-dispatch-fence` `EXIT_BUSY:16` | `EX_TEMPFAIL` — retry later: another instance holds the lock, the pane is busy, or the installer is blocked | a defect in the work being attempted. **Three meanings in our tree plus two more from the `cargo` wrapper** — see `XC-EXT-075`. Documented, not renumbered, by cas | wait and retry; do not treat as a verdict |
| `XC-076` | 76 | `pane-dispatch-fence` `EXIT_NOT_FREE:17` | `EX_PROTOCOL` — the pane is not free capacity | the pane is dead or wedged; it may be legitimately working | select another pane |
| `XC-077` | 77 | `dispatcher-deadman` (4), `tick-dispatch` (4), `fast-dispatch` (1) | **UNPROVEN** — the checker could not establish its verdict: unreadable cwd, unset `HOME`, unwritable state, a required child unavailable | that the watched condition is bad. This is the fail-closed unknown, and it is the most useful code in the registry. Note it deviates from `sysexits.h`, where 77 is `EX_NOPERM`. Filed as `omp-orchestrator-exit-reserved-range-b7f` | fix the checker's environment; the verdict is absent, not negative |
| `XC-078` | 78 | `pane-dispatch-fence` `EXIT_CONFIG` | `EX_CONFIG` — the fence's own configuration is wrong (missing flags, bad paths) | that the fleet could not be observed (that is 69) | fix the fence invocation |
| `XC-101` | 101 | `no-shell-gate` `head_compiles::GateError::exit_code` (`crates/no-shell-gate/src/head_compiles.rs:44-48`), for `DependencyResolutionFailed`, `CompileFailed` and `BuildTimedOut` | HEAD did not compile from a clean archive export, dependency resolution failed, or the bounded build elapsed | that a Rust binary panicked. **This is a standing violation of §5, which reserves 101 to the toolchain and says NEVER EMIT** — and the collision is total: `XC-EXT-101` is a real panic and `XC-101` is a gate verdict, the two remedies are opposite, and nothing in the integer separates them. `BuildTimedOut` is the sharpest case because `cargo` never emits 101 for a timeout, so a 101 here can mean a thing the borrowed vocabulary cannot express. Pinned by its own unit test (`crates/no-shell-gate/src/head_compiles.rs:405`), so it is a live contract and not an accident; documented, not renumbered. Same class as `omp-orchestrator-exit-reserved-range-b7f`, which names 124 and 77 and does not yet name this one | read the `HEAD_COMPILE_*` marker on stderr — `HEAD_COMPILE_FAILED`, `HEAD_COMPILE_DEPENDENCY_ERROR`, or the timeout line. A 101 with no such marker is a panic, not a verdict |
| `XC-124` | 124 | `loop-driver` `EXIT_DEADLINE:18` | our own deadline elapsed | that `timeout(1)` killed us — **and that is not distinguishable from outside**. Measured live while writing this document: a probe wrapped in `timeout 90` returned 124 from `timeout`, not from any of our binaries. Filed as `omp-orchestrator-exit-reserved-range-b7f` | check whether a `timeout` wrapper was in the command line before believing the binary self-limited |

## 4. Codes this workspace RECEIVES — foreign, never ours to emit

These arrive from the toolchain, the `cargo` wrapper, `rch`, or the shell. Every row was verified
against the emitting binary, not inferred from convention.

| ID | code | source | MEANS | does **NOT** mean | operator's next action |
|---|---|---|---|---|---|
| `XC-EXT-002` | 2 | `~/.local/bin/cargo` (the mint wrapper — a THIRD `cargo` on `PATH`, ahead of `~/.cargo/bin`) | a lane-identity or target-ownership refusal: `CARGO_LANE_IDENTITY_UNSAFE`, `CARGO_LANE_ISOLATED_TARGET_REQUIRED`, `CARGO_TARGET_ROOT_UNREGISTERED`, and 6 more | a compile error, and **not** our `XC-002` either | read the `CARGO_*` marker on stderr |
| `XC-EXT-006` | 6 | the same wrapper | a reclaim or contract precondition is unavailable: `CARGO_UNAVAILABLE`, `CARGO_RECLAIM_UNAVAILABLE`, `CARGO_LANE_CONTRACT_UNWRITABLE`, `CARGO_LANE_AUDIT_UNWRITABLE` | a build outcome. Nothing was compiled | fix the named precondition |
| `XC-EXT-075` | 75 | the same wrapper | the disk floor refused the build: `CARGO_MINT_CONTAINER_EXHAUSTED` or `CARGO_MINT_PEAK_WOULD_BREACH_FLOOR` | a build failure. **The wrapper is the only emitter on this machine that already ships the disambiguation in data**: its JSON carries `"exit_code":75,"is_gate_verdict":false` (`~/.local/bin/cargo:388`). That field is the precedent this whole registry generalises | reclaim disk, then re-run; `CARGO_MINT_MIN_CONTAINER_PCT=0` is a deliberate exception, not a fix |
| `XC-EXT-101` | 101 | `rustc` / `cargo` | a Rust panic — measured: a `panic!()` binary exits 101 | a test failure or a refusal | read the panic message and backtrace |
| `XC-EXT-124` | 124 | `timeout(1)` | the wrapper killed the child at its deadline | that the child decided anything. **Collides with our `XC-124`** | re-run with a longer deadline before drawing any conclusion |
| `XC-EXT-126` | 126 | POSIX shell | the file exists but is not executable | that it is missing | `chmod +x`, or check the interpreter line |
| `XC-EXT-127` | 127 | POSIX shell | command not found | that the repo is missing the file. On 2026-09-01 a 127 for `bin/check.sh` was read as "untracked" when the real cause was a **stale worktree** whose tree predated the commit that added it | verify `git ls-files` in the SAME tree that produced the 127 |
| `XC-EXT-128N` | 128+N | POSIX shell | the process died on signal N — 130 `SIGINT`, 137 `SIGKILL`/OOM, 139 `SIGSEGV` | a chosen exit. Nothing in this workspace emits above 124 | for 137, check memory and the OOM killer before the code |
| `XC-EXT-RCH` | see note | `/Users/josh/.local/bin/rch` | a remote-compilation refusal: `[RCH] remote required; refusing local fallback [RCH-E301]` | a test result — this is §1's most expensive misread, because the refusal prints alongside cargo-shaped output | read the `[RCH]` prefix and the `RCH-Exxx` code, never the integer |

**On `103` specifically, and this is a correction to the session's own account.** The number was
recorded as 103 during the incident. Two probes tonight could not reproduce it:
`rch exec -- cargo --version` under `RCH_REQUIRE_REMOTE=1` exits **1**, printing
`[RCH] remote required; refusing local fallback [RCH-E301]`; the compilation-verb probe was killed
by its own `timeout` wrapper at 124 before rch answered. `rch schema export` ships a
machine-readable catalog of **100 `RCH-Exxx` codes with ZERO numeric exit mappings**
(`grep -c exit /tmp/…/error-codes.json` finds no exit key). So: **the string is authoritative and
verified; the integer 103 is OBSERVED, not documented upstream and not reproduced here.** Recorded
that way deliberately — a registry that launders an unverified number into a fact is the artifact
this document exists to replace.

## 5. Range ownership — the reservation table

| band | owner | our use | rule |
|---|---|---|---|
| `0` | POSIX | success | `XC-000` |
| `1`–`4` | **ours**, legacy | verdict + error + usage, overloaded | do not add meanings; new semantics go to an unallocated band |
| `5`–`63` | **UNALLOCATED, ours to claim** | none | the correct home for any new distinct meaning |
| `64`–`78` | `sysexits.h` (`EX_*`) | 64, 69, 70, 75, 76, 77, 78 | keep `EX_*` semantics; `XC-077` already deviates (`EX_NOPERM`) |
| `79`–`100` | unallocated | none | free |
| `101` | Rust toolchain | **never emit** — and we do | panic. `XC-101` is a standing violation: `no-shell-gate`'s `head_compiles` gate emits 101 for a failed or timed-out HEAD build |
| `102`–`125` | toolchain and wrappers | `124` only, and it collides | **never emit in this band**; `XC-124` is a standing violation |
| `126`, `127` | POSIX shell | never | not-executable / not-found |
| `128`–`255` | POSIX shell signals | never | `128+N` |

**Collisions found, all measured, none invented:**

1. `EXIT_CONFIG` name collision — **fixed in cas**: 64 is `fleet-composite` `EXIT_EX_USAGE` (`EX_USAGE`); 78 is `pane-dispatch-fence` `EXIT_CONFIG` (`EX_CONFIG`).
2. `78` two names — **fixed in cas**: `EXIT_CANNOT_OBSERVE` moved to 69 (`EX_UNAVAILABLE`); 78 is only `EXIT_CONFIG`.

3. `75` carries **five meanings** — `EXIT_CONCURRENT`, `EXIT_BUSY`, the installer's block, and the
   wrapper's two mint refusals.
4. `2` is emitted by us (118 sites, three meanings) **and** by the `cargo` wrapper (9 markers).
5. `124` is ours and `timeout(1)`'s.
6. `77` means UNPROVEN here and `EX_NOPERM` in `sysexits.h`.
7. `EXIT_USAGE` was bound to **two values** — 2 in `crates/finding/src/main.rs:44` and 64 in
   `crates/fleet-composite/src/main.rs`. Found 2026-09-11 by
   `exit_codes::no_exit_constant_name_is_bound_to_two_values`, which is the same leg collision 1
   above was opened by: the cas rename from `EXIT_CONFIG` to `EXIT_USAGE` moved the collision
   instead of ending it. **Fixed by renaming, never by deleting a name** — two distinct causes
   must stay distinguishable to a caller. 64 is now `EXIT_EX_USAGE`.
8. `5`, `6`, `7`, `10`, `11`, `12` and `13` each carry **two to four unrelated meanings**, all in
   the 5–63 band §5 hands to new distinct semantics — `gate-runner` (11 codes), `finding` (5),
   `salvage-taxonomy` (7), `loop-queue-filter` (3) and `dispatch-saga` (2) each numbered from 5
   upward independently and collided. Documented 2026-09-11, not renumbered: every one is a live
   contract with tests on it, and the remedy the rows carry is to read the marker token. **This is
   the band the registry recommends, so the recommendation is incomplete without a claim
   mechanism** — §5 reserves a range and nothing allocates inside it.
9. `101` is ours and `rustc`'s — `XC-101` against `XC-EXT-101`. Same class as 124, and worse in
   one respect: 124's collision is with a wrapper that is visibly in the command line, while a
   101 from `head-compiles-gate` is shaped exactly like the panic of the binary that produced it.

## 6. Pass-through — where a foreign code becomes ours

**61 sites across 20 expressions** forward a code they did not choose as their own, which is how
an integer chosen elsewhere arrives wearing one of our binaries' names. Every site must be listed
here or the invariant suite fails, and every row here must still describe a site — the suite
checks both directions.

**Re-derived 2026-09-11** (`--test exit_codes`, rch lane, WORKTREE not a sha). The header read
"25 sites across 9 expressions" over a table of 10 rows, and the true figure was 61/20: ten
expressions were undeclared. They were not new — `error.exit_code()` alone has 13 sites across 8
crates — they had simply never been demanded, because the scanner that counts them was masking
comments and KEEPING string literals, so the leg that reads this table was failing on prose at the
same time. Fixing the mask (`text_structure::code_and_literals`) shrank the derived set by one
phantom site and one phantom expression and left these 61.

The suite keys on the **expression** cell and reads no other column, so adding a crate name to an
existing row declares nothing: it will look accepted and change no verdict (measured 2026-09-02).

| ID | expression | crates |
|---|---|---|
| `XC-PT-CODE` | `code` | `finding`, `gate-runner`, `no-shell-gate`, `omp-inventory-map`, `ompo-doctor` (8 sites; the cell read `loop-tick`, `omp-inventory-map` and was re-derived 2026-09-11) |
| `XC-PT-EXIT` | `exit` | `omp-idle-dispatch`, `tick-dispatch` |
| `XC-PT-OUT` | `out.code` | `verify-dispatch` |
| `XC-PT-OUTPUT` | `output.code` | `loop-driver`, `loop-queue-filter` |
| `XC-PT-RC` | `rc` | `fleet-monitor` |
| `XC-PT-VERDICT` | `verdict.exit` | `dispatcher-deadman` |
| `XC-PT-EXITCODE` | `v.exit_code()` | `pane-oracle-diff` |
| `XC-PT-OUTCOME-EXIT` | `outcome.exit_code()` | `crate-atom-gate`, `ompo-doctor` (5 sites) |
| `XC-PT-TERMINAL` | `terminal.exit_code()` | `inbox-monitor` |
| `XC-PT-OUTCOME-CODE` | `outcome.code` | `refill-idle-panes` (`src/main.rs:186,209,226,276`) |
| `XC-PT-ERROR` | `error.exit_code()` | `bead-availability`, `gate-runner`, `grader-attribution-gate`, `loop-queue-filter`, `no-shell-gate`, `omp-inventory-map`, `ompo-start`, `worker-tag-gate` — 13 sites, the largest single expression in the tree. Forwards a TYPED error's own code from inside this workspace, so its range is the `XC-*` table, not 0–255 |
| `XC-PT-REPORT` | `report.exit_code()` | `bead-availability`, `installer` (2 sites). Typed report inside this workspace; range is the `XC-*` table |
| `XC-PT-SUMMARY` | `summary.exit_code` | `ompo-doctor` (`src/main.rs:1139,1158`). A derived health summary's own severity code, not a child's |
| `XC-PT-PROBE` | `probe.exit_code` | `ompo-doctor` (`src/main.rs:942`). A single probe's code, distinct from the summary above — the two are different granularities and were both undeclared |
| `XC-PT-OUTCOME-EXITCODE` | `report.outcome.exit_code()` | `contabo-reclaim` (`src/main.rs:199`). A LOCAL typed `outcome.exit_code()`, space `{0,1,2}` (`Planned|Reclaimed|AlreadyClean|SkippedLiveBuild`→0, `Refused`→1, `Unknown|Unreachable`→2) — recorded because the recogniser cannot tell a local from a forwarded child code, which is the disclosure at `crates/no-shell-gate/tests/exit_codes.rs:65-75`, not because a foreign/remote code arrives here. RENAMED FROM `XC-PT-RESPONSE` (`response.exit_code`, retired 2026-09-12): a `contabo-reclaim` refactor renamed the expression, which orphaned the old declaration AND created this undeclared site — ONE refactor wearing TWO failing rows. Distinct id from `XC-PT-OUTCOME-EXIT` (`outcome.exit_code()`) and from `XC-PT-REPORT` (`report.exit_code()`, which is live at `:211` of this same file): the gate keys on the EXPRESSION, and this three-segment chain is neither of those spellings |
| `XC-PT-VERDICT-EXITCODE` | `verdict.exit_code()` | `doctrine-retirement-gate` (`src/main.rs:47`). Separate id from `XC-PT-VERDICT` (`verdict.exit`) because the gate keys on the expression and the two spellings are different sites |
| `XC-PT-VACUITY` | `vacuity.exit_code()` | `bead-availability` (2 sites). A typed anti-vacuity verdict; range is the `XC-*` table |
| `XC-PT-CLASSIFICATION` | `classification.gate_exit()` | `bead-availability` (1 site). The only `gate_exit()` spelling in the tree |
| `XC-PT-LEDGER-EXIT` | `ledger_exit` | `grader-attribution-gate` (`src/main.rs:82-84`). A LOCAL variable holding this crate's own `ledger_gate_exit`, whose space is `{0,1}` — recorded because the recogniser cannot tell a local from a forwarded child code, which is the disclosure at `crates/no-shell-gate/tests/exit_codes.rs:65-75`, not because a foreign code arrives here |
| `XC-PT-EXIT-CODE-LOCAL` | `exit_code` | `omp-orchestrator` (`src/resident.rs:7207,7230`). Bare local, same disclosure as the row above |

A pass-through means **the code space at that site is not ours** — it is 0–255 of whatever ran
underneath, including `XC-EXT-101` and every `XC-EXT-*` row above. Three rows are exceptions: they
forward a code from a typed value inside this workspace, so their range is the `XC-*` table.

Three of the rows above are exceptions, and each names what closes its space and what PINS that
claim. **This is deliberately a list and not a table: `registry_rows` treats any markdown row
whose first cell begins `XC-` as a declaration, so a second table reusing those ids is parsed as
a second set of declarations. Writing that table broke the gate — the third time in this session
that documenting a gate inside what the gate reads produced a false verdict.**

- **`XC-PT-VERDICT`** and **`XC-PT-EXITCODE`** — forward a code from a typed verdict inside this
  workspace, so their range is the `XC-*` table. Not separately pinned.
- **`XC-PT-OUTCOME-CODE`** — space is `{0,1,2}`. Every `RefillOutcome.code` is constructed by
  `refill_idle_panes::run_outcome`; emitters are
  `crates/refill-idle-panes/src/main.rs:186,209,226,276`. PINNED BY
  `run_outcome_only_ever_yields_a_documented_exit_code`, which enumerates all 32 `Decision`
  shapes × 3 kernel verdicts, asserts every code has an `XC-*` row, and asserts all three codes
  are REACHABLE — so a `run_outcome` that had lost the ability to refuse would fail it.

**These are DECLARATIONS, not fixes.** The scanner cannot tell a forwarded child status from a
local value reached through a field access or a zero-argument call — `passthrough_chain` accepts
any bare identifier chain, so `out.code` holding a child's status and `outcome.code` holding our
own `u8` are indistinguishable to it. The list above says which is which; the pin is what stops
that being merely asserted. A row with no pin is a claim on trust.

**A ROW CAN ALSO BE WRONG BY BEING UNMATCHED.** `no_declared_pass_through_row_outlives_its_site`
refuses a declared expression that no scanned site occupies, because two different things look
identical here: the site is gone (retire the row), or the site exists and the recogniser stopped
seeing it. `passthrough_chain` rejects any argument still containing `(` or `&` after the
trailing-`()` strip, so `f(g())` and `f(&x)` drop real sites out of the scan silently —
`crates/pane-truth/src/main.rs:49,51` are live instances, undeclared AND unrecognised.

## Validation

```bash
cargo test -p no-shell-gate --test exit_codes -- --nocapture
```

Expect **10 passed**. The suite re-derives the emission set from all `.rs` files under
`crates/*/src` and fails naming `file:line` for any code with no row here, refuses any row whose
**does NOT mean** cell is empty, refuses a pass-through row that matches no site, errors on an
empty scan set rather than passing, ignores codes that appear only in comments or string literals,
and proves it fires by planting an undocumented code in a temporary fixture.

## Cross-References

- `crates/no-shell-gate/tests/exit_codes.rs` — the invariant suite
- `crates/no-shell-gate/src/bin/pre-push-gate.rs` — `XC-001`'s triple meaning, at the lines cited
- `crates/oracle-compare/src/lib.rs:99-104` — the *good* pattern: Agree/Disagree/Unmeasurable as 0/1/2
- `crates/omp-orchestrator/src/lib.rs:1359` — `XC-070`, empty evidence is not success
- `crates/fleet-composite/src/main.rs:14-16` — `EXIT_USAGE`/`EXIT_CONFIG`, collision 1
- `crates/pane-dispatch-fence/src/main.rs:16-18` — `EXIT_BUSY`/`EXIT_NOT_FREE`/`EXIT_CONFIG`
- `crates/loop-driver/src/lib.rs:17-18` — `EXIT_CONCURRENT`/`EXIT_DEADLINE`
- `crates/fleet-monitor/src/lib.rs:413` — `EXIT_CANNOT_OBSERVE`, the file the first scan missed
- `crates/dispatcher-deadman/src/main.rs:128-159` — `XC-077`, `emit_unproven` before every 77
- `~/.local/bin/cargo:388` — the `is_gate_verdict` precedent
- `docs/contracts/asupersync_process_grade.md` — the grade this document is scored by
- `docs/plans/plan_to_write_the_document_corpus.md` — this is document #13 of 78
- `NUMBERS.toml` — the figure-carries-its-command discipline §2 follows

## Non-Coverage

- **No renumbering.** Every collision in §5 is documented, not fixed. Renumbering breaks callers
  and belongs to the three filed beads.
- **No code is changed.** Not one `ExitCode::from` was touched.
- **stderr markers are not registered.** `PRE_PUSH_GATE_REFUSED`, `CARGO_MINT_*`, `RCH-Exxx` and
  the rest are cited where they disambiguate a code, but a marker registry is its own document.
- **Non-`crates/*/src` emitters are out of scope.** Test binaries, fixtures, and the pre-commit
  hook's own shell-free wrapper are not scanned.
- **The 51 `ExitCode::SUCCESS`/`FAILURE` sites are not individually mapped.** `FAILURE` is 1 and
  folds into `XC-001`; enumerating each site would not change a row.
- **No claim that any band is empty.** §5 says which bands we *use*; a foreign binary may emit
  anything.

## NO-CLAIM

**Documenting a code does not make any caller emit it correctly.** Every row above describes what
the source does today; nothing here constrains a future emission site, and the invariant suite
catches only *undocumented* codes — it cannot tell a correct 1 from a wrong 1, because both are 1.
That is the defect `XC-001` names, and the registry does not fix it.

The suite proves **presence of a row**, not **truth of a row**: a wrong **does NOT mean** cell
passes every leg. It also cannot see emissions it does not scan — a code produced by a shell
wrapper, a build script, or a crate outside `crates/*/src` is invisible to it, and the pass-through
sites in §6 are unbounded by construction.

`XC-EXT-RCH`'s integer is unverified, stated as such. And the four misreads in §1 were made by
agents who had this repo open; a document does not remove that failure mode, it only makes the
disambiguation cheaper than re-deriving it under pressure.
