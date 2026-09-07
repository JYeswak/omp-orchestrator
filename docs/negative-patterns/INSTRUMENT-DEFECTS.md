<!-- A RECEIPTS ARCHIVE, NOT DOCTRINE. The RULES live in AGENTS.md. -->

# Instrument defects: the receipts

**The rules live in `AGENTS.md`. This file holds the evidence that earned them.**

Every row is a measured instrument defect: a probe, tool, or command that answered a different
question than the one asked, plus the measurement that caught it. `AGENTS.md` carries each rule as
an imperative and links here for the receipt.

## Why this file exists

`AGENTS.md` reached 214,681 bytes against this repository's own document contract of <= 25 KB
(`docs/contracts/asupersync_process_grade.md`, corpus median 7,132 B) -- 8.6x over. That contract's
reasoning is that a document which cannot be read in one sitting gets graded in ROUNDS, and a round
finds drift rather than absence. **This is a compression of existing content, not a new artifact:**
the same bytes, moved to where their length is not a defect.

## READ THIS BEFORE CITING A ROW

These narratives were **moved verbatim and deliberately NOT re-measured**. Every command, figure,
path, sha and quote stands as written -- including any that has since gone stale. Several rows are
themselves corrections of earlier rows, and at least one figure is known to have been overtaken by
the act of reporting it (that is `8m`, the rule about exactly this). **Re-derive before you rest a
decision on any number here.**

## The general form is `8i`

Run a negative control on your instrument before you believe its answer. Point it at a
guaranteed-absent subject; if "absent" and "present-and-fine" produce the same output, the
instrument cannot answer your question and its verdict on the real subject means nothing.
`8b` through `8h` are instances of it; `8j` through `8o` are it aimed at specific tools.

## Provenance

Extracted 2026-09-07 from `AGENTS.md` at sha256 `2fffc00bfd0add3d`, by content anchor (`8b. **` through
`9. **`) rather than line number -- the file was being concurrently rewritten and the block moved
47 lines during extraction, which is `8g` in miniature.

---


## 8b — A RETRACTION THAT LIVES ONLY IN A REPORT GETS RE-PROPOSED BY THE NEXT READER. Put it in the

8b. **A RETRACTION THAT LIVES ONLY IN A REPORT GETS RE-PROPOSED BY THE NEXT READER. Put it in the
   source, and give it a test.** `%20`, 2026-09-07, in its own words after I refuted its arc
   hypothesis.

   It had proposed a switch-on precondition of *"until `jplf.1` is selectable."* I dropped the edge
   it blamed, measured that `jplf.1` stayed invisible, and found the real cause: `jplf.1` is an
   **epic**, and `br ready` excludes epics fleet-wide — 0 of 32 open epics offered, correctly, since
   a container is not work. **So the precondition was unsatisfiable by construction** — the
   never-fires class, proposed in good faith and about to be shipped as a gate's trigger.

   **What `%20` did with the retraction is the rule.** It did not merely accept it in a report. It
   shipped `SWITCH_ON_PRECONDITION` as a **string in the binary** reading *"at least one NON-EPIC
   arc member is present in `br ready`; never keyed on jplf.1, which is an epic…"*, plus a test
   named `the_switch_on_precondition_is_not_keyed_on_an_epic` **whose job is to keep the REFUTED
   keying named** so nobody re-adopts it. Verified: 4 `SWITCH_ON_PRECONDITION` sites in source, the
   test present.

   **Why the test matters more than the string.** A comment recording a retraction is prose, and
   this file has an entire section on prose retractions being re-adopted — a stale *"this kernel is
   broken"* note licensed hours of hand-rolling after the kernel was fixed. **A test that fails if
   the refuted form returns cannot be read past.**

   The general form: when a premise is refuted, ask **where the next reader will look**. If the
   answer is "the code", the retraction belongs there. A NEGATIVE_EVIDENCE row, a bead comment and a
   commit body are all *findable*; only a failing test is *unavoidable*.

   **NO-CLAIM.** This keeps a refuted form from being silently re-adopted. It does not make the
   replacement correct — `%20`'s new leaf-keyed precondition is measured-satisfiable today
   (`jplf.9` is in `br ready`), but one row is not a moving arc, and it said so.


## 8c — AN EXCLUSION RECORD WITH NO CORRESPONDING LIVE ROW IS STALE STATE, NOT CONTENTION — and

8c. **AN EXCLUSION RECORD WITH NO CORRESPONDING LIVE ROW IS STALE STATE, NOT CONTENTION — and
   treating the two the same converts a five-second retry into a five-minute wait.** `%20`,
   2026-09-07, measured within five minutes of the rule being written down.

   ```
   [RCH-I005] refused (project_excluded); 'contabo-3' already runs this project
   rch queue --json  ->  active builds: 0
   ```

   **Zero active builds, so the exclusion was attributable to no live build — therefore not its
   own.** It unpinned the worker and the run succeeded on `contabo-1` **first try**. Under the
   previous reflex — *"`active_project_exclusion` means wait for your own job"* — it had waited
   three times earlier the same evening.

   **The discriminator is the PROJECT FIELD in the live queue, not the exclusion message.** If the
   in-flight build is someone else's, waiting is wrong; if there is no in-flight build at all, the
   record is stale and waiting is wrong for a different reason. Only *"the queue names a live build
   of my project"* justifies waiting.

   **This generalises past rch.** Any admission surface that records a reservation separately from
   the work it reserves for can hold a record whose subject is gone — the leaked-lease family, the
   phantom 4/4 slots on an idle box, the `RCH-I005` that survives a killed client. **Ask what the
   record is a record OF, and check that thing directly.**


## 8d — A TEST NAME IS A CLAIM, AND A NAME PROMISING A PROPERTY IT DOES NOT CHECK IS THE QUIET FORM

8d. **A TEST NAME IS A CLAIM, AND A NAME PROMISING A PROPERTY IT DOES NOT CHECK IS THE QUIET FORM
   OF THE CONJUNCTIVE DEFECT.** `%20`, same session, renaming its own test.

   `an_unattributable_session_log_holds_rather_than_guessing` → `..._is_unknown_with_the_unknown_code`.
   In its words: **"'Holds' was decision language on a body that asserts the outcome and the code, so
   the name promised a property it never checked."**

   Rule 7b catches the loud form — a name joining two properties with `and` is a conflated assertion
   advertising itself. **This is the quiet form: a name asserting a property the body never
   touches.** It is worse in one respect, because a reader scanning names for coverage counts it as
   covered.

   **The check is mechanical: read the name, then read the assertions, and confirm the name names
   only what the body asserts.** `%20` verified nothing was lost before renaming — the decision half
   was already covered by a separate across-every-shape leg carrying that exact evidence row.


## 8e — THE LANE SENDS YOUR WORKTREE FOR TRACKED PATHS ONLY — so `RCH-E410` has TWO causes, and

8e. **THE LANE SENDS YOUR WORKTREE FOR TRACKED PATHS ONLY — so `RCH-E410` has TWO causes, and
   rule 8 above applies ON THE REMOTE LANE.** Measured 2026-09-07 by `%19` and `%20` independently,
   from opposite ends of one fleet-wide outage. Every pane's remote build refused for roughly an
   hour with `RCH-E410 DependencyPreflightMissing` — *"remote dependency preflight found a missing
   required path"* — naming a path most of them had never heard of.

   **Cause A — a declared entrypoint that does not exist yet.** `%20` watched the refusal WALK:

   ```
   retry 1   missing crates/gate-runner/src/main.rs
   retry 2   missing crates/gate-runner/tests/roster.rs
   retry 3   COMPLETE -> ran
   ```

   `rch`'s preflight requires **every declared entrypoint to exist**, while **cargo's own resolution
   is lazy** — it does not need `tests/roster.rs` until you build that target. So a manifest
   declaring `[[bin]]` and `[[test]]` paths before the files exist **passes `cargo metadata`
   locally and refuses every remote build**, and the refusal walks from one declared path to the
   next as the author writes them. That is sharper than this file's earlier *"a `Cargo.toml` without
   a `src/`"* framing, which describes only the first step of the walk.

   **Cause B — the path exists but is UNTRACKED.** `crates/gate-runner` had all its files on disk,
   `cargo metadata --offline` loaded clean, and `git ls-files` returned **0**. `rch` ships an
   overlay built from git, so an untracked directory does not travel.

   **THE BOUNDARY IS TRACKED vs UNTRACKED — NOT INDEX vs HEAD.** I posed exactly that question and
   declined to guess; `%19` settled it by experiment and **refuted its own first conclusion:**

   ```
   experiment 1   staged, deliberately NOT committed (absent from git ls-tree -r HEAD)
                  -> lane exit=0, 1 passed        the worker ran a target in no commit
                  -> concluded "the overlay archives the INDEX"   <- WRONG

   experiment 2   the negative control it nearly skipped:
                  index holds 1 #[test], worktree holds 2 (the second UNSTAGED, panicking)
                  -> lane: "running 2 tests", exit=101, 1 passed; 1 failed
                  -> AN INDEX ARCHIVE WOULD HAVE RUN ONE TEST
   ```

   ```
   untracked path                       does NOT travel     <- the outage
   staged (A )                          travels
   UNSTAGED edit to a git-known path    travels
   ```

   **So `git add` alone is the minimal fix** for cause B — no commit required — though committing is
   still right, because a tracked-but-uncommitted crate is invisible to a fresh clone.

   **AND THE CONSEQUENCE WORTH MORE THAN THE UNBLOCK: A LANE FIGURE DESCRIBES YOUR WORKTREE, NOT
   `HEAD`.** Rule 8 says `cargo` reads the WORKTREE while a sha names a TREE. **That holds on the
   remote lane too** — which everyone here, including me, assumed the clean overlay removed. It does
   not. A grader citing an on-lane `exit=0, N passed` against a sha has **still mixed two tree
   states** unless it pinned the tree, exactly as if the run were local. In a five-agent shared
   checkout the worktree is constantly someone else's.

   **`%19`'s own summary is the reusable half:** one experiment gave a plausible mechanism and a
   correct practical conclusion; the second refuted the mechanism while preserving the conclusion.
   Stopping at one would have published *"rch archives the index"* — false, and it would have
   licensed the belief that unstaged edits are safe from the lane. **The negative control is the
   whole reason the answer is right.**

   **This is the THIRD instance of the glob-member hazard and its worst variant.** The first two
   (`response-envelope-check` 23:29, `zz-planted-dup` 05:57) broke workspace **LOADING** — loud,
   local, instant. This one loads perfectly and breaks only **TRANSFER PREFLIGHT**, so it is
   invisible from the pane that caused it and reaches peers as a refusal naming an unfamiliar path.
   And this file's existing ruling landed exactly as written: *"a gate author is precisely the agent
   most likely to create one."* The author was mid-`fsu7`, building a gate.


## 8f — `RCH-I005 project_excluded` HAS THREE CASES, NOT TWO — and `rch queue` cannot tell you which

8f. **`RCH-I005 project_excluded` HAS THREE CASES, NOT TWO — and `rch queue` cannot tell you which.**
   Rule 8c established that an exclusion with **no live row** for your project is stale state, not
   contention: unpin and go. `%8` measured the third case 2026-09-07.

   ```
   no live row for the project              STALE      -> unpin and go            (8c)
   live row that is YOUR OWN build          CONTENTION -> wait; a second request duplicates it
   live row that is a PEER's build, same
     project, different pane                CONTENTION -> REROUTE to another worker
   ```

   `%8` found `contabo-3` genuinely running `omp-orchestrator-38cf50d1` —
   `cargo test -j 2 -p gate-runner --test index_probe`, which was **`%19`'s staged-file
   experiment**. It rerouted without waiting, `contabo-1` returned `exit=0`. **Waiting would have
   been wrong**: it was not its own build, so there was nothing to duplicate.

   The trap is that **the queue row does not name the pane**, so it cannot distinguish case 2 from
   case 3 — which is why this file already records *"`rch queue` cannot tell you whether you are
   your own blocker."* Answer it from your own knowledge of what you launched, not from the row.


## 8g — A BROADCAST IS A SNAPSHOT AND CARRIES NO TIMESTAMP A READER CHECKS

8g. **A BROADCAST IS A SNAPSHOT AND CARRIES NO TIMESTAMP A READER CHECKS.** `%20`, self-reported
   2026-09-07: it told three panes that only `src/lib.rs` existed under `crates/gate-runner`, and by
   the time the message sent the owner had written `main.rs`. **True when measured, false when
   sent** — it had caught a peer mid-write.

   Its own framing is the rule: the same stale-premise class this file records on beads, *"committed
   by me in a broadcast, where it is worse because a broadcast carries no timestamp a reader
   checks."* A bead comment sits beside its own history; a broadcast arrives as present tense. **So
   a broadcast about a live tree must name its measurement time and tell recipients to re-measure**
   — which is what the correction did.


## 8h — ` M` ALONE IS NOT A COLLISION SIGNAL. ` M` WITH NONZERO INSERTIONS IS

8h. **` M` ALONE IS NOT A COLLISION SIGNAL. ` M` WITH NONZERO INSERTIONS IS.** Measured 2026-09-07,
   after a mode-only diff changed two routing decisions in one session.

   The agent write tool sets mode `100755` on `.rs` files it touches. No Rust source here is
   executable, so it is pure artifact — but in a shared checkout `git status --porcelain` is how a
   pane decides whether a file is safe to take, and **a mode-only change is indistinguishable from a
   peer mid-edit.**

   ```bash
   git diff --numstat -- <path>     # "0<TAB>0<TAB>path" => MODE ONLY, safe to take
   ```

   **Two false collisions, both of which nearly stood:**

   - `crates/omp-orchestrator/src/main.rs` was reported as *"carries several panes' uncommitted
     hunks"*, so two `finding` publisher call sites that were **refusing at runtime** were left for
     their owner. Measured `0+ 0-`, byte-identical to HEAD. The fix landed only because the diffstat
     was checked.
   - `crates/asupersync-conformance` showed 2 dirty files, and a pane correctly declined to take an
     unassigned crate someone appeared to be editing. Measured *"2 files changed, 0 insertions(+), 0
     deletions(-)"*. **It was free the whole time**, and the caution cost a round trip.

   **A SWEEP DOES NOT HOLD, AND THE SWEEP'S OWN COMMIT SAID SO.** `2bd4e99` cleared the bit from 84
   files (`0 insertions, 0 deletions`) and its NO-CLAIM predicted regeneration; **81 mode-only dirty
   files existed roughly two hours later, in the same session.** Sweeping again is theatre. The
   durable fix is a commit-time refusal, tracked as `omp-orchestrator-3xva`, and its residual is
   stated there: a commit-time gate cannot stop the write tool, so mode-only rows still appear
   between a write and a commit and this diagnostic stays necessary.

   **The general form is the reusable half:** a status flag reports *that* something changed, never
   *what*. Any decision keyed on `git status` alone — collision, ownership, staleness — is keyed on a
   coarser signal than the decision needs. Read the diff, not the flag.


## 8i — RUN A NEGATIVE CONTROL ON YOUR INSTRUMENT BEFORE YOU BELIEVE ITS ANSWER

8i. **RUN A NEGATIVE CONTROL ON YOUR INSTRUMENT BEFORE YOU BELIEVE ITS ANSWER.** Joshua,
   2026-09-07, fleet-wide. **This is the general form of rules 8b through 8h and it subsumes them.**

   Point the instrument at a **guaranteed-absent** subject — `/nonexistent/path/xyz`, a symbol that
   cannot exist, an empty scan set — and read what it returns. **If "absent" and
   "present-and-fine" produce the same output, the instrument cannot answer your question and its
   verdict on the real subject means nothing.**

   ```bash
   <tool> --root /nonexistent/path/xyz ; echo "rc=$?"      # what does ABSENT look like?
   <tool> --root . ; echo "rc=$?"                          # now the real one
   ```

   **If those two are indistinguishable, STOP and fix the instrument before reporting anything.**

   **WHY THIS AND NOT MERELY "NAME YOUR SCOPE".** Joshua's four cases:

   ```
   closure-check given an unreachable root  ->  worst=Pass exit=0, "every interpretable stage is
                                                complete"        VACUOUS, not failing
   registry-check under a skip_except stub  ->  PASS rows that were literally `true`
   a grep scoped to docs/evidence           ->  zero, read as "nothing untracked"
   a grep of config.toml for worker tags    ->  zero, read as "no darwin tag" (it is in workers.toml)
   ```

   **Naming the scope catches the last two. Only a negative control catches the first two, because
   there the instrument RAN and returned a confident green.** This is the L4 "gates proven to trip"
   discipline aimed at **any diagnostic binary**, not only at gates. **An empty scan set is an
   ERROR, never a pass. A verdict about a subject the tool could not read is not a verdict.**

   **IT INVALIDATED A DOCUMENT WITHIN AN HOUR OF ITS BEING COMMITTED.**
   `docs/plan/DISPATCH-CHAIN-FORKS.md` reported six lifecycle stages `unknown`:

   ```
   calls(zzz_cannot_exist_fn, also_absent_fn)   ->  NO VERDICT EMITTED
   calls(run_cycle, reap_finished_panes)        ->  NO VERDICT EMITTED   <- IDENTICAL
   ```

   Six rows carried zero information. Cause: `reap_finished_panes` and `ack_stage` are **crate**
   names, not functions, and two further probed symbols — `prepare_bead`, `DispatchPermit` —
   **return `grep -c` 0 anywhere in the repo. They were invented.** Re-probed with real symbols and
   both controls, **nine of eleven stages came back `confirmed`** — the chain was more wired than
   the document claimed, and the entire error was in the instrument.

   **A SECOND INSTRUMENT IN THE SAME TABLE WAS EQUALLY BLIND, and its control is the reusable
   one:**

   ```
   receiver_receipt::  (qualified, known-used)   7
   zzz_absent_crate::  (guaranteed absent)       0
   ```

   **A crate used UNQUALIFIED reads identically to an absent one under a `crate::` pattern.**
   `dispatch_claim_fence` and `dispatch_silence_watch` appear only as `use` lines and were nearly
   reported as BUILT ≠ WIRED, while the names they import are called bare — `authorize`,
   `clears_pending_dispatch_intent` ×4, `SilenceVerdict` ×8.

   **AND IT IS EXPRESSIBLE AS A TEST, WHICH IS STRICTLY BETTER THAN A HABIT.** `%20`, proving
   `m0c`'s amended `2b`, built a differential whose second arm is deliberately **not** the reporter
   — *"a reporter compared against itself agrees by construction"* — plus a leg asserting the two
   arms **MUST DISAGREE** on a known-bad input. If they ever agree, arm two has become a copy of arm
   one **and every equality in the suite is decoration.** A differential whose arms share an
   implementation is an instrument with no negative control; that leg *is* the control, in-tree and
   permanent.

   **The same rule applies to a tool's own output fields.** `rch queue` rendering `project` as `?`
   is **UNREADABLE, not absent-of-rows** — `%20`'s distinction, and it would have justified the
   opposite dispatch decision. A field you cannot read is not a field whose value is empty.


## 8j — `fh suggest` NEVER RETURNS EMPTY, SO A SUGGEST ROW IS A CANDIDATE, NOT A HIT

8j. **`fh suggest` NEVER RETURNS EMPTY, SO A SUGGEST ROW IS A CANDIDATE, NOT A HIT.** Measured
   2026-09-07 by `%19` and reproduced by me within the hour. It is rule `8i` aimed at the two tools
   Joshua directed the fleet to use.

   ```
   fh suggest "zzzqqq nonexistent xyzzy plugh frobnicate"
     -> 3 confident ranked rows about "nonexistent file" tests    NO EMPTY STATE
   fh search  "zzz_cannot_exist"
     -> [EMPTY] ... no false hit was returned          exit 4     DISCRIMINATES
   ```

   **`suggest` is lexical ranking: it always answers.** So *"it returned rows"* is not evidence that
   anything matched. **Confirm a suggest row with `fh search` — which does return `[EMPTY]` — or
   with `fh why <row-id>`, before citing it.**

   **I cited two `fh suggest` rows as before-you-build evidence for `etyur` and they survived
   confirmation** (`N046` at `ledger:franken-harvest.md:721`; `verify_binary_runs` at
   `meta_skill/src/updater/mod.rs:620-668`, both row-1/row-3 exact under `fh search`). **They
   survived by query quality, not by method** — the same verb gave `%19` pure noise. The
   discriminator is `search`, and I had not run it.

   **`fh` also reports its own freshness and its own input drift, and both change what a row means:**

   ```
   fh health        [RED] digest_missing_today / digest_stale -- the harvest did not run today
   fh doctor --json DRIFT exit 5  ACTIVE_GENERATION_PRODUCT_INPUT_DRIFT  (dirty source input)
   ```

   `%7`'s scoping is the right one: **treat fh as retrieval and provenance context, not a clean repo
   grade.** A stale row is still evidence; **its age is part of the citation.**


## 8k — `ripwire` EMITS ONE LINE, SO EVERY LINE FILTER DELETES THE WHOLE PAYLOAD — and a MISS IS NOT

8k. **`ripwire` EMITS ONE LINE, SO EVERY LINE FILTER DELETES THE WHOLE PAYLOAD — and a MISS IS NOT
   AN ABSENCE.** Measured 2026-09-07 by `%19` (four times, while it began diagnosing the tool) and
   independently by me on `--exemplar`.

   ```
   ripwire crates --uses=run_crate | grep -v ...    -> empty, FOUR TIMES
   raw                                              -> <u role="call"
                                                       p="crates/gate-runner/src/main.rs:178"
                                                       in_id="main"/>
   ```

   **`wc -l` is 0 because the payload is one line.** `head`, `grep -v`, and `sed` line filters all
   destroy it. **Read raw, or parse the XML.** My own `--exemplar` grep produced two meaningless
   fragment lines the same way.

   **Three further measured properties, each of which changes a conclusion:**

   1. **A name living only inside a MACRO STRING ARGUMENT is invisible to `--uses`.** In `uds`,
      `--uses=artifact_unchanged` → EMPTY while grep found **21** hits, with the positive control
      passing. **In a repo whose contract is typed refusals, the detector names ARE the API and they
      all live in macro strings.** The follow-up is `ripwire --grep`, **not** bare grep — it
      attributes each hit to its **enclosing symbol** (`in=`), and `unindexed_hits=` is the
      macro-string check built into the verb.
   2. **The legend is 77–95% of every response and does NOT amortise.** `%7` found the lever:
      **`--legend=compact`.** Prefer ONE well-chosen query over three exploratory ones.
   3. **It indexes what it FINDS, including scratch and vendored trees.** Measured here by `%19`:
      **8,570 `.rs` outside `crates/` against 475 inside — 18×**, from a vendored `.rch-tmp/` and
      target dirs. **An unscoped `ripwire .` in this repo is meaningless.** Scope it, and **state
      your scan set whenever you publish a count.**

   **AND IT DISTINGUISHES EXERCISED FROM CONSUMED, WHICH A GREP CANNOT.** My grep for `etyur` said
   *"`derive_checks`: 0 occurrences in `main.rs`"*. `%19`'s `--uses=derive_checks` said **5 uses,
   every one in `tests/roster.rs`, each attributed to its test fn, ZERO in any `src/`.** Strictly
   stronger — and it is exactly `N046`'s distinction: *"invocations from /tmp prove testing, not
   consumption."*

   **AND AN `fh` CAPABILITY ROW'S VERDICT SHAPE AND ITS MECHANISM ARE SEPARABLE. THE MECHANISM MUST
   BE RE-VERIFIED AGAINST THIS REPO'S OWN LINTS BEFORE IT IS COPIED.** Measured 2026-09-07: I cited
   `meta_skill/src/updater/mod.rs:620-668` `verify_binary_runs` as the arsenal precedent for
   `etyur`'s "a declared bin that cannot be exec'd must ERROR". `%19` read it **on the mirror, as
   instructed** and found:

   ```rust
   .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
   let status = loop { match child.try_wait()? { Some(s) => break s, None => { … sleep(25ms) } } };
   ```

   **That is the undrained-pipe deadlock pattern verbatim, and this repo ships
   `crates/undrained-pipe-lint` whose entire job is to refuse it** — its predicate at `src/lib.rs:9-10`
   is *"a Command builder that sets BOTH stdout and stderr to `Stdio::piped()`, whose handle is then
   polled with `try_wait()` in a loop"*, quoting the asupersync contract above.

   **It is SAFE where it lives and UNSAFE where I pointed it.** `--version` emits a few bytes so the
   pipe never fills; a declared `[package.metadata.gate]` check is an **arbitrary command with
   arbitrary output** — exactly the 64 KiB case. **Copying it would have shipped the deadlock into
   the check runner, and the tell reads as a SLOW gate rather than a WEDGED one**, which is the
   harder failure to diagnose.

   **Keep the verdict shape, replace the mechanism:** `subprocess_contract::bounded_output` is
   verified to drain — `src/lib.rs:216` spawns a `stdout_reader` thread, `:160` `join_reader`,
   `:254-255` joins both. In-repo bounded-spawn exemplars, from `ripwire --callers=bounded_output`
   (52 callers, `hop_tested=17`): `crate-soundness-verify::run_binary src/lib.rs:262`,
   `cargo-lane-budget::run_bounded:220`, `admission-reason::spawn_timeout:152`, all tested.

   **`fh` ranked the row correctly and the row is correct in its own crate. The defect exists only
   at the boundary where it would be reused, so no amount of ranking quality could surface it.**
   `fh suggest` tells you **where to look** and cannot tell you **whether to copy** — which is why
   the instruction is *"grep the mirror"*, not *"cite the row"*. `%19` followed the instruction I
   had given and not followed myself.


## 8l — A `success:false` FROM `ntm --robot-send` IS NOT PROOF OF NON-DELIVERY, AND THE RETRY MAY

8l. **A `success:false` FROM `ntm --robot-send` IS NOT PROOF OF NON-DELIVERY, AND THE RETRY MAY
   DOUBLE-PASTE.** Measured 2026-09-07 by `%19`: a callback reported
   `"success": false, "1 of 1 sends failed"` from `tmux send-keys`, then **succeeded byte-identically
   on immediate retry**, with `blocked: false` and no redaction findings.

   **So the sender's own failure report is indeterminate in both directions** — the first send may
   have landed, in which case the retry pastes a second copy into the composer. **A partial paste is
   the worse half of that outcome**, because a truncated packet reads as a malformed instruction
   rather than as a transport fault.

   This is the mirror of `cp-z42vu`, already recorded above, where a send returned `success:[4]`
   while the packet never arrived. **Both directions are the signature of an unacknowledged
   transport**, and the ACK comment is the only evidence the design admits on the tmux path. **Read
   the receiver, not the sender's verdict** — and when you retry, say so, so a doubled packet is
   attributable.


## 8m — RECORDING AN OBSERVATION CAN DESTROY THE EVIDENCE FOR IT. If your write touches the...

8m. **RECORDING AN OBSERVATION CAN DESTROY THE EVIDENCE FOR IT. If your write touches the field you
   are citing, capture the value FIRST.** Measured 2026-09-07, and it bit inside sixty seconds.

   I censused `in_progress` beads whose holder is not a live pane, found `omp-orchestrator-1p0u`
   held by `pane3` at `updated_at = 2026-09-06T23:46`, and **posted a comment recording that
   staleness.** The next read returned `updated_at = 2026-09-07T21:49`. **The comment refreshed the
   exact field that measured the thing the comment was about**, so the tracker no longer holds the
   evidence for its own annotation — a later reader sees a bead touched minutes ago and cannot see
   the 22-hour gap that justified flagging it.

   **This is a different family from every other instrument defect above.** `8b` through `8l` are
   about an instrument producing its own reading — `$?` after a pipe, `grep -c || echo 0`, a needle
   inside a comment, `git log -S` skipping merges. Here the instrument read correctly and **the ACT
   OF WRITING mutated the measurand.** No amount of negative control catches it, because the probe
   was sound both times; the subject changed between them.

   **The general form: any field maintained by the system you are annotating is destroyed by
   annotating it.** `updated_at`, `comment_count`, `last_touched`, an mtime, a "last accessed"
   timestamp, a hit counter. Capture the value in the write itself — as I did, so the figure
   survives in the comment body and the transcript — or take the measurement to a place the write
   does not reach.

   **The reusable check is one question: does my write touch the field I am about to cite?** If yes,
   the citation must be a captured literal, never a re-derivable query. And **do not fix it by
   declining to record** — an unrecorded observation is worse than a self-destroying one.


## 8n — `git add -- <path>` IS PATH-SCOPED AND STILL WHOLE-FILE. IT DOES NOT ISOLATE YOU FROM A PEER

8n. **`git add -- <path>` IS PATH-SCOPED AND STILL WHOLE-FILE. IT DOES NOT ISOLATE YOU FROM A PEER
   EDITING THE SAME FILE.** Measured 2026-09-07 by `%19`, and it is a gap in this file's own
   standing commit rule.

   The rule above says `git add -- <paths> && git commit -- <paths>`, both pathspecs. That protects
   against sweeping **other files** out of a shared index. It does **nothing** about sweeping
   **another agent's hunks inside your file** — and in a five-agent checkout that is the common case.

   Measured: `%19` staged a 21-line deletion in a file carrying a peer's 90 added lines and its first
   staging attempt captured **3 of the peer's lines** — a `rustfmt` of the helper body, adjacent to
   the deletion and inside the same hunk. Path-scoping did not see it, because the peer's work was
   not in another file.

   **The form that works, and it is a different mechanism, not a stricter pathspec:**

   ```
   git show HEAD:<path> > /tmp/base            # the tree, not the worktree
   ... produce the intended content from BASE ...
   diff -u ... | git apply --cached            # index holds HEAD-minus-your-change only
   git commit                                  # FROM THE INDEX -- no pathspec
   ```

   **The last line is the counter-intuitive half.** `git commit -- <path>` **re-reads the worktree**
   for that path and would pull the peer's lines straight back in. So once you have built a precise
   index with `git apply --cached`, a pathspec on `commit` is actively wrong. `%19` measured
   `32 deletions, 0 insertions` this way against `3 of the peer's lines` the naive way.

   **When it applies:** only when a peer is live in your file. `git status` showing ` M` with nonzero
   insertions on a path you are about to touch is the trigger (rule `8h`). Otherwise the two-pathspec
   form remains correct and is far cheaper.


## 8o — `grep -c <symbol>` COUNTS OCCURRENCES, INCLUDING THE USES INSIDE THE THING YOU ARE DELETING

8o. **`grep -c <symbol>` COUNTS OCCURRENCES, INCLUDING THE USES INSIDE THE THING YOU ARE DELETING.**
   Measured 2026-09-07, and it is the orchestrator's own error, made **one hour after** committing
   rule `8m` about instrument defects.

   `%19` proposed deleting a stale test and, conditionally, its helper *"if it has no other caller"*.
   I measured `grep -c 'workflow_invokes_lint' -> 3`, ruled *"definition plus two call sites, so a
   caller lives — keep the helper"*, and was **wrong**: both call sites are at `:271` and `:273`,
   **inside `wired_into_ci_workflow` which spans `:264-276`** — the very function being deleted.

   **`%19` refuted it with the compiler**, which is the right instrument and the reason this row
   exists: with the deletion applied, `rustc` emits
   `warning: function 'workflow_invokes_lint' is never used`.

   **This is the mention-vs-invocation defect from this file's own census correction**, where 23
   crates with a `.flywheel/` mention and no executor read as wired. A count cannot tell a definition
   from a call, a call from a comment, or **a call that dies with its caller** from one that
   survives it.

   **Use the compiler, or an enclosing-symbol tool, never a count.** And note `ripwire` failed here
   too, in a documented way: `--uses=workflow_invokes_lint` returned `count="0"` while two call sites
   plainly existed, because they sit inside `assert!()` — the macro blind spot. **A structural zero
   from a macro-blind tool is `UNKNOWN`, not absence**; `--grep` is the prescribed follow-up.
