# wiring-proof — built is not wired

Triggers: "wire", "is it used", "dead code", "unwired", "who calls",
"orphan", "zero callers", "allowlist", "trigger", "reachable".

## Pattern
Every mechanism ships a wiring-proof leg stated as commands, with a positive
control (grep something KNOWN-wired and confirm it hits — a zero from a pattern
that can never match is not evidence of absence):
```
grep -rl "<crate>"  crates/*/Cargo.toml | grep -v "crates/<crate>/Cargo.toml" | wc -l   # nonzero
grep -rl "<crate_underscored>" crates/*/src | grep -v "crates/<crate>/src" | wc -l       # nonzero
```
Then walk the caller chain to its ROOT: when a crate in the chain has no
callers, it MUST have a reachable trigger (workflow entry, hook, crontab,
launchd row, or a spawn from a triggered crate). A chain ending in a crate with
neither is UNWIRED, however many manifest edges were added along the way.
For callers/reachability/dead-code questions, use the structural tool
(`ripwire --callers/--uses`, `cargo metadata`), never a text count.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A wiring proof terminating at a fresh consumer. A new crate calling the unwired one passes the leg while moving the uncalled-ness up one level — a shell game each individual grep cannot see. | Two crates are now unwired instead of one, with a passing leg certifying it. | Part 2 of the leg: walk to the ROOT trigger; a chain ending triggerless is UNWIRED. |
| Counting mentions as invocations. 24 of 36 "triggered" bin crates had only `.flywheel/` prose mentions; zero executors. A document naming a binary does not run it. | Wrong by a factor of 24, in the reassuring direction — the direction that stops people looking. | Separate executors (workflow/hook/cron/launchd/spawn) from prose before counting; strip comments from every surface. |
| A text count as a topology answer. `grep -c` reported the flagship binary wired in 62 places; the true count was 0 — every hit an `omp-orchestrator` substring. A shell census reported 1 workspace leaf where the resolver reports 33. | The noun attached to the count was wrong; a counter that grows with healthy activity is red when the fleet is most productive. | Derive topology from `cargo metadata`, never grep; use word-boundary or structural queries; express ratchets per-crate or as ratios, never workspace-wide absolutes. |
| An ownership claim over a green reservation. A `conflict_free` lease answers "may I edit now"; the lane map answers "whose lane is this". Treating one as the other misattributes cross-lane work. | The reservation governs the edit, the map governs the assignment; neither substitutes. | Disclose the crossing; the lane owner keeps the follow-on. |
| Deleting to satisfy a gate. Removing both named rows silenced one assertion and moved another crate into a different gate's violation set — a remedy the gate prescribes by name can still be wrong. | The gate that prescribes it is not the only gate reading the same list. | Re-run the whole target after taking a test's advice, not just the test that gave it. |
| A second comment grammar beside the kernel's. Every local comment-stripper is a comment grammar that drifts; five hand matchers coexisted with the kernel while the lint named five literals and missed the gate's own. | The local copy defeats the mitigation it documents. | Route through `text_structure::code_only`; the lint fires on the verb+noun SHAPE, not a name list. |

## Negative evidence

| Row | Provenance |
|---|---|
| blocker-taxonomy 662 LOC, 19 green tests, zero callers (N043 on ourselves) | AGENTS.md rules 2+9, 2026-09-06 |
| m2-grading-lane moved uncalled-ness up one level | AGENTS.md rule 9, ~1h after it landed |
| 24-of-36 executor-vs-prose miscount | AGENTS.md rule 9 retraction, 2026-09-07 |
| `grep -c ompo` 62 vs true 0 (substring) | AGENTS.md structural-tools, four measured failures |
| Reservation-vs-ownership arbitration (%1414) | AGENTS.md arbitrations, 2026-09-02 |
| Hand-matcher family + lint miss (9ub39) | bead 9ub39; kernel-bypass-gate history |

## Check
```
cargo metadata --no-deps --format-version 1 --offline | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))'
# must agree with: find crates -mindepth 1 -maxdepth 1 -type d | wc -l
```
