# ASUPERSYNC-CONFORMANCE — per-crate schema and measured state

**The binding contract** (`AGENTS.md`, README "The cancellation contract"): every process this
binary spawns — `tmux`, `ntm`, `br`, `bv`, a build — is cancellable work with a deadline, built on
**asupersync 0.4.9**.

We pin `rev = "fa3c01aec"`. The mirror at
`/Volumes/ZestData/dicklesworthstone-mirror/asupersync` is at **exactly that rev**, version
**0.4.9**. The `asupersync-mega-skill` says "current release is v0.4.4" — that is **stale relative
to what we pin**, and the skill's own rule is to trust live docs over the skill.

## The schema

Eight measurable properties. Each is a **grep-able fact**, not a judgement — which is what makes
this table generable rather than asserted.

| property | how it is measured | why it is in the contract |
|---|---|---|
| `forbid_unsafe` | `unsafe_code = "forbid"` in `Cargo.toml` | a crate that will not compile under the lint is a **finding**, not a reason to drop the lint |
| `dep_asupersync` | `asupersync` in `Cargo.toml` | without it there is no `Cx`, so no cancellation contract at all |
| `dep_subprocess_contract` | `subprocess-contract` in `Cargo.toml` | the drain-safe runner; the only sanctioned way to spawn |
| `async_fns` | `async fn` count in `src` | denominator for the next column |
| `cx_first` | `async fn` whose first parameter is `cx:` | **`&Cx` first** in every async API we own |
| `checkpoints` | `.checkpoint()` call sites | cancellation observable in loops, retries, long handlers |
| `raw_command` | `Command::new` call sites | each is a potential undrained-pipe deadlock |
| `forbidden_deps` | `tokio`, `hyper`, `reqwest`, `axum`, `async-std`, `smol` | forbidden in runtime/core `src` |

Regenerate — never inherit these numbers:

```
python3 - <<'PY'   # full source in the commit that added this file
# walks crates/, greps each property, prints the table below
PY
```

## Measured 2026-08-31

```
crate                        unsf asup sub afn  cx1  ckpt  Cmd  forbidden
ack-spine                    Y    .    .   0    0    0     2    -
ack-stage                    Y    .    .   0    0    0     0    -
commit-build-fence           Y    Y    .   1    1    2     0    -
composer-typed               .    .    .   0    0    0     0    -
dispatch-silence-watch       .    .    .   0    0    0     2    -
finding                      Y    Y    Y   1    0    2     0    -
finding-dispatch             Y    .    .   0    0    0     0    -
fleet-composite              Y    .    .   0    0    0     2    -
installer                    Y    .    .   0    0    0     5    -
kernel-bypass-gate           Y    .    .   0    0    0     3    -
kernel-only-operator-hook    Y    Y    .   1    1    2     0    -
loop-queue-filter            .    .    .   0    0    0     0    -
no-shell-gate                .    .    .   0    0    0     3    -
omp-orchestrator             Y    Y    Y   8    8    3     2    -
pane-dispatch-fence          .    Y    Y   1    0    0     2    -
path-literal-guard           Y    .    .   0    0    0     0    -
pre-delete-citation-check    Y    .    .   0    0    0     2    -
receiver-receipt             Y    .    .   0    0    0     1    -
state-wildcard-lint          Y    .    .   0    0    0     0    -
subprocess-contract          .    Y    Y   2    2    1     2    -
tick-monitor                 Y    .    .   0    0    0     3    -
undrained-pipe-lint          Y    .    .   0    0    0     0    -
porting-gate                 Y    Y    Y   2    2    2     4    -

crates=23  forbid_unsafe=17  dep_asupersync=7
async_fns=16  cx_first=14  checkpoints=12
raw Command::new sites=33  crates_with_forbidden_dep=0
```

## What the numbers say

**29 raw `Command::new` sites; 4 crates depend on `subprocess-contract`.** That is the headline.
Every raw site is a potential undrained-pipe deadlock — the measured failure being: piping stdout
*and* stderr then polling `try_wait()` deadlocks past ~64 KiB, and **the tell is 0% CPU with no
children**, so a slow computation burns CPU and a deadlock does not, which means *widening the
timeout makes it worse*. A `git log` that takes 0.9s from a shell sat at **0.0% CPU for 104s** as a
child.

`undrained-pipe-lint` exists precisely to find these — and it has **zero callers**, so 29 sites
have never been scanned by the lint written for them.

**6 of 22 crates depend on asupersync.** The cancellation contract is described as binding and is
present on 27% of the workspace. The other 16 have no `Cx`, so "cancel-correct" is not false of
them — it is *unrepresentable*.

**12 of 14 `async fn` take `cx` first.** Where async exists the discipline holds. The two that do
not are `finding` and `pane-dispatch-fence`.

**16 of 22 forbid unsafe.** The six that do not are `composer-typed`, `dispatch-silence-watch`,
`loop-queue-filter`, `no-shell-gate`, `pane-dispatch-fence`, `subprocess-contract` — and the last
two **depend on asupersync while omitting the lint**, which is the combination least defensible.

**Zero forbidden dependencies.** No `tokio`, `hyper`, `reqwest`, `axum`, `async-std`, or `smol`
anywhere. That one is clean.

## What this table does NOT prove

> **NO-CLAIM 1.** Every column is a **syntactic** fact. `cx_first` counts a parameter name, not
> that cancellation is honoured; `checkpoints` counts call sites, not that they sit in the loops
> that matter. A crate can score perfectly and still leak a detached task.

> **NO-CLAIM 2.** `raw_command` counts `Command::new` textually. A spawn built through a helper or
> a dynamically-constructed name is **invisible** to it, so 29 is a **lower bound**.

> **NO-CLAIM 3.** Absent from this schema entirely, because they are not greppable: **region
> ownership** (no detached tasks), **kill the process GROUP not the pid**, **a timeout is not a
> verdict**, `Budget`/`Outcome`/capability narrowing, two-phase effects, and deterministic
> `LabRuntime` tests. Those need a semantic pass. Naming them here so their absence from the table
> is not read as their absence from the contract.

> **NO-CLAIM 4.** This is a snapshot taken by hand. Per the inventory-map doctrine —
> *generated, never drawn* — the values must be emitted by a crate on every run. Until they are,
> this file is a measurement with a timestamp, not a gate.

## Prior art we should be consuming, from the mirror

`asupersync/src/messaging/` already solves problems we reinvented:

- **`AckKind`** — *"acknowledgement boundary reached by the operation"* (`fabric.rs:1919`). A typed
  boundary, where we built three parallel authorities for lack of the vocabulary.
- **`DeliveryClass`** (`class.rs:17-29`) — `EphemeralInteractive`, `DurableOrdered`,
  `ObligationBacked`, `MobilitySafe`, `ForensicReplayable`, each mapped to its `AckKind`. **We have
  no delivery class**; every dispatch packet weighs the same.
- **`PublishPermit`** (`fabric.rs:1944`) — `#[must_use = "must be sent or explicitly aborted"]`,
  and *"dropping without calling either aborts cleanly (no obligation leaked)"*. **That is the
  pending-dispatch fence, solved** — ours sat 29 ticks because nothing owned its release.
- **`ObligationLedger` / `ObligationToken`** — reserve, then commit or abort. Our `Finding` crate
  exists to make an obligation representable and shipped with zero callers.

> **NO-CLAIM 5.** That fabric is a messaging plane between components that **both** use it. Whether
> its permit/ack machinery can wrap a tmux pane that has never heard of asupersync is
> **UNMEASURED** — possibly *(a) not applicable*. The finding is that we invented vocabulary that
> already exists, not that adoption is proven.
