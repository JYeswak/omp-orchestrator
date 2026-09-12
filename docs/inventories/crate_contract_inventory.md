# Crate Contract Inventory

Bead: `omp-orchestrator-crate-reachability-census-44g` (R4; Phase 1 inventory)

## Purpose

This inventory records one contract row for every package emitted by `cargo metadata --format-version 1 --no-deps --offline`: the package's observed argv/stdin/env/file inputs, stdout/files/status outputs, typed public interface, and source-visible routing through the shared kernels or a named handroll. `UNDECLARED` means the source scan did not establish that cell; it is not an inferred contract. Exit-code semantics are delegated to the canonical registry.

## Contract Artifacts

1. Canonical artifact: the metadata-derived rows between `CRATE-CONTRACT-ROWS-BEGIN` and `CRATE-CONTRACT-ROWS-END` in this file.
2. Runner: the single pasteable command in `## Validation` below.
3. INVARIANT SUITE: `crates/no-shell-gate/tests/crate_contract_inventory.rs` — metadata coverage, empty-set anti-vacuity, duplicate/extra-row rejection, and planted-package mutation.

## Inventory Rules

- **CRI-ROSTER-METADATA** — package membership comes from Cargo metadata, never a hand-written roster.
- **CRI-ROW-EXACTLY-ONCE** — each metadata package has exactly one row; duplicate and extra rows are errors.
- **CRI-IO-SOURCE** — input/output cells report only markers found in the package source scan; `UNDECLARED` is the honest result when source evidence is absent.
- **CRI-TYPED-SURFACE** — public declarations are listed when the source scan exposes them; this inventory does not invent semantics from a name.
- **CRI-KERNEL-ROUTE** — path dependencies on `subprocess-contract` or `oracle-compare` are recorded; raw `Command::new` is marked as a handroll and no route is inferred from silence.
- **CRI-EXIT-REGISTRY** — exit-code meaning comes from `docs/error_codes/exit_code_registry.md`; this inventory does not maintain a second exit-code table.

## Package Rows

The ROW SET is derived from the workspace package set and is the artifact checked by the invariant suite; row CONTENT is curated once and then preserved. Both halves are load-bearing, and the sentence that stood here claimed the first without anything implementing it: measured 2026-09-12, nothing in the repository generated this block, `CRATE-CONTRACT-ROWS` appeared only in this file and in the invariant suite, and the table had drifted to 72 rows against 94 packages. The derivation now lives in `omp_inventory_map::crate_contract` and the coverage leg PRINTS the repaired block when a package has no row, so a missing row is a paste rather than a research task. Existing rows are never rewritten by it -- several carry prose no scan can produce, and regenerating over them would replace a human's measurement with a scanner's.

| crate | inputs (argv/stdin/env/files read) | outputs (stdout/files/status; exit codes) | typed interface exposed/consumed | kernel routing or handroll |
|---|---|---|---|---|
<!-- CRATE-CONTRACT-ROWS-BEGIN -->
| `ack-spine` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | AckVerdict, detect_ack, simulate_singular_trap, classify_ack_readback, SingularTrapResult | routes subprocess-contract; process request supplied to kernel; kernel-routed br |
| `ack-stage` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | MAX_RETRY_ATTEMPTS, TransportKind, NtmRobotSendReceipt, TmuxSendKeysMeasurement, TransportReceipt | UNDECLARED route |
| `blocker-taxonomy` | argv, stdin, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | BeadRecord, ReportedBead, Report, LiveSet, BlockerKind | UNDECLARED route |
| `build-stamp` | env | stdout; exit codes: `docs/error_codes/exit_code_registry.md` | emit, resolve_from | handroll Command::new |
| `commit-build-fence` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, DEFAULT_TTL_SECS, BuildRegistration, ReleaseEvent, RegistrationStore | UNDECLARED route |
| `composer-typed` | argv, stdin, env, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ComposerVerdict, classify, main | UNDECLARED route |
| `contabo-reclaim` | argv, stdin | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | OwnerRequest, OwnerProcessOutput, OwnerForwardedResponse, ConsumerError, CliError | handroll Command::new |
| `convergence-stamp` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, Stamp, main | handroll process spawn |
| `dispatch-claim-fence` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, BeadStatus, BeadSnapshot, DispatchIntent | handroll process spawn; handroll br |
| `dispatch-saga` | argv, stdin, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DispatchKey, UnmintableKey, Saga, Receipt, Observation | routes subprocess-contract |
| `dispatch-silence-watch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SilenceVerdict, SilenceAction, classify, main | routes subprocess-contract; process request supplied to kernel; kernel-routed br |
| `dispatcher-deadman` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DispatcherDeadmanRules, DispatcherDeadmanVerdict, main | handroll process spawn; handroll br |
| `doctrine-retirement-gate` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Finding, Document, RetirementReason, SpanError, Verdict | UNDECLARED route |
| `fast-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DEFAULT_FRESH_SECONDS, AdmissionConfig, FastDispatchRules, SelectError, admission_fresh_pass | handroll process spawn; handroll br; handroll bv |
| `finding` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Finding, SpooledFinding, Filed, Waived, Publisher | routes subprocess-contract; process request supplied to kernel |
| `finding-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | finding_for, main | handroll process spawn |
| `fleet-composite` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FactorSpec, InputError, CompositeReport, SelftestCheck, SelftestReport | handroll process spawn |
| `fleet-idle-monitor` | argv, stdin, env, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TrackerBinding, QueueEntry, Dispatch, VerifiedNudge, BindRefusal | UNDECLARED route |
| `fleet-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetMonitorError, FleetMonitorConfig, AttentionEvent, monitor_once, run | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `fleet-reconcile` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReconcileVerdict, ReconcileReport, main | handroll process spawn; handroll tmux |
| `fleet-truth` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetTruthError, FleetTruthConfig, FleetSnapshot, collect, main | handroll process spawn; handroll tmux |
| `fuzz-build-gate` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FuzzTarget, RegressionInput, ContractReport, GateError, REQUIRED_WORKER | routes subprocess-contract |
| `gate-runner` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RosterEntry, GateReport, Observed, CheckPhase, CheckInvocation | routes subprocess-contract |
| `grader-attribution-gate` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ActorProvenanceRow, ActorProvenanceViolation, CloseAttempt, AttributionPass, EmptyScan | UNDECLARED route |
| `input-manifest` | argv, stdin, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Manifested, CargoTargetCount, CargoTestResult, ManifestSource, CensusRow | UNDECLARED route |
| `installer` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | InstallError, InstallTarget, RepoOwnership, IdentityCheck, main | routes subprocess-contract; process request supplied to kernel |
| `kernel-bypass-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Bypass, GateReport, main | handroll process spawn |
| `kernel-only-gate` | files | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | HandrollHit, HandrollScanReport, Verdict, SCOPE_LINE, verdict | handroll Command::new |
| `kernel-only-operator-hook` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | HookInput, Permission, Decision, ParseError, main | routes subprocess-contract; process request supplied to kernel |
| `loop-coverage` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | CoverageRow, CoverageReport, main | handroll process spawn |
| `loop-driver` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DriverConfig, DriverError, DriverState, run | handroll process spawn |
| `loop-queue-filter` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Runtime, RunOutput, main | handroll process spawn; handroll br |
| `loop-switch` | UNDECLARED | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | LoopSwitchError, main | handroll process spawn |
| `loop-tick` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TickConfig, TickError, TickOutcome, wait_deadline | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `m2-grading-lane` | argv, env | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TickRecord, LANE, ORACLE_SCHEMA, DEFAULT_ORACLE_RELATIVE, DECISION_NO_FEED | UNDECLARED route |
| `named-test-filter-gate` | argv, stdin, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Tally, NamedTestRef, GradeError, Grade, is_admit | UNDECLARED route |
| `no-shell-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, Verdict, GateError, WorkspaceLoad, main | routes subprocess-contract; process request supplied to kernel; handroll br; handroll spawn |
| `ntm-fleet-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetMonitorError, FleetMonitorConfig, FleetAction, main | handroll process spawn; handroll tmux |
| `ntm-kernel` | UNDECLARED | stdout; exit codes: `docs/error_codes/exit_code_registry.md` | NtmCall, NtmRun, PaneSnapshot, NtmVerb, NtmOutcome | routes subprocess-contract |
| `omp-host-tool-guard` | UNDECLARED | stdout; exit codes: `docs/error_codes/exit_code_registry.md` | HostToolDecl, HostToolCall, DenyRule, GuardedTool, GuardPolicy | routes subprocess-contract |
| `omp-inventory-map` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | InventoryMap, InventoryRow, InventoryNode, InventoryEdge, ProbeEvidence | handroll process spawn; handroll br |
| `omp-orchestrator` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | LoopConfig, DispatchError, SupervisorError, main | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `omp-rpc-session` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RpcSessionError, RpcSessionConfig, RpcSessionEvent, run_session | handroll process spawn |
| `omp-types` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED — source reserved | routes subprocess-contract |
| `ompo-doctor` | argv, stdin, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | OmpState, OmpMessages, MessageSummary, Signal, HealthReport | routes subprocess-contract |
| `ompo-start` | argv, stdin, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PaneId, SpawnReceipt, MailRegistration, MailRoster, PackReceipt | routes subprocess-contract |
| `oracle-compare` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | OracleError, OracleVerdict, Comparison, compare | handroll process spawn |
| `oracle-pane-state-differential` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PaneState, ProjectionState, DifferentialReport, main | routes oracle-compare; handroll process spawn; handroll tmux |
| `pane-dispatch-fence` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FenceError, FenceConfig, FenceGuard, main | routes subprocess-contract; process request supplied to kernel; handroll spawn |
| `pane-dispatch-ready` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PaneDispatchReadyState, PaneDispatchReadyRules, PaneDispatchReadyVerdict, classify, apply_composer_rc | handroll process spawn; handroll tmux |
| `pane-oracle-diff` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PaneOracleDiffError, PaneOracleDiffConfig, PaneOracleDiffReport, main | routes oracle-compare; handroll process spawn; handroll tmux |
| `pane-truth` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PaneState, PaneTruthError, PaneTruthConfig, capture, main | handroll process spawn; handroll tmux |
| `path-literal-guard` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PathLiteralViolation, scan_source, scan_paths, main | UNDECLARED route |
| `plan-assemble` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | main | handroll process spawn |
| `porting-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | PortingGateError, PortingGateReport, main | routes subprocess-contract; process request supplied to kernel |
| `pre-delete-citation-check` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Citation, CitationError, check_citations, main | handroll process spawn; handroll br |
| `r1-breadth-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SubjectScore, Report, CheckError, Attribution, LEVEL_NAMES | UNDECLARED route |
| `reap-finished-panes` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReapError, ReapConfig, ReapReport, main | handroll process spawn; handroll tmux |
| `receiver-receipt` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReceiptVerdict, ReceiptEvidence, classify, main | handroll process spawn; handroll tmux |
| `refill-idle-panes` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RefillError, RefillConfig, RefillReport, main | handroll process spawn; handroll tmux |
| `s2-gate` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | S2Refuse, FLOOR, REQUIRED_KEYS, admit, readback_ok | UNDECLARED route |
| `salvage-taxonomy` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | TurnEvidence, Classification, TurnOutcome, SalvageDecision, TaxonomyError | UNDECLARED route |
| `scratch-home` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ScratchHomeError, ScratchHomeConfig, ScratchHome, main | routes subprocess-contract; process request supplied to kernel |
| `state-wildcard-lint` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, LintReport, scan_source, main | UNDECLARED route |
| `subprocess-contract` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RunError, run_output, run_status, bounded_output, bounded_status | kernel-owned process spawn |
| `text-structure` | files | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | Section, RawTextMatch, ScanHit, ScanVerdict, code_only | UNDECLARED route |
| `tick-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TickDispatchRules, TickDispatchDecision, admit, main | routes oracle-compare; handroll process spawn; handroll tmux |
| `tick-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | MonitorError, MonitorConfig, Observation, monitor, main | handroll process spawn; handroll tmux |
| `undrained-pipe-lint` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, LintReport, strip_line_comment, find_detailed_violations_in_source, find_violations_in_source | UNDECLARED route |
| `verify-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | VerifyDispatchRule, VerifyDispatchRules, VerifyDispatchConfig, VerifyDispatchRunOutput, now_secs | handroll process spawn; handroll br |
| `wired-but-inert-guard` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, CRONTAB_ENV, REPO_ENV, TRACKED_FILE_GLOBS, GateSpec | routes subprocess-contract; process request supplied to kernel |
| `admission-reason` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `agent-mail-native` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `asupersync-conformance` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `bead-availability` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `bead-holder` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `cargo-lane-budget` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `crate-atom-gate` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `crate-soundness-verify` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `decision-ledger` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `extraction-roster` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `inbox-monitor` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `lifecycle-event` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `lifecycle-monitor` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `omp-surface-consumption` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `orchestration-tick-gate` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `preregistration-gate` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `response-envelope-check` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `s1-coverage` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `sender-identity` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `silent-success-census` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `staged-build-gate` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED | UNDECLARED route |
| `worker-oracle-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TargetRecord, CensusReport, Classification, MeasurementNamespace, TargetVerdict | UNDECLARED route |
| `worker-tag-gate` | argv, env, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | WorkerRow, GateError, OS_DARWIN, DARWIN_HOST_TAGS, declares_os_darwin | UNDECLARED route |
<!-- CRATE-CONTRACT-ROWS-END -->

### Excluded from the workspace, and therefore from the row block

`crates/omp-idle-dispatch` is named in the root manifest's `exclude`, so it is not a package `cargo metadata --no-deps` returns and `CRI-ROSTER-METADATA` puts it outside this table. Its row was inside the block until 2026-09-12 and was a SECOND failure of the coverage leg, hidden behind the first: `validate_inventory` reports missing rows before extra ones, so twenty-three absent rows masked one present-but-ineligible row for as long as the block was incomplete.

The row is KEPT HERE VERBATIM rather than deleted, because it records a scan somebody performed and deleting it would destroy that record to satisfy a rule it predates. It is not checked by the invariant suite, and it cannot be: an excluded crate has no compiler, no CI and no gate, so every cell below is UNVERIFIED by construction.

```
| `omp-idle-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | IdleDispatchError, IdleDispatchConfig, main | handroll process spawn; handroll tmux |
```


## Validation

The row set is generated from `cargo metadata`, never hand-listed. The invariant suite fails closed on an empty package set, missing row, duplicate row, extra row, or a planted package whose row is absent.

```bash
RCH_WORKER=contabo-4 CARGO_BUILD_JOBS=2 rch exec -- cargo test -j 2 -p no-shell-gate --test crate_contract_inventory -- --nocapture
```

## Cross-References

- `Cargo.toml` — workspace package authority consumed by Cargo metadata.
- `crates/no-shell-gate/tests/crate_contract_inventory.rs` — coverage invariant, anti-vacuity, and mutation suite.
- `docs/plan/03-crates.md` — R4 requirement and historical 26-row fossil this inventory replaces.
- `docs/error_codes/exit_code_registry.md` — canonical exit-code meanings and invariant suite.
- `crates/subprocess-contract/src/lib.rs` — shared process-group and dual-pipe kernel.
- `crates/oracle-compare/src/lib.rs` — independent-oracle comparison kernel.
- `crates/tick-monitor/src/lib.rs` — ground-truth observation kernel.
- `crates/pane-dispatch-ready/src/lib.rs` — readiness/admission boundary.
- `crates/fast-dispatch/src/lib.rs` — dispatch admission and selection boundary.
- `crates/tick-dispatch/src/lib.rs` — ground-truth dispatch fence.
- `docs/contracts/subprocess_contract.md` — subprocess boundary contract.
- `docs/contracts/kernel_only_policy.md` — kernel-only routing policy.
- `docs/contracts/dispatch_claim_contract.md` — claim-before-dispatch boundary.

## Non-Coverage

- No caller migration or kernel conversion occurs here.
- No runtime proof that a declared input/output marker is complete or semantically sufficient.
- No inference that `UNDECLARED` means no I/O; it means this source pass did not establish the cell.
- No re-derivation or duplication of exit-code meanings.
- No claim that a path dependency proves every call routes through that kernel; the source-side route is recorded, not behaviorally certified.
- No lifecycle-stage split unless this document exceeds the 25 KB document bar; if it does, the split is a finding and must preserve one metadata-derived row set.

## NO-CLAIM

A one-row-per-package inventory does not prove that the package contract is correct, complete, wired, or honored at runtime. It makes missing package coverage mechanically visible; it does not make an absent input/output declaration a failure unless a separate semantic gate asserts it.
