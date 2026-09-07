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

### 1. The session-jsonl liveness oracle DOES NOT TRACK OUR PANES

loktar00's headline liveness recommendation:

> *"The log's `Working...` heartbeat is NOT a health signal; the session jsonl's growth is."*
> Path: `~/.omp/agent/sessions/--<cwd-with-dashes>--/`

**Measured: every one of the 6 session dirs is stale, and 0 files anywhere under `~/.omp` were
modified in the last 10 minutes — while five panes were actively working.**

```
-tmp                                   newest  10945 min ago
--private-tmp-claude-501--…-scratchpad--        21840 min ago
-Developer-control-plane                        13071 min ago
-                                       (0 files)
-Developer-omp-orchestrator                      8326 min ago
-Developer-zeststream-cast                      11631 min ago
```

**It would report the entire fleet DEAD.** Our panes run OMP *interactively under tmux*, not via
`omp -p` headless — and only the headless path appears to write session jsonl. Also note the
directory encoding differs from the documented form: ours is `-Developer-omp-orchestrator`, not
`--Users-josh-Developer-omp-orchestrator--`. **Measure the encoding; do not assume it.**

**Consequence:** the oracle is real and would be strictly better than parsing paint — but it is
available only if we adopt `omp -p`, which is the fifth rule's actual remedy.

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
