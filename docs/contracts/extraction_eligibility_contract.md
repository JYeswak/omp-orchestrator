# Extraction Eligibility Contract

Bead: `omp-orchestrator-extraction-eligibility`

## Purpose

This contract decides, mechanically, **which `.sh` scripts become crates in this repo and which do
not.** It exists because that decision was never written down, so extraction proceeded on a
predicate the orchestrator invented, and six crates landed that blocked every commit in the
repository.

The rule is not a list. A list goes stale the moment control-plane adds a script. The rule is three
predicates, each derivable by command, and a script is eligible only if **all three** hold.

## Contract Artifacts

### EE-1 — the three predicates

A `.sh` (or `.py`) becomes a crate here **iff all three hold**:

|id|predicate|command|eligible when|
|---|---|---|---|
|`EE-P1`|**Concern is ours.** The script serves the OMP orchestration lifecycle — observe, select, claim, dispatch, verify, close.|`grep -rhoE '/Users/[a-z]+/(\.claude\|Developer/[a-z-]+)' <src>` |names no OTHER repo as its subject|
|`EE-P2`|**It is a PORT, not a wrapper.** The crate implements the logic in Rust and never routes a `.sh`/`.py` into `Command::new`, an argv vector, or `fs::read*`.|see `EE-P5` — the naive string count over-reports|`0` real sites|

`EE-P2` is the load-bearing one and the one that was missing.

### EE-2 — a wrapper is not a port

A crate that execs a `.sh`/`.py` is **a path dependency wearing a crate's name.** It is correct in
the repo that holds the script and **inert everywhere else.** Porting it moves the name and leaves
the behaviour behind.

Measured, 2026-09-02: `crates/reap-finished-panes` invokes `bin/pane-result-reaper.sh`. That script
is **tracked in control-plane** (20,512 bytes) and was **never in this repo** — `git ls-files bin/`
returns 0 and `git log --all --diff-filter=D -- 'bin/*'` returns 0, so nothing deleted it. The crate
refuses in the OBSERVE phase every cycle, **31 distinct pids across the last 60 ledger rows**,
launchd last exit 1. It never reaches the dispatch path. `refill-idle-panes` is the same class and
supervised the wrong repo for a night on inherited literals.

Extraction batch 1 then imported **six more instances of the same shape.** Exec-target counts in
`src`, measured after they landed:

|crate|`.sh`/`.py` targets|where the targets live|
|---|---:|---|
|`zestgraph-hook-substrates`|10|`~/.claude/hooks/`, `~/Developer/foundry/loop-kit/`|
|`slb-guard-fail-closed`|5|wraps `slb_guard.py`|
|`gate-chain`|4|control-plane `bin/`|
|`schedule-drift-check`|2|control-plane `bin/`|
|`stack-drift`|1|control-plane `bin/`|
|`br-comment-form-check`|1|control-plane `bin/`|

`zestgraph-hook-substrates` is the decisive case. Its literals are
`const SCRIPT: &str = "/Users/josh/.claude/hooks/accretive-write-gate.sh"` and five siblings. All
six targets **exist**, all are `.sh`, and **every one lives outside this repository** — in the
user's home and in the `foundry` project. There is **no repo-relative form**, so the marker-walk
repair prescribed by bead `7ai` cannot resolve them. It is not a portability defect. It is an
ineligible crate.

### EE-3 — the four verdicts

|verdict|meaning|action|
|---|---|---|
|`PORT`|passes P1, P2, P3|extract; the shell may remain **only** as a byte-stable differential oracle|
|`WRAPPER`|fails P2|**do not extract.** Either port the logic (a new bead) or leave it|
|`TERMINAL`|fails P1 — another repo's concern|leave it. Not ours at any fidelity|
|`UNNEEDED`|passes P1 and P2, fails P3|leave it until a caller exists. An imported crate with no caller is `BUILT != WIRED`|

### EE-4 — the classification, 36 control-plane-only crates

By `EE-P2` alone, **25 of 36 are `WRAPPER`** and were never eligible. Only these 11 have zero exec
targets: `admission-reason`, `cargo-git-reaper`, `cargo-lane-budget`, `codex-fleet-recovery`,
`crate-soundness-verify`, `docs-staleness-gate`, `fleet-arc-report`, `harvest-triage`,
`repo-hygiene-wired-proof`, `scheduled-lane-migration-check`, and the non-crate
`dispatch_cli_contract.rs`.

Of those 11, `EE-P1` disqualifies more on **ownership**, which no name-based reading would catch:

|crate|names as its subject|verdict|
|---|---|---|
|`fleet-arc-report`|`Developer/clutterfreespaces`|`TERMINAL` — CFS's|
|`repo-hygiene-wired-proof`|`Developer/zesthooks`|`TERMINAL` — zesthooks'|
|`harvest-triage`, `codex-fleet-recovery`|`Developer/control-plane`|`TERMINAL` — control-plane's own lane|

`arc-checkin` (3 exec targets) and `cfs-honesty-tick` (2) also name `clutterfreespaces`: `WRAPPER`
**and** `TERMINAL`.

### EE-P5 — the naive predicate over-reports, and three field corrections

**Corrected 2026-09-02 by two agents who re-derived `EE-P2` rather than trusting it.** The original
wording — "execs no `.sh`/`.py`" checked by counting `.sh`/`.py` string literals — is the same
shape as the handrolled predicate that caused this halt: it scores strings, not behaviour.

On the five batch-3 crates it counted **9 sites where 3 are real**:

|crate|naive|real|what the difference was|
|---|---:|---:|---|
|`scheduled-lane-doctor`|2|**0**|both hits are `.strip_suffix(".sh")` / `(".py")` extension tests in `fn lane_stem()`; a third, `.strip_suffix(".rb")`, the predicate does not match at all. Its `Command::new` sites spawn `crontab -l` and `ps`. **It PASSES `EE-P2`** — its verdict rests on `EE-P1`, and the reason first given for it was wrong|
|`close-evidence-gate`|4|**1**|two are `#[test]` fixtures; `main.rs:300/:498` **join** `bin/fleet-gate-coverage.sh` and never exec it — `resolve::sibling_repos` does `fs::read_to_string` and greps `FLEET_REPOS`/`SIBLING_REPOS` arrays out of it. **The `.sh` is a CONFIG SOURCE parsed as text.** A real `EE-P2` failure by a mechanism the original wording misses — and scraping an array literal is **harder** to port than an exec, not easier|
|`arc-keepalive`|2|2|both real, both `config.cp.join(...)` onto the **control-plane** root|
|`apfs-wave-guard`|1|1|wrapper by its own header: "The APFS script remains the measurement authority. This crate only invokes that implementation."|

**The corrected predicate.** A site counts when a `.sh`/`.py` literal flows into `Command::new`, an
argv vector, **or `fs::read*`**; and does NOT count inside `#[cfg(test)]`, a doc comment, or a
`strip_suffix`/`ends_with` extension test. **Anti-vacuity: zero sites across all crates is an
ERROR**, never a pass.

**Two further corrections, both from the field:**

- **The reference shape itself carries a literal.** `omp-idle-dispatch` — named in three dispatch
  packets as the canonical `REPO_MARKERS`/`REPO_ENV`/`ConfigError` exemplar — has
  `DEFAULT_PATH = "/opt/homebrew/bin:/Users/josh/.local/bin:…"` at `src/main.rs:39`. **The exemplar
  for literal-free resolution would fail the guard it exemplifies.** An agent copying it faithfully
  would have imported a literal.
- **A vendored `#[path]` module can add a wrapper edge invisibly.** `coordinator-reservation-preflight`
  reads 2 in control-plane and **3** after extraction, because the vendored
  `scheduled_lane_telemetry.rs` itself execs `bin/lib/scheduled-lane-telemetry.sh`. The repair
  imported a third wrapper edge while treating a symptom.

### EE-P4 — a port needs an ORACLE that survives the move

**Measured 2026-09-02.** All three eligible ports compile here and each has failing differential
tests: `admission-reason` 21 passed / 3 failed, `cargo-lane-budget` 7/1, `crate-soundness-verify`
12/1. Every failure is one class — `src/` execs nothing, but the **differential tests spawn
`bin/<name>.sh` as the equivalence oracle** and get `127` / `NotFound`.

So a crate can pass `EE-P1`, `EE-P2` and `EE-P3` and still be **unverifiable here**: its only
proof-of-equivalence lives in a `.sh` this repo forbids. Per `AGENTS.md` L3 every project names its
oracle; an extracted port must name one that exists on **this** side of the boundary — a golden
fixture, a vendored transcript, or a property test — or state plainly that it is compiled but not
proven equivalent. **"It builds and its non-differential tests pass" is the honest claim** for all
three today.

## Validation

Run from the repository root. Each command is the predicate, not a proxy for it.

```bash
# EE-P2 — the one that was missing. Non-zero means WRAPPER: do not extract.
grep -rhoE '"[^"]*\.(sh|py)"' ../control-plane/crates/<name>/src/ | wc -l

# EE-P1 — whose concern. A foreign repo in the output means TERMINAL.
grep -rhoE '/Users/[a-z]+/(\.claude|Developer/[a-z-]+)' ../control-plane/crates/<name>/src/ | sort -u

# EE-P3 — does anything here need it.
grep -rl '<name>' crates/*/src/*.rs | wc -l

# The arrival gate. THE AUTHORITATIVE SURFACE, not a standalone guard run.
.git/hooks/pre-commit   # expect exit 0
```

**The hook is authoritative and a standalone guard is not.** Measured: `cargo test -p
path-literal-guard` reports **2 passed / 0 failed** against the very worktree the hook **refuses**.
An agent citing the standalone guard reports green on a repo nobody can commit to. This is the same
family as `plan-assemble --check` falling through to the **WRITE** path, and `refill-idle-panes`
rendering a two-surface contradiction as *"nothing to do"* at exit 0: **the cheaper surface is the
one that lies.**

## Cross-References

- `AGENTS.md` "The one rule" — no `.sh`, no `.py`. This contract supplies the **runtime** half the
  one rule lacks: that rule checks file extensions in `git ls-files` and, as AGENTS.md states,
  "does not prove no crate shells out at runtime via `std::process::Command` — a separate, unbuilt
  check."
- `AGENTS.md` "no denominator, so there is no percent ported" — the classification above IS that
  denominator, and it is 11 candidates, not 20, 23, or 36.
- Bead `7ai` "Installable from anywhere" — CLOSED, and a closed portability bead is a **snapshot,
  not an invariant**: nothing refused a new crate that violated it. Bead `io3h` is the arrival gate.
- `fh N040` — a replacement claim needs a smoke check at **both** ends; the control-plane copy is
  deleted in a separate bead, never in the extraction.
- `docs/contracts/orchestration_contract.md` `OC-L4` — every claim carries its command.

## Non-Coverage

- Does **not** decide the 17 non-leaf crates' ORDER. Eligibility is per-crate; sequencing is a
  deps-first plan and a separate artifact.
- Does **not** cover `bughunt-tick` (8 exec targets, 13,927 LOC, 59 home literals). It is
  `WRAPPER` by `EE-P2` and needs its own decision, not a batch slot.
- Does **not** decide whether a `WRAPPER`'s logic is WORTH porting. It says only that copying the
  crate is not a port. `reap-finished-panes` is the live instance: port its logic, or declare the
  crate a stub here and say so.
- Does **not** address the `state-wildcard-lint` findings (bead `ewnr`) — a different gate.

## NO-CLAIM

`EE-P2` is a **string predicate over `src`**, so it is a floor, not a proof. A crate can shell out
through a path built at runtime, through an env var, or through a helper crate, and score `0`. It
catches the literal case — which is every case measured so far — and does not establish that a
zero-scoring crate never spawns a script.

`EE-P1` reads which repo a crate NAMES. A crate can serve another repo's concern while naming no
path at all; ownership then needs a human reading, not a grep.

**The classification above is unverified by a second agent.** The exec-target counts come from the
orchestrator's own commands, and the orchestrator's probes have been wrong repeatedly in this
session — a `$?` after a pipe that captured `grep`'s status, a pane-identity probe whose input
contained every agent name because the orchestrator had typed them all, and a `17/4/13` routing
census retracted as wrong in both repos. Treat the eleven-candidate figure as a claim needing a
grader, not a settled denominator.
