# Upstream patches — preserved, not published

Work committed against `Dicklesworthstone/remote_compilation_helper` (a **third-party**
repo) in a `/tmp` checkout. Preserved here because `/tmp` is clearable and the commits
are unpushed. **Publishing them is Josh's decision** via the jeff-issue-chain process;
nothing here has been pushed.

Source checkout at capture time: `/tmp/rch-fresh`, `origin/main..HEAD` = 2 commits.

## The patch filenames are misleading, and that is the point

Both commits carry a **verbatim identical** message:

```
test(disk_pressure): threshold/status agreement at real disk ratios [test]
```

`git format-patch` derives filenames from messages, so both patches are named almost
identically — and **only one of them is about disk pressure**.

| patch | commit | what it ACTUALLY changes | bead |
|---|---|---|---|
| `0001-test-disk_pressure-…` | `6a4c219` | `rchd/src/disk_pressure.rs` (+60) — threshold/status tests. **Message is accurate.** | `2z2.1` |
| `0002-test-disk_pressure-…` | `feb3107` | `rch/src/hook.rs` (+24), `rch/src/hook/tests.rs` (+48) — **`is_regenerable_registry_root()`, the registry-mirroring fix.** Message is WRONG. | `2z2.2` |

## Why this index exists

Two panes reported on `2z2.2` and **contradicted each other**: one said the fix was
committed as `feb3107`, the other closed the bead as `MINED-AND-LOCATED / NOT
committed`. Measurement settles it — the fix **is** committed, 12 occurrences of
`is_regenerable_registry_root` are in the tree, and `feb3107` carries them.

The second pane was not careless. **`git log` genuinely shows two identical
`disk_pressure` commits**, so a reader checking whether the registry work landed sees
nothing about registries and reasonably concludes it did not. The misleading message
manufactured a false negative in a colleague's review.

That is the durable lesson: a commit message that misdescribes its diff does not just
lose information, it actively produces wrong answers in anyone who trusts it. This
repo's own `commit-msg` hook enforces a verification-level tag; it does not check that
the subject matches the diff, and nothing does.

## Verified, so nobody re-derives it

- `feb3107` is **NOT** an ancestor of `origin/main` — nothing was pushed to a third
  party. Checked with `git merge-base --is-ancestor`.
- `6a4c219` is likewise unpushed.
- Applying either patch requires a fresh clone of the upstream repo; the `/tmp`
  checkout they came from may already be gone.
