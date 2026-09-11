# shared-checkout — survive the shared tree

Triggers: "commit", "stage", "shared checkout", "peer", "index", "swept",
"worktree dirty", "whose change", "hunk", "stale intent", "E0583".

## Pattern
The index is shared state; the worktree is five agents wide. Every commit:
1. `git diff --cached --name-only` FIRST — if it lists a file you do not own, STOP.
2. `git add -- <paths> && git commit -- <paths>` — BOTH pathspecs, never `-A`, never bare.
3. New files must be TRACKED to be named in a pathspec commit; `git add` the new file ALONE, verify the cached list shows ONLY it.
4. Read the sha back (`git show --stat HEAD`); for new files prove presence via `git ls-tree -r HEAD`.
5. Never `git checkout --` / `git restore` (blocked); revert by inverse edit. Never `rm -rf` / `find -delete` as direct calls.

## Anti-patterns

| Anti-pattern | Why it fails | Fix |
|---|---|---|
| A bare `git commit` after path-scoped `git add`. The index is shared: add-then-commit without a pathspec takes the WHOLE INDEX including whatever another agent staged — measured twice, sweeping a 220-line taxonomy and 236 bead lines under unrelated subjects. | Peer work lands under your name and your message asserts what it predicted, falsely. | `git commit -- <paths>` always; a commit message asserts what `git show --stat` proves. |
| Committing a shared-dirty file with both pathspecs. Path-scoping is not hunk-scoping: committing lib.rs took 225 peer lines plus the author's own, and raced out 4 of them — HEAD failed E0583 on `installer` and every dependent. | The commit is green locally (file on disk hides it); only `git ls-tree -r HEAD` sees the break. Fresh clones cannot build. | Hand-built blob plus bare commit when a peer hunk is present; BOTH pathspecs ONLY when the worktree content is exactly yours. Verify new files via `ls-tree`. |
| `git diff --numstat` as a collision signal. ` M` with zero insertions means no peer is editing; mode changes leave numstat identical both ways. | Content-dirt and mode-dirt need different instruments; one answers the wrong question. | Discriminate: `git diff --numstat` for content, `git diff --summary` for mode. |
| `git add -- <path>` as isolation from a peer editing the same file. Path-scoped add is still whole-file; a peer's hunks ride along. | Isolation theater ends in the same sweep. | Stage hunks with `git apply --cached` against HEAD content, then commit FROM THE INDEX WITH NO PATHSPEC (a pathspec re-reads the worktree and undoes the isolation). |
| Citing `git show HEAD:<path>` content without checking whose bytes they are. A hand-built blob encoded a stale judgement and deleted HEAD content committed between test run and commit. | Index checks cannot catch stale intent. | Re-read shared files immediately before commit; verify as content from the commit blob. |
| `grep -c` symbol counts as dead-code proof. Occurrence counts include the uses inside the thing being deleted — both call sites of a deleted function matched inside the function itself. | Deletion looks unused right up to the breakage. | The compiler's dead-code warning, or a structural caller census that attributes to the enclosing symbol. |

## Negative evidence

| Row | Provenance |
|---|---|
| Bare-commit sweeps (×2), add-then-bare-commit taxonomy sweep | AGENTS.md commit rules, 2026-09-02 |
| f5eedd8 swept 225 peer lines + raced out 4 own (E0583) | commit f5eedd8; restored c8ef85c |
| numstat-vs-summary mode blindness (8h) | AGENTS.md; proven on content-dirty file 2026-09-07 |
| Path-scoped add is whole-file (8n) | AGENTS.md |
| grep -c occurrences incl. self-uses (8o) | AGENTS.md; `workflow_invokes_lint` ruling |
| Stale intent beats index checks | bead-mining: c8ef85c cluster |

## Check (before every commit)
```
git diff --cached --name-only
git status --porcelain -- <your paths>
# after: git show --stat HEAD   (must list ONLY yours)
```
