# 07 — Installability: distribution, identity, and the canonical CLI contract

This section answers one question an investor will ask before any other: *if this works on Josh's
Mac Studio, what makes it work on a second machine, in a second repo, for a person who is not
Josh?* Today the honest answer is "nothing yet, and we can prove it." What follows is the measured
starting point, then the contract we will ship against.

All measurements in this section were taken on 2026-08-31 at repo HEAD `fb89714`
(`git rev-parse --short HEAD`). Every current claim is marked MEASURED or PROJECTED. MEASURED means a
command in this document produced the number; historical values explicitly marked non-rederivable
are retained only as audit context and are not acceptance evidence. PROJECTED means we intend to
build it and it does not exist.
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

**MEASURED — the workspace declares 21 binary targets, and the installer knows about 3 of them.**
The target declaration count is not itself a successful artifact count; the artifact proof is the command and receipt described below.

The number was `18` until round 10, taken from `grep -rl 'fn main' crates --include='main.rs' | wc -l`.
The investor lens filed it as *"fn main count is not binary/build evidence"* and it was right twice
over — the proxy was wrong, and by the time it was challenged the count had drifted too. Three
instruments, three answers, re-measured at commit time:

| instrument | answer | what it actually counts |
|---|---:|---|
| `grep -rl 'fn main' --include=main.rs` | 18 | crates having a `main.rs` — a **source shape**, not an artifact (re-measured 2026-09-01 at both fb89714 and be012d9; the 17 figure and the NUMBERS.toml note were one behind) |
| `grep -c '^\[\[bin\]\]' crates/*/Cargo.toml` | 16 | **explicitly declared** targets; misses every implicit `src/main.rs` |
| `cargo metadata --format-version 1 --no-deps` (targets with `bin` kind) | **21** | the workspace's declared binary targets; this is the canonical denominator |

Only the third is the canonical **target** denominator; it counts both implicit and explicit Cargo
binary targets and does not pretend that a source shape or an explicit-`[[bin]]` grep is an artifact.
Successful output is a separate proof: `cargo build --workspace --message-format=json` is filtered to
`compiler-artifact` messages whose target kind is `bin`, and the receipt records target names,
artifact paths, and the command exit code. A target in metadata without a successful artifact is
reported missing and cannot satisfy install acceptance.

`crates/installer/src/main.rs:12` declares
`const BINARIES: &[&str] = &["omp-orchestrator", "tick-monitor", "pane-truth"];`.

**MEASURED — the current installer list covers 3/21 target rows (14%), but only 2/21 are owned
install entries (9.5%); the third is foreign.** The remaining 19 workspace targets have no current
owned install path and are reachable only by someone who already has the repo, a toolchain, and the
knowledge of what to build.

**Canonical manifest (PROJECTED; the one denominator for this section).** The release manifest is
generated from the Cargo target graph with the exact command in `NUMBERS.toml`:
`cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -c "import sys,json;m=json.load(sys.stdin);print(len([t for p in m['packages'] for t in p['targets'] if 'bin' in t['kind']]))"`.
It has one row per target and the fields `target`, `owner`, `distribution` (`INSTALL`,
`FOREIGN`, or `UNRELEASED`), and `adapter`. The installer, runtime adapter list, and
post-install expected set MUST all be projections of this manifest; none may maintain a second-
hand count. At this HEAD the manifest denominator is `workspace_targets=21`; the current installer
list is `installer_entries=3`, of which `foreign=1` (`pane-truth`) and `owned_entries=2`.
The seven runtime adapters are a projected manifest field, not a second denominator. The expected
install set is exactly the rows with `distribution=INSTALL`, so a foreign or unreleased row can
never be mistaken for a missing artifact.

**The defect class, for the fourth time in this document's history:** a number produced by an
instrument that does not measure the quantity in the sentence. `06-gates` did it with test counts
(`370`/`379`), `gap_propagation.rs` did it with a baseline carried between detectors, `02-surface-census`
did it with a denominator that grew 50% in one exchange, and this section did it with `fn main`.
Every one survived multiple readings because a plausible integer reads as a measurement.

**MEASURED — one of those three names has no source in this workspace.**
`which pane-truth` returns `/Users/josh/.local/bin/pane-truth`. There is no `crates/pane-truth`
directory (`ls -d crates/*/ | wc -l` → `26`, and the listing contains no `pane-truth`; this agrees
with the census figure `workspace_crates=26` in the brief §3.2). The binary belongs to
control-plane, a different repo. It is on PATH, it is named in our installer's BINARIES list, and
we do not build it. That is the GHOST class: an artifact the tooling asserts authority over and
cannot produce. Any drift check that treats it as ours will report a mismatch forever, because
there is no HEAD in this repo it could ever agree with.
**MEASURED — one of those three binaries cannot report its own identity.**
`grep -c 'version' crates/tick-monitor/src/main.rs` returns `0`. `tick-monitor` has no `--version`
flag and no version string anywhere in its entrypoint. Its identity is not *unmeasured*; it is
*unmeasurable by construction*. No amount of probing an installed `tick-monitor` will tell you
which commit produced it. Across the workspace, only 5 of 21 binary targets mention `--version` at all
(`for c in $(ls crates); do grep -c '\-\-version' crates/$c/src/*.rs; done` — nonzero for
`installer`, `kernel-only-operator-hook`, `omp-inventory-map`, `omp-orchestrator`,
`omp-rpc-session`).

That is not an incidental omission. The brief's five-stage control loop (formerly "five-stage" — renamed, the table has five stages and seven rows) table (§4) records exactly one
layer as **WORKS**: *observe*, and its mechanism is `tick-monitor`. The single working layer of the
system is carried by the one binary in the install set whose provenance cannot be established from
the artifact. If observation is the only thing we can currently trust, we cannot currently prove
which build produced the observations.

**MEASURED, and it bounds everything in this section.** The brief's §4 also records *actuate* as
**DOES NOT EXIST — a human types into panes**, and *complete* as **DOES NOT EXIST — every
completion this session was found by a human looking**. Installability is therefore not merely
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
standard. The unit of installation is a single umbrella binary, `ompo`, which scopes every one of
the 21 workspace binary targets as an **adapter** — the aggregator shape both skills mandate: when a CLI
proxies to N substrates, the triad is scoped per-adapter, never bundled. We do not ship 21 CLIs
each with its own doctor; we ship one CLI whose doctor takes an adapter name. Every command emits
the envelope we already emit today:

```json
{"schema_version":"<surface>/v1","command":"<verb>","status":"OK|DEGRADED|DOWN|UNKNOWN|REFUSED","data":{}}
```

Probe ids are namespaced under `^omp(\.[a-z][a-z0-9_-]*){2,}$` — e.g.
`omp.identity.binary.tick_monitor.version_absent`; bare segments are rejected at construction, not
at review. Every probe detail is a structured `=N` value, never prose: `installed_binaries=3`,
`workspace_targets=21`, `foreign=1`, never "looks about right".

#### The mandatory triad

**`ompo doctor [<adapter>] [--fix] [--json]`** — diagnose every subsystem, or one adapter.
*Purpose:* answer "what is wrong and where" for an operator or agent with no context.
*Exit:* `0` all probes green; `1` at least one FAIL; `2` usage error.
*Envelope:* `data.probes[]`, each `{id, status, detail, upstream_owner?, repair_target?}`.
*Negative pattern it refuses:* **doctor must never crash when a subsystem is dead.** A dead adapter
is `status:"DOWN"` with an exit-0-or-1 envelope, never a panic and never a nonzero the caller
cannot interpret. It also refuses the bundle: `ompo doctor` with a broken `tick-monitor` must name
`tick-monitor`, not report `adapters_ok=false`.

**`ompo health [<adapter>] [--watch -i N] [--json]`** — single-shot rollup, cheap enough for a
monitor loop.
*Purpose:* one line of truth for a supervisor, not a diagnosis.
*Exit:* `0` green; `1` degraded; `3` critical.
*Envelope:* `data.rollup` plus one line per adapter. Health is strictly a rollup *of doctor's probe
set* — the two MUST NOT be able to disagree, which means they share one classifier, not two
copies of similar logic. Two robot queries that disagree about the same state is a contract
violation, and we will pin it with a convergence test.
*Negative pattern it refuses:* health MUST NOT perform I/O that mutates or that can hang
unboundedly. It is called in a loop; a health check that blocks is an outage amplifier.

**`ompo repair --scope <adapter> [--dry-run] [--apply --confirm]`** — idempotent fix for a named
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
`.omp/runs/<run-id>/backups/` before touching anything, so `ompo repair undo <run-id>` restores
byte-for-byte. If it cannot be undone byte-for-byte from the artifact, it does not ship.

#### The subsidiary triad

We are unambiguously state-handling: we read `git ls-files`, write to `~/.local/bin`, own a launchd
plist, and drive tmux panes through `ntm`. The exemption does not apply.

**`ompo validate <thing>`** — pure read, zero side effects.
*Purpose:* verify a config, a dispatch packet, a plist, or an install target *before* anything acts
on it, so a bad input is rejected at the boundary rather than half-applied downstream.
*Exit:* `0` valid; `74` validate failure (distinct from `1` so a caller can branch on "your input
is wrong" versus "the system is unhealthy"); `2` usage.
*Envelope:* on reject, `{status:"REJECT", reason, expected, observed, observed_length}` — the
observed field is mandatory, because a rejection without the observed value is undebuggable.
*Negative pattern it refuses:* validate MUST NOT touch the filesystem outside reads, and MUST NOT
be satisfiable by a mutation. A "validate" that fixes as it goes is a repair with a lying name.

**`ompo audit [--since <ts>]`** — append-only ledger of every mutation with provenance.
*Purpose:* answer "what did this tool change on this machine, when, and under whose authority"
without reading the tool's source.
*Exit:* `0`; `1` ledger unreadable or corrupt.
*Envelope:* rows carry `{ts, actor, verb, idempotency_key, touched_paths[], receipt_path,
post_check, result}`.
*Negative pattern it refuses:* the ledger is append-only and schema-versioned. A repair that
mutates without appending an audit row is a bug the mutate-auditor test fails the build on. An
ambiguous audit read refuses the mutation rather than proceeding blind.

**`ompo why <id>`** — provenance trace for one object.
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
author in the room. Under the umbrella, every adapter is reachable as `ompo doctor <adapter>`,
`ompo help <adapter>` names the command, and `ompo doctor capabilities --json` enumerates every
probe id — so ADDRESSABLE is discharged by the single umbrella CLI shape, rather than by separate
implementations that can each drift independently. A capabilities snapshot is checked in as a
golden artifact; drift between the declared probe list and the implemented one fails CI.

#### Upstream-report

We wrap `omp`, `ntm`, `br`, `bv`, `git`, `cargo`, and `tmux` (versions in the surface census
section). When an adapter probe fails on the *substrate* side, the envelope carries
`class:"upstream_substrate_issue"` and `upstream_owner:"<vendor>"`, and
`ompo upstream-report <adapter>` drafts the issue. Without this, every upstream bug is silently
absorbed as our bug and we lose the forcing function to file it.

NO-CLAIM: §2 specifies a surface. It does not claim any of these commands exist, and it does not
specify the internal probe list — that is the gate section's job.

### 3. Exit-code dictionary

PROJECTED as a shipped contract; the rows marked MEASURED are already emitted by code at HEAD.

| Code | Name | Meaning | Caller should |
|---:|---|---|---|
| 0 | `OK` | Work performed, all green. Never emitted by a command that declined to run. | Proceed |
| 1 | `FINDINGS` | Work performed, at least one FAIL. MEASURED: `installer/src/main.rs:93`. | Read `data.probes[]` |
| 2 | `USAGE` | Malformed invocation. MEASURED: `main.rs:41` and the `CONFIG_ERROR` envelope. | Fix the command line |
| 2 | `UNKNOWN` (envelope) | MEASURED: the inventory map exits 2 carrying `"status":"UNKNOWN"` — a probe ran but could not reach a verdict. | Treat as not-green |
| 3 | `CRITICAL` / `NO_INPUT` | Prerequisite absent: no git HEAD, no build output. MEASURED: `main.rs:61`, `main.rs:150`. | Fix environment |
| 5 | `CONCURRENCY_LOST` | Another mutation holds the lock. | Retry later |
| 70 | `ADVISORY` | Non-blocking finding. | Log |
| 71 | `SYSTEM_ERROR` | Our bug, not the user's. | File a bead |
| 74 | `VALIDATE_FAILURE` | Input is invalid; the system is fine. | Fix the input |
| 75 | `REFUSED` | The command declined to run. MEASURED: `main.rs:107` returns 75 when the build fence blocks install. | **Not a result** |
| 103 | `REFUSED_UPSTREAM` | An upstream guard declined. MEASURED behaviour: the RCH / mint-floor guard exits 103 with `0 passed / 0 failed`. | **Not a result** |

The two refusal rows carry the sharpest operational lesson in this document. **`exit 103` with
`0 passed / 0 failed` is a refusal, not a test result.** Zero failures did not happen because the
code is good; zero failures happened because zero tests ran. Reading that as green is precisely the
error our async contract names as *"a timeout is not a verdict"*: the absence of a negative signal
from a process that never produced a signal is not evidence. Every refusal code therefore gets its
own `status:"REFUSED"` value in the envelope, distinct from `OK` and from `DOWN`, and the CI
aggregator treats `REFUSED` as blocking rather than passing.

NO-CLAIM: this table does not claim the codes are currently uniform across all 21 binary targets. At
HEAD, `installer` uses 1/2/3/75 and `omp-inventory-map` uses 2; the remaining 20 are unaudited against
this table.

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
`identity: consistent=N drifted=M foreign=K expected=E workspace_targets=21 probed=P` — six named integers whose
relationship a reader can check, not a bare `2/2`. MEASURED defect this rule exists to prevent:
`crates/installer/src/main.rs:68` and `:87` (§1). The exclusion logic decremented one variable
while the message printed another, and the resulting `2/0` was arithmetically impossible but
visually plausible. A ratio is only verifiable if both terms are separately named and separately
sourced.

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
XDG paths, PATH setup, `trap cleanup EXIT`, a final per-component status summary, and printed
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

**The self-test exercises the install; it does not certify it.** `ompo doctor --json` immediately
post-install is the acceptance criterion, not "the files landed." The install is accepted when the
four-way identity check reports `consistent=N drifted=0 foreign=K expected_set=N missing=0 probed=P` with every term
printed, and `ompo health` returns 0. That raises the floor from "bytes were copied" to "the copied
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
| median time to first use | ≤ 10 minutes | timestamps from verified bootstrap start to first successful `ompo doctor` and one accepted adapter invocation |
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
**Disagreement with the brief, stated plainly.** The brief §3.7 records the mirror at
`/Volumes/ZestData/dicklesworthstone-mirror` as **216 repos** — a figure since retired in favour of
**210** git work-trees, the only count that counts repositories. I ran `ls $M | wc -l` and got
**218**. The difference is almost certainly definitional rather than substantive: the listing
includes entries that are not live repos, e.g. `ntm.corrupt-20260819`, which appears alongside
`ntm` (`ls $M | grep -i 'tmux\|ntm'` → `ntm`, `ntm.corrupt-20260819`, `useful_tmux_commands`).
Nothing in this section depends on which figure is right, and I have not resolved it — I record the
disagreement rather than quietly adopting either number, because an unreconciled count in two
sections of the same document is exactly the "unstated denominator" defect rule 4 forbids. The
orchestrator should pick one definition (`ls` entries versus git work-trees) and state it once.

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
This is a place where our aggregator shape is genuinely different: the manifest exposes seven runtime
adapters, not seven binaries; each adapter maps to one or more target rows and is independently
refused or reported as DOWN; the refusal is a first-class envelope status. We will design it ourselves and be explicit that it is not borrowed.

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

NO-CLAIM: these four are constraints this section proposes and writes down. They have not been
ratified by a gate, and none of them is enforced by code at HEAD `fb89714`.

---

**Section NO-CLAIM.** This section specifies distribution, identity, and the CLI contract. It does
not specify the probe list (gates section), the orchestration semantics (crate specs), or the
milestone at which each surface lands (milestones section). Every PROJECTED item is unbuilt at
HEAD `fb89714`; the only MEASURED install-adjacent code in the tree is `crates/installer`, which
covers 3 installer entries against 21 workspace targets, one of which is foreign and not built here.
