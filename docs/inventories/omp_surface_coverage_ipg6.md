# OMP surface coverage — wave `ipg.6` (VERIFY): eval, if-bench, hindsight, debug, dap, autoresearch, autolearn, advisor

Bead: `omp-orchestrator-omp-coverage-mission-ipg.6` — Wave VERIFY: eval, if-bench, hindsight, debug, dap, autoresearch, autolearn, advisor

## Purpose

Classifies the eight OMP **VERIFY** surfaces — the debugger, benchmark and eval roots we substitute with print statements. It also carries the resolution of the symbol-count BLOCKER, whose three disagreeing values are the reason `NUMBERS.toml` now owns a counting rule.

It owns the CLASSIFICATION of these 8 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix F (lines 1106–1162 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.6**: *Wave VERIFY. Skill /brennerbot-with-ntm — a session is a machine for deleting
> hypothesis space cheaply. Prefer refuters over supporters; no falsifier means no session.*

**IPG.6 COUNT BOUNDARY.** The registered NUMBERS.toml command is authoritative for the aggregate 621-symbol total. The per-root table immediately below is a historical/incomplete breakdown whose listed rows sum to 503; it is not used as a decomposition of the current aggregate until re-generated from the same counting rule.
**Swept 2026-09-01.** Eight type roots, 66 files, 488KB, **621 exported symbols** by the counting
rule now declared in `NUMBERS.toml` as `ipg6_root_symbols` (top-level
`export [declare] {type,interface,const,function,class,enum} NAME` in `*.d.ts`), walked to symbol
level. All eight are agent-plane quality-improvement or debugging features: eval kernels,
instruction-following benchmarks, memory retrieval, debug UIs, a full DAP client, self-improvement
research, self-learning, and an advisory review panel. None crosses the process boundary into our
orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `eval` | 17 | 216 | 95 | — — — — — — — — | **(a) NOT OURS** — kernel-session eval system: agent bridges, budget/completion/concurrency bridges, runner cache, runtime env, probe |
| `if-bench` | 5 | 20 | 30 | — — — — — — — — | **(a) NOT OURS** — instruction-following benchmark (glyph array actions, cat-sound directives) |
| `hindsight` | 9 | 52 | 81 | — — — — — — — — | **(a) NOT OURS** — memory retrieval (MentalModels, RecallTagsMatch, BankScope, HindsightApi, Budget "low"/"mid"/"high") |
| `debug` | 11 | 44 | 55 | — — — — — — — — | **(a) NOT OURS** — agent debug UI (DebugSelectorComponent, OverlayPanel, formatDebugLogLine) |
| `dap` | 5 | 40 | 93 | — — — — — — — — | **(a) NOT OURS** — full DAP client (DapClient, waitForTcpServerListening, DapAdapterConfig, resolveAdapter, LaunchAdapterSelection); a typed debugger we reimplement with print statements |
| `autoresearch` | 7 | 52 | 83 | — — — — — — — — | **(a) NOT OURS** — self-improvement research loop (DashboardController, AutoresearchRuntime, EnsureAutoresearchBranch) |
| `autolearn` | 2 | 8 | 13 | — — — — — — — — | **(a) NOT OURS** — agent self-learning (AutoLearnController, buildAutoLearnInstructions) |
| `advisor` | 10 | 56 | 53 | — — — — — — — — | **(a) NOT OURS** — advisory review panel (AdviseParams, AdvisorSeverity "nit"/"concern"/"blocker", AdviseDetails) |

**Positive control: FAILED — 0 of 8 FULLY COVERED.** Fifth consecutive wave. The pattern is now
exhaustive: every OMP type root splits into orchestration-plane (consumed in wave 1: session-
adjacent output, subprocess, jsonrpc, cli, commands, slash-commands — 7 consumes edges) and
agent-plane (not adopted). Eight more agent-plane surfaces confirmed.

**Anti-vacuity: PASSED** — 8 surfaces enumerated, 66 files walked to symbol level, 0 is not the
count.

#### The two surfaces worth naming

**`dap`** is a full Debug Adapter Protocol client — `DapClient`, `waitForTcpServerListening`,
`connectSocket`, `getAdapterConfigs`, `resolveAdapter`, `getAvailableAdapters`,
`LaunchAdapterSelection` — and the bead's own briefing names it: *"a typed debugger surface we
reimplement with print statements."* When a dispatch goes wrong tonight, the forensic trail is
`println!` and scrollback. The DAP client exists in the tool we wrap, DECLARED only. Adoption
would be a debugging-infrastructure decision, not an orchestration change.

**`advisor`** has `AdvisorSeverity: "nit" | "concern" | "blocker"` — a typed severity taxonomy
that directly parallels our convergence-lens severity tags (BLOCKER/MAJOR/MINOR). The prior art
is the same shape: a reviewer classifying findings by severity so downstream work can prioritize.
The vocabulary is one `use` away; the gap is that neither surface is consumed by a crate.

#### Why all eight are (a), and the convergence is complete

Five consecutive waves (ipg.1 through ipg.5, plus this ipg.6) have mapped 20+ OMP type roots and
every one outside the original 7-consumes-edge set is (a) NOT OURS. The pattern is structural:
the OMP type roots split into an orchestration plane (session-adjacent output, subprocess,
jsonrpc, cli, commands, slash-commands — consumed by omp-inventory-map and omp-rpc-session) and
an agent plane (eval, benchmarks, memory, debug, DAP, self-improvement, advisory — consumed by
the agent inside the pane, not by the orchestrator outside it). The mapping has converged: the
boundary is correct, and the remaining roots confirm it rather than challenge it.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.6`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=6 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
