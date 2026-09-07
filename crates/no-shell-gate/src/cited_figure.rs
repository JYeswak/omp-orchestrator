//! Refuse a cited test figure that does not state its SCOPE.
//!
//! Bead `omp-orchestrator-mmt4`, item 3. Ruling of 2026-09-07:
//!
//!   1. an ACCEPTANCE leg MUST cite a NAMED TARGET (`--test <target>`) -- it cannot truncate
//!   2. a CRATE-HEALTH claim MUST carry `--no-fail-fast` AND the target denominator
//!   3. a bare `cargo test -p <crate>` figure is INADMISSIBLE as evidence
//!   4. a `--no-fail-fast` failure count is a COUNT, not a DEFECT count
//!
//! WHY A FIGURE NEEDS ITS SCOPE. `cargo test` runs test TARGETS in order and stops at the
//! first one that fails, so a per-crate aggregate is a PREFIX of the suite and WHICH prefix
//! depends on which target fails first -- which is environment-dependent. Measured on
//! `no-shell-gate`: 33 passed / 3 failed locally over 11 targets, 23 passed / 2 failed on the
//! lane over 9, and 251 passed / 55 failed over 52 with `--no-fail-fast`. One command, three
//! answers, all real. A bare `N passed / M failed` cannot say which it is.
//!
//! THIS IS A REPORTING INSTRUMENT OVER LIVE DATA AND A GATE ONLY OVER FIXTURES, deliberately.
//! Measured 2026-09-07: the tracker holds 601 cited figures of which 351 are bare. Gating that
//! would be RED BY CONSTRUCTION on history nobody can retroactively re-run, and a gate that is
//! red by construction gets routed around -- the failure mode this repo has measured three
//! times. So [`scan_jsonl`] REPORTS with denominators and the refusal legs run on fixtures.
//! Enforcement of NEW citations belongs at the close boundary, not here, and is not built.

/// One cited pass/fail figure and whether it stated its scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigureCitation {
    /// Bead the citation was found in.
    pub bead: String,
    /// Which surface: `close_reason`, `description`, or `comment[N]`.
    pub field: String,
    /// The matched figure, verbatim.
    pub figure: String,
    /// True when the surrounding window names a target, a denominator, or `--no-fail-fast`.
    pub denominated: bool,
}

impl std::fmt::Display for FigureCitation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "CITED_FIGURE_WITHOUT_SCOPE bead={} field={} figure={:?} \
             required=one of `--test <target>` | `--no-fail-fast` + `across N targets` | `--lib`",
            self.bead, self.field, self.figure
        )
    }
}

/// How much text either side of a figure may carry its scope.
///
/// 260 chars, chosen because a real citation states the command on the line above or the
/// per-target split on the line below; a whole-comment window would let one `--test` anywhere
/// in a 6 KB report launder every bare figure in it.
const WINDOW: usize = 260;

/// Find `N passed ... M failed` in `text`. Tolerates `/`, `;`, and a newline between them,
/// because all three spellings appear in the tracker.
fn figures(text: &str) -> Vec<(usize, usize, String)> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let rest: String = bytes[index..].iter().take(40).collect();
        let lower = rest.to_ascii_lowercase();
        if !lower.starts_with(" passed") {
            continue;
        }
        // Walk to the next digit run and require "failed" after it.
        let mut cursor = index + " passed".len();
        while cursor < bytes.len() && !bytes[cursor].is_ascii_digit() {
            if !matches!(bytes[cursor], ' ' | ';' | ',' | '/' | '\n' | '\r' | '\t') {
                break;
            }
            cursor += 1;
        }
        if cursor >= bytes.len() || !bytes[cursor].is_ascii_digit() {
            continue;
        }
        let mut end = cursor;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let tail: String = bytes[end..].iter().take(20).collect();
        if !tail.to_ascii_lowercase().trim_start().starts_with("failed") {
            continue;
        }
        let stop = (end + 7).min(bytes.len());
        out.push((start, stop, bytes[start..stop].iter().collect()));
        index = stop;
    }
    out
}

/// True when `window` names a target, a stated denominator, or `--no-fail-fast`.
fn states_scope(window: &str) -> bool {
    let lower = window.to_ascii_lowercase();
    lower.contains("--test ")
        || lower.contains("--no-fail-fast")
        || lower.contains("--lib")
        || lower.contains("targets=")
        || lower.contains("target=")
        || (lower.contains("across ") && lower.contains(" target"))
        || lower.contains("target denominator")
}

/// Scan one text surface for cited figures.
#[must_use]
pub fn scan_blob(bead: &str, field: &str, text: &str) -> Vec<FigureCitation> {
    let chars: Vec<char> = text.chars().collect();
    figures(text)
        .into_iter()
        .map(|(start, stop, figure)| {
            let lo = start.saturating_sub(WINDOW);
            let hi = (stop + WINDOW).min(chars.len());
            let window: String = chars[lo..hi].iter().collect();
            FigureCitation {
                bead: bead.to_owned(),
                field: field.to_owned(),
                figure,
                denominated: states_scope(&window),
            }
        })
        .collect()
}

/// Scan a `.beads/issues.jsonl` mirror.
///
/// RESTRICTIVE: an empty or record-free mirror is an `Err`, never an empty success. An empty
/// scan set reports identically to a complete one that found nothing.
pub fn scan_jsonl(jsonl: &str) -> Result<Vec<FigureCitation>, String> {
    let mut rows = 0usize;
    let mut found = Vec::new();
    for (index, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        rows += 1;
        let value: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            format!("CITED_FIGURE_SCAN_UNREADABLE reason=malformed_jsonl line={index} detail={error}")
        })?;
        let bead = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        for field in ["close_reason", "description"] {
            if let Some(text) = value.get(field).and_then(serde_json::Value::as_str) {
                found.extend(scan_blob(bead, field, text));
            }
        }
        if let Some(comments) = value.get("comments").and_then(serde_json::Value::as_array) {
            for (position, comment) in comments.iter().enumerate() {
                if let Some(text) = comment.get("text").and_then(serde_json::Value::as_str) {
                    found.extend(scan_blob(bead, &format!("comment[{position}]"), text));
                }
            }
        }
    }
    if rows == 0 {
        return Err("CITED_FIGURE_SCAN_EMPTY reason=zero_bead_records_readable".to_owned());
    }
    Ok(found)
}

/// The split, with both denominators, so a reader can never see a bare ratio.
#[must_use]
pub fn summarise(rows: &[FigureCitation]) -> String {
    let bare = rows.iter().filter(|row| !row.denominated).count();
    format!(
        "CITED_FIGURE_CENSUS figures={} denominated={} bare={}",
        rows.len(),
        rows.len() - bare,
        bare
    )
}
