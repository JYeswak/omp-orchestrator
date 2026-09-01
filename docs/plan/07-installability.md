# 07 — Installability: distribution, identity, and the canonical CLI contract

This section answers one question an investor will ask before any other: *if this works on Josh's
Mac Studio, what makes it work on a second machine, in a second repo, for a person who is not
Josh?* Today the honest answer is "nothing yet, and we can prove it." What follows is the measured
starting point, then the contract we will ship against.

**Provenance boundary.** Historical measurements in this section were taken at repo HEAD fb89714 and are retained as audit context only. The current worktree authority is the command-backed boundary in §1 below; any later historical value is explicitly labelled and cannot serve as current acceptance evidence. PROJECTED means we intend to build it and it does not exist.
**Scope note (important):** this is an investor-facing installability plan and contract, not a
dispatchable operator runbook. It therefore does not pretend to carry runbook-only fields such as
Trigger, Dispatch packet, Amazing/Adequate bars, Skills, or Done signals. Those fields belong in
the operational runbook that will execute this contract; their absence here is intentional scope,
not a missing installability requirement.
Where `$M` appears in a measurement command, it is explicitly
`/Volumes/ZestData/dicklesworthstone-mirror`; the reproducible shell prefix is
`M=/Volumes/ZestData/dicklesworthstone-mirror`. No command in this section relies on an ambient,
undefined `$M`.

### 1. The measured starting point
**CURRENT WORKTREE AUTHORITY (re-derived during this integration).** The exact cargo metadata target filter returns **48** binary targets in the shared worktree. Revision, command output, and install receipt must be captured together for a release claim; the historical hashes below do not serve as current acceptance evidence.

**HISTORICAL MEASURED BASELINE — 21 binary targets at `fb89714`; superseded plan snapshot = 23.** The installer knows about 3 of them. The target declaration count is not itself a successful artifact count; the artifact proof is the command and receipt described below.

The number was `18` until round 10, taken from `grep -rl 'fn main' crates --include='main.rs' | wc -l`.
The investor lens filed it as *"fn main count is not binary/build evidence"* and it was right twice
over — the proxy was wrong, and by the time it was challenged the count had drifted too. Three
instruments, three answers, re-measured at commit time:

| instrument | answer | what it actually counts |
|---|---:|---|
| `grep -rl 'fn main' --include=main.rs` | 18 | crates having a `main.rs` — a **source shape**, not an artifact (re-measured 2026-09-01 at both fb89714 and be012d9; the 17 figure and the NUMBERS.toml note were one behind) |
| `grep -c '^\[\[bin\]\]' crates/*/Cargo.toml` | 16 | **explicitly declared** targets; misses every implicit `src/main.rs` |
| `cargo metadata --format-version 1 --no-deps` (historical snapshot) | **23** | the workspace's declared binary targets in the superseded plan snapshot; current target authority is 48 above |
The historical metadata measurement was the only canonical target denominator in that snapshot; current target authority is the 48-row metadata result above. Both counts include implicit and explicit Cargo binary targets and neither is an artifact-success proof.
Successful output is a separate proof: `cargo build --workspace --message-format=json` is filtered to
`compiler-artifact` messages whose target kind is `bin`, and the receipt records target names,
artifact paths, and the command exit code. A target in metadata without a successful artifact is
reported missing and cannot satisfy install acceptance.

`crates/installer/src/main.rs:12` declares
`const BINARIES: &[&str] = &["omp-orchestrator", "tick-monitor", "pane-truth"];`.


**HISTORICAL MEASURED PLAN SNAPSHOT.** The installer list covered 3/23 target rows (13%), with 2/23 owned install entries (8.7%) and one foreign; 20 targets had no owned path in that snapshot. Current acceptance uses the 48-target metadata denominator above.
**Canonical manifest (PROJECTED; the one denominator for this section).** The release manifest is
generated from the Cargo target graph with the exact command in `NUMBERS.toml`:
`cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c "import sys,json;m=json.load(sys.stdin);print(len([t for p in m['packages'] for t in p['targets'] if 'bin' in t['kind']]))"`.
It has one row per target and the fields `target`, `owner`, `distribution` (`INSTALL`,
`FOREIGN`, or `UNRELEASED`), and `adapter`. The installer, runtime adapter list, and
post-install expected set MUST all be projections of this manifest; none may maintain a second-hand count. In the historical 23-target snapshot, installer_entries=3, foreign=1 (pane-truth), and owned_entries=2; current metadata has 48 target rows and the current ratio is recorded in NUMBERS.toml.
~~The seven runtime adapters are a projected manifest field, not a second denominator.~~ **RETRACTED 2026-09-01 — see §7.9: no seven-item list exists anywhere in the repo.** The expected
install set is exactly the rows with `distribution=INSTALL`, so a foreign or unreleased row can
never be mistaken for a missing artifact.

**The defect class, for the fourth time in this document's history:** a number produced by an
instrument that does not measure the quantity in the sentence. `06-gates` did it with test counts
(`370`/`379`), `gap_propagation.rs` did it with a baseline carried between detectors, `02-surface-census`
did it with a denominator that grew 50% in one exchange, and this section did it with `fn main`.
Every one survived multiple readings because a plausible integer reads as a measurement.

**CURRENT RECHECK — pane-truth is now present in this workspace.** which pane-truth still resolves to /Users/josh/.local/bin/pane-truth, but crates/pane-truth exists and ls -1 crates | wc -l returns 50. The old GHOST conclusion was valid only for the pre-extraction snapshot; it is retired. The remaining identity question is whether the installed binary matches current HEAD, which requires the four-way identity receipt below.
**MEASURED — one of those three binaries cannot report its own identity.**
`grep -c 'version' crates/tick-monitor/src/main.rs` returns `0`. `tick-monitor` has no `--version`
flag and no version string anywhere in its entrypoint. Its identity is not *unmeasured*; it is
*unmeasurable by construction*. No amount of probing an installed `tick-monitor` will tell you
which commit produced it. In the 23-target historical snapshot, only 5 targets mentioned --version at all
(`for c in $(ls crates); do grep -c '\-\-version' crates/$c/src/*.rs; done` — nonzero for
`installer`, `kernel-only-operator-hook`, `omp-inventory-map`, `omp-orchestrator`,
`omp-rpc-session`).

That is not an incidental omission. The brief's five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (§4) records exactly one
layer as **WORKS**: *observe*, and its mechanism is `tick-monitor`. The single working layer of the
system is carried by the one binary in the install set whose provenance cannot be established from
the artifact. If observation is the only thing we can currently trust, we cannot currently prove
which build produced the observations.

**MEASURED, and it bounds everything in this section.** The brief's §4 also records *actuate* as
**DOES NOT EXIST — a human types into panes**, and *complete* as **AVAILABLE, NOT WIRED —
`AgentEndEvent` crossed the wire but the supervisor does not consume it**. Installability is therefore not merely
unbuilt; two of the five layers it would need to install are unbuilt. A second machine that ran a
perfect installer today would receive a working observer, a broken actionable filter, a fenced
consumer, and two absent layers. Every PROJECTED item below is contingent on those layers landing,
and §09 owns that sequencing.
**CORRECTED 2026-09-01 — the defect described here is gone at HEAD, and the description below
matched code at neither of the two commits we checked.** An earlier revision of
`crates/installer/src/main.rs` derived `owned` as `BINARIES.len() - foreign`; at snapshot fb89714
and at HEAD be012d9 the code instead counts `owned` explicitly in the loop (`:74`), with a comment
(`:69-71`) explaining that the derived form is avoided so the OK line prints the same variable it
decremented. What survives is the lesson, which is why the paragraph stays: the OK line and the
DRIFT line must print counts from the SAME variable, or a mismatch between them is undetectable.
The current code satisfies that; the paragraph is now a regression note, not a live defect.

**MEASURED — the false-green class is real and recent.** Earlier in this session the installer's
`main()` printed `installer: not yet wired to the live fleet` and returned SUCCESS. A no-op that
exits 0 is not "incomplete"; it is a *false green* — it supplies evidence of health it did not
gather. HEAD `fb89714` has replaced it with `--check`/`--install` verbs, but the lesson is a
permanent gate item: **a command that did not perform its work MUST NOT exit 0.** The refusal exit
code (§3) exists so that "I declined" is distinguishable from "I checked and it was fine."

**Historical MEASURED, prior to HEAD.** The four-way identity proof, first run, found 3/3 probed
binaries disagreeing with HEAD, and the launchd supervisor running a build 23 commits behind HEAD.
Not re-derivable at `fb89714`; recorded as history, not current state.

NO-CLAIM: §1 measures the *declared* install surface and identity surface. It does not claim the
18 unlisted binaries are unusable, nor that any of them is broken — only that none of them has a
documented, reproducible path from this repo onto a second machine.

### 2. The canonical CLI contract we will ship

PROJECTED for the whole of §2, except the envelope shape, which is MEASURED at
`crates/omp-inventory-map/src/lib.rs:613` and `:1366`.

We adopt `/canonical-cli-scoping` and `/cfs-cli-discipline` wholesale rather than minting a local
standard. The unit of installation is a single umbrella binary, `omp-orchestrator`, which scopes every one of
the 48 workspace binary targets as an adapter — the aggregator shape both skills mandate. We do not ship one CLI per target; we ship one CLI whose doctor takes an adapter name. Every command emits
the envelope we already emit today:

```json
{"schema_version":"<surface>/v1","command":"<verb>","status":"OK|DEGRADED|DOWN|UNKNOWN|REFUSED","data":{}}
```

Probe ids are namespaced under `^omp(\.[a-z][a-z0-9_-]*){2,}$` — e.g.
`omp.identity.binary.tick_monitor.version_absent`; bare segments are rejected at construction, not
at review. Every probe detail is a structured =N value, never prose: installed_binaries=3, workspace_targets=48, foreign=1, never looks about right.

#### The mandatory triad

**`omp-orchestrator doctor [<adapter>] [--fix] [--json]`** — diagnose every subsystem, or one adapter.
*Purpose:* answer "what is wrong and where" for an operator or agent with no context.
*Exit:* `0` all probes green; `1` at least one FAIL; `2` usage error.
*Envelope:* `data.probes[]`, each `{id, status, detail, upstream_owner?, repair_target?}`.
*Negative pattern it refuses:* **doctor must never crash when a subsystem is dead.** A dead adapter
is `status:"DOWN"` with **exit 1 for doctor (or exit 3 for critical health)**, never a panic and never zero
the caller cannot interpret. It also refuses the bundle: `omp-orchestrator doctor` with a broken
`tick-monitor` must name `tick-monitor`, not report `adapters_ok=false`.

**`omp-orchestrator health [<adapter>] [--watch -i N] [--json]`** — single-shot rollup, cheap enough for a
monitor loop.
*Purpose:* one line of truth for a supervisor, not a diagnosis.
*Exit:* `0` green; `1` degraded; `3` critical.
*Envelope:* `data.rollup` plus one line per adapter. Health is strictly a rollup *of doctor's probe
set* — the two MUST NOT be able to disagree, which means they share one classifier, not two
copies of similar logic. Two robot queries that disagree about the same state is a contract
violation, and we will pin it with a convergence test.
*Negative pattern it refuses:* health MUST NOT perform I/O that mutates or that can hang
unboundedly. It is called in a loop; a health check that blocks is an outage amplifier.

**`omp-orchestrator repair --scope <adapter> [--dry-run] [--apply --confirm]`** — idempotent fix for a named
failure class.
*Purpose:* convert a doctor finding into a corrected state, reversibly.
*Exit:* `0` no-op or success; `1` at least one repair failed; `5` concurrency lost (another repair
holds the lock).
*Envelope:* `data.actions[]` with `{action, state: PLANNED|APPLIED|SKIPPED, reason, backup_path,
before_hash, after_hash}`; in dry-run, `data.actual_actions` MUST be empty and the envelope MUST
say so explicitly.
*Negative pattern it refuses:* dry-run is the default. `--apply` without `--confirm` is a usage
error, not a prompt. `--dry-run --apply` together is rejected as oxymoronic. And per
`/world-class-doctor-mode-for-cli-tools`: **detect-then-fix, never fix-then-detect** — every write
routes through one `mutate()` chokepoint that takes a verbatim backup into
`.omp/runs/<run-id>/backups/` before touching anything, so `omp-orchestrator repair undo <run-id>` restores
byte-for-byte. If it cannot be undone byte-for-byte from the artifact, it does not ship.

#### The subsidiary triad

We are unambiguously state-handling: we read `git ls-files`, write to `~/.local/bin`, own a launchd
plist, and drive tmux panes through `ntm`. The exemption does not apply.

**`omp-orchestrator validate <thing>`** — pure read, zero side effects.
*Purpose:* verify a config, a dispatch packet, a plist, or an install target *before* anything acts
on it, so a bad input is rejected at the boundary rather than half-applied downstream.
*Exit:* `0` valid; `74` validate failure (distinct from `1` so a caller can branch on "your input
is wrong" versus "the system is unhealthy"); `2` usage.
*Envelope:* on reject, `{status:"REJECT", reason, expected, observed, observed_length}` — the
observed field is mandatory, because a rejection without the observed value is undebuggable.
*Negative pattern it refuses:* validate MUST NOT touch the filesystem outside reads, and MUST NOT
be satisfiable by a mutation. A "validate" that fixes as it goes is a repair with a lying name.

**`omp-orchestrator audit [--since <ts>]`** — append-only ledger of every mutation with provenance.
*Purpose:* answer "what did this tool change on this machine, when, and under whose authority"
without reading the tool's source.
*Exit:* `0`; `1` ledger unreadable or corrupt.
*Envelope:* rows carry `{ts, actor, verb, idempotency_key, touched_paths[], receipt_path,
post_check, result}`.
*Negative pattern it refuses:* the ledger is append-only and schema-versioned. A repair that
mutates without appending an audit row is a bug the mutate-auditor test fails the build on. An
ambiguous audit read refuses the mutation rather than proceeding blind.

**`omp-orchestrator why <id>`** — provenance trace for one object.
*Purpose:* explain how a binary, a probe verdict, a bead, or a dispatch reached its current state,
including where the chain of evidence breaks.
*Exit:* `0` found; `1` unknown id.
*Envelope:* `data.chain[]` from origin to current state with the evidence at each hop.
*Negative pattern it refuses:* `why` MUST NOT synthesize a plausible explanation. If the chain has
a gap, the gap is a node in the output (`{hop:"build_id", status:"UNKNOWN", reason:"binary exposes
no --version"}`), not an omission. This is the direct answer to `tick-monitor`.

#### Self-documentation and discoverability

`--info`, `examples`, `quickstart`, `help <topic>`, `completion <shell>` are mandatory, and
`completion bash | bash -n -` must exit 0 — a completion script that emits broken syntax fails
silently in production.

**ADDRESSABLE is a first-class gate property, not a nicety.** MEASURED (brief §3.6):
`omp-inventory-map --help` returns
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR","data":null,"error":"CONFIG_ERROR unknown argument --help"}`.
Historical MEASURED values from brief §3.6 were `13 tests` and `544 KB` of doctor output, but
that record retained neither the exact command nor the output artifact. They are therefore not
re-derivable measurements and MUST NOT serve as acceptance denominators. The replacement receipt
must record these exact commands and their artifacts:

`cargo test -p omp-inventory-map -- --list 2>/dev/null | tee artifacts/inventory-map-tests.list | grep -c ': test$'` (test count and saved stdout), and
`omp-inventory-map doctor --json > artifacts/inventory-map-doctor.json; wc -c < artifacts/inventory-map-doctor.json; grep -E 'Observation|CONVERGE|Verdict' artifacts/inventory-map-doctor.json` (byte count and token search, with the named JSON artifact). The receipt records the
exit code and commit for both; a changed command, scope, or artifact path is a new measurement.
This is not built-versus-wired; it is **wired-but-unaddressable**, and a correct gate nobody can invoke has the
same operational value as no gate.

Installability is where that property is either satisfied or lost for good. A binary distributed to
a second machine is reachable only through its documented surface; there is no repo to grep and no
author in the room. Under the umbrella, every adapter is reachable as `omp-orchestrator doctor <adapter>`,
`omp-orchestrator help <adapter>` names the command, and `omp-orchestrator doctor capabilities --json` enumerates every
probe id — so ADDRESSABLE is discharged by the single umbrella CLI shape, rather than by separate
implementations that can each drift independently. A capabilities snapshot is checked in as a
golden artifact; drift between the declared probe list and the implemented one fails CI.

#### Upstream-report

We wrap `omp`, `ntm`, `br`, `bv`, `git`, `cargo`, and `tmux` (versions in the surface census
section). When an adapter probe fails on the *substrate* side, the envelope carries
`class:"upstream_substrate_issue"` and `upstream_owner:"<vendor>"`, and
`omp-orchestrator upstream-report <adapter>` drafts the issue. Without this, every upstream bug is silently
absorbed as our bug and we lose the forcing function to file it.

NO-CLAIM: §2 specifies a surface. It does not claim any of these commands exist, and it does not
specify the internal probe list — that is the gate section's job.

### 3. Exit-code dictionary

PROJECTED as a shipped contract; the rows marked MEASURED are already emitted by code at HEAD.

| Code | Name | Meaning | Caller should |
|---:|---|---|---|
| 0 | `OK` | Work performed, all green. Never emitted by a command that declined to run. | Proceed |
| 1 | FINDINGS | Work performed, at least one FAIL. MEASURED: installer/src/main.rs:103-105. | Read data.probes[] |
| 2 | USAGE | Malformed invocation. MEASURED: installer/src/main.rs:45 prints usage and returns 2. | Fix the command line |
| 2 | `UNKNOWN` (envelope) | MEASURED: the inventory map exits 2 carrying `"status":"UNKNOWN"` — a probe ran but could not reach a verdict. | Treat as not-green |
| 3 | CRITICAL / NO_INPUT | Prerequisite absent: no git HEAD or no build output. MEASURED: installer/src/main.rs:65 and :170. | Fix environment |
| 5 | `CONCURRENCY_LOST` | Another mutation holds the lock. | Retry later |
| 70 | `ADVISORY` | Non-blocking finding. | Log |
| 71 | `SYSTEM_ERROR` | Our bug, not the user's. | File a bead |
| 74 | `VALIDATE_FAILURE` | Input is invalid; the system is fine. | Fix the input |
| 75 | REFUSED | The command declined to run. MEASURED: installer/src/main.rs:126 returns 75 when the build fence blocks install. | Not a result |
| 103 | `REFUSED_UPSTREAM` | An upstream guard declined. MEASURED behaviour: the RCH / mint-floor guard exits 103 with `0 passed / 0 failed`. | **Not a result** |

The two refusal rows carry the sharpest operational lesson in this document. **`exit 103` with
`0 passed / 0 failed` is a refusal, not a test result.** Zero failures did not happen because the
code is good; zero failures happened because zero tests ran. Reading that as green is precisely the
error our async contract names as *"a timeout is not a verdict"*: the absence of a negative signal
from a process that never produced a signal is not evidence. Every refusal code therefore gets its
own `status:"REFUSED"` value in the envelope, distinct from `OK` and from `DOWN`, and the CI
aggregator treats `REFUSED` as blocking rather than passing.

NO-CLAIM: this table does not claim exit codes are uniform across the 48 current binary targets. At the historical snapshot, installer used 1/2/3/75 and omp-inventory-map used 2; the other targets remain unaudited against this projected dictionary.

### 4. Identity and drift

PROJECTED as a shipped always-on check. The four-way identity proof asserts that four independently
sourced facts agree:

1. **HEAD** — `git rev-parse HEAD` in the owning repo.
2. **build_id** — the commit sha compiled *into* the artifact.
3. **`--version`** — what the installed binary says when asked.
4. **running** — what the currently-executing process (launchd job, tmux pane, daemon) reports.

Disagreement between (1) and (2) means the artifact was not built from the tree. Between (2) and
(3), the binary on disk is not the artifact we built. Between (3) and (4), the process serving
traffic is not the binary on disk — a stale process still holding an unlinked inode, which is the
failure mode that let the launchd supervisor run 23 commits behind HEAD while every static check
passed. The proof is stated as *detection of disagreement*, deliberately: agreement across four
sources raises the floor on how a stale artifact can survive, and does not guarantee freshness.
Rule 5 of the writing contract applies to identity checks as much as to gates.

Three design commitments follow:

**Build-id embedding is mandatory, not optional.** Every crate that produces a binary gets a
`build.rs` that emits the git sha into the binary, and every binary exposes it. This closes
`tick-monitor` structurally: a binary that cannot answer "which commit are you" cannot be
installed, because the installer refuses to install an artifact that fails the identity self-report
at install time. MEASURED prior art: `beads_rust/build.rs` in the mirror uses `vergen-gix` to emit
`VERGEN_GIT_SHA` plus build timestamp, target triple, and rustc semver, with a quiet
`rev-parse --is-inside-work-tree` guard so the build still succeeds outside a work tree. That guard
matters — a build script that hard-fails when git is absent breaks `cargo install` from a crates.io
tarball. Eight mirror repos ship a `build.rs` of this shape
(`grep -rl 'GIT_SHA\|vergen\|git_hash\|BUILD_SHA' $M/*/build.rs $M/*/src/*.rs`).

**Every binary declares its owning repo.** The declaration is compiled in alongside build_id. A
binary on PATH whose declared owner is not this repo is reported `FOREIGN`, named explicitly, and
**excluded from the drift denominator** — because `pane-truth` will never agree with our HEAD and a
check that reports a permanent, unfixable mismatch trains operators to ignore the check. FOREIGN is
a third outcome alongside CONSISTENT and DRIFTED, and it is printed, not swallowed.

**The denominator is printed with its derivation.** Output is
identity: consistent=N drifted=M foreign=K expected=E workspace_targets=48 probed=P — six named integers whose relationship a reader can check, not a bare 2/2.
The historical MEASURED defect was in crates/installer/src/main.rs:68 and :87 (§1): exclusion logic decremented one variable while the message printed another, yielding an arithmetically impossible but visually plausible 2/0. A ratio is only verifiable when both terms are separately named and separately sourced.

NO-CLAIM: the four-way proof detects *disagreement*. It does not prove any of the four sources is
itself honest — a binary that lies about its build_id passes. Detecting that requires reproducible
builds, which we are not claiming.

### 5. Distribution

PROJECTED. Per `/installer-workmanship`, the shipped install path must have: a curl one-liner with
a cache buster, proxy support on every fetch, platform detection (`darwin`/`linux` ×
`x86_64`/`aarch64`, musl for Linux), preflight checks (disk, write perms, network, existing
install), atomic mkdir-based locking with stale-PID detection (never `flock` — absent on macOS),
SHA256 verification via `sha256sum` *or* `shasum -a 256`, Sigstore verification when cosign is
present, a build-from-source fallback, `install -m 0755` for the atomic place, shell completions to
XDG paths, PATH setup, trap cleanup EXIT, a final per-component status summary, and printed
uninstall instructions.
**Bootstrap authenticity is a precondition, not a post-install hope.** The generated `install.sh`
MUST never be executed directly from a curl pipeline. The one-liner downloads it into the private
temporary directory, verifies its SHA256 against the pinned digest in the versioned release metadata
(or verifies a detached signature against the release channel's trusted public key), and executes it
only after verification succeeds. The Rust installer then repeats artifact verification before
placing any target. If neither a trusted package channel nor a pinned digest/signature is available,
the curl path is unavailable and the supported fallback is an exact-commit `cargo install --git`
from the trusted source; there is no unsigned bootstrap fallback.

MEASURED prior art from the mirror (`ls $M/*/install.sh | wc -l` → 50 installers; count caveat in
§7): 38/50 verify a checksum, 29/50 use `install -m 0755`, 17/50 mention uninstall, 16/50 install
completions. Checksum verification and atomic placement are near-universal in the house style;
reversibility is not. We will be in the 17, not the 33.

**The tension, named.** The repo's one hard rule is **no `.sh`, no `.py`** — a Rust gate walks
`git ls-files` and fails the build on either extension, with an empty exemption list. Every
reference installer in the mirror is `install.sh`. These are in direct conflict and hand-waving it
is not acceptable in a document meant to be attacked.

**The resolution.** The rule governs *tracked files in this repo*, not *published release
artifacts*. So:

- The install logic lives in the `installer` Rust crate. It is the real implementation: platform
  detection, checksum verification, atomic placement, identity proof, completion install, plist
  management, uninstall.
- The curl one-liner is a **generated release artifact**. A `cargo xtask`-style Rust command emits
  `install.sh` into `target/release-artifacts/` at release time; CI uploads it to the GitHub
  release; it is never `git add`ed. `git ls-files` never sees it, so the gate never fires, and the
  gate keeps its empty exemption list — which is the property that makes the gate credible.
- That generated shell script is deliberately thin: detect platform, download `install.sh` and its
  pinned release metadata, verify the bootstrap before executing it, then fetch `installer-<target>`
  and verify its SHA256 against the published `SHASUMS256.txt`. What shell is bad at
  — JSON merging, identity arithmetic, idempotent repair — happens in Rust. What shell is uniquely
  good at — bootstrapping before any of our binaries exist — happens in ~80 readable lines. The
  generator is golden-tested: the emitted script is diffed against a checked-in expected output
  stored as a `.txt` fixture (not `.sh`, so the gate stays happy), and drift fails CI.
The installer-workmanship contract is complete only when the release path also covers the following
items; each is an acceptance check, not an optional polish item:

- **Shell safety and output:** the generated bootstrap uses `set -euo pipefail`, quotes every
  expansion, uses a private temporary directory, traps cleanup on EXIT and signals, and emits
  human-readable gum/ANSI status only when stdout is a TTY (plain deterministic lines otherwise).
- **Fetch modes:** every network fetch honors `HTTPS_PROXY`, `HTTP_PROXY`, and `NO_PROXY`;
  `--proxy` overrides them explicitly, and `--offline` refuses before any network access while
  consuming only a verified local cache. No fetch path bypasses the selected proxy or offline mode.
- **Version and repeatability:** an explicit version is preferred; if the latest-version lookup is
  unavailable, the installer falls back to the release version encoded in the immutable artifact
  URL and reports that fallback. An already-installed matching build exits successfully without
  rewriting files; a different build produces a planned replacement and requires confirmation.
- **Operator setup:** installation configures the agent-facing PATH, shell completions, hook and
  skill registration, and the doctor/health entrypoints, with each resulting path listed in the
  final status summary. Missing optional integrations are reported as `SKIPPED`, never silently
  treated as installed.
- **Migration and removal:** a predecessor install is detected by its owner/build id, backed up,
  migrated or explicitly refused before replacement, and left reversible. The final summary prints

exact uninstall and rollback commands and identifies every component that was changed.
The contract is not discharged until each bullet has a deterministic acceptance result in the
install receipt; the generated-script golden test covers syntax and control flow, while the Rust
installer tests exercise the platform, proxy/offline, repeat-install, setup, and migration matrix.

This is not a loophole. It is the honest boundary: the rule exists so that logic is not smuggled
into untested shell inside our repo, and an 80-line generated bootstrap whose output is
golden-tested in Rust does not violate that intent. If a reviewer disagrees, the fallback is
`cargo install --git`, which needs no shell at all and which we will document either way.

**The self-test exercises the install; it does not certify it.** `omp-orchestrator doctor --json` immediately
post-install is the acceptance criterion, not "the files landed." The install is accepted when the
four-way identity check reports `consistent=N drifted=0 foreign=K expected_set=N missing=0 probed=P` with every term
printed, and `omp-orchestrator health` returns 0. That raises the floor from "bytes were copied" to "the copied
bytes answer for themselves"; it does not establish that the installed system does useful work, which is §6's and §09's problem.
The `expected_set` is not a best-effort count: it is the sorted target-name set from canonical
manifest rows with `distribution=INSTALL`, printed (or emitted as a hash plus names) in the receipt.
The doctor MUST probe every member, report each absent member as `missing=<target>`, and refuse the
install with a nonzero exit whenever `missing!=0` or any expected target is `DRIFTED`. `FOREIGN`
rows are named and excluded from `expected_set`; they cannot make a partial install appear green.
An installer that copies files and exits 0 without running the check is the §1 false-green class again.

NO-CLAIM: HEAD does not claim that a signing key already exists or that signed releases are ready at
launch. The release contract nevertheless requires a pinned bootstrap digest from a trusted channel,
or a detached signature verified against a trusted public key, before execution. Sigstore may be the
signature mechanism; if a signature is advertised and cosign is present, a bad signature hard-fails.
Absent both a trusted channel and a pinned digest/signature, the curl installer is not published.

### 6. Multi-machine

PROJECTED. Three categories of hardcoding must be resolved, and they resolve differently.
**Internal-buyer pilot scoreboard (PROJECTED).** Installability earns adoption only if a person who
is not Josh can reach first useful use. We will run a five-person/five-machine pilot and record one
receipt per attempt. Baseline at this HEAD is `pilot_attempts=0`, so the operational baseline for
success, time, and support is **not yet measured**, rather than an invented percentage. The targets
are:

| metric | pilot target | evidence in each receipt |
|---|---:|---|
| first-attempt install success | ≥ 4/5 (80%) | verified bootstrap, expected-set `missing=0`, and health exit 0 |
| median time to first use | ≤ 10 minutes | timestamps from verified bootstrap start to first successful `omp-orchestrator doctor` and one accepted adapter invocation |
| operator support burden | ≤ 1 intervention per install | intervention count plus named reason; retries caused by installer defects count |
| downstream value | ≥ 4/5 complete one real, reversible adapter action | receipt with `status=APPLIED`; typed refusals are reported separately and do not count as useful work |

A release does not claim adoption from installation bytes alone: it must publish the pilot counts,
median, and intervention total, and any target miss remains a release-blocking finding until the
cause is classified. This measures internal buyer value without pretending that an external customer
or useful-work result already exists.

**Becomes config.** `crates/installer/src/main.rs:25` falls back to
`PathBuf::from("/Users/josh")` when `HOME` is unset, and lines 118-119 default `CARGO` to
`~/.cargo/bin/cargo` — a deliberate bypass of the RCH shim measured at
`/Users/josh/.rch/shims/cargo`. Both are correct behaviours for this machine and wrong as
defaults. They become a config file at `$XDG_CONFIG_HOME/omp-orchestrator/config.toml` with
per-machine overrides, and the `/Users/josh` literal becomes a hard error rather than a fallback —
an unset `HOME` is an environment we should refuse, not guess at. This is the scope the
portability bead already owns; it is not new work invented here.

**Becomes discovery.** Repo root is currently derived from `CARGO_MANIFEST_DIR` at compile time
(`main.rs:16-20`), which is correct when running from the build tree and meaningless for an
installed binary. The installed binary discovers its repo by walking up from `cwd` to the nearest
`.git`, and cross-checks the discovered repo against its compiled-in owning-repo declaration
(§4). Mismatch is a typed refusal, not a guess.

**Becomes a typed refusal.** A machine without `tmux`, `ntm`, or `br` cannot run the orchestrator.
It must **fail closed** with a structured envelope that names the missing dependency, the probed
path, and the install command:

```json
{"schema_version":"omp-preflight/v1","command":"doctor","status":"REFUSED",
 "data":{"probe":"omp.preflight.dependency.ntm.absent","required":"ntm",
         "probed":["/usr/local/bin/ntm","/opt/homebrew/bin/ntm"],
         "install":"curl -fsSL https://.../ntm/install.sh | bash","exit":75}}
```

Never a silent partial. An orchestrator that starts without `ntm` and quietly dispatches nothing is
the false-green class at fleet scale — it will report ticks, report health, and move zero work. The
degradation contract is binary per adapter: an adapter is either fully available or `DOWN` with a
named remediation. There is no "mostly working."

Note the version fragility this introduces: we wrap `tmux`, which MEASURED **rejects `--version`**
(`tmux --version` → `tmux: unknown option -- -`). Dependency probing must not assume every
substrate answers a version flag; the probe records `version:"UNPROBEABLE"` with the rejection text
as evidence rather than treating the failure as absence.

NO-CLAIM: §6 addresses a second *macOS* machine and a Linux machine with the same substrates
installed. It does not claim Windows support, and it does not claim the orchestration semantics
(pane dispatch, ack staging) are machine-independent — only that the install and preflight surfaces
are.

### 7. What Jeffrey would do

For every command in this section, `M` is explicitly `/Volumes/ZestData/dicklesworthstone-mirror`;
the reproducible shell prefix is `M=/Volumes/ZestData/dicklesworthstone-mirror` followed by the
command. The mirror counts below are historical prior-art measurements, not workspace denominators.
**Mirror census, historical and definitionally unresolved.** The prior brief recorded 216 entries, the mirror index reported 210 filesystem .git entries, and a direct ls returned 218. None of those counts validates every entry as a live git work-tree; the difference is definitional, with ntm.corrupt-20260819 and useful_tmux_commands present beside ntm. This section records the measurements as prior-art context, not as a repository denominator. The current fh index is stale and direct citations remain bounded by the command shown.

`fh`'s MCP surface is failing closed with a typed `SERVE_INPUT_STALE`, so every citation below is a
direct grep of the mirror with the command shown. Per the brief, failing closed with a remediation
hint is the model, not a defect — it is the same behaviour §6 specifies for a missing dependency.

**Gap: no reversible doctor.** *Prior art found.*
`grep -rl '"doctor"' $M/*/src/*.rs` surfaces
`coding_agent_session_search/src/doctor_chokepoint.rs`, `doctor_undo.rs`, `doctor_runs.rs`,
`doctor_robot_docs.rs`, `doctor_recover.rs`. The chokepoint's own header states the contract we
should copy verbatim: every disk write reachable from `--fix` flows through one `mutate()`, which
verifies scope, computes `before_blake3`, copies a verbatim backup into
`.doctor/runs/<run-id>/backups/<rel-path>` preserving permissions and mtime, mutates atomically via
write-tmp-then-rename, computes `after_blake3`, and appends an `ActionRecord` to `actions.jsonl`.
It also does something we should copy at the *doctrine* level: it states its own scope honestly —
"existing repair codepaths are not refactored to flow through `mutate()` in pass-1 — that is a
pass-2 task" — with a Phase-7 auditor test that ensures *new* paths use the chokepoint. That is how
you ship a partial safety envelope without lying about coverage.

**Gap: no build-id embedding.** *Prior art found.* `beads_rust/build.rs` uses `vergen-gix` to emit
`VERGEN_GIT_SHA`, build timestamp, target triple, and rustc semver, guarded by a quiet
`rev-parse --is-inside-work-tree` probe. Eight mirror repos ship the same shape (`beads_rust`,
`beads_viewer_rust`, `coding_agent_session_search`, `coding_agent_usage_tracker`,
`cross_agent_session_resumer`, `destructive_command_guard`, `pi_agent_rust`, `rust_stream_deck`).
We adopt it unchanged rather than hand-rolling a `git rev-parse` in each `build.rs`.

**Gap: no installer.** *Prior art found, 50 exemplars.* Counts in §5. The reference pair named by
`/installer-workmanship` — `destructive_command_guard/install.sh` and
`remote_compilation_helper/install.sh` — are the shape to emulate for the generated bootstrap.

**Gap: no typed missing-dependency refusal.** *Searched
`MISSING_DEPENDENCY|missing_dependency|DependencyMissing` across `$M/*/src/*.rs` — no prior art
found.* Jeffrey's installers detect missing dependencies in shell at install time; none of the
Rust binaries surfaced by that pattern carries a typed runtime refusal for an absent substrate.
This is a place where our aggregator shape is genuinely different: the release manifest must expose an explicitly enumerated adapter set, but **no seven-item adapter list exists today**. Each future adapter may map to one or more target rows and be independently refused or reported as DOWN; that design remains PROJECTED and is not borrowed prior art.

NO-CLAIM: §7 cites prior art for the *pattern*. It does not claim we have read those
implementations in full, that they are bug-free, or that their licenses permit copying source —
only that they establish the house convention and the shape we will implement independently.

### 8. Constraints this section adds to the repo (R11)

R11 says a requirement that lives only in conversation is a requirement that will be dropped. Four
constraints were derived here and did not previously exist in writing anywhere in `docs/`. They are
recorded as repo doctrine, not as prose inside an argument:

1. **A command that did not perform its work MUST NOT exit 0.** Refusal gets its own code and its
   own `status:"REFUSED"`, and CI treats `REFUSED` as blocking. Derived from the false-green
   installer and from `exit 103` with `0 passed / 0 failed`.
2. **A ratio is printed with both terms separately named and separately sourced.** No `2/2`. This
   generalises rule 4 of the writing contract from prose into program output.
3. **A binary that cannot report its own build_id is not installable.** The installer refuses it.
   This converts `tick-monitor`'s missing `--version` from a nice-to-have into a release blocker.
4. **The no-`.sh` rule governs tracked files, not generated release artifacts.** The installer is
   Rust; the published one-liner is emitted at release time into `target/release-artifacts/`, is
   never `git add`ed, and its expected output is golden-tested from a `.txt` fixture. The gate's
   exemption list stays empty, which is the property that makes the gate credible.

NO-CLAIM: these four are constraints this section proposes. They are not ratified by a gate, and none is asserted as enforced until a current receipt names the source revision, artifact, and command.

---

**Section NO-CLAIM.** This section specifies distribution, identity, and the CLI contract. It does
not specify the probe list (gates section), the orchestration semantics (crate specs), or the
milestone at which each surface lands (milestones section). Every PROJECTED item is unbuilt in the current worktree; the only MEASURED install-adjacent code is crates/installer, which covers 3 installer entries against 48 current workspace targets, one of which is foreign and not built here.

---

## 7.9 Historical blocker resolution — the seven-adapter ambiguity is superseded
GradeInstall filed two BLOCKERs against this section:

> The document asserts every target becomes an adapter (21) while simultaneously
> claiming seven adapters. If 21 targets map to 7 adapters, the grouping rule is
> absent — which targets belong in which adapter is unspecified, making the CLI
> contract unexecutable.

> The document specifies commands that take `<adapter>` parameters but provides zero
> examples of valid adapter names. A user on a second machine cannot invoke
> `omp-orchestrator doctor <adapter>` without guessing.

Both are correct, and measurement makes them worse rather than better.

### What is actually true, 2026-09-01

| claim | measured | derivation |
|---|---:|---|
| binary targets | **48** | cargo metadata --format-version 1 --no-deps, registered as built_binaries |
| known to the installer | **3** | `omp-orchestrator`, `pane-truth`, `tick-monitor` |
| "seven runtime adapters" | **no list exists** | grep of the whole repo finds no enumeration |

The `21` was already stale when graded — two crates landed since — which is why it is
registered as a derived figure rather than written in prose. **The `7` is worse than
stale: it is unsourced.** Five places in this section invoke `omp-orchestrator doctor <adapter>`,
and no adapter is named in any of them.

### Retraction

The sentence *"the seven runtime adapters are a projected manifest field, not a second
denominator"* is **retracted**. It defended a number against being read as a
denominator while never establishing where the number came from. There is no
seven-item list, no grouping rule from 23 targets onto 7 names, and no way for a
reader to check either.


What replaces it: **48 current target rows, 3 installer names, and 45 unlisted targets**. Of the three names, two are owned install entries and one is the foreign pane-truth binary; this is the current arithmetic, not a claim that all 48 targets are installable.

**NO-CLAIM:** this resolves the arithmetic, not the design. Whether the right shape is one omp-orchestrator aggregator with adapter subcommands, separate installed binaries, or something else remains open — against a measured 3-of-48 listed-target count, rather than behind a seven that nobody could look up.
---

## 7.10 Historical journey surfaces, mapped — 1 of 16 names its own timeout

Josh's standing objective, verbatim: *"Every surface of our journey mapped to specific
commands with proper guards and timeouts, everything typed — nothing unknown."*

`Lens05Actions`, the held-out operator-at-3am lens, filed the BLOCKED form of this:

> 466-line specification of what 11 actions SHOULD do but **NO STATED COMMAND,
> BINARY, API, or FUNCTION CALL** to actually RUN any action.

**HISTORICAL MEASURED (2026-09-01; provenance incomplete).** The source revision, exact per-target
`--help` invocation, and captured output artifact were not retained. The table below is therefore a
plan snapshot, not acceptance evidence. A future bead MUST record the metadata target list, the exact
probe command, every target's output/exit code, and a SHA-256 before publishing these ratios.

### Historical 23-target help snapshot

| behaviour | count | what it means for a stranger |
|---|---:|---|
| **NOT-BUILT** | 7 | the release binary does not exist; nothing to invoke |
| **REAL-HELP** | 6 | historical snapshot: usage line with no error; the snapshot's first-line instrument answered 7, while the current registered help_discoverable_binaries command answers 10 |
| **REJECTS** | 3 | answers `unknown argument: --help` |
| **HELP-AS-PATH** | 3 | treats `--help` as a **filesystem path** and reports it missing |
| **SILENT** | 2 | prints nothing at all |
| **EXECUTES** | 1 | **runs the gate** instead of describing it |
| ERRORS | 1 | errors on an unrelated precondition |

**Historical snapshot: names a timeout or deadline in its own help: 1 of 16 buildable** —
dispatch-silence-watch. Fifteen did not in that snapshot; this ratio is not the current 48-target help census.

### The three that deserve naming

- **`no-shell-gate`, `state-wildcard-lint`, `undrained-pipe-lint`** treat `--help` as a
  path: *"cannot read --help"*, *"repo root --help: No such file or directory"*. An
  operator asking a gate what it does is told their file is missing.
- **`pre-commit-gate` EXECUTES.** Asking it for help runs the gate — output
  `MULTI-GATE: no staged files to check`. On a dirty tree that is a real gate run with
  real refusals, produced by a request for documentation.
- **`loop-queue-filter` and `pre-delete-citation-check` are silent.** No usage, no
  error, exit and nothing. Indistinguishable from a binary that does nothing.

### What this does and does not close

**Historical snapshot only:** the commands existed for 16 built targets, and guards/timeouts were discoverable for 1 of 16. The current registered first-line help command answers 10; a fresh full matrix receipt is still required before current invocation coverage is claimed.

**Does not close** the actions themselves. This maps BINARIES; `Lens05Actions`'
complaint was about **actions A1–A11**, which are specified as *behaviours* and do not
correspond one-to-one with bin targets. Several actions are functions inside
`omp-orchestrator` with no independent entry point, so no `--help` probe can find them.
Mapping action → binary → subcommand is a further step and is **not built**.

**NO-CLAIM:** `--help` answering is a proxy for discoverability, not a measure of it.
A binary with perfect help can still be undiscoverable if nothing tells an operator it
exists — which is §2's `ADDRESSABLE` property, satisfied by **zero of eight gates**.
This probe measures the second gate of two, and the first is still shut.
