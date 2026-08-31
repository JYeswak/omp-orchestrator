# WAVE.md — who is who, who owns what, and how we talk

The live coordination contract for the `omp-orchestrator` NTM session. Read `AGENTS.md` first —
this file says only who does what and how you communicate.

---

## Roster — re-measured 2026-08-31 07:05Z

Keyed on `pane_id`, which is the only stable handle. The `Idx` column is a convenience that
**goes stale** — it already did: the previous version of this table listed `%1398`/`%1399`, which
exist in no live session, and inverted the codex/glm profiles. That cost a real lost handoff
(see below). Re-measured with:

```bash
tmux list-panes -a -F '#{pane_id} #{session_name}:#{window_index}.#{pane_index} #{pane_title}'
```

| tmux id | Idx | Agent | Model | Mail identity | Role |
|---|---:|---|---|---|---|
| `%1396` | 0 | — | shell | — | Operator scratch. Not an agent. Never dispatch here. |
| `%1397` | 1 | `omp-claude` | Opus 5 (anthropic, OAuth) | — (registration blocked, `-ygc`) | **Orchestrator / integrator.** Cross-crate decisions, verification, the only closer. |
| `%1413` | 2 | `omp-codex` | GPT-5.6-Luna (OAuth) | `GreenFrog` | **Extraction owner.** |
| `%1414` | 3 | `omp-codex` | GPT-5.6-Luna (OAuth) | `BlueLantern` | **Safety + conformance owner.** |
| `%1408` | 4 | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | `AmberGate` | **Gate owner.** |
| `%1409` | 5 | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | `GoldLark` | **Portability owner.** |

**Roster names are not pane ids, and the mapping is not guessable.** `GoldLark` is the mail
identity reserving `-7ai`'s files; the prose roster calls the same worker `SilverWolf`. Treat the
mail identity as authoritative for reservations and the `pane_id` as authoritative for delivery.

**The measured cost of trusting a stale roster:** at 06:58Z the gate owner closed `-4ak` and sent
its "gate landed, extraction unblocked" handoff to **`%1398`** — read straight out of the old table
above. `%1398` does not exist, so `GreenFrog` (`%1413`), the actual extraction owner, never received
it. A send to a dead pane id is silent. The orchestrator re-delivered it at 07:09Z, corrected,
and verified receipt at the receiving end. Tracked as `omp-orchestrator-75l`.

---

## Assignments

Each pane owns **one bead** and the files that bead names. No two panes hold the same file.

| Pane | Bead | Status 07:10Z | Owns |
|---|---|---|---|
| `%1408` | `omp-orchestrator-4ak` | **CLOSED** `3f821d4` | The no-shell/no-python gate. Landed first, on an empty tree. |
| `%1409` | `omp-orchestrator-7ai` | in_progress | Kill every hardcoded path. 3 repo-root + 6 home refs, measured. |
| `%1414` | `omp-orchestrator-5cl` + `-a3p` | `-5cl` in_progress, `-a3p` actionable | Forbid unsafe on all 23; then the unwired-lane conformance test. |
| `%1413` | `omp-orchestrator-815` | blocked on `-7ai` only | Extract 23 crates, deps-first. |
| `%1397` | `omp-orchestrator-kxe` | blocked on `-815` | The lifecycle binary. Integrates the rest; **starts last on purpose.** |

**`-815`'s stated precondition "commit the port in control-plane first" is CLEARED** and must not
be re-derived: all 12 files of the three ported crates are committed at `45c613d`, plus `8fc3e4b`
for the unsafe lint. What still blocks `-815` is `-7ai` alone — `GoldLark` holds four uncommitted
`src/*.rs` files in those crates, and copying them now would either take a dirty tree or silently
drop `-7ai`'s work.

**Ordering is not advice.** The gate lands before any crate is copied, or the repo starts dirty and
the gate gets weakened to make the build pass. Portability lands before extraction, or the new repo
inherits `/Users/josh/Developer/control-plane` — which **compiles** after a move and then silently
reads the wrong repo.

---

## Comms

**NTM is the notification layer. Agent Mail is not.** Mail's two real jobs are file reservations and
the durable record. Using mail to page someone makes coordination depend on an agent *choosing* to
check an inbox — a hope, not a mechanism.

| Question | Surface |
|---|---|
| "here is your next unit of work" | `ntm --robot-send=omp-orchestrator --panes=N --msg='<packet>'` |
| "I am about to edit these paths" | Agent Mail **reservation** — mandatory before shared-file edits |
| "the durable record of what I decided and why" | Agent Mail thread + a bead comment |
| "are you done / are you stuck" | `ntm --robot-wait` / a two-capture read — **never a mail poll** |

Agent Mail is healthy as of 2026-08-31 (`127.0.0.1:8765/health` → `status: ready`).

**Reserve before editing.** `file_reservation_paths(project_key="/Users/josh/Developer/omp-orchestrator",
agent_name=<yours>, paths=[...], reason="<bead-id>")`. Reserve the **narrow** set you will actually
edit — never `**/*`, which serialises the whole wave. Release when done.

**Thread on the bead id.** `thread_id="omp-orchestrator-4ak"`, subject prefixed the same way. One
thread per bead means the next agent can reconstruct the decision without asking.

**Delivery is not receipt.** `--robot-send` reports success when the text lands in a composer, not
when it is read. Verify at the **receiving** end by capturing the target pane, and sweep **all**
panes with a positive control — a packet delivered to the wrong pane looks identical to one
delivered nowhere.

**A pane is idle only on two captures.** `Working (27s)` and a frozen pane render identically.
Compare the timer **and** a spinner-stripped content hash ≥75s apart, reading the **last** status
line — a stale spinner sits in scrollback and will tell you a dead pane is alive.

---

## Standing rules for every pane

**Consult `fh` before you build.** Not once — at every non-obvious decision.

```bash
fh suggest "<the thing you are about to write>"
fh why <row-id>          # provenance before you believe it
```

`fh` already answered two questions in this wave: `beads_rust`'s `canonical_source_repo(beads_dir)`
is the repo-discovery pattern for `-7ai`, and `franken_lean`'s `UNWIRED_LANE_ALLOWANCE: &[] ` (an
**empty** allowlist) is the mechanism for `-a3p`. Reuse both; do not re-derive them. `fh` prints a
`STALE` banner when its ledger is old — read it and say so, because a row's age is part of the
citation.

**Search by meaning first.** `mcp__socraticode__codebase_search`, then `fh`. `grep` is the follow-up
that jumps to a line one of them surfaced, never the opening move.

**Every gate proves it bites.** Fires-on-known-bad **and** a known-good leg — an attack-only suite
ships an over-strict gate, and an over-strict gate gets routed around, which is a slower death than
no gate. Add a mutation that turns the leg RED, and restore byte-identically.

**Anti-vacuity.** An empty scan set is an **ERROR**, never a pass.

**Re-run, don't read.** A test that passed in CI yesterday is inadmissible. A test passes
meaningfully only when it (a) exists, (b) exits 0, **and** (c) asserts non-trivially against the
production path.

**Close reasons start with** `MUTATION-VERIFIED` / `DONE` / `APPROVED` / `WONTFIX`, or policy
refuses them and the refusal scrolls past unnoticed. Read the status back to confirm.

**Commits are path-scoped:** `git commit -- <explicit paths>`. Never `-A`. Messages carry a
verification-level tag.

**`br` reads the cwd.** `cd /Users/josh/Developer/omp-orchestrator` before any bead command, or you
will file into control-plane's tracker — which is exactly what happened once already.

---

## Known state, so nobody re-derives it

- **The port already happened.** `omp-idle-dispatch` (1033 LOC), `wired-but-inert-guard` (984),
  `fleet-composite` (1047) exist in control-plane, build clean, **29 tests pass**. The four
  `.sh`/`.py` originals are deleted. **It is UNCOMMITTED** — it must be committed in control-plane
  before extraction touches it.
- **Extraction set is 23 crates, not 20.** The three above are new.
- **Unsafe tally is 3 of 23**, not 2 of 20. `omp-idle-dispatch` already forbids it;
  `wired-but-inert-guard` and `fleet-composite` do not.
- **`cargo test` emits `error: unclosed table, expected ]`** pointing at asupersync's
  `tests/fixtures/migration_readiness_planner/malformed/Cargo.toml`. That is a **deliberate
  known-bad fixture in a vendored dependency**, not our break. Do not chase it.

---

## Tracker-query traps — measured, and each produced a CONFIDENT ZERO

Three defects on 2026-08-31 had one shape: **a query returned zero, and the zero was read as
"the data is absent" when the data existed and the query was wrong.** A confident zero is
indistinguishable from a healthy empty result, which is why all three survived review.

| Wrong query | What it actually returns | Right query |
|---|---|---|
| `br dep list <id>` to ask "what depends on this" | **OUT-edges only** — what `<id>` depends on | read the children, or `.triage.recommendations` |
| `bv --robot-triage \| jq '.triage.quick_ref.top_picks[]'` | a WEAKER view: `unblocks=0` on everything, omits high scorers | `jq -r '.triage.recommendations[:8][] \| "\(.id) score=\(.score)"'` — rank by `score`; `unblocks` is not even a key there |
| `git log --all --grep=<bead-short-id>` | matches the **tracker** commit (`chore(beads): consolidate…`) for unrelated ids | join on a SHA cited in the bead and verify it with `git cat-file -e` |

**`br show` returns a BARE list; `br list` wraps rows in `.issues`.** Guessing a jq path across
the two produces confidently wrong readings.

> **The generalisation, kept verbatim because two agents reached it independently within one
> hour (recorded in control-plane `cp-khil9`): "envelope field, not substrate."** Check the
> other fields of an envelope before diagnosing the tool.

**MATERIALIZED_ORPHAN = 0 as of 10:55Z.** An earlier note said 3 (`-gfb`, `-ygc`, `-6gq`). That
figure was produced by the out-edge bug above: `-gfb` had an in-edge from `-kxe` the whole time.
`-ygc` is closed; `-6gq` now carries a `related` edge to `-815`. **Do not re-derive 3 from a
stale note** — an orphan requires zero edges in BOTH directions, and `crates/tick-monitor`'s
`lifecycle` subcommand now computes in-degree in a second pass:

```bash
tick-monitor lifecycle --repo <repo> [--repo <repo>]
```

**The DAG is NOT edgeless.** Measured on the control-plane tracker: 1412 of 1864 records carry
dependency rows (1287 parent-child, 173 blocks, 5 discovered-from, 2 related). The real shape is
~7.4:1 **taxonomy over sequencing** — beads filed into an epic and never sequenced against each
other. So `unblocks=0` is TRUE and is an authoring gap, not a `bv` defect.

**Parent-epic inversion.** `br create --deps blocks:X` means *this bead depends on X*, which is
the inverse of what it reads like. A parent epic blocking its own child makes BOTH permanently
unclosable. `br create` does not show you the edges it made — **`br dep list` readback is the
only thing that catches it**, and `--force` with a recorded reason is correct where the graph is
already inverted.
