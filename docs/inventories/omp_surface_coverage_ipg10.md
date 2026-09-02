# OMP surface coverage — wave `ipg.10` (IO): web, exa, stt, tts, ssh, internal-urls, tools, cli

Bead: `omp-orchestrator-omp-coverage-mission-ipg.10` — Wave IO: web, exa, stt, tts, ssh, internal-urls, tools, cli

## Purpose

Classifies the eight OMP **IO** surfaces. Every row is category (a): a legitimate terminal state, recorded here so nobody re-argues it wave by wave.

It owns the CLASSIFICATION of these 8 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix J (lines 1387–1415 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.10**: *Wave IO. Eight agent-plane type roots — search providers, speech I/O, remote
> access, internal URI routing, the tool registry, and CLI argument parsing.*

**Swept 2026-09-01.** Eight type roots, 171 files, ~1,800 exported symbols, walked to symbol
level. All eight are agent-plane features. None crosses the process boundary.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `web` | 4 | 488 | 29 | — — — — — — — — | **(a) NOT OURS** — web-search provider types (KagiSearchRequest/Result, AnthropicProvider) |
| `exa` | 3 | 12 | 17 | — — — — — — — — | **(a) NOT OURS** — Exa search integration (ExaSearchResponse, findApiKey) |
| `stt` | 10 | 44 | 50 | — — — — — — — — | **(a) NOT OURS** — speech-to-text (STTController, EndpointerConfig, STT_MODELS) |
| `tts` | 12 | 52 | 54 | — — — — — — — — | **(a) NOT OURS** — text-to-speech (TtsDownloadProgress, KOKORO_VOICES) |
| `ssh` | 5 | 32 | 57 | — — — — — — — — | **(a) NOT OURS** — SSH config/host management for the agent (SSHHostConfig, RemoteFileRead/WriteOptions) |
| `internal-urls` | 22 | 100 | 68 | — — — — — — — — | **(a) NOT OURS** — internal URI scheme resolver (AgentProtocolHandler, ResolvedArtifactFile) |
| `tools` | 94 | 732 | 860 | — — — — — — — — | **(a) NOT OURS** — agent tool registry (shouldRouteWriteThroughBridge, ApprovalPolicy) — LARGEST by symbols in the workspace |
| `cli` | 51 | 352 | 361 | — — — — — — — — | **(a) NOT OURS** — CLI argument parsing (AgentsAction, ResolvedCliArgv) |

**Positive control: FAILED — 0 of 8 FULLY COVERED.** Ninth consecutive wave. The pattern is
exhaustive: every OMP type root is either orchestration-plane or agent-plane.

**Anti-vacuity: PASSED** — 8 surfaces enumerated, 171 files walked to symbol level.

**`tools`** is the largest by symbol count in the entire workspace (860 exported symbols, 94
files, 732KB). It is the agent's complete tool registry — every built-in tool the agent can
invoke, with approval policies, bridge routing, and activity snapshots. No crate in our workspace
imports any of these types.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.10`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=10 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
