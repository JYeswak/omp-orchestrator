# OMP surface coverage — index

Bead: `omp-orchestrator-omp-coverage-mission-ipg` — the wave epic these eleven waves hang from

## Purpose

Eleven surface-coverage waves were declared as beads ipg.1–ipg.11. All eleven now have a coverage
document. This index owns the roll-up — which wave owns which surfaces, where its document is, and
which classifications were assigned. Per-surface rows live in the sibling documents;
machine-readable dispositions live in docs/plan/OMP-COVERAGE-TABLE.jsonl.

Every one of these documents was Appendix B–K of docs/plan/12-journey.md until 50df553. They were
extracted because a 125 KB runbook cannot show a missing appendix when the classification is buried
inside it.

## The eleven waves

`a` = not ours · `b` = reimplemented by scraping · `c` = unused capability we should adopt.

| wave | name | surfaces | classification | document |
|---|---|---:|---|---|
| `ipg.1` | PLAN | 3 | a×1 / b×1 / c×1 | [`omp_surface_coverage_ipg1.md`](omp_surface_coverage_ipg1.md) |
| `ipg.2` | BEADS | 3 | a×1 / b×2 | [`omp_surface_coverage_ipg2.md`](omp_surface_coverage_ipg2.md) |
| `ipg.3` | TRIAGE | 3 | a×3 | [`omp_surface_coverage_ipg3.md`](omp_surface_coverage_ipg3.md) |
| `ipg.4` | DISPATCH | 7 | b×5 / c×2 | [`omp_surface_coverage_ipg4.md`](omp_surface_coverage_ipg4.md) |
| `ipg.5` | OBSERVE | 4 | a×3 / b×1 | [`omp_surface_coverage_ipg5.md`](omp_surface_coverage_ipg5.md) |
| `ipg.6` | VERIFY | 8 | a×8 | [`omp_surface_coverage_ipg6.md`](omp_surface_coverage_ipg6.md) |
| `ipg.7` | MEMORY | 5 | a×5 | [`omp_surface_coverage_ipg7.md`](omp_surface_coverage_ipg7.md) |
| `ipg.8` | EDIT | 6 | a×6 | [`omp_surface_coverage_ipg8.md`](omp_surface_coverage_ipg8.md) |
| `ipg.9` | SECURITY | 4 | a×4 | [`omp_surface_coverage_ipg9.md`](omp_surface_coverage_ipg9.md) |
| `ipg.10` | IO | 8 | a×8 | [`omp_surface_coverage_ipg10.md`](omp_surface_coverage_ipg10.md) |
| `ipg.11` | RUNTIME | 6 | a×6 | [`omp_surface_coverage_ipg11.md`](omp_surface_coverage_ipg11.md) |

## What the extraction exposed, measured

**1 — ipg.4 DISPATCH is now covered by a dedicated seven-row inventory.**
The wave contains irc, collab, jsonrpc, mcp, launch, exec, and subprocess. The document
omp_surface_coverage_ipg4.md records all seven rows against the eight-clause per-crate contract:
irc and collab are (c) unused capabilities; jsonrpc, mcp, launch, exec, and subprocess are (b)
reimplemented-by-scraping or local boundary alternatives, each naming the existing OMP alternative.
The subprocess row is the positive control and reports FULLY COVERED at the local contract boundary;
that does not claim OMP type adoption.

The prior absence was real historical evidence: three Phase 1 contracts carried the ipg.4 bead id,
so the wave id had been reused for contract-corpus work while the seven surface rows were missing.
That gap is closed in the inventory document and machine ledger; the classification is a mapping,
not an adoption claim.

**2 — 57 surfaces classified; 45 are (a) not ours, 9 are (b) scraped, 3 are (c) unused capabilities.** The
three (c) rows are goals from ipg.1 plus irc and collab from ipg.4. Nine of the eleven waves report
Positive control: FAILED; ipg.4 and ipg.11 report a local-boundary positive control. A census whose
positive control fails on an OMP type plane can still describe a real ownership boundary, but the
local positive control must remain explicit.

**3 — 19 of those 57 surfaces reached the machine ledger.** docs/plan/OMP-COVERAGE-TABLE.jsonl now
holds 19 rows, with ipg.4 added to the graded wave set. The remaining wave documents are still
prose-plus-validation artifacts unless their rows are added to the machine ledger.

**4 — the eleven tables carry multiple schemas for one concern.** The wave documents retain their
human-readable table shapes, while the machine ledger carries the normalized per-surface fields.
This distinction is deliberate: a table can be readable without being machine-checkable.

## NO-CLAIM

Adding ipg.4 changed the classified-surface and ledger counts, but it did not adopt an OMP type or
prove any local alternative correct. The index records what the wave documents say; it does not
establish that the 45 (a), 9 (b), or 3 (c) classifications are themselves correct. The map remains
an inventory and a decision surface, not a runtime guarantee.

## Cross-References

- docs/plan/12-journey.md — the nine-stage runbook these appendices were extracted from
- docs/plan/OMP-COVERAGE-TABLE.jsonl — the 19-row machine ledger
- docs/plan/ipg10-coverage.json, docs/plan/ipg11-coverage.json — per-wave capture artifacts
- docs/plan/02-surface-census.md — the surface census these waves refine
- crates/no-shell-gate/tests/coverage_rows.rs — the gate over ipg*-coverage.json
- crates/omp-inventory-map/tests/coverage_mission.rs — the gate over OMP-COVERAGE-TABLE.jsonl


## Validation

Three legs, each deriving both sides. Leg 1 is the absence check that motivated the split: it
prints every declared surface-coverage wave with no document, and every document with no wave.
Leg 2 diffs each document's tabulated surfaces against its bead title. Leg 3 counts ledger
coverage. An empty parse anywhere is a FAIL, not a silent pass.

```bash
cd /Users/josh/Developer/omp-orchestrator

# LEG 1 — declared waves (ipg.1-11 are the surface-coverage waves) vs documents on disk
DECL=$(jq -r 'select(.id|test("omp-coverage-mission-ipg\\.([1-9]|10|11)$"))|.id' .beads/issues.jsonl \
       | sed 's/.*ipg\.//' | sort -u)
DOCS=$(ls docs/inventories/omp_surface_coverage_ipg*.md | sed -E 's#.*ipg([0-9]+)\.md#\1#' | sort -u)
test -n "$DECL" -a -n "$DOCS" || { echo "FAIL leg1 vacuous"; exit 1; }
echo "waves declared with NO document: $(comm -23 <(echo "$DECL") <(echo "$DOCS") | tr '\n' ' ')"
echo "documents with NO declared wave: $(comm -13 <(echo "$DECL") <(echo "$DOCS") | tr '\n' ' ')"

# LEG 2 — every document's surface set against its bead title
for W in $DOCS; do
  D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg$W.md" \
      | grep -vE '^(surface|-+)$' | sort -u)
  B=$(jq -r --arg i "omp-orchestrator-omp-coverage-mission-ipg.$W" 'select(.id==$i)|.title' .beads/issues.jsonl \
      | head -1 | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u)
  if [ -n "$D" ] && [ -n "$B" ] && diff <(echo "$D") <(echo "$B") >/dev/null
  then echo "PASS ipg.$W"; else echo "FAIL ipg.$W"; fi
done

# LEG 3 — surfaces tabulated in prose vs rows in the machine ledger
P=$(cat docs/inventories/omp_surface_coverage_ipg*.md \
    | sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' | grep -vE '^(surface|-+)$' | sort -u | wc -l)
L=$(wc -l < docs/plan/OMP-COVERAGE-TABLE.jsonl)
echo "surfaces tabulated: $P   rows in OMP-COVERAGE-TABLE.jsonl: $L"
```

Expected on a healthy tree today: leg 1 prints an empty missing-document list and an empty
orphan-document list; leg 2 prints eleven PASS rows; leg 3 prints 57 tabulated surfaces and 19
machine-ledger rows. A non-empty absence list or a zero count is an error, not a quiet success.
