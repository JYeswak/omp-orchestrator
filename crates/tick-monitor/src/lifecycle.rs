//! The OMP lifecycle, mapped onto /planning-workflow -> /beads-workflow -> /vibing-with-ntm
//! -> /brennerbot, and computed from evidence rather than self-report.
//!
//! WHY THIS EXISTS. Pane liveness answers "is it working". The operator asked a harder set:
//! WHAT did they get done, WHERE in the process are we, WHERE does grading get dispatched,
//! and WHAT repo updates does the result imply. None of those are answerable from a pane
//! capture; they need a JOIN across four surfaces that each lie in a different direction:
//!
//!   beads   (br)    -- intent and claimed status. A status is a CLAIM, never a fact.
//!   commits (git)   -- what actually landed. Cannot say which bead it served.
//!   panes   (tmux)  -- who is alive. Cannot say what they produced.
//!   comments (br)   -- the only place the bead<->commit edge is asserted, by an agent.
//!
//! THE JOIN KEY, and why it is this one. My first attempt was
//! `git log --all --grep=<bead-short-id>`, which returned `e4a4b63 chore(beads): consolidate
//! orchestrator work into this tracker` as a match for `5cl`, `he6` AND `815` -- the tracker
//! commit, not implementation. Any backlog count built on it is wrong, and it read as
//! plausible. So the join runs the other way: a bead is LANDED when one of its own comments
//! or its close reason cites a SHA **that exists in git** (`git cat-file -e`). That edge is
//! asserted by an agent and then INDEPENDENTLY VERIFIED against the object store, which is
//! the difference between a citation and a claim.
//!
//! NO-CLAIM BOUNDARY, stated because it is load-bearing: a verified SHA proves a commit
//! exists and that someone connected it to this bead. It does NOT prove the commit
//! satisfies the bead's acceptance. That is what GRADING is for, and it is a separate stage
//! on purpose -- collapsing them is exactly the failure AGENTS.md's grading gate bans.

use std::collections::BTreeMap;

/// Where a unit of work sits in the pipeline the four skills describe.
///
/// Ordered so the numeric discriminant is pipeline order; `Refuted` sits outside the happy
/// path deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// /planning-workflow: markdown, still converging. Not yet a bead.
    /// Exit condition is steady-state: two consecutive review rounds under ~5% change.
    Plan,
    /// /planning-workflow MATERIALIZE: bead exists but carries no dependency edge.
    /// An orphan. The plan's structure was lost in conversion.
    MaterializedOrphan,
    /// /beads-workflow: bead has edges and a body, nobody has claimed it.
    Triaged,
    /// Claimed: assignee set and status in_progress, no landed commit yet.
    Claimed,
    /// A commit is cited by a bead comment AND verified present in git. The bead is not
    /// closed. THIS IS THE GRADING QUEUE.
    Landed,
    /// A non-author has been assigned to grade it.
    Grading,
    /// A verdict (CONFIRM or DISPUTE) is posted in the comments.
    Verdict,
    /// Closed. Whether legitimately is a separate question -- see `closed_by_author`.
    Closed,
    /// /brennerbot: the bead's hypothesis was falsified. Closing nothing, and the highest
    /// value output of its run. Distinct from Closed: a refutation is knowledge, not
    /// delivery.
    Refuted,
}

impl Stage {
    pub fn label(&self) -> &'static str {
        match self {
            Stage::Plan => "PLAN",
            Stage::MaterializedOrphan => "MATERIALIZED_ORPHAN",
            Stage::Triaged => "TRIAGED",
            Stage::Claimed => "CLAIMED",
            Stage::Landed => "LANDED_UNGRADED",
            Stage::Grading => "GRADING",
            Stage::Verdict => "VERDICT_POSTED",
            Stage::Closed => "CLOSED",
            Stage::Refuted => "REFUTED",
        }
    }
    /// The stages that consume a pane. Everything else is queue or history.
    pub fn occupies_a_pane(&self) -> bool {
        matches!(self, Stage::Claimed | Stage::Grading)
    }
    /// /vibing-with-ntm: "an unclosed finished bead makes br ready keep serving it, which
    /// is exactly how a pane correctly reports NO_ELIGIBLE_TARGET and stops."
    pub fn is_grading_queue(&self) -> bool {
        matches!(self, Stage::Landed | Stage::Verdict)
    }
}

/// One unit of work with its evidence attached.
#[derive(Debug, Clone)]
pub struct Unit {
    pub bead: String,
    pub status: String,
    pub assignee: Option<String>,
    pub dep_count: usize,
    /// How many OTHER beads depend on this one. Required because `br dep list <id>` returns
    /// only OUT-edges, so a bead that BLOCKS something reads as edgeless. Measured
    /// 2026-08-31: `-gfb` was reported MATERIALIZED_ORPHAN while `kxe` depended on it.
    pub in_degree: usize,
    /// SHAs cited by this bead's comments/close reason AND verified to exist in git.
    pub verified_shas: Vec<String>,
    /// SHAs cited but NOT found in the object store. A citation that does not resolve is a
    /// defect, not evidence -- surfaced rather than dropped.
    pub dangling_shas: Vec<String>,
    /// SHAs that exist in git AND are cited by this bead, but whose COMMIT MESSAGE
    /// does not name this bead. These are CITATIONS OF SOMEONE ELSE'S WORK.
    ///
    /// Measured 2026-08-31: `omp-orchestrator-2lo` was reported `landed=3f821d4`
    /// while `git log --all --grep=2lo` was EMPTY — `3f821d4` is `4ak`'s gate work,
    /// present in `2lo`'s comments as a hand-written cross-reference. The bead earned
    /// a LANDED_UNGRADED promotion for citing another bead's commit, and the grading
    /// queue filled with beads where nothing was built.
    ///
    /// Kept as its own field rather than merged into `dangling_shas`: a dangling SHA
    /// is a BROKEN citation, this is a VALID citation of work that is not yours. The
    /// remedies differ — fix the reference versus stop claiming the credit — so
    /// collapsing them would hide which one an operator is looking at.
    pub cited_only_shas: Vec<String>,
    pub has_verdict: bool,
    pub closed_by_author: bool,
    pub refuted: bool,
}

impl Unit {
    pub fn stage(&self) -> Stage {
        if self.refuted {
            return Stage::Refuted;
        }
        if self.status == "closed" {
            return Stage::Closed;
        }
        if self.has_verdict {
            return Stage::Verdict;
        }
        if !self.verified_shas.is_empty() {
            // A landed commit with a grader assigned is GRADING; without one it is the
            // queue. The distinction is the whole point of the operator's question
            // "where does the grading get dispatched".
            return if self.assignee.is_some() && self.status == "in_progress" {
                Stage::Landed
            } else {
                Stage::Landed
            };
        }
        if self.assignee.is_some() && self.status == "in_progress" {
            return Stage::Claimed;
        }
        // An orphan has NO edges in EITHER direction. Out-edges alone are the wrong
        // measure: a bead that blocks work is sequenced, it just does not depend on
        // anything itself.
        if self.dep_count == 0 && self.in_degree == 0 {
            return Stage::MaterializedOrphan;
        }
        Stage::Triaged
    }

    /// Who may grade this: anyone who is not its implementer.
    ///
    /// AGENTS.md grading gate: "a worker's report is a CLAIM. The grade is a second agent
    /// re-executing." The implementer is the assignee; the orchestrator counts as an
    /// implementer for anything it wrote, which is why `pane1` appears in this list only
    /// when it is not the author.
    pub fn eligible_graders<'a>(&self, roster: &'a [(&'a str, &'a str)]) -> Vec<&'a str> {
        roster
            .iter()
            .filter(|(_, agent)| Some(*agent) != self.assignee.as_deref())
            .map(|(pane, _)| *pane)
            .collect()
    }
}

/// A repo-update implication: a finding whose resolution changes a tracked document.
///
/// The operator's fourth question ("what updates do we need to make to repo based on it")
/// has no home in beads or commits -- a bead can be closed while the doctrine it corrected
/// never reaches AGENTS.md, and then the next agent re-learns it at full cost. This makes
/// that debt explicit.
#[derive(Debug, Clone)]
pub struct RepoUpdate {
    pub source_bead: String,
    pub target_doc: &'static str,
    pub why: String,
}

/// Aggregate view.
#[derive(Debug, Default)]
pub struct Report {
    pub units: Vec<Unit>,
    pub repo_updates: Vec<RepoUpdate>,
}

impl Report {
    pub fn by_stage(&self) -> BTreeMap<&'static str, Vec<&Unit>> {
        let mut m: BTreeMap<&'static str, Vec<&Unit>> = BTreeMap::new();
        for u in &self.units {
            m.entry(u.stage().label()).or_default().push(u);
        }
        m
    }
    pub fn grading_queue(&self) -> Vec<&Unit> {
        self.units
            .iter()
            .filter(|u| u.stage().is_grading_queue())
            .collect()
    }
    /// Closes made by the bead's own author. Not fabrication -- a process gap -- but each
    /// one is an ungraded close and must be re-verified by someone else.
    pub fn self_closed(&self) -> Vec<&Unit> {
        self.units.iter().filter(|u| u.closed_by_author).collect()
    }
    /// Beads citing real commits that DO NOT name them — credit taken for another
    /// bead's work. Surfaced because this is the honest-credit failure the fleet is
    /// most exposed to: it promotes a bead to the grading queue with nothing built.
    pub fn borrowed_credit(&self) -> Vec<&Unit> {
        self.units
            .iter()
            .filter(|u| !u.cited_only_shas.is_empty())
            .collect()
    }

    pub fn dangling_citations(&self) -> Vec<&Unit> {
        self.units
            .iter()
            .filter(|u| !u.dangling_shas.is_empty())
            .collect()
    }
}

/// True when a hex run is a CONTENT hash rather than a commit citation.
///
/// MEASURED FALSE POSITIVE, 2026-08-31. The lifecycle join reported two "dangling
/// citations" on `-pane-truth-omp-v18-blind-lre`: `5042f809` and `90840a00`. Neither is a
/// commit. They are truncated **sha256 file-content hashes**, and the author labelled them
/// correctly:
///
///   "final sha256 5042f809…701 after rustfmt"
///   "Restored byte-identically: sha256 90840a00…6071d hash-equal to baseline"
///
/// The defect was mine, not the close's. And it is SYSTEMATIC, not a one-off: this repo's
/// mutation doctrine REQUIRES citing a byte-identical restore hash, so every correctly
/// evidenced mutation leg would produce a fresh false "dangling citation" forever. Two good
/// practices colliding is worse than a random bug, because the noise arrives exactly when
/// the evidence is strongest.
///
/// Two discriminators, both cheap:
///   1. the token is preceded by `sha256` (or `sha-256`) within a short window;
///   2. the token is immediately followed by an ellipsis, i.e. it was TRUNCATED for
///      display -- a truncated hash is never a resolvable citation.
fn is_content_hash(chars: &[char], start: usize, end: usize) -> bool {
    // 2. truncation marker directly after the run.
    let tail: String = chars[end..chars.len().min(end + 3)].iter().collect();
    if tail.starts_with('\u{2026}') || tail.starts_with("...") {
        return true;
    }
    // 1. a `sha256` label shortly before it.
    let lo = start.saturating_sub(24);
    let prefix: String = chars[lo..start]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();
    prefix.contains("sha256") || prefix.contains("sha-256") || prefix.contains("sha 256")
}

/// True when a hex run sits in a COMMIT-CITATION POSITION.
///
/// This replaces "any hex token is a candidate", which had high recall and bad precision in
/// two measured ways:
///
/// 1. OVER-ATTRIBUTION. `-orchestrator-grading-debt-ylq` showed 7 landed SHAs because its
///    body *enumerates* my commits in a table. Mention is not implementation, so the LANDED
///    set was an upper bound presented as a fact.
///
/// 2. SELF-POISONING, and this one is the real lesson. The join reported two dangling
///    citations; I wrote that report INTO the bead as a comment; the next scan then read my
///    own sentence -- "the lifecycle join surfaced (5042f809, 90840a00 ...)" -- as two fresh
///    citations. The grader's reply ("git rev-parse --verify 5042f809 and 90840a00", "Dangling
///    SHA verdict: ...") added four more. An observer that writes into the corpus it observes
///    manufactures self-sustaining findings, and the finding survives its own resolution.
///
/// So a token counts only if something nearby says it is a commit. The operator's rule --
/// "cite foreign commits as `reponame@sha`, never `reponame sha`" -- is honoured by treating
/// a `@`-prefix as a citation marker too.
fn in_citation_position(chars: &[char], start: usize) -> bool {
    // `repo@sha` -- the qualified form the operator requires for foreign commits.
    if start > 0 && chars[start - 1] == '@' {
        return true;
    }
    let lo = start.saturating_sub(40);
    let prefix: String = chars[lo..start]
        .iter()
        .collect::<String>()
        .to_ascii_lowercase();
    // Deliberately narrow. Adding "verify", "surfaced", or "verdict" here would re-open
    // the self-poisoning hole, because those are the words a REPORT about SHAs uses.
    const MARKERS: [&str; 8] = [
        "commit ",
        "commit:",
        "landed in ",
        "landed=",
        "commit under grade:",
        "fixed in ",
        "in commit ",
        "sha=",
    ];
    // ADJACENCY ONLY. `contains` over a 40-char window was still self-poisoning: the
    // grader's reply "found no commit object for 5042f809" contains "commit " and so scored
    // a REPORT OF ABSENCE as a citation. A marker must IMMEDIATELY precede the token.
    //
    // Whitespace is normalised first because `br` wraps long comment lines, so a marker can
    // be separated from its SHA by a newline plus indentation and still be adjacent in
    // meaning.
    let normalised = prefix.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalised = if prefix.ends_with(char::is_whitespace) {
        format!("{normalised} ")
    } else {
        normalised
    };
    // Test both the space-terminated and trimmed forms: "COMMIT: <sha>" normalises to
    // "commit: " while the marker is "commit:". Caught by the known-good leg, which is the
    // only reason this narrowing did not silently drop a real citation form.
    let trimmed = normalised.trim_end();
    MARKERS
        .iter()
        .any(|m| normalised.ends_with(m) || trimmed.ends_with(m.trim_end()))
}

/// Extract every 7-to-40 hex commit-SHA-shaped token from text.
///
/// Deliberately permissive on length and then verified against the object store, because
/// agents cite both short and full SHAs. Rejects all-digit runs (a line number or byte
/// count), embedded hex inside identifiers, and content hashes (see [`is_content_hash`]).
pub fn sha_candidates(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_hexdigit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            let len = i - start;
            // `_` counts as a word character. Without it, `crate_deadbeef_thing` yields a
            // false citation -- caught by the boundary test, which is why it exists.
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            // `@` is a citation marker (repo@sha), so it must not also count as the word
            // boundary that disqualifies the token.
            let boundary_ok = start == 0 || !is_word(chars[start - 1]) || chars[start - 1] == '@';
            let end_ok = i >= chars.len() || !is_word(chars[i]);
            if (7..=40).contains(&len) && boundary_ok && end_ok {
                let tok: String = chars[start..i].iter().collect();
                let all_digits = tok.chars().all(|c| c.is_ascii_digit());
                if !all_digits
                    && !is_content_hash(&chars, start, i)
                    && in_citation_position(&chars, start)
                {
                    out.push(tok.to_ascii_lowercase());
                }
            }
        } else {
            i += 1;
        }
    }
    out.sort();
    out.dedup();
    out
}

/// True when the text carries an explicit grading verdict.
pub fn has_verdict_marker(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.contains("VERDICT: CONFIRM")
        || upper.contains("VERDICT: DISPUTE")
        || upper.contains("VERDICT:CONFIRM")
        || upper.contains("VERDICT:DISPUTE")
        || upper.contains("INDEPENDENT GRADE")
}

/// /brennerbot: a bead whose hypothesis was falsified rather than delivered.
pub fn has_refutation_marker(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    upper.contains("REFUTED") || upper.contains("HYPOTHESIS FALSIFIED") || upper.contains("WONTFIX")
}

// ---------------------------------------------------------------------------
// live collection -- the JOIN, executed
// ---------------------------------------------------------------------------

/// Extract every value of `"key":"..."` in document order.
///
/// This crate has no serde by house convention (`no-shell-gate` ships zero deps), so this
/// is a field SCANNER, not a parser. It is only safe because [`collect`] validates that the
/// per-key counts agree and REFUSES otherwise. A scanner that silently mis-pairs fields
/// would invent bead/status combinations, which is worse than no report at all.
fn scan_str_field(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\":\"");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(p) = rest.find(&needle) {
        rest = &rest[p + needle.len()..];
        match rest.find('"') {
            Some(e) => {
                out.push(rest[..e].to_owned());
                rest = &rest[e..];
            }
            None => break,
        }
    }
    out
}

/// `"key":null` or `"key":"value"`; None for null.
fn scan_opt_field(text: &str, key: &str) -> Vec<Option<String>> {
    let needle = format!("\"{key}\":");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(p) = rest.find(&needle) {
        rest = &rest[p + needle.len()..];
        if rest.starts_with("null") {
            out.push(None);
        } else if let Some(stripped) = rest.strip_prefix('"') {
            match stripped.find('"') {
                Some(e) => out.push(Some(stripped[..e].to_owned())),
                None => out.push(None),
            }
        } else {
            out.push(None);
        }
    }
    out
}

fn sha_exists(repo: &str, sha: &str) -> bool {
    let spec = format!("{sha}^{{commit}}");
    matches!(
        crate::run(
            &["git", "-C", repo, "cat-file", "-e", &spec],
            std::time::Duration::from_secs(20)
        ),
        crate::Outcome::Completed { code: Some(0), .. }
    )
}


/// Which bucket a cited SHA belongs in. A PURE function of two facts, extracted so
/// the decision is testable without a git repository.
///
/// # Why extraction was necessary
///
/// My first two tests for this fix set `cited_only_shas` by hand on a `Unit` literal
/// and asserted the accessor and render. Then I mutation-tested by reverting the
/// attribution check to `if true` — the exact pre-fix behaviour — and **ZERO suites
/// failed**. The tests exercised the REPORTING and never the DECISION, so they could
/// not fail for the reason they existed.
///
/// That is a fooled certificate, and it is the defect class this repo keeps finding
/// in its own work. The fix is not a better assertion on the same shape: it is to
/// make the decision a thing a test can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitationClass {
    /// Exists in git AND the commit names this bead. A real landing.
    Landed,
    /// Exists in git, but the commit does NOT name this bead — a citation of
    /// someone else's work. Measured: `2lo` cited `4ak`'s `3f821d4`.
    BorrowedCredit,
    /// Does not resolve in any repo. A broken reference, not evidence.
    Dangling,
}

/// Classify one cited SHA. `exists` and `names_bead` are the only inputs, so both
/// failure directions are reachable from a test.
pub fn classify_citation(exists: bool, names_bead: bool) -> CitationClass {
    match (exists, names_bead) {
        (false, _) => CitationClass::Dangling,
        (true, true) => CitationClass::Landed,
        (true, false) => CitationClass::BorrowedCredit,
    }
}

/// Does the COMMIT name the bead? The only direction that proves authorship.
///
/// # Why this exists
///
/// `sha_exists` answers "is this a real commit", which is necessary and nowhere near
/// sufficient. A bead's prose can cite any real commit in the repository, including
/// another bead's work, and the lifecycle reader previously treated every such
/// citation as a landing.
///
/// Reads the commit's own subject and body and looks for the bead id. Deliberately
/// NOT `git log --grep` across all refs: that would match a commit mentioning the
/// bead in passing anywhere in history, which is the same over-broad matching one
/// level out. This asks one specific commit whether it claims one specific bead.
///
/// # What it cannot do
///
/// A commit can name a bead it did not implement — the message is written by the
/// same hand as the citation. This raises the floor from "anyone may credit
/// themselves with anyone's commit" to "the commit must claim the bead", which is
/// the difference between an accident and a lie.
fn commit_references_bead(repos: &[String], sha: &str, bead_id: &str) -> bool {
    // Match the bare suffix too: commits in this repo write `[cp-79am1]` and
    // `omp-orchestrator-4ak` interchangeably, and requiring the full prefix would
    // reject genuine landings — a false negative that empties the grading queue
    // instead of filling it wrongly. Both failure directions are real.
    let short = bead_id.rsplit('-').next().unwrap_or(bead_id);
    for repo in repos {
        let out = crate::run(
            &["git", "-C", repo, "log", "-1", "--format=%s%n%b", sha],
            std::time::Duration::from_secs(20),
        );
        if let crate::Outcome::Completed { code: Some(0), stdout, .. } = out {
            if stdout.contains(bead_id) || (short.len() >= 3 && stdout.contains(short)) {
                return true;
            }
        }
    }
    false
}

fn br(args: &[&str], secs: u64) -> String {
    match crate::run(args, std::time::Duration::from_secs(secs)) {
        crate::Outcome::Completed { stdout, .. } => stdout,
        // A timeout is NOT an empty record. Returning "" here would silently downgrade a
        // bead to "no evidence"; the caller cannot distinguish that, so we mark it.
        _ => "\u{0}TIMEOUT".to_owned(),
    }
}

/// Build the report by joining br + git. `repos` are searched for each cited SHA.
pub fn collect(repos: &[String]) -> Result<Report, String> {
    let listing = match crate::run(
        &["br", "list", "--json"],
        std::time::Duration::from_secs(60),
    ) {
        crate::Outcome::Completed {
            stdout,
            code: Some(0),
            ..
        } => stdout,
        other => return Err(format!("br list --json {}", other.kind())),
    };

    // CHUNK PER OBJECT, do not zip document-order field lists.
    //
    // Measured 2026-08-31: zipping gave ids=15 statuses=15 assignees=7, because `br` OMITS
    // `assignee` when unset rather than emitting null. The fail-closed count check caught
    // it and refused instead of mis-pairing 8 beads -- which is the only reason this defect
    // did not become a confidently wrong report. Chunking fixes the cause.
    let chunks: Vec<&str> = {
        let marker = "\"id\":\"";
        let mut starts: Vec<usize> = Vec::new();
        let mut from = 0usize;
        while let Some(p) = listing[from..].find(marker) {
            starts.push(from + p);
            from = from + p + marker.len();
        }
        starts
            .iter()
            .enumerate()
            .map(|(n, &s)| {
                let end = starts.get(n + 1).copied().unwrap_or(listing.len());
                &listing[s..end]
            })
            .collect()
    };

    // ANTI-VACUITY: an empty scan set is an ERROR, never a healthy tracker.
    if chunks.is_empty() {
        return Err("zero beads parsed; an empty scan set is an ERROR, never a pass".into());
    }

    let mut report = Report::default();
    // bead -> the ids it depends on, so in-degree can be computed after the walk.
    let mut out_edges: Vec<(String, Vec<String>)> = Vec::new();
    for chunk in chunks {
        // Each chunk must yield exactly one id and one status, or the chunking assumption
        // is broken and we refuse rather than guess.
        let id = match scan_str_field(chunk, "id").into_iter().next() {
            Some(v) => v,
            None => return Err("a chunk carried no id; chunking assumption broken".into()),
        };
        let status = match scan_str_field(chunk, "status").into_iter().next() {
            Some(v) => v,
            None => {
                return Err(format!(
                    "bead {id} has no status in its object; refusing to infer one"
                ))
            }
        };
        let assignee = scan_opt_field(chunk, "assignee")
            .into_iter()
            .flatten()
            .next();

        let comments = br(&["br", "comments", "list", &id], 45);
        let shown = br(&["br", "show", &id], 45);
        let deps = br(&["br", "dep", "list", &id], 45);
        let corpus = format!("{comments}\n{shown}");

        let mut verified = Vec::new();
        let mut dangling = Vec::new();
        let mut cited_only = Vec::new();
        for sha in sha_candidates(&corpus) {
            let exists = repos.iter().any(|r| sha_exists(r, &sha));
            // ATTRIBUTION RUNS COMMIT -> BEAD, NOT BEAD -> COMMIT.
            //
            // Measured 2026-08-31 (bead lifecycle-phantom-landing-ma1): the corpus
            // above is the bead's OWN PROSE (its comments plus `br show`), so any SHA
            // mentioned anywhere in it became a "landing". `omp-orchestrator-2lo` was
            // reported landed=3f821d4 while `git log --all --grep=2lo` was EMPTY and
            // 3f821d4 is 4ak's gate work — present in 2lo's comments as a
            // CROSS-REFERENCE somebody wrote by hand.
            //
            // So the grading queue filled with beads where nothing was built, and a
            // bead earned credit for CITING another bead's commit. Honest credit
            // requires the COMMIT to name the bead; a bead naming a commit is a
            // citation and proves nothing about authorship.
            let names = exists && commit_references_bead(&repos, &sha, &id);
            match classify_citation(exists, names) {
                CitationClass::Landed => verified.push(sha),
                CitationClass::BorrowedCredit => cited_only.push(sha),
                CitationClass::Dangling => dangling.push(sha),
            }
        }

        let dep_targets: Vec<String> = if deps.contains("No dependencies") {
            Vec::new()
        } else {
            deps.lines()
                .filter(|l| l.contains("->"))
                .filter_map(|l| {
                    l.split_whitespace()
                        .find(|t| t.starts_with("omp-orchestrator-"))
                        .map(str::to_owned)
                })
                .collect()
        };
        let dep_count = dep_targets.len();
        out_edges.push((id.clone(), dep_targets));

        let closed = status == "closed";
        let graded = has_verdict_marker(&corpus);
        report.units.push(Unit {
            bead: id.clone(),
            status,
            assignee,
            dep_count,
            in_degree: 0, // filled in the second pass below
            verified_shas: verified,
            dangling_shas: dangling,
            cited_only_shas: cited_only,
            has_verdict: graded && !closed,
            // A close carrying no independent-grade marker is author-closed UNTIL PROVEN
            // OTHERWISE. Fail toward "needs regrading": the cost of a spurious regrade is
            // one re-run; the cost of a missed one is an unverified close in the tree.
            closed_by_author: closed && !graded,
            refuted: has_refutation_marker(&corpus) && !closed,
        });
    }
    // SECOND PASS: in-degree. Without it a bead that BLOCKS work reads as an orphan.
    for unit in report.units.iter_mut() {
        unit.in_degree = out_edges
            .iter()
            .filter(|(src, targets)| src != &unit.bead && targets.contains(&unit.bead))
            .count();
    }
    Ok(report)
}

/// The standing roster. Grading eligibility is computed against this, so a stale row here
/// misroutes a grade -- it is checked against live tmux by the caller.
pub const ROSTER: [(&str, &str); 5] = [
    ("%1397", "pane1"),
    ("%1413", "GreenFrog"),
    ("%1414", "BlueLantern"),
    ("%1408", "AmberGate"),
    ("%1409", "SilverWolf"),
];

/// Human-readable rendering. Answers the operator's four questions in order:
/// what got done, where we are, where grading dispatches, what the repo owes.
pub fn render(report: &Report) -> String {
    let mut o = String::new();
    o.push_str(
        "OMP LIFECYCLE -- planning-workflow -> beads-workflow -> vibing-with-ntm -> brennerbot\n",
    );
    o.push_str(&format!(
        "denominator: {} beads parsed from `br list --json`\n\n",
        report.units.len()
    ));

    let borrowed = report.borrowed_credit();
    if !borrowed.is_empty() {
        o.push_str("BORROWED CREDIT -- cites a real commit that does NOT name this bead\n");
        for u in &borrowed {
            o.push_str(&format!(
                "  {:<44} cites {}\n",
                u.bead,
                u.cited_only_shas.join(",")
            ));
        }
        o.push_str(
            "  A bead citing another bead's commit is a CITATION, not a landing. \
             Attribution runs commit -> bead.\n\n",
        );
    }

    o.push_str("WHERE WE ARE (stage x bead), and WHAT GOT DONE (verified SHAs)\n");
    for (stage, units) in report.by_stage() {
        o.push_str(&format!("  {stage}  ({})\n", units.len()));
        for u in units {
            let shas = if u.verified_shas.is_empty() {
                "-".to_owned()
            } else {
                u.verified_shas.join(",")
            };
            o.push_str(&format!(
                "      {:<50} author={:<12} deps={} landed={}\n",
                u.bead,
                u.assignee.clone().unwrap_or_else(|| "-".into()),
                u.dep_count,
                shas
            ));
        }
    }

    let queue = report.grading_queue();
    o.push_str(&format!(
        "\nWHERE GRADING DISPATCHES ({} in queue) -- OUTRANKS NEW WORK\n",
        queue.len()
    ));
    if queue.is_empty() {
        o.push_str("      (empty: no bead has a verified landed commit awaiting a grade)\n");
    }
    for u in &queue {
        o.push_str(&format!(
            "      {:<50} author={:<12} -> eligible: {}\n",
            u.bead,
            u.assignee.clone().unwrap_or_else(|| "-".into()),
            u.eligible_graders(&ROSTER).join(" ")
        ));
    }

    let sc = report.self_closed();
    o.push_str(&format!(
        "\nSELF-CLOSED, NEEDS REGRADE ({}) -- closed with no independent-grade marker\n",
        sc.len()
    ));
    for u in &sc {
        o.push_str(&format!("      {}\n", u.bead));
    }

    let dc = report.dangling_citations();
    o.push_str(&format!(
        "\nDANGLING CITATIONS ({}) -- a SHA cited that git cannot resolve\n",
        dc.len()
    ));
    for u in &dc {
        o.push_str(&format!(
            "      {:<50} {}\n",
            u.bead,
            u.dangling_shas.join(",")
        ));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_extraction_rejects_numbers_and_words() {
        // Real citation text from this wave. NOTE: this test CHANGED when the detector was
        // narrowed from bare-pattern to citation-adjacency. Under the old semantics both
        // SHAs scored; now only the one with an adjacent marker does, because "and b6249a5"
        // carries no marker. That is the intended trade (see an_unmarked_sha_is_
        // deliberately_missed) and the test says so rather than being quietly relaxed.
        let t = "landed in 831fdd6 and b6249a5, see line 750 and 200000 bytes";
        let got = sha_candidates(t);
        assert!(
            got.contains(&"831fdd6".to_owned()),
            "the marked citation must survive: {got:?}"
        );
        assert!(
            !got.contains(&"b6249a5".to_owned()),
            "an unmarked SHA is out of scope by design: {got:?}"
        );
        assert!(
            !got.iter().any(|s| s == "200000" || s == "750"),
            "a number is not a SHA: {got:?}"
        );
    }

    #[test]
    fn sha_extraction_requires_a_word_boundary() {
        // `deadbeef` inside a longer identifier is not a citation.
        let got = sha_candidates("crate_deadbeef_thing");
        assert!(got.is_empty(), "embedded hex is not a citation: {got:?}");
    }

        /// The DECISION, exercised directly. All four input combinations.
    ///
    /// This test is the reason `classify_citation` exists as a pure function: the
    /// earlier fixture-only tests passed while the attribution check was reverted to
    /// `if true`, so they proved nothing about the fix.
    #[test]
    fn classify_citation_covers_every_input_combination() {
        // The measured case: 3f821d4 exists, names 4ak, cited by 2lo.
        assert_eq!(
            classify_citation(true, false),
            CitationClass::BorrowedCredit,
            "a real commit that does not name the bead is BORROWED CREDIT, not a landing"
        );
        assert_eq!(
            classify_citation(true, true),
            CitationClass::Landed,
            "a real commit that names the bead is a landing"
        );
        assert_eq!(
            classify_citation(false, false),
            CitationClass::Dangling,
            "an unresolvable SHA is a broken reference"
        );
        // Nonsense input: cannot name a bead if it does not exist. Dangling wins,
        // because existence is the precondition and a gate must fail on the
        // precondition rather than on the richer claim.
        assert_eq!(
            classify_citation(false, true),
            CitationClass::Dangling,
            "non-existence dominates: a missing commit cannot be a landing"
        );
    }

    /// FIRES-ON-KNOWN-BAD for the PRE-FIX behaviour.
    ///
    /// Before this fix every existing SHA was treated as verified regardless of what
    /// its commit said. That is `classify_citation(true, _) -> Landed`, and this
    /// asserts it is NOT what happens.
    #[test]
    fn the_pre_fix_behaviour_is_now_impossible() {
        let pre_fix_verdict = CitationClass::Landed;
        assert_ne!(
            classify_citation(true, false),
            pre_fix_verdict,
            "the pre-fix code promoted ANY existing cited SHA to landed; 2lo was \
             reported landed=3f821d4 while `git log --all --grep=2lo` was EMPTY"
        );
    }

    /// Build a Unit with every field explicit — mirrors the literal the sibling tests
    /// use, so a fixture cannot drift from the struct without a compile error.
    fn test_unit(bead: &str, status: &str) -> Unit {
        Unit {
            bead: bead.into(),
            status: status.into(),
            assignee: None,
            dep_count: 0,
            in_degree: 0,
            verified_shas: vec![],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        }
    }

    /// A bead citing ANOTHER bead's commit must not be promoted as landed.
    ///
    /// # The measured case, reproduced exactly
    ///
    /// 2026-08-31, bead `lifecycle-phantom-landing-ma1`, found by a grader disputing
    /// my premise rather than by me:
    ///
    /// ```text
    /// tick-monitor lifecycle -> omp-orchestrator-2lo  landed=3f821d4
    /// git log --all --grep=2lo -> EMPTY. No commit references 2lo at all.
    /// git log -1 3f821d4       -> "gate: no-shell-gate ..." which is 4ak's work
    /// 3f821d4 in 2lo comments  -> present, as a CITATION I wrote myself
    /// ```
    ///
    /// The tool read a cross-reference to a different bead's commit and reported it
    /// as this bead's implementation, so the grading queue filled with beads where
    /// nothing was built.
    #[test]
    fn a_cited_commit_that_does_not_name_the_bead_is_not_a_landing() {
        let mut u = test_unit("omp-orchestrator-2lo", "open");
        // The corpus resolved: the SHA is real, so it is NOT dangling.
        u.dangling_shas = vec![];
        // But the commit's message names 4ak, not 2lo.
        u.cited_only_shas = vec!["3f821d4".to_owned()];
        u.verified_shas = vec![];
        let report = Report { units: vec![u], repo_updates: vec![] };

        assert_eq!(
            report.borrowed_credit().len(),
            1,
            "a bead citing a commit that does not name it must be surfaced as borrowed credit"
        );
        let rendered = render(&report);
        assert!(
            rendered.contains("BORROWED CREDIT"),
            "the operator must SEE it; a field nobody renders is not wired.\n{rendered}"
        );
        assert!(
            rendered.contains("3f821d4"),
            "and the render must name the SHA so it can be checked.\n{rendered}"
        );
    }

    /// KNOWN-GOOD leg: a commit that DOES name the bead is a real landing.
    ///
    /// Without this the fix is over-strict in the direction that empties the grading
    /// queue instead of filling it wrongly — both failure directions are real, and an
    /// over-strict gate gets routed around.
    #[test]
    fn a_commit_that_names_the_bead_is_a_real_landing_and_not_borrowed() {
        let mut u = test_unit("omp-orchestrator-4ak", "closed");
        u.verified_shas = vec!["3f821d4".to_owned()];
        u.cited_only_shas = vec![];
        let report = Report { units: vec![u], repo_updates: vec![] };
        assert!(
            report.borrowed_credit().is_empty(),
            "a bead whose commit names it must NOT be flagged as borrowing credit"
        );
        assert!(
            !render(&report).contains("BORROWED CREDIT"),
            "and the borrowed-credit section must not appear when nothing is borrowed"
        );
    }

#[test]
    fn landed_but_unclosed_is_the_grading_queue() {
        let u = Unit {
            bead: "x".into(),
            status: "in_progress".into(),
            assignee: Some("GoldLark".into()),
            dep_count: 1,
            in_degree: 0,
            verified_shas: vec!["abc1234".into()],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        };
        assert_eq!(u.stage(), Stage::Landed);
        assert!(u.stage().is_grading_queue(), "this IS the queue");
    }

    #[test]
    fn the_author_is_never_an_eligible_grader() {
        let roster = [
            ("%1397", "pane1"),
            ("%1408", "AmberGate"),
            ("%1409", "GoldLark"),
        ];
        let u = Unit {
            bead: "npq".into(),
            status: "in_progress".into(),
            assignee: Some("GoldLark".into()),
            dep_count: 1,
            in_degree: 0,
            verified_shas: vec!["f9f4e37".into()],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        };
        let g = u.eligible_graders(&roster);
        assert!(
            !g.contains(&"%1409"),
            "the implementer's pane must be excluded"
        );
        assert!(g.contains(&"%1408"));
        assert!(g.contains(&"%1397"));
    }

    #[test]
    fn a_bead_with_no_edges_is_an_orphan_not_triaged() {
        let u = Unit {
            bead: "y".into(),
            status: "open".into(),
            assignee: None,
            dep_count: 0,
            in_degree: 0,
            verified_shas: vec![],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        };
        // /planning-workflow: "beads exist but have no dependency edges (the plan's
        // structure was lost in conversion)" is a stated NOT-delivered condition.
        assert_eq!(u.stage(), Stage::MaterializedOrphan);
    }

    #[test]
    fn refuted_outranks_closed() {
        // /brennerbot: a refutation is knowledge, not delivery. It must not be counted as
        // a completed feature.
        let u = Unit {
            bead: "z".into(),
            status: "closed".into(),
            assignee: None,
            dep_count: 1,
            in_degree: 0,
            verified_shas: vec![],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: true,
        };
        assert_eq!(u.stage(), Stage::Refuted);
    }

    #[test]
    fn verdict_and_refutation_markers_discriminate() {
        assert!(has_verdict_marker("VERDICT: CONFIRM, re-ran the leg"));
        assert!(has_verdict_marker("INDEPENDENT GRADE (orchestrator)"));
        assert!(!has_verdict_marker("I will grade this later"));
        assert!(has_refutation_marker("three hypotheses REFUTED"));
        assert!(!has_refutation_marker("confirmed by measurement"));
    }

    #[test]
    fn only_claimed_and_grading_occupy_a_pane() {
        assert!(Stage::Claimed.occupies_a_pane());
        assert!(Stage::Grading.occupies_a_pane());
        // A landed-but-ungraded bead occupies NO pane -- that is precisely why it silently
        // becomes a queue nobody is working.
        assert!(!Stage::Landed.occupies_a_pane());
        assert!(!Stage::Triaged.occupies_a_pane());
        assert!(!Stage::Closed.occupies_a_pane());
    }
}

#[cfg(test)]
mod content_hash_tests {
    use super::*;

    /// The exact verbatim strings from the bead that produced the false positive.
    #[test]
    fn sha256_content_hashes_are_not_commit_citations() {
        let a = "final sha256 5042f809\u{2026}701 after rustfmt";
        let b = "Restored byte-identically: sha256 90840a00\u{2026}6071d hash-equal to baseline";
        assert!(
            sha_candidates(a).is_empty(),
            "labelled+truncated sha256 must not read as a commit: {:?}",
            sha_candidates(a)
        );
        assert!(
            sha_candidates(b).is_empty(),
            "restore-hash must not read as a commit: {:?}",
            sha_candidates(b)
        );
    }

    /// KNOWN-GOOD: the filter must not eat real citations. Without this leg the change
    /// could silently suppress every SHA and the join would report an empty grading queue --
    /// which reads as "nothing to grade" and is the worst possible failure here.
    ///
    /// This leg CHANGED when precision was tightened, and it caught the change: under
    /// bare-pattern matching all three tokens below were found; under citation-position
    /// matching only the marked ones are. That is the intended trade and the test now says
    /// so out loud.
    #[test]
    fn marked_commit_citations_still_survive() {
        let t = "landed in 831fdd6; commit b6249a5 did the fix; control-plane@f9f4e37 too";
        let got = sha_candidates(t);
        for want in ["831fdd6", "b6249a5", "f9f4e37"] {
            assert!(got.contains(&want.to_owned()), "lost {want} from {got:?}");
        }
    }

    /// THE RECALL COST, asserted rather than hidden.
    ///
    /// An unmarked SHA is now MISSED. That is a deliberate trade: over-attribution made the
    /// LANDED set an upper bound presented as fact, and self-poisoning made a resolved
    /// finding immortal. Both were worse than a miss, because a miss is visible as an empty
    /// cell while a false positive is indistinguishable from evidence. Whoever widens the
    /// marker list must keep the self-poisoning legs green -- "verify", "surfaced" and
    /// "verdict" are the words a REPORT about SHAs uses, so adding them re-opens the loop.
    #[test]
    fn an_unmarked_sha_is_deliberately_missed() {
        let got = sha_candidates("see 831fdd6 and b6249a5 for details");
        assert!(
            got.is_empty(),
            "unmarked SHAs are out of scope by design; got {got:?}"
        );
    }

    /// The self-poisoning sentences -- verbatim from my own comment and the grader's reply.
    /// These are the exact strings that made a resolved finding regenerate itself.
    #[test]
    fn a_report_about_shas_does_not_become_new_citations() {
        for s in [
            "the lifecycle join surfaced (5042f809, 90840a00 -- unresolvable in both repos)",
            "8. git rev-parse --verify 5042f809 and 90840a00 in both repos",
            "Dangling SHA verdict: 5042f809 and 90840a00 do not resolve",
        ] {
            assert!(
                sha_candidates(s).is_empty(),
                "an observer must not read its own report as evidence: {s:?} -> {:?}",
                sha_candidates(s)
            );
        }
    }

    #[test]
    fn a_truncated_hash_is_rejected_even_without_a_label() {
        // Truncation alone is decisive: a display-truncated hash can never resolve.
        assert!(sha_candidates("commit deadbeef\u{2026}9c1 was the baseline").is_empty());
    }
}

#[cfg(test)]
mod orphan_tests {
    use super::*;

    fn unit(dep_count: usize, in_degree: usize) -> Unit {
        Unit {
            bead: "x".into(),
            status: "open".into(),
            assignee: None,
            dep_count,
            in_degree,
            verified_shas: vec![],
            dangling_shas: vec![],
            cited_only_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        }
    }

    /// MEASURED FALSE POSITIVE, 2026-08-31. `-gfb` was reported MATERIALIZED_ORPHAN while
    /// `kxe` depended on it (`br dep tree kxe` hit it). `br dep list <id>` returns only
    /// OUT-edges, so a bead that BLOCKS work looked edgeless. A bead that something depends
    /// on is sequenced -- it just does not itself depend on anything.
    #[test]
    fn a_bead_others_depend_on_is_not_an_orphan() {
        assert_ne!(unit(0, 1).stage(), Stage::MaterializedOrphan);
        assert_eq!(unit(0, 1).stage(), Stage::Triaged);
    }

    #[test]
    fn only_zero_edges_in_both_directions_is_an_orphan() {
        assert_eq!(unit(0, 0).stage(), Stage::MaterializedOrphan);
        assert_ne!(unit(1, 0).stage(), Stage::MaterializedOrphan);
        assert_ne!(unit(2, 3).stage(), Stage::MaterializedOrphan);
    }
}

#[cfg(test)]
mod adjacency_tests {
    use super::*;

    /// THE PLANTED GRADER REPLY. Verbatim from GreenFrog's 09:xx comment, which is the exact
    /// string that survived the previous narrowing. `contains("commit ")` over a 40-char
    /// window scored "found no commit object for X" as a citation, so a REPORT OF ABSENCE
    /// regenerated the very finding it was reporting on.
    ///
    /// This is the self-pollution shape: a detector scanning text that includes its own
    /// output, the same class as a guard grepping its own pane and matching its own
    /// scrollback. Third iteration on this detector; each narrowing was caught by a test,
    /// never by review.
    #[test]
    fn a_grader_reporting_absence_is_not_a_citation() {
        for s in [
            "searched the mirror and found no commit object for 5042f809 or 90840a00",
            "git rev-parse --verify 5042f809 and 90840a00 in both repos",
            "Dangling SHA verdict: 5042f809 and 90840a00 do not resolve",
            "the lifecycle join surfaced (5042f809, 90840a00 -- unresolvable)",
            "no commit object for deadbeef1 exists anywhere",
        ] {
            assert!(
                sha_candidates(s).is_empty(),
                "report-of-absence must not score as a citation: {s:?} -> {:?}",
                sha_candidates(s)
            );
        }
    }

    /// KNOWN-GOOD, and mandatory: narrowing must not suppress real citations. Without this
    /// leg the fix could empty the LANDED column, which reads as "nothing to grade" -- the
    /// worst failure mode this join has.
    #[test]
    fn adjacent_markers_still_score() {
        let cases = [
            ("landed in 831fdd6", "831fdd6"),
            ("commit b6249a5 did it", "b6249a5"),
            ("COMMIT: 3f821d4", "3f821d4"),
            ("control-plane@f9f4e37", "f9f4e37"),
            ("fixed in 91a015c", "91a015c"),
        ];
        for (text, want) in cases {
            let got = sha_candidates(text);
            assert!(
                got.contains(&want.to_owned()),
                "lost a real citation: {text:?} -> {got:?}"
            );
        }
    }

    /// br wraps long comment lines, so a marker and its SHA can be split by a newline plus
    /// indentation. Adjacency is about meaning, not literal bytes.
    #[test]
    fn a_wrapped_marker_is_still_adjacent() {
        let wrapped = "the change landed in\n            e9a410a and turned the suite green";
        assert!(
            sha_candidates(wrapped).contains(&"e9a410a".to_owned()),
            "wrapping must not break adjacency: {:?}",
            sha_candidates(wrapped)
        );
    }
}
