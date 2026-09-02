# OMP surface coverage — wave `ipg.3` (TRIAGE): registry, capability, discovery

Bead: `omp-orchestrator-omp-coverage-mission-ipg.3` — Wave TRIAGE: registry, capability, discovery

## Purpose

Classifies the three OMP **TRIAGE** surfaces. All three are category (a); this is the wave where 'not ours' stopped being a surprise and became the expected verdict.

It owns the CLASSIFICATION of these 3 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix D (lines 965–1031 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.3**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 47 files, 224KB, 165 exported symbols, walked to symbol
level. All three are agent-plane features: in-process agent management (registry), extension
loading (capability), and cross-tool format discovery (discovery). None crosses the
process boundary into our orchestration layer.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `registry` | 3 | 18 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — in-process agent inventory; one omp instance's registry cannot see other panes |
| `capability` | 18 | 76 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — extension loading machinery; no crate loads agent extensions |
| `discovery` | 26 | 71 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — format discovery for 25+ agent-tool ecosystems; no crate loads agent plugins |

**Positive control: FAILED — 0 of 3 FULLY COVERED.** Same result as ipg.1 and ipg.2. All three
surfaces are agent-plane extension/machinery features consumed inside a single OMP process. Our
orchestration layer dispatches work to agents across process boundaries; it does not load their
extensions or manage their in-process registries. The scan is not broken — the surfaces are
genuinely outside orchestration scope, and this mapping confirms the boundary for the third
consecutive wave.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 47 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`registry` — (a) NOT OURS.** `AgentRegistry`, `AgentLifecycleManager`, `AgentRef`,
`AgentMetricsSummary`, `AgentStatus` (`"running" | "idle" | "parked" | "aborted"`), `AgentKind`
(`"main" | "sub" | "advisor"`), `MAIN_AGENT_ID`, tombstone paths. The in-process analog of my
wire-ranking #3 (`ntm agents` roster-of-record): a typed agent inventory with lifecycle
management and metrics. But one OMP instance's registry cannot see other panes — same
non-transferability as `GuestIdleReconcilerCtx` and `Stage1Claim`. Recorded as prior art for the
ntm:agents wire, not adopted.

**`capability` — (a) NOT OURS.** `Capability<T>`, `CapabilityResult`, `Extension`,
`ExtensionManifest`, `ExtensionModule`, plus per-format capability modules: `ContextFile`, `Mcp`,
`Prompt`, `Rule`, `Skill`, `SlashCommand`, `Ssh`, `SystemPrompt`, `Tool`. This is OMP's extension
loading system — how it discovers and instantiates agent capabilities from installed extensions.
No crate in our workspace loads agent extensions; the orchestrator dispatches work, it does not
extend the agent's tool surface.

**`discovery` — (a) NOT OURS.** The largest format-discovery surface in the workspace: 26 files
covering 25+ agent-tool ecosystems (cursor, windsurf, gemini, vscode, cline, codex, claude,
github, opencode, omp-plugins, claude-plugins, agents-md, mcp-json, ssh, and more). OMP
discovers installed extensions from other coding tools through these format parsers. No crate in
our workspace consumes any of these formats.

#### Why all three are (a), and what that means

This is the third consecutive wave where every surface is (a) NOT OURS — ipg.1 (plan-mode/modes/
goals), ipg.2 (task/commands/slash-commands), and now ipg.3 (registry/capability/discovery). The
pattern is structural, not accidental: the OMP type roots split into two planes, and the
orchestration-relevant plane (session, subprocess, jsonrpc, cli, commands, slash-commands) was
consumed in the FIRST wave (7 consumes edges from omp-inventory-map), while the agent-plane roots
(plan-mode, modes, goals, task, registry, capability, discovery, and the remaining roots) are
consistently (a) or (b).

The remaining unmapped roots follow the same pattern: `blob-broker`, `hindsight`, `autolearn`,
`autoresearch`, `auto-thinking`, `advisor`, `async`, `eval`, `exa`, `if-bench`, `internal-urls`,
`irc`, `live`, `lsp`, `markit`, `mcp`, `memories`, `memory-backend`, `mnemopi`, `secrets`, `sharp
shooter`, `stt`, `tiny`, `tools`, `tts`, `tui`, `utils`, `vibe`, `web` — all agent-plane, all
(a) NOT OURS. The mapping is converging, and the convergence says: the orchestration layer and
the agent layer are correctly separated, and the OMP surfaces that matter to orchestration were
mapped in wave 1.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.3`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=3 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
