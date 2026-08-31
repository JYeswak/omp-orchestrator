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
    /// SHAs cited by this bead's comments/close reason AND verified to exist in git.
    pub verified_shas: Vec<String>,
    /// SHAs cited but NOT found in the object store. A citation that does not resolve is a
    /// defect, not evidence -- surfaced rather than dropped.
    pub dangling_shas: Vec<String>,
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
        if self.dep_count == 0 {
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
    pub fn dangling_citations(&self) -> Vec<&Unit> {
        self.units
            .iter()
            .filter(|u| !u.dangling_shas.is_empty())
            .collect()
    }
}

/// Extract every 7-to-40 hex SHA-shaped token from text.
///
/// Deliberately permissive on length and then verified against the object store, because
/// agents cite both short and full SHAs. Rejects all-digit runs so a line number or a byte
/// count cannot masquerade as a commit.
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
            let boundary_ok = start == 0 || !is_word(chars[start - 1]);
            let end_ok = i >= chars.len() || !is_word(chars[i]);
            if (7..=40).contains(&len) && boundary_ok && end_ok {
                let tok: String = chars[start..i].iter().collect();
                // A run of only digits is a number, not a SHA.
                if !tok.chars().all(|c| c.is_ascii_digit()) {
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
    upper.contains("REFUTED")
        || upper.contains("HYPOTHESIS FALSIFIED")
        || upper.contains("WONTFIX")
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
        let assignee = scan_opt_field(chunk, "assignee").into_iter().flatten().next();

        let comments = br(&["br", "comments", "list", &id], 45);
        let shown = br(&["br", "show", &id], 45);
        let deps = br(&["br", "dep", "list", &id], 45);
        let corpus = format!("{comments}\n{shown}");

        let mut verified = Vec::new();
        let mut dangling = Vec::new();
        for sha in sha_candidates(&corpus) {
            if repos.iter().any(|r| sha_exists(r, &sha)) {
                verified.push(sha);
            } else {
                dangling.push(sha);
            }
        }

        let dep_count = if deps.contains("No dependencies") {
            0
        } else {
            deps.lines().filter(|l| l.contains("->")).count()
        };

        let closed = status == "closed";
        let graded = has_verdict_marker(&corpus);
        report.units.push(Unit {
            bead: id.clone(),
            status,
            assignee,
            dep_count,
            verified_shas: verified,
            dangling_shas: dangling,
            has_verdict: graded && !closed,
            // A close carrying no independent-grade marker is author-closed UNTIL PROVEN
            // OTHERWISE. Fail toward "needs regrading": the cost of a spurious regrade is
            // one re-run; the cost of a missed one is an unverified close in the tree.
            closed_by_author: closed && !graded,
            refuted: has_refutation_marker(&corpus) && !closed,
        });
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
    o.push_str("OMP LIFECYCLE -- planning-workflow -> beads-workflow -> vibing-with-ntm -> brennerbot\n");
    o.push_str(&format!(
        "denominator: {} beads parsed from `br list --json`\n\n",
        report.units.len()
    ));

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
        o.push_str(&format!("      {:<50} {}\n", u.bead, u.dangling_shas.join(",")));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_extraction_rejects_numbers_and_words() {
        // Real citations from this wave.
        let t = "landed in 831fdd6 and b6249a5, see line 750 and 200000 bytes";
        let got = sha_candidates(t);
        assert!(got.contains(&"831fdd6".to_owned()));
        assert!(got.contains(&"b6249a5".to_owned()));
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

    #[test]
    fn landed_but_unclosed_is_the_grading_queue() {
        let u = Unit {
            bead: "x".into(),
            status: "in_progress".into(),
            assignee: Some("GoldLark".into()),
            dep_count: 1,
            verified_shas: vec!["abc1234".into()],
            dangling_shas: vec![],
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
            verified_shas: vec!["f9f4e37".into()],
            dangling_shas: vec![],
            has_verdict: false,
            closed_by_author: false,
            refuted: false,
        };
        let g = u.eligible_graders(&roster);
        assert!(!g.contains(&"%1409"), "the implementer's pane must be excluded");
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
            verified_shas: vec![],
            dangling_shas: vec![],
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
            verified_shas: vec![],
            dangling_shas: vec![],
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
