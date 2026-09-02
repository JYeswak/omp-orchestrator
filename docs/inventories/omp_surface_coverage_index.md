# OMP surface coverage — index

Bead: `omp-orchestrator-omp-coverage-mission-ipg` — the wave epic these eleven waves hang from

## Purpose

Eleven surface-coverage waves were declared as beads `ipg.1`–`ipg.11`. Ten produced a coverage
table; one never did. This index owns the roll-up — which wave owns which surfaces, where its
document is, and which declared wave has no document at all. Per-surface rows live in the sibling
documents; machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`.

Every one of these documents was Appendix B–K of `docs/plan/12-journey.md` until `50df553`. They were
extracted because a 125 KB runbook cannot show you a missing appendix, and this one is missing:
see `ipg.4` below.

## The eleven waves

`a` = not ours · `b` = reimplemented by scraping · `c` = unused capability we should adopt.

| wave | name | surfaces | classification | document |
|---|---|---:|---|---|
| `ipg.1` | PLAN | 3 | a×1 / b×1 / c×1 | [`omp_surface_coverage_ipg1.md`](omp_surface_coverage_ipg1.md) |
| `ipg.2` | BEADS | 3 | a×1 / b×2 | [`omp_surface_coverage_ipg2.md`](omp_surface_coverage_ipg2.md) |
| `ipg.3` | TRIAGE | 3 | a×3 | [`omp_surface_coverage_ipg3.md`](omp_surface_coverage_ipg3.md) |
| `ipg.4` | DISPATCH | 7 | — | **ABSENT** |
| `ipg.5` | OBSERVE | 4 | a×3 / b×1 | [`omp_surface_coverage_ipg5.md`](omp_surface_coverage_ipg5.md) |
| `ipg.6` | VERIFY | 8 | a×8 | [`omp_surface_coverage_ipg6.md`](omp_surface_coverage_ipg6.md) |
| `ipg.7` | MEMORY | 5 | a×5 | [`omp_surface_coverage_ipg7.md`](omp_surface_coverage_ipg7.md) |
| `ipg.8` | EDIT | 6 | a×6 | [`omp_surface_coverage_ipg8.md`](omp_surface_coverage_ipg8.md) |
| `ipg.9` | SECURITY | 4 | a×4 | [`omp_surface_coverage_ipg9.md`](omp_surface_coverage_ipg9.md) |
| `ipg.10` | IO | 8 | a×8 | [`omp_surface_coverage_ipg10.md`](omp_surface_coverage_ipg10.md) |
| `ipg.11` | RUNTIME | 6 | a×6 | [`omp_surface_coverage_ipg11.md`](omp_surface_coverage_ipg11.md) |

## What the extraction exposed, measured

**1 — `ipg.4` DISPATCH has a bead and no coverage document, because the bead was repurposed.**
Seven surfaces were declared for the wave — `irc`, `collab`, `jsonrpc`, `mcp`, `launch`, `exec`,
`subprocess` — and none was ever swept. The bead is not idle: three Phase 1 contracts
(`docs/contracts/subprocess_contract.md`, `cancellation_contract.md`,
`dispatch_claim_contract.md`) carry `Bead: omp-orchestrator-omp-coverage-mission-ipg.4` on line 3,
so the wave id now names contract-corpus work instead of a surface sweep. That covers three of the
seven concerns from a different direction and leaves `irc`, `collab`, `jsonrpc` and `mcp` with no
coverage row of any kind — and those four are the orchestrator's own transport surfaces. Round 13
filed `ipg.4 absent` against `12-journey` and it stayed absent, because inside a 21-heading
document an absent heading looks like nothing at all. As eleven filenames it is one `ls`.

**2 — 50 surfaces classified; 45 are (a) not ours, 4 are (b) scraped, 1 is (c) adoptable.** The
single (c) is `goals` in `ipg.1`. Nine of the ten waves report `Positive control: FAILED`; only
`ipg.11` passes, at 1 of 6. A census whose positive control fails nine times running is measuring
a real boundary, but it is also a census that has not yet found the thing it was built to find.

**3 — 12 of those 50 surfaces reached the machine ledger.** `docs/plan/OMP-COVERAGE-TABLE.jsonl`
holds 12 rows, `graded_by` naming only `ipg.8` and `ipg.11`. The other eight waves exist as prose
tables and nothing else, so no gate can read them.

**4 — the ten tables carry four different schemas for one concern.** Three waves use a 12-column
form, five collapse the eight clauses into a single `1-8 clauses` column, and two use a 14-column
form that disagrees with itself on the second clause's name (`2 forbid` vs `2 unsafe`). One
concern, four shapes, invisible while they were 600 lines apart in the same file.

## NO-CLAIM

Splitting these out changed no classification, no count and no verdict. The bytes are the
appendix bytes with a one-level heading demotion; a smaller mean document size is not evidence
that any row in them is correct. In particular this index does NOT establish that the 45 (a)
rows are rightly (a) — it establishes only that 45 rows say so, and that 38 of the 50 were never
written to a ledger anything can check.

## Cross-References

- `docs/plan/12-journey.md` — the nine-stage runbook these appendices were extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — the 12-row machine ledger
- `docs/plan/ipg10-coverage.json`, `docs/plan/ipg11-coverage.json` — per-wave capture artifacts
- `docs/plan/02-surface-census.md` — the surface census these waves refine
- `crates/no-shell-gate/tests/coverage_rows.rs` — the gate over `ipg*-coverage.json`
- `crates/omp-inventory-map/tests/coverage_mission.rs` — the gate over `OMP-COVERAGE-TABLE.jsonl`

## Validation

Three legs, each deriving both sides. Leg 1 is the absence check that motivated the split: it
prints every declared surface-coverage wave with no document, and every document with no wave.
Leg 2 diffs each document's tabulated surfaces against its bead title. Leg 3 counts ledger
coverage. An empty parse anywhere is a FAIL, not a silent pass.

```bash
cd /Users/josh/Developer/omp-orchestrator

# LEG 1 — declared waves (ipg.1-11 are the surface-coverage waves) vs documents on disk
DECL=$(jq -r 'select(.id|test("omp-coverage-mission-ipg\\.([1-9]|10|11)$"))|.id' .beads/issues.jsonl \
       | sed 's/.*ipg\.//' | sort -un)
DOCS=$(ls docs/inventories/omp_surface_coverage_ipg*.md | sed -E 's#.*ipg([0-9]+)\.md#\1#' | sort -un)
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

Expected on a healthy tree today: leg 1 prints `4` on the first line and nothing on the second;
leg 2 prints ten `PASS`; leg 3 prints `50` and `12`. The first line going empty means `ipg.4`
was finally swept — lower nothing, just delete that sentence.
