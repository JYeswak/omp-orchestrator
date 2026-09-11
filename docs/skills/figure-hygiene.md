# figure-hygiene — every number carries its denominator

Triggers: "how many", "count", "measure", "cite", "figures", "prove", "survey",
"coverage", "anchor", "line number", "quote", "stale".

## Pattern
A figure is cited with (a) the producing command, (b) the population and tree
it was measured on, (c) a positive control proving the instrument can return
nonzero. A line citation uses content anchors (proven by OBSERVED RENDER, e.g.
`pandoc -f gfm -t html` read of the real `id=`), never a derived slug. A quote
is byte-exact with file+line-range, or it is paraphrase explicitly.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A transcribed value. Counts, paths, and states copied between beads, docs, and packets go stale within hours — three of three checked hard-count beads were stale, all in the direction that makes dead work look live, one nearly deleting a running crate. | The queue serves fiction; a pane spends its unit discovering the premise is gone. | Re-derive before dispatch/citation; carry measurement time and expiry; bind claims to something that re-derives the value. |
| A line-range citation. CONTRACT.md said BUILD FREEZE at :59 while the amendment lifting it sat at :101 — a reader who stops at FREEZE never reaches AUTHORIZED, and 131 authorized beads sat claimable for a session. | A range points at text that moved; the citation rots silently. | Cite by content anchor; a superseding amendment lands AT the original text, never downstream. |
| A derived anchor slug. Two markdown anchors derived by slug rules shipped broken (dropped underscore, missed triple hyphen) — caught only by rendering the real `id=`. | `yaml.safe_load` also silently accepts duplicate keys, so a bare load cannot disprove a duplicate-key claim either. | Render, don't derive; load with a strict duplicate-key loader. |
| Shell-pipeline attribution. `$?` after a pipe, `grep -c` occurrence counts, `M` without insertions, `�grep -c … || echo 0` emitting `"0\n0"` into a `usize` count — a family of instruments that each answered a different question than asked. | Every case produced a confident-wrong figure that survived until a second instrument disagreed. | The tell is two instruments disagreeing; run the pair deliberately (e.g. new-function count AND a known-present control). |
| Borrowed claims held as fact. A `dcg`-denied probe became an asserted negative, then house doctrine, across three agents in under an hour — each restating the last. | Social weight replaces verification; the retraction needs the same standard as the claim it replaces. | Cite it and verify it, or attribute it and mark it unverified. If you can state its file+line but haven't opened the file, you are transmitting, not verifying. |
| Absolute figures over moving populations. "81 methods, 17 used" retired as a pair; crate counts moved 27→50→51→65 inside one document's lifetime; a ratchet ceiling breached by exactly the number of crates ADDED. | Each stale integer was corrected by an agent who wrote a fresh integer that went stale in turn. | The correction is no number: derive from `cargo metadata`, name whole phase arcs once, express ratchets as ratios or per-unit assertions. |
| Fabricated or drifted citations. A verbatim quote with no byte-exact source; cites pointing at nonexistent files; seven-adapter claims no grep can find. | Defending a number's reading while never sourcing it is worse than no number. | Verbatim needs byte-exact provenance or it ships as paraphrase; load-bearing numbers get live derivations or explicit RETRACTs. |

## Negative evidence

| Row | Provenance |
|---|---|
| Stale hard counts n4q/i0uh/815 (3/3, one near-deletion) | AGENTS.md transcribed-value rule, 2026-09-08 |
| CONTRACT.md:59 FREEZE vs :101 AUTHORIZED (131 beads idle) | AGENTS.md authorization block |
| Derived-slug anchors wrong twice; safe_load accepts dup keys | AGENTS.md structural-tools + rule 6 (m0c) |
| Pipeline/`grep -c`/numstat occurrence family | AGENTS.md rules 8h/8n/8o; bead-mining C12 |
| Denied-probe-to-doctrine in under an hour | AGENTS.md borrowed-claim rule (LIFECYCLE-FAILURES) |
| Retired figure pairs; 14-crate-breached ceilings | AGENTS.md crate table; crate-atom-gate rule 10 |

## Check
```
# before citing any figure, re-run its producing command and paste output;
# before citing any anchor, render it: pandoc -f gfm -t html <file> | grep 'id='
```
