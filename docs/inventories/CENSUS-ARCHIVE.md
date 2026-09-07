# Census Archive

> **ARCHIVED FIGURES ARE NOT CITABLE.** This index and its parts are historical receipts, not current ground truth. Re-run the producing command from each source excerpt before using any figure.
>
> Archive snapshot: **2026-09-07**. The body excerpts are verbatim from the pre-rulebook AGENTS.md snapshot; no census was remeasured for this move.

## Anchor map

| Source section | Archive part | Original measurement dates present |
|---|---|---|
| The fifth rule: the crates exist to orchestrate OMP, and today they scrape it | [CENSUS-ARCHIVE-OMP.md#the-fifth-rule-the-crates-exist-to-orchestrate-omp-and-today-they-scrape-it](CENSUS-ARCHIVE-OMP.md#the-fifth-rule-the-crates-exist-to-orchestrate-omp-and-today-they-scrape-it) |  |
| OMP lifecycles — what they are and where to find them | [CENSUS-ARCHIVE-OMP.md#omp-lifecycles--what-they-are-and-where-to-find-them](CENSUS-ARCHIVE-OMP.md#omp-lifecycles--what-they-are-and-where-to-find-them) |  |
| The crate extraction target list — what each one is, and which repository it actually is in | [CENSUS-ARCHIVE-CRATES.md#the-crate-extraction-target-list--what-each-one-is-and-which-repository-it-is-actually-in](CENSUS-ARCHIVE-CRATES.md#the-crate-extraction-target-list--what-each-one-is-and-which-repository-it-is-actually-in) |  |
| Instrument contracts: what each surface ACTUALLY returns | [CENSUS-ARCHIVE-INSTRUMENTS.md#instrument-contracts-what-each-surface-actually-returns](CENSUS-ARCHIVE-INSTRUMENTS.md#instrument-contracts-what-each-surface-actually-returns) |  |

## Known stale figures

- **8h mode-only dirty census:** the historical `81 mode-only dirty files` claim is stale after the 2026-09-07 cleanup; the measured post-cleanup state is `0` mode-only files. Do not cite either without the producing command.
- The historical package/inventory counts, OMP surface counts, extraction LOC/test totals, unsafe-count ratios, and installed-artifact identity figures listed in the callback are suspected stale and remain explicitly non-citable.
- No other archived figure was corrected in place.

## Scope decision

The OMP surface section remains whole in its archive part and should remain whole in AGENTS.md: it is the repository's standing product identity and adoption gap, not merely a historical measurement.
