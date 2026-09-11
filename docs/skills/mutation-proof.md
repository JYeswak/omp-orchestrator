# mutation-proof — every gate proves it bites

Triggers: "add a gate", "write a leg", "mutation", "known-bad", "fires on",
"prove the check works", "over-strict", "anti-vacuity", "ceiling", "ratchet".

## Pattern
A gate ships with four legs, each mechanical:
1. **Fires-on-known-bad**, with a message AND an exit code pinned together — pinning either alone is defeasible in a direction measured in this repo.
2. **Known-GOOD leg** — an attack-only suite ships an over-strict gate, and an over-strict gate gets routed around.
3. **Mutation leg** — break the thing the gate keys on; the leg must go RED naming the break; restore byte-identically (`sha256sum -c`). A mutation that leaves green means the gate is decoration.
4. **Anti-vacuity** — an empty scan set is an ERROR, never a pass. A deliverable never checked reports identically to one that passed.

## The parent rule: a claim about content must cite a content HASH

**Both standards in this file reduce to one sentence, and they were derived independently from
opposite ends on 2026-09-11 — the revert rule says *pin the bytes, because HEAD and the index move
under you*; the surface rule says *name the blob, because the path moves under you*.**

> **Worktree, index and HEAD are all MOVING POINTERS. A pointer cannot carry a claim about bytes.**

**Measured on this file, in one evening, by four agents:**

```
worktree  7686 -> 10677 -> 11279 -> 13637 B    four edits
index     4001 -> 11279 B                      the author NEVER staged it -- and the stager is
                                               UNIDENTIFIED, because git records no author for the
                                               index. "not me" is not "a peer".
HEAD      0 B, and it moved five times elsewhere in the same checkout
```

**So `git show :<path>` is NOT a fixed surface.** A census published against it — *"six of seven
files carry zero"* — was true of the **4001-byte blob** and is false of the **11279-byte blob at the
same `:<path>`**. The correction `name the surface, not the path` was one rung short of its own
conclusion; the settling form is `git show <sha>`, or a hash recorded at a stated instant.

**And the correction to that correction is the rule eating its own tail, which is why it is here:**
the first published form said *"re-staged by a PEER"*. The author had measured only that **it did
not stage the file** and inferred an actor from a state change — **git has no author field on the
index, so nothing could contradict it.** The honest form is *"the index advanced and the stager is
unidentified"*, which is **weaker in attribution and stronger for the rule**: an unowned surface
moved by an unidentifiable party is worse than one moved by a nameable peer. **A moved pointer is
not evidence of who moved it.**

> **A plausible actor stops the asking** — the fourth terminator, and like the other three it is a
> *success* that ends the investigation. An agent that abstains only when no candidate comes to mind
> has not got a discipline; it has an empty candidate list.

**The specimen is the sharpest part: the agent restating the rule printed a hardcoded label
`(staged blob, unchanged all hour)` beside a number it had measured exactly once — inside the
command demonstrating surface discipline.** A label whose text describes a predicate its command
never evaluates. *A workaround that works stops the investigation; a correction that lands stops the
measuring; **a label that agrees with you stops the reading.***

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A leg pins the message but not the code. A publisher returning no id still printed `FINDING_PUBLISH_FAILED` while the exit collapsed 5→3 — a message-only leg stays GREEN while two causes merge into one code. | Cause-collapse hides behind the right string. | Pin both; the two failure directions (same code/different message, same message/different code) are symmetric and both measured. |
| A leg pins the code but not the message. An unrelated workspace-load outage returned the same 101 as the planted known-bad with a completely different message — a code-only leg ticks the box on the wrong evidence. | Any unrelated breakage goes green. | Grep the specific string AND print the output so a reader sees which cause fired. |
| A ceiling is raised to silence the gate. Raising 44→100 (both anchor fields in lockstep) stayed green because the anchor asserted a relation between two editables, not a relation to reality. | The ratchet becomes a budget nobody is near; growth goes silent. | Bind both sides to the measurement (`live <= CEILING <= live+TOLERANCE` with a named, justified tolerance); the lockstep mutation must redden. |
| An `#[ignore]`d leg is counted as coverage. The only leg validating production data was ignored with zero in-tree callers of `--include-ignored` — default `cargo test` reported green while production went unchecked. | BUILT ≠ WIRED at test granularity. | If a leg must be opt-in, something in-tree opts in; otherwise delete the ignore or the leg. |
| A prefix is asserted instead of the message. A mutation trap pinned the wrong DECISION and a prefix while the mutation moved the exact reason text underneath — the message half was decorative. | The mutation the leg was built for survives a prefix assertion. | Assert the substring the mutation actually moves. |
| Two mutations redden overlapping leg sets and are called independent. One shared test reddening under both proves at least one test conflates two properties. | Correlation masquerades as independence; the defect the pair was supposed to rule out survives. | Partition: one leg asserts the code and says nothing about the decision, another the reverse; a disjoint redden is then a measurement. |
| **A verified measurement is RETRACTED because a peer's figure disagrees, without re-running either.** An agent censused seven files with `git show :<path>` — the correct surface — got the right answer, then disowned it in public when a peer published a contradicting count taken from the worktree of a file another agent was editing. | Every other failure here is accepting an unverified claim; this one DISCARDS a verified one, and it is worse: the room's discipline rewards self-correction, so **a false retraction arrives wearing the costume of rigour and nobody challenges it.** Four agents then propagated the wrong census, and the retraction was one of its load-bearing supports — it read as the original author conceding. | Re-run before you concede. The discriminator here was one `wc -c`: six of seven files were byte-identical staged-vs-worktree and one was 4001 vs 7686. **Name the surface, not the path** — `git show :<path>` for staged, `git show HEAD:<path>` for committed, never the worktree path in a tree many agents are writing to. |
| **A verified measurement is CORRECTED by a peer whose figure came from a different surface, delivered as a finding rather than a disagreement.** The challenger opened with *"off by one file"*, attached a real positive control, and had the discriminating anomaly in its own output — six files silent, one loud, which is the signature of a live edit and not of a corpus. | **A retraction wears the costume of rigour; a CORRECTION wears the costume of diligence, and it is the more forceful of the two.** Rigour applied to the wrong object reads as rigour: the control was genuine and the surface was wrong, which is the worst combination available. This is what *causes* the false retraction — a false retraction needs a challenger. | **A retraction is a measurement — and so is a correction.** Both need the instrument of the claim they move. Before correcting a peer's figure, ask the one question that ends it: *which surface does the claim name?* |
| **The pin covers the MUTATED file only.** A mutation reverted cleanly on `src/lib.rs` with a clean pin proves nothing about the leg that graded it — a mutation plus a quietly relaxed assertion reads GREEN under a source-only pin. | The pin on the code interrogates the code; only a pin on the TEST interrogates the test. A single-file pin cannot distinguish *the mutation was reverted* from *the mutation was reverted and the assertion was relaxed*. | Pin EVERY file in the unit, including the test you are proving with. The pair then also witnesses "I did not disturb the leg while proving it." |
| **A mutation that hits the wrong line returns `exit=101` with NO `test result:` line — a COMPILER red impersonating an assertion red.** A misplaced edit clobbered an `Ok` wildcard arm and produced `E0004 non-exhaustive patterns`; the exit code was identical to a correctly-planted known-bad. | **An exit code alone would have read as a successful mutation.** A rustc red proves the file no longer compiles, which is no evidence at all about the gate — and it is the same 101 the real known-bad produces. | Require BOTH proof lines. The presence of `test result:` is what separates an assertion red from a compile red; its ABSENCE is the tell. Void the window, restore the pin exactly, re-apply on the correct line — never salvage the reading. |
| **Identity measured at CLOSE is reported as if it bracketed the window.** A grader verified its test files byte-identical to HEAD *after* the run and had taken no pre-pin on them — so an edit-and-restore inside its own window is not excluded by that evidence. | **A value measured once is not a value held across an interval.** Same-value-at-the-end and unchanged-throughout are two claims, and only the second needs a pin at both edges. Vanishing likelihood is not proof. | Pin at BOTH edges or state which property you established. "Byte-identical at close" is honest; "unchanged during the window" requires the pre-pin you did not take. |

## Negative evidence

| Row | Provenance |
|---|---|
| Pin-both rule, both failure directions | AGENTS.md gate rule 7 (+7b); `%19` M1 / `%20` legs 2026-09-07 |
| Ceiling 44→100 lockstep raise stayed green; tolerance-band remedy | bead zhr29#c3357 + revision 19a5c4b |
| Comment-blind detector arm (reachability without WHAT) | bead udtqk#c3363; revision cfb6e1a |
| `#[ignore]`d production leg, zero `--include-ignored` callers | AGENTS.md gate rule 6, 2026-09-02 |
| Growth-only ratchet + raisable ceiling + dirt-misread (48 hits) | bead-mining C09: zhr29, udtqk, hw99, vdxb, 8r0r |
| **RETIRED HABIT: an absence grep alone passes a DELETED guard as a clean revert** | measured twice 2026-09-11, two crates, two agents — see below |
| **RETIRED HABIT: bare `git diff -- <file>` passes a STAGED difference as a clean revert** | measured 2026-09-11 on a live staged `AGENTS.md` — see below |
| **RETIRED HABIT: `git diff HEAD` compares against a MOVING HEAD** | measured 2026-09-11 — HEAD moved five times in one checkout that evening |

## The revert is a claim too — and four instruments cannot verify it

**Measured 2026-09-11 on two crates by two agents, independently, and it retires a habit every agent
in this fleet was using.** A BSD `sed c\` replacement consumed both lines of a guard and inserted
nothing, so the guard was **deleted** rather than restored. The mutation-absent grep returned `0` —
**a clean revert by that instrument.** Only the pin caught it.

```
one-sided ABSENCE grep for the mutation string   ->  0            reads CLEAN REVERT
the PIN at the same instant, sha                 ->  CHANGED      deletion VISIBLE
```

Reproduced deliberately on a second crate: planted a deletion, absence grep `0`, pin
`58095c2d -> 7198711b`, restored to `58095c2d`.

**Why it is not merely weak — it is asking an unrelated question.** The absence check is a predicate
about the **MUTATION**; the pin is a predicate about the **FILE**. A deletion is not the mutation
string, so only the pin can see damage the mutation never named.

**And the failing form returns a clean answer rather than an error**, which is why it is retired
rather than cautioned. An `#[ignore]`d leg announces itself; this does not.

### The third hole: `git diff -- <file>` compares to the INDEX, not to HEAD

**Measured 2026-09-11 on a live staged file, no planting required.** A file that is STAGED reads as a
clean diff while differing from HEAD:

```
git diff -- AGENTS.md          (worktree vs INDEX)   ->  0 lines     reads CLEAN
git diff HEAD -- AGENTS.md     (worktree vs HEAD)    ->  29 lines    actually differs
blob worktree 186035ec…  !=  blob HEAD a18cea65…
CONTROL, an UNSTAGED dirty file: both forms return 314 — they agree only when nothing is staged
```

**So the two forms agree on unstaged paths and diverge completely on staged ones.** Revert a mutation
in a path that someone else has also staged, verify with bare `git diff`, and you get a clean answer
from a tree that differs from HEAD. **In a shared checkout you do not control what is staged.**

**The comparison must NAME HEAD, or the index can answer for it.**

### Order the report the way the evidence runs

The pin is primary, the grep is corroboration. Two agents reported *"mutation string absent, sha
matches"* — grep first — and an agent copying the **reported** method rather than the commands
inherits the insufficient half. **The reporting order was the defect; the protocol was sound.**

**And a correct practice defended by a premise that does not carry it is its own hazard.** One agent's
nine reverts were safe because it compared against `git show HEAD:<path>`, while its stated reason
named `git diff -- <file>` — a reader adopting the reason inherits the hole. Its own line is the one
to keep: **correct practice adopted for the wrong reason is not the same as knowing why it is correct,
and it fails the moment the unrelated reason goes away.**

### The fourth hole: `git diff HEAD` compares against a target that MOVES

**Measured 2026-09-11: HEAD moved five times in this checkout in one evening.** A revert verified
against HEAD at 20:10 and re-read at 20:40 is comparing to **two different trees**, and the second
comparison can come back clean for a file a peer has since committed.

```
absence grep     interrogates the MUTATION              -> passes a deletion
git diff -- <f>  interrogates worktree vs the INDEX     -> passes a staged difference
git diff HEAD    interrogates worktree vs a MOVING HEAD -> sound only while HEAD stands still
git hash-object  interrogates the BYTES against a pin you took yourself
                 -> invariant under staging AND under HEAD moving
```

**A pre-recorded blob is a statement about bytes that no later commit, staging or reset can satisfy
retroactively.** Both sides read the file from disk; git is never consulted. That is the only form
immune to all four holes, and it is what the 1598-commit extraction already prescribed two lines
above — **the corpus had the practice; this session found what defeats the substitutes.**

⚠️ **SPECIMEN, NO PLANTING REQUIRED: this file is itself a staged path.** While it is staged,
`git diff -- docs/skills/mutation-proof.md` returns **0 lines** while `git diff HEAD` returns 105.
**The document that retires the bare-diff habit is a live instance of it.** A reader verifying this
edit with the form this edit retires gets a clean answer from a tree that differs from HEAD.

## Check
```
# after any gate change, on the lane:
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p <crate> 2>&1 | grep -E "test result:|Remote command finished:"
# per changed leg: plant the known-bad, paste RED, restore, paste GREEN
#
# the revert itself. THE PIN IS THE PRIMARY CLAIM -- everything below it is corroboration:
git hash-object <file>                 # BEFORE the mutation. Record it. This is the pre-pin.
git hash-object <file>                 # AFTER the restore. MUST equal the pre-pin, byte for byte.
#
# corroboration only, each with a hole named:
git diff HEAD -- <file>                # empty. NOT bare `git diff` (index answers) and NOT sound
                                       #   if HEAD moved between your pre-read and this check
grep -c '<mutation string>' <file>     # 0. Cannot see a DELETION -- asks about the mutation, not the file
grep -c '<original string>' <file>     # nonzero, SAME grep shape. This is what catches an over-deletion.
```
