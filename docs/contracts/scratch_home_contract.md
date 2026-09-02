# Scratch Home Contract

Bead: `omp-orchestrator-scratch-home-contract-d64`

## Purpose

Define the session-scoped namespace, owner attribution, ephemeral-buffer exception, and fail-closed age-reaping boundary for scratch work that outlives the command that created it. The contract keeps durable artifacts below `ZS_SCRATCH`, treats missing or invalid ownership as `UNKNOWN`, requires proven idleness before deletion, and separates a commit-aware ownership receipt from a TTL fallback.

## Contract Artifacts

1. Canonical artifact: `crates/scratch-home/src/lib.rs`
2. Runner: the package-level command in `## Validation` below.
3. Invariant suite: the `#[cfg(test)] mod tests` in `crates/scratch-home/src/lib.rs`
4. CLI surface: `crates/scratch-home/src/main.rs`
5. Production caller: `crates/ack-spine/src/main.rs:171-201`, which creates a pending-marker job through `ScratchRoot`
6. Adjacent ownership boundary: `docs/contracts/dispatch_claim_contract.md`, law `DCL-L5-CLAIM-DIES-WITH-WORK`

## Scratch Home Model

The production namespace is:

```text
$HOME/.local/state/zeststream/scratch/<ntm-session>/<pane-or-agent>/<job>/
```

`ZS_SCRATCH` is the environment variable exported into an NTM pane. `SCHEMA_VERSION=1` identifies the `.owner.json` sidecar format. The default base is the relative suffix `DEFAULT_BASE`, resolved under `$HOME`; an explicit `ScratchRoot::new` base is an injection seam for controlled callers and test fixtures, not permission to place durable production work in an unowned temporary directory.

| Value | Stable ID | Description |
|---|---|---|
| `ScratchRoot::default()` | `SH-ROOT-DEFAULT` | Resolves `$HOME/.local/state/zeststream/scratch`. |
| `session_path(session)` | `SH-SESSION-PATH` | Validates one session component and appends it without creating it. |
| `pane_env(session)` | `SH-PANE-ENV` | Produces `ZS_SCRATCH=<session-root>/{pane}` for NTM pane creation. |
| `create_job(session, pane_or_agent, job, owner)` | `SH-JOB-OWNER` | Creates the job directory and atomically writes matching owner metadata. |
| `OwnerMetadata` | `SH-OWNER-METADATA` | Durable attribution: schema, session, pane/agent, job, and owner. |
| `OwnerActivity` | `SH-OWNER-ACTIVITY` | Activity result supplied by an independent owner/session authority. |
| `IdleProof` | `SH-IDLE-PROOF` | Identity-bound activity evidence supplied to `reap` and `apply`. |
| `UnknownEntry` | `SH-UNKNOWN` | Any missing, malformed, mismatched, symlinked, or unreadable ownership boundary. |
| `ProtectedEntry` | `SH-PROTECTED` | Age-qualified work blocked by active, unknown, conflicting, or absent idle proof. |
| `ReapCandidate` | `SH-REAP-CANDIDATE` | A matching owner sidecar, age-qualified job, and matching idle authority. |
| `ReapReport` | `SH-REAP-REPORT` | Separates candidates, protected entries, and unknown entries; only candidates may reach `apply`. |
| `unix_now()` | `SH-CLOCK` | Current Unix seconds for callers that need a timestamp. |

The nine local public types are `ScratchError`, `ScratchRoot`, `OwnerMetadata`, `OwnerActivity`, `IdleProof`, `ReapCandidate`, `UnknownEntry`, `ProtectedEntry`, and `ReapReport`. `SCHEMA_VERSION`, `ENV_VAR`, and `DEFAULT_BASE` are public constants, not additional types.

## Operations and Ownership

- `ScratchRoot::ensure_session` creates only the validated session directory and rejects a symlink root.
- `ScratchRoot::pane_env` and `ntm_spawn_args` carry the session-scoped path into NTM rather than asking a caller to reconstruct it.
- `ScratchRoot::create_job` validates session, pane/agent, job, and owner components; rejects traversal and control characters; creates the job; then atomically renames `.owner.json.<pid>-<nonce>.tmp` into `.owner.json`.
- `ScratchRoot::reap` scans one session and produces a report. Valid sidecar identity, age, and a matching idle proof can produce a candidate; active, unknown, conflicting, or absent idle evidence is protected.
- `ScratchRoot::apply` revalidates each candidate immediately before recursive removal, including identity and idle proof. A report is not by itself authority to delete.
- `ReapReport::auto_reapable` is a diagnostic projection of whether protected or unknown entries exist; it is not a substitute for candidate revalidation.

An owner string in `.owner.json` is durable attribution for recovery, not proof that an owner process is alive. A live ownership claim must have a lifecycle receipt outside this crate and must die with the owned job or its explicit resolution. A PID in a durable marker is never sufficient evidence of current ownership.

## Laws

### SH-L1-SESSION-SCOPED

Durable work belongs under `$HOME/.local/state/zeststream/scratch/<session>/<pane-or-agent>/<job>/` and is exported as `ZS_SCRATCH`. Production callers must use `ScratchRoot::default`, `create_job`, or the resulting `pane_env`; they must not place durable artifacts under `/private/tmp` or `$TMPDIR`. An explicit `ScratchRoot::new` is allowed only for a controlled, attributed root such as a test fixture or an explicitly owned alternate state root.

### SH-L2-OWNERSHIP-DIES-WITH-WORK

Ownership is a lifecycle claim, not a string that lives forever. The job and its `.owner.json` attribution are one owned unit: when the job is closed, cancelled, reassigned, or explicitly resolved, the owner claim ends and the job may be removed through the reaper. A transient process or shell cannot leave a PID marker that later masquerades as a live claim. Durable owner metadata may survive process death only as attribution for safe recovery; it never proves that the dead process still owns the job.

This agrees with `dispatch_claim_contract.md` law `DCL-L5-CLAIM-DIES-WITH-WORK`: the work lifecycle ends ownership, while a clock is only crash recovery. The scratch contract specializes that rule to filesystem ownership and adds the requirement that owner liveness or job idleness come from an independent authority rather than from the sidecar text.

### SH-L3-UNKNOWN-INHABITED

Missing, malformed, wrong-schema, mismatched, symlinked, non-regular, or unreadable owner metadata produces an inhabited `UnknownEntry`. `UNKNOWN` is never converted to a reap candidate and is never auto-reaped. Symlinked roots, panes, jobs, and files are likewise not trusted as owned scratch. `apply` removes only a reported candidate whose ownership is revalidated; it never searches for or invents an owner.

### SH-L4-OWNER-AND-IDLE-REQUIRED

Age reaping requires both a proven owner and a proven idle window. Filesystem age alone is not a proof that the job is idle, the owner is gone, or the work is resolved. `reap` requires a matching `IdleProof` from an authority with a non-empty identity; active, unknown, conflicting, or absent proof yields `ProtectedEntry` rather than a candidate.

`apply` revalidates owner identity and idle proof immediately before deletion, but the current implementation calls `inspect_job(..., Duration::ZERO)`, so it does not recheck the original age threshold at mutation time. That is an explicit age-only conformance gap, not a safe interpretation of age, and is tracked by `omp-orchestrator-scratch-reaping-owner-idle-proof-jwe`.

### SH-L5-EPHEMERAL-MKTEMP

A single-command ephemeral buffer may use `mktemp` only when it is removed before the command exits. It must not be used as a durable handoff, pending marker, build artifact, or cross-process job root. Any data that survives the creating command uses the session-scoped, attributed namespace instead. Test fixtures may use a unique temporary directory only when they remove it before the test returns and do not expose it as production scratch.

## Commit-Aware Ownership Receipt

A path-scoped commit can be the preferred receipt for releasing a file reservation after the protected work lands: `LEASE-HELD` → commit observed → `LEASE-RELEASED`. TTL remains the crash-recovery fallback. This is the same direction as `DCL-L5-CLAIM-DIES-WITH-WORK`, and there is no disagreement: both contracts reject a TTL expiry as proof that work ended.

The commit is not, by itself, proof that a scratch job is idle, that an owner process exited, or that a dispatch was delivered. `scratch-home` does not own git or Agent Mail, so it can express this lifecycle boundary but cannot enforce commit observation or release a reservation. The reaper still needs its own owner and idle evidence.

## VIOLATION — where shipped code contradicts this law

- **Declared:** `SH-L1-SESSION-SCOPED` forbids durable scratch fallback to an unowned temporary directory. **Shipped:** `crates/ack-spine/src/main.rs:184-197` falls back to `std::env::temp_dir()` for `pending.json` when `ScratchRoot::default().create_job(...)` fails. **Consequence:** the pending marker outlives the command without session/pane/job attribution and cannot be safely reaped. Tracked by `omp-orchestrator-remove-unattributed-scratch-fallback-7bp`.
- **Declared:** `SH-L4-OWNER-AND-IDLE-REQUIRED` requires age and current idle proof at deletion. **Shipped:** `crates/scratch-home/src/lib.rs:322-327` revalidates with `inspect_job(..., Duration::ZERO)`, so the original age threshold is not rechecked even though the current implementation accepts idle proofs. **Consequence:** a previously reported candidate can be removed without a current age proof. Tracked by `omp-orchestrator-scratch-reaping-owner-idle-proof-jwe`.
- **Declared:** `SH-L2-OWNERSHIP-DIES-WITH-WORK` distinguishes durable attribution from live ownership. **Shipped:** `crates/scratch-home/src/lib.rs:362-385` stores static owner/session/job strings and an activity result, but no owner-lifecycle receipt. **Consequence:** the sidecar and idle proof can attribute and classify a job, but cannot prove that the named owner is still alive or that the work lifecycle ended; they must not be treated as a live lease.

## Non-Coverage

- No caller migration, reaper hardening, or ack-spine fallback repair is included; fallback repair is tracked by `omp-orchestrator-remove-unattributed-scratch-fallback-7bp`, and owner/idle reaping is tracked by `omp-orchestrator-scratch-reaping-owner-idle-proof-jwe`.
- No process liveness, NTM session state, pane activity, git commit observation, Agent Mail reservation release, or operator approval is invented by this crate.
- No arbitrary `ScratchRoot::new` base is accepted as proof of production ownership; callers remain responsible for using an attributed state root.
- No PID, shell lifetime, mtime, or owner string alone proves that a job is idle or that its ownership ended.
- No unknown or malformed entry is auto-reaped, even when other known candidates exist.
- No `mktemp` directory or temporary file is durable scratch. Test-only temporary fixtures are outside the production namespace and must be cleaned before the test returns.
- No full-workspace build or test suite is part of this contract validation; the package-level invariant runner is the intended execution boundary.

## Validation

```bash
tmp="$(mktemp -d)"; trap 'rmdir "$tmp"' EXIT; TMPDIR="$tmp" RCH_ENABLED=false CARGO_MINT_MIN_CONTAINER_PCT=0 cargo test -p scratch-home --lib -- --nocapture --test-threads=1
```

## Cross-References

- `crates/scratch-home/src/lib.rs` — namespace, component validation, owner sidecar, reaping, and tests
- `crates/scratch-home/src/main.rs` — `resolve`, `pane-env`, `job`, `reap`, and `spawn` CLI operations
- `crates/ack-spine/src/main.rs:171-201` — production caller and current unattributed fallback
- `crates/subprocess-contract/src/lib.rs` — passthrough subprocess boundary used by scratch-home spawn
- `crates/tick-monitor/src/lib.rs` — session/pane observation authority; not owned by scratch-home
- `docs/contracts/dispatch_claim_contract.md` — `DCL-L5-CLAIM-DIES-WITH-WORK` and commit/TTL boundary
- `docs/contracts/ack_spine_contract.md` — durable uncertainty is not delivery or acknowledgement
- `docs/contracts/pane_observation_contract.md` — independent evidence is stronger than one capture
- `AGENTS.md` — mandatory `ZS_SCRATCH` path and unknown-owner prohibition
- `omp-orchestrator-remove-unattributed-scratch-fallback-7bp` — fallback defect bead
- `omp-orchestrator-scratch-reaping-owner-idle-proof-jwe` — owner/idle reaping defect bead

## NO-CLAIM

This contract defines the scratch ownership and reaping boundary but does not make current callers conform. It does not prove that the ack-spine fallback is removed, that `apply` rechecks age, that any owner is alive, or that any job is actually idle. It does not bind Agent Mail reservations automatically to commits; it only specifies the preferred commit-aware release receipt with TTL as crash fallback. A clean `scratch-home` invariant suite proves the crate's modeled filesystem cases, not the safety of every caller or the truth of an external lifecycle authority.
