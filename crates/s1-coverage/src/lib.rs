#![forbid(unsafe_code)]

//! Pure S1 requirement extraction, exact bead joining, and coverage reporting.
//!
//! The library owns no filesystem or process access. The CLI supplies source
//! snapshots and bead records, so tree/worktree selection is visible at the
//! boundary instead of hidden in the join logic.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const CONTRACT_PATHS: [&str; 6] = [
    "docs/contracts/s1_l0_install.md",
    "docs/contracts/s1_l1_doctor.md",
    "docs/contracts/s1_l2_ecosystem.md",
    "docs/contracts/s1_l3_walkthrough.md",
    "docs/contracts/s1_l4_liveness.md",
    "docs/contracts/s1_l5_portal.md",
];

pub const GENERATOR_PATHS: [&str; 2] = ["crates/s1-coverage/Cargo.toml", "crates/s1-coverage/src/main.rs"];

pub const CRATE_ATOM_PARTS: [&str; 9] = [
    "claim",
    "slo",
    "oracle",
    "fuzz",
    "wired-caller",
    "event-row",
    "gate-trip",
    "formal",
    "lock",
];

const HOOK_FIELDS: [&str; 12] = [
    "id",
    "event",
    "matcher",
    "class",
    "fail_mode",
    "binary",
    "policy_file",
    "source_commit",
    "language",
    "stage",
    "certified",
    "harm_class",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceText {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeadRecord {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub acceptance_criteria: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageInput {
    pub contracts: Vec<SourceText>,
    pub s1_toml: String,
    pub beads: Vec<BeadRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageState {
    Covered,
    Missing,
    #[serde(rename = "DOC-ONLY")]
    DocOnly,
    #[serde(rename = "DECLARED-UNEXTRACTABLE")]
    DeclaredUnextractable,
}

impl CoverageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Covered => "COVERED",
            Self::Missing => "MISSING",
            Self::DocOnly => "DOC-ONLY",
            Self::DeclaredUnextractable => "DECLARED-UNEXTRACTABLE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSource {
    ContractStableId,
    ContractNamedTest,
    BoxGap,
    BoxObservability,
    BoxHook,
    BoxBranchDiagram,
    LayerExists,
    BoxBranchTest,
    DecisionsHd,
    CrateAtomL0,
}

impl RequirementSource {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ContractStableId => "contract.stable_id",
            Self::ContractNamedTest => "contract.named_test",
            Self::BoxGap => "box.gap",
            Self::BoxObservability => "box.observability",
            Self::BoxHook => "box.hook",
            Self::BoxBranchDiagram => "box.branch.diagram",
            Self::LayerExists => "layer.exists",
            Self::BoxBranchTest => "box.branch.test",
            Self::DecisionsHd => "decisions.HD",
            Self::CrateAtomL0 => "crate-atom.L0",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub source: RequirementSource,
    pub stable_id: String,
    pub requirement: String,
    pub bead_id: Option<String>,
    pub state: CoverageState,
    pub predicate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Totals {
    pub requirements: usize,
    pub covered: usize,
    pub missing: usize,
    pub doc_only: usize,
    pub declared_unextractable: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSummary {
    pub source: String,
    pub requirements: usize,
    pub covered: usize,
    pub missing: usize,
    pub doc_only: usize,
    pub declared_unextractable: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InputState {
    Tree,
    Index,
    Worktree,
    WorktreeOnly,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoricalClaim {
    Unreproduced,
    IndependentlyProven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConvergenceDecision {
    Converging,
    NonConverging,
}

impl fmt::Display for ConvergenceDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Converging => "CONVERGING",
            Self::NonConverging => "NON_CONVERGING",
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ManifestVerdict {
    DenominatorConsistent,
    DenominatorWorktreeOnly,
}

/// The provenance of one coverage measurement: which revision, and which of tree / index /
/// worktree the file set came from.
///
/// RENAMED FROM `InputManifest` 2026-09-11. The `input-manifest` crate exports a public
/// `InputManifest` too — an enum about cargo input BOUNDS (`Full` / `Refused`), consumed by
/// `gate-runner::ci_citation` — and two crates exporting one public type name is the seam bug
/// `no_public_type_name_collisions_across_crates` refuses. The domains are genuinely disjoint,
/// which is what an allowance row would have recorded; a specific name records it too and
/// leaves nothing to expire.
///
/// `tests/head_fabrication_pin.rs:115,139` pin the constructor by the SOURCE TEXT
/// `"InputManifest::new("`, and that needle still matches as a substring of
/// `CoverageInputManifest::new(`, so the fabrication pin's mechanism is unchanged by the
/// rename — verified rather than assumed, because a rename that silently disarms a pin is the
/// failure this crate's own pin exists to catch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageInputManifest {
    pub revision: String,
    pub tree: Vec<String>,
    pub index: Vec<String>,
    pub worktree: Vec<String>,
    pub worktree_only: Vec<String>,
    pub historical: HistoricalClaim,
}

impl CoverageInputManifest {
    #[must_use]
    pub fn new(
        revision: impl Into<String>,
        tree: Vec<String>,
        index: Vec<String>,
        worktree: Vec<String>,
        worktree_only: Vec<String>,
    ) -> Self {
        Self {
            revision: revision.into(),
            tree,
            index,
            worktree,
            worktree_only,
            historical: HistoricalClaim::Unreproduced,
        }
    }

    #[must_use]
    pub fn verdict(&self) -> ManifestVerdict {
        if self.worktree_only.is_empty() {
            ManifestVerdict::DenominatorConsistent
        } else {
            ManifestVerdict::DenominatorWorktreeOnly
        }
    }

    #[must_use]
    pub fn generator_tracked(&self) -> bool {
        GENERATOR_PATHS.iter().all(|path| self.tree.iter().any(|actual| actual == path))
    }

    #[must_use]
    pub fn paths(&self, state: InputState) -> &[String] {
        match state {
            InputState::Tree => &self.tree,
            InputState::Index => &self.index,
            InputState::Worktree => &self.worktree,
            InputState::WorktreeOnly => &self.worktree_only,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageComparison {
    pub schema: String,
    pub base_revision: String,
    pub head_revision: String,
    pub input_manifest: CoverageInputManifest,
    pub worktree_only: Vec<String>,
    pub growth: usize,
    pub closure: usize,
    pub decision: ConvergenceDecision,
    pub no_claim: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoverageReport {
    pub schema: String,
    pub rev_mode: String,
    pub revision: String,
    pub input_manifest: CoverageInputManifest,
    pub manifest_verdict: ManifestVerdict,
    #[serde(default)]
    pub generator_tracked: bool,
    pub worktree_only: Vec<String>,
    pub growth: usize,
    pub closure: usize,
    pub decision: ConvergenceDecision,
    pub totals: Totals,
    pub source_breakdown: Vec<SourceSummary>,
    pub requirements: Vec<Requirement>,
    pub no_claim: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageError {
    ScanEmpty,
    InvalidDocOnlyReason,
    MalformedBead { line: usize },
    WorktreeOnly { paths: Vec<String> },
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScanEmpty => f.write_str("DENOMINATOR_EMPTY_SCAN_SET: no requirement inputs were provided"),
            Self::InvalidDocOnlyReason => {
                f.write_str("DOC_ONLY_REASON_MISSING: DOC_ONLY requires a non-empty reason")
            }
            Self::MalformedBead { line } => write!(f, "BEAD_JSON_INVALID: line={line}"),
            Self::WorktreeOnly { paths } => {
                write!(f, "DENOMINATOR_WORKTREE_ONLY: paths={}", paths.join(","))
            }
        }
    }
}

impl CoverageError {
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::ScanEmpty => 3,
            Self::WorktreeOnly { .. } => 2,
            Self::InvalidDocOnlyReason | Self::MalformedBead { .. } => 2,
        }
    }
}

impl std::error::Error for CoverageError {}

#[must_use]
pub fn validate_manifest(manifest: &CoverageInputManifest) -> Result<ManifestVerdict, CoverageError> {
    if manifest.worktree_only.is_empty() {
        Ok(ManifestVerdict::DenominatorConsistent)
    } else {
        Err(CoverageError::WorktreeOnly {
            paths: manifest.worktree_only.clone(),
        })
    }
}

#[must_use]
pub const fn classify_convergence(growth: usize, closure: usize) -> ConvergenceDecision {
    if closure > growth {
        ConvergenceDecision::Converging
    } else {
        ConvergenceDecision::NonConverging
    }
}

#[must_use]
pub fn manifest_for_input(revision: impl Into<String>, input: &CoverageInput) -> CoverageInputManifest {
    let tree: Vec<String> = input.contracts.iter().map(|source| source.path.clone()).collect();
    CoverageInputManifest::new(revision, tree.clone(), tree, Vec::new(), Vec::new())
}

/// True when git refused because the CHECKOUT cannot resolve the revision, as opposed to any
/// failure of this crate. Keyed on git's own wording for an unresolvable rev or an unusable repo.
///
/// `ambiguous argument` is the one a DEPTH-ZERO checkout actually emits — an initialised `.git`
/// with no commits and no objects, which is what `rch`'s remote snapshot was measured to be on
/// 2026-09-10 (`is-shallow-repository` false, no `.git/shallow`, no grafts). A set guessed from
/// the shallow hypothesis would miss it, so these needles are OBSERVED text, not predicted text.
///
/// Deliberately does NOT key on `exists on disk, but not in`: that message varies with on-disk
/// presence and carries no information about resolvability, so three readers read three different
/// conclusions out of it on 2026-09-10. It is not a resolvability oracle and is never used as one.
#[must_use]
pub fn checkout_cannot_resolve(git_error: &str) -> bool {
    let lower = git_error.to_ascii_lowercase();
    [
        "not a git repository",
        "invalid object name",
        "not a valid object name",
        "ambiguous argument",
        "does not have any commits yet",
    ]
    .into_iter()
    .any(|needle| lower.contains(needle))
}

/// The refusal emitted when the checkout cannot resolve a revision this crate was asked to read.
///
/// Attribution is the whole point: the previous message named this crate's own tree-read error and
/// a contract path, never the checkout, so a remote runner that carries no resolvable history read
/// as a coverage defect. The denominator is UNKNOWN here, never zero and never CONVERGING.
#[must_use]
pub fn checkout_unusable(revision: &str, detail: &str) -> String {
    format!(
        "S1_COVERAGE_CHECKOUT_UNUSABLE revision={revision} \
cause=CHECKOUT_CANNOT_RESOLVE_REVISION detail={detail} \
note=the checkout under test cannot resolve this revision; the denominator is UNKNOWN, not zero"
    )
}

/// Exit code for a rendered refusal: 4 when the CHECKOUT is at fault, 2 when this crate is.
///
/// A caller grading an acceptance leg has to tell those apart without parsing prose, and the
/// message alone was not enough: code and message are both pinned so neither can drift alone.
#[must_use]
pub fn refusal_exit_code(message: &str) -> u8 {
    if message.starts_with("S1_COVERAGE_CHECKOUT_UNUSABLE") {
        4
    } else {
        2
    }
}


#[must_use]
pub fn validate_doc_only_reason(
    state: CoverageState,
    reason: &str,
) -> Result<(), CoverageError> {
    if matches!(state, CoverageState::DocOnly) && reason.trim().is_empty() {
        return Err(CoverageError::InvalidDocOnlyReason);
    }
    Ok(())
}

/// Parse the tracked JSONL bead export without silently dropping malformed rows.
pub fn parse_beads_jsonl(text: &str) -> Result<Vec<BeadRecord>, CoverageError> {
    let mut beads = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|_| CoverageError::MalformedBead { line: index + 1 })?;
        let id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if id.is_empty() {
            return Err(CoverageError::MalformedBead { line: index + 1 });
        }
        beads.push(BeadRecord {
            id,
            title: value
                .get("title")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            acceptance_criteria: value
                .get("acceptance_criteria")
                .or_else(|| value.get("acceptance"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            description: value
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        });
    }
    Ok(beads)
}

/// Compute a report from already-loaded snapshots. No filesystem or process I/O occurs here.
pub fn compute(
    input: &CoverageInput,
    rev_mode: impl Into<String>,
    revision: impl Into<String>,
) -> Result<CoverageReport, CoverageError> {
    let rev_mode = rev_mode.into();
    let revision = revision.into();
    let manifest = manifest_for_input(revision.clone(), input);
    compute_with_manifest(input, rev_mode, revision, manifest)
}

/// Compute a report while carrying the exact input provenance manifest.
pub fn compute_with_manifest(
    input: &CoverageInput,
    rev_mode: impl Into<String>,
    revision: impl Into<String>,
    manifest: CoverageInputManifest,
) -> Result<CoverageReport, CoverageError> {
    if input.contracts.is_empty() {
        return Err(CoverageError::ScanEmpty);
    }

    let mut requirements = Vec::new();
    for contract in &input.contracts {
        let relevant = filter_contract_sections(&contract.text);
        let ids = extract_backticked_requirements(&relevant);
        for stable_id in &ids {
            push_requirement(
                &mut requirements,
                RequirementSource::ContractStableId,
                stable_id.clone(),
                format!("stable id in {}", contract.path),
                stable_id.clone(),
                "backticked stable ID; Cross-References and WBS sections excluded",
                &input.beads,
                None,
            );
        }
        for test_name in extract_backticked_tests(&relevant) {
            push_requirement(
                &mut requirements,
                RequirementSource::ContractNamedTest,
                test_name.clone(),
                format!("named test in {}", contract.path),
                test_name,
                "backticked .rs::function name; Cross-References and WBS sections excluded",
                &input.beads,
                None,
            );
        }
        for hidden in extract_unbackticked_requirements(&relevant, &ids) {
            if !extract_backticked_requirements(&relevant).contains(&hidden) {
                requirements.push(Requirement {
                    source: RequirementSource::ContractStableId,
                    stable_id: hidden.clone(),
                    requirement: format!("bold stable ID in {}", contract.path),
                    bead_id: None,
                    state: CoverageState::DeclaredUnextractable,
                    predicate: "bold ID is not backticked; extractor cannot join it".to_owned(),
                    reason: Some("DECLARED-UNEXTRACTABLE: backtick the stable ID".to_owned()),
                });
            }
        }
    }

    add_s1_toml_requirements(&mut requirements, &input.s1_toml, &input.beads);
    if requirements.is_empty() {
        return Err(CoverageError::ScanEmpty);
    }
    add_decision_requirements(&mut requirements, &input.s1_toml, &input.beads);
    for part in CRATE_ATOM_PARTS {
        push_requirement(
            &mut requirements,
            RequirementSource::CrateAtomL0,
            format!("ATOM-L0-{part}"),
            format!("crate-atom-gate part {part} for crates/installer"),
            String::new(),
            "static nine-part crate-atom inventory; no bead token provided",
            &input.beads,
            None,
        );
    }

    if requirements.is_empty() {
        return Err(CoverageError::ScanEmpty);
    }
    for row in &requirements {
        validate_doc_only_reason(row.state, row.reason.as_deref().unwrap_or_default())?;
    }

    let totals = totals(&requirements);
    let source_breakdown = summarize_sources(&requirements);
    Ok(CoverageReport {
        schema: "s1-coverage/v1".to_owned(),
        rev_mode: rev_mode.into(),
        revision: revision.into(),
        input_manifest: manifest.clone(),
        manifest_verdict: manifest.verdict(),
        generator_tracked: manifest.generator_tracked(),
        worktree_only: manifest.worktree_only.clone(),
        growth: 0,
        closure: 0,
        decision: classify_convergence(0, 0),
        totals,
        source_breakdown,
        requirements,
        no_claim: vec![
            "COVERED means an exact token appeared in a bead title or acceptance_criteria; it does not prove the bead is implemented or closed.".to_owned(),
            "The report is reproducible for its declared tree revision; worktree mode is an explicit local-edit view.".to_owned(),
            "A zero MISSING count does not establish S1 convergence, wiring, or runtime correctness.".to_owned(),
            "NO-COVERAGE: this matrix does not prove implementation, wiring, or runtime correctness.".to_owned(),
        ],
    })
}
#[must_use]
pub fn compare_reports(base: &CoverageReport, head: &CoverageReport) -> CoverageComparison {
    let base_states: BTreeMap<String, CoverageState> = base
        .requirements
        .iter()
        .map(|row| (format!("{}::{}", row.source.as_str(), row.stable_id), row.state))
        .collect();
    let head_states: BTreeMap<String, CoverageState> = head
        .requirements
        .iter()
        .map(|row| (format!("{}::{}", row.source.as_str(), row.stable_id), row.state))
        .collect();
    let growth = head_states
        .keys()
        .filter(|key| !base_states.contains_key(*key))
        .count();
    let closure = base_states
        .iter()
        .filter(|(key, state)| {
            *state != &CoverageState::Covered
                && head_states.get(*key) == Some(&CoverageState::Covered)
        })
        .count();
    CoverageComparison {
        schema: "s1-coverage/comparison-v1".to_owned(),
        base_revision: base.revision.clone(),
        head_revision: head.revision.clone(),
        input_manifest: head.input_manifest.clone(),
        worktree_only: head.worktree_only.clone(),
        growth,
        closure,
        decision: classify_convergence(growth, closure),
        no_claim: vec![
            "growth counts newly declared requirement keys; closure counts prior non-covered keys that become covered.".to_owned(),
            "The relation-based decision does not prove requirement correctness, implementation, or deployment authorization.".to_owned(),
        ],
    }
}

fn push_requirement(
    requirements: &mut Vec<Requirement>,
    source: RequirementSource,
    stable_id: String,
    requirement: String,
    token: String,
    predicate: &str,
    beads: &[BeadRecord],
    reason: Option<String>,
) {
    let bead_id = if token.is_empty() {
        None
    } else {
        find_bead(beads, &token)
    };
    let state = if bead_id.is_some() {
        CoverageState::Covered
    } else if reason.is_some() {
        CoverageState::DocOnly
    } else {
        CoverageState::Missing
    };
    requirements.push(Requirement {
        source,
        stable_id,
        requirement,
        bead_id,
        state,
        predicate: predicate.to_owned(),
        reason,
    });
}

fn find_bead(beads: &[BeadRecord], token: &str) -> Option<String> {
    beads
        .iter()
        .find(|bead| token_in(&bead.title, token) || token_in(&bead.acceptance_criteria, token))
        .map(|bead| bead.id.clone())
}

/// A token is whole only when neither adjacent character is alphanumeric or '-'.
#[must_use]
pub fn token_in(haystack: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let mut offset = 0;
    while let Some(relative) = haystack[offset..].find(token) {
        let start = offset + relative;
        let end = start + token.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '-');
        let after_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '-');
        if before_ok && after_ok {
            return true;
        }
        offset = end;
        if offset >= haystack.len() {
            break;
        }
    }
    false
}

fn is_stable_id(value: &str) -> bool {
    let prefixes = [
        "L0-", "L1-", "L2-", "L3-", "L4-", "L5-", "L0P-", "L1P-", "L2P-", "L3P-",
        "L4P-", "L5P-", "LAW-L0-", "LAW-L1-", "LAW-L2-", "LAW-L3-", "LAW-L4-",
        "LAW-L5-", "OBS-L0-", "OBS-L1-", "OBS-L2-", "OBS-L3-", "OBS-L4-", "OBS-L5-",
    ];
    prefixes.iter().any(|prefix| {
        value.starts_with(prefix)
            && value.len() > prefix.len()
            && value[prefix.len()..]
                .chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
    })
}

fn extract_backticked_requirements(text: &str) -> Vec<String> {
    let regex = Regex::new(r"`([^`\n]+)`").expect("backtick regex");
    let mut seen = BTreeSet::new();
    for capture in regex.captures_iter(text) {
        let value = capture.get(1).map_or("", |m| m.as_str());
        if is_stable_id(value) {
            seen.insert(value.to_owned());
        }
    }
    seen.into_iter().collect()
}

fn extract_backticked_tests(text: &str) -> Vec<String> {
    let regex = Regex::new(r"`([A-Za-z0-9_.-]+\.rs::[A-Za-z0-9_]+)`").expect("test regex");
    let mut seen = BTreeSet::new();
    for capture in regex.captures_iter(text) {
        if let Some(value) = capture.get(1) {
            seen.insert(value.as_str().to_owned());
        }
    }
    seen.into_iter().collect()
}

fn extract_unbackticked_requirements(text: &str, backticked: &[String]) -> Vec<String> {
    let regex = Regex::new(r"(?:LAW-|OBS-)?L[0-5](?:P|R)?-[A-Z0-9-]+")
        .expect("stable ID regex");
    let mut seen = BTreeSet::new();
    for matched in regex.find_iter(text) {
        let value = matched.as_str();
        let before_ok = text[..matched.start()]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '-');
        let after_ok = text[matched.end()..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '-');
        let inside_backticks = text[..matched.start()]
            .bytes()
            .filter(|byte| *byte == b'`')
            .count()
            % 2
            == 1;
        if before_ok
            && after_ok
            && !inside_backticks
            && is_stable_id(value)
            && !backticked.iter().any(|known| known == value)
        {
            seen.insert(value.to_owned());
        }
    }
    seen.into_iter().collect()
}

fn filter_contract_sections(text: &str) -> String {
    let mut excluded = false;
    let mut output = String::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            excluded = excluded_heading(line);
        }
        if !excluded {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

fn excluded_heading(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("## cross-references")
        || lower.starts_with("## work breakdown")
        || lower.starts_with("## wbs")
}

fn clean_toml(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn blocks(text: &str, header: &str) -> Vec<String> {
    let marker = format!("[[{header}]]");
    clean_toml(text)
        .split(&marker)
        .skip(1)
        .map(|part| part.split("[[").next().unwrap_or(part).to_owned())
        .collect()
}

fn quoted_field(block: &str, field: &str) -> String {
    let prefix = format!("{field} = \"");
    block
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_default()
        .to_owned()
}

fn add_s1_toml_requirements(
    requirements: &mut Vec<Requirement>,
    toml: &str,
    beads: &[BeadRecord],
) {
    for (index, block) in blocks(toml, "box.gap").iter().enumerate() {
        let what = quoted_field(block, "what");
        let resolves = quoted_field(block, "resolves");
        let token = Regex::new(r"omp-orchestrator-[A-Za-z0-9._-]+")
            .expect("bead id regex")
            .find(&resolves)
            .map_or("", |m| m.as_str())
            .to_owned();
        push_requirement(
            requirements,
            RequirementSource::BoxGap,
            format!("GAP-{}", index + 1),
            what.chars().take(120).collect::<String>(),
            token,
            "S1.toml box.gap resolves token; bead title or acceptance_criteria exact token",
            beads,
            None,
        );
    }

    for block in blocks(toml, "box.observability") {
        let layer = quoted_field(&block, "layer");
        for field in ["event_row", "artifact", "monitor", "gate"] {
            push_requirement(
                requirements,
                RequirementSource::BoxObservability,
                format!("{layer}.{field}"),
                format!("{layer} observability {field}"),
                format!("{layer}-{field}"),
                "S1.toml observability field; exact bead token in title or acceptance_criteria",
                beads,
                None,
            );
        }
        push_requirement(
            requirements,
            RequirementSource::BoxObservability,
            format!("{layer}.known_bad"),
            format!("{layer} observability known-bad"),
            format!("{layer}-known_bad"),
            "S1.toml observability known_bad; exact bead token in title or acceptance_criteria",
            beads,
            None,
        );
        push_requirement(
            requirements,
            RequirementSource::BoxObservability,
            format!("{layer}.metric"),
            format!("{layer} observability metric"),
            format!("{layer}-METRIC"),
            "S1.toml observability metric; exact bead token in title or acceptance_criteria",
            beads,
            None,
        );
    }

    for (index, block) in blocks(toml, "box.hook").iter().enumerate() {
        let surface = quoted_field(block, "surface");
        for field in HOOK_FIELDS {
            push_requirement(
                requirements,
                RequirementSource::BoxHook,
                format!("HOOK-{}-{field}", index + 1),
                format!("{surface} certified field {field}"),
                String::new(),
                "S1.toml hook field; no bead token is inferred",
                beads,
                None,
            );
        }
    }

    let clean = clean_toml(toml);
    let branch_body = clean
        .split("branches = [")
        .nth(1)
        .and_then(|rest| rest.split(']').next())
        .unwrap_or_default();
    let quoted = Regex::new(r#""([^"]+)""#).expect("branch regex");
    for (index, branch) in quoted
        .captures_iter(branch_body)
        .filter_map(|capture| capture.get(1).map(|m| m.as_str().to_owned()))
        .enumerate()
    {
        push_requirement(
            requirements,
            RequirementSource::BoxBranchDiagram,
            format!("BR-{index}-diagram", index = index + 1),
            branch.chars().take(120).collect::<String>(),
            String::new(),
            "S1.toml branches array; explicit branch predicate, no inferred bead",
            beads,
            None,
        );
        push_requirement(
            requirements,
            RequirementSource::BoxBranchTest,
            format!("BR-{index}-test", index = index + 1),
            format!("test for branch: {}", branch.chars().take(100).collect::<String>()),
            String::new(),
            "S1.toml branch test obligation; explicit branch predicate, no inferred bead",
            beads,
            None,
        );
    }

    for block in blocks(toml, "box.layer") {
        let layer = quoted_field(&block, "id");
        let exists = quoted_field(&block, "exists");
        push_requirement(
            requirements,
            RequirementSource::LayerExists,
            format!("{layer}.exists"),
            format!("{layer} exists={}", exists.chars().take(80).collect::<String>()),
            String::new(),
            "S1.toml box.layer exists predicate; explicit crate presence, no inferred bead",
            beads,
            None,
        );
    }
}

fn add_decision_requirements(
    requirements: &mut Vec<Requirement>,
    s1_toml: &str,
    beads: &[BeadRecord],
) {
    let regex = Regex::new(r"HD-[0-9]{4}").expect("decision ID regex");
    let mut seen = BTreeSet::new();
    for matched in regex.find_iter(s1_toml) {
        let id = matched.as_str();
        if matches!(id, "HD-0009" | "HD-0010" | "HD-0011" | "HD-0012") {
            seen.insert(id.to_owned());
        }
    }
    for id in seen {
        push_requirement(
            requirements,
            RequirementSource::DecisionsHd,
            id.clone(),
            "HD decision missing from decisions.jsonl".to_owned(),
            String::new(),
            "HD reference is extracted from S1.toml; docs/decisions.jsonl is intentionally not read",
            beads,
            None,
        );
    }
}

fn totals(requirements: &[Requirement]) -> Totals {
    let mut out = Totals {
        requirements: requirements.len(),
        ..Totals::default()
    };
    for requirement in requirements {
        match requirement.state {
            CoverageState::Covered => out.covered += 1,
            CoverageState::Missing => out.missing += 1,
            CoverageState::DocOnly => out.doc_only += 1,
            CoverageState::DeclaredUnextractable => out.declared_unextractable += 1,
        }
    }
    out
}

fn summarize_sources(requirements: &[Requirement]) -> Vec<SourceSummary> {
    let mut by_source: BTreeMap<String, Totals> = BTreeMap::new();
    for requirement in requirements {
        let entry = by_source
            .entry(requirement.source.as_str().to_owned())
            .or_default();
        entry.requirements += 1;
        match requirement.state {
            CoverageState::Covered => entry.covered += 1,
            CoverageState::Missing => entry.missing += 1,
            CoverageState::DocOnly => entry.doc_only += 1,
            CoverageState::DeclaredUnextractable => entry.declared_unextractable += 1,
        }
    }
    by_source
        .into_iter()
        .map(|(source, counts)| SourceSummary {
            source,
            requirements: counts.requirements,
            covered: counts.covered,
            missing: counts.missing,
            doc_only: counts.doc_only,
            declared_unextractable: counts.declared_unextractable,
        })
        .collect()
}

fn render_manifest_summary(report: &CoverageReport) -> String {
    format!(
        "INPUT_MANIFEST TREE={} INDEX={} WORKTREE={} WORKTREE_ONLY={} HISTORICAL={:?}\nMANIFEST_VERDICT={:?} GROWTH={} CLOSURE={} DECISION={}\n\n",
        report.input_manifest.paths(InputState::Tree).len(),
        report.input_manifest.paths(InputState::Index).len(),
        report.input_manifest.paths(InputState::Worktree).len(),
        report.input_manifest.paths(InputState::WorktreeOnly).len(),
        report.input_manifest.historical,
        report.manifest_verdict,
        report.growth,
        report.closure,
        report.decision,
    )
}

#[must_use]
pub fn render_markdown(report: &CoverageReport) -> String {
    let mut output = String::new();
    output.push_str("# S1 Coverage Matrix\n\n");
    output.push_str(&format!(
        "Generated by s1-coverage. REV_MODE={} REVISION={}\n\n",
        report.rev_mode, report.revision
    ));
    output.push_str(&render_manifest_summary(report));
    output.push_str(&format!("GENERATOR_TRACKED={}\n\n", if report.generator_tracked { "YES" } else { "NO" }));
    output.push_str("WIRED_FROM=s1-coverage-cli\n");
    output.push_str("NO-COVERAGE: this matrix does not prove implementation, wiring, or runtime correctness.\n\n");
    output.push_str(&format!(
        "S1_REQUIREMENTS={} COVERED={} MISSING={} DOC_ONLY={} DECLARED-UNEXTRACTABLE={}\n\n",
        report.totals.requirements,
        report.totals.covered,
        report.totals.missing,
        report.totals.doc_only,
        report.totals.declared_unextractable
    ));
    output.push_str("Counts are derived by each row's predicate column; no count is hand-entered.\n\n");
    output.push_str("## Source counts\n\n| source | n | COVERED | MISSING | DOC_ONLY | DECLARED-UNEXTRACTABLE |\n|---|---:|---:|---:|---:|---:|\n");
    for summary in &report.source_breakdown {
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            summary.source,
            summary.requirements,
            summary.covered,
            summary.missing,
            summary.doc_only,
            summary.declared_unextractable
        ));
    }
    output.push_str("\n## Matrix\n\n| source | stable_id | requirement | bead_id | state | predicate |\n|---|---|---|---|---|---|\n");
    for row in &report.requirements {
        let bead = row.bead_id.as_deref().unwrap_or("—");
        output.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | {} | {} |\n",
            row.source.as_str(),
            row.stable_id,
            table_safe(&row.requirement),
            bead,
            row.state.as_str(),
            table_safe(&row.predicate)
        ));
    }
    output.push_str("\n## NO-CLAIM\n\n");
    for claim in &report.no_claim {
        output.push_str(&format!("- {claim}\n"));
    }
    output
}

fn table_safe(value: &str) -> String {
    value.replace('|', "/").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(contract: &str, beads: Vec<BeadRecord>) -> CoverageInput {
        CoverageInput {
            contracts: vec![SourceText {
                path: "docs/contracts/s1_l1_doctor.md".to_owned(),
                text: contract.to_owned(),
            }],
            s1_toml: "".to_owned(),
            beads,
        }
    }

    #[test]
    fn exact_token_joins_title_and_acceptance_but_not_description() {
        let contract = "law `L1-REAL`\n";
        let title_hit = BeadRecord {
            id: "omp-orchestrator-real-title".to_owned(),
            title: "L1-REAL".to_owned(),
            acceptance_criteria: String::new(),
            description: String::new(),
        };
        let description_only = BeadRecord {
            id: "omp-orchestrator-description".to_owned(),
            title: "plain".to_owned(),
            acceptance_criteria: String::new(),
            description: "mentions L1-REAL".to_owned(),
        };
        let report = compute(&input(contract, vec![title_hit, description_only.clone()]), "tree", "HEAD").unwrap();
        assert_eq!(report.requirements[0].state, CoverageState::Covered);
        assert_eq!(report.requirements[0].bead_id.as_deref(), Some("omp-orchestrator-real-title"));

        let report = compute(&input(contract, vec![description_only]), "tree", "HEAD").unwrap();
        assert_eq!(report.requirements[0].state, CoverageState::Missing);
    }

    #[test]
    fn hyphenated_bead_id_is_not_a_whole_stable_id_token() {
        let contract = "law `L1-REAL`\n";
        let bead = BeadRecord {
            id: "omp-orchestrator-hyphen".to_owned(),
            title: "omp-orchestrator-l1-real-aaaa".to_owned(),
            acceptance_criteria: String::new(),
            description: String::new(),
        };
        let report = compute(&input(contract, vec![bead]), "tree", "HEAD").unwrap();
        assert_eq!(report.requirements[0].state, CoverageState::Missing);
    }

    #[test]
    fn cross_references_and_wbs_are_excluded_from_contract_extraction() {
        let contract = "main `L1-REAL`\n\n## WBS\n| `L1-WBS` |\n\n## Cross-References\n`L1-XREF`\n";
        let report = compute(&input(contract, Vec::new()), "tree", "HEAD").unwrap();
        let ids: Vec<_> = report.requirements.iter().map(|row| row.stable_id.as_str()).collect();
        assert!(ids.contains(&"L1-REAL"));
        assert!(!ids.contains(&"L1-WBS"));
        assert!(!ids.contains(&"L1-XREF"));
    }

    #[test]
    fn bold_identifier_is_declared_unextractable() {
        let report = compute(&input("bold **LAW-L1-HIDDEN**\n", Vec::new()), "tree", "HEAD").unwrap();
        assert_eq!(report.requirements[0].state, CoverageState::DeclaredUnextractable);
        assert!(report.requirements[0].reason.as_deref().unwrap().contains("backtick"));
    }


    #[test]
    fn doc_only_without_reason_is_an_error() {
        assert_eq!(
            validate_doc_only_reason(CoverageState::DocOnly, ""),
            Err(CoverageError::InvalidDocOnlyReason)
        );
    }

    #[test]
    fn s1_toml_sources_are_counted_with_predicates() {
        let toml = r#"
[[box.gap]]
what = "missing caller"
resolves = "omp-orchestrator-gap-aaaa"
[[box.observability]]
layer = "L1"
event_row = "row"
artifact = "artifact"
monitor = "monitor"
gate = "gate"
metric = "metric"
[[box.hook]]
surface = "pre-commit"
[[box.layer]]
id = "L1"
exists = "none"
branches = ["one", "two"]
"#;
        let report = compute(
            &CoverageInput {
                contracts: vec![SourceText {
                    path: "docs/contracts/s1_l1_doctor.md".to_owned(),
                    text: "`L1-REAL`".to_owned(),
                }],
                s1_toml: toml.to_owned(),
                beads: Vec::new(),
            },
            "tree",
            "HEAD",
        )
        .unwrap();
        assert!(report
            .source_breakdown
            .iter()
            .any(|row| row.source == "box.observability"));
        assert!(report
            .requirements
            .iter()
            .all(|row| !row.predicate.is_empty()));
    }

    #[test]
    fn markdown_includes_mode_predicates_and_no_claim() {
        let report = compute(&input("`L1-REAL`", Vec::new()), "tree", "HEAD").unwrap();
        let markdown = render_markdown(&report);
        assert!(markdown.contains("REV_MODE=tree"));
        assert!(markdown.contains("predicate"));
        assert!(markdown.contains("NO-CLAIM"));
    }
}
