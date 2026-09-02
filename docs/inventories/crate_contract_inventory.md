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

The row block is generated from Cargo metadata and is the artifact checked by the invariant suite. The table is intentionally compact so it remains a single reviewable inventory; lifecycle ownership belongs to the package descriptions and the stage-specific contracts.

| crate | inputs (argv/stdin/env/files read) | outputs (stdout/files/status; exit codes) | typed interface exposed/consumed | kernel routing or handroll |
|---|---|---|---|---|
<!-- CRATE-CONTRACT-ROWS-BEGIN -->
| `ack-spine` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | AckVerdict, detect_ack, simulate_singular_trap, classify_ack_readback, SingularTrapResult | routes subprocess-contract; process request supplied to kernel; kernel-routed br |
| `ack-stage` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | MAX_RETRY_ATTEMPTS, TransportKind, NtmRobotSendReceipt, TmuxSendKeysMeasurement, TransportReceipt | UNDECLARED route |
| `commit-build-fence` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, DEFAULT_TTL_SECS, BuildRegistration, ReleaseEvent, RegistrationStore | UNDECLARED route |
| `composer-typed` | argv, stdin, env, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ComposerVerdict, classify, main | UNDECLARED route |
| `convergence-stamp` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, Stamp, main | handroll process spawn |
| `dispatch-claim-fence` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, BeadStatus, BeadSnapshot, DispatchIntent | handroll process spawn; handroll br |
| `dispatch-silence-watch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SilenceVerdict, SilenceAction, classify, main | routes subprocess-contract; process request supplied to kernel; kernel-routed br |
| `dispatcher-deadman` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DispatcherDeadmanRules, DispatcherDeadmanVerdict, main | handroll process spawn; handroll br |
| `fast-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DEFAULT_FRESH_SECONDS, AdmissionConfig, FastDispatchRules, SelectError, admission_fresh_pass | handroll process spawn; handroll br; handroll bv |
| `finding` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Finding, SpooledFinding, Filed, Waived, Publisher | routes subprocess-contract; process request supplied to kernel |
| `finding-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | finding_for, main | handroll process spawn |
| `fleet-composite` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FactorSpec, InputError, CompositeReport, SelftestCheck, SelftestReport | handroll process spawn |
| `fleet-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetMonitorError, FleetMonitorConfig, AttentionEvent, monitor_once, run | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `fleet-reconcile` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReconcileVerdict, ReconcileReport, main | handroll process spawn; handroll tmux |
| `fleet-truth` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetTruthError, FleetTruthConfig, FleetSnapshot, collect, main | handroll process spawn; handroll tmux |
| `installer` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | InstallError, InstallTarget, RepoOwnership, IdentityCheck, main | routes subprocess-contract; process request supplied to kernel |
| `kernel-bypass-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Bypass, GateReport, main | handroll process spawn |
| `kernel-only-operator-hook` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | HookInput, Permission, Decision, ParseError, main | routes subprocess-contract; process request supplied to kernel |
| `loop-coverage` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | CoverageRow, CoverageReport, main | handroll process spawn |
| `loop-driver` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | DriverConfig, DriverError, DriverState, run | handroll process spawn |
| `loop-queue-filter` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Runtime, RunOutput, main | handroll process spawn; handroll br |
| `loop-switch` | UNDECLARED | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | LoopSwitchError, main | handroll process spawn |
| `loop-tick` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TickConfig, TickError, TickOutcome, wait_deadline | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `no-shell-gate` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, Verdict, GateError, WorkspaceLoad, main | routes subprocess-contract; process request supplied to kernel; handroll br; handroll spawn |
| `ntm-fleet-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | FleetMonitorError, FleetMonitorConfig, FleetAction, main | handroll process spawn; handroll tmux |
| `omp-idle-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | IdleDispatchError, IdleDispatchConfig, main | handroll process spawn; handroll tmux |
| `omp-inventory-map` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | InventoryMap, InventoryRow, InventoryNode, InventoryEdge, ProbeEvidence | handroll process spawn; handroll br |
| `omp-orchestrator` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | LoopConfig, DispatchError, SupervisorError, main | routes subprocess-contract; process request supplied to kernel; handroll tmux; handroll spawn |
| `omp-rpc-session` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RpcSessionError, RpcSessionConfig, RpcSessionEvent, run_session | handroll process spawn |
| `omp-types` | UNDECLARED | UNDECLARED; exit codes: `docs/error_codes/exit_code_registry.md` | UNDECLARED — source reserved | routes subprocess-contract |
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
| `reap-finished-panes` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReapError, ReapConfig, ReapReport, main | handroll process spawn; handroll tmux |
| `receiver-receipt` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ReceiptVerdict, ReceiptEvidence, classify, main | handroll process spawn; handroll tmux |
| `refill-idle-panes` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RefillError, RefillConfig, RefillReport, main | handroll process spawn; handroll tmux |
| `scratch-home` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | ScratchHomeError, ScratchHomeConfig, ScratchHome, main | routes subprocess-contract; process request supplied to kernel |
| `state-wildcard-lint` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, LintReport, scan_source, main | UNDECLARED route |
| `subprocess-contract` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | RunError, run_output, run_status, bounded_output, bounded_status | kernel-owned process spawn |
| `tick-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | TickDispatchRules, TickDispatchDecision, admit, main | routes oracle-compare; handroll process spawn; handroll tmux |
| `tick-monitor` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | MonitorError, MonitorConfig, Observation, monitor, main | handroll process spawn; handroll tmux |
| `undrained-pipe-lint` | argv, files | stdout, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | Violation, LintReport, strip_line_comment, find_detailed_violations_in_source, find_violations_in_source | UNDECLARED route |
| `verify-dispatch` | argv, env, files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | VerifyDispatchRule, VerifyDispatchRules, VerifyDispatchConfig, VerifyDispatchRunOutput, now_secs | handroll process spawn; handroll br |
| `wired-but-inert-guard` | files | stdout, files written, exit/status; exit codes: `docs/error_codes/exit_code_registry.md` | SCHEMA_VERSION, CRONTAB_ENV, REPO_ENV, TRACKED_FILE_GLOBS, GateSpec | routes subprocess-contract; process request supplied to kernel |
<!-- CRATE-CONTRACT-ROWS-END -->

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
