# Census Archive — Instrument Contracts

> **ARCHIVED FIGURES ARE NOT CITABLE.** This file is a historical receipts archive, not current ground truth. Re-run the producing command from the source section before use.
>
> Archive snapshot: **2026-09-07**. The source excerpt below is verbatim; its figures are not remeasured or corrected here.
>
> Original measurement dates present in this excerpt: not stated in the excerpt.
>
## Instrument contracts: what each surface ACTUALLY returns

Measured 2026-09-05/06. Every row cost someone real work in one session; five of the seven were
the conductor's own faults, corrected only because a second instrument disagreed. These are not
style notes — each is a case where the OBVIOUS read of a tool reports the OPPOSITE of the truth.

|surface|the obvious read|what it actually does|the correct test|
|---|---|---|---|
|`am file_reservations reserve`|nonzero on refusal|**exits 0** with `granted: []` and `conflicts` naming the holder|`len(granted) > 0`|
|`jq 'if .granted then'`|false on `[]`|**an empty array is TRUTHY in jq**, so a refusal prints "granted"|test the length, never the array|
|`inbox-monitor --watch`|zero on success|**exits 12** on a settled watch — the nonzero IS the finding|read `verdict`, not the exit code|
|`br show <short-id>`|exact match|**suffix-resolves** — `…-ipg.18` returns `…-omp-coverage-mission-ipg.18`|read `.id` back before comparing surfaces|
|`br list --json`|carries comments|**no `comments` key at all**; a classifier keyed on it reports zero for every row|read `.beads/issues.jsonl`; control on a bead you know|
|`$?` after a pipe|the subject's status|the **pipeline's last** command — `cmd \| head` reports `head`|redirect to a file, capture separately|
|`cargo test -p X`|the crate's tests|**that target only** — integration targets are separate, so a count can be honestly low|name the target, or sum them|

**The unifying fault: the exit code is asked to carry a status it cannot express.** Two rows above
fail in OPPOSITE directions — `reserve` exits 0 on refusal, `inbox-monitor` exits nonzero on success
— so **no single exit-code convention is safe across our own tooling.** Read the payload.

### The failure this prevents is not a wrong number, it is a confident wrong CAUSE

In every instance the instrument produced a plausible story and the story was believed:

- A `jq` truthiness bug printed `RESERVED for pane1` while `CloudyGrove` held the lease. **One step
  from two agents editing one file.** What stopped it was an unexplained exit code from the
  surrounding pipeline, not the probe.
- `br` suffix-resolution vs Python exact-matching made six beads look absent from the JSONL. The
  conclusion published was **"the JSONL is stale"**, followed by a pointless flush that correctly
  answered *"Nothing to export."* The JSONL was never stale.
- `br list --json`'s missing `comments` key made a classifier report **0 reapable beads out of 119**.
  The positive control that caught it: `eg0m` has **17** comments. A reader returning 0 for `eg0m`
  is broken; the data is not empty.
- `$?` after a pipe reported a gate exiting **0** when its true exit was **1** with 37 real rows.
  A correctly firing gate was one sentence from being graded as non-firing.

### The offload lane is not the only lane, and a timeout is not a verdict

`rch`'s workers are **Linux x86_64**. That is a property of the OFFLOAD FLEET, not an absence of a
build lane. `RCH_CARGO_WRAPPER_BYPASS=1 cargo …` is the sanctioned local path, it produces
`Mach-O 64-bit executable arm64`, and it built and installed five codesigned binaries in one
session. Measured contrast: `path-literal-guard` returns `10 passed` in **0.00s** locally where the
same suite hit an RCH `queue_timeout` at **300s**.

This stale premise cost real work **twice in one session**: two panes refused to install, believing
"no Mach-O artifact lane" existed, and a grading batch labelled **six** offload timeouts as `GAP`.

**A grading verdict vocabulary needs four values, not three: PASS, GAP, STALE, and UNKNOWN.**
`GAP` means the work fails its acceptance. **"I could not execute the check" is UNKNOWN and says
nothing about the work.** A batch reporting `PASSED=0 GAPPED=16` where six checks never ran does not
describe a broken codebase — it describes a saturated queue, and it hands the next reader sixteen
verdicts of which six were never measured.

### Agent NAME is not an identity

Two panes signed ACKs as `WildStone` simultaneously; one of them was `RubyGate` in Agent Mail; the
`am` roster separated them only by **model**. `created_by` reads `josh` or `None` on the rows that
matter, so a grader-≠-author check built on it **excludes nobody while reporting success** — a
vacuous filter that routed 13 beads to their own authors. The only key that proved unique was the
**pane id parsed out of the bead's own ACK lines**, and even that is unique only per OCCUPANCY,
which is why `pane-dispatch-fence`'s `PaneIncarnation` exists.

**NO-CLAIM.** This table is a list of measured surprises, not a specification. Every row states what
was observed on one host on one date; none of them was read from the tool's source. `reserve`'s exit
code on a SUCCESSFUL grant is UNMEASURED — only the refusal case was observed — and `br`'s behaviour
when a suffix matches two beads is likewise unmeasured and must not be assumed to error.

---

