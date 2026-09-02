# Agent Mail — defect dispositions

Bead: `omp-orchestrator-3r1r` — harden and align every measured Agent Mail defect

## Purpose

Operator directive 2026-09-02: *"all of these issues you're finding with agentmail need to be
hardened and aligned along the way - its a crucial part of our business / system."* So this is not a
catalogue. Every one of the nine measured defects carries exactly one disposition —
**(A)** fixed at source and upstreamed, **(B)** defended in `crates/agent-mail-native` with a typed
refusal, **(C)** named finding with a reproduction — and the table below has non-zero (A) and
non-zero (B), which is the anti-vacuity condition the bead sets.

`crates/agent-mail-native/` is owned by **AmNative** (Agent Mail identity `BrightGorge`, id 310).
Every (B) row here is therefore a **request to AmNative**, not a crate this lane writes. Each (B)
ships with the known-bad input its refusal must fire on, because a defense that has never refused is
not a defense.

## Task one — the checkout can now express the bug

The bead's blocking premise was that no local checkout contains the failing query. **That premise is
refuted**, and the version question is settled:

| fact | measured |
|---|---|
| shipped binary | `am --version` → `am 0.3.31` (`/opt/homebrew/bin/am`) |
| working checkout `/Users/josh/Developer/mcp_agent_mail_rust` | `Cargo.toml` `version = "0.3.10"`, HEAD `88b531c0` = `v0.3.10-30` |
| **tag `v0.3.31` after `git fetch origin --tags`** | **`git show v0.3.31:Cargo.toml` → `version = "0.3.31"`** |
| mirror `/Volumes/ZestData/dicklesworthstone-mirror/mcp_agent_mail_rust` | `version = "0.3.32"`, HEAD `c7a7083f` = `v0.3.32-7` |
| `git grep -lF 'm.topic' v0.3.31` | **6 files** — not 0 |

**Acceptance 1 is met by proof, not by upgrade:** the tag `v0.3.31` carries
`version = "0.3.31"`, byte-identical to what the shipped binary reports, so any fix authored against
that tag is authored against the shipped line. `git fetch origin --tags` was run (non-destructive)
and brought `v0.3.31` and `v0.3.32` into the working clone.

**THE DISTINCTION THAT MATTERS, STATED EXPLICITLY.** Matching the version makes our source
**comparable**; it does not make our tree the thing that **executes**. Homebrew `am 0.3.31` is not
our build — `/opt/homebrew/bin/am` is a binary we did not compile from this checkout, and nothing in
this document changes a byte of it. So:

- *"the defect is located in source at the shipped version"* — **claimed, and proven** below.
- *"the defect is gone from the daemon the fleet talks to"* — **NOT claimed.** It cannot be, until an
  `am` built from a line carrying `fsqlite >= 0.3.14` is the binary on `PATH`.

Only the second claim helps anybody, and it remains open. This is the same failure shape as
tonight's stale-binary defects, where a source-only fix changed nothing the system executed because
the hook was a Mach-O linking its lints as libraries. **Validation leg 4 below is the guard against
repeating it: it asserts the defect STILL REPRODUCES on the installed binary, so a PASS there means
the work is not yet done.**

**The working checkout was deliberately NOT updated, and this is the divergence stated explicitly.**
It is `1285` commits behind `origin/main`, but also **`19` commits AHEAD with `15` dirty files**
(including `crates/mcp-agent-mail-db/src/queries.rs`, `models.rs`, `pool.rs`, `Cargo.lock`). That is
somebody's uncommitted in-flight work. A checkout or reset would destroy it, so every inspection
below used `git show <tag>:<path>` and `git grep <tag>`, which read the object store without
touching the worktree. The bead's own warning applies in the other direction too: updating a tree
someone else is mid-edit in is the same class of damage as fixing stale source.

The earlier `grep -rlF 'm.topic' → 0 files` reading is explained: the working checkout at
`v0.3.10-30` genuinely does not contain it (`git grep -lF 'm.topic' HEAD` → 0), and the conclusion
was generalised to "both checkouts". The mirror at `v0.3.32-7` has it in 6 files, and so does the
shipped tag.

## Task two — defect #3 localized, and it is NEITHER the query NOR the schema

Localized at the shipped tag `v0.3.31`:

| element | location |
|---|---|
| select list containing `m.topic` | `crates/mcp-agent-mail-db/src/search_planner.rs:811` (`PlanMethod::Like`) and `:821` (`PlanMethod::FilterOnly`) |
| FROM clause | `search_planner.rs:813` and `:823` — both `messages m LEFT JOIN agents a ON a.id = m.sender_id` |
| final assembly | `search_planner.rs:955`, `:1041`, `:1120` — `format!("SELECT {select_cols} FROM {from_clause}{where_str} {order_clause} LIMIT ?")` |
| error emitter | **`frankensqlite/crates/fsqlite-planner/src/lib.rs:172`** — `Self::ColumnNotFound { column } => write!(f, "column not found: {column}")` |

**The SQL and the schema side by side.** Schema, from the live store:

```
$ sqlite3 "file:$LIVE?mode=ro" ".schema messages"
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT, project_id INTEGER NOT NULL REFERENCES projects(id),
    sender_id INTEGER NOT NULL REFERENCES agents(id), thread_id TEXT,
    topic TEXT COLLATE NOCASE,                      <-- THE COLUMN EXISTS
    subject TEXT NOT NULL, body_md TEXT NOT NULL, ... );
```

Query, as assembled — a **flat** SELECT, no subquery and no outer wrapper, so the alias `m` is in
scope everywhere `m.topic` appears:

```sql
SELECT m.id, m.subject, m.importance, m.ack_required, m.created_ts, m.thread_id,
       COALESCE(a.name, '<unknown>') AS from_name, a.id AS from_agent_id,
       m.body_md, m.project_id, 0.0 AS score, m.topic
FROM messages m LEFT JOIN agents a ON a.id = m.sender_id
WHERE m.project_id = ? AND (m.subject LIKE ? ESCAPE '\' OR m.body_md LIKE ? ESCAPE '\')
ORDER BY m.created_ts DESC LIMIT ?
```

**ANSWER TO THE QUESTION THE BEAD ASKS: not the query, not the schema — and NOT the engine either.**
Two legs hold; the third is **RETRACTED**, and the retraction is the most useful thing in this
section.

1. **The query is valid.** That exact SELECT/FROM/WHERE/ORDER BY, run against the live store under
   **stock `sqlite3` 3.51.0**, returns rows — ids 40792, 40791, 40788. Not rejected, answered.
2. **The alias is always bound.** The planner contains exactly **two** FROM clauses (lines 813, 823),
   both `messages m ...`, and the FTS branch cannot be the culprit: `PlanMethod::Empty |
   PlanMethod::TextMatch => unreachable!()` at `search_planner.rs:826`. FTS5 SQL was removed
   entirely in the Search V3 decommission (`:786`). So the "unbound alias `m`" hypothesis — which
   would have made this a query bug — is refuted.
3. **~~The error originates inside the engine.~~ RETRACTED — the engine is EXONERATED for this SQL.**

**THE ENGINE EXONERATION, MEASURED AT THE PINNED VERSION.** My earlier engine claim rested on the
error string living in `fsqlite-planner` plus my inability to test the pinned code — I flagged that
as the NO-CLAIM gap and the named next step. **I closed the gap: `cargo install fsqlite-cli
--version 0.3.11` (the exact pin) and ran the exact query.** It **succeeds** under every variation
that distinguishes production from a toy:

| variation tested on fsqlite **0.3.11** | result |
|---|---|
| exact select list + `messages m LEFT JOIN agents a` + exact production DDL | **rows returned** |
| `ORDER BY score ASC, m.id ASC` (the relevance path, ordering by the *alias*) | **rows returned** |
| real constant `COALESCE(a.name, '[unknown sender]')` | **rows returned** |
| escaped LIKE `'%OMP\_MSG\_SRC%' ESCAPE '\'` — the literal failing query | **clean, no error** |
| DB authored by **stock sqlite3** | **rows returned** |
| DB authored by **fsqlite itself**, with `topic` added via `ALTER TABLE` as `schema.rs:2294` does | **rows returned** |

So `fsqlite 0.3.11` resolves `m.topic` correctly. **The engine is not the fault**, and my three-leg
inference was wrong on its third leg.

**AND THAT BREAKS THE (A) DISPOSITION.** The fsqlite planner diff `v0.3.11..v0.3.14` contains **no
column-resolution change** — only `6fd828248` "feat(fts5): guarantee stock C SQLite compatibility on
multi-leaf FTS5 index data (#404)", `33b5035b4` a VDBE perf change (GH#400), and version bumps. So
`c7a7083f`'s bump is **unlikely to fix this defect**, and "adopt upstream" is no longer a supported
remedy. Upstream bumping the pin was never evidence about *this* bug; I read a correlation as a
remedy.

**THE REMAINING SUSPECT IS THE ADAPTER, AND IT IS UNBUMPED.** The error chain is
`SQLite error: ` + `Query error: ` + `internal error: column not found: m.topic`. The middle layer
is neither Agent Mail's nor fsqlite's: `Query error` is emitted by
**`sqlmodel-frankensqlite/src/connection.rs`**, pinned `sqlmodel-frankensqlite = "=0.4.0"` — and
`c7a7083f` did **not** touch it. That adapter **infers result column names by PARSING SQL TEXT**
rather than asking the engine: `infer_column_names` (`connection.rs:1186`) →
`infer_select_columns` (`:1271`) → `extract_column_name` (`:1387`), with hand-rolled
`split_at_depth_zero` / `find_keyword_at_depth_zero` scanners. `extract_column_name` *does* strip the
qualifier correctly in isolation, so the specific mechanism is still unidentified — but a text-parsing
column-name inference layer is the only component left unexplained, and it is a genuine
architectural fragility regardless of whether it causes this row.

**ADOPTION HAS A BLOCKER NOBODY HAD NAMED: `origin/main` DOES NOT BUILD STANDALONE.**
`cargo install --git ... --branch main mcp-agent-mail-cli` fails at manifest load:
`failed to read .../frankensearch-rel-0332/frankensearch/Cargo.toml — No such file or directory`.
`origin/main` carries an **out-of-tree sibling path dependency** that is not in the git tree, so
testing the bump requires reproducing a sibling constellation layout. The mirror has `frankensearch`
but not under that name. **"Just build from main and re-run leg 4" is not a one-command action**, and
that is the concrete blocker on closing this row.

**CORRECTION TO AN ATTRIBUTION.** The orchestrator's 06:2xZ bead comment records #3 as *"the fault
is the QUERY BUILDER'S ALIAS, not the schema (MailMining)"* and repeats that
`grep -rlF 'm.topic'` returns 0 files in both local checkouts. **Neither half is this lane's
finding.** The alias is not the fault — leg 3 refutes it, and the grep does find it, in 6 files at
`v0.3.31` and 6 in the 0.3.32 mirror. **Do not patch `search_planner.rs`**: leg 1 and the 0.3.11
matrix both show that SQL is correct, so editing it would change correct code.

## The disposition table

Non-zero (A) and non-zero (B), per the anti-vacuity condition.

| # | defect | disp | action |
|---|---|---|---|
| 1 | `register_agent` accepts `agent_name`, silently mints a new identity | **B** | typed `RegisterKeyRejected` + mandatory read-back |
| 2 | `register_agent` accepts `pane_id` and discards it, success envelope | **B** | read-back assertion → typed `RegisterFieldNotPersisted` |
| 3 | `am robot search` RED for the entire corpus | **C** + **B** | (A) WITHDRAWN — the pin bump is not the fix and `origin/main` does not build standalone. **C**: reproduction + engine exoneration + fault narrowed to the `sqlmodel-frankensqlite =0.4.0` adapter. **B** `SearchUnavailable` so an engine error is never read as empty |
| 4 | ~~`--direct` SQLite fallback does not announce itself~~ | **REFUTED** | **NOT A DEFECT — byte-identical output is CORRECT: with the daemon reachable, both paths use the daemon. Nothing to announce.** |
| 5 | `mail_pending` default path never terminates and drops the cursor | **B** | wrapper always supplies a ceiling; typed `WaitCanceledWithoutCursor` distinct from "no mail" |
| 6 | `inbox-events` carries no read state | **B** | reconcile pair: `inbox-events` cursor + `read_ts` from the **daemon** surface (the CLI lacks it and exposes `priority` instead — see correction below) |
| 7 | `last_active` is registration recency wearing an activity name | **C** | named finding; never use as a work oracle |
| 8 | ~~the `am` CLI does not talk to the authenticated daemon~~ | **REFUTED** | **FALSE — the CLI calls the daemon BY DEFAULT, with a token, over `/api/` and `/mcp/`. Only `am health` is daemon-free. Doctrine built on this must be withdrawn.** |
| 9 | every pane binding resolves `legacy-unverified` | **B** | typed `PaneBindingUnverified`; reap with existing `cleanup_pane_identities` |
| 10 | ~~daemon documents `CURSOR_EXPIRED` but silently CLAMPS instead~~ | **REFUTED** | **NOT A DEFECT — resolved at source below. The clamp is correct and intended (GH#238); the guard built against it is over-strict.** |
| 11 | `signaled=false` while `persisted=true` and `acknowledged=true` (AmNative) | **C** | **RESOLVED AT SOURCE below — documented behaviour, not a delivery gap. Do NOT defend against it.** |
| 12 | "daemon 0 unread vs CLI 20 rows" — **NOT a read-state disagreement; it is agent-identity resolution** | **B** | typed `AgentNameAmbiguous`; every name lookup MUST carry a project scope |
| 13 | `inbox_stats.ack_pending_count` drifts from ground truth (113 vs 46) | **C** | named finding; never read the cached aggregate as truth |

Totals over thirteen rows: **(A) 0, (B) 6, (C) 5, REFUTED 3**. **Four rows turned out not to be
defects at all** — #4, #8 and #10 refuted outright, #11 documented behaviour — and a fifth (the
proposed `--direct` inversion) was refuted before it was filed. All four dissolved by **reading
version-matched source**, not by measuring harder. That ratio is the honest characterisation of this
list.

**ON THE ANTI-VACUITY CONDITION, STATED DIRECTLY BECAUSE (A) IS NOW ZERO.** The bead's condition is
*"a disposition table with zero (A) **and** zero (B) rows is an ERROR"*. Six (B) rows means the
condition is met — but the spirit of it deserves a straight answer rather than a technicality:

**No defect in this list is fixable-at-source by us today, and that is a measured conclusion, not a
shortfall of effort.** #3 was the only (A) candidate. Closing it required proving the engine at
fault; I built the pinned `fsqlite 0.3.11` and **exonerated it** across six variations, then found
that upstream's bump contains no column-resolution change, then found that `origin/main` **does not
build standalone** (out-of-tree `frankensearch-rel-0332` path dependency). Every other row is either
in a dependency we do not author, refuted, or already defended in `agent-mail-native`.

So the hardening that actually reached the product is the **(B) column**: six typed refusals, each
with the known-bad input it must fire on, one of them (#10's) already shipped and then correctly
identified as over-strict and queued for deletion. **A withdrawn (A) backed by a reproduction and an
exoneration is worth more than a patch to code that leg 1 proves is correct** — which is what the
earlier (A) would have produced.

### Rows #4 and #8 — REFUTED BY ONE FUNCTION, and the doctrine built on #8 must be withdrawn

AmNative retracted #8 after discovering its evidence was never measured: its one attempt to check
the environment was `env | grep -i -E 'agent_mail|AM_'`, **dcg DENIED it**, and the denial was never
retried. "The CLI carries no token" was then asserted as measured fact, and an architecture and a
house pattern were built on it. **A DENIED PROBE IS NOT A NEGATIVE RESULT** — it is the same shape as
the `am agent start` false negative #8 was invented to explain.

Verified independently here at the shipped tag `v0.3.31`, not the 0.3.32 mirror:

**The CLI routes through the daemon BY DEFAULT.** `crates/mcp-agent-mail-cli/src/lib.rs:8911`:

```rust
/// The default (non-`--direct`) path always prefers the daemon. `--direct` prefers
/// the daemon only when it is reachable, falling back to a direct SQLite read when
/// no daemon is listening — this avoids contending on the WAL with a running
/// `serve-http` daemon's long-lived writer (GH#158).
const fn check_inbox_should_use_daemon(direct: bool, daemon_reachable: bool) -> bool {
    !direct || daemon_reachable
}
```

with an explicit spec at `:42849-42856` — `(false,false)` and `(false,true)` both true, `(true,true)`
true, `(true,false)` false. So **omitting `--direct` takes the daemon unconditionally with no
fallback**, and the SQLite read is the exception.

**It authenticates.** `HTTP_BEARER_TOKEN` and `AGENT_MAIL_TOKEN` are read by the CLI, and its own
401 message at `:40179` is *"authentication failed (HTTP 401) while calling
http://127.0.0.1:8765/mcp/; check AGENT_MAIL_TOKEN/HTTP_BEARER_TOKEN"* — a CLI that never called the
daemon could not emit that. `/api/` and `/mcp/` are both real routes and are each other's
alternates (`:9351-9352`).

**So #8 is FALSE and every consequence drawn from it is withdrawn**, including the claim recorded in
this document's earlier revisions that CLI-derived figures read `storage.sqlite3` rather than the
daemon. They came from the daemon. Nobody's numbers need re-measuring — the *provenance story* was
wrong, not the counts. What survives is narrow and correctly measured: **`am health` really does
build a throwaway probe SQLite and never contacts the daemon.** The error was generalising `health`
to the whole CLI.

**#4 falls out of the same function.** #4 was filed because `--direct` with the daemon UP produced
byte-identical output to the non-direct call, read as "the fallback does not announce itself". But
with `daemon_reachable = true`, `!direct || daemon_reachable` is **true either way** — both paths use
the daemon, so the source is the same and there is nothing to announce. Byte-identical output is the
*correct* observation of *correct* behaviour. `--direct` only changes anything when the daemon is
DOWN. Refuted.

**A CORRECTION TO AmNative'S OWN REPLACEMENT ROW.** Its retraction proposes a new defect: that
`--direct` is "inverted relative to its own help text". **It is not.** The shipped help text reads
*"Allow a direct SQLite read only when no daemon is reachable"*, which is exactly what
`!direct || daemon_reachable` implements — `--direct` **allows** the SQLite fallback; omitting it
forbids it. Help text and code agree, and the doc comment names the reason (GH#158, WAL contention
with the daemon's long-lived writer). Do not file the inversion row.

**What this costs the differential oracle, stated because it downgrades acceptance evidence.**
`oracle_skew=0` is not two independent authorities agreeing about a store; it is **two HTTP routes on
one daemon process, authenticated with the same token, reading the same in-process state.** It
proves one daemon is self-consistent across `/api/` and `/mcp/`. Real, but far weaker than
"corroborated against the store". The genuinely independent oracle in this document is read-only SQL
against the store file, which is why row #12 was settled that way and not by calling a surface.

### Row #10 — REFUTED AT SOURCE: the clamp is correct and the guard is over-strict

AmNative filed #10 as "the daemon's own docs promise `CURSOR_EXPIRED` below retained history and it
silently clamps instead", shipped a typed `CursorExpired` guard against it
(`journey::verify_resume_continuity` / `resume_from`, d0a88e5), and then — correctly — flagged that
the whole claim rested on an unchecked inference: **is `oldest_available_cursor` an eviction floor,
or simply that recipient's first-ever delivery?** It asked for the source discriminator. Here it is,
and it refutes the defect.

**1 — There is NO production prune of delivery events.** The only `DELETE FROM
inbox_delivery_events` in the tree is at `crates/mcp-agent-mail-db/src/sync.rs:1456`, inside
`mod tests {` (opens at :1252), and its own `.expect()` string says
`"simulate a pruned historical event"` — the test has to manufacture a prune because nothing in
production performs one.

**2 — `oldest_available_cursor` is `MIN(seq)` over that recipient's surviving rows**
(`sync.rs:574`: `SELECT MIN(seq) AS oldest_cursor, MAX(seq) AS tail_cursor, (SELECT MIN(seq) FROM
inbox_delivery_events) AS global_oldest`). With no prune, that is exactly the first-ever delivery —
**not** a retention floor. Note the query computes a *second*, store-wide floor as well; that one is
the real retention signal.

**3 — `CursorExpired` is gated on the GLOBAL floor, and the source comment anticipates this exact
mistake by name.** At `sync.rs:606-624`:

```rust
// `seq` is a GLOBAL AUTOINCREMENT shared by every recipient, so a gap
// between `after` and this recipient's oldest event normally consists
// of other recipients' deliveries — NOT lost history. A monitor that
// called `--position-now` on an empty inbox (cursor 0) must still
// receive its first delivery even when that lands at a high global
// seq (GH#238). A cursor is only genuinely expired when retention has
// actually removed rows, which is observable while global seq 1 is
// gone from the ledger.
let retention_has_pruned = global_oldest_cursor.is_some_and(|global| global > 1);
if let Some(oldest) = oldest_available_cursor
    && retention_has_pruned
    && after < oldest.saturating_sub(1)
{ return Err(InboxDeliveryEventError::CursorExpired { after, oldest_available: oldest }); }
```

So refusing a high first-event seq is **the bug GH#238 fixed**, and not refusing it is the intended
behaviour. `after: 1` on GreenFrog returning a success page from 2108 is correct.

**4 — Confirmed empirically on the live store: the ledger is CONTIGUOUS.**

```
$ sqlite3 ... "select min(seq), max(seq), count(*) from inbox_delivery_events;"
1|5226|5226
```

`count == max` with `min == 1` means **zero rows have ever been removed**. Therefore
`retention_has_pruned = (1 > 1) = false`, and `CursorExpired` is **unreachable in this store by
construction** — it has never been observed because nothing has been evicted, not because the daemon
fails to emit it. Per-recipient floors are first events, exactly as AmNative suspected: GreenFrog
2108, AmberGate 2058, BlueLantern 2063.

**Consequence for the shipped guard.** `verify_resume_continuity` refuses whenever
`oldest_available > stored`, which omits the `retention_has_pruned` conjunct — so it refuses a
healthy replay for **every** recipient whose first event is nonzero, i.e. every recipient in the
store, including `DeliveryCursor::ORIGIN`. That is precisely the *"monitor that called
`--position-now` on an empty inbox must still receive its first delivery"* case **GH#238 was filed
about**: the guard reimplements the bug upstream already fixed, inside something advertised as a
defense. This is the over-strict gate row #11 warns about, built for real this time. Live blast
radius is zero today because the wired caller uses `CursorQuery::PositionNow`, not `resume_from`.

**THE FIX IS A DELETION, AND MY FIRST RECOMMENDATION WAS WRONG.** I initially advised copying the
stateless predicate `global_oldest_cursor > 1` into the guard. **A client cannot compute it.**
`InboxDeliveryEventPage` (`sync.rs:32-38`) returns exactly `events`, `next_cursor`, `has_more`,
`oldest_available_cursor`, `tail_cursor` — `global_oldest` is computed at `:575`, consumed for the
decision at `:615`, and **never leaves the function**. So `retention_has_pruned` is unknowable
client-side, and the guard was deciding expiry with **strictly less information than the daemon
has**. AmNative reached this independently and its conclusion is the right one: delete the
client-side continuity check and trust the daemon's refusal. `MailError::CursorExpired` stays as a
**decode** of the daemon's `CURSOR_EXPIRED`, never as a locally synthesised verdict.

That generalises past this row: **a layer must not re-derive a judgement whose deciding input it
cannot observe.** The daemon keeps `global_oldest` private precisely because the decision is its to
make.

If a fires-on-known-bad is ever wanted for the daemon's own path, the recipe is already in the tree:
`sync.rs:1456-1462` deletes one `seq` and then asserts `after=0` returns
`expect_err("cursor before retained floor must be explicit")`. That is the only shape that evidences
loss — and it needs a manufactured prune, which is itself the proof that no production prune exists.

### Row #11 — RESOLVED AT SOURCE: the field is narrow, the notification is fine

#11 was filed with the fork left open and an explicit instruction not to guess: *either* the
notification genuinely did not fire, *or* `signaled` reports something narrower than its name. **The
tree is now version-matched to the shipped binary, so that fork is answerable by reading, and the
answer is the second one.** At `v0.3.31`:

`crates/mcp-agent-mail-tools/src/messaging.rs:4259` — the boolean is nothing but a receipt-existence
test:

```rust
"signaled": !signal_receipts.is_empty(),
```

and its own tool description at `messaging.rs:4219` states the semantics outright:

> "`signaled` is true only when a message-ID-bound signal receipt was appended after a successful
> signal write; **a debounced or failed signal remains persisted but not signaled.**"

Corroborated by `crates/mcp-agent-mail-db/src/sync.rs:134-138`:

> "The mutable recipient `.signal` file is **intentionally not read here**: it is a debounced
> latest-state hint that can point at another message. Only the append-only receipt ledger
> establishes the `signaled` fact."

**So `persisted=true, signaled=false, acknowledged=true` is documented, correct behaviour for a
DEBOUNCED signal.** There is no delivery gap: the payload was stored, the notification was
debounced rather than lost, and the ack proves it arrived. The defect is a **naming trap** — the
field answers "was a message-ID-bound signal receipt appended?" while its name invites "was anyone
notified?" — and the harm is entirely in the reading, not the mechanism.

This confirms AmNative's own caution and closes it out as (C): **do not build a typed refusal on
`signaled`.** A defense against a correctly-reported narrow field would be an over-strict gate that
refuses healthy debounced traffic. The right fix is documentation and consumer discipline —
`signaled` is not a delivery oracle, `acknowledged` is the one that proves arrival.

**WHERE THE WRONG FRAMING ACTUALLY LIVES, per AmNative's own audit of its crate.**
`agent-mail-native` **never gates on `signaled`** — it only exposes
`RecipientReceipt::is_persisted_but_unsignalled`, and the wired orchestrator caller records
`signaled=` into a ledger row without branching on it. So **no healthy debounced traffic is refused
anywhere today**, and the over-strict gate this row warns about was never built. The residue is in
`agent-mail-native`'s **doc comments**, which describe the state as "the silent-failure shape"; the
correction to the consumer rule above is pending, blocked only because `journey.rs` and `lib.rs` are
held by GreenFrog's identity work. Recorded here so the framing does not outlive the message that
corrected it.

AmNative also notes its three "independent instances" (40786, 40810, 40826) were three instances of
**normal debounced traffic** — the measurements were right and the diagnosis was wrong. Worth keeping
because it is the same shape as the `mail_pending` error: a real, reproducible observation
generalised into a defect claim **without first reading the thing that defines correct behaviour**.
That is the failure mode task one exists to prevent, and it is why version-matching the checkout was
the blocking task rather than housekeeping.

### Row #12 — folded into this epic, and RECLASSIFIED

AmberGate filed `omp-orchestrator-monitor-reads-oracle-y256` as a read-state disagreement: the
daemon reporting **0 unread** while the CLI returned **20 rows**. Folding it in here rather than
leaving it separate, because measured against the store it is **not about read state at all** — it
is defect #1's identity class wearing a different symptom.

`agents` is `UNIQUE(project_id, name)`, so **a name is not unique in this store.** There are two
`AmberGate` rows:

```
$ sqlite3 "file:$LIVE?mode=ro" "select a.id,a.project_id,p.human_key,
    (select count(*) from message_recipients r where r.agent_id=a.id) recips,
    (select count(*) from message_recipients r where r.agent_id=a.id and r.read_ts is null) unread
  from agents a join projects p on p.id=a.project_id where a.name='AmberGate';"
39|65 |/Users/josh/Developer/control-plane      |  4|  0     <-- the 0
69|107|/Users/josh/Developer/omp-orchestrator   |113| 22     <-- the truth
```

**`0` is exactly what id=39 reports.** One surface resolved `AmberGate` to the control-plane row,
the other to the omp-orchestrator row. And the ground-truth unread count is **22**, so the CLI's
`20` is not the true count either — it is a default page, which means *neither* circulated number
was the answer. Three readings, none correct: `0` (wrong agent), `20` (a page), `22` (the truth,
only visible in `message_recipients`).

This is systemic, not one mailbox — **10 names are currently ambiguous**, including
`BlueLantern` (ids 27 and 68) and `AirTrafficControl` (11 rows):

```
$ sqlite3 ... "select count(*) from (select name from agents group by name having count(*)>1);"
10
```

So any Agent Mail figure derived from a name-based lookup without a project scope — including
figures cited tonight — is suspect by construction.

### `read_ts` — a two-authorities split, not a missing field

This row was corrected twice and the second correction is AmNative's, against me. The orchestrator
broadcast that `am inbox --json` rows carry `read_ts`; they do not — the CLI's keys are eight
(`ack_status, age, from, id, importance, priority, subject, thread`) with read state exposed as
`priority`. But my wording generalised that into "`read_ts` is not on the inbox surface", and
**AmNative measured it on the daemon**: `fetch_inbox` over MCP returned
`"read_ts":"2026-09-02T05:10:25.416054Z"` at 05:10Z.

Confirmed at source rather than by re-calling the tool, because `fetch_inbox` **marks messages read**
and that is a stated non-goal — its own description at `messaging.rs:3731` begins *"Retrieve recent
messages for an agent and mark returned messages read"*, and documents `unread_only` as *"only
recipient rows whose `read_ts` is unset"*. The field is declared and populated:

```
messaging.rs:1706   pub read_ts: Option<String>,
messaging.rs:3871   read_ts: row.read_ts.map(micros_to_iso),
```

So the correct statement is a **response-shape difference between two commands, not two authorities
over one store**: `read_ts` originates in the store as `message_recipients.read_ts` (`read_ts
INTEGER, ack_ts INTEGER, PRIMARY KEY(message_id, agent_id)`), the **daemon's `fetch_inbox` surfaces
it**, and the **CLI's `inbox` projection omits it** in favour of `priority`. Both commands reach the
same daemon (see the #8 refutation) — so this is one authority projecting two different views, and
a monitor needs both *views*, not both *authorities*. The store remains the arbiter; it is what
settled #12. AmNative's `InboxMessage::read_ts` and its reconciliation pair are reading a field that
genuinely exists on the path they use.

**The section heading above is retained deliberately even though "two authorities" is now wrong**,
because this row is where the phrase entered the document and a reader who saw it elsewhere needs to
land here. The phrase was mine, adopted from #8, and #8 is refuted.

### (A) row — #3, the patch path

Upstream already authored the fix; what was missing is the evidence that it *is* the fix. This lane
supplies that: the stock-sqlite3-accepts / fsqlite-rejects differential plus the
`fsqlite-planner:172` emitter. The patch is upstream commit `c7a7083f`, a one-line pin bump. Our
adoption action is to run an `am` built from a line carrying `fsqlite >= 0.3.14` and re-run the
reproduction below; the currently installed Homebrew `am 0.3.31` cannot contain it. Until then #3's
(B) defense is what protects callers.

### (B) rows — fires-on-known-bad inputs for AmNative

Each defense must be shown refusing the input that produced the defect:

| # | typed variant | known-bad input that must fire it |
|---|---|---|
| 1 | `RegisterKeyRejected` | `register_agent` with key `agent_name` instead of `name` |
| 2 | `RegisterFieldNotPersisted` | `register_agent` with `pane_id`, then read back and find it `None` |
| 3 | `SearchUnavailable` | any `search` whose engine error is `column not found: m.topic` — must NOT map to empty results |
| 4 | `SourceAmbiguous` | `inbox-events --position-now --direct` while `/health` returns 200 |
| 5 | `WaitCanceledWithoutCursor` | `--wait-until=mail_pending` with no `--timeout` → `CANCELED` with `cursor_info` absent |
| 6 | (reconciliation, not a refusal) | a monitor reading only `inbox-events` reporting "no unread" while `message_recipients.read_ts` is null for that recipient |
| 9 | `PaneBindingUnverified` | `resolve_pane_identity('%1397')` returning `binding: "legacy-unverified"` |
| 12 | `AgentNameAmbiguous` | resolving the bare name `AmberGate` with no project scope, which matches agent ids **39 and 69** — must refuse, never silently pick one |

**#12 CARRIES NO FIRES-ON-KNOWN-BAD LEG TODAY, AND THAT IS RECORDED RATHER THAN PAPERED OVER.**
AmNative reports `agent-mail-native` is already structurally safe here: every `journey` function
takes `&ProjectKey` alongside `&AgentName`, and `ResumePoint` binds project + recipient + cursor
together so a position cannot travel without its owner. The ambiguous-resolution path is therefore
**unreachable through that API**.

But it holds because the known-bad input is **unconstructible**, not because anything refuses it —
and by this document's own rule (a defense that has never refused is not a defense),
**"unrepresentable" and "refuses" are different claims.** AmNative declined to report the second
while shipping the first, which is the correct call. So:

- **What is true today:** the bad input cannot be expressed against `agent-mail-native`'s types.
  Arguably stronger than a refusal, since there is no path to gate.
- **What is NOT true today:** that any code fires on a bare unscoped `AmberGate`. Nothing does,
  because nothing can receive it.
- **The residual gap:** callers that build arguments dynamically — from a config string, a message
  body, a pane label — can still resolve a bare name through some *other* surface. The typed
  `AgentNameAmbiguous` variant is a belt for exactly those, and it belongs with the identity work
  currently held by GreenFrog, not beside it.

Type-level unrepresentability is counted as satisfying #12's (B) **only** for calls that go through
`agent-mail-native`. It closes nothing on the CLI or raw-MCP paths, where 10 ambiguous names remain
resolvable by name alone.

### (C) rows — reproductions

**What my `curl` probe actually measured — and what it does NOT establish.** I recorded this as
"#8 verified by this lane independently of AmNative's diagnosis". **That label was wrong**, and it is
worth keeping the correction visible because the measurement is sound while the inference was not:

```
$ curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8765/health   → 200
$ curl -s -w "%{http_code}" http://127.0.0.1:8765/mcp/                  → 401 {"detail":"Unauthorized"}
```

All this shows is that **an unauthenticated caller** reaches `/health` and is refused at `/mcp/`,
which is `/mcp/` **working correctly** — it requires auth. The probe says nothing about the CLI,
because the probe was `curl` with no token, not `am`. The CLI has `HTTP_BEARER_TOKEN` /
`AGENT_MAIL_TOKEN` and authenticates fine. So this leg corroborated a *different* proposition than
the one I attached it to: I verified "unauthenticated `/mcp/` returns 401" and labelled it
"the CLI cannot reach the daemon". **The consequence I carried forward — that every CLI-derived
figure read `storage.sqlite3` rather than the daemon — is withdrawn.**

The residual true finding is narrow: **`am health` builds a throwaway probe SQLite and never
contacts the daemon** (AmNative's measurement, correctly made). Whether `am agent start`'s "no
listener" has some other cause is now **UNEXPLAINED and open**, not answered by #8.

**#7**, from the live store — `last_active` equals the re-registration second, not the work second:
`agents` id=69 `AmberGate` `last_active` = `2026-09-02 04:56:48`, one minute after phantom id=306
`WindyWren` was minted at `04:55:39`, which is when the re-registration happened.

## Validation

```bash
cd /Users/josh/Developer/mcp_agent_mail_rust
LIVE=/Users/josh/.local/share/mcp-agent-mail-rust-live/storage.sqlite3

# LEG 0 — ANTI-VACUITY: the shipped tag exists and matches the shipped binary, or FAIL.
BIN=$(am --version | awk '{print $2}')
TAG=$(git show "v$BIN:Cargo.toml" 2>/dev/null | sed -nE 's/^version = "(.*)"/\1/p' | head -1)
test -n "$TAG" || { echo "FAIL leg0: no tag v$BIN — cannot author against the shipped line"; exit 1; }
test "$TAG" = "$BIN" || { echo "FAIL leg0: tag says $TAG, binary says $BIN"; exit 1; }
echo "leg0 PASS shipped=$BIN tag=v$BIN version=$TAG"

# LEG 1 — the query is VALID: stock sqlite3 executes it and returns rows (empty = FAIL).
N=$(sqlite3 "file:$LIVE?mode=ro" "SELECT count(*) FROM (
      SELECT m.id, m.topic, COALESCE(a.name,'u') AS from_name, 0.0 AS score
      FROM messages m LEFT JOIN agents a ON a.id = m.sender_id
      WHERE m.project_id=107 LIMIT 5);")
test "${N:-0}" -gt 0 && echo "leg1 PASS stock sqlite3 answered the m.topic query (rows=$N)" \
                     || { echo "FAIL leg1: stock rejected it too — reclassify as a query bug"; exit 1; }

# LEG 2 — the schema HAS the column (absence would reclassify this as a schema bug).
sqlite3 "file:$LIVE?mode=ro" ".schema messages" | grep -qF 'topic TEXT COLLATE NOCASE' \
  && echo "leg2 PASS messages.topic exists" || { echo "FAIL leg2: column really is missing"; exit 1; }

# LEG 3 — the alias is always bound: exactly 2 FROM clauses, both 'messages m'.
F=$(git show v0.3.31:crates/mcp-agent-mail-db/src/search_planner.rs \
    | grep -cE '"messages m LEFT JOIN agents a ON a\.id = m\.sender_id"')
test "$F" -eq 2 && echo "leg3 PASS both planner FROM clauses bind m=messages" \
                || echo "leg3 NOTE FROM-clause count changed ($F) — re-localize"

# LEG 4 — FIRES-ON-KNOWN-BAD: the defect still reproduces on the installed binary.
am robot --json search 'OMP_MSG_SRC' >/tmp/amd_out.txt 2>/tmp/amd_err.txt; RC=$?
grep -qF 'column not found: m.topic' /tmp/amd_err.txt \
  && echo "leg4 PASS defect #3 reproduces (rc=$RC, engine error on stderr)" \
  || echo "leg4 NOTE #3 no longer reproduces — check for fsqlite>=0.3.14 and close the (A) row"
test -s /tmp/amd_out.txt && echo "leg4 WARN stdout non-empty — silent-success risk" \
                         || echo "leg4 PASS stdout empty, failure not silent"

# LEG 5 — what the unauthenticated probe ACTUALLY shows: /mcp/ requires auth. This is NOT
# evidence about the CLI (see the withdrawn #8). Kept because the posture itself is worth watching.
H=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8765/health)
M=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8765/mcp/)
test "$H" = "200" -a "$M" = "401" \
  && echo "leg5 PASS unauthenticated posture: /health $H open, /mcp/ $M requires auth (correct)" \
  || echo "leg5 NOTE posture changed (/health $H, /mcp/ $M)"

# LEG 5b — #4 and #8 REFUTED: the CLI routes through the daemon by default and authenticates.
CLI=$(git show v0.3.31:crates/mcp-agent-mail-cli/src/lib.rs)
printf '%s' "$CLI" | grep -qF '!direct || daemon_reachable' \
  && echo "leg5b PASS default path prefers the daemon (!direct || daemon_reachable)" \
  || { echo "FAIL leg5b: routing predicate changed — re-read before trusting the #4/#8 refutations"; exit 1; }
printf '%s' "$CLI" | grep -qF 'check AGENT_MAIL_TOKEN/HTTP_BEARER_TOKEN' \
  && echo "leg5b PASS the CLI emits a 401-from-daemon message, so it does call the daemon" \
  || echo "leg5b NOTE the auth error message moved"
# and the help text AGREES with the code, so the proposed 'inversion' row is not a defect
am inbox-events --help 2>&1 | grep -qF 'Allow a direct SQLite read only when no daemon is reachable' \
  && echo "leg5b PASS help text matches !direct || daemon_reachable (no inversion defect)" \
  || echo "leg5b NOTE help text changed — recheck the inversion claim"

# LEG 6 — upstream's remedy is an engine bump, not a SQL change.
git diff --quiet v0.3.31 origin/main -- crates/mcp-agent-mail-db/src/search_planner.rs \
  && echo "leg6 PASS search SQL unchanged upstream (engine bug)" \
  || echo "leg6 NOTE upstream edited the search SQL — re-read the diff before adopting"
git show origin/main:Cargo.toml | grep -m1 -E '^fsqlite'

# LEG 7 — #12 root: the name is ambiguous, and 0 is the OTHER agent's answer.
ROWS=$(sqlite3 "file:$LIVE?mode=ro" "select count(*) from agents where name='AmberGate';")
COLL=$(sqlite3 "file:$LIVE?mode=ro" \
  "select count(*) from (select name from agents group by name having count(*)>1);")
test "${ROWS:-0}" -ge 2 -a "${COLL:-0}" -ge 1 \
  && echo "leg7 PASS name ambiguous: AmberGate rows=$ROWS, colliding names=$COLL" \
  || echo "leg7 NOTE ambiguity gone (rows=$ROWS colliding=$COLL) — reclassify #12"
sqlite3 "file:$LIVE?mode=ro" "select a.id||' proj='||a.project_id||' unread='||
  (select count(*) from message_recipients r where r.agent_id=a.id and r.read_ts is null)
  from agents a where a.name='AmberGate' order by a.id;"

# LEG 8 — #13: the cached aggregate disagrees with ground truth (equality here would close the row).
GT=$(sqlite3 "file:$LIVE?mode=ro" \
  "select count(*) from message_recipients where agent_id=69 and ack_ts is null;")
CA=$(sqlite3 "file:$LIVE?mode=ro" "select ack_pending_count from inbox_stats where agent_id=69;")
test -n "$GT" -a -n "$CA" || { echo "FAIL leg8 vacuous: no rows for agent 69"; exit 1; }
test "$GT" != "$CA" && echo "leg8 PASS cache drift reproduces: ground_truth=$GT cached=$CA" \
                    || echo "leg8 NOTE cache now agrees ($GT) — close #13"

# LEG 9 — #11: `signaled` really is receipt-existence, and source really says "debounced".
git show v0.3.31:crates/mcp-agent-mail-tools/src/messaging.rs \
  | grep -qF '"signaled": !signal_receipts.is_empty(),' \
  && echo "leg9 PASS signaled == receipt-existence test" \
  || { echo "FAIL leg9: the boolean's computation changed — re-read before trusting row #11"; exit 1; }
git show v0.3.31:crates/mcp-agent-mail-tools/src/messaging.rs \
  | grep -qF 'a debounced or failed signal remains persisted but not signaled' \
  && echo "leg9 PASS source documents the debounced case (not a delivery gap)" \
  || echo "leg9 NOTE the documented semantics moved — #11 may need reopening"

# LEG 10 — read_ts really is on the DAEMON surface (verified at source, never by calling
# fetch_inbox, which marks messages read and is a stated non-goal).
git show v0.3.31:crates/mcp-agent-mail-tools/src/messaging.rs > /tmp/amd_msg.rs
grep -qF 'pub read_ts: Option<String>,' /tmp/amd_msg.rs \
  && grep -qF 'read_ts: row.read_ts.map(micros_to_iso),' /tmp/amd_msg.rs \
  && echo "leg10 PASS read_ts declared AND populated on the daemon inbox surface" \
  || echo "leg10 NOTE daemon read_ts wiring changed — recheck the two-authorities row"
grep -qF 'mark returned messages read' /tmp/amd_msg.rs \
  && echo "leg10 PASS fetch_inbox self-documents that it MUTATES read state (do not call it)" \
  || echo "leg10 NOTE fetch_inbox no longer documents the mutation"

# LEG 11 — #10 REFUTED: the ledger is contiguous, so CursorExpired is unreachable, and the
# deciding input never reaches the client.
read -r G T N <<<"$(sqlite3 "file:$LIVE?mode=ro" \
  "select min(seq)||' '||max(seq)||' '||count(*) from inbox_delivery_events;")"
test -n "$G" || { echo "FAIL leg11 vacuous: no delivery events"; exit 1; }
test "$G" = "1" -a "$T" = "$N" \
  && echo "leg11 PASS ledger contiguous (min=$G max=$T count=$N) -> retention_has_pruned=false -> the clamp is CORRECT" \
  || echo "leg11 NOTE ledger no longer contiguous (min=$G max=$T count=$N) -> a real prune may now exist; re-open #10"
# The only DELETE is test-only, which is why no prune exists. NOTE: `git grep -c` against a REV
# prefixes `<rev>:<path>:` to the count, so the bare output is never a number — strip it, or the
# check reports NOTE forever. (This bit me writing the leg.)
DEL=$(git grep -cF 'DELETE FROM inbox_delivery_events' v0.3.31 -- '*sync.rs' | sed 's/.*://')
test "${DEL:-0}" = "1" \
  && echo "leg11 PASS exactly one DELETE ($DEL), and it is inside mod tests" \
  || echo "leg11 NOTE DELETE count is $DEL — a production prune may have been added"
grep -qF 'simulate a pruned historical event' /tmp/sync331.rs 2>/dev/null \
  && echo "leg11 PASS that DELETE self-documents as a SIMULATED prune" \
  || echo "leg11 NOTE the simulate-a-prune marker is gone — re-read before trusting #10"
# and the deciding input is NOT exposed to callers
git show v0.3.31:crates/mcp-agent-mail-db/src/sync.rs \
  | sed -n '/pub struct InboxDeliveryEventPage/,/^}/p' | grep -qF 'global_oldest' \
  && echo "leg11 NOTE global_oldest is now exposed — a client guard becomes possible" \
  || echo "leg11 PASS global_oldest absent from the page: clients CANNOT decide expiry"

# LEG 12 — #3 ENGINE EXONERATION at the pinned version. Requires a one-time build:
#   cargo install fsqlite-cli --version 0.3.11 --root /tmp/fsq311
# Skips loudly rather than passing vacuously if that binary is absent.
FSQ=/tmp/fsq311/bin/fsqlite
if [ ! -x "$FSQ" ]; then
  echo "leg12 SKIP (not a pass): build it with 'cargo install fsqlite-cli --version 0.3.11 --root /tmp/fsq311'"
else
  test "$("$FSQ" --version | awk '{print $2}')" = "0.3.11" \
    || { echo "FAIL leg12: wrong fsqlite version — the exoneration is version-specific"; exit 1; }
  rm -f /tmp/mm_leg12.db
  sqlite3 /tmp/mm_leg12.db "CREATE TABLE agents (id INTEGER PRIMARY KEY, project_id INTEGER, name TEXT);
    CREATE TABLE messages (id INTEGER PRIMARY KEY, project_id INTEGER, sender_id INTEGER,
      thread_id TEXT, topic TEXT COLLATE NOCASE, subject TEXT, body_md TEXT,
      importance TEXT DEFAULT 'normal', ack_required INTEGER DEFAULT 0, created_ts INTEGER);
    INSERT INTO agents VALUES(1,107,'Alice');
    INSERT INTO messages(id,project_id,sender_id,subject,body_md,created_ts)
      VALUES(1,107,1,'hello reservation','body reservation',1);"
  OUT=$("$FSQ" /tmp/mm_leg12.db --command "SELECT m.id, m.subject, m.importance, m.ack_required,
      m.created_ts, m.thread_id, COALESCE(a.name, '[unknown sender]') AS from_name,
      a.id AS from_agent_id, m.body_md, m.project_id, 0.0 AS score, m.topic
    FROM messages m LEFT JOIN agents a ON a.id = m.sender_id
    WHERE m.project_id = 107 ORDER BY score ASC, m.id ASC LIMIT 50;" 2>&1)
  printf '%s' "$OUT" | grep -qF 'column not found' \
    && echo "leg12 NOTE the engine DOES reject m.topic at 0.3.11 — the exoneration is wrong, reopen the engine hypothesis" \
    || echo "leg12 PASS fsqlite 0.3.11 resolves m.topic (engine exonerated; fault is above the engine)"
  printf '%s' "$OUT" | grep -qF 'hello reservation' \
    && echo "leg12 PASS and it returned the row, so the query really executed" \
    || echo "FAIL leg12 vacuous: no error AND no row — the query did not run"
fi
```

All **twelve** legs run today, all PASS: leg0 `shipped=0.3.31 tag=v0.3.31 version=0.3.31`; leg1 `rows=5`
(stock sqlite3 answered the `m.topic` query); leg2 `messages.topic exists`; leg3 both planner FROM
clauses bind `m=messages`; leg4 `rc=1`, engine error on stderr, **stdout empty**; leg5
`/health 200 open, /mcp/ 401 requires auth (correct, and NOT evidence about the CLI)`; leg5b the
default path prefers the daemon, the CLI emits a 401-from-daemon message, and the help text matches
the predicate — so #4, #8 and the proposed inversion row are all refuted; leg6 search SQL unchanged
upstream with `origin/main` pinning
`fsqlite = "=0.3.14"`; leg7 `AmberGate rows=2, colliding names=10`; leg8 cache drift
`ground_truth=115 cached=47`; leg9 `signaled == receipt-existence` and the debounced case documented;
leg10 `read_ts` declared and populated on the daemon surface, and `fetch_inbox` self-documents that
it mutates read state; leg11 ledger contiguous `min=1 max=5226 count=5226` (so
`retention_has_pruned=false` and the clamp is correct), exactly one delete site and it self-documents
as a SIMULATED prune, and `global_oldest` absent from the page a client receives.

**Two legs invert the usual reading and must not be pattern-matched.** Leg 4 PASSES when the defect
**still reproduces on the installed binary** — its PASS means the work is *not* done, and its
flipping to NOTE is the signal to close the (A) row. Leg 8 likewise PASSES on *disagreement*.

Leg 8's numbers drift because the store is live: it read `113/46` when row #13 was written and
`115/47` at validation, so the gap widened from 67 to 68. The assertion is an **inequality** for
that reason; only a genuine convergence closes #13.

## Non-Coverage

- **The working checkout was not updated.** 1285 behind, 19 ahead, 15 dirty. Inspection was via
  `git show`/`git grep` against tags. Updating it is a separate, owner-coordinated action.
- **No minimal engine repro.** The `fsqlite 0.1.15` CLI did not reproduce and is the wrong version
  line; building `fsqlite 0.3.11` is the named next step.
- **No patch authored by this lane.** #3's (A) adopts upstream `c7a7083f`; nothing was written to
  `mcp_agent_mail_rust`.
- **No (B) code written.** `crates/agent-mail-native/` is AmNative's; the variants and known-bad
  inputs above are requests to it, delivered by `hub`.
- **Defects #2, #4, #5, #9 were not independently re-measured by this lane** — they are carried from
  AmNative, JourneySpine, and AmberGate with attribution. **#1, #3, #7, #8, #12, #13 were verified
  here** against the store or the live daemon.
- **#12 was reclassified, not re-measured end-to-end.** I did not reproduce "daemon 0 vs CLI 20" by
  calling both surfaces — deliberately, because `am inbox` risks mutating AmberGate's read state and
  that is a stated non-goal. I measured the *store* instead and found an ambiguity that fully
  accounts for a `0`. That is a strong explanation, not a captured round-trip.
- **Row #6's corrected field was verified only in the schema**, not by diffing the two surfaces'
  JSON keys myself; the eight-key list is carried from the orchestrator's correction.
- Reservation, thread, and attachment surfaces remain **UNMEASURED**.

## NO-CLAIM

A disposition is a decision about what to do, **not evidence that the defect is understood
correctly**. Specifically:

- **#3's root cause is an inference from three consistent legs, not from a minimal repro.** Stock
  accepting the SQL, the alias being bound, and the emitter living in `fsqlite-planner` together make
  the engine the only remaining candidate — but I did not reproduce the failure against pinned
  `fsqlite 0.3.11`, and until someone does, "fsqlite 0.3.11 mis-resolves `m.topic`" is the best
  supported hypothesis rather than a demonstrated fact. Adopting `c7a7083f` on this reasoning is
  cheap and reversible; **upstreaming a claim about fsqlite's planner on it would not be.**
- Upstream bumping the pin is **consistent with** an engine bug; it is not proof that this defect is
  what the bump fixed. The commit message cites GH#399, GH#402 and FTS5 compat, none of which
  mentions qualified-column resolution.
- **#12's reclassification is an explanation, not a captured round-trip.** Two `AmberGate` rows with
  `0` and `22` unread fully account for a surface reporting `0`, but I never observed the daemon
  resolving the name to id=39. An unscoped-name lookup is the most economical explanation; a
  project-scoped lookup that is separately broken would look identical from here.
- **Eleven defects, not nine, and still a floor** — one evening, one machine, one version. The set is
  biased toward the identity and comms path we happened to exercise, and rows #12 and #13 were both
  found by looking at the store rather than by using the product, which is a different sampling bias
  again.
- (B) rows are **requests**, so their fires-on-known-bad legs are unrun by definition until AmNative
  lands them. This document does not claim any defense currently refuses anything.
- **The disposition letters are decisions, not outcomes.** Nothing here has yet changed the binary
  the fleet talks to, and leg 4 passing is the standing proof of that.
- **Four of thirteen rows were not defects, and the list's own error rate is the finding.** #4, #8
  and #10 are refuted outright and #11 was documented behaviour. #8 was the worst: its evidence was
  never gathered at all, because the one probe that would have settled it (`env | grep ...`) was
  **DENIED by dcg and never retried**, and absence-of-a-probe was then written down as
  absence-of-a-token. **A denied or erroring probe yields UNKNOWN, never a negative result** — the
  honest move is to retry differently (`printenv` was available the whole time) or to say unknown.
  This document inherited that error and propagated it for several revisions.
- **My own `curl` leg was mislabelled for those revisions**: it measured "unauthenticated `/mcp/`
  returns 401" and was captioned "the CLI cannot reach the daemon". The probe never involved the
  CLI. Corrected above and in leg 5.
- **`oracle_skew=0` is weaker than earlier revisions claimed.** It compares two HTTP routes on one
  daemon process under one token, so it evidences self-consistency, not corroboration against the
  store. The only genuinely independent oracle used here is read-only SQL against the store file.
- **`am agent start`'s "no listener" is now UNEXPLAINED.** #8 was invented to explain it; #8 is
  false, so the contradiction is open again and nothing in this document accounts for it.

## Cross-References

- `docs/contracts/kernel_only_policy.md` — fixing a broken kernel IS the work
- `crates/agent-mail-native/` — AmNative's typed binding, owner of every (B)
- Bead `omp-orchestrator-3r1r` — the nine defects with their original reproductions
