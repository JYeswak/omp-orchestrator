#![forbid(unsafe_code)]
//! ONE stamped document for every grading round, and a refusal until it covers them all.
//!
//! # The measured problem
//!
//! Josh, 2026-09-01: *"make sure we get all rounds of edits into a single doc — no more rounds
//! of convergence until we have a stamped doc with all rounds included in it. only then can we
//! start another round."*
//!
//! Measured the same day, and this is why the ruling exists. The round record was scattered
//! across **twelve** files with two disjoint halves that no reader could reconcile:
//!
//! | source | rounds it holds | declared findings |
//! |---|---|---:|
//! | `docs/plan/CONVERGENCE.jsonl` | 8, 9, 10, 12, 13, 14, 15, 22 | 360 |
//! | `docs/plan/round*.jsonl` (11 files) | 16, 17, 18, 19, 20, 21, 22 | 199 |
//!
//! Rounds **16 through 21 never reached CONVERGENCE.jsonl at all** — they exist only in
//! per-agent files, one per grader, which is how a round can be run, recorded, and then be
//! invisible to anything that reads the canonical ledger. Round **11 is absent from both**.
//! `docs/plan/CONVERGENCE.md`, the human-readable convergence document, is hand-written prose
//! that stops at round 10.
//!
//! So "have we converged?" had no answer any single artifact could give, and a new round could
//! start without anyone noticing that six prior rounds were unrepresented.
//!
//! # What this crate does
//!
//! It INGESTS every source, emits ONE document (`docs/plan/ROUNDS.md`) plus a machine stamp
//! (`docs/plan/STAMP.toml`), and provides the predicate a gate uses to refuse a new round.
//!
//! The stamp is not a signature of approval. It is a **coverage claim with a hash**: these are
//! the rounds that exist, these are the section files at these digests, this is how much of the
//! declared work is dispositioned. A new round is refused when a round exists that the stamp
//! does not name, or when a section has changed since the stamp was cut.
//!
//! # What this does NOT claim
//!
//! A current stamp does not mean the plan is good, converged, or correct. It means the record
//! is COMPLETE and CURRENT — that every round is in one place and the sections have not moved
//! underneath it. Convergence is a separate judgement made against that record, and a stamp
//! with 559 declared findings and 21 dispositioned is a perfectly valid stamp of a plan that
//! is nowhere near done.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// One grading round's row, from whichever file held it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundRow {
    pub round: u64,
    pub section: String,
    pub lens: String,
    pub graded_by: String,
    pub declared: u64,
    pub verdict: String,
    pub source_file: String,
}

/// Everything the stamp needs to know, all read from disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoundCensus {
    pub rows: Vec<RoundRow>,
    /// round -> declared findings summed across every row for that round
    pub declared_by_round: BTreeMap<u64, u64>,
    /// round -> the files that carried it
    pub sources_by_round: BTreeMap<u64, BTreeSet<String>>,
    /// section file -> sha256 of its bytes
    pub section_digests: BTreeMap<String, String>,
    /// finding ids already dispositioned in FINDINGS.jsonl
    pub dispositioned: BTreeSet<String>,
    /// dispositioned counted per round
    pub dispositioned_by_round: BTreeMap<u64, u64>,
    /// Rounds whose rows carry `void: true`.
    ///
    /// Joshua's ruling, 2026-09-01: *"a resolution of a finding from a voided round must not
    /// appear as coverage in either direction, not as done and not as outstanding."* So a void
    /// round is EXCLUDED from both the numerator and the denominator, and is reported in its
    /// own section — never folded into "unreconciled", which would read as outstanding work.
    pub void_rounds: BTreeSet<u64>,
    /// Rounds that predate per-finding identity and are outside HD-0006's 15-22 scope.
    ///
    /// Measured, and broader than the ruling assumed: ZERO rows in `CONVERGENCE.jsonl` carry a
    /// `finding_ids` field — 0 of 70 below round 15 AND 0 of 17 at 15 and above. So every id in
    /// `FINDINGS.jsonl` was minted by a reconciling pane. Below round 15 there is additionally
    /// no reconciliation bead, so minting 340 more would manufacture a ledger that looks
    /// complete and cites nothing. Recorded, not dispositioned; bead
    /// `omp-orchestrator-mine-pre-identity-rounds-8-14-lfa`.
    pub pre_identity_rounds: BTreeSet<u64>,
}

/// HD-0006 scopes reconciliation to rounds 15 through 22. Rounds below this are recorded and
/// excluded from coverage rather than mass-minted.
pub const HD_0006_SCOPE_START: u64 = 15;

/// Why a new round is refused. Every arm names what to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundRefusal {
    /// No stamp on disk at all.
    NoStamp,
    /// A round exists in the repo that the stamp does not name.
    RoundNotInStamp { round: u64, sources: Vec<String> },
    /// A section file changed since the stamp was cut.
    SectionDrifted { section: String, stamped: String, current: String },
    /// The stamp names a section that no longer exists.
    SectionVanished { section: String },
    /// ANTI-VACUITY: the census found nothing, which reports identically to a clean repo.
    EmptyCensus,
}

impl RoundRefusal {
    pub fn code(&self) -> &'static str {
        match self {
            RoundRefusal::NoStamp => "ROUND_REFUSED_NO_STAMP",
            RoundRefusal::RoundNotInStamp { .. } => "ROUND_REFUSED_ROUND_NOT_STAMPED",
            RoundRefusal::SectionDrifted { .. } => "ROUND_REFUSED_SECTION_DRIFTED",
            RoundRefusal::SectionVanished { .. } => "ROUND_REFUSED_SECTION_VANISHED",
            RoundRefusal::EmptyCensus => "ROUND_REFUSED_EMPTY_CENSUS",
        }
    }

    /// The remedy MUST be invocable. A gate whose remedy does not exist is a trap, not a guard
    /// — three instances of that were measured in this repo on 2026-09-01, two of them citing
    /// `/tmp` paths a reboot deletes.
    pub fn remedy(&self) -> String {
        match self {
            RoundRefusal::NoStamp | RoundRefusal::RoundNotInStamp { .. } => {
                "cargo run -p convergence-stamp -- --write".to_owned()
            }
            RoundRefusal::SectionDrifted { .. } | RoundRefusal::SectionVanished { .. } => {
                "cargo run -p convergence-stamp -- --write   (the sections moved; re-cut the stamp)"
                    .to_owned()
            }
            RoundRefusal::EmptyCensus => {
                "check the repo root: no round ledger and no plan sections were found".to_owned()
            }
        }
    }
}

impl std::fmt::Display for RoundRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoundRefusal::NoStamp => write!(f, "{} docs/plan/STAMP.toml is absent", self.code()),
            RoundRefusal::RoundNotInStamp { round, sources } => write!(
                f,
                "{} round={round} sources={}",
                self.code(),
                sources.join(",")
            ),
            RoundRefusal::SectionDrifted { section, stamped, current } => write!(
                f,
                "{} section={section} stamped={} current={}",
                self.code(),
                &stamped[..stamped.len().min(16)],
                &current[..current.len().min(16)]
            ),
            RoundRefusal::SectionVanished { section } => {
                write!(f, "{} section={section}", self.code())
            }
            RoundRefusal::EmptyCensus => write!(f, "{} nothing was scanned", self.code()),
        }?;
        write!(f, " -> {}", self.remedy())
    }
}

// ───────────────────────────────────────────────────────────────── sha256

/// SHA-256, so a stamp pins content and not an mtime.
///
/// Hand-written because this workspace carries no digest dependency and the repo's rule is to
/// reuse what exists before adding a dependency. FIPS 180-4, verified against the two published
/// test vectors in `tests/`.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().enumerate().take(16) {
            let b = &chunk[i * 4..i * 4 + 4];
            *word = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip(v) {
            *slot = slot.wrapping_add(value);
        }
    }
    h.iter().fold(String::with_capacity(64), |mut out, word| {
        let _ = write!(out, "{word:08x}");
        out
    })
}

// ───────────────────────────────────────────────────────────────── census

fn str_field(row: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|n| row.get(*n).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn read_rows(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Every file that can hold a round row. Discovered from disk, never hand-listed — a
/// hand-listed set is how rounds 16-21 went unnoticed in the first place.
pub fn round_ledger_files(root: &Path) -> Vec<PathBuf> {
    let plan = root.join("docs/plan");
    let mut out = Vec::new();
    let canonical = plan.join("CONVERGENCE.jsonl");
    if canonical.is_file() {
        out.push(canonical);
    }
    if let Ok(entries) = fs::read_dir(&plan) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("round") && name.ends_with(".jsonl") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

pub fn section_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join("docs/plan")) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let numbered = name.len() > 3
                && name.as_bytes()[0].is_ascii_digit()
                && name.as_bytes()[1].is_ascii_digit()
                && name.as_bytes()[2] == b'-';
            if numbered && name.ends_with(".md") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

pub fn census(root: &Path) -> RoundCensus {
    let mut c = RoundCensus::default();

    for file in round_ledger_files(root) {
        let label = file
            .strip_prefix(root)
            .unwrap_or(&file)
            .to_string_lossy()
            .into_owned();
        for row in read_rows(&file) {
            let Some(round) = row.get("round").and_then(Value::as_u64) else { continue };
            let section = str_field(&row, &["section"]).unwrap_or_else(|| "<none>".to_owned());
            let declared = row.get("new_findings").and_then(Value::as_u64).unwrap_or(0);
            c.rows.push(RoundRow {
                round,
                section: section.clone(),
                lens: str_field(&row, &["lens"]).unwrap_or_else(|| "<none>".to_owned()),
                graded_by: str_field(&row, &["graded_by", "recorded_by"])
                    .unwrap_or_else(|| "<none>".to_owned()),
                declared,
                verdict: str_field(&row, &["verdict"]).unwrap_or_else(|| "<none>".to_owned()),
                source_file: label.clone(),
            });
            *c.declared_by_round.entry(round).or_default() += declared;
            c.sources_by_round.entry(round).or_default().insert(label.clone());
            // A voided round row is the RECORD that an unpinned round was attempted. Keep the
            // row and exclude the ROUND from coverage — deleting it would erase the attempt.
            if row.get("void").and_then(Value::as_bool).unwrap_or(false) {
                c.void_rounds.insert(round);
            }
        }
    }
    c.rows.sort_by(|a, b| {
        (a.round, &a.section, &a.graded_by).cmp(&(b.round, &b.section, &b.graded_by))
    });

    for file in section_files(root) {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if let Ok(bytes) = fs::read(&file) {
            c.section_digests.insert(name, sha256_hex(&bytes));
        }
    }

    for row in read_rows(&root.join("docs/plan/FINDINGS.jsonl")) {
        let voided = row.get("void").and_then(Value::as_bool).unwrap_or(false);
        if voided {
            if let Some(round) = row.get("round").and_then(Value::as_u64) {
                c.void_rounds.insert(round);
            }
            continue; // not coverage in either direction
        }
        if let Some(id) = str_field(&row, &["id", "finding_id"]) {
            c.dispositioned.insert(id);
        }
        if let Some(round) = row.get("round").and_then(Value::as_u64) {
            *c.dispositioned_by_round.entry(round).or_default() += 1;
        }
    }

    // Rounds below HD-0006's scope with declared findings and no dispositions predate
    // per-finding identity: recorded, never mass-minted.
    for (round, declared) in &c.declared_by_round {
        if *round < HD_0006_SCOPE_START
            && *declared > 0
            && c.dispositioned_by_round.get(round).copied().unwrap_or(0) == 0
        {
            c.pre_identity_rounds.insert(*round);
        }
    }

    c
}

impl RoundCensus {
    /// Rounds that count toward coverage: not void, not pre-identity.
    ///
    /// This is the whole of Joshua's first ruling expressed once, in one place, so no renderer
    /// can accidentally fold a void round into "outstanding".
    pub fn live_rounds(&self) -> Vec<u64> {
        self.declared_by_round
            .keys()
            .copied()
            .filter(|r| !self.void_rounds.contains(r) && !self.pre_identity_rounds.contains(r))
            .collect()
    }

    pub fn declared_live(&self) -> u64 {
        self.live_rounds()
            .iter()
            .map(|r| self.declared_by_round.get(r).copied().unwrap_or(0))
            .sum()
    }

    pub fn dispositioned_live(&self) -> u64 {
        self.live_rounds()
            .iter()
            .map(|r| self.dispositioned_by_round.get(r).copied().unwrap_or(0))
            .sum()
    }
}

// ───────────────────────────────────────────────────────────────── the stamp

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamp {
    pub rounds: BTreeSet<u64>,
    pub section_digests: BTreeMap<String, String>,
}

pub fn render_stamp(c: &RoundCensus, cut_at: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "# STAMP.toml — the coverage claim behind docs/plan/ROUNDS.md\n\
         #\n\
         # GENERATED by `cargo run -p convergence-stamp -- --write`. Never hand-edit: a\n\
         # hand-edited stamp is a claim with no derivation, which is the defect the whole\n\
         # NUMBERS.toml discipline exists to kill.\n\
         #\n\
         # A new grading round is REFUSED while any round in the repo is missing from `rounds`\n\
         # or any section digest here disagrees with the file on disk. The stamp says the\n\
         # RECORD is complete and current. It does NOT say the plan is converged.\n\n",
    );
    let _ = writeln!(out, "[stamp]");
    let _ = writeln!(out, "cut_at = \"{cut_at}\"");
    let _ = writeln!(
        out,
        "rounds = [{}]",
        c.declared_by_round
            .keys()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(out, "round_count = {}", c.declared_by_round.len());
    let _ = writeln!(out, "row_count = {}", c.rows.len());
    let _ = writeln!(
        out,
        "declared_findings_total = {}",
        c.declared_by_round.values().sum::<u64>()
    );
    out.push('\n');
    out.push_str(
        "# COVERAGE, split three ways per Joshua's rulings of 2026-09-01. A void round and a\n\
         # pre-identity round are excluded from BOTH the numerator and the denominator: a\n\
         # resolution from a voided round must not read as done OR as outstanding, and rounds\n\
         # that predate per-finding identity must not be mass-minted into false coverage.\n",
    );
    let _ = writeln!(out, "[coverage]");
    let _ = writeln!(
        out,
        "live_rounds = [{}]",
        c.live_rounds().iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(out, "declared_live = {}", c.declared_live());
    let _ = writeln!(out, "dispositioned_live = {}", c.dispositioned_live());
    let _ = writeln!(
        out,
        "void_rounds = [{}]   # HD-0005: unpinned, record kept, coverage excluded",
        c.void_rounds.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        out,
        "pre_identity_rounds = [{}]   # no finding_ids ever recorded; bead omp-orchestrator-mine-pre-identity-rounds-8-14-lfa",
        c.pre_identity_rounds.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        out,
        "declared_pre_identity = {}",
        c.pre_identity_rounds
            .iter()
            .map(|r| c.declared_by_round.get(r).copied().unwrap_or(0))
            .sum::<u64>()
    );
    let _ = writeln!(out, "section_count = {}", c.section_digests.len());
    out.push('\n');
    let _ = writeln!(out, "[declared_by_round]");
    for (round, n) in &c.declared_by_round {
        let done = c.dispositioned_by_round.get(round).copied().unwrap_or(0);
        let class = if c.void_rounds.contains(round) {
            "void"
        } else if c.pre_identity_rounds.contains(round) {
            "pre_identity"
        } else {
            "live"
        };
        let _ = writeln!(
            out,
            "r{round} = {{ declared = {n}, dispositioned = {done}, class = \"{class}\" }}"
        );
    }
    out.push('\n');
    let _ = writeln!(out, "[sections]");
    for (name, digest) in &c.section_digests {
        let _ = writeln!(out, "\"{name}\" = \"{digest}\"");
    }
    out
}

pub fn parse_stamp(text: &str) -> Stamp {
    let mut stamp = Stamp::default();
    let mut in_sections = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if l.starts_with('[') {
            in_sections = l == "[sections]";
            continue;
        }
        if in_sections {
            if let Some((k, v)) = l.split_once('=') {
                let key = k.trim().trim_matches('"').to_owned();
                let val = v.trim().trim_matches('"').to_owned();
                if !key.is_empty() && !val.is_empty() {
                    stamp.section_digests.insert(key, val);
                }
            }
            continue;
        }
        if let Some(rest) = l.strip_prefix("rounds") {
            if let Some((_, list)) = rest.split_once('=') {
                for part in list.trim().trim_matches(['[', ']']).split(',') {
                    if let Ok(n) = part.trim().parse::<u64>() {
                        stamp.rounds.insert(n);
                    }
                }
            }
        }
    }
    stamp
}

/// The predicate a gate uses. Fail CLOSED: any doubt refuses the round.
pub fn refusals(c: &RoundCensus, stamp: Option<&Stamp>) -> Vec<RoundRefusal> {
    // ANTI-VACUITY first: an empty census reports identically to a clean repo.
    if c.rows.is_empty() || c.section_digests.is_empty() {
        return vec![RoundRefusal::EmptyCensus];
    }
    let Some(stamp) = stamp else { return vec![RoundRefusal::NoStamp] };

    let mut out = Vec::new();
    for (round, sources) in &c.sources_by_round {
        if !stamp.rounds.contains(round) {
            out.push(RoundRefusal::RoundNotInStamp {
                round: *round,
                sources: sources.iter().cloned().collect(),
            });
        }
    }
    for (section, current) in &c.section_digests {
        match stamp.section_digests.get(section) {
            None => out.push(RoundRefusal::SectionDrifted {
                section: section.clone(),
                stamped: "<absent>".to_owned(),
                current: current.clone(),
            }),
            Some(stamped) if stamped != current => out.push(RoundRefusal::SectionDrifted {
                section: section.clone(),
                stamped: stamped.clone(),
                current: current.clone(),
            }),
            Some(_) => {}
        }
    }
    for section in stamp.section_digests.keys() {
        if !c.section_digests.contains_key(section) {
            out.push(RoundRefusal::SectionVanished { section: section.clone() });
        }
    }
    out
}

// ───────────────────────────────────────────────────── the single document

pub fn render_document(c: &RoundCensus, cut_at: &str) -> String {
    let declared: u64 = c.declared_by_round.values().sum();
    let mut out = String::new();

    out.push_str("# ROUNDS — every grading round, in one document\n\n");
    let _ = writeln!(
        out,
        "> **GENERATED.** `cargo run -p convergence-stamp -- --write`. Do not hand-edit; \
         re-run it.\n>\n> **Josh, 2026-09-01:** *\"make sure we get all rounds of edits into a \
         single doc — no more rounds of convergence until we have a stamped doc with all rounds \
         included in it. only then can we start another round.\"*\n"
    );
    out.push_str(
        "\nBefore this document existed the round record was split across twelve files with two \
         halves that no reader could reconcile: `CONVERGENCE.jsonl` held rounds 8-15 and 22, \
         while rounds **16 through 21 existed only in eleven per-agent `round*.jsonl` files** and \
         had never reached the canonical ledger. Round 11 is absent from both. \
         `CONVERGENCE.md`, the human-readable convergence document, is hand-written prose that \
         stops at round 10. So \"have we converged?\" had no answer a single artifact could give, \
         and a new round could start while six prior rounds were unrepresented.\n\n\
         This document is the answer, and `STAMP.toml` is its coverage claim. A new round is \
         refused while any round on disk is missing from the stamp, or any plan section has \
         changed since the stamp was cut.\n\n",
    );

    let _ = writeln!(out, "## Stamp\n");
    let _ = writeln!(out, "| field | value |");
    let _ = writeln!(out, "|---|---|");
    let _ = writeln!(out, "| cut at | `{cut_at}` |");
    let _ = writeln!(out, "| rounds recorded | **{}** |", c.declared_by_round.len());
    let _ = writeln!(out, "| round rows | **{}** |", c.rows.len());
    let _ = writeln!(out, "| declared findings, all rounds | **{declared}** |");
    let _ = writeln!(out, "| plan sections digested | **{}** |", c.section_digests.len());
    out.push('\n');

    let _ = writeln!(out, "### Coverage, split three ways\n");
    out.push_str(
        "Joshua's rulings of 2026-09-01. A **void** round and a **pre-identity** round are \
         excluded from BOTH the numerator and the denominator — the first because *\"a resolution \
         of a finding from a voided round must not appear as coverage in either direction, not as \
         done and not as outstanding\"*, the second because minting ids for rounds that never had \
         them *\"would manufacture a ledger that looks complete and cites nothing.\"*\n\n",
    );
    let _ = writeln!(out, "| class | rounds | declared | dispositioned | counts toward coverage |");
    let _ = writeln!(out, "|---|---|---:|---:|---|");
    let live = c.live_rounds();
    let _ = writeln!(
        out,
        "| **LIVE** | {} | **{}** | **{}** | yes |",
        live.iter().map(u64::to_string).collect::<Vec<_>>().join(", "),
        c.declared_live(),
        c.dispositioned_live()
    );
    let pre_declared: u64 = c
        .pre_identity_rounds
        .iter()
        .map(|r| c.declared_by_round.get(r).copied().unwrap_or(0))
        .sum();
    let _ = writeln!(
        out,
        "| PRE-IDENTITY | {} | {pre_declared} | 0 | no — recorded, never minted |",
        c.pre_identity_rounds.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
    );
    let void_declared: u64 = c
        .void_rounds
        .iter()
        .map(|r| c.declared_by_round.get(r).copied().unwrap_or(0))
        .sum();
    let _ = writeln!(
        out,
        "| VOID (HD-0005) | {} | {void_declared} | 0 | no — record kept, coverage excluded |",
        c.void_rounds.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        out,
        "\n**{} of {} declared findings are dispositioned on the live rounds.** That ratio is the \
         honest state of the plan, and a current stamp does not improve it — the stamp says the \
         RECORD is complete, never that the work is.\n",
        c.dispositioned_live(),
        c.declared_live()
    );

    let _ = writeln!(out, "## Every round\n");
    let _ = writeln!(
        out,
        "| round | class | rows | declared | dispositioned | sections graded | source files |"
    );
    let _ = writeln!(out, "|---:|---|---:|---:|---:|---:|---|");
    for (round, n) in &c.declared_by_round {
        let rows: Vec<&RoundRow> = c.rows.iter().filter(|r| r.round == *round).collect();
        let sections: BTreeSet<&str> = rows.iter().map(|r| r.section.as_str()).collect();
        let done = c.dispositioned_by_round.get(round).copied().unwrap_or(0);
        let class = if c.void_rounds.contains(round) {
            "VOID"
        } else if c.pre_identity_rounds.contains(round) {
            "PRE-IDENTITY"
        } else {
            "LIVE"
        };
        let sources: Vec<String> = c
            .sources_by_round
            .get(round)
            .map(|s| s.iter().map(|f| format!("`{f}`")).collect())
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "| **{round}** | {class} | {} | {n} | {done} | {} | {} |",
            rows.len(),
            sections.len(),
            sources.join("<br>")
        );
    }

    if !c.pre_identity_rounds.is_empty() {
        let _ = writeln!(
            out,
            "\n### PRE-IDENTITY: {} rounds, {pre_declared} declared findings, recorded and not \
             dispositioned\n",
            c.pre_identity_rounds.len()
        );
        let _ = writeln!(out, "| round | declared | dispositioned |");
        let _ = writeln!(out, "|---:|---:|---:|");
        for round in &c.pre_identity_rounds {
            let _ = writeln!(
                out,
                "| {round} | {} | 0 |",
                c.declared_by_round.get(round).copied().unwrap_or(0)
            );
        }
        out.push_str(
            "\n**Joshua's ruling, 2026-09-01 — a bead, not a blocker, and the reasoning is the \
             load-bearing part:** *\"Those rounds predate the finding-id discipline entirely — \
             their rows carry no `finding_ids` field, so there is no per-finding identity to \
             disposition against. Reconciling them means MINTING 340 ids that join back to \
             nothing, which is precisely the defect I found in the R16 migration: 'the R16-00-001 \
             ids were minted by the reconciling pane and do not join back.' Doing that 340 times \
             would manufacture a ledger that looks complete and cites nothing.\"*\n\n\
             Measured, and **broader than the ruling assumed**: `jq`-equivalent over \
             `CONVERGENCE.jsonl` finds **zero** rows carrying `finding_ids` at ANY round — 0 of 70 \
             below round 15 and 0 of 17 at 15 and above. So every id in `FINDINGS.jsonl` was \
             minted by a reconciling pane, not by the grader who declared the finding. Below round \
             15 there is additionally no reconciliation bead, which is what separates \"recorded\" \
             from \"reconciled\" here.\n\n\
             HD-0006 scopes reconciliation to rounds 15-22 and that scope stands. Mining bead: \
             `omp-orchestrator-mine-pre-identity-rounds-8-14-lfa`. If a real finding dies in this \
             gap, the correct response is an example and a re-ruling — not a mass id mint.\n",
        );
    }

    if !c.void_rounds.is_empty() {
        let _ = writeln!(
            out,
            "\n### VOID under HD-0005: {} round(s), excluded from coverage in BOTH directions\n",
            c.void_rounds.len()
        );
        for round in &c.void_rounds {
            let _ = writeln!(
                out,
                "- **round {round}** — {} declared, rows retained. The rows are the RECORD that an \
                 unpinned round was attempted; deleting them would erase the attempt. Their \
                 dispositions do not count as done OR as outstanding.",
                c.declared_by_round.get(round).copied().unwrap_or(0)
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Every row\n");
    let _ = writeln!(
        out,
        "| round | section | lens | graded by | declared | verdict | source |"
    );
    let _ = writeln!(out, "|---:|---|---|---|---:|---|---|");
    for row in &c.rows {
        let _ = writeln!(
            out,
            "| {} | `{}` | {} | {} | {} | {} | `{}` |",
            row.round, row.section, row.lens, row.graded_by, row.declared, row.verdict,
            row.source_file
        );
    }

    let _ = writeln!(out, "\n## Section digests at this stamp\n");
    let _ = writeln!(out, "| section | sha256 |");
    let _ = writeln!(out, "|---|---|");
    for (name, digest) in &c.section_digests {
        let _ = writeln!(out, "| `{name}` | `{}` |", &digest[..16]);
    }

    out.push_str(
        "\n## NO-CLAIM\n\nA current stamp means the record is COMPLETE and the sections have not \
         moved since it was cut. It does not mean the plan is converged, correct, or good, and it \
         does not mean the declared findings were addressed — the dispositioned column is the only \
         thing that speaks to that. A stamp over a plan with hundreds of undispositioned findings \
         is a valid stamp of an unfinished plan.\n\nThis document is GENERATED from \
         `docs/plan/CONVERGENCE.jsonl`, every `docs/plan/round*.jsonl`, `docs/plan/FINDINGS.jsonl`, \
         and the numbered section files. It discovers its inputs from disk rather than a \
         hand-listed set, because a hand-listed set is exactly how rounds 16-21 went unnoticed.\n",
    );
    out
}
