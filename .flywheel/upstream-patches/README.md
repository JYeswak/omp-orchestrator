# Upstream patches — preserved, not published

Work committed against `Dicklesworthstone/remote_compilation_helper` (a **third-party**
repo) and never pushed. Preserved here because one source checkout is in `/tmp` and
therefore clearable. **Publishing is Josh's decision** via the jeff-issue-chain
process; nothing here has been pushed, verified below.

## TWO DIVERGENT UNPUSHED CHECKOUTS, and neither contains the other's work

Measured 2026-09-01:

| checkout | HEAD | unpushed | risk |
|---|---|---:|---|
| `/tmp/rch-fresh` | `e3ab4c8` | **3** | **`/tmp` is clearable** |
| `~/Developer/jeff-corpus/remote_compilation_helper` | `e9b9e7ea` | 1 | durable |

`e3ab4c8` is **absent** from the jeff-corpus tree; `e9b9e7ea` is absent from
`/tmp/rch-fresh`. Four commits of real work exist in two trees that have diverged, and
a `/tmp` clear would silently delete three of them. That is why all four are captured
here as patches rather than left in place.

## Which patch is which — the filenames actively mislead

`git format-patch` derives filenames from commit messages, and two of these commits
carry a **verbatim identical** message. Read this table, not the filenames.

| patch | commit | what it ACTUALLY changes | message accurate? | bead |
|---|---|---|:--:|---|
| `0001-test-disk_pressure-…` | `6a4c219` | `rchd/src/disk_pressure.rs` +60 — threshold/status tests | **yes** | `2z2.1` |
| `0002-test-disk_pressure-…` | `feb3107` | `rch/src/hook.rs` +24, `hook/tests.rs` +48 — **`is_regenerable_registry_root()`, the registry-mirroring fix** | **NO** | `2z2.2` |
| `0003-test-transfer-rsync-path-…` | `e3ab4c8` | `rch/src/transfer.rs` +41 — asserts the rsync-path preamble quotes the cache glob for zsh workers | yes | `2yf` |
| `0090-fix-rch-bound-large-source-…` | `e9b9e7ea` | Josh's own fix, from the **jeff-corpus** checkout | yes | — |

Numbered `0090` deliberately: it comes from a different tree than 0001–0003 and must
not be applied as if it were sequential with them.

## The lesson that cost a colleague a wrong answer

Two panes reported on `2z2.2` and **contradicted each other**: one said the fix was
committed as `feb3107`, the other closed the bead `MINED-AND-LOCATED / NOT committed`.
Measurement settles it — the fix **is** committed, with 12 occurrences of
`is_regenerable_registry_root` in the tree.

The pane that said otherwise was **not careless**. `git log` genuinely shows two
identical `disk_pressure` commits, so a reviewer checking whether the registry work
landed sees nothing about registries and reasonably concludes it did not.

**A commit message that misdescribes its diff does not merely lose information — it
produces a wrong answer in anyone who trusts it.** This repo's `commit-msg` hook
enforces a verification-level tag; nothing checks that the subject matches the diff,
and nothing does.

## Verified, so nobody re-derives it

- None of the four commits is an ancestor of `origin/main`. Checked per-commit with
  `git merge-base --is-ancestor`. **No third-party publish occurred.**
- Applying any of these needs a fresh clone of the upstream repo. The `/tmp` checkout
  they came from may already be gone by the time you read this.
- `2yf`'s underlying fix was already upstream (single quotes present since `54e6b95`,
  Aug 28) and is in the installed `rch 1.0.62`. `0003` adds the **test**, not the fix.
