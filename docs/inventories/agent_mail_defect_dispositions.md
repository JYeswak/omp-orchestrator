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

**ANSWER TO THE QUESTION THE BEAD ASKS: neither. The engine is wrong.** Three independent legs:

1. **The query is valid.** That exact SELECT/FROM/WHERE/ORDER BY, run against the live store under
   **stock `sqlite3` 3.51.0**, returns rows — ids 40792, 40791, 40788. It is not rejected, it is
   answered.
2. **The alias is always bound.** The planner contains exactly **two** FROM clauses
   (`grep -nE '"(messages|message_fts|messages_fts)[^"]*"\.to_string\(\)'` → lines 813, 823), both
   `messages m ...`. The FTS branch cannot be the culprit: `PlanMethod::Empty | PlanMethod::TextMatch
   => unreachable!()` at `search_planner.rs:826`. So the "unbound alias `m`" hypothesis — which
   would have made this a query bug — is refuted.
3. **The error originates inside the engine.** The string `column not found: m.topic` is not
   Agent Mail's; it is `fsqlite-planner`'s `PlannerError::ColumnNotFound` at `lib.rs:172`. The
   engine is **FrankenSQLite**, not stock SQLite: `git show v0.3.31:Cargo.toml` →
   `fsqlite = "=0.3.11"`.

**Upstream's own remedy corroborates the engine diagnosis.** Between `v0.3.31` and `origin/main` the
search SQL is untouched — `git diff --stat v0.3.31 origin/main -- .../search_planner.rs` reports no
change to that file — while the single commit matching `topic|search|fsqlite|fts` is `c7a7083f`
*"deps: bump fsqlite =0.3.11 -> =0.3.14 (GH#399 corruption wave + GH#402 checkpoint watermark +
**FTS5 stock-compat**)"*. Pin progression: `v0.3.31` `=0.3.11`, `v0.3.32` `=0.3.11`,
`origin/main` `=0.3.14`. **The shipped 0.3.31 binary predates the fix, and the fix is a dependency
bump, not a SQL change** — exactly what an engine bug looks like when it is repaired.

**What I could NOT prove, stated plainly.** I tried to reduce this to a minimal upstreamable repro
against the engine and **failed to reproduce**. A synthetic two-table DB with the same shape,
queried through `~/.cargo/bin/fsqlite`, returned the row correctly (`1 | 'hello' | 'Alice' | 1 |
0.0 | NULL`). That is **not** evidence the engine is fine: the available CLI is `fsqlite 0.1.15`,
a different version line from the pinned `=0.3.11` library, so it is not a valid test of the pinned
code. A true minimal repro requires building `fsqlite 0.3.11` and is the named next step. Nothing in
this document claims the minimal repro exists.

## The disposition table

Non-zero (A) and non-zero (B), per the anti-vacuity condition.

| # | defect | disp | action |
|---|---|---|---|
| 1 | `register_agent` accepts `agent_name`, silently mints a new identity | **B** | typed `RegisterKeyRejected` + mandatory read-back |
| 2 | `register_agent` accepts `pane_id` and discards it, success envelope | **B** | read-back assertion → typed `RegisterFieldNotPersisted` |
| 3 | `am robot search` RED for the entire corpus | **A** | adopt upstream `c7a7083f` (fsqlite `=0.3.14`); + **B** `SearchUnavailable` so an error is never read as empty |
| 4 | `--direct` SQLite fallback does not announce itself | **B** | probe `/health` first; label source; typed `SourceAmbiguous` |
| 5 | `mail_pending` default path never terminates and drops the cursor | **B** | wrapper always supplies a ceiling; typed `WaitCanceledWithoutCursor` distinct from "no mail" |
| 6 | `inbox-events` carries no read state | **B** | reconcile pair: `inbox-events` cursor + the CLI's `priority` (NOT `read_ts` — see correction below) |
| 7 | `last_active` is registration recency wearing an activity name | **C** | named finding; never use as a work oracle |
| 8 | the `am` CLI does not talk to the authenticated daemon | **C** | named finding; house pattern = daemon-primary, CLI-as-differential-oracle |
| 9 | every pane binding resolves `legacy-unverified` | **B** | typed `PaneBindingUnverified`; reap with existing `cleanup_pane_identities` |
| 12 | "daemon 0 unread vs CLI 20 rows" — **NOT a read-state disagreement; it is agent-identity resolution** | **B** | typed `AgentNameAmbiguous`; every name lookup MUST carry a project scope |
| 13 | `inbox_stats.ack_pending_count` drifts from ground truth (113 vs 46) | **C** | named finding; never read the cached aggregate as truth |

Totals: **(A) 1, (B) 7, (C) 4** — one row (#3) carries both an (A) and a supporting (B).

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

### Correction to a row inherited from the orchestrator

The broadcast that `am inbox --json` rows carry `read_ts` is **wrong**, and defect #6's action above
is corrected accordingly. Measured keys are eight: `ack_status, age, from, id, importance,
priority, subject, thread` — no `read_ts`, no `topic`. Read state on the CLI surface is `priority`.
True per-recipient read state lives in the store as **`message_recipients.read_ts`** (schema
confirmed: `read_ts INTEGER, ack_ts INTEGER, PRIMARY KEY(message_id, agent_id)`), which is the
arbiter both surfaces should be checked against. The *conclusion* — a monitor needs both surfaces —
stands; only the field name changes.

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

### (C) rows — reproductions

**#8**, verified by this lane independently of AmNative's diagnosis:

```
$ curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8765/health   → 200
$ curl -s -w "%{http_code}" http://127.0.0.1:8765/mcp/                  → 401 {"detail":"Unauthorized"}
```

An unauthenticated caller gets `ready` from `/health` and `401` from `/mcp/`. That is the
**auth-failure-reported-as-absence** root: `am agent start` says "no listener" while the daemon is
up. Consequence carried forward: every CLI-derived Agent Mail figure cited tonight read
`storage.sqlite3` directly rather than the daemon.

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

# LEG 5 — #8 root: unauthenticated /health ready while /mcp/ refuses.
H=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8765/health)
M=$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8765/mcp/)
test "$H" = "200" -a "$M" = "401" && echo "leg5 PASS #8 auth-as-absence reproduces (/health $H, /mcp/ $M)" \
                                  || echo "leg5 NOTE posture changed (/health $H, /mcp/ $M)"

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
```

All eight legs run today, all PASS: leg0 `shipped=0.3.31 tag=v0.3.31 version=0.3.31`; leg1 `rows=5`
(stock sqlite3 answered the `m.topic` query); leg2 `messages.topic exists`; leg3 both planner FROM
clauses bind `m=messages`; leg4 `rc=1`, engine error on stderr, **stdout empty**; leg5
`/health 200, /mcp/ 401`; leg6 search SQL unchanged upstream with `origin/main` pinning
`fsqlite = "=0.3.14"`; leg7 `AmberGate rows=2, colliding names=10`; leg8 cache drift
`ground_truth=115 cached=47`.

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

## Cross-References

- `docs/contracts/kernel_only_policy.md` — fixing a broken kernel IS the work
- `crates/agent-mail-native/` — AmNative's typed binding, owner of every (B)
- Bead `omp-orchestrator-3r1r` — the nine defects with their original reproductions
