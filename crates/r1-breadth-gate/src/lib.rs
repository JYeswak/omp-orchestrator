#![forbid(unsafe_code)]

//! R1 breadth-before-depth (`d81g`).
//!
//! HD-0014 made `max(maturity) - median(maturity) <= 1` binding. Nothing computed
//! it, and the denominator was unpinned so the verdict was choosable. This crate
//! reads the pin from CONTRACT.md, scores every pinned box, and refuses extras
//! on disk that the pin omits.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const LEVEL_NAMES: [&str; 6] = [
    "MAPPED",
    "GROUNDED",
    "CONTRACTED",
    "FALSIFIABLE",
    "COMPILABLE",
    "FROZEN",
];

const PIN_NEEDLE: &str = "R1_POPULATION_BOXES=";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectScore {
    pub id: String,
    pub path: String,
    pub level: u8,
    pub level_name: &'static str,
    pub citation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub scores: Vec<SubjectScore>,
    pub max: u8,
    pub median: u8,
    pub delta: u8,
}

impl Report {
    pub fn render(&self) -> String {
        let mut out = String::new();
        for s in &self.scores {
            out.push_str(&format!(
                "{} level={} {} cite={}\n",
                s.id, s.level, s.level_name, s.citation
            ));
        }
        out.push_str(&format!(
            "R1 max={} median={} delta={}\n",
            self.max, self.median, self.delta
        ));
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    PopulationUnpinned,
    UnpinnedSubject { id: String, path: String },
    ScoreUncited { id: String },
    DeltaExceeded {
        max: u8,
        median: u8,
        delta: u8,
        table: String,
    },
    Io { detail: String },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PopulationUnpinned => write!(
                f,
                "ERR_POPULATION_UNPINNED missing {PIN_NEEDLE} in CONTRACT.md — refusing to default a denominator"
            ),
            Self::UnpinnedSubject { id, path } => write!(
                f,
                "ERR_UNPINNED_SUBJECT id={id} path={path} — on disk but omitted from {PIN_NEEDLE}"
            ),
            Self::ScoreUncited { id } => {
                write!(f, "ERR_SCORE_UNCITED id={id} — a score with no file:line")
            }
            Self::DeltaExceeded {
                max,
                median,
                delta,
                table,
            } => write!(
                f,
                "{table}ERR_R1_DELTA max={max} median={median} delta={delta} > 1 and no unexpired CRITICAL_PATH_EXCEPTION"
            ),
            Self::Io { detail } => write!(f, "ERR_R1_IO {detail}"),
        }
    }
}

/// Repo-relative paths this gate READS. ONE list, so the reader and the attributor cannot
/// drift apart on what the gate's inputs are.
pub const CONTRACT_PATH: &str = "docs/plan/flow/CONTRACT.md";
/// Directory holding one `<id>.toml` per pinned subject.
pub const BOXES_DIR: &str = "docs/plan/flow/boxes/";
/// Directory holding the numbered plan markdown this gate also scores.
pub const PLAN_DIR: &str = "docs/plan/";

/// True for a `NN-name.md` numbered plan file NAME (not a path).
///
/// Extracted so [`list_numbered_plan`] and [`is_gate_input`] apply the SAME predicate. Two
/// copies of a membership rule drift, and a drifted attributor would call a staged input
/// foreign -- which is the one direction that must never happen.
pub fn is_numbered_plan_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    name.len() >= 6
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'-'
        && name.ends_with(".md")
}

/// True when `path` (repo-relative, as `git diff --cached --name-only` prints it) is one of
/// this gate's INPUTS.
pub fn is_gate_input(path: &str) -> bool {
    if path == CONTRACT_PATH || path.starts_with(BOXES_DIR) {
        return true;
    }
    match path.strip_prefix(PLAN_DIR) {
        // Only the numbered plan files at the TOP of docs/plan are scored; a nested
        // docs/plan/<sub>/NN-x.md is not read by `list_numbered_plan`, which does not
        // recurse. Saying so here keeps the attributor honest rather than generous.
        Some(rest) if !rest.contains('/') => is_numbered_plan_name(rest),
        _ => false,
    }
}

/// Whether THIS COMMIT can be responsible for anything this gate finds.
///
/// THE DEFECT THIS CLOSES (ruled 2026-09-11, the `r1_breadth` half of the attribution
/// class `omp-orchestrator-nu8lc` names). `check_repo` is DECLARED repo-wide and its
/// subject -- is the flow population's breadth within one level -- is correct repo-wide:
/// narrowing the READ would change what the gate means. But on the commit path it refused
/// the committer for a population property nobody in this commit created. The tree carried
/// 75 dirty files tonight; a false red trains every reader to discount the verdict, which
/// is worse than a silent gate.
///
/// ATTRIBUTION, NOT SURFACE: the read stays whole-repo, and only the VERDICT is
/// partitioned. A refusal stands when the commit stages any input the score is computed
/// from; otherwise the caller REPORTS it, typed and named, and does not refuse.
///
/// FAIL-CLOSED, both clauses carried over from GATE 8 where they were invented:
/// an INSTRUMENT error ([`CheckError::Io`], [`CheckError::PopulationUnpinned`]) refuses
/// regardless of attribution -- a gate that could not measure has not found the commit
/// innocent -- and an EMPTY staged set is not evidence of foreignness, it is the absence of
/// a commit, so [`attribution_of`] leaves it to the caller's own empty-commit refusal.
pub fn commit_touches_inputs(staged: &[String]) -> bool {
    staged.iter().any(|path| is_gate_input(path))
}

/// What the caller should do with `error` for a commit staging `staged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    /// This commit stages an input, or the gate could not measure at all. REFUSE.
    Refuse,
    /// Nothing this commit stages feeds the finding. REPORT it, typed, and do not refuse.
    ReportForeign,
}

/// Classify a refusal against the staged set. INSTRUMENT errors always [`Attribution::Refuse`].
pub fn attribution_of(error: &CheckError, staged: &[String]) -> Attribution {
    match error {
        // The gate did not measure. Silence here would convert a blind instrument into a
        // clean bill, which is the vacuous-green inversion this repository keeps removing.
        CheckError::Io { .. } | CheckError::PopulationUnpinned => Attribution::Refuse,
        CheckError::UnpinnedSubject { .. }
        | CheckError::ScoreUncited { .. }
        | CheckError::DeltaExceeded { .. } => {
            if commit_touches_inputs(staged) {
                Attribution::Refuse
            } else {
                Attribution::ReportForeign
            }
        }
    }
}
pub fn check_repo(root: &Path) -> Result<Report, CheckError> {
    let contract = root.join("docs/plan/flow/CONTRACT.md");
    let contract_text = fs::read_to_string(&contract).map_err(|e| CheckError::Io {
        detail: format!("{}: {e}", contract.display()),
    })?;
    let pin = parse_pin(&contract_text).ok_or(CheckError::PopulationUnpinned)?;
    let boxes_dir = root.join("docs/plan/flow/boxes");
    let on_disk = list_box_ids(&boxes_dir)?;
    for (id, path) in &on_disk {
        if !pin.iter().any(|p| p == id) {
            return Err(CheckError::UnpinnedSubject {
                id: id.clone(),
                path: path.display().to_string(),
            });
        }
    }
    let mut scores = Vec::new();
    for id in &pin {
        let path = boxes_dir.join(format!("{id}.toml"));
        let text = fs::read_to_string(&path).map_err(|e| CheckError::Io {
            detail: format!("{}: {e}", path.display()),
        })?;
        let score = score_box(id, &path, &text)?;
        scores.push(score);
    }
    for md in list_numbered_plan(root)? {
        let text = fs::read_to_string(&md).map_err(|e| CheckError::Io {
            detail: format!("{}: {e}", md.display()),
        })?;
        scores.push(score_markdown(&md, &text)?);
    }
    for s in &scores {
        if s.citation.is_empty() {
            return Err(CheckError::ScoreUncited { id: s.id.clone() });
        }
    }
    let box_levels: Vec<u8> = pin
        .iter()
        .filter_map(|id| scores.iter().find(|s| s.id == *id).map(|s| s.level))
        .collect();
    let (max, median, delta) = stats(&box_levels);
    let report = Report {
        scores,
        max,
        median,
        delta,
    };
    if delta > 1 && !exception_unexpired(&contract_text) {
        return Err(CheckError::DeltaExceeded {
            max,
            median,
            delta,
            table: report.render(),
        });
    }
    Ok(report)
}

fn parse_pin(contract: &str) -> Option<Vec<String>> {
    for line in contract.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(PIN_NEEDLE) {
            let ids: Vec<String> = rest
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
            if ids.is_empty() {
                return None;
            }
            return Some(ids);
        }
    }
    None
}

fn list_box_ids(dir: &Path) -> Result<Vec<(String, PathBuf)>, CheckError> {
    let mut out = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| CheckError::Io {
        detail: format!("{}: {e}", dir.display()),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        if stem.is_empty() {
            continue;
        }
        out.push((stem, path));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn list_numbered_plan(root: &Path) -> Result<Vec<PathBuf>, CheckError> {
    let dir = root.join("docs/plan");
    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| CheckError::Io {
        detail: format!("{}: {e}", dir.display()),
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // ONE predicate, shared with `is_gate_input`. Two copies of a membership rule
        // drift, and a drifted attributor would call a staged input foreign.
        if is_numbered_plan_name(name) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

fn score_box(id: &str, path: &Path, text: &str) -> Result<SubjectScore, CheckError> {
    let mut level: u8 = 0;
    let mut citation = String::new();
    if let Some(line) = find_line(text, &format!("id = \"{id}\"")) {
        level = 0;
        citation = cite(path, line);
    }
    if let Some(line) = find_line(text, "kernel_input") {
        if text.contains("kernel_output") && text.contains("event_row") && text.contains("validator")
        {
            // GROUNDED if any measured/command row exists
            if let Some(g) = find_line(text, "measured")
                .or_else(|| find_line(text, "command"))
                .or_else(|| find_line(text, "claim"))
            {
                level = 1;
                citation = cite(path, g);
            }
            level = 2;
            citation = cite(path, line);
        }
    }
    if let Some(line) = first_class_known_bad(text) {
        level = 3;
        citation = cite(path, line);
    }
    if text.contains("[box.contracts]")
        && text.contains("[[box.observability]]")
        && text.contains("[[box.hook]]")
    {
        if let Some(line) = find_line(text, "[box.contracts]") {
            level = 4;
            citation = cite(path, line);
        }
    }
    if text.contains("agreement.status") && text.contains("approval") && text.contains("HD-") {
        if let Some(line) = find_line(text, "converged") {
            level = 5;
            citation = cite(path, line);
        }
    }
    if citation.is_empty() {
        return Err(CheckError::ScoreUncited { id: id.to_owned() });
    }
    Ok(SubjectScore {
        id: id.to_owned(),
        path: path.display().to_string(),
        level,
        level_name: LEVEL_NAMES[level as usize],
        citation,
    })
}

fn score_markdown(path: &Path, text: &str) -> Result<SubjectScore, CheckError> {
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("section")
        .to_owned();
    let mut level: u8 = 0;
    let mut citation = find_line(text, "#")
        .map(|n| cite(path, n))
        .unwrap_or_default();
    if let Some(line) = find_line(text, "MEASURED") {
        level = 1;
        citation = cite(path, line);
    }
    if let Some(line) = find_line(text, "verdict").or_else(|| find_line(text, "CONTRACT")) {
        level = 2;
        citation = cite(path, line);
    }
    if let Some(line) = find_line(text, "known-bad").or_else(|| find_line(text, "falsif")) {
        level = 3;
        citation = cite(path, line);
    }
    if citation.is_empty() {
        return Err(CheckError::ScoreUncited { id });
    }
    Ok(SubjectScore {
        id,
        path: path.display().to_string(),
        level,
        level_name: LEVEL_NAMES[level as usize],
        citation,
    })
}

fn first_class_known_bad(text: &str) -> Option<usize> {
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("known_bad") && t.contains('=') && !t.contains("resolves") {
            return Some(i + 1);
        }
    }
    None
}

fn find_line(text: &str, needle: &str) -> Option<usize> {
    text.lines().position(|l| l.contains(needle)).map(|i| i + 1)
}

fn cite(path: &Path, line: usize) -> String {
    format!("{}:{line}", path.display())
}

fn stats(levels: &[u8]) -> (u8, u8, u8) {
    let mut v = levels.to_vec();
    v.sort_unstable();
    let max = *v.last().unwrap_or(&0);
    let median = if v.is_empty() {
        0
    } else if v.len() % 2 == 1 {
        v[v.len() / 2]
    } else {
        v[v.len() / 2 - 1]
    };
    let delta = max.saturating_sub(median);
    (max, median, delta)
}

fn exception_unexpired(contract: &str) -> bool {
    let keys = [
        "R1_EXCEPTION_REASON=",
        "R1_EXCEPTION_AFFECTED_SECTIONS=",
        "R1_EXCEPTION_EXPIRY=",
        "R1_EXCEPTION_REVIEWER=",
        "R1_EXCEPTION_RISKS=",
    ];
    keys.iter().all(|k| {
        contract.lines().any(|line| {
            let t = line.trim();
            t.starts_with(k) && t.len() > k.len() && !t.ends_with("EXPIRED")
        })
    })
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let base = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join(".local/state/zeststream/scratch/r1-breadth-gate-tests");
        let dir = base.join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("docs/plan/flow/boxes")).unwrap();
        fs::create_dir_all(dir.join("docs/plan")).unwrap();
        dir
    }

    fn write_box(root: &Path, id: &str, body: &str) {
        fs::write(root.join(format!("docs/plan/flow/boxes/{id}.toml")), body).unwrap();
    }

    fn pin(root: &Path, boxes: &str) {
        fs::write(
            root.join("docs/plan/flow/CONTRACT.md"),
            format!("# c\n{PIN_NEEDLE}{boxes}\n"),
        )
        .unwrap();
    }

    fn md(root: &Path, name: &str) {
        fs::write(
            root.join("docs/plan").join(name),
            "# x\nMEASURED\nverdict\nknown-bad\n",
        )
        .unwrap();
    }

    fn mature4(id: &str) -> String {
        format!(
            "[[box]]\nid = \"{id}\"\nname = \"n\"\nkernel_input = \"i\"\nkernel_output = \"o\"\nevent_row = \"e\"\nvalidator = \"v\"\nmeasured = \"yes\"\nknown_bad = \"planted\"\n[box.contracts]\nx = 1\n[[box.observability]]\nk = 1\n[[box.hook]]\nh = 1\n"
        )
    }

    fn mature2(id: &str) -> String {
        format!(
            "[[box]]\nid = \"{id}\"\nname = \"n\"\nkernel_input = \"i\"\nkernel_output = \"o\"\nevent_row = \"e\"\nvalidator = \"v\"\nmeasured = \"yes\"\n"
        )
    }

    #[test]
    fn thirteenth_box_at_four_goes_red() {
        let root = scratch("plant4");
        pin(&root, "S1,S2,S3,S4,S5a,S5b,S6a,S6b,S6c,S7,S8,S9,SX");
        for id in [
            "S2", "S3", "S4", "S5a", "S5b", "S6a", "S6b", "S6c", "S7", "S8", "S9",
        ] {
            write_box(&root, id, &mature2(id));
        }
        write_box(&root, "S1", &mature4("S1"));
        write_box(&root, "SX", &mature4("SX"));
        md(&root, "00-brief.md");
        let err = check_repo(&root).unwrap_err();
        assert!(
            matches!(err, CheckError::DeltaExceeded { delta: 2, .. }),
            "{err}"
        );
    }

    #[test]
    fn missing_pin_refuses_default_denominator() {
        let root = scratch("nopin");
        fs::write(root.join("docs/plan/flow/CONTRACT.md"), "# no pin\n").unwrap();
        write_box(&root, "S1", &mature2("S1"));
        md(&root, "00-brief.md");
        let err = check_repo(&root).unwrap_err();
        assert!(matches!(err, CheckError::PopulationUnpinned), "{err}");
    }

    #[test]
    fn extra_box_not_in_pin_is_refused() {
        let root = scratch("extra");
        pin(&root, "S1");
        write_box(&root, "S1", &mature2("S1"));
        write_box(&root, "SX", &mature4("SX"));
        md(&root, "00-brief.md");
        let err = check_repo(&root).unwrap_err();
        assert!(matches!(err, CheckError::UnpinnedSubject { .. }), "{err}");
    }

    #[test]
    fn delta_zero_exits_ok() {
        let root = scratch("ok");
        pin(&root, "S1,S2");
        write_box(&root, "S1", &mature2("S1"));
        write_box(&root, "S2", &mature2("S2"));
        md(&root, "00-brief.md");
        let report = check_repo(&root).unwrap();
        assert_eq!(report.delta, 0);
        assert!(report.render().contains("S1"));
    }
}
