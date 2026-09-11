#![forbid(unsafe_code)]
//! BEAD SHAPE GATE — the label index must stay an index.
//!
//! # What went wrong, measured 2026-09-01
//!
//! `.beads/issues.jsonl` carried **119 distinct labels over 142 beads — one label
//! per 1.2 beads**, with **75 of the 119 used exactly once** and 29 beads carrying
//! no label at all. The reference corpus (166,757 beads / 150 repos) runs 1,314
//! labels over 166,757 beads: **one per 127**.
//!
//! At one-per-1.2 a label is an adjective, not an index. `bv -l <label>
//! --robot-insights` returns one bead or zero, and the `bv --severity warning`
//! alert surface has nothing to group by — which disables the *navigation* half of
//! the whole method while every individual bead still looks well-formed.
//!
//! Consolidation to an 18-label controlled taxonomy fixed it once. This gate is
//! what stops it growing back, because it grows back one well-intentioned
//! one-off label at a time and no single addition ever looks like the problem.
//!
//! # ⛔ RE-SHAPED 2026-09-11 — from LEVEL to DELTA. Read this before editing a number.
//!
//! The gate had four red legs, all of them counting a LEVEL over the whole live
//! board. Measured here, independently, before changing anything:
//!
//! | leg | measured | ceiling |
//! |---|---|---|
//! | off-taxonomy beads | 672 of 1092 live (worktree), 692 (earlier mirror) | 526 |
//! | distinct labels in use | 368 | 267 |
//! | singleton labels | 196 | 127 |
//! | unused allowlist entries | 1 (`decision`) | 0 |
//!
//! Both obvious repairs were measured and both are DISPROVEN:
//!
//! 1. **Re-record the ceiling.** 526 was re-recorded on 2026-09-05 and breached
//!    six days later. A level ceiling over a board that grows is a countdown, not
//!    a ratchet: the number rises when the fleet FILES WORK, so the gate reddens
//!    for productivity and goes green for idleness.
//! 2. **Expand the taxonomy.** Admitting the entire structural family (`s*`, `l*`,
//!    `phase-*` — 434 label uses on workable beads) moves off-taxonomy workable
//!    beads from **531 to 530**. ONE bead. A bead offends on ANY off-taxonomy
//!    label and offenders carry several, so the bead-level metric is SATURATED: it
//!    cannot respond to the very remediation it demands. A metric that will not
//!    move under the correct fix is a wall, not a ratchet.
//!
//! So this gate now polices the **DELTA**, not the level, on evidence that the
//! delta is already clean:
//!
//! | creation day | workable beads | off-vocabulary |
//! |---|---|---|
//! | 2026-09-03 | 473 | 406 |
//! | 2026-09-07 | 65 | 63 |
//! | 2026-09-10 | 7 | **0** |
//! | 2026-09-11 | 61 | **0** |
//!
//! Every workable bead created on or after **2026-09-10** already carries only
//! in-vocabulary labels. The delta rule below is therefore not an amnesty for new
//! work — it is the fleet's CURRENT behaviour, pinned so it cannot regress. The
//! 548 pre-cutoff offenders are grandfathered by a **dated, identity-pinned,
//! expiring** register that cannot grow, plus a down-only count.
//!
//! # Contract
//!
//! - Four registers live in **`.beads/LABEL-TAXONOMY.md`**, each a fenced block.
//!   This file does **not** contain a second copy of any of them. Two copies of an
//!   allowlist is two allowlists, and they drift.
//!   - ```` ```taxonomy ```` — the 18-term controlled topical vocabulary.
//!   - ```` ```axes ```` — closed, enumerated plan-axis vocabularies (`s`, `l`,
//!     `phase`). An axis is a partition declared a priori, so a member with one
//!     carrier is not the one-off defect this gate exists to stop; an axis that
//!     indexes nothing IS, and is refused below.
//!   - ```` ```amnesty-labels ```` — the off-vocabulary labels observed at the
//!     2026-09-11 census. DOWN only, expires, and no label may enter it later.
//!   - ```` ```reserved ```` — allowlist entries with no carrier yet, named
//!     individually with a death date rather than tolerated by a count.
//! - **Delta, zero tolerance:** a workable bead created on/after `DELTA_CUTOFF`
//!   must carry only `taxonomy ∪ axes` labels. No count, no ceiling, no slack.
//! - **Vocabulary, identity-pinned:** every label in use on any live bead must be
//!   in `taxonomy ∪ axes ∪ amnesty-labels`. Pinning identity is strictly stronger
//!   than the old distinct-label COUNT: 368 ≤ 368 permits swapping a dead label
//!   for a fresh one-off; set membership does not.
//! - **Ratchets may only FALL**, and are seeded from *this gate's own scan* at the
//!   moment the census landed. Seeding a ratchet from a neighbouring measurement
//!   (a `jq` count, a sibling test) is a measured defect from this same session: a
//!   ceiling seeded at 42 while the scan counted 41 let the mutation probe PASS
//!   when it should have failed.
//! - **The amnesty dies.** Past `AMNESTY_EXPIRY` this gate fails unconditionally.
//!   Renewal is a code edit plus a ruling, not a silence.
//! - **Anti-vacuity is an ERROR, not a skip.** A missing JSONL, an unparseable
//!   one, a missing register, and a genuinely clean board are indistinguishable
//!   from a `return`. The sibling `bead_standard.rs` prints `SKIP:` and returns
//!   green in that case; this gate refuses to.
//!
//! Tombstones are excluded from every leg because `br update` refuses to mutate
//! them (`cannot update tombstone issue`), so their labels are frozen and
//! unfixable — three of them (`plan-derived`, `contract`, `convergence`) are still
//! in the JSONL for exactly that reason.
//!
//! Reads the committed JSONL rather than shelling out to `br` on purpose:
//! bead `omp-orchestrator-hermetic-detector-tests-47m` in this very tracker is a
//! defect filed against tests coupled to live tracker state. The artifact under
//! review is the tracked file, so the tracked file is what gets read.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Census of record: 2026-09-11. Ratchets seeded from THIS gate's own scan.
// ---------------------------------------------------------------------------

/// Beads created on or after this UTC date must carry only `taxonomy ∪ axes`
/// labels, with NO allowance. Seeded from this gate's own scan: workable beads
/// created 2026-09-10 (7) and 2026-09-11 (61) carry **zero** off-vocabulary
/// labels, so the cutoff pins behaviour the fleet already has, two days back of
/// margin included. Moving it FORWARD is amnesty for new work and is refused on
/// review; moving it BACKWARD is a tightening and needs only the scan to stay
/// green.
const DELTA_CUTOFF: &str = "2026-09-10";

/// Off-vocabulary labels carried by live beads at the census, pinned by NAME in
/// the `amnesty-labels` register. 332, measured over the union of HEAD and the
/// working tree so a dirty mirror cannot make the gate disagree with CI.
/// DOWN only. Dies at `AMNESTY_EXPIRY`.
const AMNESTY_LABEL_CEILING: usize = 332;

/// Workable (non-tombstone, non-closed) beads carrying an off-vocabulary label.
/// Census scan: **548 at HEAD, 530 in the working tree** — the ceiling is the
/// larger because CI's subject is HEAD. DOWN only, and it falls for free as
/// beads close, so it measures remediation rather than population.
const ACTIVE_OFF_VOCABULARY_CEILING: usize = 548;

/// Re-recorded 2026-09-05: 59 unlabelled live beads (was 145). Scoped 2026-09-11
/// to beads that can still be WORKED, because a label on a closed bead has no
/// consumer. Scan: 35 at HEAD, 55 in the working tree. Dies when every workable
/// bead has a label; then LOWER. Never raise it silently.
const UNLABELLED_WORKABLE_CEILING: usize = 59;

/// 2026-12-11, as days since the UNIX epoch. Past this day EVERY leg that leans
/// on the amnesty fails, including the green ones. Three months is the whole
/// budget for draining 548 beads; it is not renewable by editing this number
/// without the ruling that justifies it.
const AMNESTY_EXPIRY_EPOCH_DAY: u64 = 20798;
const AMNESTY_EXPIRY_HUMAN: &str = "2026-12-11";

/// Past ~25 an allowlist stops being a controlled vocabulary and becomes a
/// record of everything anyone ever wanted to say.
const TAXONOMY_MAX: usize = 25;
/// An axis is a declared partition. Three is already generous; a fourth is a
/// taxonomy decision, not a bead edit.
const AXIS_MAX: usize = 3;
const AXIS_MEMBER_MAX: usize = 12;
/// An axis that indexes nothing is a smuggling route for one-off labels: two
/// members in use and eight carriers is the floor at which `bv -l` groups.
const AXIS_MIN_MEMBERS_IN_USE: usize = 2;
const AXIS_MIN_CARRIERS: usize = 8;

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<crate> has two ancestors")
        .to_path_buf()
}

fn taxonomy_doc() -> (PathBuf, String) {
    let path = repo_root().join(".beads/LABEL-TAXONOMY.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "ANTI-VACUITY: cannot read the registers at {}: {e}. Without them this gate would \
             pass everything, which is worse than failing.",
            path.display()
        )
    });
    (path, text)
}

/// Lines of one fenced ```<name> block. Panics rather than defaulting: a gate
/// that cannot find its register must not fall back to "allow everything".
fn register(name: &str) -> Vec<String> {
    let (path, text) = taxonomy_doc();
    let open = format!("```{name}");
    let mut rows = Vec::new();
    let mut inside = false;
    let mut saw_block = false;
    for line in text.lines() {
        let t = line.trim();
        if !inside {
            if t == open {
                assert!(
                    !saw_block,
                    "two ```{name} blocks in {} — two copies of a register is two registers, \
                     and they drift",
                    path.display()
                );
                inside = true;
                saw_block = true;
            }
            continue;
        }
        if t.starts_with("```") {
            inside = false;
            continue;
        }
        if t.is_empty() {
            continue;
        }
        rows.push(t.to_owned());
    }

    assert!(
        saw_block,
        "ANTI-VACUITY: {} has no ```{name} fenced block. The register moved or was renamed; \
         this gate is not silently permissive when that happens.",
        path.display()
    );
    assert!(
        !rows.is_empty(),
        "ANTI-VACUITY: the ```{name} block in {} is empty. An empty register reads like a \
         catastrophe rather than a missing list.",
        path.display()
    );
    rows
}

fn register_set(name: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for row in register(name) {
        assert!(
            set.insert(row.clone()),
            "duplicate entry `{row}` in the ```{name} register — a register is a set"
        );
    }
    set
}

/// The controlled topical vocabulary.
fn taxonomy() -> BTreeSet<String> {
    register_set("taxonomy")
}

/// Closed, enumerated plan axes: `axis: member member ...`, one axis per line.
fn axes() -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in register("axes") {
        let (axis, members) = row.split_once(':').unwrap_or_else(|| {
            panic!(
                "```axes row `{row}` is not `axis: member member ...`. An axis whose name is \
                 implicit is an axis nobody can audit."
            )
        });
        let axis = axis.trim().to_owned();
        assert!(
            !axis.is_empty(),
            "```axes row `{row}` has an empty axis name"
        );
        let mut set = BTreeSet::new();
        for m in members.split_whitespace() {
            // Membership is name-checked so an axis line cannot smuggle an
            // arbitrary one-off label in under a legitimate axis.
            let tail = m
                .strip_prefix(&axis)
                .unwrap_or_else(|| panic!("axis `{axis}` member `{m}` does not start with `{axis}`"));
            let digits = tail.strip_prefix('-').unwrap_or(tail);
            assert!(
                !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()),
                "axis `{axis}` member `{m}` is not `{axis}<n>` or `{axis}-<n>`; an axis is a \
                 numbered partition, not a second topical vocabulary"
            );
            assert!(
                set.insert(m.to_owned()),
                "duplicate member `{m}` on axis `{axis}`"
            );
        }
        assert!(
            !set.is_empty(),
            "axis `{axis}` declares no members — an empty axis allows nothing and explains nothing"
        );
        assert!(
            out.insert(axis.clone(), set).is_none(),
            "axis `{axis}` declared twice in the ```axes register"
        );
    }
    out
}

fn axis_members(axes: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    axes.values().flatten().cloned().collect()
}

/// Everything a bead created on/after `DELTA_CUTOFF` may carry.
fn vocabulary() -> BTreeSet<String> {
    let mut v = taxonomy();
    v.extend(axis_members(&axes()));
    v
}

struct Bead {
    id: String,
    status: String,
    created: String,
    labels: Vec<String>,
}

impl Bead {
    /// Can still be worked: label has a consumer (routing, triage, selection).
    fn workable(&self) -> bool {
        self.status != "tombstone" && self.status != "closed"
    }
    fn post_cutoff(&self) -> bool {
        self.created.as_str() >= DELTA_CUTOFF
    }
}

/// Parse `.beads/issues.jsonl`. Every failure mode is a panic with the reason
/// named, because each one is indistinguishable from "clean board" if swallowed.
fn beads() -> Vec<Bead> {
    let path = repo_root().join(".beads/issues.jsonl");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ANTI-VACUITY: cannot read {}: {e}", path.display()));

    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
            panic!(
                "{}:{}: not valid JSON ({e}). A tracker that does not parse is not a \
                 tracker that passes.",
                path.display(),
                n + 1
            )
        });
        let id = v["id"].as_str().unwrap_or_default().to_owned();
        assert!(
            !id.is_empty(),
            "{}:{}: bead record has no `id` — the parser is misaligned with the schema",
            path.display(),
            n + 1
        );
        // A record with no parseable `created_at` is treated as POST-cutoff, i.e.
        // held to the strict rule. Fail-closed: the alternative silently
        // grandfathers anything that drops the field.
        let created = v["created_at"]
            .as_str()
            .map(|s| s.chars().take(10).collect::<String>())
            .filter(|s| s.len() == 10)
            .unwrap_or_else(|| "9999-99-99".to_owned());
        out.push(Bead {
            id,
            status: v["status"].as_str().unwrap_or_default().to_owned(),
            created,
            labels: v["labels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }

    assert!(
        !out.is_empty(),
        "ANTI-VACUITY: {} parsed to ZERO beads. An empty scan set is an ERROR, never a pass: \
         a deliverable that was never checked reports exactly like one that passed.",
        path.display()
    );
    out
}

fn live(beads: &[Bead]) -> Vec<&Bead> {
    beads.iter().filter(|b| b.status != "tombstone").collect()
}

fn distinct_live_labels(beads: &[Bead]) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for b in live(beads) {
        for l in &b.labels {
            *m.entry(l.clone()).or_default() += 1;
        }
    }
    m
}

fn today_epoch_day() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is before 1970")
        .as_secs()
        / 86_400
}

// ---------------------------------------------------------------------------
// Anti-vacuity — runs first in spirit; every other test depends on it holding.
// ---------------------------------------------------------------------------

#[test]
fn the_scan_set_is_real() {
    let all = beads();
    let live = live(&all);
    let tax = taxonomy();
    let axes = axes();
    let amnesty = register_set("amnesty-labels");

    assert!(
        !live.is_empty(),
        "ANTI-VACUITY: every bead is a tombstone. Nothing below would be checked, and all of \
         it would report green."
    );
    assert!(
        live.iter().any(|b| !b.labels.is_empty()),
        "ANTI-VACUITY: not one live bead carries a label. Either the `labels` key moved or the \
         index was wiped; both look like a pass to the tests below."
    );
    assert!(
        live.iter().any(|b| b.workable()),
        "ANTI-VACUITY: not one live bead is workable. The delta leg would have nothing to \
         check and would report green."
    );
    assert!(
        live.iter().any(|b| b.post_cutoff()),
        "ANTI-VACUITY: not one live bead was created on/after {DELTA_CUTOFF}. The delta leg — \
         the whole point of this gate — would be scanning an empty set and reporting green."
    );

    eprintln!(
        "SCAN: {} beads total, {} live, {} tombstone, {} workable, {} created >= {DELTA_CUTOFF}",
        all.len(),
        live.len(),
        all.len() - live.len(),
        live.iter().filter(|b| b.workable()).count(),
        live.iter().filter(|b| b.post_cutoff()).count(),
    );
    eprintln!(
        "SCAN: registers — taxonomy {}, axes {} ({} members), amnesty-labels {} (ceiling \
         {AMNESTY_LABEL_CEILING}), expiry {AMNESTY_EXPIRY_HUMAN}",
        tax.len(),
        axes.len(),
        axis_members(&axes).len(),
        amnesty.len(),
    );
}

// ---------------------------------------------------------------------------
// KNOWN-GOOD leg — MANDATORY. The vocabulary as landed must pass.
// ---------------------------------------------------------------------------

/// ⛔ RE-SHAPED 2026-09-11. The off-taxonomy half of this leg was a LEVEL count
/// (672 beads, ceiling 526) that rose when the fleet filed work and could not be
/// lowered by the remedy it demanded — admitting the whole `s*`/`l*`/`phase-*`
/// family (434 label uses) moved it 531 → 530. Replaced by the DELTA: beads
/// created on/after {DELTA_CUTOFF} get ZERO allowance, which is measurably what
/// the fleet already does (68 such beads, 0 offenders). The accumulated 548 are
/// grandfathered by the dated registers, ratcheted DOWN by
/// `pre_cutoff_debt_only_falls`, and killed outright by `the_amnesty_expires`.
///
/// The unlabelled half is unchanged and was already green.
#[test]
fn every_workable_bead_carries_at_least_one_taxonomy_label() {
    let all = beads();
    let vocab = vocabulary();

    let unlabelled: Vec<String> = live(&all)
        .into_iter()
        .filter(|b| b.workable() && b.labels.is_empty())
        .map(|b| b.id.clone())
        .collect();
    eprintln!(
        "SCAN: unlabelled workable beads = {} (ceiling {UNLABELLED_WORKABLE_CEILING})",
        unlabelled.len()
    );
    assert!(
        unlabelled.len() <= UNLABELLED_WORKABLE_CEILING,
        "{} workable bead(s) carry NO label (ceiling {UNLABELLED_WORKABLE_CEILING}):\n  {}",
        unlabelled.len(),
        unlabelled.join("\n  ")
    );

    let mut new_offenders = Vec::new();
    for b in live(&all) {
        if !b.workable() || !b.post_cutoff() {
            continue;
        }
        let bad: Vec<&str> = b
            .labels
            .iter()
            .filter(|l| !vocab.contains(l.as_str()))
            .map(String::as_str)
            .collect();
        if !bad.is_empty() {
            new_offenders.push(format!("{} (created {}): {}", b.id, b.created, bad.join(", ")));
        }
    }
    assert!(
        new_offenders.is_empty(),
        "{} bead(s) created on/after {DELTA_CUTOFF} carry a label outside the controlled \
         vocabulary. There is NO allowance for these — the vocabulary is {} topical labels plus \
         {} declared axis members, and every bead the fleet filed on 2026-09-10 and 2026-09-11 \
         already obeyed it:\n  {}\n\nUse a taxonomy label. The pre-census offenders are \
         grandfathered by the dated ```amnesty-labels register; a bead filed after the census \
         is not.",
        new_offenders.len(),
        taxonomy().len(),
        vocab.len() - taxonomy().len(),
        new_offenders.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// Ratchets
// ---------------------------------------------------------------------------

/// ⛔ RE-SHAPED 2026-09-11: identity-pinned, not counted. The old form allowed
/// 267 distinct labels and reddened at 368; raising the number to 368 would have
/// permitted swapping a dead label for a fresh one-off indefinitely, which is
/// exactly the one-at-a-time growth this file exists to stop. Set membership does
/// not permit that: a label that was not in use at the census cannot appear at
/// all, on any bead, ever — and the register it would have to join has a
/// DOWN-only ceiling and an expiry date.
#[test]
fn distinct_label_count_does_not_grow() {
    let all = beads();
    let counts = distinct_live_labels(&all);
    let vocab = vocabulary();
    let amnesty = register_set("amnesty-labels");

    let intruders: Vec<String> = counts
        .iter()
        .filter(|(l, _)| !vocab.contains(l.as_str()) && !amnesty.contains(l.as_str()))
        .map(|(l, n)| format!("{l} ({n} carrier(s))"))
        .collect();
    eprintln!(
        "SCAN: distinct labels in use across live beads = {} ({} in vocabulary, {} grandfathered)",
        counts.len(),
        counts.keys().filter(|l| vocab.contains(l.as_str())).count(),
        counts.keys().filter(|l| amnesty.contains(l.as_str())).count(),
    );
    assert!(
        intruders.is_empty(),
        "{} label(s) are in use but in NO register — not in the {}-term taxonomy, not on a \
         declared axis, and not in the dated ```amnesty-labels census:\n  {}\n\nThis is the \
         one-label-at-a-time growth that made `bv -l` useless: 119 labels over 142 beads, 75 of \
         them used once. Map it onto a taxonomy label. The amnesty register is CLOSED — it was \
         a census of what existed on 2026-09-11 and it may only shrink.",
        intruders.len(),
        taxonomy().len(),
        intruders.join("\n  ")
    );
}

/// ⛔ RE-SHAPED 2026-09-11: the ratchet moved from the population to the
/// REGISTER. Counting singleton labels over live beads was population-coupled in
/// the worst direction — closing a bead strands its labels at one carrier, so the
/// number rose when the fleet SUCCEEDED (196 against a ceiling of 127). The
/// property that actually matters — one-off labels cannot multiply — is now
/// enforced by identity upstream in `distinct_label_count_does_not_grow`, and
/// what is ratcheted here is the size of the one-off vocabulary that is still
/// tolerated at all.
#[test]
fn singleton_labels_do_not_multiply() {
    let amnesty = register_set("amnesty-labels");
    let vocab = vocabulary();

    let overlap: Vec<&str> = amnesty
        .iter()
        .filter(|l| vocab.contains(l.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        overlap.is_empty(),
        "{} label(s) are in BOTH the controlled vocabulary and the amnesty register: {}. A \
         label cannot be both blessed and dying; that is how the two lists drift.",
        overlap.len(),
        overlap.join(", ")
    );

    eprintln!(
        "SCAN: grandfathered one-off vocabulary = {} (ceiling {AMNESTY_LABEL_CEILING}, DOWN only)",
        amnesty.len()
    );
    assert!(
        amnesty.len() <= AMNESTY_LABEL_CEILING,
        "the ```amnesty-labels register holds {} labels; the ceiling is \
         {AMNESTY_LABEL_CEILING} and may only FALL. The register is a CENSUS of what was in use \
         on 2026-09-11, not a queue. Adding a label to it is adding a label to the index — do \
         that in the ```taxonomy block, with a scope row, or not at all.",
        amnesty.len()
    );
}

/// The one number that measures remediation instead of population: pre-census
/// offenders still sitting on WORKABLE beads. It falls when a bead is relabelled
/// AND when a bead is closed, and it cannot rise, because a post-cutoff bead
/// carrying an off-vocabulary label is refused outright above.
#[test]
fn pre_cutoff_debt_only_falls() {
    let all = beads();
    let vocab = vocabulary();
    let offenders: Vec<String> = live(&all)
        .into_iter()
        .filter(|b| b.workable())
        .filter(|b| b.labels.iter().any(|l| !vocab.contains(l.as_str())))
        .map(|b| b.id.clone())
        .collect();
    eprintln!(
        "SCAN: workable beads carrying grandfathered labels = {} (ceiling \
         {ACTIVE_OFF_VOCABULARY_CEILING}, DOWN only, dies {AMNESTY_EXPIRY_HUMAN})",
        offenders.len()
    );
    assert!(
        offenders.len() <= ACTIVE_OFF_VOCABULARY_CEILING,
        "{} workable bead(s) carry a grandfathered off-vocabulary label; the ratchet ceiling is \
         {ACTIVE_OFF_VOCABULARY_CEILING} and may only FALL. It cannot rise from new beads — \
         those are refused outright — so a rise means a PRE-census bead was re-labelled with a \
         dying label. Map it onto a taxonomy label instead.\n  {}",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// The death condition. Without it a "temporary" register is permanent, and the
/// 548 grandfathered beads become the new normal by silence. Fails CLOSED: a
/// clock that cannot be read is a failure, not a skip.
#[test]
fn the_amnesty_expires() {
    let today = today_epoch_day();
    eprintln!(
        "SCAN: today = epoch day {today}; amnesty expires epoch day \
         {AMNESTY_EXPIRY_EPOCH_DAY} ({AMNESTY_EXPIRY_HUMAN}); {} day(s) left",
        AMNESTY_EXPIRY_EPOCH_DAY.saturating_sub(today)
    );
    assert!(
        today <= AMNESTY_EXPIRY_EPOCH_DAY,
        "the label amnesty granted on 2026-09-11 EXPIRED on {AMNESTY_EXPIRY_HUMAN} (epoch day \
         {AMNESTY_EXPIRY_EPOCH_DAY}); today is epoch day {today}. Three months was the whole \
         budget for draining the grandfathered beads. Drain them and delete the \
         ```amnesty-labels register, or take a ruling and re-date it — but do not extend it by \
         editing this constant quietly."
    );
}

// ---------------------------------------------------------------------------
// The registers themselves
// ---------------------------------------------------------------------------

#[test]
fn the_allowlist_is_small_enough_to_be_an_index() {
    let tax = taxonomy();
    let axes = axes();
    eprintln!(
        "SCAN: taxonomy = {} (max {TAXONOMY_MAX}), axes = {} (max {AXIS_MAX})",
        tax.len(),
        axes.len()
    );
    assert!(
        tax.len() <= TAXONOMY_MAX,
        "{} labels in the taxonomy. Past ~{TAXONOMY_MAX} the allowlist stops being a controlled \
         vocabulary and becomes a record of everything anyone ever wanted to say.",
        tax.len()
    );
    assert!(
        axes.len() <= AXIS_MAX,
        "{} declared axes (max {AXIS_MAX}). Axes exist because a plan-section partition is not a \
         topical label; a fourth axis is probably a topical vocabulary wearing a number.",
        axes.len()
    );
    for (axis, members) in &axes {
        assert!(
            members.len() <= AXIS_MEMBER_MAX,
            "axis `{axis}` declares {} members (max {AXIS_MEMBER_MAX})",
            members.len()
        );
    }
    for l in tax.iter().chain(axis_members(&axes).iter()) {
        assert!(
            l.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "vocabulary label `{l}` is not lowercase-kebab; `gates` vs `gate` and `S3` vs `s3` \
             are how a controlled vocabulary silently forks"
        );
    }
}

/// An axis is admitted as a WHOLE because it is a partition declared a priori,
/// so `phase-7` with one carrier is not the one-off defect. That concession is
/// only honest if the axis itself indexes: an axis nobody uses is a smuggling
/// route for arbitrary numbered labels.
#[test]
fn every_declared_axis_actually_indexes() {
    let all = beads();
    let counts = distinct_live_labels(&all);
    for (axis, members) in axes() {
        let in_use = members
            .iter()
            .filter(|m| counts.contains_key(m.as_str()))
            .count();
        let carriers: usize = members.iter().filter_map(|m| counts.get(m)).sum();
        eprintln!(
            "SCAN: axis `{axis}` — {} member(s), {in_use} in use, {carriers} carrier(s)",
            members.len()
        );
        assert!(
            in_use >= AXIS_MIN_MEMBERS_IN_USE && carriers >= AXIS_MIN_CARRIERS,
            "axis `{axis}` has {in_use} member(s) in use over {carriers} carrier(s); an axis \
             must reach {AXIS_MIN_MEMBERS_IN_USE} members and {AXIS_MIN_CARRIERS} carriers to be \
             an index at all. Below that it is a one-off label with a digit on the end, and it \
             belongs in the bead body."
        );
    }
}

#[test]
fn every_allowlist_label_is_actually_used() {
    // An allowlist entry with no carrier is aspiration, not vocabulary — and it
    // is the one direction the ratchets above cannot see. A controlled
    // vocabulary must nonetheless be allowed to PRECEDE its first use, so the
    // exceptions are named individually in the ```reserved register rather than
    // tolerated by a count: `unused <= 1` hides WHICH label is dead.
    let all = beads();
    let counts = distinct_live_labels(&all);
    let reserved = register_set("reserved");
    let tax = taxonomy();
    let unused: Vec<String> = tax
        .iter()
        .filter(|l| !counts.contains_key(l.as_str()))
        .cloned()
        .collect();

    let unnamed: Vec<&str> = unused
        .iter()
        .filter(|l| !reserved.contains(l.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        unnamed.is_empty(),
        "{} allowlist label(s) are carried by no live bead and are NOT named in the ```reserved \
         register: {}. Remove them, apply them, or reserve them explicitly with a death date; a \
         vocabulary you do not speak is not enforcing anything.",
        unnamed.len(),
        unnamed.join(", ")
    );

    // The other direction: a reservation that came true is a dead row, and dead
    // rows are how a register stops describing reality.
    let stale: Vec<&str> = reserved
        .iter()
        .filter(|l| counts.contains_key(l.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        stale.is_empty(),
        "{} ```reserved label(s) now have carriers: {}. Delete the reservation — it has served \
         its purpose and now only grants future silence.",
        stale.len(),
        stale.join(", ")
    );
    let unknown: Vec<&str> = reserved
        .iter()
        .filter(|l| !tax.contains(l.as_str()))
        .map(String::as_str)
        .collect();
    assert!(
        unknown.is_empty(),
        "{} ```reserved label(s) are not in the taxonomy at all: {}. Reserving a label you have \
         not declared reserves nothing.",
        unknown.len(),
        unknown.join(", ")
    );
    eprintln!(
        "SCAN: taxonomy labels with no carrier = {} ({}), all reserved",
        unused.len(),
        if unused.is_empty() {
            "-".to_owned()
        } else {
            unused.join(", ")
        }
    );
}
