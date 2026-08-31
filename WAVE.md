# WAVE.md — who is who, who owns what, and how we talk

The live coordination contract for the `omp-orchestrator` NTM session. Read `AGENTS.md` first —
this file says only who does what and how you communicate.

---

## Roster (measured 2026-08-31, `tmux list-panes` + process profile)

| Pane | tmux id | Agent | Model | Role |
|---:|---|---|---|---|
| 0 | `%1396` | — | shell | Operator scratch. Not an agent. Never dispatch here. |
| 1 | `%1397` | `omp-claude` | Opus 5 (anthropic, OAuth) | **Integrator.** Owns cross-crate decisions and the final merge. |
| 2 | `%1408` | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | **Gate owner.** |
| 3 | `%1409` | `omp-glm` | GLM 5.3 (openrouter, 1.3M) | **Portability owner.** |
| 4 | `%1398` | `omp-codex` | GPT-5.6-Luna (OAuth) | **Extraction owner.** |
| 5 | `%1399` | `omp-codex` | GPT-5.6-Luna (OAuth) | **Verification owner.** |

**Pane indices are not stable handles.** They shift when panes are added or removed — measured
twice in one session. Address by `pane_id` (`%1408`) and re-resolve indices immediately before use:

```bash
tmux list-panes -a -F '#{pane_id} #{session_name}:#{window_index}.#{pane_index}'
```

---

## Assignments

Each pane owns **one bead** and the files that bead names. No two panes hold the same file.

| Pane | Bead | Owns |
|---:|---|---|
| 2 | `omp-orchestrator-4ak` | The no-shell/no-python gate. **Lands first**, on an empty tree. |
| 3 | `omp-orchestrator-7ai` | Kill every hardcoded path. 3 repo-root + 6 home refs, measured. |
| 4 | `omp-orchestrator-815` | Extract 23 crates, deps-first. **Blocked on 3 and on the control-plane commit.** |
| 5 | `omp-orchestrator-5cl` + `-a3p` | Forbid unsafe on all 23; then the unwired-lane conformance test. |
| 1 | `omp-orchestrator-kxe` | The lifecycle binary. Integrates the rest; **starts last on purpose.** |

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
