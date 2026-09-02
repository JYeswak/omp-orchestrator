# OMP surface coverage — wave `ipg.7` (MEMORY): memories, memory-backend, mnemopi, blob-broker, export

Bead: `omp-orchestrator-omp-coverage-mission-ipg.7` — Wave MEMORY: memories, memory-backend, mnemopi, blob-broker, export

## Purpose

Classifies the five OMP **MEMORY** surfaces. Cross-session state is how a swarm survives compaction; this wave measures what OMP already types against what we carry in bead comments and scrollback.

It owns the CLASSIFICATION of these 5 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix G (lines 1163–1235 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.7**: *Wave MEMORY. Cross-session state is how a swarm survives compaction. We currently
> carry it in bead comments and pane scrollback — scrollback dies with the pane.*

**Swept 2026-09-01.** Five type roots, 46 files, 284KB, 243 exported symbols, walked to symbol
level. All five are agent-plane memory/export features: memory instruction pipelines, pluggable
memory backends, mnemonic embedding engines, blob storage brokers, and session sharing. None
crosses the process boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `memories` | 2 | 8 | 26 | — — — — — — — — | **(a) NOT OURS** — memory-instruction pipeline (Stage1Claim, MemoryThread, buildMemoryToolDeveloperInstructions) |
| `memory-backend` | 8 | 36 | 18 | — — — — — — — — | **(a) NOT OURS** — pluggable memory-backend interface (MemoryBackend, localBackend, re-exports MnemopiBackendConfig) |
| `mnemopi` | 7 | 36 | 42 | — — — — — — — — | **(a) NOT OURS** — mnemonic embedding engine (MnemopiEmbedClient, MnemopiBankScope, MnemopiEmbedWorkerHandle, resolveMemoryCompletionInput) |
| `blob-broker` | 26 | 180 | 141 | — — — — — — — — | **(a) NOT OURS** — blob storage/routing broker (BlobBackend, BlobDestinationId, ExposureKind, UploaderKind); largest surface in this wave |
| `export` | 3 | 24 | 16 | — — — — — — — — | **(a) NOT OURS** — session export/sharing (CustomShareResult, CustomShareFn, LoadedCustomShare) |

**Positive control: FAILED — 0 of 5 FULLY COVERED.** Sixth consecutive wave. The pattern is
exhaustive and structural: the OMP type roots split into an orchestration plane (consumed in
wave 1: 7 consumes edges from omp-inventory-map) and an agent plane (not adopted). The mapping
has converged: every remaining root is agent-plane, and the boundary is correct.

**Anti-vacuity: PASSED** — 5 surfaces enumerated, 46 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`memories` — (a) NOT OURS.** `Stage1Claim`, `MemoryThread`,
`buildMemoryToolDeveloperInstructions`, `startMemoryStartupTask` — the agent's memory-instruction
pipeline. The `Stage1Claim` name echoes the claims vocabulary we assessed in ipg.1 (non-
transferable to bead custody), and `MemoryThread` is agent-session memory threading, not
orchestration state.

**`memory-backend` — (a) NOT OURS.** `MemoryBackend`, `MemoryBackendSaveInput/Result/SearchItem/
Options`, `localBackend`, re-exports of `MnemopiBackendConfig` — the pluggable backend interface
that `mnemopi` and `sharpshooter` implement. The interface is well-designed (save/search/expire
operations over a pluggable store) but our durable state is the bead board + per-unit ledgers,
not an agent memory backend.

**`mnemopi` — (a) NOT OURS.** `MnemopiEmbedClient`, `MnemopiEmbedWorkerHandle`, `MnemopiBankScope`,
`MemoryCompletionInput`, `resolveMemoryCompletionInput` — an LLM-powered memory embedding engine
(embed workers, bank scoping, completion resolution). The embedding infrastructure is real but
the orchestrator does not embed memories.

**`blob-broker` — (a) NOT OURS.** 26 files, 180KB, 141 symbols — the largest surface in this
wave. `BlobBackend`, `BlobDestinationId`, `ExposureKind` (serve vs upload), `UploaderKind`, and
destination-specific modules. A blob storage/routing broker for agent session artifacts (screenshots,
exports, uploads). Our orchestrator writes bead comments and per-unit ledgers; it does not route
session blobs.

**`export` — (a) NOT OURS.** `CustomShareResult`, `CustomShareFn`, `LoadedCustomShare` — session
export/sharing via encrypted links and HTML rendering. The 08-end-users bead already assessed the
agent's share command as (a) NOT OURS.

#### Why all five are (a), and what the cross-session gap actually is

The bead's framing is correct: *"cross-session state is how a swarm survives compaction."* But
the OMP memory surfaces answer a different question than ours. OMP's memory backends store
*agent-session context* (what the agent was thinking, what files it read, what the user said) so
the agent can resume with context. Our cross-session state is *orchestration state* (which bead,
which pane, what receipt, what verdict, what decision) so the supervisor can resume without
re-briefing. These are different domains with different storage requirements.

The adequate substrate for our cross-session state already exists: the bead board (durable,
survives panes), the per-unit ledgers (typed, queryable), and the packet journal (append-only).
The gap is not storage — it is that the dispatch loop does not yet write per-unit ledgers (S9
UNKNOWN), and the decision ledger has **three manual rows** (S9 GAP: automated writer/consumer absent). Those are 12-journey S9's findings, and this mapping confirms them rather than replacing them.

The blob-broker is the one surface with potential orchestration relevance: if dispatch packets
grow beyond text (screenshots of pane state, recording artifacts), a blob broker becomes the
natural storage layer. But that is an S5 Cost-field decision, not this mapping's.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.7`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=7 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
