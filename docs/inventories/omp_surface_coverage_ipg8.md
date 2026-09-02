# OMP surface coverage — wave `ipg.8` (EDIT): edit, lsp, commit, compress, cleanse, markit

Bead: `omp-orchestrator-omp-coverage-mission-ipg.8` — Wave EDIT: edit, lsp, commit, compress, cleanse, markit

## Purpose

Classifies the six OMP **EDIT** surfaces. This is one of only two waves whose rows reached the machine ledger, so it is the wave where prose and `OMP-COVERAGE-TABLE.jsonl` can actually be diffed.

It owns the CLASSIFICATION of these 6 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix H (lines 1236–1299 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.8**: *Wave EDIT. We spawn git 4 times directly and have no LSP integration in any crate.
> Measured commit defects this wave: a double-quoted `-m` EXECUTES backticks (silent, exit 0),
> and a bare commit swept 8 files including a 678-line crate into a probe commit.*

**Swept 2026-09-01.** Six type roots, 118 files, 548KB, 645 exported symbols, walked to symbol
level. All six are agent-plane editing/IDE/commit/compression features. None crosses the process
boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `edit` | 28 | 132 | 153 | — — — — — — — — | **(a) NOT OURS** — agent file-editing machinery (RepairRegion, AppliedEditSnapshot, file-snapshot-store, blackbox edit observation) |
| `lsp` | 24 | 124 | 225 | — — — — — — — — | **(a) NOT OURS** — full LSP client (setSharedLspEnabled, isIdleClient, applyWorkspaceEditWithLsp, supportsDocumentDiagnostics, isRustAnalyzerClient, shutdownStaleClients) |
| `commit` | 40 | 200 | 172 | — — — — — — — — | **(a) NOT OURS** — commit pipeline (CommitInference, conventional/validation, agentic, changelog, pipeline) — overlaps our commit gates but approaches from the authoring side |
| `compress` | 4 | 16 | 14 | — — — — — — — — | **(a) NOT OURS** — context compression (resolveCompressTargets, runCompressCommand) |
| `cleanse` | 8 | 32 | 40 | — — — — — — — — | **(a) NOT OURS** — session hygiene (CleanseAgentHooks, CleanseAgentRuntime) |
| `markit` | 7 | 32 | 10 | — — — — — — — — | **(a) NOT OURS** — document format conversion (Markit, DocxConverter, EpubConverter, PdfConverter, PptxConverter) |

**Positive control: FAILED — 0 of 6 FULLY COVERED.** Seventh consecutive wave. The pattern is
exhaustive: every OMP type root is either orchestration-plane or agent-plane, and the mapping has
covered every root in both planes. The boundary is correct and the mapping is complete.

**Anti-vacuity: PASSED** — 6 surfaces enumerated, 118 files walked to symbol level, 0 is not the
count.

#### The `commit` surface, and why it is the most interesting (a)

`commit` is 40 files/200KB/172 symbols — the largest surface in this wave, and the one that
overlaps most directly with work we just built. It ships:
- `CommitInference` — AI-powered commit-message inference (analysis/summary/map/fast roles)
- `conventional/validation.d.ts` — conventional-commit validation with `ValidationSeverity`
  ("error" | "warning") and `ValidationIssue`
- `pipeline.d.ts` — a commit pipeline
- `changelog/` — changelog generation
- `git/` — git integration

We built commit-msg round-trip gates (refusing `-m` with backticks), pre-delete-citation-check,
and a canonical commit-message standard. OMP's commit surface approaches the same problem from
the AUTHORING side (AI infers the message) while we approach from the VALIDATION side (gates
refuse bad messages). The two are complementary, not competing — but we never evaluated whether
OMP's `conventional/validation` subsumes our commit-msg gate's checks. That evaluation is a gap,
recorded rather than resolved.

The measured commit defects this wave (double-quoted `-m` executing backticks, bare commit
sweeping 8 files) would be unconstructible if OMP's commit pipeline were the only commit path —
but adopting it would bypass our pre-commit gates (no-shell-gate, commit-msg round-trip,
path-literal-guard), which are the enforcement layer those defects spawned. The correct
architecture is: the agent AUTHORS the message, our gates VALIDATE it. OMP's inference feeds our
gates; neither replaces the other.

#### Why all six are (a)

`edit` is the agent's file-editing machinery (RepairRegion, AppliedEditSnapshot, blackbox
observation, file-snapshot-store — undo/repair capability). `lsp` is a complete Language Server
Protocol client (rust-analyzer client detection, document diagnostics, workspace edits, stale
client shutdown). `compress` and `cleanse` are agent-session hygiene. `markit` is document format
conversion. All six serve the agent's interactive experience — what the agent does inside the
pane, not what the orchestrator does outside it.

The orchestration-relevant OMP surfaces were mapped in wave 1 (session-adjacent output,
subprocess, jsonrpc, cli, commands, slash-commands — 7 consumes edges from omp-inventory-map).
Every root since then has been agent-plane. The mapping has converged.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.8`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=8 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
