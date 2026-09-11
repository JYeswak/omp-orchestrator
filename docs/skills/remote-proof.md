# remote-proof — prove a remote run actually ran

Triggers: "remote test", "rch exec", "prove it ran", "exit code", "did the build run",
"test result", "offloaded run", "refused build", "CI says", "lane says".

## Pattern
Every remote verdict cites BOTH lines, pasted, plus the denominator:
`Remote command finished: exit=<N>` AND `test result: ... N passed`.
A refused build exits 0 and prints neither; both absent also means still running.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A bare `cargo` exit of 101 is read as a compile error. Three consecutive 101s on three workers were SIGKILL, SIGKILL, and an untracked file the worker never received — zero compile errors between them. The exit code is cargo's generic failure, not a diagnosis. | Misdiagnosis routes the fix at code that is fine while the lane stays broken. | Read `signal:` before `E`. Quote the refusal line and name the refusal class. |
| `$?` after a pipe is cited as the subject's verdict. A pipeline's status is its last command; three false findings in audited surfaces came from reading the pipeline's `$?` as the tool's. | The instrument produced the reading, not the subject. | Capture the tool's own status line (`Remote command finished`, `test result:`), never the shell's. |
| `test result: ok. 0 passed` is cited as a pass. A green zero-pass run satisfies the two-line bar while running nothing — an empty selector, a filtered-out suite, a missing target. | Vacuous green is indistinguishable from verified green at the only point that matters. | State the denominator every time. `0 passed` means the selector matched nothing until proven otherwise. |
| A `cargo` figure is cited as evidence about a commit. `cargo test` reads the WORKTREE; a sha names a TREE. In a shared checkout those diverge constantly. | A grade mixes two tree states and silently attributes one to the other. | Pin the tree: read with `git show <sha>:<path>`, diff with `git log <sha>..HEAD -- <path>`, state which tree every number came from. |
| A figure from the installed binary is cited as source truth. A compile-time-generated roster reports the workspace as it was when the binary was built; every crate added after an install is invisible until reinstall. | Stale doctrine licenses routing around a thing that exists. | A parity leg must read a fresh build; a figure from the installed binary is labelled INSTALLED or it is wrong. |

## Negative evidence

| Row | Provenance |
|---|---|
| NE-001: bare-101 uninformative (SIGKILL ×2, untracked path) | NEGATIVE_EVIDENCE.md NE-001, measured 2026-09-03 |
| Installed-binary roster drifted both ways; generator never wrong | AGENTS.md honest-limits, `%7` 2026-09-07 (`ompo` 86 vs 88 bins) |
| Cargo-reads-worktree grade mixed trees (22 passed/3 failed belonged to uncommitted work) | AGENTS.md gate rule 8 |
| Six red CI runs unread in one evening while local receipts stayed green | AGENTS.md third rule, 2026-09-02 |

## Check (paste, do not paraphrase)
```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test -j 2 -p <crate>
# require BOTH lines in output before any verdict:
#   Remote command finished: exit=<N>
#   test result: ok. <N> passed
```
