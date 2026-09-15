# Plan — Materialize the A–Z Kernel Lifecycle

Status: design and staged K0–K14 scope approved for implementation; scope decision recorded 2026-09-02.
Owner: GreenFrog (pane 2, `%1413`); canonical journey files are exclusively reserved.

## Objective

Turn the existing S1–S9 journey into an executable, evidence-carrying A–Z lifecycle. Add the native Agent Mail lane, conversation-to-skill mining, signed identity, durable inbox monitoring, and byte-exact event mapping without collapsing transport, receiver, tracker, or human authorities.

## Files

1. `docs/plan/12-journey.md` — add the A–Z index, K0–K14 proposal, native AM lane, exact event envelope, and stage-to-kernel matrix.
2. `docs/plan/11-lifecycle.md` — add current-versus-target evidence, archive-mining results, research citations, and explicit unresolved edges.
3. `docs/plans/plan_to_materialize_a_to_z_kernel_journey.md` — this implementation sequence and scope gate.

## Ordered work

1. Preserve the existing S1–S9 contract and state that A–Z is its operational expansion, not a replacement.
2. Add K0–K14 as proposed kernel lanes; each lane names its owning crate/adapter, input bytes, output bytes, authority, event, and done oracle.
3. Define the canonical event envelope: field-ordered UTF-8 JSONL, explicit schema/version, raw input/output SHA-256, authority, stage, actor identity, session, pane index and pane ID, bead, attempt, cursors, outcome, error, previous hash, and event hash.
4. Define native Agent Mail operations: identity registration/readback, project namespace, reservation lifecycle, signed send/reply, delivery cursor, inbox `read_ts`, acknowledgement, queue/failure status, timeline/thread retrieval, and conversation-to-skill candidate extraction.
5. Encode silent-success refusal: compare every settable request field with the response; omitted fields are RED. Resolve `binding` and reject `legacy-unverified`; use target-qualified tmux identity.
6. Encode monitoring as two surfaces: durable `am inbox-events --after <cursor>` plus daemon-backed `am inbox` read-state reconciliation. Require an explicit shorter caller timeout for dispatch latency and cursor preservation: the documented NTM mail wake deadline is 300 seconds; caller ceilings below 300 observe their own signal as `CANCELED` without cursor info, while a caller-owned timeout returns `TIMEOUT` with resumable cursor info; `robot-attention` is the working live wake.
7. Add the archive evidence and arXiv citations with narrow claims and explicit non-claims.
8. Joshua approved staged implementation: Wave A K0/K5/K6/K7/K8/K9; Wave B K2/K3/K4/K10/K11/K12/K13; Wave C K1/K14. Create beads dependency-ordered, not as one fifteen-lane blob.

## Acceptance

- Every A–Z row has one kernel owner, one typed event, one authority, one refusal, and one observable done signal.
- No row claims that sender success implies receiver delivery, comprehension, tracker acknowledgement, completion, or shipment.
- The event-byte recipe is deterministic and hashes exactly the emitted bytes.
- Identity rows include `FROM` and `REPLY VIA`, live pane index and pane ID, and read-back binding status.
- `am robot search` is a source-level alias defect (`m.topic` is present in the schema but the query builder aliases it incorrectly); externally-signalled NTM cancellation discards cursor info while the internal deadline returns it. `am inbox-events` returning `inbox_events_unavailable` is correct fail-closed behavior and is excluded from the silent-success census.
- Authenticated Agent Mail daemon/MCP HTTP is primary; Homebrew `am 0.3.31` CLI storage access is a differential oracle. Delivery cursors are global monotonic positions with recipient-scoped sparse tails and oldest positions; pair cursor plus recipient and refuse ambiguous continuity rather than claiming eviction.
- Every Agent Mail defect is dispositioned as source fix/upstream, typed defense in `agent-mail-native`, or a named finding with a reproduction command. Pipe-based exit-code audits are forbidden because `$?` otherwise reports the pipeline.
- Archive counts and arXiv citations are source-linked; no paper is treated as proof of OMP invariants.
- Existing S1–S9 references remain valid and the dirty shared checkout is not swept by this work.

## Verification

Use document citation checks, `git diff --check` on owned files, `lsp diagnostics` only if Rust changes are introduced, and the existing repo gates after shared edits settle. Verify caller wiring separately; documentation is not implementation proof.

## JSM skill-library overlay

JSM discovery was authenticated and online: 135 local skills, 85 saved skills, a 47-day-old sync, 47 active bandit arms, 5,473 feedback events, and zero evidence records. Exact-term searches found the relevant installed skills; long natural-language searches returned empty sets, and some facet queries exposed malformed null numeric metadata. Search results require exact-term retries and result-shape checks.

Selected depth: `agent-orchestration` adds dependency-aware fan-out/fan-in and completion tracking; `agent-mail` adds reserve-before-edit, identity, threads, and ACKs; `agent-monitoring` adds layered health/trajectory/SLO; `agent-lifecycle` adds reversible version/rollback/retirement; `agent-memory`, `operationalizing-expertise`, and `self-improving-agent` add provenance-backed capture, join keys, falsifiers, and reviewed skill promotion; `accretive-cron-orchestration` and `loop-enforcement` add SWEEP/AUDIT/LEARN, typed tick modes, and escalation; `human-in-the-loop` adds risk-proportional approval and demotion; `rust-core-thin-frontend-workspace` adds core/harness/thin adapter separation; `testing-conformance-harnesses` and `oracle-gates` add MUST matrices, external truth, and fail-closed missing-oracle behavior; `condition-based-waiting` adds fresh reads and explicit ceilings; `resource-exhaustion-hunting` adds bounded falsifiers.

Each project receives a skill manifest containing project ID, query, skill/version/content hash, trigger, refusal, source message IDs, owner, and held-out result. JSM discovery never auto-installs or auto-promotes. The remote/private `agent-mail-patterns` result is not installed here and is not treated as loaded authority.
