# 10 — What would Jeffrey do: prior art mined from the mirror

**Requirement served: R7.** *"use fh — mine the dicklesworthstone projects along the way — anywhere we find a gap — we should ask — what would jeffrey do in one of his projects."*

Nine named gaps are researched below. Each record gives the gap, exact search, a named construct and quote (or a scoped not-found), and an **ADOPT / ADAPT / REJECT** verdict. Mirror citations are tied to §10.1; installed OMP citations are tied to §11.1. A declaration is not runtime proof, and a runtime injection receipt is not recipient acknowledgement. The nine gaps and the seven OMP rows in §11 are not one-to-one: only Gaps 1 and 7 have direct OMP mappings; the other OMP rows are adjacent mechanisms.

---

## 0. Corpus, tooling, and measurement boundaries

Recorded filesystem measurements:

```
ls -1 | wc -l                                      -> 218   # visible entries
find . -maxdepth 1 -mindepth 1 -type d | wc -l     -> 217   # directories
find . -maxdepth 2 -mindepth 2 -name .git | wc -l  -> 210   # git work-trees
ls -1 | grep -c corrupt                            -> 1     # ntm.corrupt-20260819
```

The research denominator is **MEASURED 210 git work-trees**, not 218 entries, 217 directories, or Brief §3.7's un-re-derived “216 repos.” `fh` refused stale input as `SERVE_INPUT_STALE` after mirror movement `5dec4212… -> ecdea397…`; its CLI also refused `SEARCH_INDEX_STALE` at exit 3:

```
SEARCH_INDEX_STALE: published key f2845efff917afd4 differs from current b1acb6e7b011b1f5
hint: run `fh technical-manifest`
```

All mirror searches therefore ran against the filesystem, without semantic-index assistance. Full mirror commit IDs were unavailable in this pass; §10.1 records that limitation instead of inventing revisions.

### 0.1 Measurement ledger

| measure | derivation/input | result and boundary |
|---|---|---|
| mirror denominator | `find . -maxdepth 2 -mindepth 2 -name .git \| wc -l` | **MEASURED 210** work-trees at one snapshot; not 216/218 entries |
| visible entries / directories | `ls -1 \| wc -l`; `find . -maxdepth 1 -mindepth 1 -type d \| wc -l` | **MEASURED 218 / 217**, not project denominators |
| `vergen` matches | `grep -rl vergen --include=Cargo.toml . \| wc -l` | **MEASURED 18 manifest paths / 210 work-trees**; unique-repository numerator is NO-CLAIM |
| supervision variants | direct reads of `enum SupervisionEvent` and `enum StopReason` | **MEASURED 8 + 6 = 14 variants** in the named file, not corpus-wide |
| OMP completion capture | `/tmp/grade/r7-agent-end.md` and raw frame | **MEASURED 1 `agent_end` frame** in one ephemeral run |
| seeded corrections | comparison of nine seeded Gap rows with this record | **PROJECTED 6/9 = 66.7%**; document comparison, not an independent run |
| refuted not-founds | four false-zero mechanisms described below | **PROJECTED 7** session count; completeness and attribution are NO-CLAIM |

The correction arithmetic is `6 ÷ 9 = 0.666…`. “Strongest,” “highest priority,” and similar rankings are not measured claims; where priority is discussed it is marked **PROJECTED** from stated evidence.

Four false-zero mechanisms remain explicit: an empty `--include=` can return exit 0; an extension filter can target the wrong language (`ntm` is Go); a search space can omit where the answer lives; and a not-found can be published without a recorded search. A negative result is publishable only with its command, source scope, observed output, and remaining NO-CLAIM.

---

## Gap 1 — A publish that returns no receipt

**Gap.** Dispatch emits no typed acknowledgement, so “sent” and “accepted” are one observable.

**Search.** `grep 'pub (struct|enum) (PublishReceipt|AckKind|DeliveryClass|PublishPermit)'` over the whole `asupersync/src/messaging` module.

**Found.** `asupersync/src/messaging/fabric.rs:1913` (`struct PublishReceipt`) carries `subject`, `payload_len`, `ack_kind`, and `delivery_class`; `asupersync/src/messaging/fabric.rs:1944` carries `#[must_use = "a PublishPermit must be sent or explicitly aborted"]`. `AckKind` is `asupersync/src/messaging/class.rs:83`; `DeliveryClass` is `asupersync/src/messaging/class.rs:17`; `cost_vector` and `minimum_ack` carry the `#[must_use]` rule at `asupersync/src/messaging/class.rs:43,56`.

**OMP boundary.** `irc/bus.d.ts:30-34` (`interface IrcDeliveryReceipt`) reports only `injected | woken | revived | failed`; `irc/bus.d.ts:53-61` says it reports how a message reached the recipient, “not what they did with it.” `async/job-manager.d.ts:37-48` (`type AsyncJobDeliverySink`) is a callback/dead-letter shape. **NO-CLAIM:** neither declaration proves recipient acceptance, readback, or durable acknowledgement.

**Verdict: ADAPT.** `crates/omp-types/Cargo.toml:10-18` records the local dependency boundary. Adopt an ack-bearing receipt and non-droppable permit locally; keep `cp-z42vu` ADAPT until recipient-level runtime proof exists.

---

## Gap 2 — An allowance list that outlives the defect it records

**Gap.** Declare an unwired lane without allowing the declaration to outlive its defect.

**Search.** `grep -n 'UNWIRED_LANE_ALLOWANCE' franken_lean/crates/fln-conformance/tests/contract_roots.rs`.

**Found.** `franken_lean/crates/fln-conformance/tests/contract_roots.rs:284-288` (`UNWIRED_LANE_ALLOWANCE`) is empty. Its doc comment says an undeclared unwired lane fails and a declared lane that has since been wired also fails. `fn allowance_verdict_fails_in_both_directions` at `franken_lean/crates/fln-conformance/tests/contract_roots.rs:777` exercises both directions; refusal wording is at `franken_lean/crates/fln-conformance/tests/contract_roots.rs:757-761`.

**Scoped NO-CLAIM.** Source root is `/Volumes/ZestData/dicklesworthstone-mirror`; search space is the named contract-root file plus `crates/omp-inventory-map/src/types_inventory.rs:176-178`; observed allowance is empty and the dual-direction test is named. This does not establish every allowance list in the corpus or that the local inventory list is fixed.

**Verdict: ADOPT** the dual-direction allowance shape.

---

## Gap 3 — A binary that cannot say which source built it

**Gap.** An installed binary cannot prove it matches the source tree it claims.

**Search.** `grep -rl vergen --include=Cargo.toml .` then `grep -rn 'binary_identity|build_id|running_binary'`. The first command yields **18 matching manifest paths**, not 18 unique repositories.

**Found.** `beads_rust/build.rs:41-45` (`fn emit_git_metadata`) emits `VERGEN_GIT_DIRTY`. `frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846` (`fn running_binary_identity`) states: “Fails closed: a gate that cannot name the exact binary it measured is not admissible evidence, so an unresolvable path or any read error panics rather than degrading to an unidentified run.”

**Scoped NO-CLAIM.** The result establishes identity metadata in 18 matching manifest paths and the named fail-closed construct in the recorded mirror snapshot. It does not establish 18 repositories, an exposed `br` identity, or that `fh` source is in the mirror. The old `ls -1 | grep -i harv` top-level probe cannot establish source absence.

**Verdict: ADOPT (PROJECTED locally).** Adopt tree digest plus dirty flag and require a local runtime identity capture before claiming drift prevention.

---

## Gap 4 — The canonical doctor shape

**Gap.** `omp-inventory-map --help` returns `CONFIG_ERROR unknown argument --help`; doctor must remain discoverable and runnable while the workspace is broken.

**Search.** `grep -rn 'DoctorExitCode' beads_rust/src`; `grep -n 'Commands::Doctor' beads_rust/src/main.rs`; `find pi_agent_rust -name doctor.rs`.

**Found.** `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:45-51` (`enum DoctorExitCode`) declares eleven variants in two bands: domain verdicts 0–6 and sysexits 64/66/73/74. `FixFailedRolledBack = 3` is documented at `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:21-24` as returning to the verbatim backup. `beads_rust/src/cli/commands/doctor_subsystems/capabilities_doctor.rs:1-15` (`br.doctor.capabilities.v1`) declares `write_scopes`, `env_vars`, `fixers`, `detectors`, and derived `exit_codes`. `eidetic_engine_cli/src/cache/hotset.rs:1504,1519` emits a runnable `repair` string.

The derived list reduces this specific drift; it does not prove drift impossible under all edits. Operationally, the useful shape is that the tool run to diagnose a broken workspace must not require that workspace to be intact.

**Verdict: ADOPT**, as the spine of `07-installability.md`: typed two-band exits, derived capabilities, runnable repair text, and doctor preflight exemption.

---

## Gap 5 — Mutation through a real hook, not a fixture

**Gap.** Fixture tests cannot prove the installed hook still refuses.

**Search.** `grep -rln 'hooks/pre-commit' beads_rust franken_lean destructive_command_guard` with no extension filter.

**Found.** `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362` copies the real hook into a lab repository and chmods it executable. `franken_lean/scripts/git-hooks/test_projection_guard.sh:202-212` drives real `git commit` and checks chaining to an existing `.git/hooks/pre-commit`. `franken_lean/ci/VERIFICATION_MANIFEST.jsonl:93` explains why a silent successful hook needs a planted-defect cell. `franken_lean/scripts/git-hooks/test_projection_guard.sh:520-524` records size-dependent refusal rates as a race measurement, not a threshold.

`asupersync/src/subsystem_mutation_testing.rs:9` is a counterexample: it builds a `LabRuntime` over a `TempDir`, so it is a fixture. **NO-CLAIM:** the search establishes both patterns, not that every hook has installed-artifact coverage.

**Verdict: ADOPT** the real-hook plus planted-defect rule.

---

## Gap 6 — Refusing an empty scan set

**Gap.** A scan that returns no input must not read as a clean result.

**Search.** The first documentation-only search was a false zero. Re-derived with no extension filter:

```
grep -rli 'vacuous' asupersync                         -> 37 files
grep -rlEi 'vacuit' --include=*.rs .                  -> 236 files
grep -rlEi 'anti.vacuity' --include=*.rs .            -> 63 files
```

The synonym set also included `scanned zero`, `empty scan set`, `no files were scanned`, `scan set is empty`, `would pass vacuously`, and `zero (files|candidates) (scanned|examined)`. **Scoped NO-CLAIM:** these are matching-file counts from the indicated roots and synonyms, not unique repositories or exhaustive absence.

**Found.** The named shapes are present in `asupersync/src/messaging/jetstream.rs:2460` (`vacuous_zero_wait_refusal`) and `scripts/run_jetstream_publish_backpressure_smoke.sh:181-186`; `asupersync/src/runtime/scheduler/metamorphic_tests.rs:438-442,517-522,661-662` asserts exercised workloads; `franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115` enforces `workflows.len() >= 2`; `franken_lean/tribunal/epoch-lab/tests/derived_input_provenance.rs:538` asserts `item_count > 0`; `franken_lean/tribunal/epoch-lab/tests/build_gate_governed_sets.rs:549,613` names `VACUOUS PASS`; `asupersync/tests/atp_rq_observability_metrics.rs:134-135` uses a positive control; and `asupersync/src/trace/tla_export.rs:111-114` plus `combinator/map_reduce.rs:140-144` carry the rule into types/returns.

`asupersync/CHANGELOG.md:1077-1078` records six RFC 9112 tests that previously passed vacuously; `audit_index.jsonl:3251` records `MR2 cancellation_state_consistency` as fixed after a vacuous test. These are source-record counts, not comparative rankings.

**Verdict: ADOPT — PROJECTED priority** from the production-telemetry citation and our census defect. Shape 1 is the first implementation candidate, not a measured ranking.

---

## Gap 7 — A worker that cannot say it is done

**Gap.** Brief §4’s `complete` row says worker completion was found by a human looking.

**Search.** `grep 'pub enum Outcome' asupersync/src`; then `grep 'pub enum (ChildExit|ExitReason|ChildOutcome|SupervisionEvent|ChildStatus)' supervision.rs gen_server.rs spork.rs` over the three enumerated supervision surfaces.

**Found, half.** `asupersync/src/types/outcome.rs:213-227` (`enum Outcome<T,E>`) declares `Ok`, `Err`, `Cancelled`, and `Panicked`, with the cited severity order `Ok < Err < Cancelled < Panicked`.

**Not found in the inspected declarations.** `asupersync/src/supervision.rs:3122` (`enum SupervisionEvent`) has eight variants and `asupersync/src/supervision.rs:3098` (`enum StopReason`) has six; none means worker success, and `RestartComplete` means restart completion. This is **MEASURED 8 + 6 = 14 variants in these declarations**, not a 210-work-tree result. The adjacent `EvidenceEntry` record at `asupersync/src/supervision.rs:3208-3213` is described as a structured, deterministic, test-assertable supervision-decision record.

**Scoped NO-CLAIM.** Source root `/Volumes/ZestData/dicklesworthstone-mirror`; search space is `asupersync/src` and the three named supervision files; observed no success variant in those declarations; unestablished are absence in other work-trees, runtime semantics, and local consumption.

**Verdict: ADAPT.** Adopt `Outcome<T,E>` and the one-entry-per-decision ledger as **PROJECTED** local design inputs. The mirror result alone does not justify inventing a new completion protocol because OMP supplies a separate candidate, documented in §11.

---

## Gap 8 — Per-adapter scoping and typed missing dependencies

**Gap.** A CLI run inside another repository must scope what it touches and degrade per adapter when a dependency is absent.

**Search 1 (Rust-only, negative).** `grep '(adapter|Adapter)\\w*(registry|Registry|scope|Scope)|per-adapter' beads_rust/src eidetic_engine_cli/src` -> no matches. **Scoped NO-CLAIM:** this establishes only that regex had no match in those two Rust roots; it says nothing about asupersync or Go.

**Search 2 (asupersync declaration).** `grep -rn 'AdapterCategory|AdapterCertificationStatus|AdapterRenderedStatus|AdapterCertificationDeclaration' asupersync/src/adapter_certification.rs` finds the module doc at `asupersync/src/adapter_certification.rs:1-6`, `enum AdapterCategory` at `asupersync/src/adapter_certification.rs:10`, `enum AdapterCertificationStatus` at `asupersync/src/adapter_certification.rs:39`, `enum AdapterRenderedStatus` at `asupersync/src/adapter_certification.rs:65`, and `struct AdapterCertificationDeclaration` at `asupersync/src/adapter_certification.rs:88`.

**Search 3 (Go dependency vocabulary).** `grep -rn 'ErrNotInstalled|DEPENDENCY_MISSING' ntm` with no extension filter finds the Go vocabulary. The spaces are separate; the asupersync result was not a first-pass result from the Rust-only command.

**Found.** `ntm/internal/bv/bv.go:31`, `ntm/internal/cass/client.go:13`, and `ntm/internal/caut/client.go:14` define typed `ErrNotInstalled` sentinels. `ntm/docs/robot-action-handoff-contract.md:379` defines `ErrCodeDependencyMissing = "DEPENDENCY_MISSING"`. `ntm/internal/cli/bugs.go:85-89` carries remediation in the envelope; `ntm/internal/alerts/generator.go:383-385` makes per-call-site degradation explicit; `ntm/internal/cli/robot_registry_conformance_test.go:15` pins the exit-code taxonomy.

**Verdict: ADOPT (PROJECTED locally)** the typed sentinel, wire taxonomy, in-envelope remediation, per-call-site policy, and conformance-test shape. No runtime claim is made for this repository.

---

## Gap 9 — Probing tool presence without trusting exit codes

**Gap.** A present binary can be marked absent when its chosen version flag fails.

**Search.** `grep -n 'PresenceOnly|ProbeExecution|fn check_tool|fn probe_failure_is_known_nonfatal|fn which_tool|status.success()' pi_agent_rust/src/doctor.rs`, then read each named construct.

**Found.** In `pi_agent_rust/src/doctor.rs`, `fn check_tool` is at `pi_agent_rust/src/doctor.rs:924`; the naive success arm at `pi_agent_rust/src/doctor.rs:950`; the two-signal arm at `pi_agent_rust/src/doctor.rs:967-968`; `fn probe_failure_is_known_nonfatal` at `pi_agent_rust/src/doctor.rs:1052`; its one-tool allowlist at `pi_agent_rust/src/doctor.rs:1057`; and `fn which_tool` at `pi_agent_rust/src/doctor.rs:1066`. Tests `fn check_tool_falls_back_when_probe_args_are_unsupported` and `fn check_tool_reports_invocation_failure_for_broken_executable` are at `pi_agent_rust/src/doctor.rs:13948` and `pi_agent_rust/src/doctor.rs:13964`. The design separates presence (`which_tool`) from version probing and forgives only a named failure.

The workstation measurement is:

```
tmux --version                          -> exit 1, STDERR banner, stdout empty
env -i /opt/homebrew/bin/tmux --version -> exit 1
tmux -V                                 -> exit 0, "tmux 3.6a"
```

The earlier exit-0 claim came from `tmux --version 2>&1 | head -1` (`PIPESTATUS=(1 0)`), not tmux. **NO-CLAIM:** this is one workstation’s `/opt/homebrew/bin/tmux` 3.6a measurement, not all binaries or environments.

**Verdict: ADOPT + NAMED GAP.** Adopt the two-signal structure and both tests; the cited allowlist omits tmux, so tmux still falls into the failure arm unless its `-V` flag is explicitly configured.

---

## Summary and direct mapping

| # | plan gap | verdict | normalized evidence |
|---|---|---|---|
| 1 | delivery receipts | **ADAPT** | mirror `asupersync/src/messaging/fabric.rs:1913,1944`, `asupersync/src/messaging/class.rs:17,43,56,83`; OMP `irc/bus.d.ts:30-34` injection-only |
| 2 | unwired allowance | **ADOPT** | `franken_lean/crates/fln-conformance/tests/contract_roots.rs:284-288,757-761,777` |
| 3 | binary identity | **ADOPT (PROJECTED locally)** | `beads_rust/build.rs:41-45`; `frankensqlite/crates/fsqlite-e2e/tests/bd_wsw3p_concurrent_write_showcase.rs:840-846`; 18 is manifest paths, not repos |
| 4 | doctor shape | **ADOPT** | `beads_rust/src/cli/commands/doctor_subsystems/exit_codes.rs:21-24,45-51`; `beads_rust/src/cli/commands/doctor_subsystems/capabilities_doctor.rs:1-15`; `beads_rust/src/main.rs:104,297`; `eidetic_engine_cli/src/cache/hotset.rs:1504,1519` |
| 5 | real hook mutation | **ADOPT** | `franken_lean/crates/fln-conformance/tests/evidence_finalization.rs:360-362`; `franken_lean/scripts/git-hooks/test_projection_guard.sh:202-212,520-524`; `franken_lean/ci/VERIFICATION_MANIFEST.jsonl:93` |
| 6 | anti-vacuity | **ADOPT (PROJECTED priority)** | `asupersync/src/messaging/jetstream.rs:2460`; `asupersync/src/runtime/scheduler/metamorphic_tests.rs:438-442`; `franken_lean/crates/fln-conformance/tests/marrow_sanitizer_dispatch.rs:105-115`; `asupersync/CHANGELOG.md:1077-1078` |
| 7 | worker completion | **ADAPT mirror / OMP adoption candidate** | mirror `asupersync/src/supervision.rs:3098,3122` scoped negative; OMP `AgentEndEvent` declaration and capture in §11 |
| 8 | adapter scope/dependency | **ADOPT (PROJECTED locally)** | `ntm/internal/bv/bv.go:31`; `ntm/docs/robot-action-handoff-contract.md:379`; `ntm/internal/cli/bugs.go:85-89`; `ntm/internal/alerts/generator.go:383-385`; `ntm/internal/cli/robot_registry_conformance_test.go:15` |
| 9 | tool probe | **ADOPT + NAMED GAP** | `pi_agent_rust/src/doctor.rs:924,950,967-971,1052,1057,1066,13948,13964`; workstation tmux measurement above |

**Correction count:** seeded comparison is **PROJECTED 6/9 = 66.7%**; seven refuted not-founds is also **PROJECTED** from the session record. These are not mirror coverage rates. Gap 7’s old corpus-wide precedent-free conclusion is retracted.

**Direct mapping:** Gap 1 maps to receipt prior art; Gap 7 maps to completion prior art. The five other OMP rows are adjacent mechanisms, not replacements for Gaps 2–6, 8, or 9.

---

## 10.1 Mirror source manifest and search provenance

| source | revision identity | search/input | durable evidence |
|---|---|---|---|
| Jeffrey mirror | root `/Volumes/ZestData/dicklesworthstone-mirror`; snapshot manifest `/Volumes/ZestData/dicklesworthstone-mirror/MANIFEST.json` SHA-256 `09640f3d3f7fdabbd21d13aa3e3881d8e88aa8d080c21075fb3ed9765e530947`; sync log SHA-256 `9e6af9e7df9de50020dd28a8235aed276515699143e6c31973538e437dc51b40`; only observed movement `5dec4212… -> ecdea397…`; full commit unavailable because stale input was refused | per-gap commands above; Gap 7 limited to named asupersync roots/files | cited source constructs at the recorded snapshot; no broader absence claim |
| OMP signal sweep | installed package, not mirror | suffix sweep over all `.d.ts` plus field reads; root and instrument are recorded in the artifact | `/tmp/grade/omp-signals.md`, SHA-256 `f9b33f5e9f11ab5f003740b2161cb2aebdf77691615d81070c3051a796819cb0` |
| OMP completion capture | installed package, not mirror | `/Users/josh/.local/bin/omp --mode=rpc --no-session --no-tools --no-lsp --max-time=30`; stdin prompt `AGENT_END_PROBE_OK` | `/tmp/grade/r7-agent-end.md`, SHA-256 `661a6125fe36a71fc698ddadcfebb6769cbcea2c5f92e736c3e6c10d37af0d50`; raw frame SHA-256 `d8bd80c6949b2ec48af1639b5b5e241bd90b4dce1e769483dd1690ed2be8f644` |
| mux adjacent probe | installed package, not mirror | Content-Length JSON-RPC `omp/muxPing` against scoped `lsp-mux.sock` endpoints, three probes per endpoint | `/tmp/grade/mux-investigation.md`, SHA-256 `d6a58a671112fadb95e3f1fe4499eaf659af26d6007c64f983eb0d8356ec34f1` |

---

## 11. OMP typed mechanisms: declarations versus runtime

### 11.1 Installed package identity

`@oh-my-pi/pi-coding-agent` version `18.0.11`, repository `https://github.com/can1357/oh-my-pi.git`, package directory `packages/coding-agent`, installed root `/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent`.

`package.json` SHA-256 `dbd14cc1f445c16d485fa0571178a69100b8c485a42549d1643db390ebd2cc53`.

Declaration-file SHA-256s: `extensibility/shared-events.d.ts` `78a6e4236680fa2439f1f54c42fb04600fbf165c3901892a028a4f9064e0d910`; `tools/hub/types.d.ts` `78872c3812d17ab9a7121dd6e1b142e45d5c358e003c8fa6741904691e862053`; `irc/bus.d.ts` `ceb6083006b67db05cdf9244bbfdef20f5626c9a9bdb740c3efa57baaa7db0ed`; `async/job-manager.d.ts` `9da456cc3f70f113abaf683bee9ee4a75a36f629bbb7f4f4cfb5df292e5a3f06`; `modes/rpc/rpc-types.d.ts` `be5dece0f09f9931aba483905a51a94dfd84296f06d07a4cc462a07828e68f7f`; `session/agent-session-events.d.ts` `5668a7f0df5cbc24dca00ed5acc51b6b7144f6616a9d4cd81ad4dfd36453ed1b`; `memories/storage.d.ts` `2cf89566a79d2fbf432ec2a82066ba903653c192b06b9e5337c792ce9285441f`; `collab/guest.d.ts` `66f4a581a0586fd8dece35bc82768f7ff21de2cd97acf8fce69546f1a114b61c`.

### 11.2 Exact typed-mechanism map

| plan mapping | exact OMP declaration and construct | evidence level / boundary |
|---|---|---|
| Gap 7, worker completion | `extensibility/shared-events.d.ts:153-163`, `interface AgentEndEvent` (`type: "agent_end"`, `messages`, optional `willContinue`); `shared-events.d.ts:325-327`, `interface SessionStopEventResult` (`continue?`); `shared-events.d.ts:82-93`, `interface SessionStopEvent` has no `settle`; `session/agent-session-events.d.ts:10-18`, `type AgentSessionEvent`; `modes/rpc/rpc-types.d.ts:589`, `type RpcSessionEventFrame = AgentSessionEvent | RpcSubagentFrame` | **WIRE-PROVEN** only for the captured run; aliases are declaration evidence, not standalone runtime proof |
| Gap 1, dispatch receipts | `irc/bus.d.ts:30-34`, `interface IrcDeliveryReceipt`; `tools/hub/types.d.ts:79-90`, `CoordinationDetails.receipts`; `async/job-manager.d.ts:37-48`, `type AsyncJobDeliverySink` | **DECLARED ONLY / transport injection**; outcomes are `injected|woken|revived|failed`, not recipient acceptance |
| adjacent claim/ownership | `memories/storage.d.ts:18-29`, `Stage1Claim` and `GlobalClaim` with `ownershipToken` and `inputWatermark` | **DECLARED ONLY**; candidate schema, no local consumer or runtime capture |
| adjacent idle reconciliation | `collab/guest.d.ts:9-17`, `GuestIdleReconcilerCtx`; `collab/guest.d.ts:18-30`, `reconcileGuestIdleHostState(ctx, isStreaming)` | **DECLARED ONLY**; `isStreaming` UI reconciler, not settle/continuation proof |
| adjacent roster | `tools/hub/types.d.ts:32-39`, `interface HubRosterCounts` with `running`, `idle`, `parked`, `shown`, `truncated` | **DECLARED ONLY**; no runtime roster capture here |
| adjacent cost measurement | `extensibility/extensions/types.d.ts:238-241`, `interface ContextUsage`; `extensibility/extensions/types.d.ts:303`, `getContextUsage()` | **DECLARED ONLY**; “Estimated context tokens” is not a cost ledger |
| adjacent compaction | `extensibility/shared-events.d.ts:53-77`, exact `SessionBeforeCompactEvent`, `SessionCompactingEvent`, `SessionCompactEvent`; `extensibility/extensions/types.d.ts:832-834` handler registrations | **DECLARED ONLY**; typed hooks, no process capture or recovery proof |

### 11.3 Per-mechanism scoped NO-CLAIMs

- **AgentEndEvent:** one ephemeral `--mode=rpc` capture observed one `agent_end` frame. **NO-CLAIM:** no claim about every OMP mode, continuation behavior beyond that frame, or local adapter consumption.
- **IrcDeliveryReceipt / AsyncJobDeliverySink:** **NO-CLAIM:** local injection vocabulary is not recipient delivery, acceptance, readback, or durable acknowledgement.
- **Stage1Claim / GlobalClaim:** **NO-CLAIM:** declared fields are not proof of ownership enforcement or watermark correctness here.
- **GuestIdleReconcilerCtx:** **NO-CLAIM:** an `isStreaming` reconciler is not proof of `NewlyIdle`/`ConfirmedIdle` transitions here.
- **HubRosterCounts:** **NO-CLAIM:** five tally fields are not proof of live roster truth or churn handling.
- **ContextUsage:** **NO-CLAIM:** estimated tokens are not a cost ledger or spend measurement.
- **SessionBeforeCompactEvent / SessionCompactingEvent / SessionCompactEvent:** **NO-CLAIM:** typed hooks are not proof of capture, persistence, or recovery after compaction.

### 11.4 Gap 7 retraction

The capture artifact `/tmp/grade/r7-agent-end.md:26-45` observed one `agent_end` frame in an OMP `--mode=rpc` stream. Its parsed frame at `/tmp/grade/r7-agent-end.md:47-71` has `isTerminal: true`, two messages, and absent `willContinue`. Gap 7 is therefore **REFUTED as a claim that no completion signal exists in OMP**, while the mirror negative remains a scoped declaration result. Remaining work is adapter/event consumption, not inventing a new completion type.

### 11.5 Receipt boundary

The OMP receipt is **transport-injection prior art only**. `irc/bus.d.ts:53-61` says it reports how the message reached the recipient, “not what they did with it”; `async/job-manager.d.ts:41-48` specifies owner routing and dead-letter behavior. `cp-z42vu` remains **ADAPT** until recipient-level runtime evidence exists.

### 11.6 Adjacent mux observation

The mux artifact records six workers and 18/18 correct socket probes returning `pong`; this is a durable adjacent observation, not a ranking and not evidence that the OMP stdin `muxPing` endpoint was correct. It closes no Gap 1–9 claim.

---

## 12. Dispatchable prior-art runbook contract

**Trigger**: a named local gap needs upstream precedent, or a citation/absence must be re-derived after source or package drift.

**Dispatch packet**: `{gap_id, local_gap, exact_search, source_root, source_revision, synonym_set, expected_artifact, no_claim_scope}`. It names mirror versus installed OMP declarations and never substitutes an opaque pane/run ID for an artifact.

**Amazing**: an evaluator can replay the command against the pinned or explicitly unavailable revision identity, inspect a content-addressed output, identify each named construct and quote, distinguish declaration from runtime evidence, and trace the verdict to a Gap ID.

**Adequate**: command, root, revision identity or unavailable marker, output artifact/hash, construct/path, observed result, verdict, and local NO-CLAIM are present; missing runtime capture leaves the result DECLARED ONLY or PROJECTED.

**Negative patterns**: wrong-language extension filters; empty `--include` at exit 0; a scope that cannot contain the answer; pipeline status hiding producer failure; opaque run IDs; bare line citations; declaration-to-runtime overclaim; recipient-delivery overclaim.

**Skills**: `upstream-doctrine-mining` for mirror searches; `research-software` for installed OMP declaration/runtime probing; `verification` for end-to-end capture. This section records their contract but does not claim their execution.

**Done signal**: write an immutable result artifact containing `{gap_id, command, input, source_root, source_revision, result, verdict, no_claim}`; record its SHA-256; exit `0` only when the artifact exists and verdict is `ADOPT`, `ADAPT`, or `REJECT`, and exit non-zero on missing artifact, unreadable source, or unscoped negative.

**Out of scope for this section:** implementing adopted mechanisms, running build/gate verification, and reconciling `NUMBERS.toml` are implementation/orchestrator tasks; this section records prior art and evidence boundaries only.
