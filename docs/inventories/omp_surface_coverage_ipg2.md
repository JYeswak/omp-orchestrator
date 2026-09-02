# OMP surface coverage — wave `ipg.2` (BEADS): task, commands, slash-commands

Bead: `omp-orchestrator-omp-coverage-mission-ipg.2` — Wave BEADS: task, commands, slash-commands

## Purpose

Classifies the three OMP **BEADS** surfaces. Two are category (b) — we consume their rendered output rather than their types — which is the coupling this document exists to name.

It owns the CLASSIFICATION of these 3 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix C (lines 893–964 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.2**: *each surface gets a row in the coverage table with all 8 columns and a classification —
> (a) not ours, (b) reimplemented by scraping, (c) unused capability.*

**Swept 2026-09-01.** Three type roots, 82 files total, walked to export level. The per-crate
contract's eight clauses are assessed against our crates.

| surface | OMP files | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `task` | 27 | ~200 | — | — | — | — | — | — | — | — | **(b) REIMPLEMENTED BY SCRAPING** — the agent's entire subagent lifecycle (spawn, parallel, worktree, structured output, yield) consumed as pane text by tick-monitor |
| `commands` | 42 | ~120 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | **(a) NOT OURS** — 39 agent CLI subcommands for human users; we probe `--version` and `--help` only |
| `slash-commands` | 13 | ~80 | ✓² | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓² | **(b) REIMPLEMENTED BY SCRAPING** — census `slash_commands=0` vs `expected=136`: we try to scrape them over RPC and get zero; the scanner consumes the type root but enumerates nothing |

¹ The ✓s on `commands` and `slash-commands` are omp-inventory-map's census clauses: the scanner
consumes the type root, parses the installed cli.js, and emits typed rows. They are NOT a
modes-adopting crate's clauses — the census observes, it does not adopt.
² `slash-commands` is consumed by the census (type_root:slash-commands is one of the 7 consumes
edges) but the census probe returns zero slash commands (the `slash_commands=0`/`expected=136`
mismatch), so the coverage is scanner-level only: the type root is touched, the commands are not
enumerated.

**Positive control: FAILED — 0 of 3 surfaces is FULLY COVERED.** Same result as ipg.1, same reason:
all three are agent-plane features. `task` is the agent's subagent lifecycle; `commands` are the
agent's CLI verbs for human users; `slash-commands` are the agent's interactive-session shortcuts.
Our orchestration layer dispatches work to agents, it does not BE the agent. The scan is not broken
— the surfaces are genuinely outside the orchestration scope, and mapping them confirms the
boundary rather than expanding it.

**Anti-vacuity: PASSED** — 3 surfaces enumerated, 82 files walked to export level, 0 is not the
count.

#### Per-surface detail

**`task` — (b) REIMPLEMENTED BY SCRAPING.** 27 files covering the agent's ENTIRE subagent
lifecycle: `AgentDefinition` parsing, `StructuredSubagent` with schema modes ("permissive" |
"strict"), `mapWithConcurrencyLimit` (parallel execution), `WorktreeBaseline`/`RepoBaseline`
(worktree isolation), `ResolvedSpawnPolicy`, `PromptPolicy`, `YieldItem`/`assembleYieldResult`
(yield assembly), `SubprocessToolRegistry`, `OutputManager`, `ErrorAttribution`, `PersistedRevive`,
and `PreWalk`. We interact with agents through tmux panes (screen-scraping), consuming the rendered
output without adopting any of these types. The `parallel.d.ts` concurrency primitive
(`mapWithConcurrencyLimit`) is a generic utility our subprocess-contract could use, but adopting
a TypeScript concurrency function into a Rust crate is not a type-adoption — it is a
reimplementation decision.

**`commands` — (a) NOT OURS.** 42 CLI subcommand classes for human users interacting with the
agent: `acp`, `agents`, `auth-broker`, `auth-gateway`, `bench`, `browser-relay`, `cleanse`,
`commit`, `complete`, `completions`, `compress`, `config`, `dry-balance`, `gallery`, `gc`, `git`,
`grep`, `grievances`, `if-bench`, `images`, `install`, `join`, `models`, `plugin`, `ps`, `read`,
`render`, `say`, `search`, `setup`, `share`, `shell`, `ssh`, `stats`, `tiny-models`, `token`,
`ttsr`, `update`, `usage`, `web-search`, `worktree`. Our orchestrator probes `--version` and
`--help` for census and identity purposes; it does not consume the command classes.

**`slash-commands` — (b) REIMPLEMENTED BY SCRAPING.** 13 files of built-in slash-command
definitions (ACP builtins, collaboration, completions, control, lifecycle, marketplace, modes,
registry, session). The census consumes this type root and probes the RPC startup stream for
slash commands, finding **zero** against `expected_slash_commands=136`. The 136-command gap is the
largest unmapped OMP surface and this scanner-level gap is why the type root is consumed but the
commands are not enumerated.

#### What would Jeffrey do

`task/parallel.d.ts`'s `mapWithConcurrencyLimit` is the surface that crosses the agent/orchestration
boundary most cleanly — it is a generic concurrency primitive that does not know about coding
agents. If our subprocess-contract grew a TypeScript-bridged concurrency adapter, it would use this
shape. But adopting a TypeScript function into a Rust crate is a reimplementation decision, not a
type adoption, and the bridge cost exceeds the benefit when `rayon` or `tokio::spawn` already
provide the same primitive in Rust.

NO-CLAIM: mapping is not adopting. The coverage table records what exists; the adoption decision
is §09's.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.2`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=2 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
