# Asupersync Process Grade

Bead: `omp-orchestrator-doc-grade`

## Purpose

This contract defines how our documentation is graded against asupersync's, the seven substance
markers the grade measures, the pass bar for a single document, and the command that re-derives
every figure. It answers Josh, 2026-09-01: *"we need to grade our docs against his — what
substance does he have — how do we meet his quality on each doc."*

## Contract Artifacts

1. Canonical artifact: this file's §Grade table, re-derived at every phase boundary
2. Grade runner: `docs/contracts/asupersync_process_grade.md` §Validation (below)
3. Corpus root: `/Volumes/ZestData/dicklesworthstone-mirror/asupersync`

## The substance he has — measured across all 97 `*contract*.md`

| ID | marker | his adoption | ours (59 docs) |
|---|---|---:|---:|
| SM-VALIDATION | a Validation section | 43% | 61% |
| SM-COMMAND | a runnable fenced bash block | 62% | 69% |
| SM-IDS | at least 5 stable IDs | 50% | 52% |
| SM-PURPOSE | a Purpose paragraph | 47% | 61% |
| SM-BEAD | a Bead line binding the doc to the DAG | 40% | 61% |
| SM-XREF | a Cross-References list of paths | 38% | 61% |
| SM-ARTIFACTS | a Contract Artifacts triple | 26% | 38% |

**Size:** corpus contracts median **7,132 bytes** (min 2,177, max 27,727). The current validation
scope is 59 docs with median **11,701 bytes** and 6 over 50 KB; the runner below is the authority.

**Current measurement:** the runner reports every marker above for the current docs tree. Earlier
21-document and zero-percent values are historical snapshots, not current claims.

## Stable ID vocabulary

The pass bar uses these additional stable IDs for the boundaries this document itself enforces:

| ID | refusal boundary |
|---|---|
| DPB-COMMAND | the command block is missing or not pasteable |
| DPB-NON-COVERAGE | the document hides an excluded claim without naming it |
| DPB-STATUS-BOUNDARY | PROJECTED, EXISTS, and UNPROVEN are collapsed |
| DPB-OWNER | a failing acceptance has no accountable owner |
| DPB-REPRODUCIBLE | a cited result has no command and source scope |
| DPB-NO-EMPTY | an empty scan is reported as a successful grade |
| DPB-DOES-NOT-MEAN | the contract lacks the negative interpretation boundary |
| DPB-DOCUMENT | the file path or document kind is not the stated contract scope |
| DPB-SCOPE | the scanner's input universe is narrower than the claim |
| DPB-GENERATED | a derived artifact is edited without its source/assembly contract |
| DPB-FRESHNESS | a cited result has no revision or capture boundary |
| DPB-EVIDENCE-SOURCE | a claimed result lacks an attributable writer and reader |



## The anatomy, quoted

`docs/failure_domain_contract.md` — **2,885 bytes, 69 lines**, and it is the template:

```
# Failure Domain Compiler Contract      <- names the ONE concern
Bead: asupersync-1508v.9.5              <- SM-BEAD: bound to the DAG
## Purpose                              <- SM-PURPOSE: one dense paragraph, no preamble
## Contract Artifacts                   <- SM-ARTIFACTS: 1 canonical JSON, 2 smoke runner,
                                        <-   3 invariant suite (tests/failure_domain_contract.rs)
## Failure Domain Model                 <- a TABLE of enum values, each with a stable ID
### Domain Properties                   <- FDP-BOUNDARY-EXPLICIT, FDP-UNIQUE-MEMBERSHIP, ...
## Restart Topology                     <- TABLE: topology x domain type
### Hooks                               <- TABLE: hook x phase x purpose
## Recovery Authority Rules             <- RA-NARROW-ON-CRASH, RA-NO-AMBIENT-DURING-RECOVERY, ...
## Validation                           <- SM-VALIDATION: ONE copy-pasteable command
## Cross-References                     <- SM-XREF: paths to artifacts AND src/*.rs
```

Seventeen stable IDs in 69 lines. **Zero narrative.** No "why this matters", no hedging, no
history. Every line is a normative statement, a table row, a path, or a command.

## What the measurements mean now

The validation runner below is the authority for current scope and percentages. At the current
run it scanned 59 documents in this repository: the median is 11,701 bytes and 6 documents exceed
50 KB. Earlier 21-document, 0-percent, and 25–120 KB statements are historical snapshots. The
corpus comparison remains a snapshot of the daily mirror; it is not a product readiness score.

The seven marker rows are structural obligations, not quality claims. A current document may meet
the marker while its command is weak, its cited artifact is stale, or its prose is substantively
wrong. That is why the runner reports scope and the contract carries explicit non-coverage below.

The useful adoption rule is narrow: every contract must carry a purpose, a bead binding, a
canonical artifact/runner/invariant-suite triple, a pasteable validation command, stable IDs, and
cross-references. The runner refuses an empty input set and exposes the current denominator rather
than preserving a remembered percentage.

## The pass bar for a single document

A document in `docs/contracts/`, `docs/policies/`, or `docs/schemas/` PASSES when:

- **DPB-SIZE** ≤ 25 KB. Past that it splits, and the split is a finding.
- **DPB-PURPOSE** one `## Purpose` paragraph naming what the document defines. No preamble.
- **DPB-BEAD** a `Bead:` line. If no bead exists, file one first.
- **DPB-VALIDATION** one `## Validation` block whose command a reader can paste and run.
- **DPB-XREF** a `## Cross-References` list of real paths — artifacts and source files.
- **DPB-IDS** ≥5 stable IDs when the document defines a vocabulary, states properties, or lists
  rules. A narrative document is exempt and must say why.
- **DPB-ARTIFACTS** for a contract: the triple — canonical artifact, runner, invariant suite. A
  contract naming no invariant suite is a description, not a contract.
- **DPB-NO-NARRATIVE** no section whose only function is motivation. The Purpose carries it.

The four Phase 0 contracts in flight are the first documents held to this bar.

## Validation

```bash
cd /Users/josh/Developer/omp-orchestrator && python3 - <<'PY'
import pathlib,re
M=pathlib.Path('/Volumes/ZestData/dicklesworthstone-mirror/asupersync/docs')
def score(t):
    return dict(
      cmd='```bash' in t or '```sh' in t,
      ids=len(set(re.findall(r'\b([A-Z]{2,6}-[A-Z0-9]{2,}(?:-[A-Z0-9]+)*)\b', t)))>=5,
      purpose=bool(re.search(r'^##+ *Purpose', t, re.M|re.I)),
      validation=bool(re.search(r'^##+ *Validation', t, re.M|re.I)),
      bead=bool(re.search(r'^Bead:', t, re.M)),
      xref=bool(re.search(r'^##+ *Cross.?Ref', t, re.M|re.I)),
      artifacts=bool(re.search(r'^##+ *(Contract )?Artifacts', t, re.M|re.I)))
for label, docs in (('HIS', sorted(M.glob('*contract*.md'))),
                    ('OURS', sorted(pathlib.Path('docs').rglob('*.md')))):
    n=len(docs); agg={}
    for p in docs:
        for k,v in score(p.read_text(errors='replace')).items():
            agg[k]=agg.get(k,0)+bool(v)
    sizes=sorted(len(p.read_text(errors='replace')) for p in docs)
    print(label, n, 'docs, median', sizes[n//2], 'bytes',
          {k:f'{100*v//n}%' for k,v in sorted(agg.items())})
PY
```

Expected on a passing run: OURS meets or exceeds HIS on every marker. Anything at 0% is a
category we have not started.

## Cross-References

- `docs/plans/plan_to_write_the_document_corpus.md` — the 78-document manifest this grades
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — Phase 0, first docs held to the bar
- `docs/plan/round24-L1-dag.md` — the corpus measurements and the retracted claim
- `/Volumes/ZestData/dicklesworthstone-mirror/asupersync/docs/failure_domain_contract.md` — template
- `/Volumes/ZestData/dicklesworthstone-mirror/asupersync/docs/crash_only_region_contract.md`

## Non-Coverage

- This document checks structural markers, size, and scan scope. It does not prove that a named
  command returns the right verdict or that a cited artifact contains the claimed truth.
- The corpus is one daily mirror snapshot. It does not establish an optimum, transferability, or
  runtime correctness of this repository.
- No marker substitutes for semantic review, external validation, or a product-level shipping
  receipt. Those remain separate claims with separate oracles.

## NO-CLAIM

The seven markers are **structural**, and structure is not substance: a document can carry all
seven and still be wrong. A `## Validation` block whose command does not actually test the
document's claims is worse than none, because the grade will read as passing — the same defect as
a `*_receipt.md` with no run behind it.

Two of my own probes on this grade were badly shaped and both were corrected in place: searching
four filenames at depth 2 produced "the corpus has zero planning documents" (retracted, 84833e0),
and a bold-only ID regex produced "4% have stable IDs" when the real figure is 50%. Every figure
here is re-derivable by the §Validation command; prefer running it to citing this table.

The corpus is one author's practice read through a daily mirror snapshot. It is the strongest
local evidence of a proven approach, not proof of an optimum.
