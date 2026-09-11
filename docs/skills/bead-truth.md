# bead-truth — the tracker is the spec

Triggers: "dispatch", "claim bead", "close bead", "grade", "ACK", "release",
"assign", "in_progress", "reopen", "blocked", "acceptance".

## Pattern
```
file -> claim -> dispatch -> ACK -> observe -> verify -> RELEASE -> close
```
The dispatch path refuses a packet naming a bead that is not `in_progress` and
assigned to the receiver. One ACK per packet, token mechanical
(`ACK <last-hyphen-segment> on <pane> --`). A bead closes by a DIFFERENT pane
than the implementer, re-executing acceptance, reason starting
MUTATION-VERIFIED / DONE / APPROVED / WONTFIX — and the status is read back,
because nothing refuses a prose reason and the refusal would scroll past.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| Dispatching to an unclaimed bead. The bead sits `open`, assignee none, zero comments, while a worker carries it — silent in both directions, and the follow-up detector (keyed on assigned+in_progress+silence) watches an empty slot. | Dispatched-then-silent is indistinguishable from never-asked. | Claim first: `br update <id> --status in_progress --assignee 'pane=%N;agent=<name>'`, read it back. |
| Scope that lives only in the dispatch packet. An added NUMBERS.toml row existed nowhere but the packet; the bead closed without it and grep later proved it never landed — invisible to verification by construction, even when grader and author are the same agent. | Every acceptance check keys on the bead; packet-only scope cannot be checked. | Write added scope into the bead's acceptance BEFORE sending, then read it back. |
| Per-bead ACKs on a batch packet. Every ACK takes the tracker write lock; six ACKs across a 31 MB database serialised the fleet behind a delivery receipt, including a deadlock-grade wait on a peer's process. | Delivery receipts don't scale; the second ACK for the same packet raises no evidence tier. | Exactly one ACK token per packet (one real bead token from the batch). Verdicts stay per bead. |
| A nickname ACK token. The match is an exact prefix on the bead's last hyphen segment; `ACK s1ratify` on a bead whose token is `jplf.7.2` never matches, and 37 of 452 ACKs were unmatchable by construction. | The dispatch landed, the work happened, and the evidence is unusable — read as unproven transport. | Use the bare token of one real bead from the packet. |
| Grading your own bead. Two of the first three closes were self-certified; both carried real evidence, which is why this is a process gap and not fabrication — but a report is a CLAIM, and the whole point of the grade is that a second agent ran the command. | Self-grading collapses the only independent check the system has. | Different pane re-runs, cites what IT executed, closes. (Lineage may match; pane may not.) |
| Routing a grade to a pane while the author holds assignee. Two panes refused within a minute in near-identical words — both refusals correct, the routing bug the dispatcher's. | An implementation-complete bead assigned to its implementer is not grade-ready however the packet is worded. | The release beat: the dispatcher refuses to route a grade while assignee names another pane; reassignment IS the release. |
| An agent NAME as identity. One persona spans three panes; a lineage spans everything. Eligibility keyed on the name cannot tell author from grader. | `pane=` is the only unique field; everything else collides. | Claim as `pane=%N;agent=NAME`; check eligibility on the pane field. |
| Citing a stale acceptance count. Three of three checked beads cited hard counts that had all moved — every one stale in the direction that makes a bead look like live work, one nearly costing a running crate its deletion. | The queue serves it forever; a pane spends its unit discovering the premise is gone. | Re-derive any count/path/state before dispatch; amend acceptance FIRST. Verdicts: ALREADY-FIXED / PREMISE-FALSE / STILL-LIVE — never WONTFIX for a stale premise. |

## Negative evidence

| Row | Provenance |
|---|---|
| Unclaimed dispatch 5rh→%1413, zero comments | AGENTS.md fourth rule, 2026-08-31 |
| Dispatch-only NUMBERS.toml row, grep proved 0 | AGENTS.md, 2026-09-02 |
| ACK amplification deadlock (PID 15800, 1m17s lock wait) | AGENTS.md, 2026-09-07 |
| 37/452 unmatchable ACKs; `br list --json` omits comments | AGENTS.md; pre-delete-citation-check measurement |
| Self-closes -7ai/-a3p vs independent -4ak | AGENTS.md grading gate, 2026-08-31 |
| Three stale counts, all live-looking (n4q/i0uh/815) | AGENTS.md transcribed-value rule, 2026-09-08 |

## Check
```
br show <id> --json   # status + assignee + acceptance, read back after every transition
# note: br list EXCLUDES closed rows by default; br show returns a bare list
```
