# OMP surface coverage — wave `ipg.5` (OBSERVE): session, live, tui, sharpshooter

Bead: `omp-orchestrator-omp-coverage-mission-ipg.5` — Wave OBSERVE: session, live, tui, sharpshooter

## Purpose

Classifies the four OMP **OBSERVE** surfaces. `session` is the closest near-miss in the census: we scrape a braille spinner out of pane text to derive state that this surface types directly.

It owns the CLASSIFICATION of these 4 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix E (lines 1032–1105 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.5**: *each surface gets a coverage-table row with all 8 columns + classification
> (a) not ours / (b) reimplemented by scraping / (c) unused capability.*

**Swept 2026-09-01.** Four type roots, 102 files, 660KB, 598 exported symbols, walked to symbol
level. The `session` root is the largest in the workspace (78 files/395KB/499 symbols) and the
one where our scraping approach diverges most sharply from the vendor's typed event plane.

| surface | OMP files | OMP KB | OMP symbols | 1 asuper | 2 forbid | 3 cancel | 4 typed | 5 logged | 6 observable | 7 robot | 8 WIRED | classification |
|---|---:|---:|---:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|---|
| `session` | 78 | 564 | 499 | ✓¹ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓¹ | **(b) REIMPLEMENTED BY SCRAPING** — tick-monitor reads rendered pane text; OMP ships `AgentSessionEvents`, `SessionStopEvent` plus `SessionStopEventResult.continue`, `ArtifactManager`, and 78 files of typed session/event/artifact surface that we parse from screenshots |
| `live` | 6 | 24 | 29 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — Codex live voice/audio streaming (`LiveSessionController`, `LIVE_MODEL: "gpt-live-1-codex"`, `CodexLiveTransport`) |
| `tui` | 10 | 40 | 36 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — terminal UI rendering components (`renderCodeCell`, `renderMarkdownCell`, `FramedBlock`, `Hasher`) for the agent's interactive display |
| `sharpshooter` | 8 | 32 | 34 | — | — | — | — | — | — | — | — | **(a) NOT OURS** — agent memory-file curation (`SharpshooterDelta`, `ConsolidationResult`, `MemoryBackend`) |

¹ The ✓s on `session` are tick-monitor's clauses at the output plane: the crate is asupersync-
compatible (subprocess-contract for spawns), forbids unsafe, is cancel-correct (timeout is not a
verdict, `Outcome::TimedOut` distinct from `Completed`), typed (exhaustive `classify` match, no
wildcard arm), logged (machine-readable `why: &'static str`), observable (every field its own
predicate), robot-reachable (`--selftest`, 55 tests), and WIRED (omp-orchestrator consumes it).
The TYPE-plane coverage is zero: no crate imports any of the 78 session `.d.ts` files.

**Positive control: FAILED — 0 of 4 FULLY COVERED.** The `session` surface is the closest
(tick-monitor covers 7 of 8 clauses at the output plane), but the TYPE plane is zero: no crate
imports any of the 78 session `.d.ts` files. This is the fourth consecutive wave with an honest
positive-control failure, and the pattern is now confirmed: the OMP type roots split into an
orchestration plane (consumed in wave 1) and an agent plane (not adopted), and `session` is the
largest surface on the agent plane.

**Anti-vacuity: PASSED** — 4 surfaces enumerated, 102 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`session` — (b) REIMPLEMENTED BY SCRAPING.** The bead's own measured artifacts prove why the
scraping approach fails: two panes read `<no marker>` because a tool-call box border rendered
AFTER the status line (A1's measured defect, 05-actions L30-32); a stale spinner in scrollback
reported a dead pane BUSY forever (the whole-buffer-scan defect, fixed by `last_status_line`);
and the 75-second MIN_GAP_SECS floor discards positive liveness evidence below the threshold
(A1's open asymmetry). OMP ships typed alternatives for every one of these: `AgentSessionEvents`
for event-plane observation, `SessionStopEventResult.continue` for terminal-vs-continue (the
NewlyIdle/ConfirmedIdle distinction, WIRE-PROVEN), `ArtifactManager` for durable artifact
tracking, `checkpoint-entries.d.ts` for compaction recovery. The scraping approach works today
(7 of 8 clauses at the output plane) but it is the coupling cost this classification names: every
OMP rendering change is a potential tick-monitor defect, and the type plane would eliminate the
coupling.

**`live` — (a) NOT OURS.** `LiveSessionController`, `CodexLiveTransport`, `LIVE_MODEL:
"gpt-live-1-codex"`, `LiveTranscript`, `LiveSessionCallbacks`, `LiveContextChannel` — live
voice/audio streaming for the Codex model. Our orchestrator dispatches text to tmux panes; it
does not stream audio.

**`tui` — (a) NOT OURS.** `renderCodeCell`, `renderMarkdownCell`, `FramedBlockComponent`,
`FileEntry`/`FileListOptions`, `Hasher` — terminal UI rendering components for the agent's
interactive display. Our orchestrator reads pane output; it does not render the agent's UI.

**`sharpshooter` — (a) NOT OURS.** `releaseSharpshooterSession`, `sharpshooterBackend:
MemoryBackend`, `SharpshooterConsolidationResult`, `runSharpshooterConsolidation`,
`flushSharpshooterExtraction` — agent memory-file curation and consolidation. Our durable state
is the bead board + per-unit ledgers, not agent memory.

#### The session root, and why it matters most

The `session` surface is where the gap between scraping and adoption is widest. 78 files,
395KB, 499 exported symbols — the vendor has typed the entire session lifecycle: events, artifacts,
checkpoint entries, async job delivery, auth broker config, blob store, bash runner. Our
tick-monitor reconstructs the pane state from rendered text using stable-hash stripping and
braille-filtering, then classifies with an exhaustive match. It works — it is the one layer the
plan calls WORKS — but the information it extracts is a lossy projection of what the session root
types carry. The golden-frame test (reprobe wave, VALIDATE classification) would pin the
projection against the vendor's names; the type adoption would eliminate the projection entirely.
Neither has been built. Both are recorded here.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.5`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=5 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
