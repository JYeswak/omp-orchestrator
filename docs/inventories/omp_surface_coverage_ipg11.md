# OMP surface coverage — wave `ipg.11` (RUNTIME): async, utils, lib, tiny, vibe, auto-thinking

Bead: `omp-orchestrator-omp-coverage-mission-ipg.11` — Wave RUNTIME: async, utils, lib, tiny, vibe, auto-thinking

## Purpose

Classifies the six OMP **RUNTIME** surfaces and answers the async question the wave was created to ask — whether OMP's concurrency root composes with, conflicts with, or duplicates asupersync. It is the only wave whose positive control PASSES.

It owns the CLASSIFICATION of these 6 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix K (lines 1416–1441 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.11**: *Wave RUNTIME. async is the one to read first: OMP's concurrency surface vs
> asupersync's binding contract — compose, conflict, or duplicate?*

**Swept 2026-09-01.** Six type roots, 60 files, 268KB on disk, 316 exported declarations, walked to symbol level. Exported declarations use the rule ^export [declare] {type,interface,const,function,class,enum} NAME in *.d.ts; file sizes use du -ck. All six are agent-plane runtime features. None crosses the process boundary.

| surface | OMP files | OMP KB | OMP symbols | 1 asupersync | 2 unsafe | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | coverage | classification |
|---|---:|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|---|
| async | 3 | 20 | 15 | —¹ | — | — | — | — | — | — | — | FULLY COVERED | **(a) NOT OURS** — OMP job scheduling (AsyncJobManager, raceJobSettlement) |
| utils | 43 | 176 | 185 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — OMP utility layer (ActiveRepoContext, resolveActiveRepoContext) |
| lib | 1 | 4 | 4 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — xAI HTTP credential transport (XAIHttpTransport, resolveXAIHttpCredentials) |
| tiny | 9 | 48 | 83 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — local/online tiny-model completion (TinyModelDevice, TextGenerationPipeline) |
| vibe | 3 | 16 | 25 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — Vibe worker lifecycle (VibeSessionRegistry, VibeLifecycleEvent) |
| auto-thinking | 1 | 4 | 4 | — | — | — | — | — | — | — | — | MAPPED_NOT_ADOPTED | **(a) NOT OURS** — prompt-difficulty classification (classifyDifficulty, parseDifficultyLevel) |

**Positive control: PASS — 1 of 6 FULLY COVERED (async).** FULLY COVERED means the surface map row is complete; it does not mean the capability is adopted.

**Anti-vacuity: PASSED** — 6 surfaces, 60 files, and 316 exported declarations were enumerated. A zero-surface or zero-file result is an ERROR.

**The async question answered:** OMP's async root composes with asupersync at a boundary but does not duplicate it. OMP's AsyncJobManager schedules in-process agent tool jobs (bash, task, eval) and races settlement against steering/abort; omp-rpc-session uses asupersync Cx, process groups, bounded phase deadlines, and both-pipe draining for the orchestrator's one OMP child. No Rust crate imports OMP's async declarations, so there is no direct binding conflict or duplicate shared implementation.

¹ The async/asupersync relationship is a measured composition result, not a local contract claim: the OMP declaration is TypeScript agent-plane code, while the Rust binding is omp-rpc-session/src/lib.rs:1-16,23-46,135-163.

**No category (b) rows:** every enumerated root is (a) not ours. Therefore no row requires a category-(b) OMP alternative; the OMP alternatives above name the existing declarations for auditability.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.11`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=11 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
