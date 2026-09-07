# Census Archive — Crate Inventory

> **ARCHIVED FIGURES ARE NOT CITABLE.** This file is a historical receipts archive, not current ground truth. Re-run the producing command from the source section before use.
>
> Archive snapshot: **2026-09-07**. The source excerpt below is verbatim; its figures are not remeasured or corrected here.
>
> Original measurement dates present in this excerpt: not stated in the excerpt.
>
## The crate extraction target list — what each one is, and **which repository it is actually in**

**Read the STATUS column before you reason about any row.** The table below has 24 rows and is a
**historical extraction-target list, not the package inventory.** The workspace is now far larger
than the table; a row marked CONTROL-PLANE is not an available local dependency and must not be
cited as present.

### DO NOT CITE A PACKAGE COUNT FROM THIS FILE. RUN THE COMMAND.

**This figure has moved `27 → 50 → 51 → 65` inside the lifetime of this one document**, and each
stale value was corrected by a later agent who then wrote a fresh integer that went stale in turn.
A count in prose is wrong the moment anyone lands a crate, and this section has now proven that
four times. The correction is not a better number — it is **no number**:

```bash
# in either repo; these two must agree, or you have a manifest-vs-directory discrepancy
cargo metadata --no-deps --format-version 1 --offline \
  | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["packages"]))'
find crates -mindepth 1 -maxdepth 1 -type d | wc -l

# names present in BOTH repos
comm -12 <(find crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort) \
         <(find /Users/josh/Developer/control-plane/crates -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)
```

**One dated measurement, as evidence that the commands run — never as a figure to cite.** Measured
2026-09-02 by the orchestrator: **65 here** (`cargo metadata` and the directory count agree),
**62 in control-plane**, **28 names in both**. Compare against the previously-published `27 / 59 / 4`:
every one of the three moved, and the intersection grew **7×**. The extraction wave is landing
crates faster than any prose table can track it.

**There is no percent-ported figure, and there cannot be one from this file.** The numerator moves
hourly and the denominator was never established — the extraction scope has been asserted as 20 and
as 23 crates and **neither figure ever shipped a producing command.** Treat both the way the retired
"81 JSON-RPC methods, 17 used" pair is treated: cite neither. `NUMBERS.toml` exists precisely so
plan sections resolve figures to a runner instead of typing integers; a section that types an
integer here is a defect, not a shortcut.

### Current workspace packages outside the legacy extraction table

These are HERE, but were not part of the 24-row extraction target list:

| Crate | STATUS |
|---|---|
| ack-spine | **HERE** |
| ack-stage | **HERE** |
| commit-build-fence | **HERE** |
| dispatch-claim-fence | **HERE** |
| dispatch-silence-watch | **HERE** |
| finding | **HERE** |
| finding-dispatch | **HERE** |
| installer | **HERE** |
| kernel-bypass-gate | **HERE** |
| kernel-only-operator-hook | **HERE** |
| no-shell-gate | **HERE** |
| omp-inventory-map | **HERE** |
| omp-orchestrator | **HERE** |
| omp-rpc-session | **HERE** |
| omp-types | **HERE** |
| path-literal-guard | **HERE** |
| porting-gate | **HERE** |
| pre-delete-citation-check | **HERE** |
| receiver-receipt | **HERE** |
| state-wildcard-lint | **HERE** |
| subprocess-contract | **HERE** |
| tick-monitor | **HERE** |
| undrained-pipe-lint | **HERE** |
LOC and `tests/` counts on every `CONTROL-PLANE` row are read from the control-plane working tree.
They describe source you do not have here. Grouped by the lifecycle stage they serve.

### Ground truth — "what is actually true right now"

These exist because **every classifier we trusted has been wrong at least once**, and a wrong
liveness read either interrupts real work or leaves a worker idle beside a full queue.

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `pane-truth` | **CONTROL-PLANE** | 1247 | Ground-truth tmux pane state | The shell version remains the differential oracle; this is the typed reading |
| `fleet-truth` | **CONTROL-PLANE** | 1621 | Fleet-wide inspection register | One place answers "what is the fleet doing" so callers stop re-deriving it |
| `fleet-reconcile` | **CONTROL-PLANE** | 1424 | NTM projection vs tmux reality | NTM's snapshot returns `total_sessions: 0` with `success: true` when stale; tmux does not lie |
| `oracle-compare` | **CONTROL-PLANE** | 449 | Shared comparator: claim vs independent oracle | An empty or unreadable oracle must be an ERROR, never a silent agreement |
| `pane-oracle-diff` | **CONTROL-PLANE** | 741 | tmux pane census vs ntm projection | Catches projection drift before a dispatch rides it |
| `oracle-pane-state-differential` | **CONTROL-PLANE** | 613 | session:index pane-set differential (tmux vs ntm) | Uses the shared set comparator; this source has no Z3 implementation |
| `fleet-composite` | `HERE` | 1372 | Geometric fleet-health composite and diagnostic CLI | Refuses malformed, empty, and non-finite inputs instead of inventing a score |

### Readiness and admission — "may this pane receive work"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `pane-dispatch-ready` | **CONTROL-PLANE** | 1555 | Can this pane SAFELY receive a dispatch | `safe_to_dispatch` is not liveness |
| `pane-dispatch-fence` | `HERE` | 468 | Cross-process per-pane admission fence | Two dispatchers landing during a `/clear` vaporise the packet |
| `composer-typed` | `HERE` | 556 | Does the composer hold real TYPED text | Sender success is not receiver receipt |
| `ntm-fleet-monitor` | **CONTROL-PLANE** | 3122 | Typed fleet actions + approval waves. **Classifies; does not send** | Separating classification from actuation makes the verdict auditable |

### Selection — "what should be worked next"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `loop-queue-filter` | `HERE` | 912 | Fail-closed queue selector | Epics invite unbounded scope; in-flight work must not be re-offered |
| `loop-coverage` | **CONTROL-PLANE** | 926 | Typed coverage matrix. **A map, not a gate** | Says honestly what is *not* covered rather than implying completeness |
| `refill-idle-panes` | **CONTROL-PLANE** | 842 | Refill every idle pane from the bv DAG | An idle worker beside a ready queue is the conductor's failure |
| `omp-idle-dispatch` | **CONTROL-PLANE** | 1667 | Fail-closed idle OMP pane dispatch lane | Makes repository, session, ledger, and admission inputs explicit before dispatch |

### Dispatch — "send the work"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `fast-dispatch` | **CONTROL-PLANE** | 2292 | Admit on a fresh standing verdict, select free panes | Must fail closed on a stale verdict |
| `tick-dispatch` | **CONTROL-PLANE** | 990 | Ground-truth pane dispatch fence | Decided by tmux/ntm truth, not a cached label |
| `loop-driver` | **CONTROL-PLANE** | 2484 | Single-instance, deadline-bounded driver | Two ticks fighting over one pane is corruption |
| `loop-tick` | **CONTROL-PLANE** | 1480 | Single-pane dispatch tick | The unit the driver repeats |
| `fleet-monitor` | **CONTROL-PLANE** | 2569 | OBSERVE lane: attention wait + idle/ready scan | Block on a state transition; polling is the anti-pattern |

### Verification and reaping — "did it actually happen"

| Crate | STATUS | LOC | What it does | Why it exists |
|---|---|---:|---|---|
| `verify-dispatch` | **CONTROL-PLANE** | 1291 | Verification from **bead status only** | Ground truth, never a pane's self-report |
| `dispatcher-deadman` | **CONTROL-PLANE** | 883 | Watchdog: eligible work that received no packet | The failure that is invisible because everything looks healthy |
| `reap-finished-panes` | **CONTROL-PLANE** | 1189 | Sweep finished panes before the next dispatch | An unreaped pane is capacity that silently disappears |
| `wired-but-inert-guard` | **CONTROL-PLANE** | 1394 | Fail-closed proof that declared dispatch gates are actually invoked | Prevents a green unused gate from counting as coverage |

**Dependency shape** (from each `Cargo.toml`, current 24-row table): 17 leaves with zero path deps;
7 with exactly one — `ntm-fleet-monitor` → `loop-coverage`, `fleet-monitor` →
`ntm-fleet-monitor`, `pane-oracle-diff` → `oracle-compare`,
`oracle-pane-state-differential` → `oracle-compare`, `tick-dispatch` → `oracle-compare`,
`fast-dispatch` → `loop-switch`, and `loop-driver` → `loop-switch`. **Extract leaves first.**

### Porting order over the whole source workspace (measured 2026-08-31)

The dependency shape above is scoped to **the 24 rows of this table only**. The extraction frontier
is the whole source workspace, and it is larger. Derived from the resolver, not from text:

```bash
# Run in /Users/josh/Developer/control-plane. Topology comes from cargo, never from grep.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq -r '[.packages[] | {n: .name,
                           d: ([.dependencies[] | select(.path != null) | .name] | unique | length)}] as $p
           | "members=\($p | length)",
             "leaves=\([$p[] | select(.d == 0)] | length)",
             "one-dep=\([$p[] | select(.d == 1)] | length)",
             "two-plus=\([$p[] | select(.d >= 2)] | length)"'
```

Result: **57 members — 33 true leaves (zero intra-workspace path deps), 23 with exactly one, and 1
with two** (`controller-tick` → `loop-switch`, `admission-reason`). `crates/loop-tick/Cargo.toml`
declares its own `[workspace]` and is therefore **not** one of the 57; measured standalone it is
also a zero-path-dep leaf, so the leaf count is **33 of 57 loaded, or 34 counting the excluded
manifest**. Cite which denominator you mean. **Extract leaves first**: a leaf ports without
dragging a second crate across the repo boundary.

**The topology must not come from grep, and here is the actual reproduction** — corrected, because
the first diagnosis published for this was also wrong, which is the more useful lesson. The
conductor's original loop reported **1 leaf out of 59** where `cargo metadata` reports 33. The
published explanation was "the pattern missed Cargo's inline-table syntax." **That explanation is
false.** The pattern matched fine; only **22 path lines exist across all 57 manifests**, so most
crates genuinely have no match. The real cause is one shell idiom:

```bash
d=$(grep -c 'path = "\.\./' "crates/$c/Cargo.toml" 2>/dev/null || echo 0)
[ "$d" = "0" ] && n=$((n+1))     # never fires
```

`grep -c` **already prints `0`** and *then* exits 1 when nothing matches, so `|| echo 0` appends a
**second** zero. `d` becomes `$'0\n0'`, the equality test fails, and every zero-dependency crate is
scored as *having* dependencies. Measured directly: `d='0'$'\n''0'` → FALSE; dropping the `|| echo 0`
→ `d2=0` → TRUE.

That is the **same family as `[RCH] remote required` exiting 103 with `0 passed 0 failed`**, which I
also briefly read as a test result: *a command's failure path emitting something shaped like data*.
A Rust `count()` returns a `usize` and cannot produce `"0\n0"` — which is the concrete reason this
repo forbids shell rather than merely discouraging it.

A subagent independently **could not reproduce** the claim, because it ran a differently-shaped
command (`grep -rlE`, anchored → 0 files; unanchored → 24). Both of us were measuring real things
and neither was measuring the other's. **A defect report must carry the exact command**, or the
next person disproves a claim you never made. This is the same confident-zero class as the retired
"81 JSON-RPC methods, 17 used" figure above. **Derive topology from `cargo metadata`.**

### Specimen: `pane-truth`, installed here and un-portable to this repo

One row, made concrete, because it is the shape of the whole defect:

- `/Users/josh/.local/bin/pane-truth` — **installed**, 2,489,600 bytes, Aug 31 02:22.
- `/Users/josh/Developer/omp-orchestrator/crates/pane-truth` — **does not exist**.
- Its only source is `/Users/josh/Developer/control-plane/crates/pane-truth`, whose HEAD is
  `407ecb5` — an **unrelated history** to ours, sharing no commit with this repo.

So a binary built from another repository's tree sits on `PATH` under a name this workspace
documents and does not contain. The installer's identity check compares the installed artifact
against **this** repo's HEAD, which it can never equal, and therefore reports **MISMATCH
permanently** — not as a transient staleness signal but as a fixed point. A MISMATCH that can never
clear is not a gate; it is noise that trains operators to ignore the gate. `pane-truth` is not
installed-and-drifted. It is **installed-from-elsewhere**, and no rebuild here changes that until
the crate is actually extracted.

### NO-CLAIM: there is no denominator, so there is no "percent ported"

The 57 control-plane members are **candidates, not a work queue.** Some are cron-lane scaffolding
that should be **deleted rather than moved** — porting them would import a lane we already retired.
Nothing in this file establishes which of the 57 are targets and which are terminal.

The extraction scope has been stated in this repository as **20 crates** and as **23 crates**
(bead `omp-orchestrator-815`), and **neither figure was ever derived from a command.** They were
asserted. With the numerator moving and the denominator never established, **"how much extraction
is left" is undefined**, and any percentage, burndown, or "N of M ported" claim built on these
numbers is unfounded — including one built on the 4-of-24 split above, which measures **this
table**, not the extraction set.

This is the **unstated-denominator defect**, the same failure as the retired
"81 JSON-RPC methods, and we currently use 17 of the 81" pair earlier in this file: an inherited
ratio, no producing command, not re-derivable. That pair is retired and cited by nobody. **Treat
20 and 23 the same way.** The denominator is established by a command that enumerates targets and
names the terminal crates, or it is not established at all.

**Unsafe posture in the current 24-row table: 5 of 24.** `ntm-fleet-monitor`,
`refill-idle-panes`, `omp-idle-dispatch`, `wired-but-inert-guard`, and `fleet-composite`
declare `unsafe_code = "forbid"`. The 815 extraction scope is 23 crates and is also 5-for-23;
the historical 815 comment claiming 3-for-23 is stale after control-plane commit `8fc3e4b`, which
added the lint to the other two ported crates. A crate that will not compile under the lint is a
**finding**, not a reason to drop the lint.

**Measured set reconciliation (2026-08-31).** The pre-audit table had 21 rows, not 20. It included
the real `oracle-pane-state-differential` crate. The three ported crates named by bead
`omp-orchestrator-815` bring the documented table to 24 rows, while 815's stated 23-crate
extraction scope is its original 20 rows plus those three and therefore excludes
`oracle-pane-state-differential`. That is a real scope mismatch, not a rounding issue.

- Target workspace `/Users/josh/Developer/omp-orchestrator`: 8 loaded Cargo packages.
- Source workspace `/Users/josh/Developer/control-plane`: 58 tracked top-level crate manifests;
  Cargo loads 57 packages. The excluded top-level manifest is `crates/loop-tick/Cargo.toml`,
  which declares its own `[workspace]`; the two other tracked manifests are fixture manifests.
- Working-tree source totals for the current 24-row table: 32,087 Rust LOC and 22 crate-level
  `tests/` directories. The 815 23-crate scope totals 31,474 Rust LOC and 21 `tests/`
  directories under the same counting rule.

The audit is re-runnable from the target repo with the source root explicit:

```bash
# Target package count; run in /Users/josh/Developer/omp-orchestrator.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Source package count; run in /Users/josh/Developer/control-plane. The warnings are meaningful.
/Users/josh/.cargo/bin/cargo metadata --no-deps --format-version 1 \
  | jq '[.packages[].manifest_path | select(test("/crates/[^/]+/Cargo.toml$"))] | length'

# Every documented row -> source files and working-tree Rust LOC.
bun -e 'const s=await Bun.file("AGENTS.md").text(); const start=s.indexOf("## The crates:"); const a=s.slice(start,s.indexOf(String.fromCharCode(10)+"## Use fh",start)); const ns=a.split(String.fromCharCode(10)).filter(x=>x.startsWith("| "+String.fromCharCode(96))).map(x=>x.split("|")[1].trim().slice(1,-1)); for(const n of ns){const d="/Users/josh/Developer/control-plane/crates/"+n; const p=Bun.spawnSync(["find",d,"-type","f","-name","*.rs","-print"]); const fs=new TextDecoder().decode(p.stdout).trim().split(String.fromCharCode(10)).filter(Boolean); let loc=0; for(const f of fs){const t=await Bun.file(f).text(); loc+=t.split(String.fromCharCode(10)).length-(t.endsWith(String.fromCharCode(10))?1:0)} console.log(n+String.fromCharCode(9)+loc+String.fromCharCode(9)+fs.join(","))}'
```

**Source audit result (control-plane `src/lib.rs`/`src/main.rs`, unless noted):**

- **CONFIRMED** — `pane-truth`: pane rules, external command output, and two-capture timing are present.
- **CONFIRMED** — `fleet-truth`: fleet sensors and truth-row rendering are present.
- **CONFIRMED** — `fleet-reconcile`: tmux/NTM reconciliation, typed verdicts, and self-test are present.
- **CONFIRMED** — `oracle-compare`: count/set verdicts and unreadable/empty-arm handling are present.
- **CONFIRMED** — `pane-oracle-diff`: agent-pane census and NTM projection comparison are present.
- **DIVERGENT** — `oracle-pane-state-differential`: it compares session:index `BTreeSet` values through `oracle-compare`; no Z3 dependency or Z3 implementation is present. The table row now states the implementation rather than the stale label.
- **CONFIRMED** — `pane-dispatch-ready`: busy, agent, quota, composer, and motion checks feed admission classification.
- **CONFIRMED** — `pane-dispatch-fence`: per-session/per-pane lock acquisition and release are implemented.
- **CONFIRMED** — `composer-typed`: marker/ANSI-aware typed-composer parsing and self-test are implemented.
- **CONFIRMED** — `ntm-fleet-monitor`: typed actions and approval/refusal wave rendering are implemented; the binary does not send.
- **CONFIRMED** — `loop-queue-filter`: runtime-configured, fail-closed queue filtering is implemented.
- **CONFIRMED** — `loop-coverage`: proof levels, loop layers, edge cases, and reuse authorities form a coverage map, not a gate.
- **CONFIRMED** — `refill-idle-panes`: pane survey, refusal classification, recommendation parsing, and bounded assignment planning are implemented.
- **CONFIRMED** — `fast-dispatch`: fresh-verdict admission, free-pane selection, bounded children, and lock/ledger handling are implemented.
- **CONFIRMED** — `tick-dispatch`: ground-truth pane, discovery, readiness, and send decisions are implemented.
- **CONFIRMED** — `loop-driver`: single-instance locking and deadline-bounded driver output are implemented.
- **CONFIRMED** — `loop-tick`: single-pane dispatch decisions, bounded child execution, and lock acquisition are implemented; its standalone `[workspace]` manifest is the inventory caveat above.
- **CONFIRMED** — `fleet-monitor`: observe wait, idle/ready scan, and standing-verdict writing are implemented.
- **CONFIRMED** — `verify-dispatch`: bead-status-only verification and differential CLI behavior are implemented.
- **CONFIRMED** — `dispatcher-deadman`: eligible-work/no-packet watchdog behavior is implemented.
- **CONFIRMED** — `reap-finished-panes`: finished-pane sweep and bounded external probes are implemented.
- **CONFIRMED** — `omp-idle-dispatch`: fail-closed idle-pane dispatch with typed repository/config inputs is implemented.
- **CONFIRMED** — `wired-but-inert-guard`: tracked caller discovery, gate scans, fail-closed empty-scan handling, and diagnostic commands are implemented.
- **CONFIRMED** — `fleet-composite`: four-factor geometric scoring, malformed-input refusal, and diagnostic CLI behavior are implemented.

The four names shared by both repositories were checked explicitly: `composer-typed`,
`fleet-composite`, and `loop-queue-filter` are byte-identical between
`/Users/josh/Developer/control-plane/crates/<name>` and
`/Users/josh/Developer/omp-orchestrator/crates/<name>`; `pane-dispatch-fence` has the same
purpose but differs in both `Cargo.toml` and `src/main.rs` (the target adds
`subprocess-contract`).

This source audit finds one description divergence and the explicit set/inventory mismatches above;
it does not establish runtime correctness, wiring, or future drift.

---

