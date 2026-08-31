# docs/ — where NEW documents live

## Why the root files did not move

A hygiene pass proposed relocating `WAVE.md`, `ASUPERSYNC-CONFORMANCE.md`, `OMP-SURFACE-MAP.toml`
and `test-j58-clean.rs` into subdirectories. **The pre-delete citation check refused it**, and the
measurement is the reason:

| path | cited in beads | of which **closed** |
|---|---:|---:|
| `AGENTS.md` | 23 | **12** |
| `WAVE.md` | 8 | **6** |
| `OMP-SURFACE-MAP.toml` | 7 | 1 |
| `README.md` | 3 | 1 |
| `test-j58-clean.rs` | 2 | **2** |

**Twenty-two closed beads cite root paths.** A closed bead's evidence is a *live dependency on the
filesystem*, not a historical note — moving the file invalidates the citation silently. That exact
failure is already on the record as `cp-rjuzj`: a port deleted four scripts, every closed bead
citing them was silently invalidated, and it surfaced hours later as a gate refusing every dispatch
with everything downstream unrun.

So the rule for this repo is **additive structure only**:

- `AGENTS.md`, `CLAUDE.md`, `README.md` stay at root — agents auto-load the first two and GitHub
  renders the third. Moving them breaks tooling as well as citations.
- `WAVE.md`, `ASUPERSYNC-CONFORMANCE.md`, `OMP-SURFACE-MAP.toml` stay at root — heavily cited.
- `test-j58-clean.rs` stays at root — it is the gate's known-good specimen and both citing beads
  are closed. It is one line (`// clean`) and relocating it costs more than it saves.
- **New** documents land here in `docs/`.

## What lives here

| file | what it is |
|---|---|
| `PLAN.md` | the single plan document — reviewed in rounds, graded to an investor bar, materialized into the bead DAG |

## The relocation debt, stated rather than hidden

Root-level sprawl is real: 10 tracked files, half of them markdown, no hierarchy. This directory
does not fix that — it stops it growing. Relocating the cited files becomes cheap only once a
`git mv`-aware citation rewriter exists, or once the citing beads are old enough that their
evidence is no longer load-bearing. Neither is true today, and pretending otherwise would trade a
real evidence chain for a tidier tree.
