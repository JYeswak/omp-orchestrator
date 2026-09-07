# `f2sh` grade — the dispatcher auto-escalating its own verbs

Grader: pane `%19` (PearlGate, claude). Implementer: `%8` (WildStone, codex). Non-author of bead and
code. Commits `b69ea7e` **and `3eff7c4`** — see "the tree moved twice under me". Date 2026-09-07.

## VERDICT: APPROVED — all six items met, including item 3, which was a residual at `%6`'s
## measurement and is now RESOLVED. Both mutations attributable, restore byte-identical.

## THE TREE MOVED TWICE MID-GRADE, AND I CHECKED RATHER THAN BLAMED THE INSTRUMENT

At grade start, `git log b69ea7e..HEAD -- crates/decision-ledger docs/decisions.jsonl` returned
**empty**. Twenty minutes later the same file's content did not match `b69ea7e`:

```
worktree / HEAD   dfd3440df3f4bc4a66919dbe
b69ea7e           848ff05138192300d4d5541b     <- DIFFERENT
```

My first hypothesis was `git log`'s history simplification — the documented `git log -S` trap. **It
was not.** `3eff7c4` has **one parent** and **is on the first-parent path**, so simplification could
not have hidden it, and re-running my exact original command now lists it. The timestamps settle it:

```
b69ea7e   committed 2026-09-07T18:10:01Z
3eff7c4   committed 2026-09-07T18:20:09Z     <- AFTER my check
84afb5a   committed 2026-09-07T18:22:03Z     <- HEAD moved again, different bead
```

**The subject changed because the implementer is still working, not because my instrument lied.**
That is the same shape as the error I made earlier tonight refuting `%20`'s manifest diagnosis on a
tree it had already fixed — and this time the correct move was to measure the timestamp instead of
accusing the tool. My earlier "no later commits touched those paths" was **true when measured and
stale within ten minutes**; it is retracted as a current claim.

The `edit` tool independently caught the same race: *"Recovered from a stale file hash… file changed
externally between read and edit."* I verified my mutation diff was **exactly one line** and clobbered
no peer work.

`3eff7c4` is squarely in scope — its subject line is *"prove backfill planner and narrow label
scope"*, which is precisely `%6`'s two open items — so this grade covers both commits.

## Items 1, 2, 5, 6

**Item 1 — MET.** The template literal has **0** occurrences in any `crates/*/src/*`:

```
git grep -n "Assign a holder, re-scope it, or park it" -- 'crates/*/src/*'   -> 0
POSITIVE CONTROL, files that DO contain it: .beads/issues.jsonl, AGENTS.md, docs/decisions.jsonl
```

The positive control matters: a zero from a pattern that can never match is not evidence. The string
survives in ledger DATA and in doctrine, which is correct — item 1 governs the **writer**.

**Item 2 — MET.** 21 rows carry a machine disposition with a named capacity reason:

```
13  capacity:no_eligible_pane
 8  transport:receipt_unproven;next_action=select-different-pane
```

Both of `%6`'s figures reproduce. **Note where the `8` lives:** in `disposition_reason`, not in
`question`. My first probe searched `question` and returned **0** — a wrong-field zero, not a real
one.

**Item 5 — MET.** 21 moved, 0 remaining; derived below rather than read.

**Item 6 — MET.** `nsx1` left open and stated; no self-close of another bead.

## THE READING THAT DECIDES THIS BEAD — `%6`'s "0 open" is the CRATE's verdict, not a choice

A per-line count of the ledger contradicts the report, and I want the contradiction on the record
because the naive read looks like a failure:

```
rows                          83   (62 before b69ea7e, +21)
auto-template rows            42   of which UNDISPOSITIONED  21
duplicate ids                 35   ids appearing twice
REQUEUED rows                 21   supersedes set on:  0 of 21
```

So a reader counting lines finds **21 auto-template rows still undispositioned** and concludes the
backfill did nothing. **That reader is wrong, and the crate says why.**
`decision-ledger/src/lib.rs:420-421`:

> *"The answer links are the append-log authority: a request is a candidate only when no response row
> already carries `answers=<id>`."*

The fix **appends an answering row** carrying `id == answers == <request id>` plus the disposition; it
does not edit the original. Under that semantics:

```
latest/answer-resolved view:  auto-template ids 21, undispositioned 0
                              latest undispositioned 25  <- exactly the title's "25 open"
```

**`%6`'s figures reproduce exactly under the crate's own resolution rule.** Three traps for the next
reader, all of which caught me first:

1. **`supersedes` is empty on all 21.** The mechanism is `answers`, not supersession. A grader
   looking for `supersedes` finds nothing and concludes nothing was resolved.
2. **35 ids appear twice by design.** Any id-keyed dedup — a `dict` keyed on id, as in my first pass
   — silently drops half the log. My "pre-existing rows whose disposition CHANGED: 0" was an artifact
   of exactly that.
3. **There is no `status` or `clause` key on a row at all.** My first derivation defaulted absent
   `status` to open and reported "83 genuinely open". Read the struct, not the shape you expect.

## ITEM 4 — ANTI-VACUITY. The concern is RESOLVED, with a positive control.

`%6` flagged `reconcile-dispatch --dry-run: candidates=0 moved=0 remaining=0` as *"indistinguishable
from a command that cannot see rows"*, and asked for a planted-row proof. **I built one independently
before finding that `%8` had landed the same thing.**

My differential oracle, replicating `dispatch_backfill_candidates` (`lib.rs:423-451`) over the two
pinned trees:

```
candidates BEFORE b69ea7e   21   (HD-0021 … HD-0041)
candidates AFTER  b69ea7e    0
original auto-template rows already self-answering BEFORE the fix:  0 of 21
   -> had that been 21, candidates=0 would have been VACUOUS. It was not.
POSITIVE CONTROL: plant one unanswered dispatch row -> candidates = 1  ['HD-PLANT']
```

**The zero is a measurement, not an incapacity.**

And `3eff7c4` adds the leg for it, `planted_dispatch_row_is_visible_to_dry_run_planner`, asserting
**both directions** — a planted row yields exactly one candidate with `AgentDisposition::Requeued`
and `capacity:no_eligible_pane`, and adding the answer link empties the set with the message *"an
answer link must make the dry-run candidate disappear."* It also extracts
`dispatch_backfill_candidates` from `main.rs` into `lib.rs` as `pub fn`, which is what makes it
unit-observable at all.

**Two independent derivations, same result.** That convergence is worth more than either alone.

## ITEM 3 — MET, and it changed under me in the right direction

The write-time guard is real and mutation-verified (`lib.rs:339-341`). On the label population,
`%6` measured 5 non-terminal rows carrying `human-decision` with no clause and leaned
*residual-with-disclosure*. **Re-measured now, that residual is gone:**

```
labelled human-decision   3   (was 9)   -- ALL THREE CLOSED: 9g5 (P0), f3p (P1), q10 (P1)
labelled agent-work      24   (was 18)  -- 6 relabelled
non-terminal human-decision WITHOUT a clause:   0 of 0
closed      human-decision WITHOUT a clause:    3
```

So the only clauseless rows left are **closed historical records** — exactly the category `%6` said
is a record defect and not a live cop-out. **Item 3 is MET for the population the guard governs**,
and I am recording it as MET rather than residual because the non-terminal set is empty, not small.

**`f2sh` is NOT in the label set**, which confirms `%6`'s own correction: its ninth match was a
grep artifact from the bead that documents the four clause words. I excluded it explicitly and
declared the exclusion, since a self-referential corpus is the ninth instance of that class tonight.

## MESSAGE EXACTNESS — `%6`'s question answered precisely, and the answer is a distinction

`%6` asked whether the message assertion is the exact text or a prefix. **Neither. The quoted
string is not an assertion at all.** `tests/ledger.rs:209-211`:

```rust
let error = append_request(&path, &request).expect_err("clause is mandatory");   // :209 PANIC MSG
assert_eq!(error, LedgerError::MissingField { field: "clause" });                // :210 EXACT, typed
assert!(error.to_string().contains("field=clause"), "{error}");                  // :211 SUBSTRING
```

- `:209` is an `expect_err` **panic message**. It fires only when the call **succeeds** — which is
  why it is what the mutation prints. It asserts nothing about the refusal text.
- `:210` pins the **typed variant** with `assert_eq!` — exact, and **stronger than any string
  check**, because it cannot pass on a different error and cannot drift when someone rewords
  `Display`.
- `:211` pins the rendered field name as a substring.

**So rule 7b is satisfied in the strongest available form: typed cause AND rendered field, neither
alone.** This is the same "BOTH, not either" conclusion I reached grading `2sx1`, arrived at from
the opposite direction.

**One precision residual, named not scored:** because the mutation makes the call **succeed**,
`:210` and `:211` are never reached during the mutation. The mutation proves the guard **exists**;
it does not prove the message assertions **bite**. A second mutation — changing `field: "clause"` to
another name — would exercise them. Not a defect in this bead; a sharper leg for whoever touches it.

## MUTATION — one mutation, one leg, disjoint by construction

```
guard neutered (request.clause.is_none() -> false)
   exit=101  contabo-3  bypass=0   11 passed; 1 failed
   RED = a_clauseless_request_is_refused_with_a_named_field   ONLY
   ledger.rs:209:49  clause is mandatory: Appended { id: "HD-0001" }
RESTORED  worktree sha == HEAD sha (dfd3440df3f4bc4a66919dbe), git status clean
   exit=0  contabo-1  12 passed / 0 failed
```

`%6`'s reported evidence reproduces byte for byte. **One mutation, one leg**, so rule 7b's partition
requirement is satisfied without needing the disjointness argument `djfu` requires.

## Lane

All figures ON-LANE: `Remote command finished: exit=` present and the `RCH BYPASS` banner absent in
every log, checked per run. Verdict work only — **no triple passed**, per the corrected binding.

The restore run was refused twice on `contabo-3` with
`[RCH-I005] … refused (project_excluded)` and succeeded on `contabo-1` after dropping the pin.
**That was the right move under the corrected rule**: `active_project_exclusion` was a peer's
in-flight build of this project, and waiting on someone else's build is wrong — request another
worker.

## NO-CLAIM

- **`docs/decisions.jsonl` was not re-derived from the ledger's own reader**, only from its data plus
  the crate's documented predicate reimplemented in Python. That is a differential oracle, not a
  proof that the compiled reader agrees.
- **The 3 closed clauseless rows are a record defect I am disclosing, not fixing**, and not scoring
  as a failure. If a future guard ever validates historical rows, they will fail it.
- **`3eff7c4` is graded from source and from its tests' pass on lane**, not from a separate mutation
  of its own new leg. I mutated the clause guard, not the planted-row planner.
- **HEAD moved again to `84afb5a` while I wrote this.** Every figure above is pinned to a named tree
  or to `HEAD` as of `dfd3440d` for `decision-ledger/src/lib.rs`; nothing here is a claim about "the
  repo right now".
- **The `100755` mode bit on `crates/decision-ledger/src/lib.rs` is pre-existing at HEAD**, not
  introduced by my edits. Flagging it because a source file with the executable bit set is the same
  write-tool artifact recorded earlier on `ompo-doctor/src/main.rs`.
