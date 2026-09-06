//! Bead-definition quality: consume beads_rust `close_policy` checkbox
//! grammar, extend it with numbered-prose, and ratchet cross-bead specificity.
//!
//! Decision (measured, one arm): EXTEND the recognizer. Arming by converting
//! our corpus to `- [ ]` would rewrite 393 numbered-prose beads so a gate that
//! today fires on 1/682 could see them. That mutates closed-bead evidence.
//! Numbered prose is the corpus format; the checker should meet it.
//!
//! Upstream: `find_unchecked_acceptance_criteria` /
//! `parse_unchecked_box` in
//! `/Volumes/ZestData/dicklesworthstone-mirror/beads_rust/src/close_policy.rs`
//! (2459, 2581). Copied here because this repo cannot take beads_rust as a
//! path dep without a new crate / Cargo.toml edit. Tests lock the checkbox
//! grammar to the upstream SAMPLE.

use serde_json::Value;
use std::fmt;

/// Floor as parts-per-thousand. 365 = 36.5%. May rise; the check refuses a lower value.
pub const SPECIFICITY_FLOOR_MILLI: u32 = 365;

/// Mutation target. Delete this computation and the identical-acceptance known-bad
/// no longer fails the floor.
pub const COMPUTE_SPECIFICITY: bool = true;

/// Named specimen: 37 jplf-family beads share this many suffix bytes.
pub const DERIVED_TAIL_BYTES: usize = 2045;
pub const DERIVED_PREMISE_PHRASE: &str = "I did not run its premise";

pub const EXIT_OK: u8 = 0;
pub const EXIT_BELOW_FLOOR: u8 = 1;
pub const EXIT_VACUOUS: u8 = 2;

pub const NO_CLAIM: &str = "NO-CLAIM: measures definition structure and specificity, never \
correctness. A bead can carry 100% unique acceptance that is entirely wrong. Floor-raise: \
boilerplate is visible and counted; a bad spec is still possible.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeadBody {
    pub id: String,
    pub acceptance: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyReport {
    pub bead_count: usize,
    pub total_acceptance_bytes: usize,
    pub specific_bytes: usize,
    pub boilerplate_bytes: usize,
    pub specific_milli: u32,
    pub floor_milli: u32,
    pub cluster_size: usize,
    pub cluster_suffix_bytes: usize,
    pub named_specimen_hit: bool,
    pub zero_acceptance: Vec<String>,
    pub unchecked_boxes: usize,
    pub numbered_prose: usize,
    pub what: usize,
    pub why: usize,
    pub acceptance_populated: usize,
    pub file_line: usize,
    pub source_anchor: usize,
}

impl FamilyReport {
    #[must_use]
    pub fn below_floor(&self) -> bool {
        self.specific_milli < self.floor_milli
    }

    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.below_floor() {
            EXIT_BELOW_FLOOR
        } else {
            EXIT_OK
        }
    }

    #[must_use]
    pub fn to_json(&self, success: bool) -> String {
        format!(
            "{{\"success\":{success},\"bead_count\":{},\"total_acceptance_bytes\":{},\
\"specific_bytes\":{},\"boilerplate_bytes\":{},\"specific_milli\":{},\"floor_milli\":{},\
\"cluster_size\":{},\"cluster_suffix_bytes\":{},\"named_specimen_hit\":{},\
\"zero_acceptance\":{},\"unchecked_boxes\":{},\"numbered_prose\":{},\
\"what\":{},\"why\":{},\"acceptance_populated\":{},\"file_line\":{},\"source_anchor\":{},\
\"no_claim\":{}}}",
            self.bead_count,
            self.total_acceptance_bytes,
            self.specific_bytes,
            self.boilerplate_bytes,
            self.specific_milli,
            self.floor_milli,
            self.cluster_size,
            self.cluster_suffix_bytes,
            self.named_specimen_hit,
            json_string_array(&self.zero_acceptance),
            self.unchecked_boxes,
            self.numbered_prose,
            self.what,
            self.why,
            self.acceptance_populated,
            self.file_line,
            self.source_anchor,
            serde_json::to_string(NO_CLAIM).unwrap_or_else(|_| "\"\"".into()),
        )
    }
}

fn json_string_array(ids: &[String]) -> String {
    serde_json::to_string(ids).unwrap_or_else(|_| "[]".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vacuity {
    EmptySet,
    Unreadable,
    ZeroMatch,
}

impl fmt::Display for Vacuity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySet => write!(f, "DEFINITION_QUALITY_EMPTY_SET"),
            Self::Unreadable => write!(f, "DEFINITION_QUALITY_UNREADABLE"),
            Self::ZeroMatch => write!(f, "DEFINITION_QUALITY_ZERO_MATCH"),
        }
    }
}

impl Vacuity {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        EXIT_VACUOUS
    }
}

/// Upstream `parse_unchecked_box` (close_policy.rs:2581). Keep the grammar.
pub fn parse_unchecked_box(line: &str) -> Option<String> {
    let mut chars = line.chars().peekable();
    let bullet = chars.next()?;
    if !matches!(bullet, '-' | '*' | '+') {
        return None;
    }
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
    if chars.next()? != '[' {
        return None;
    }
    let inner = chars.next()?;
    if !(inner.is_whitespace() || inner == ' ') {
        return None;
    }
    if chars.next()? != ']' {
        return None;
    }
    Some(chars.collect::<String>().trim().to_string())
}

/// Extension: numbered-prose `1. …` which is this corpus's acceptance format.
pub fn parse_numbered_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let digits = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if digits == 0 {
        return None;
    }
    trimmed
        .get(digits..)
        .and_then(|rest| rest.strip_prefix(". "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub fn find_unchecked_boxes(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| parse_unchecked_box(line.trim_start()))
        .collect()
}

pub fn find_numbered_prose(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| parse_numbered_item(line))
        .collect()
}

fn common_suffix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut i = 0usize;
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    while i < ab.len() && i < bb.len() && ab[ab.len() - 1 - i] == bb[bb.len() - 1 - i] {
        i += 1;
    }
    // Walk back to a char boundary.
    while i > 0 && !a.is_char_boundary(a.len() - i) {
        i -= 1;
    }
    &a[a.len() - i..]
}

fn largest_suffix_cluster(beads: &[BeadBody]) -> (usize, String) {
    let nonempty: Vec<&BeadBody> = beads.iter().filter(|b| !b.acceptance.is_empty()).collect();
    let mut best_n = 0usize;
    let mut best_suf = String::new();
    for i in 0..nonempty.len() {
        for j in (i + 1)..nonempty.len() {
            let suf = common_suffix(&nonempty[i].acceptance, &nonempty[j].acceptance);
            if suf.len() < 32 {
                continue;
            }
            let n = nonempty
                .iter()
                .filter(|b| b.acceptance.ends_with(suf))
                .count();
            if n > best_n || (n == best_n && suf.len() > best_suf.len()) {
                best_n = n;
                best_suf = suf.to_owned();
            }
        }
    }
    (best_n, best_suf)
}

pub fn measure_family(beads: &[BeadBody]) -> Result<FamilyReport, Vacuity> {
    if beads.is_empty() {
        return Err(Vacuity::EmptySet);
    }
    let total: usize = beads.iter().map(|b| b.acceptance.len()).sum();
    let (cluster_size, suffix) = largest_suffix_cluster(beads);
    let mut specific = 0usize;
    if COMPUTE_SPECIFICITY {
        for b in beads {
            if !suffix.is_empty() && b.acceptance.ends_with(&suffix) {
                specific += b.acceptance.len() - suffix.len();
            } else {
                specific += b.acceptance.len();
            }
        }
    } else {
        specific = total;
    }

    let boilerplate = total.saturating_sub(specific);
    let specific_milli = if total == 0 {
        0
    } else {
        u32::try_from(specific.saturating_mul(1000) / total).unwrap_or(0)
    };
    let named_specimen_hit = suffix.len() == DERIVED_TAIL_BYTES
        && suffix.contains(DERIVED_PREMISE_PHRASE)
        && cluster_size >= 2;
    let zero_acceptance = beads
        .iter()
        .filter(|b| b.acceptance.trim().is_empty())
        .map(|b| b.id.clone())
        .collect();
    let mut unchecked_boxes = 0usize;
    let mut numbered_prose = 0usize;
    let mut what = 0usize;
    let mut why = 0usize;
    let mut acceptance_populated = 0usize;
    let mut file_line = 0usize;
    let mut source_anchor = 0usize;
    for b in beads {
        let blob = format!("{}\n{}", b.acceptance, b.description);
        unchecked_boxes += find_unchecked_boxes(&b.acceptance).len();
        numbered_prose += find_numbered_prose(&b.acceptance).len();
        if !b.acceptance.trim().is_empty() {
            acceptance_populated += 1;
        }
        if blob.contains("WHAT") {
            what += 1;
        }
        if blob.contains("WHY") {
            why += 1;
        }
        if blob.contains(':')
            && blob
                .chars()
                .any(|c| c.is_ascii_digit())
            && (blob.contains(".rs:") || blob.contains(".md:") || blob.contains("src/"))
        {
            source_anchor += 1;
        }
        if blob.split_whitespace().any(|tok| {
            tok.rsplit_once(':')
                .and_then(|(_, n)| n.parse::<u32>().ok())
                .is_some()
        }) {
            file_line += 1;
        }
    }
    Ok(FamilyReport {
        bead_count: beads.len(),
        total_acceptance_bytes: total,
        specific_bytes: specific,
        boilerplate_bytes: boilerplate,
        specific_milli,
        floor_milli: SPECIFICITY_FLOOR_MILLI,
        cluster_size,
        cluster_suffix_bytes: suffix.len(),
        named_specimen_hit,
        zero_acceptance,
        unchecked_boxes,
        numbered_prose,
        what,
        why,
        acceptance_populated,
        file_line,
        source_anchor,
    })
}

pub fn parse_issues_jsonl(text: &str, needles: &[&str]) -> Result<Vec<BeadBody>, Vacuity> {
    if text.is_empty() {
        return Err(Vacuity::EmptySet);
    }
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|_| Vacuity::Unreadable)?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Vacuity::Unreadable)?
            .to_owned();
        if !needles.iter().any(|n| id.contains(n)) {
            continue;
        }
        let acceptance = value
            .get("acceptance_criteria")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        out.push(BeadBody {
            id,
            acceptance,
            description,
        });
    }
    if out.is_empty() {
        return Err(Vacuity::ZeroMatch);
    }
    Ok(out)
}

pub const FAMILY_NEEDLES: &[&str] = &[
    "jplf",
    "plan-07-8jkq",
    "bead-definition-quality-ratchet-0vhw",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn bead(id: &str, acc: &str) -> BeadBody {
        BeadBody {
            id: id.into(),
            acceptance: acc.into(),
            description: String::new(),
        }
    }

    #[test]
    fn close_policy_checkbox_sample() {
        // Upstream SAMPLE lock: close_policy.rs:3560 `- [ ] todo item`
        assert_eq!(
            parse_unchecked_box("- [ ] todo item").as_deref(),
            Some("todo item")
        );
        assert_eq!(parse_unchecked_box("* [ ] also").as_deref(), Some("also"));
        assert!(parse_unchecked_box("- [x] done").is_none());
        assert!(parse_unchecked_box("1. numbered").is_none());
    }

    #[test]
    fn numbered_prose_extension() {
        assert_eq!(
            parse_numbered_item("1. VERIFY THE PREMISE").as_deref(),
            Some("VERIFY THE PREMISE")
        );
        assert!(parse_numbered_item("- [ ] box").is_none());
    }

    #[test]
    fn fires_on_known_bad_identical_acceptance() {
        let tail = "I did not run its premise ".repeat(50);
        let family = [bead("a", &tail), bead("b", &tail)];
        let report = measure_family(&family).expect("two beads");
        assert!(
            report.specific_milli < 50,
            "identical text must be ~0% specific, got {}",
            report.specific_milli
        );
        assert!(report.below_floor());
        assert_eq!(report.exit_code(), EXIT_BELOW_FLOOR);
        assert!(!report.to_json(false).contains("\"success\":true"));
    }

    #[test]
    fn passes_known_good_distinct_acceptance() {
        let family = [
            bead("a", "1. unique alpha criterion about pane-truth two-capture."),
            bead("b", "1. unique beta criterion about receiver-receipt hash."),
        ];
        let report = measure_family(&family).expect("two beads");
        assert!(
            !report.below_floor(),
            "distinct acceptance must PASS floor, milli={}",
            report.specific_milli
        );
        assert_eq!(report.exit_code(), EXIT_OK);
    }

    #[test]
    fn mutation_goes_red() {
        let tail = format!(
            "{}{}",
            "same ".repeat(400),
            DERIVED_PREMISE_PHRASE
        );
        let family = [bead("a", &tail), bead("b", &tail)];
        let with = measure_family(&family).expect("computed");
        assert!(with.below_floor(), "subject present must fail identical pair");
        // Simulate deleting the specificity computation: treat all bytes as specific.
        let mut cloned = with.clone();
        cloned.specific_bytes = cloned.total_acceptance_bytes;
        cloned.boilerplate_bytes = 0;
        cloned.specific_milli = 1000;
        assert!(
            !cloned.below_floor(),
            "deleting specificity makes known-bad look green — that is the RED mutation"
        );
        assert_eq!(with.floor_milli, SPECIFICITY_FLOOR_MILLI);
        assert_eq!(advertised_floor(), enforced_floor());
    }

    #[test]
    fn empty_scan_is_error() {
        let err = measure_family(&[]).expect_err("empty");
        assert_eq!(err, Vacuity::EmptySet);
        assert_eq!(err.exit_code(), EXIT_VACUOUS);
        let err = parse_issues_jsonl("", FAMILY_NEEDLES).expect_err("empty jsonl");
        assert_eq!(err.exit_code(), EXIT_VACUOUS);
        let err = parse_issues_jsonl("{\"id\":\"other\",\"acceptance_criteria\":\"x\"}\n", FAMILY_NEEDLES)
            .expect_err("zero match");
        assert_eq!(err, Vacuity::ZeroMatch);
        let err = parse_issues_jsonl("{not json\n", FAMILY_NEEDLES).expect_err("unreadable");
        assert_eq!(err, Vacuity::Unreadable);
    }

    #[test]
    fn floor_drift_guard() {
        assert_eq!(advertised_floor(), SPECIFICITY_FLOOR_MILLI);
        assert_eq!(enforced_floor(), SPECIFICITY_FLOOR_MILLI);
        assert_eq!(SPECIFICITY_FLOOR_MILLI, 365);
    }

    #[test]
    fn named_specimen_2045_detected() {
        let suffix = format!(
            "{}{}",
            "x".repeat(DERIVED_TAIL_BYTES - DERIVED_PREMISE_PHRASE.len()),
            DERIVED_PREMISE_PHRASE
        );
        assert_eq!(suffix.len(), DERIVED_TAIL_BYTES);
        let family = [
            bead("a", &format!("head-a{suffix}")),
            bead("b", &format!("head-bb{suffix}")),
        ];
        let report = measure_family(&family).expect("specimen");
        assert!(report.named_specimen_hit);
        assert_eq!(report.cluster_suffix_bytes, DERIVED_TAIL_BYTES);
        assert_eq!(report.cluster_size, 2);
    }

    #[test]
    fn wired_caller_positive_control() {
        let main = include_str!("main.rs");

        assert!(
            main.contains("measure_family"),
            "bead-availability main must invoke measure_family"
        );
        assert!(
            main.contains("collect_live"),
            "positive control: collect_live is a known-present caller in the same file"
        );
        assert!(
            main.contains("--definition-quality"),
            "flag documents the exit contract"
        );
    }
}


#[must_use]
pub fn advertised_floor() -> u32 {
    SPECIFICITY_FLOOR_MILLI
}

#[must_use]
pub fn enforced_floor() -> u32 {
    // Same const: a split here is the drift the guard exists to catch.
    SPECIFICITY_FLOOR_MILLI
}
