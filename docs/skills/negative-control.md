# negative-control — validate the instrument before believing it

Triggers: "diagnose", "measure", "survey says", "probe shows", "is this wired",
"absence of", "detector says", "census", "scan results", "no evidence of".

## Pattern
Point the instrument at a guaranteed-absent subject first. If ABSENT and
PRESENT produce the same output, the instrument cannot answer the question and
its verdict on the real subject means nothing:
```
<tool> --root /nonexistent/path/xyz ; echo "rc=$?"   # what does ABSENT look like?
<tool> --root . ; echo "rc=$?"                         # now the real one
```
Indistinguishable → fix the instrument before reporting anything. A denied,
errored, empty, or unreadable probe is UNKNOWN, never a negative result.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A structural miss is reported as absence. `ripwire --uses` returned 0 for a symbol with two live call sites because they sat inside `assert!()` macro arguments, invisible to the tool. A zero from a macro-blind tool was read as "no callers". | The checker's blind spot becomes the finding. Absence was never measured. | Pair the structural tool with its prescribed follow-up (`--grep`); a structural zero without one is UNKNOWN. |
| `fh suggest` rows are cited as hits. `suggest` never returns empty, so every row is a candidate; a STALE index returns the same banner for both verbs. | Following a candidate as a hit builds on unconfirmed retrieval. | `fh search` is the negative control (`[EMPTY]` on true miss); confirm freshness with `fh health` first. |
| A single-oracle conclusion ships. Concluding "gate on observation_state, never state" from one 5-pane snapshot died 6 minutes after landing when a second surface read idle@0.95 on a WORKING pane. | One oracle cannot refute; agreement between correlated oracles is one probe wearing two names. | Two differently-shaped readers; refuters over supporters; forced third alternative. |
| Lenient parsing certifies what strict execution rejects. A duplicate-key workflow starts ZERO jobs while `yaml.safe_load` accepts it silently; a missing job key halved coverage with no error anywhere. | The validator and the executor disagree, and the validator is the one being asked. | Parse with the strict loader (duplicate keys rejected); keep a lenient reader only as the documented-wrong control. |
| Unmeasurable is reported as a property. "fsync issuance UNMEASURABLE on this lane" was order-only proof until a grader found strace on the Linux root lane and re-measured exit=0. | A lane limit was published as a subject property and would have gated decisions. | Name the lane with the limit; a NO-CLAIM must carry where it was measured. |

## Negative evidence

| Row | Provenance |
|---|---|
| 8i parent rule + 8b–8h instances, 8j–8o tool aims | AGENTS.md gate rules; INSTRUMENT-DEFECTS.md |
| ripwire macro-argument blind spot (8k) | AGENTS.md structural-tools section, 2026-09-07 |
| fh suggest/search staleness (8j) | AGENTS.md, measured 2026-09-07 |
| observation_state retraction (609a97b) | bead comment, 6 min after landing |
| gate.yml 60-run separation (11 strict nonzero / 49 dup zero) | bead m0c#c2491 |
| strace re-measurement (q7jz#c3510) | bead comment, grader re-trace |

## Check
```
command -v <tool> && <tool> --help 2>&1 | head -n 5
# then the two-probe form above; paste both outputs with the verdict
```
