# 11 — The lifecycle: idea to shipped, walked down the crates and the skills

**R13, added by Josh mid-grading:** *"part of our plan needs to be intimately aware of the entire
lifecycle of an idea to a finished project then walk the list down the crates and our skills to
ensure that throughout dispatch we have proper templates, proper dispatch, proper reap, proper
logging, proper build grading, etc."*

No section was written against R13, so this one is new and its findings are mostly **absences**.
That is the expected result and it is the point: the plan had ten sections describing *parts* and
none describing the *spine*. A reader could finish §00–§10 knowing every crate and still not know
what happens between "someone has an idea" and "it shipped."

Every claim here is `MEASURED` unless marked `PROJECTED`.

---

## 11.1 The nine stages, and who owns each

| # | stage | owning crate(s) | covering skill | measured state |
|---|---|---|---|---|
| S1 | **idea** | none | `/idea-wizard` | **NO CRATE.** Ideas arrive in chat and die there — the failure R11 was written to stop |
| S2 | **plan** | none | `/planning-workflow` | **NO CRATE.** This document is the artifact; nothing enforces its shape |
| S3 | **bead** | none (uses `br`) | `/beads-workflow` | wrapped, not owned. Close-policy lives in `br` |
| S4 | **select** | `loop-queue-filter` | `/beads-bv` | crate exists; **`Command::new("bv")` appears nowhere** — the graph is never called |
| S5 | **dispatch** | `dispatch-claim-fence`, `pane-dispatch-fence`, `fast-dispatch`, `tick-dispatch` | `/ntm-fleet-monitor` | four crates, one fence up 4.2h, **actuation is a human** |
| S6 | **work** | — | `/vibing-with-ntm` | the agent's own business; we observe only |
| S7 | **reap** | `reap-finished-panes` | `/vibing-with-ntm` | crate exists; the only reap this session was hand-run |
| S8 | **grade** | `verify-dispatch`, `ack-spine`, `ack-stage` | none | grading is prose; **6 Verdict types, no shared trait, nothing countable** |
| S9 | **ship** | `installer`, `commit-build-fence` | `/installer-workmanship` | installer covers 3 of 18 binaries; `--install` never run against a foreign host |

**Four of nine stages have no owning crate.** S1, S2, S6 arguably should not — an idea is not a
program. But **S3 has none and should**: the bead lifecycle is where our close-policy discipline
lives, and it is entirely delegated to `br`, which means our rules about evidence-bearing closes
are convention, not code.

---

## 11.2 The five dispatch properties Josh named, measured per stage

For each stage: is there a **template**, a **dispatch** mechanism, a **reap**, **logging**, and
**build grading**? `Y` = exists and was used this session. `y` = exists, unused. `—` = absent.

| stage | template | dispatch | reap | logging | build grading |
|---|:--:|:--:|:--:|:--:|:--:|
| S1 idea | — | — | — | — | — |
| S2 plan | — | — | — | — | — |
| S3 bead | — | n/a | — | `Y` (`.beads/issues.jsonl`) | — |
| S4 select | — | n/a | n/a | — | n/a |
| S5 dispatch | **`y`** | `Y` (hand-rolled) | n/a | partial | n/a |
| S6 work | n/a | n/a | n/a | — | n/a |
| S7 reap | — | n/a | `y` | — | n/a |
| S8 grade | — | n/a | n/a | `Y` (bead comments) | **`Y`** |
| S9 ship | — | — | n/a | — | `Y` |

**Reading the table honestly: 3 cells are `Y`-and-used out of 45. Two are `y`-exists-unused, and
those two are the most damning, because they cost nothing to adopt.**

---

## 11.3 The template we own and did not use

`ntm template list` returns four templates. One is exactly the thing this session hand-rolled
roughly thirty times:

```
Name:        dispatch
Description: ZestStream controller dispatch packet — bounded assignment with proof obligations
             and a named …
Path:        /Users/josh/.config/ntm/templates/dispatch.md
Variables:
  - objective (required)   ONE outcome, stated as a result not an activity
  - target    (required)   Absolute repo/worktree path, and the bead ID
  - why_now
```

Every dispatch packet in this session was written by hand into a `/tmp` file and sent with
`tmux send-keys -l`. The template has **required variables that fail closed on omission** —
`objective`, `target` — and `ntm send -t dispatch --var … --dry-run` performs full substitution and
refuses on a missing required variable.

**Two dispatch defects this session would have been structurally impossible with it.** The
`5rh`-to-`%1413` packet named an **unclaimed bead** — the missing file → CLAIM → dispatch middle
beat — and `target` is a required variable pairing the path *with the bead ID*, which is where a
claim check belongs. And the `omp-coverage-mission-ipg.4` packet was sent without the claim written
first, for the same reason.

That is the sharpest instance of `BUILT ≠ WIRED` in the lifecycle: not a mechanism nobody built,
but **a mechanism already built, already correct, and never invoked by the person who most needed
it.** `NO-CLAIM:` I have not verified that the template's body would have caught these two cases —
only that its required-variable contract addresses the field that was wrong in both.

---

## 11.4 The build-grading hook is a shell script, and the rule cannot see it

The repo's one hard rule is **no `.sh`, no `.py`**, enforced by `no-shell-gate` with an empty
exemption list. Measured:

```
ls -la .git/hooks/*.sh
  .git/hooks/commit-msg-verification-level.sh   6288 bytes

git ls-files | grep -c commit-msg-verification
  0
```

**The script that enforces our build-grading discipline is a 6.3 KB shell script**, and it is
invisible to the rule because `no-shell-gate` scans the git index. The gate is not defective — it
declares this boundary itself, at `crates/no-shell-gate/src/lib.rs:14`: *"this gate covers FILE
EXTENSIONS of tracked files, nothing else."* An honest, stated limit.

The finding is about **coverage, not correctness**: our most consequential policy hook lives in the
one directory the policy cannot reach. Three readings, and the plan must pick one:

1. `.git/hooks` is legitimately outside the rule — hooks are machine-local, never distributed, and
   a shell hook is the ecosystem norm. Then say so in the rule, because right now the rule reads
   absolute and is not.
2. It is a real violation and the hook should be a Rust binary like the other five gates. Note the
   installed `pre-commit` already *is* Mach-O, so precedent favours this.
3. It is a violation we accept with a named reason and an owner — the allowance-row shape.

**Silence is the one unacceptable option**, and silence is the current state. Registered as **Q13**.

---

## 11.5 Where the lifecycle actually breaks

Reading §11.1 and §11.2 together, the spine has three severed links, and they are not the ones the
crate list would suggest:

**S4 → S5, selection to dispatch.** `loop-queue-filter` exists and `bv` provides the graph, but
`grep -rn 'Command::new("bv")'` over `crates/` returns **no matches** — measured. Nothing in this
workspace ever invokes the ranking tool. Selection this session was the conductor picking by
recency of his own discovery, which `/beads-bv` names as the cherry-picking pathology, and the
graph disagreed: the top-3 PageRank items sat unclaimed.

**S6 → S7, work to reap.** There is no worker→conductor completion path at all. §10 Gap 7 measured
that this is **precedent-free across 210 mirror work-trees** — `SupervisionEvent` has 8 variants and
`StopReason` 6, and not one of the 14 means *"the worker finished."* Every completion tonight was
found by a human looking.

**S8, grade, has no template and no type.** Grading produced four long prose documents this
session. `omp-types` exists to fix exactly this and has **zero dependents**, and the half of the
vocabulary that would collapse the ack dialects is blocked upstream behind
`messaging-fabric → test-internals` (issue #46). So the type that would make a grade *countable*
is not merely unadopted — it is unreachable at our pinned rev.

---

## 11.6 What R13 demands that is not yet built

| # | demand | state |
|---|---|---|
| L1 | every dispatch goes through the `dispatch` template, not a hand-rolled file | **not adopted** — template exists, unused |
| L2 | selection calls the graph | **not wired** — zero `bv` invocations in the workspace |
| L3 | a worker can signal completion in a typed way | **precedent-free**; the largest gap in the plan |
| L4 | a grade is a value, not a document | **blocked upstream** at the feature boundary |
| L5 | reap runs before refill, mechanically | **hand-run once**; `reap-finished-panes` exists uncalled |
| L6 | every stage writes a durable record | **3 of 9 stages log anything** |
| L7 | the build-grading hook obeys the repo's own rule, or names its exemption | **unresolved**, Q13 |

---

**NO-CLAIM.** This section maps nine stages and measures five properties across them. It does not
establish that nine is the right decomposition — S1/S2/S6 may not want crates at all, and a
different cut might make the severed links appear elsewhere. It does not measure how often each
break actually costs anything; the `bv` gap is measured as *never invoked*, not as *decisions made
worse*, and those are different claims. Every `PROJECTED` remedy in §11.6 is unbuilt and unowned
except where an owner is named.
