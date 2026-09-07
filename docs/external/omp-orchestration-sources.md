# External OMP orchestration sources — saved, and VERIFIED AGAINST OUR VERSION

**Purpose.** Four external projects orchestrate OMP. Three of them independently confirm rules this
repo derived the hard way, and all four use OMP's *programmatic* surface — which is the gap
`AGENTS.md`'s fifth rule names ("the crates exist to orchestrate OMP, and today they scrape it").

**Read the VERDICT column before adopting anything.** Every source is pinned to an OMP older than
ours, and terminal-marker drift has already broken our classifier once (0/3 on live payload,
2026-08-31). A borrowed claim inherits its author's burden — `AGENTS.md`, "A BORROWED CLAIM INHERITS
ITS AUTHOR'S BURDEN".

```
ours (measured 2026-09-07):  omp/18.1.13
AGENTS.md was measured at:   omp/18.0.11          <- already stale by a patch line
loktar00:                    "current as of 2026-08-25"
awslabs CAO:                 OMP 17.2.10 fixtures
dsebban:                     OMP 17.3.5 or newer
wolfiesch/omp-best-of:       OMP 17 or newer
```

---

## The sources

| source | kind | install |
|---|---|---|
| [`loktar00/agent-skills` → `omp-orchestration/SKILL.md`](https://github.com/loktar00/agent-skills/blob/master/omp-orchestration/SKILL.md) | **skill** — distilled from ~90 real omp turns across 10 models, 5 providers | copy into `~/.claude/skills/` |
| [`dsebban/skills`](https://github.com/dsebban/skills) | **three skills** — `poteto-mode`, `pstack-omp`, `orchestrate-omp` | `npx skills add dsebban/skills --skill poteto-mode pstack-omp orchestrate-omp --global --yes` |
| [`awslabs/cli-agent-orchestrator` → `docs/omp-cli.md`](https://github.com/awslabs/cli-agent-orchestrator/blob/main/docs/omp-cli.md) | **reference doc** — CAO drives OMP through tmux terminal markers | `cao install <profile> --provider omp` |
| [`wolfiesch/omp-best-of`](https://github.com/wolfiesch/omp-best-of) | **omp plugin** — best-of-N with verifier-ranked selection | `omp plugin marketplace add wolfiesch/omp-best-of` |

`skills` CLI is **ABSENT** here; `npx` resolves at `/opt/homebrew/bin/npx`, so the `dsebban` install
line works as written. `omp plugin` exists (3 mentions in `omp --help`).

---

## VERIFIED HERE — measured 2026-09-07 against omp/18.1.13

| claim | source | verdict |
|---|---|---|
| `omp -p` non-interactive exists | loktar00 | **PRESENT** (21 `-p` mentions in `--help`) |
| `--approval-mode`, `--max-time`, `--thinking`, `--extension`, `--append-system-prompt` | loktar00 / CAO | **ALL PRESENT**, 1 mention each |
| `~/.omp/agent/sessions/` exists | loktar00 | **PRESENT**, 6 session dirs |
| `git add -A <dir>` then `git commit -m …` sweeps another lane's staged files | loktar00 §7 | **CONFIRMED TONIGHT** — `14b34f1` swept `%8`'s `mad1` hunks from a *correctly* path-scoped commit |
| "Built-but-never-connected" at every granularity | loktar00 §8.1 | **CONFIRMED** — our BUILT ≠ WIRED rule, hit by 2–7 independent models in their runs |
| a later ready status line makes an older marker stale | CAO | **CONFIRMED** — our "read the LAST status line, never the buffer" rule, independently derived |
| "it exited 0" and a self-report are not verification | loktar00 §6 | **CONFIRMED** — our grading gate |
| cheapest check fails first (ordered preflight) | omp-best-of | **CONFIRMED** — our gate ladder |

---

## UNAVAILABLE HERE — do not adopt these

### 1. ⛔ RETRACTED 2026-09-07 — THE LIVENESS ORACLE IS AVAILABLE. I MEASURED THE WRONG PATH WITH A BROKEN COMMAND.

**This section previously read "The session-jsonl liveness oracle DOES NOT TRACK OUR PANES" and
claimed the oracle would report the entire fleet dead. Both halves were wrong, and the error was
instrumental, not observational.** Caught by `%20`, which named the right path from its own
measurement.

**Error 1 — the command could not work.** I used GNU syntax on BSD `find`:

```
$ find /tmp -name freshness-control-probe -newermt '-10 minutes'
find: I cannot figure out how to interpret ‘-10 minutes’ as a date or time
```

I piped stderr away, so **a failing command reported `0` and I read it as "no fresh files."** A
positive control — `touch` a file, then look for it — returns `0` under that form and `1` under the
BSD idiom `-mmin -10`. Same family as every other instrument defect in `AGENTS.md`: the instrument
produced the reading, not the subject.

**Error 2 — the wrong path.** loktar00 documents `~/.omp/agent/sessions/`. Our sessions are
**profile-scoped**: `~/.omp/profiles/<profile>/agent/sessions/`, 11 profiles.

**Re-measured with `-mmin`:**

```
                                    <10min   <60min   total
~/.omp/profiles/codex/…/sessions      14      101     21829
~/.omp/profiles/claude/…/sessions      5       10      2162
~/.omp/profiles/glm/…/sessions         0        0       159
~/.omp/profiles/grok/…/sessions        0        0       778
~/.omp/agent/sessions                  0        0         9   <- the path I measured. genuinely dead.
```

**19 files written in 10 minutes across the two live profiles.** `%20` measured 26 in its own window
and reports it is **pane-attributable** — claude 3/3 and codex 2/2 live, grok 0 live against 1 dead
pane.

**So both facts are true and I published only half.** The documented path IS dead here (9 files,
nothing inside 24h). The profile-scoped path is alive. The oracle is usable **today**, without
adopting `omp -p`, which retracts this section's original conclusion.

**Why it matters:** `AGENTS.md`'s fifth rule complains that we classify pane state with a
braille-spinner regex over `capture-pane` — *"a spinner is a rendering, and we are parsing paint."*
A profile-scoped session-jsonl mtime is a **non-rendering** liveness signal available now. It does
not replace the footer parse (a jsonl says a process is writing, not what state it is in), but it is
an independent second channel — and independence is exactly what `riqd`'s leg 3 found missing, where
both existing channels derive from the same capture and therefore agree when wrong.

**NO-CLAIM.** Freshness is bursty: the same command returned 0 fresh for every profile minutes
earlier, when panes had just delivered reports and were idle. So an mtime gap is not evidence of a
dead pane on any single sample — it needs a window, exactly as loktar00's own table says (*"session
jsonl stale >20 min"* is their threshold, not >0). And the directory encoding still differs from the
documented form; measure it, never assume it.

### 2. CAO's marker table is 0-of-4 applicable

CAO's fixtures (OMP 17.2.10) versus our live panes:

| CAO state | CAO evidence | our 18.1.13 |
|---|---|---|
| `PROCESSING` | `Working… ⟨esc⟩` | **0 hits** — v18 never renders the word |
| `IDLE`/`COMPLETED` | ready frame `in: … out: … t: … tok/s: …` | **0 hits in all four panes** |
| `WAITING_USER_ANSWER` | `Allow tool: <name>` | 0 — no dialog was active, so **UNMEASURED**, not refuted |
| `ERROR` | runtime/provider error frame | **UNMEASURED** |

**Our actual footer, decoded from a live pane:**

```
⠹ 7m · ◕ Opus 5 · 📁 …ator · ⑂ main *90 ?28 ·
```

Braille spinner + elapsed timer + model + cwd + branch with dirty counts (`*90` modified,
`?28` untracked). CAO says it explicitly: *"recapture terminal fixtures before changing markers for
a new OMP release."* **They were right and their own table is the casualty.**

### 3. ⛔ RETRACTED — "LOCAL `rch exec` CPU TIME FROZEN" IS NOT A WEDGE SIGNATURE

**Retracting a claim I made in commit `b0ef77c`'s message and used to justify cancelling a build.**
I reported three local `rch exec` PIDs with *"elapsed climbing 10:28 → 10:36, CPU time FROZEN at
00:00 across three samples"* and called that the wedge signature, citing the `zeststream-rch` skill.

**`%20` sampled the same way and refuted it:**

```
sample 1  pid=37956  elapsed=01:50  cputime=00:00
sample 2  pid=37956  elapsed=02:02  cputime=00:00
sample 3  pid=8388   elapsed=00:07  cputime=00:00   <- brand new
```

`cputime=00:00` appeared on **a healthy `registry-check` build**, on the wedged franken-harvest
darwin build, and on PIDs that appeared and vanished between samples. **`rch exec` is a thin local
orchestrator — the CPU burns on the remote box, so `00:00` is its NORMAL state.** A wedge detector
built on local CPU time would report **every healthy build as wedged**.

**What survives:** *elapsed climbing while REMOTE progress stalls* — i.e. `progress_age_secs` from
`rch queue --json`. The CPU half is uninformative on this side of the SSH boundary. The skill's own
table gives the local-CPU form as the discriminator; on this measurement that half does not hold,
and the skill should be corrected upstream.

**The cancellation was still correct** — but on the `progress_age=233s` evidence, not on the CPU
reading I cited beside it. Two pieces of evidence in one paragraph, only one load-bearing, and I did
not separate them.

### 4. `rch queue` CANNOT TELL YOU WHETHER YOU ARE YOUR OWN BLOCKER

Measured by `%20`. At the moment `contabo-3` refused it with *"'contabo-3' already runs this
project"*, `rch queue --json` showed **two active builds and NO contabo-3 row at all**. The retry
succeeded on contabo-3 sixty seconds later, so the exclusion was almost certainly its own first
invocation — **registered by admission and invisible to the queue.**

**Why this is load-bearing:** `active_project_exclusion` reads identically to capacity loss, and the
skill's remedy table says *"pin a DIFFERENT worker"* for one and *"wait for your own job"* for the
other. **Requesting another worker is exactly how this repo's verdict builds scattered onto
`contabo-1` (zeststream-cast's) and `contabo-2` (control-plane's).** The queue view cannot
discriminate; a probe loop that waits is the correct response.

---

## WORTH ADOPTING — ranked, with what each buys us

1. **`omp -p` + `EXIT $?` sentinel + session-jsonl liveness.** This is the fifth rule's remedy as a
   package: a typed launch, a typed completion signal, and a liveness oracle that is not a
   rendering. Cost: our panes become headless lanes rather than interactive tmux sessions, which is
   a topology change, not a flag change.
2. **CAO's `Allow tool:` HOLD-DELIVERY design.** *"CAO holds orchestrated inbox delivery while that
   dialog is active."* We have this open as P1 bead `hwcv` — *"a pane blocked on an interactive
   dialog reads unproven"*. CAO's answer is to detect the dialog and **hold**, rather than to
   classify harder.
3. **CAO's dispatch-history discriminator.** *"CAO uses dispatch history to distinguish the first
   ready state from a finished turn."* Our receiver-receipt contract reconstructs this from a timer
   reset plus a spinner-stripped content hash ≥75 s apart. **Keying on dispatch history instead of
   timing removes the whole two-capture hack.**
4. **loktar00's failure-mode table**, which we have no equivalent of:
   - `EXIT 1` + `Deadline exceeded` → work usually MOSTLY landed → **salvage**, do not relaunch
   - `EXIT 1` + provider error → relaunch with a **CONTINUE preamble** naming current repo state
   - log stuck + session jsonl stale >20 min → **silent process death**
   - `EXIT 0` + zero work → empty completion → **relaunch as-is**
5. **`--append-system-prompt`, never `--system-prompt`**, and CAO's deliberate omission list:
   `--profile --config --tools --no-tools --no-skills --no-rules --no-extensions --auto-approve
   --approval-mode`. Preserves native project config.
6. **omp-best-of's dirty-tree refusal.** Its preflight *refuses to start* on a dirty tree; we ran
   all night at **124 dirty files** and `*90 ?28` shows in the footer of every pane.
7. **OMP-managed isolation workspaces.** *"OMP selects the host's best copy-on-write backend and
   falls back to a Git worktree when needed"*, plus *"baseline-aware delta capture"* for a
   binary-safe patch. We hand-manage worktrees.
8. **`omp` JSON event mode** (`omp-best-of` runs "in JSON event mode with extensions and sessions
   disabled") — a typed transcript instead of a parsed terminal.

---

## HONEST LIMITS

- **`--tools` cannot fully restrict discovered tools.** CAO states it: *"OMP's `--tools` only filters
  built-ins; it cannot fully restrict discovered custom, extension, or MCP tools. CAO restrictions
  are consequently **advisory** for `omp`, not hard enforcement."* Any sandboxing claim we build on
  `--tools` inherits that limit.
- **Nothing here is installed.** No skill from these repos is on this machine; the two install lines
  above are untested here.
- **Only the flag *presence* was verified**, by counting `omp --help` mentions. No flag was exercised.
  Presence in help text is not behaviour.
- **The `Allow tool:` and `ERROR` markers are UNMEASURED, not refuted** — no dialog or error frame
  was active during the capture. A future capture may find them intact.
- **`omp-best-of`'s verifier claims are the authors' own** and explicitly not benchmarked by them for
  the sampled backend: *"Sampled mode does not receive token logprobs and must not be presented as a
  reproduction of LLM-as-a-Verifier's continuous scoring."* Do not cite it as a verification method
  we have validated.
