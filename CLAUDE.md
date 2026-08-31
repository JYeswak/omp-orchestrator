# CLAUDE.md — omp-orchestrator

**Read and obey `AGENTS.md` before acting.** It is the canonical, platform-neutral contract: the
one rule (no `.sh`, no `.py`), BUILT ≠ WIRED, the OMP lifecycles, the four-skill philosophy, the
crate map, the fh rows, and the asupersync contract.

This file adds only what is Claude-specific. Where the two disagree, **AGENTS.md wins**.

---

## First turn

1. Read `AGENTS.md` completely. It is the contract, not background.
2. `cd /Users/josh/Developer/omp-orchestrator` — **`br` reads the cwd.** A `br` command run from
   control-plane files into control-plane's tracker. This has already happened once.
3. `br ready --json` — this repo's beads carry the prefix `omp-orchestrator`.
4. Before building anything: `mcp__socraticode__codebase_search` by meaning, then
   `fh suggest "<the thing>"`. We own 56 crates and a 180-repo mirror.

---

## Skills to load, and when

| Situation | Load |
|---|---|
| Touching spawn, cancellation, deadlines, scheduling | `/asupersync-mega-skill` — **before** writing, not after |
| A crate will not compile under `forbid(unsafe_code)` | `/rust-unsafe-code-exorcist` |
| Adding or changing a binary's CLI surface | `/canonical-cli-scoping` + `/agent-ergonomics-and-intuitiveness-maximization-for-cli-tools` |
| Converting a plan into beads | `/beads-workflow` (frozen prompts — copy verbatim, do not paraphrase) |
| Deciding what to work next | `/beads-bv` — never bare `bv`, it launches a TUI |
| Dispatching to panes, or a pane looks stuck | `/vibing-with-ntm` |
| Grading a close, yours or anyone's | `/beads-compliance-and-completion-verification` |
| Something is wrong and you have a theory | `/brennerbot-with-ntm` — write the falsifier first |
| A session feels busy but ships nothing | `/just-say-no-to-process-porn-and-ceremony` |

---

## What Claude specifically gets wrong here

Each of these was measured on this stack. They are not hypotheticals.

**Reading a `| tail` exit status as the command's own.** Misreported results seven times in one
session. `x=$(cmd) || rc=$?` is the only shape that survives `set -e` and preserves the code. A
Python traceback piped to `tail` reports `EXIT=0`.

**Trusting a fixture instead of running it.** I retracted a *true* statement on the authority of a
**failing** upstream test. Reading a fixture is not verifying it — run it before believing its
constants (`fh C38`).

**Grepping the whole buffer for pane state.** Matches your own scrollback. Read the **last** status
line. A guard that greps its own pane pollutes itself.

**Counting a comment as code.** Three legs of mine passed tonight because a grep matched the comment
explaining the rule rather than the code implementing it. Anchor structural assertions on the
executable line — a function definition, an assignment — never a mention count.

**Announcing a fix without re-running under the real environment.** `cron` resolves `env bash` to
bash 3.2 and gives no locale. `env -i` is the only honest test of a scheduled path.

**Closing my own work.** Only an independent verifier closes with cited evidence. Solo, separate the
hats: re-verify by **re-execution** against the original acceptance criteria, and state plainly what
was *not* independently verified.

---

## Reporting

Lead with the operational result. Separate **observation** from **inference**.

```
Result:   <what changed, with the sha or the artifact>
Evidence: <the command and its actual output>
Blocked:  <typed blocker, or a named decision>
Next:     <one action>
NO-CLAIM: <what this does not prove>
```

**The NO-CLAIM line is not optional** on any "done" report. A claim without its boundary is the
overclaim that makes a reader stop looking.

---

## Autonomy

Reversible local work — code, tests, builds, beads, commits, backups to our own remotes — proceeds
without asking. **Escalate only three things:** spending money, obtaining credentials, deployment
decisions.

A blocker **halts and names one decision**. It does not get routed around with adjacent machinery —
filling the time is a defect, not diligence.
