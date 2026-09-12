# Lifecycle failures — receipts archive

This file is a verbatim receipts archive extracted from `AGENTS.md`. It is NOT doctrine. Binding rules remain in `AGENTS.md`; its rulebook stubs should link here.

## The fourth rule: file → **claim** → dispatch, and never skip the middle beat

A packet naming an unclaimed bead is a dispatch that **the tracker never learned about**. It does
not appear as `in_progress`, `bv` cannot see it, no follow-up check can watch it, and when the pane
goes quiet nothing can distinguish *"the worker went silent"* from *"nobody was ever asked."*

**Measured 2026-08-31.** `5rh` was dispatched to `%1413` and never claimed. The bead sat `open`,
`assignee: none`, **zero comments**, while a worker was carrying it. The failure was silent in both
directions: the pane looked idle-with-no-assignment, and the queue looked like it still had ready
work nobody had taken.

**It also defeats the follow-up stage**, which is the part worth understanding. `classify_followup`
keys on *assigned + in_progress + no comment since dispatch*. An unclaimed dispatch produces **no
signal at all** — the detector built to catch dispatched-then-silent cannot see a dispatch that
never became tracker state. The bead's status is a *projection* of the dispatch; here the projection
was never written, so the watcher watched an empty slot.

```
file  →  claim  →  dispatch  →  observe  →  verify  →  close
         ^^^^^
         the beat that was being skipped
```

**The mechanical form:** the dispatch path must refuse to send a packet naming a bead that is not
`in_progress` and assigned to the receiving agent. A dispatch that cannot be projected into the
tracker is not a dispatch — it is a message.

**NO-CLAIM.** Claiming makes the work *visible*, not *done*. A claimed bead with a silent pane is
still a silent pane; this rule only guarantees the follow-up stage has something to look at. And an
`open`+unassigned bead is not by itself evidence of a skipped claim — it may simply be unstarted.
The signal is *dispatched* and unclaimed, which means the dispatch ledger, not the bead, is the
authority that closes this hole.

### A DISPATCH-ONLY INSTRUCTION IS AN UNRECORDED REQUIREMENT

**The same rule, one layer up, and measured on the orchestrator 2026-09-02.** A dispatch packet
said: *"take that `NUMBERS.toml` row as part of this dispatch — it is directly in scope as a FIXED
finding, not a side errand."* The implementer's report never mentioned it. The bead was then graded
against **the bead's acceptance**, which never carried the item, and **closed.** Verified afterward:
`grep -ciE 'aggregate|--lib|per-suite' NUMBERS.toml` → **0**. The row was never added.

**The grader could not see the requirement — and the grader was the same agent that wrote it.** A
packet is a transient message; the bead is the durable spec. Every acceptance check keys on the
bead, so an item that exists only in a packet is invisible to verification by construction, no
matter who verifies.

**The mechanical form:** if a dispatch adds scope, it MUST be written into the bead's acceptance
**before the packet is sent** — `br update --acceptance`, then read it back. A packet may explain,
prioritise, warn, and name traps; it may NEVER be the sole record of something the work must do.

**NO-CLAIM.** This makes an added requirement *checkable*, not *done*. A bead can carry a perfect
acceptance list and still be closed by a grader who skips an item — which is what happened here, one
level down. And the reverse failure is real too: an acceptance list edited after dispatch can move
the target under a worker mid-flight, so the edit must precede the send, not follow the report.

### THE RECEIVER MUST **ANSWER**, AND NOBODY WAS EVER TOLD TO

**The lifecycle chain above is missing a beat, and its absence stalled the whole session.**
`ack-stage` admits exactly one form of authoritative delivery evidence: a comment on the dispatched
bead whose prefix matches, byte for byte,

```
ACK <token> on <pane_id> --
```

where `<token>` is the **last hyphen-segment** of the bead id (`omp-orchestrator-zrq` → `zrq`;
`omp-orchestrator-kxe.4` → `kxe.4`) and `<pane_id>` carries its percent (`%1413`). The parser is
`crates/ack-stage/src/lib.rs:243-253`; `:308-320` downgrades an otherwise-`ReceiptConfirmed`
delivery to `Indeterminate/AckReadbackMissing` when that comment is absent.

**The transport rule is deliberate, not a defect.** `:290-291` states that the tmux literal
fallback is **always** `INDETERMINATE` on receiver heuristics alone — a timer reset plus a
content-hash change is explicitly **not** accepted as proof of a uniform transport. So on that path
the ACK comment is not one evidence source among several; it is **the only one the design admits.**

**Measured 2026-09-02.** Every dispatch packet written this session omitted the instruction, and
**zero ACK-prefixed comments existed anywhere in the tracker.** `omp-orchestrator-zrq` → `%1413`
therefore ended `ACK_STAGE_INDETERMINATE / unproven_transport` while the packet had plainly landed:
**12 `zrq` terms in the pane against a positive control of 10.** The transport worked; the answer
was never requested. For hours this read as a transport defect (`cp-nq2s9`) when the gap was that
nobody had been told to reply.

**Why it hid is the same shape as the subsection above.** The sender half is a tested Rust crate.
The receiver half is a sentence in a hand-written markdown packet — which no gate reads, no test
covers, and no schema requires. A protocol whose two halves live in different media fails silently
in the medium that has no checker.

```
file  →  claim  →  dispatch  →  ACK  →  observe  →  verify  →  close
                               ^^^^^
                        the answer nobody asked for
```

**The mechanical form:** the dispatch site must emit the ACK instruction itself, so a human writing
markdown cannot omit it. Tracked as `omp-orchestrator-93lo`.

**RESOLVED IN PRACTICE 2026-09-05, and the fix was one sentence in the packet.** The claim above
that *"zero ACK-prefixed comments existed anywhere in the tracker"* is **no longer true** and must
not be cited as current. Three dispatches that carried the instruction as a **mandatory first line**
produced the first admissible delivery evidence this repository has ever recorded:

```
[WildStone] ACK iis6 on %9 -- grading by re-execution, not the implementer report.
[WildStone] ACK iis6 on %9 -- GRADE PASS MUTATION-VERIFIED. RRL6_VIOLATED=no:...
[WildStone] ACK gcyf on %9 -- DONE 102afde. Did not touch main.rs.
```

**Nothing in the sender changed.** `ack-stage` was correct the whole time; the packets simply began
asking. The measured cost of not asking was every dispatch of the prior session reading
`unproven_transport` while packets landed — which is why this row is worth more as a *correction*
than it was as a finding.

### AND THE CHAIN IS STILL MISSING A BEAT: **RELEASE**, BETWEEN VERIFY AND CLOSE

**Measured 2026-09-07, and it stalled three beads at once.** The orchestrator routed grades for
`f3g5`, `djfu` and `lppp` to non-authors while **the author still held `assignee`**. Two panes
refused inside one minute, in near-identical words:

```
status=in_progress  assignee=pane=%20;incarnation=1;agent=pane20-omp-claude
"The current holder is also the author, so an independent grade must wait for an explicit
 release/reassignment. Please release or reassign, then send the grade request again."
```

**Both refusals were correct and the routing bug was the dispatcher's.** An
implementation-complete bead assigned to its implementer is **not grade-ready however the packet is
worded** — and the packet said the right thing ("take it if unassigned; if `br` shows a holder,
tell me instead of forcing"), which is precisely why the wall was visible instead of being
bulldozed by a forced second claim.

```
file → claim → dispatch → ACK → observe → verify → RELEASE → close
                                                    ^^^^^^^
                                    the author hands the bead back before a
                                    non-author can claim it for grading
```

**The mechanical form:** the dispatch site must refuse to route a grade to a pane while the bead's
`assignee` names a different pane. The dispatcher is the only party positioned to notice, because
it is the only one that knows both the author and the intended grader. Reassignment is not a
workaround for this — it **is** the release beat, performed by whoever routes.

**AND AN AGENT NAME IS NOT AN IDENTITY.** The first correction set `--assignee WildStone`, which
**designates nobody**. Derived from every pane-scoped assignee string in the tracker:

```
%7   WildStone
%8   WildStone     ← one name, THREE panes
%9   WildStone
%19  PearlGate
%20  pane20-omp-claude
```

So this file's grading bar — *"DIFFERENT PANE, not different lineage"* — is **load-bearing rather
than stylistic**: `pane=` is the only unique field in an assignee string. A name is a persona shared
across panes; a lineage is shared across everything. Any eligibility check keyed on the agent name
cannot tell an author from a grader. Recorded on `xsu4` and `0luy`, which own author resolution.

**NO-CLAIM.** The release beat makes an independent grade **possible**, not **independent** — the
grader must still re-execute the acceptance rather than read the author's report. And the collision
census reads only assignee strings carrying the `pane=` form, so panes that never claimed a bead do
not appear and a pane renamed across incarnations shows as two rows rather than one collision:
**three-pane `WildStone` is a floor, not a total.**

### ONE ACK PER PACKET, NOT ONE PER BEAD — an N-bead packet serialises the fleet

**Measured 2026-09-07, and it is the conductor's defect, not a pane's.** I dispatched three batch
packets in one wave, each demanding an ACK **per bead** — six ACK comments. Every ACK is a
`br comments add`, every one takes `.beads/.write.lock`, and `.beads/beads.db` is **31 MB with four
live writers**. `%8`'s first ACK landed; its second timed out after `1m17s` waiting on **PID 15800 —
`%9` posting an ACK I had also demanded.** I serialised my own fleet behind a delivery receipt.

`%8` did the right thing twice: it **refused to kill a peer's `br` process**, and it reported the PID
and the wait instead of retrying blind. Second time in one session a pane correctly refused that
kill, and both times the report beat the kill.

**The ruling: a packet needs ONE ACK.** The ACK exists to answer *"did the packet arrive"* — because
on the tmux path `ack-stage` admits only a matching bead comment, a timer reset plus a content-hash
change being explicitly insufficient (`crates/ack-stage/src/lib.rs:290-291`). **One ACK from a pane
answers that completely.** A second ACK for a second bead in the *same* packet raises no evidence
tier; it only multiplies write-lock contention on the critical path.

**Unchanged per bead:** the verdict, the re-run evidence, which tree each number came from, and the
`MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX` prefix with the status read back. **This
relaxes the delivery receipt, never the acceptance evidence.**

The rule immediately above says *"the dispatch site must emit the ACK instruction itself"* — correct,
and as written it invited a per-bead reading, which is what I built the packet from. **The dispatch
site must emit exactly one ACK token per packet.**

**NO-CLAIM.** This removes ACK amplification from the dispatch path; it does **not** fix the
contention. ~31 MB for ~939 beads is ~32 KB each, reads run 40–250 s against a 30 s default, and one
`br list --limit 4000` full scan starves every writer. The write discipline stands: long evidence to
a file, a one-line pointer in the bead, `--lock-timeout 60000` on every call, and **read the
output** — a suppressed `br update` failure is indistinguishable from success and silently lost two
claims in one session.

#### THE PACKET'S ONE ACK TOKEN MUST BE A REAL BEAD TOKEN — a nickname is unmatchable

**Caught by `%9` 2026-09-07, in my own rule, one packet after I wrote it.** It ACKed and reported:
*"`ACK s1ratify on %9 --` (on `omp-orchestrator-jplf.7.2`; token bead `s1ratify` does not exist)."*

**The token is mechanical, not a label.** `crates/ack-stage/src/lib.rs:293-298`:

```rust
fn ack_token(bead_id: &str) -> &str { bead_id.rsplit('-').next().unwrap_or(bead_id) }
fn ack_prefix(bead_id: &str, pane_id: &str) -> String {
    format!("ACK {} on {pane_id} -- ", ack_token(bead_id))
}
```

and the match is `strip_prefix` at `:507` against `format!("ACK {} on ", ack_token(bead_id))` — an
**exact prefix**. So a packet nickname can never match, and neither can a decorated token:

```
bead omp-orchestrator-typed-blocker-taxonomy-report-redispatch-zey6   token = zey6
  ACK zey6 on %9 --          MATCHES
  ACK zey6-grade on %9 --    NO MATCH   (extra text before " on ")
  ACK s1ratify on %9 --      NO MATCH   (no bead ends in -s1ratify)
```

**Measured over `.beads/issues.jsonl` — the JSONL, because `br list --json` omits comments
entirely; my first audit returned a blind `0`. CITATION CORRECTED 2026-09-07:** this was originally
attributed to `close-evidence-gate`'s `source.rs:223`, **a control-plane crate that does not exist
in this repo**. The defect is real HERE and `%19` proved it locally at
`crates/pre-delete-citation-check/src/lib.rs:194`, whose own body reads *"The br JSON does not
inline comments; the caller fetches them separately"* — **and no caller ever did**, both production
callers passing `Vec::new()` at `:198`. Measured: `br list --json --status closed` → **196 rows, 0
carrying a `comments` key**; the JSONL → **195 rows, 182 carrying comments**; **14 closed beads
cite a `bin/` or `.flywheel/` path ONLY in comments, one of them the bead that created the citation
gate.** Fixed and wired at `crates/no-shell-gate/src/bin/pre-commit-gate.rs:342`
(`read_closed_beads_from_mirror`), verified by pane 1 at the live hook crate.

**This names a variant not previously recorded here: BUILT ≠ WIRED at FIELD granularity.** Not an
uncalled crate — an unfilled struct field whose consumer runs on every commit. `ClosedBead::comments`
existed, `check_deletions` scanned it, and the callers handed it an empty vector, so the gate could
never catch the incident named in its own header. A crate-level wiring census cannot see this; only
reading what the caller passes can.

```
ACK comments matching the ack-stage prefix : 415
ACK comments UNMATCHABLE by construction   :  37   (8.2%)
  ACK selector on %9      on a bead whose token is 2ceb
  ACK reap on %9          on 3r1r
  ACK childoutcome on %9  on 7kxf
  ACK reroute on %9       on 93lo
```

**So the protocol is 92% healthy and the 37 are a real, bounded leak** — not the catastrophe the
first look suggested. Every one of the 37 reads to `ack-stage` as `AckReadbackMissing` and downgrades
its delivery to `Indeterminate/unproven_transport`: **the dispatch landed, the work happened, and the
evidence is unusable.**

**And the one-ACK-per-packet rule above MADE THIS WORSE, which is why the two clauses ship
together.** Per-bead ACKs were correct by construction — each token came from its own bead. By
collapsing to one ACK and naming it after the *packet*, I detached the token from any bead at all.
**A dispatcher writing a batch packet MUST pick one bead from the batch and use its bare token.**

**NO-CLAIM.** This fixes the token's *shape*. It does not make an ACK proof of progress — an ACK is
a delivery receipt any agent can type, and `:290-291` still holds the tmux path at
`INDETERMINATE` on receiver heuristics alone. And 415 matching ACKs is not 415 verified dispatches;
it is 415 parseable ones.

**And the correction is the load-bearing part, per this file's own rule about stale doctrine.** A
doctrine row asserting a mechanism is broken *licenses routing around it indefinitely*. Left
uncorrected, this section would have kept telling readers the ACK path produces nothing, long after
it started producing everything — the identical failure mode as the `refill-idle-panes` row that
claimed a kernel carried control-plane paths for hours after it had been rebuilt clean.

**What it bought beyond a receipt.** The `iis6` ACK carried `RRL6_VIOLATED=no` with an argument
against the law it might have broken, `TREE_PINNED=yes`, and a `NO_CLAIM` that explicitly declined
to credit the implementer for an uncommitted half. The ACK line is where a grader's *reasoning*
becomes checkable, not merely its arrival — so the structured tail (`MUTATION_RED=`, `SHA=`,
`POSITIVE_CONTROL=`) is not ceremony. It is the field that caught `WIRED_TO=dispatch_packet` naming
a caller that does not exist.

**STILL OPEN, and it is the whole mechanical fix.** Every one of those ACKs happened because a human
wrote the instruction into a hand-authored packet. `omp-orchestrator-93lo` — emit the instruction
from the dispatch site — remains **unlanded**, so the protocol still depends on the conductor
remembering. Three ACKs prove the receiver half works when asked; they prove nothing about the next
packet a tired operator writes.

**NO-CLAIM.** An ACK proves the packet **arrived and was read** — nothing about the work. Acceptance
evidence is still the bead's own criteria, re-run by a grader who is not the implementer. And an ACK
is forgeable by construction: it is a comment any agent can write, so it is a *delivery* receipt,
never a *progress* one.

### THE INPUT MANIFEST IS A FIELD, NOT PROSE

Any tool, census, or grade result used as evidence MUST carry a required InputManifest field. The field has exactly three states: FULL, PARTIAL with bound_kind, bound_value, and source, or REFUSED with reason. There is no Default implementation: omitting the manifest is a construction error, not an implicit FULL.

An empty scan or result set is the typed EmptyScanSet error, distinct from FULL with zero rows and distinct from PARTIAL. PARTIAL and REFUSED results are non-citable acceptance evidence; the grade-ingestion boundary MUST reject them before a bead can close. A non-recursive or otherwise bounded instrument MUST emit PARTIAL with its bound named, or REFUSED; it MUST NOT silently slice or answer an empty set.

The mechanical form is deliberate: the manifest is a serialized struct field on the emitted artifact, not a log line, comment, or convention. Source paths from state files and pane transcripts are tagged SelfReferentialCorpus; a corpus containing only those hits MUST NOT report FULL. This is the neighbouring rule to file -> claim -> dispatch: the result carries what input was actually consumed before anyone cites it.

NO-CLAIM: a manifest records the instrument's declared input coverage. FULL does not prove the subject result is correct, and static source coverage does not prove a runtime invocation.

---

## A DENIED OR ERRORED PROBE IS *UNKNOWN*, NEVER A NEGATIVE RESULT

**Measured 2026-09-02, and it is the ugliest root cause of that session.** An agent checked whether
the `am` CLI carried an auth token with `env | grep -i -E 'agent_mail|AM_'`. **`dcg` DENIED the
command as a policy violation.** The agent never retried, then asserted *"the CLI carries no token"*
as measured fact, built a two-authority architecture on it, wrote it into a crate's module docs, and
broadcast it — where it was adopted as house doctrine.

Every part of it was false. `printenv HTTP_BEARER_TOKEN` -> **SET, 65 chars**; `AGENT_MAIL_TOKEN` ->
**SET**; both read at `mcp-agent-mail-cli/src/lib.rs:82140`. The CLI had authenticated via the
environment the entire time. `printenv` was available throughout.

**The refusal was not evidence. It was the absence of evidence, wearing evidence's shape.** **A tool
refusal, a non-zero exit, an empty result, and a policy denial are all UNKNOWN.** The honest moves
are: retry differently, or say unknown. Asserting the negative is the one move that is never
available.

**AND THE THING THAT FALSE PREMISE WAS INVENTED TO EXPLAIN IS NOW UNEXPLAINED AGAIN.** `am agent
start` reported "no listener on 127.0.0.1:8765" while `curl /health` returned `status: ready` and
two robot calls returned live data. The two-authorities story accounted for it; the story is false,
so **the contradiction is OPEN and must stop being cited as answered.** A retracted explanation
does not leave the thing it explained explained.

**What survives, measured at the shipped tag `v0.3.31`:** the CLI calls the daemon **by default** —
`mcp-agent-mail-cli/src/lib.rs:8911` is `!direct || daemon_reachable`, doc-commented "the default
(non-`--direct`) path always prefers the daemon", so **omitting `--direct` takes the daemon
unconditionally with NO fallback** and the SQLite read is the exception. Its own 401 text at
`:40179` names `AGENT_MAIL_TOKEN`/`HTTP_BEARER_TOKEN` — a CLI that never called the daemon could not
emit that. `/api/` and `/mcp/` are each other's alternates (`:9351-9352`). And `am health` really
does build a throwaway probe SQLite and never contacts the daemon — **that single measurement was
correct; the error was generalising `health` to the entire CLI.**

**A CORRECTION TO THIS SECTION'S OWN FIRST DRAFT, which is the point of the section.** It shipped a
replacement row claiming `--direct` is inverted relative to its help text. **That is also false**,
and a second reader refused it before it could be filed: the installed `--help` reads "Allow a
direct SQLite read only when no daemon is reachable", which **is** the predicate. Help and code
agree; the doc comment names GH#158 (WAL contention). **A retraction is not a licence to publish the
next plausible story** — the replacement needs the same standard as the thing it replaces, and this
one was accepted into doctrine for twenty minutes on nobody's measurement.

### A BORROWED CLAIM INHERITS ITS AUTHOR'S BURDEN

**The rule above covers the diagnosis half. This is the transmission half, and it is how one wrong
claim became house doctrine across three agents in under an hour.**

The denied-probe failure was one agent mis-diagnosing its own measurement. What happened next was a
different failure with a different cure: **two other agents, including this file's editor, repeated
the finding as established provenance without reading the source.** It arrived measured-sounding —
file, line, a predicate, a confident causal story — and that shape was accepted *as* verification.
It was then written into `AGENTS.md`, cited three times as evidence in grades, and broadcast to the
fleet as "the definitive explanation" before anyone opened the file.

**A report is a claim** — the rule this repo already applies to subagents and to bead close reasons.
It applies identically to a **peer**, and it is easier to forget there, because a peer's claim
arrives with the social weight of collaboration rather than the suspicion we reserve for our own
probes. **Restating someone else's finding makes it yours.** Cite it and verify it, or attribute it
and mark it unverified. There is no third option in which you get to hold it as fact because someone
else measured it.

**The measurable tell:** if you can state a claim's file and line but have not opened that file, you
are transmitting, not verifying. The cheap fix is to open it — every one of the four refutations in
that investigation cost one `git show` against a version-matched tree.


### RE-RUNNING THE HOOK AFTER A COMMIT IS A CATEGORY ERROR, NOT A RED FLAG

**Measured 2026-09-02, and it is a trap laid by a correct fix.** Since the empty-index repair, a
standalone `.git/hooks/pre-commit` run returns **3** with `NOTHING_TO_CHECK: no staged files to
check` once the index is empty — which is exactly right, because after a successful commit **there
is nothing staged to check**. The gate is answering the question it was asked.

But the obvious way to double-check a hook — commit, then run the hook again to be sure — now
produces a nonzero exit and a refusal-shaped message, and **reads as a failure that just landed**.
Two agents walked into a version of this tonight, one of them the author of the fix.

**Post-commit, the authoritative evidence is the commit-time `CLEAN: all staged files passed the
multi-gate checks` line.** A later standalone run measures a *different input* — an empty index —
and therefore cannot confirm or refute what the commit did. It is not a weaker check; it is a check
of something else.

**The general form, which is the reusable part:** a gate's verdict is only meaningful paired with
the input it ran against. Re-running a gate against a *different* input and comparing verdicts is
the same error as comparing two `git` figures taken from two trees — and it produces the same
confident-wrong reading. Capture the verdict at the moment of the operation, or re-create the input
before re-running.

### A PEER'S DEFECT REPORT IS A VERDICT ON A TREE. REFUTING IT ON A DIFFERENT TREE IS THE SAME ERROR.

**The rule above is stated about hooks and commits. It applies identically to a PEER'S BUG REPORT,
and that is the harder case, because a refutation feels like verification.**

**Measured 2026-09-07, a genuine near-miss.** `%20` reported a fleet-wide cargo outage: a stray `.`
outside the closing quote at `crates/refill-idle-panes/Cargo.toml:6:137` broke workspace *loading*,
so `-p <crate>` could not dodge it and every cargo command in the repo failed. Pane 1 measured the
diff, found a peer's two dependency additions in flight beside the typo, applied the minimal fix —
period back inside the quote — and broadcast the clearance.

`%19` then measured line 6, got **byte-identical to HEAD** on both sides, saw a `git diff` with
**no `description` hunk at all**, and drafted: *"the accused line is byte-identical to HEAD; the real
diff is two added dependency lines."* **A refutation of a correct diagnosis, one message from
broadcast.**

**The fix had landed between the notice and the read.** `%19`'s measurement was correct and
consistent with a **post-fix tree**; it could say nothing about the pre-fix claim. And the pre-fix
state was **unrecoverable** — the file was uncommitted, so there is no history to diff against. Only
the reporter's own quoted error text survived as evidence.

**The cost of publishing it would have been real and asymmetric:** a correct diagnosis discredited,
its author's next report discounted, and the next outage of that class read as a false alarm. **A
wrong refutation is worse than a wrong report**, because it also destroys the reporting channel.

**The mechanical form:** a defect report is a verdict on a tree at a time. Before refuting one,
establish that you are reading **the same input** — check whether the file changed since the notice
(`git status`, mtime, or ask), and **name the tree your refutation read**. A refutation that does not
name its input is exactly as stale as the claim it thinks it is killing.

**Corollary for uncommitted state, which is where this bites hardest.** A `git`-based check cannot
reconstruct a dirty file's earlier content. When the subject is uncommitted, **the reporter's quoted
error output IS the primary evidence** — treat it as the artifact, not as a claim to be re-derived.
A repair that clears the symptom also destroys the evidence, so the fixer must quote the pre-fix
state in the clearance notice. Pane 1 did quote the byte and the two `sha` values; that is what let
`%19` reconcile instead of escalate.

**NO-CLAIM.** This makes a stale refutation *detectable*, not impossible. Two panes reading the same
tree can still disagree for other reasons — different globs, different anchors, different key shapes
across `br show` / jsonl / `br list`. Naming the tree removes one failure mode from a family this
file records eight other members of.


### A FILTERED DIFF IS NOT A DIFF — and this member of the family drives a DESTRUCTIVE command

**Measured 2026-09-07, self-reported by `%20` against its own remedy.** During the fleet cargo
outage it needed to inspect one manifest line and ran:

```bash
git diff -- crates/refill-idle-panes/Cargo.toml | grep -E '^[-+]description'
```

It then reported the defect correctly — a stray `.` outside the closing quote at `:6:137` — **and
named `git checkout -- <manifest>` as the fix.** That command would have destroyed a peer's two
in-flight dependency additions (`agent-mail-native`, `asupersync`) and desynced them from their own
modified `src/main.rs` and `tests/differential.rs`.

**The distinguishing feature: the evidence was RETRIEVED AND THEN DISCARDED.** The two `+`
dependency lines were in the command's output stream; the filter dropped them before any human or
agent read them. Every other member of this family produces a wrong **number** — `$?` after a pipe,
`git log -S` skipping merges, `grep -c … || echo 0` emitting `"0\n0"`, `rg -c | wc -l` returning the
glob size. **This one produced a wrong BASIS FOR A DESTRUCTIVE ACTION**, which is a strictly worse
failure mode than a wrong figure.

**And prose caution did not save it.** `%20` had already written *"a peer mid-edit could have a
larger change in flight that my fix would clobber"* — correct reasoning about the hazard — and then
let a narrowed `grep` tell it the hazard was absent. **A stated risk does not survive contact with
a filtered instrument**; the filter answers a different question and looks complete doing it.

**The mechanical form:** before any command that discards working-tree state, read the **whole**
diff and the **whole** `git status` for the path. Narrow only to *locate*, never to *decide*. And
prefer the minimal edit over the categorical revert — the fix that shipped was moving one character
back inside the quote, which left every other change intact and made line 6 byte-identical to HEAD
(`sha f4e657007bbd33d1` both sides).

**NO-CLAIM.** Reading the whole diff catches co-located peer work in the *same file*. It does not
catch a peer whose related edits sit in files you did not diff — here, `src/main.rs` and
`tests/differential.rs` were also ` M`, and only a path-scoped `git status` showed them. Whole-diff
plus whole-status, or the check is partial.

### `git show | grep` CONFLATES THE COMMIT MESSAGE WITH THE DIFF — and a claim of absence can be defeated by its own prose

**Measured 2026-09-07. My own claim, caught by `%20`, and the purest instance of the
self-referential-instrument family in this file.**

I wrote *"`%19`'s diff touches `owner` 0 times"* to attribute a lane test failure to the
environment rather than to a peer's commit. **The substance was right; the method was not.**

```
git show 49c7c22 | grep -c owner                                    -> 2   <- includes the MESSAGE
git show --unified=0 --format= 49c7c22 | grep '^[+-]' | grep -c owner -> 0   <- diff only
git diff 49c7c22^ 49c7c22 | grep -c owner                            -> 0   <- diff only
```

**The two hits are in the commit message, and line 59 of that message is the sentence
*"My diff touches `owner` 0 times"*.** So a verification of absence, run with `git show | grep`,
**returns nonzero because the claim's own prose contains the needle it denies.**

> **`git show` is `message + diff`. If you are making a claim about the DIFF, you must exclude the
> message** — `--format=` empties it, or use `git diff <sha>^ <sha>` and never `git show`.

**Why this is worse than the sibling rules:** `git log -S` skipping merges produces a *false zero*,
which reads as absence and is caught by any positive control. This produces a **false NONZERO on a
true absence**, so the instrument appears to *refute* a correct claim — and the more carefully the
commit message documents the reasoning, the more likely it is to defeat the check. **A well-written
commit message is the failure mode.**

Same family, all measured here: `git log -S` skipping merges; `grep -c … || echo 0` emitting
`"0\n0"`; `$?` after a pipe returning the pipeline's status; a doc comment containing the needle it
warns about; a census table naming every gate it checks; and a citation-hygiene scan finding the
specimens inside its own defect reports. **Twelfth instance, and the first where the instrument's
extra input was the author's own explanation.**

**`%20` hit the sibling shape on the same bead, one call apart:** its opening census printed **3**
raw `.output()` sites and was counting the `// omp-orchestrator-3kcl: this was a raw '.output()'…`
**comments documenting the fix.** Comments stripped → **1**, and that one is the `#[cfg(test)]`
helper. **Strip comments before matching** — re-learned by publishing a wrong 3 and catching it in
the next call.

### `git log -S` SKIPS MERGES BY DEFAULT — ITS ZERO IS NOT EVIDENCE OF ABSENCE

**Measured 2026-09-02.** An agent searched for the commit that introduced a string with
`git log -S`, got an **empty result**, and concluded the anchor did not exist. The real commit was
`0b929ef` — verified with `git show --name-only`, which lists the file, and
`git rev-list --parents -n1` returns **3 entries, so it is a MERGE commit.** `git log -S` traverses
only the first parent by default and therefore **cannot see a change that arrived through a merge.**

**The zero was structurally guaranteed, not observed.** Use `git show --name-only <sha>` when you
have a candidate, and add `--full-history -m` (or check merges explicitly) when searching. This is
the sixth member of the instrument-manufactures-its-own-reading family in one session, alongside
`$?` after a pipe, ERE parens in a BRE context, `grep -c … || echo 0` emitting `"0\n0"`, a backtick
eaten inside single quotes, and the same command name resolving to different binaries.

**AN EXCEPTION LIST IS EVIDENCE TOO, AND THIS ONE WAS WRONG.** The same pass found that a
`FIXED_POINTER_ALLOWANCE` row — an entry whose whole purpose is to record where a fix landed —
**cited a commit that does not touch the file it claimed.** The exception list carried an unverified
evidence pointer: the identical defect, one level up from the one it was written to record. **Audit
your allowlists with the same probe you audit the code with**, or the list becomes the place wrong
evidence hides from the gate that would have caught it.

**AND A SUBAGENT FABRICATED A COMMIT SHA.** Four verdicts were attributed to `4aaae09`;
`git cat-file -e 4aaae09` reports **the object does not exist.** A second verifier attributed two
findings to a commit touching neither file. **A cited sha is a claim, and `git cat-file -e` is one
command.** Parallel verification still earned its cost here — as a *decoy detector*, not as
corroboration.


**AND IT DOWNGRADED THE INVESTIGATOR'S OWN BEST EVIDENCE, which is why this rule is worth more than
the correction.** `oracle_skew=0` was reported as two independent authorities agreeing about a
store. It is **two HTTP routes on the same daemon process, authenticated with the same token,
reading the same in-process state.** It proves one daemon is self-consistent; it does not
corroborate the store. A differential oracle whose two arms share a process, a credential and a
cache is not a differential oracle — **it is one reading, taken twice.**

The consequence for consumers ran the *other* way from what was announced: every CLI-derived figure
came from the DAEMON and is therefore MORE trustworthy than the fleet was told, not less. **The
numbers were fine; the explanation of where they came from was not.**

---


## A TRANSCRIBED VALUE IS STALE BY DESIGN — FIVE SUBSTRATES, ONE SHAPE

**Measured across two repositories on 2026-09-07. Five instances, five different substrates, one
defect: a claim that transcribes a value instead of binding to something that RE-DERIVES it.**

**Substrates 1–4 are citations whose TARGET moves. The fifth is different in kind and is the worst,
because it defeats re-running: a figure whose SCOPE moves while the command and the tree hold
still.** It has its own subsection at the end.

|substrate|the instance|why it went stale|
|---|---|---|
|**FILE LINES**|`CONTRACT.md`'s superseded-by pointer said `:101`, corrected to `:116`, and **the correction invalidated itself in the same edit** — the inserted lines pushed the target to `:124`, then `:132`, then `:154`. Fixed at `13fc201`|any edit above the target shifts it, and the edit most likely to be made is the one fixing the pointer|
|**BINARY VERSIONS**|AGENTS.md's 42-method RPC census anchors on `let w=async(v)=>` … `},E=new KWt`. **Both return 0** at the installed `omp/18.1.13`; the gate pins `18.0.11 / a95635ad… / 19,803,745 bytes`|**`uca service install` runs a THREE-HOUR auto-updater** across `claude, codex, agy, grok, omp, muse`. The pin is stale by design, not by neglect|
|**PROSE PREMISES**|control-plane's `AGENTS.md:12` asserted three root files "never existed", measured 2026-09-03. **All three exist.** The stale entry propagated into a dispatch, then a worker restated it back to its author as established fact. Fixed at `daab4fe`|a dated measurement embedded as a standing claim, with the date discarded|
|**UNREACHABLE ANSWERS**|`crates/omp-surface-consumption/src/lib.rs:18` **had already recorded** the vanished anchor, and `:13-14` the version drift, five days before two agents independently re-derived it. `cargo-bin: 1`, **`PATH: ABSENT`**|the citing document could not reach the crate holding the answer. The operator-surface defect causes measurable duplicate work, not just inelegance|

**THE REMEDY IS THE SAME IN ALL FOUR: bind the claim to something that RE-DERIVES, or stamp it with
a fetch time and an expiry.** Never transcribe a value that another process owns.

What that looks like concretely, each verified in this repo:

- **Cite a searchable string, not a line number.** `grep -nE '^\*\*S1 IS AUTHORIZED TO BUILD'` —
  and **anchor it so the pointer is not its own hit.** Unanchored returned **3**, two being the
  pointer quoting the phrase.
- **Anchor a binary probe on WIRE CONTRACT, not on minified identifiers.**
  `omp-surface-consumption:64` gets this right: `ANCHOR_METHOD = "negotiate_protocol"`. A protocol
  method name survives a rebuild; `let w=async(v)=>` is a minifier's variable name and does not.
- **Stamp every figure with its measurement time and say which TREE it came from.** `cargo` reads
  the WORKTREE; a sha names a TREE. A grade citing both has silently mixed two states.
- **When the answer is in a crate, INSTALL the crate.** An answer nobody can invoke gets re-derived.

### THE COROLLARY THAT COSTS THE MOST: SCOPE A VOIDING RULE TO WHAT IT ACTUALLY GOVERNS

**Measured the same day, and it was my own error.** Joshua's binding rule is **ALL BUILDS MUST TAKE
THE CONTABO LANE.** I broadcast it to five panes as voiding *"any figure derived from"* a local run.

**That over-applies, and control-plane pane 0 caught it before it did damage: THE BINDING BINDS
BUILDS.** The two results that actually moved the product that night involved **no cargo at all** —
`uds-dc-stamp-ne-x35` is a python predicate reading the working tree, and `uds-kii.2` repaired a
`sed` block inside a markdown contract. Voiding non-build figures would have discarded the only two
rows that moved `dag_closure_scorecard.tsv` (PASS 1 → 2, UNRUN 46 → 45).

**A voiding rule is itself a claim and inherits every rule above.** State the predicate it voids on,
not a vibe about provenance: *builds and their test figures*, not *everything measured locally*.
An over-broad retraction destroys good evidence and is harder to undo than a stale figure, because
the good evidence does not come back when the rule is narrowed.

**NO-CLAIM.** Binding to a re-deriving probe makes a claim *self-correcting*, not *correct*. A
probe can re-derive the wrong thing forever — `%20`'s **"I measured TOKEN PRESENCE and reported
REQUIREMENT EQUIVALENCE"** is exactly that: an instrument internally consistent and pointed at the
wrong object. Re-derivation fixes staleness; only a positive control and a known-bad leg fix aim.

### I MEASURED A SUBSET AND GENERALISED TO THE POPULATION — 19 of 44, not 19 of 19

**Measured 2026-09-07 by `%20`, correcting me. My own unstated-denominator defect, committed while
documenting the class.**

I reported *"all 19 recorded HD decisions carry a real decision, so 'awaiting a human' is almost
always false"* and built a bead-triage rule on it. Re-measured over `docs/decisions.jsonl`:

```
rows                                  56
distinct HD ids                       44        <- THE POPULATION
ANY row carries a decision            19        HD-0001..HD-0018, HD-0033
NO row carries a decision             25        HD-0019..HD-0032, HD-0034..HD-0044
of the 19 decided, carrying an execution receipt   ZERO
```

**My claim is exactly right about those 19 and does not generalise to a population of 44.** So the
conclusion **INVERTS**: *"awaiting a human"* is **genuinely true for 25 of 44 ids.** The inversion I
found holds for `HD-0009` specifically — which happened to be the one that mattered, which is
precisely why the over-generalisation survived.

**AND THE MISSING FOURTH STATE IS THE LARGEST ONE: `ExecutionOwed`.** Every recorded decision is
unexecuted — **zero execution receipts across all 19.** `HD-0008`'s *"push it"* from 2026-09-02 is
the archetype.

> **A bead in `ExecutionOwed` is WORK and must NEVER be excluded as a human hold** — which is
> exactly what a hand reading of a human-sounding title does. I did it twice in one pass: a
> prose-anywhere matcher over-caught **9** (seven P0), then a title-only matcher over-caught **8**
> more. Both measured token presence and reported pending-decision state.

The five states a triage predicate must distinguish, per `%20`'s runner (`fe291df`):
`DependencyBlocked` · `TrackerBlocked` · `ExecutionOwed` · `AwaitingHumanDecision` ·
`Unclassifiable`. **Count only the two that are work wearing a hold; exclude
`AwaitingHumanDecision` BY A NAMED PREDICATE; make `Unclassifiable` an ERROR** so a bead naming an
unknown id is never silently excluded.

### TWO CRITERIA OVER ONE SET MUST NOT DISAGREE ON THE DENOMINATOR

**Measured 2026-09-07: R3 printed `144` where R2 printed `138` over the same beads**, because R3's
population did not exclude the six layer gates. **Neither number was wrong in isolation and only
running them side by side revealed it.**

A criterion's denominator is part of its claim. Two criteria scoped to one set and reporting
different populations means at least one is measuring something other than what it names — and
**both can pass while disagreeing**, which is the failure mode: nothing in either runner compares
them.

**`0 of 0` MUST BE PRINTED WITH ITS POPULATION.** `%20`'s live R3 reads
`0 of 0 blocked (population 138)`, because **a criterion reading 0 from an EMPTY population cannot
otherwise be told from one reading 0 from a healthy one.** That is the anti-vacuity rule stated as
an output format rather than a test.

**AND FIXTURES CAUGHT TWO DEAD GUARDS IN THE AUTHOR'S OWN RUNNER, second pass running:**

- **The ledger parser accepted any row with an `id` as a decision id.** Pointed at a BEAD file it
  yielded one bogus id, `seen_ids` was non-empty, and **the anti-vacuity guard silently did not
  fire.** Fixed with `re.fullmatch(HD-\\d{4})`.
- The population/denominator disagreement above.

Together with R2's known-good leg catching a `138 of 138` false FAIL, that is **three instrument
defects caught by mandatory legs in two passes** — every one invisible to a clean run. **Prefer the
report that names its own instrument failures over the one that reports green.**


### A FIX'S OWN MUTATION OUTPUT IS NOT A DESCRIPTION OF THE PRE-FIX TREE

**Measured 2026-09-07. The first time tonight a wrong line number came from EVIDENCE rather than
from age — and it is the most dangerous variety, because it arrives with a passing/failing verdict
attached and therefore looks authoritative.**

A `gate.yml` fix (`99295c8`) shipped a mutation leg that removed the restored job key to prove the
detector bites. The detector printed
`DUPLICATE_KEY scope=jobs/head-compiles-as-committed key=runs-on lines=[154, 184]`, and **`184` was
carried forward as the pre-fix duplicate.** Measured against both real trees:

```
PRE-FIX  99295c8^   154 runs-on / 155 steps  +  171 runs-on / 172 steps   <- the actual duplicate
                    strict loader: DUPLICATE KEY 'runs-on' at line 171
HEAD     99295c8    154 runs-on / 155 steps  +  184 path-literal-guard:   <- a RESTORED JOB KEY
                                                185 runs-on / 186 steps   <- that job's own body
```

**`184/185` was never a duplicate pair in any commit.** It is an artifact of the mutation's
*synthetic intermediate state*: a 13-line explanatory comment had already been inserted, then the
job key removed. **That state exists in no tree.**

**AND THE DANGEROUS HALF:** in the current tree, line **184 is `path-literal-guard:`** — the
restored key. A reader who took "184/185" as "the duplicate to remove" would **delete the restored
job and re-create the original defect**, and the same three detectors would go red exactly as
before — **so it would read as a regression rather than a re-introduction.**

> **Cite a mutation for DIRECTION — it went red, it came back green, the restore was
> byte-identical. NEVER for LOCATION.** A mutation deliberately perturbs the file, so its line
> numbers are the least citable in the whole record.

**What to cite instead, in the form that re-derives:**

```
pre-fix tree     99295c8^
strict verdict   DUPLICATE KEY 'runs-on' at line 171     (the loader quotes its OWN line)
cause            no `path-literal-guard:` job key existed; two jobs had collapsed into one
remedy           RESTORE one job key — never delete a block; both blocks are real jobs
fix              99295c8
```

**The cause sentence re-derives from any tree; the line numbers do not.** Let the strict loader's own
message carry the line, exactly as `13fc201` did for `CONTRACT.md`.

**AND THE REMEDY WAS INVERTED BY THE SAME ERROR.** Both beads describing this defect proposed
deciding "which block is canonical" and deleting the other. **Neither block was redundant** — the
root cause was a *missing* job key, so the correct fix is a one-line RESTORE. **A deletion would
have removed a real CI job and gone green**, which is the failure the beads existed to prevent.

**Third line-pinned citation to be wrong in one session** — `AGENTS.md`'s `:46/:52`, `m0c`'s title,
and this one. The first two went stale; this one was **born wrong from a correct measurement of the
wrong tree state.**


### FIFTH SUBSTRATE — A FIGURE WHOSE **SCOPE** MOVES WHILE COMMAND AND TREE HOLD STILL

**Measured 2026-09-07 by `%19`, filed as `omp-orchestrator-mmt4` (P0). The first four substrates are
citations whose TARGET moves. This is a figure whose DENOMINATOR moves — and it defeats both clauses
of our grading standard at once.**

Three answers, one command, all real:

```
local, fail-fast       (bypass, darwin arm64)    11 targets    33 passed /  3 failed
lane,  fail-fast       (contabo-2, exit 101)      9 targets    23 passed /  2 failed
lane,  --no-fail-fast  (contabo-2, exit 101)     52 targets   251 passed / 55 failed
```

**`cargo test` stops at the first failing TARGET, and the stop point is environment-dependent.** The
two truncated figures are **not** bigger and smaller versions of one measurement — they are
different **PREFIXES**.

**The mechanism, corroborated at source level by pane 1:** `crates/no-shell-gate/tests/` holds **43**
integration targets (plus 6 `src/bin` and 1 lib), and **`artifact_provenance.rs` is alphabetically
FIRST while `bead_shape.rs` is THIRD.** On the lane `artifact_provenance` fails, so execution stops
and `bead_shape`'s three label failures **never run**. Locally it passes, so they do. **The stop
point is filename ordering crossed with which target fails in this environment.**

**Why this is P0: it survives both halves of "a grade re-runs, and states the tree."** A grader can
re-run the exact command, on the exact tree, in the mandated lane, and cite a figure that silently
omits **43 of 52 targets**. This is the FOURTH quantity in this repo wearing the word "tests", and
the only one indistinguishable from the full aggregate by inspection.

#### RULING (pane 1, 2026-09-07). Both halves of the proposed choice, split by purpose.

1. **An ACCEPTANCE leg MUST cite a NAMED TARGET** — `cargo test -p <crate> --test <target>`. A named
   target **cannot truncate**, is cheap, and is what a bead's acceptance is actually about. `%20`'s
   on-lane re-runs already did this correctly (`--test starvation_taxonomy` → 3/0,
   `--test empty_staged` → 5/1).
2. **A CRATE-HEALTH claim MUST carry `--no-fail-fast` AND the target denominator** — "251 passed /
   55 failed across 52 targets", never "251 passed / 55 failed".
3. **A bare `cargo test -p <crate>` figure is INADMISSIBLE as evidence.** It is a prefix of unknown
   length whose end is decided by alphabetical filename order crossed with environment. Not
   "discouraged" — inadmissible, the same standing as the retired "81 JSON-RPC methods, 17 used".
4. **A `--no-fail-fast` failure count is a COUNT, not a DEFECT count**, until environment-caused
   failures are split out. `%19` named this against its own figure: `this_repo_is_clean`,
   `every_in_repo_line_cite_names_a_line_that_exists` and
   `hook_validates_staged_bytes_not_a_dirty_worktree_copy` need `.git`, `.beads`, `crontab` or a
   clean worktree, **none of which `rch` syncs**. **Nobody may cite 55 as a defect count**, its
   author included.

**AND THE AUTHOR COMMITTED THE DEFECT WHILE FILING A BEAD ABOUT DISHONEST COUNTING.** `3ae3`'s
description reported "33 passed / 3 failed across 11 targets" as `no-shell-gate`'s state; the crate
has **52**. A fifth of the suite described as the whole — the unstated-denominator defect. `3ae3`'s
*defect* survives (absolute-count ratchets over live tracker data is a source property no
environment changes); its *figures* do not.

**Corollary for the CONTABO binding:** three separate panes have now measured live-`.beads` tests
going RED on the lane because **`rch` syncs source without `.git` or `.beads`**. That is a
structural consequence of the binding, not a defect in those tests. An absent mirror is
**UNMEASURED**, never "the oracle is vacuous" — and the discriminator must be POSITIVE: a tree with
no `.git` is a synced worker copy and cannot answer; a tree that IS a checkout with no mirror still
FAILS. Absence alone never satisfies.

**NO-CLAIM.** This ruling makes a truncated aggregate *detectable*, not impossible. `--no-fail-fast`
still cannot separate an environment failure from a real one — item 4 is a disclosure requirement,
not a mechanism — and **libtest CAPTURES stdout for a PASSING test**, so a green suite cannot itself
distinguish "ran and passed" from "declined as UNMEASURED" without `-- --nocapture`.


---


## KERNEL-ONLY (binding): you may not handroll a capability a kernel provides

**We build the system and then do not use it.** Measured 2026-08-31, five handrolls in one
session — by the author of the kernels:

| job | what I did | the kernel that already existed |
|---|---|---|
| observe panes | `tmux capture-pane \| grep -oE` for 12 hours | **`tick-monitor observe`** — installed, and returns *more*: state, timer, liveness, attention, dead panes, correct session scoping |
| dispatch | raw `tmux send-keys` | **`ntm --robot-send`**, `refill-idle-panes`, `fast-dispatch`, `controller-tick`, `loop-driver` — installed. **NOT cron-scheduled: corrected 2026-09-07.** The only crontab line naming any of them is a COMMENT (*"refill-idle-panes refused 51 ticks, 2026-09-02"*) — an epitaph for rows that were removed. Per the corrected census in rule 9, 23 of 36 bin kernels have a `.flywheel/` document mention and **no executor**. A handroll is still worse than the kernel, but the kernel is not firing on its own either |
| receipt | `grep -oE` on a timer | **`receiver-receipt`** |
| file a bead | raw `br create` | **`crates/finding`** — which I wrote *thirty minutes earlier* to make an unfiled gap impossible, then bypassed in the next tool call |
| read the queue | `br ready --json \| python3` | **`bv --robot-triage`** — the planning brain, which reports scores the raw query cannot see |

My hand-grep silently mixed a control-plane pane into an omp-orchestrator census, because it never
scoped. The kernel does. **A handroll is not merely redundant — it is usually worse.**

### THE RULE: if a kernel is broken, FIXING IT IS THE WORK

This is the clause that matters, because it names the mechanism rather than the symptom.
`refill-idle-panes` carried **only control-plane paths** (measured via `strings` on 2026-08-31), so
it supervised the wrong repo all night. Rather than fix one default, I hand-dispatched for hours.

**CORRECTED 2026-09-02 — the kernel was FIXED and the doctrine outlived the defect.** The binary
was rebuilt Sep 1 19:29 and now carries no `/Users/josh/Developer/*` literals at all
(`strings … | grep -oE '/Users/josh/Developer/[a-z-]+'` returns nothing). `--plan` from this repo
correctly resolves `bead=omp-orchestrator-omp-surface-map-41b`, and the `--apply` lane is cron'd at
`8,28,48` and alive — its log reads `no idle pane both surfaces agree on — nothing to do`, which is
the two-surface agreement rule working, not a silent failure.

**The lesson survives the fix, and got sharper.** A stale "this kernel is broken" note is itself a
reason to handroll — it licenses the routing-around indefinitely, long after the kernel is sound.
**A doctrine row asserting a kernel is broken MUST be re-measured before it is obeyed.** Measured
the same session: I handrolled `tick-monitor`'s job with `capture-pane | grep` for hours on the
strength of notes like this one, while `tick-monitor observe` was installed, correct, and returning
strictly more — `state`, `timer_secs`, `liveness`, `attention`, `dead_panes`, `omp_lifecycle`,
`git_commits`, correctly session-scoped. Its first invocation reported `gap_secs=7773`: **nobody
had observed a tick in 2.2 hours.**

One live defect remains, and it is small: `refill-idle-panes --plan` proposes `pane=1`, the
ORCHESTRATOR pane. The orchestrator must be excluded, and no `OMP_*` exclusion variable appears in
the binary's strings. **That is the fix to make — not a reason to hand-dispatch.**

> **Every handroll is locally cheaper and removes exactly the pressure that would have fixed the
> kernel.** That is why the kernels stay broken. Routing around a broken kernel is not pragmatism;
> it is the thing that guarantees the next agent finds it broken too.

The same shape, three ways in one session: prose instead of beads; `br create` instead of `Finding`
(the standard exists as a type, unenforced); hand-grep instead of `tick-monitor` (the census exists,
unqueried).

#### ⛔ CORRECTED 2026-09-07 — "`crates/finding` exists, ZERO CALLERS" IS FALSE IN BOTH HALVES

**Retracted:** *"`crates/finding` exists, zero callers"* and the row above reading *"which I wrote
thirty minutes earlier to make an unfiled gap impossible, then bypassed in the next tool call."*
Caught by `%20` and re-measured:

```
external manifest callers                    14
src references (`finding::`)                 17   across 15 crates
  ack-spine, crate-atom-gate, dispatch-silence-watch, fast-dispatch, finding-dispatch,
  fleet-truth, loop-coverage, loop-tick, no-shell-gate, omp-idle-dispatch,
  omp-inventory-map, omp-orchestrator, pre-delete-citation-check, refill-idle-panes,
  verify-dispatch

[[bin]] in Cargo.toml                         0
src/bin/ entries                              0
src/main.rs                                   ABSENT
`finding` on PATH                             ABSENT
```

**It is WELL WIRED as a library and has NO OPERATOR SURFACE.** An agent at a shell **cannot invoke
it**, so `br create` is the only path available — and that **inverts this rule's premise.** The
failure was never *"the author bypassed his own kernel."* It is **"the kernel has no operator surface
to bypass."**

**The verdict class was wrong, per gate rule 4a (`fh C69`).** "Zero callers" is an **ABSENT** verdict
— *build the callers*. The truth is a **MISSING BIN** — *build the operator surface*. Fourteen
manifest edges were sitting there the whole time the row said none existed.

**And this is the SECOND stale broken-kernel row hit in one session**, after `refill-idle-panes`,
which this file already records as *"CORRECTED — the kernel was FIXED and the doctrine outlived the
defect."* This file's own warning applies to itself: **a doctrine row asserting a kernel is broken
licenses routing around it indefinitely, so it MUST be re-measured before it is obeyed.** Two rows,
one night, both stale in the direction that excuses a handroll.

**The live gap is a missing `[[bin]]` on `crates/finding`** — that is the fix, and it is S1-authorized
work, not a doctrine note.

### The full loop, demonstrated end to end through kernels only

```
observe   tick-monitor observe --session omp-orchestrator   → dispatchable/free/attention/dead
dispatch  ntm --robot-send=omp-orchestrator --panes=5       → {"success": true}
receipt   tick-monitor observe                              → %1409 WORKING t=6
```

`t=6` from a previously IDLE pane is an **idle→working transition with a fresh timer** — the
strongest receipt available, per the receiver-receipt contract. No `capture-pane`, no `grep`, no
`send-keys`.

### Enforcement, because a written rule has failed five times tonight

- **Source half** — a gate scanning tracked files for handrolled equivalents, emitting `file:line`
  **and naming the kernel that should have been used** (a finding that does not name the
  replacement is not actionable). Its known-good leg is mandatory and non-obvious: the **kernel
  crates themselves must pass**, since `tick-monitor` legitimately calls tmux and
  `subprocess-contract` legitimately spawns — via a **declared, not inferred** allowlist.
- **Operator half** — the gate can only see committed source. It **cannot** see an operator
  handrolling in a shell, which is how all five above happened. That needs a `PreToolUse` hook and
  is a separate bead. **The source gate must say so in its own output** rather than implying
  coverage it does not have.


## Three graph and evidence rules that strangled real work tonight

### An epic OWNS its leaves via parent-child — NEVER a `blocks` edge onto its own leaf

A `blocks` edge from an epic onto a leaf it owns is **circular by construction**: the epic gates
the leaf, so the leaf cannot start until the epic closes, and the epic cannot close until its
children finish. **13 of the first 30 unassigned open beads were strangled this way, four of them
P0.**

`br show` reads `open` and unassigned and looks perfectly claimable. **The authority is ATTEMPTING
the transition and reading the refusal text** — quote it when reporting a blocker:

```
br update cp-u9ikt --status in_progress
  -> Error: cannot claim blocked issue: cp-epic-fleet-work-quality-08l6.74
```

**Find the writer before fixing the edges.** `br dep add <child> <parent>` **transposed** produces
exactly this shape, so repaired edges regrow while the writer still runs. Also: `br dep list <id>`
returns **OUT-edges only** — absence of an in-edge is not evidence of an orphan. And for triage use
`.triage.recommendations`, never `.quick_ref.top_picks` (it reports `unblocks=0` and omits
high-scoring beads); **skip epic containers**, whose PageRank accumulates from every child so they
top the list and can never close.

### A port that deletes a file invalidates every CLOSED bead that cited it

`control-plane@45c613d` deleted four scripts. All four were legitimately superseded and every citing bead was
**validly closed at the time**. Hours later it surfaced as `check.sh` close-evidence RED with
everything downstream UNRUN — a gate refusing every dispatch, far from the mistake.

**Before `git rm`, grep CLOSED beads for the path.** A closed bead's evidence is a live dependency
on the filesystem, not a historical note. Measured exposure: 2 beads via `close_reason`, plus
comment-level citations the raw count hides. Tracked as `cp-rjuzj`; the commit-time gate that would
have caught it at the point of the mistake is `omp-orchestrator-pre-delete-citation-check-igk`.

### READ THE CONSUMER BEFORE SCANNING FOR IT — and the harvester manufactures its own failures

> ## ⚠ CORRECTED 2026-09-07 — THIS ENTIRE SECTION IS ABOUT **CONTROL-PLANE**, NOT THIS REPO
>
> **`crates/close-evidence-gate` DOES NOT EXIST HERE AND NEVER DID.** Measured:
> `git ls-tree -r HEAD --name-only | grep -c close-evidence-gate` → **0**;
> `git log --all --diff-filter=D --name-only -- 'crates/close-evidence-gate/*'` → **0**, so it was
> never deleted either; `grep -rn CITED_PATH crates/` → **0 occurrences anywhere in this repo**.
>
> It lives at `/Users/josh/Developer/control-plane/crates/close-evidence-gate/src/blob.rs`. **The
> readings below are REAL — of the wrong repository.** They were published here as "this repo's
> close-evidence extractor" by the same agent that wrote the fifth rule about not confusing the two
> boundaries. **A BORROWED CLAIM INHERITS ITS AUTHOR'S BURDEN applies to a borrowed REPOSITORY too**,
> and the tell was in the text the whole time: the tracking bead is `cp-…`, control-plane's prefix.
>
> **THE OPERATIONAL HARM IS THE IMPERATIVE, NOT THE CITATIONS.** *"Write every path in a bead
> comment inside backticks"* was published as governing this repo. **There is no harvester here to
> evade.** Agents were instructed to obscure paths from a consumer that does not exist — and this
> repo's real consumer runs the other way: `%19` measured 14 closed beads citing a `bin/` or
> `.flywheel/` path ONLY in comments, one of them the very bead that created the citation gate, and
> `crates/no-shell-gate/src/bin/pre-commit-gate.rs:342` now reads those comments through
> `read_closed_beads_from_mirror`. **Here, a backticked path is a path the gate should still see.**
>
> **Retained deliberately, because the mechanism transfers even though the location does not:** read
> the consumer before scanning for it, strip comments and fenced code before matching, over-strip
> rather than under-strip, and never size an extractor from an inferred regex. Those are why the
> section stays instead of being deleted. **Do not act on its paths, counts, or the backtick rule
> inside this repository.**

The close-evidence extractor was twice sized from an **inferred** regex. Read from source
(`control-plane:crates/close-evidence-gate/src/blob.rs:59` — **not this repo**) it is:

```
const CITED_PATH: &str = r"(?:^|[^\w/.])(bin/[\w.-]+|\.flywheel/[\w./-]+)";
```

Three facts that only reading it establishes:

1. **It harvests `bin/` and `.flywheel/` ONLY** — not `crates/`. A scan including `crates/`
   overstated the problem by ~13×.
2. **The gate reads `close_reason` + `comments`, NOT `description`** (`grade.rs:44-51`: the `Bead`
   struct has no description field). So a path in a description cannot break the gate — and a scan
   restricted to `close_reason` still **understates** it, because comments count.
3. **Fenced blocks and inline code are blanked before harvesting** (`blob.rs:95-96`:
   `fence.replace_all` then `inline_code.replace_all`). So in **control-plane**, backticks are a
   mitigation: a path written `` `bin/foo.sh` `` is invisible to that harvester.

> **THE BACKTICK RULE IS CONTROL-PLANE-ONLY AND IS RETRACTED FOR THIS REPO.** The `WAVE.md`
> measurement below was taken with control-plane's stripping applied to this repo's file, which is
> why it reported 0 — it measured a consumer that never reads here.

**And 47 of 71 unresolvable citations are a REGEX ARTIFACT, not broken evidence.** Both alternations
end in a greedy class containing `.`, so a sentence-ending period is absorbed:
`.flywheel/HARVEST-LOOP-PLAN.md.` — which resolves the moment the dot is stripped. A further 13 are
prose fragments (`bin/a`, `bin/b`, `bin/crates`, the last a truncation at the next slash). **The
real broken-citation count is 9**, and editing bead comments to work around the harvester would
leave it live to re-manufacture the same rows forever. Tracked as `cp-cited-path-trailing-period-n5mkc`.


## Post-mortem: the fleet went idle for 6+ hours while every watchdog fired (2026-08-31, session post-wave)

The session's product claim — no session goes idle until Joshua says so — failed for ~6 hours
(roughly 10:00Z to 16:00Z) while every detection layer worked. The causal chain, each link
measured, not inferred:

1. THE CONDUCTOR FAMILY IS WIRED AND FIRING — and refused at one gate, for hours. Cron entries
   exist for controller-tick (18,38,58), fast-dispatch (*/5), loop-driver, refill-idle-panes,
   challenge-lane, fleet-monitor, reap-finished-panes. controller-tick's log tail at 15:59:51Z:
   "ADMISSION REFUSED — no fresh standing PASS at check-sh-ledger.json". fast-dispatch's log:
   "drift UNRUN skipped-after-close-evidence; tests UNRUN; mutation UNRUN". The fail-fast chain
   means ONE red gate makes every downstream gate UNRUN, and the admission verdict can never go
   green while any single gate is red.
2. THE GATES WENT RED FASTER THAN THEY WERE FIXED. The standing check-sh verdict failed at 10:00
   (docs-staleness: the staleness metric counts commits since the doc's last DISK WRITE, so a wave
   committing ~2/min re-stales AGENTS.md in ~25 minutes). Fixed by landing real findings (5107abc).
   Then close-evidence RED: 39+5 closed beads without audit-trail comments — backfilled (34 fixed
   by close-reason evidence patterns already present; 5 unfixable by comments because
   close-evidence-gate's bead source omits the comments field entirely — source.rs:223). Then
   bead-lineage RED. Each fix revealed the next red: the chain re-fails on the next gate every
   time, and at wave rate the admission verdict was red ~continuously.
3. THE DISK WALL MADE THE REST OF THE CHAIN UNFIXABLE. The `tests` and `mutation` gates require
   cargo builds; builds are refused at the mint floor (container 6.5-6.8% vs 8%,
   CARGO_MINT_CONTAINER_EXHAUSTED exit 75) — escalated to Joshua (cp-oakbv). The admission verdict
   therefore cannot go PASS regardless of gate fixes until disk headroom exists.
4. THE WATCHDOGS DETECTED AND FILED — AND THE P0s SAT OPEN. challenge-lane auto-filed
   cp-rjuzj ("close-evidence RED blocks all dispatch") and cp-vgine ("idle OMP capacity beside a
   ready queue") — both P0, both correct, both sat open for hours. dispatcher-deadman exists for
   exactly this class. Detection fired; the response layer does not exist: every lane fail-closes
   on admission, and no mechanism is authorized to act on a DEGRADED signal.
5. THE CONDUCTOR WAS A PANE. Pane 1 hand-routed work all night (four grades, two fixes, the
   blocker map) — the manual orchestration was load-bearing while the automated conductor was
   admission-blocked. When pane 1 investigated the blockers, routing stopped and seven panes went
   idle. The product's own claim (loop-driver: single-instance deadline-bounded conductor;
   refill-idle-panes: "an idle worker beside a ready queue is the conductor's failure") is that
   the conductor is a BINARY. The binary exists, is cron'd, and was refused — see 1-3.

THE NAMED MECHANISMS THAT PREVENT RECURRENCE (in order of leverage):
  M1 — TYPED DEGRADED DISPATCH: when admission is red, the conductor dispatches LOW-STAKES beads
       (grading/verification/hygiene — the classes that need no green admission) with
       admission=stale marked on the lifecycle row. Challenge-lane's own acceptance says
       "dispatch the idle panes, OR name why the queue is not eligible" — the naming has run all
       night and must be allowed to end in a dispatch for the work that does not need a green
       tree. High-stakes dispatch keeps the full gate.
  M2 — GRADING AS A DISPATCH LANE: `crates/m2-grading-lane` consumes `dispatch_saga::m2::route`.
       Lineage is `--profile` from argv, not `AGENT_NAME`. Decide-only by default; `--apply`
       would send. It does not close. IMPL→GRADING *status* remains `dispatch-saga grading-transition`.


  M3 — CROSS-SESSION ROUTING: panes idled in a session whose repo was admission-blocked while
       real work existed in the other repo (grades, backfills, doc currency). The conductor must
       route by WORK LOCATION, not by session membership.
  M4 — THE DISK WALL (cp-oakbv, with Joshua): the admission chain's tests/mutation gates
       physically cannot run below the mint floor. Until resolved, M1 is the only dispatch path.
  M5 — DOCS-STALENESS METRIC REDESIGN: a counter that re-stales in 25 minutes on a wave is a
       gate that is red ~forever when the fleet is most active. Measure staleness against
       substantive-commit classes, or gate it to a longer window during declared waves.

ALSO CORRECTED IN THIS POST-MORTEM (the truncated-instrument class, third instance tonight):
an early crontab read (head -10) reported controller-tick REMOVED from cron; grep found it at
line 11+. Read the whole instrument. An uncommitted-edit attribution was also corrected by the
orchestrator to a committed-land state (228f42a) — check git status at report time, not from
memory.

---
