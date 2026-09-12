use ack_stage::ack_instruction;
use dispatch_claim_fence::BeadSnapshot;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    PacketFieldMissing(&'static str),
    FiledOnlyRecord {
        bead: String,
        marker: &'static str,
    },
    PacketAddsScope {
        bead: String,
        line: usize,
        detail: String,
    },
    BeadNotClaimed {
        bead: String,
        expected_pane: String,
        actual_status: String,
        actual_assignee: Option<String>,
    },
    MutationClauseMissing {
        bead: String,
    },
    MutationClauseTooCoarse {
        bead: String,
        detail: String,
    },
    /// Field empty, description has an ACCEPTANCE heading. Distinct from
    /// [`PacketError::PacketFieldMissing`] — one label for both is xy8oo.
    AcceptanceInDescriptionOnly {
        bead: String,
    },
}
impl PacketError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::PacketFieldMissing(_) => "PACKET_FIELD_MISSING",
            Self::FiledOnlyRecord { .. } => "FILED_ONLY_RECORD",
            Self::PacketAddsScope { .. } => "PACKET_ADDS_SCOPE",
            Self::BeadNotClaimed { .. } => "BEAD_NOT_CLAIMED",
            Self::MutationClauseMissing { .. } => "MUTATION_CLAUSE_MISSING",
            Self::MutationClauseTooCoarse { .. } => "MUTATION_CLAUSE_TOO_COARSE",
            Self::AcceptanceInDescriptionOnly { .. } => "ACCEPTANCE_IN_DESCRIPTION_ONLY",
        }
    }

    pub fn operator_exit_code(&self) -> u8 {
        match self {
            Self::BeadNotClaimed { .. } => 3,
            Self::PacketFieldMissing(_) => 2,
            Self::AcceptanceInDescriptionOnly { .. } => 4,
            Self::FiledOnlyRecord { .. }
            | Self::PacketAddsScope { .. }
            | Self::MutationClauseMissing { .. }
            | Self::MutationClauseTooCoarse { .. } => 1,
        }
    }
}

impl fmt::Display for PacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketFieldMissing(field) => {
                write!(formatter, "PacketFieldMissing(\"{field}\")")
            }
            Self::FiledOnlyRecord { bead, marker } => write!(
                formatter,
                "PACKET_REFUSED_FILED_ONLY bead={bead} marker={marker} reason=record-not-implementable"
            ),
            Self::PacketAddsScope {
                bead,
                line,
                detail,
            } => write!(
                formatter,
                "PacketAddsScope bead={bead} line={line} detail={detail} remedy=br update {bead} --acceptance-criteria"
            ),
            Self::BeadNotClaimed {
                bead,
                expected_pane,
                actual_status,
                actual_assignee,
            } => write!(
                formatter,
                "BEAD_NOT_CLAIMED bead={bead} expected_pane={expected_pane} status={actual_status} actual_assignee={} reason=requires_in_progress_claim_on_receiving_pane",
                actual_assignee.as_deref().unwrap_or("unassigned")
            ),
            Self::MutationClauseMissing { bead } => write!(
                formatter,
                "MUTATION_CLAUSE_MISSING bead={bead} reason=mutation_demanded_with_empty_clause"
            ),
            Self::MutationClauseTooCoarse { bead, detail } => write!(
                formatter,
                "MUTATION_CLAUSE_TOO_COARSE bead={bead} detail={detail} reason=mutation_must_name_file_rs_line"
            ),
            Self::AcceptanceInDescriptionOnly { bead } => write!(
                formatter,
                "ACCEPTANCE_IN_DESCRIPTION_ONLY bead={bead} reason=populate_acceptance_criteria_field"
            ),
        }
    }
}

/// Beads created on or after this UTC date must name `file.rs:N` in a mutation
/// clause. Pre-cutoff coarse clauses are grandfathered at render. Moving the
/// cutoff FORWARD is amnesty for new work and is refused on review; moving it
/// BACKWARD is a tightening.
pub const MUTATION_SITE_CUTOFF: &str = "2026-09-12";

/// Coarse mutation clauses on non-terminal beads created before
/// [`MUTATION_SITE_CUTOFF`]. Seeded 2026-09-12 from the live ledger after
/// word-boundary matching. May only fall.
pub const PRE_CUTOFF_COARSE_CEILING: usize = 44;

/// Coarse mutation clauses on non-terminal beads created on/after the cutoff.
/// Seeded 2026-09-12. May only fall. New post-cutoff work that omits `file.rs:N`
/// raises this and is a gate failure, not a ceiling raise.
pub const POST_CUTOFF_COARSE_CEILING: usize = 9;

/// What [`classify_mutation_clause`] found in one acceptance body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationClauseClass {
    /// No mutation demand. Historical beads without a mutation clause stay
    /// dispatchable (item 4 of 6we9q).
    None,
    /// At least one demand names `file.rs:N`; none are empty or coarse.
    NamedSite,
    /// A demand exists and names no `file.rs:N`.
    Coarse(String),
    /// A `Mutation:` heading with an empty body. Distinct from coarse.
    Missing,
}

/// Classifies an acceptance body with the same helpers render uses.
pub fn classify_mutation_clause(acceptance: &str) -> MutationClauseClass {
    let mut missing = false;
    let mut coarse: Option<String> = None;
    let mut named = false;
    for line in acceptance.lines() {
        let Some(body) = mutation_clause_body(line) else {
            continue;
        };
        if body.is_empty() {
            missing = true;
            continue;
        }
        if names_file_rs_line(body) {
            named = true;
        } else {
            coarse.get_or_insert_with(|| body.to_owned());
        }
    }
    if missing {
        MutationClauseClass::Missing
    } else if let Some(detail) = coarse {
        MutationClauseClass::Coarse(detail)
    } else if named {
        MutationClauseClass::NamedSite
    } else {
        MutationClauseClass::None
    }
}

/// How a bead's acceptance is sourced. One function, two callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResolution {
    /// Typed `acceptance_criteria` field is populated.
    Field(String),
    /// Field empty; description has an ACCEPTANCE heading. Not packet text.
    DescriptionOnly,
    Missing,
}

/// THE one reader. Dispatch render and at-send capture both call this.
pub fn resolve_acceptance(snapshot: &BeadSnapshot) -> AcceptanceResolution {
    if nonempty(snapshot.acceptance_criteria()).is_some() {
        return AcceptanceResolution::Field(snapshot.acceptance_criteria().trim().to_owned());
    }
    if section_from_description(snapshot.description(), "ACCEPTANCE").is_some() {
        return AcceptanceResolution::DescriptionOnly;
    }
    AcceptanceResolution::Missing
}

/// True iff the typed field is populated. Capture uses this; dispatch agrees.
pub fn has_typed_acceptance(snapshot: &BeadSnapshot) -> bool {
    matches!(resolve_acceptance(snapshot), AcceptanceResolution::Field(_))
}

/// Acceptance text a packet may render: typed field only.
pub fn acceptance_text(snapshot: &BeadSnapshot) -> Option<String> {
    match resolve_acceptance(snapshot) {
        AcceptanceResolution::Field(text) => Some(text),
        AcceptanceResolution::DescriptionOnly | AcceptanceResolution::Missing => None,
    }
}

fn packet_acceptance(snapshot: &BeadSnapshot, bead: &str) -> Result<String, PacketError> {
    match resolve_acceptance(snapshot) {
        AcceptanceResolution::Field(text) => Ok(text),
        AcceptanceResolution::DescriptionOnly => Err(PacketError::AcceptanceInDescriptionOnly {
            bead: bead.to_owned(),
        }),
        AcceptanceResolution::Missing => Err(PacketError::PacketFieldMissing("acceptance")),
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn strip_nonsemantic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let quote = chars[index];
        if !matches!(quote, '\'' | '"' | '\u{60}') {
            output.push(quote);
            index += 1;
            continue;
        }
        let mut end = index + 1;
        let mut escaped = false;
        while end < chars.len() {
            if quote != '\u{60}' && chars[end] == '\n' {
                break;
            }
            if chars[end] == quote && !escaped {
                break;
            }
            if chars[end] == '\\' {
                escaped = !escaped;
            } else {
                escaped = false;
            }
            end += 1;
        }
        if end < chars.len() && chars[end] == quote {
            for _ in index..=end {
                output.push(' ');
            }
            index = end + 1;
        } else {
            output.push(quote);
            index += 1;
        }
    }
    output
}

fn filed_only_marker(text: &str) -> Option<&'static str> {
    let lower = strip_nonsemantic(text).to_ascii_lowercase();
    if lower.contains("file not claim") {
        return Some("FILE NOT CLAIM");
    }
    if lower.contains("filed only") {
        return Some("filed only");
    }
    let contextual = lower.lines().any(|line| {
        let line = line.trim_start();
        (line.starts_with("status:") || line.starts_with("stage:") || line.starts_with("marker:"))
            && line.contains("do not claim")
    });
    contextual.then_some("do not claim")
}

fn filed_only_marker_for(snapshot: &BeadSnapshot) -> Option<&'static str> {
    filed_only_marker(snapshot.description())
        .or_else(|| filed_only_marker(snapshot.acceptance_criteria()))
}

fn section_from_description(description: &str, heading: &str) -> Option<String> {
    let mut found = false;
    let mut lines = Vec::new();
    for line in description.lines() {
        let trimmed = line.trim();
        let normalized = trimmed.trim_start_matches('#').trim();
        if normalized
            .trim_end_matches(':')
            .eq_ignore_ascii_case(heading)
        {
            found = true;
            continue;
        }
        if found {
            let upper = normalized.to_ascii_uppercase();
            if normalized.starts_with("## ")
                || upper.starts_with("NO-CLAIM")
                || upper.starts_with("NO CLAIM")
                || upper.starts_with("NON-GOAL")
            {
                break;
            }
            lines.push(line.trim_end());
        }
    }
    let section = lines.join("\n").trim().to_owned();
    nonempty(&section).map(ToOwned::to_owned)
}

fn scope(snapshot: &BeadSnapshot) -> String {
    section_from_description(snapshot.description(), "SCOPE")
        .or_else(|| section_from_description(snapshot.description(), "OWNED FILES"))
        .unwrap_or_else(|| "the bead's named files, commands, and acceptance surface".to_owned())
}

fn explicit_done_signal(bead: &str, acceptance: &str) -> Option<String> {
    for line in acceptance.lines() {
        let lower = line.to_ascii_lowercase();
        let Some(exit) = lower.find("exit") else {
            continue;
        };
        let tail = &lower[exit + 4..];
        let digits: String = tail
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(|character| character.is_ascii_digit())
            .collect();
        let Ok(code) = digits.parse::<u8>() else {
            continue;
        };
        let command = line.trim().trim_matches('`');
        if command.is_empty() {
            continue;
        }
        return Some(format!("Done: re-run {command}; expect exit code {code}."));
    }
    Some(format!(
        "Done: br comments add {bead} --actor <you> \"DONE ...\" then a DIFFERENT agent: br close {bead} --actor <grader> --reason \"MUTATION-VERIFIED ...\" (grader re-runs the bead's acceptance; read status back)"
    ))
}

fn numbered_must(line: &str) -> bool {
    let trimmed = line.trim_start();
    let digit_count = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digit_count == 0 {
        return false;
    }
    let Some(separator) = trimmed.chars().nth(digit_count) else {
        return false;
    };
    if !matches!(separator, '.' | ')' | ':') {
        return false;
    }
    trimmed[digit_count + 1..].split_whitespace().any(|word| {
        word.trim_matches(|character: char| !character.is_ascii_alphabetic())
            .eq_ignore_ascii_case("must")
    })
}

fn reject_scope_additions(bead: &str, traps: &str) -> Result<(), PacketError> {
    for (index, line) in traps.lines().enumerate() {
        let normalized = line.trim_start_matches('#').trim();
        if normalized
            .trim_end_matches(':')
            .to_ascii_uppercase()
            .starts_with("ACCEPTANCE")
            || numbered_must(line)
        {
            return Err(PacketError::PacketAddsScope {
                bead: bead.to_owned(),
                line: index + 1,
                detail: line.trim().to_owned(),
            });
        }
    }
    Ok(())
}

fn close_reason_mutation_prefix(lower: &str) -> bool {
    lower.contains("mutation-verified")
        || lower.contains("mutation-not-required")
        || lower.contains("mutation-attributed")
}

/// True when `needle` occurs in `haystack` with non-alphanumeric (or start/end)
/// boundaries. `unwire` must not match `unwired`; `uncall` must not match
/// `uncalled`. Both sides already ASCII-lowercased.
fn contains_ascii_word(haystack: &str, needle: &str) -> bool {
    let hay = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    let last = hay.len() - needle.len();
    let mut index = 0;
    while index <= last {
        if &hay[index..index + needle.len()] == needle {
            let before_ok = index == 0 || !hay[index - 1].is_ascii_alphanumeric();
            let after = index + needle.len();
            let after_ok = after == hay.len() || !hay[after].is_ascii_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn mutation_heading_body<'a>(trimmed: &'a str, lower: &str) -> Option<&'a str> {
    let start = lower.find("mutation:")?;
    if start > 0 {
        let before = lower.as_bytes()[start - 1];
        if before == b'-' || before.is_ascii_alphanumeric() {
            return None;
        }
    }
    Some(trimmed[start + "mutation:".len()..].trim())
}

fn mutation_clause_body(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if close_reason_mutation_prefix(&lower) {
        return None;
    }
    if let Some(body) = mutation_heading_body(trimmed, &lower) {
        return Some(body);
    }
    if contains_ascii_word(&lower, "un-call")
        || contains_ascii_word(&lower, "uncall")
        || contains_ascii_word(&lower, "un-wire")
        || contains_ascii_word(&lower, "unwire")
        || contains_ascii_word(&lower, "delete the call")
        || contains_ascii_word(&lower, "comment out")
    {
        return Some(trimmed);
    }
    None
}

fn names_file_rs_line(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 4 < bytes.len() {
        if bytes[index..].starts_with(b".rs:")
            && index > 0
            && (bytes[index - 1].is_ascii_alphanumeric()
                || bytes[index - 1] == b'_'
                || bytes[index - 1] == b'-'
                || bytes[index - 1] == b'/')
            && bytes[index + 4].is_ascii_digit()
        {
            return true;
        }
        index += 1;
    }
    false
}

fn bead_created_day(repo: &Path, bead_id: &str) -> Option<String> {
    let path = repo.join(".beads/issues.jsonl");
    let text = fs::read_to_string(path).ok()?;
    let id_needle = format!("\"id\":\"{bead_id}\"");
    for line in text.lines() {
        if line.trim().is_empty() || !line.contains(&id_needle) {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("id").and_then(|id| id.as_str()) != Some(bead_id) {
            continue;
        }
        let created = value.get("created_at").and_then(|stamp| stamp.as_str())?;
        let day: String = created.chars().take(10).collect();
        if day.len() == 10 {
            return Some(day);
        }
        return None;
    }
    None
}

fn is_grandfathered(repo: &Path, bead_id: &str) -> bool {
    match bead_created_day(repo, bead_id) {
        Some(day) => day.as_str() < MUTATION_SITE_CUTOFF,
        None => false,
    }
}

fn reject_coarse_mutation_clause(
    bead: &str,
    acceptance: &str,
    repo: &Path,
) -> Result<(), PacketError> {
    match classify_mutation_clause(acceptance) {
        MutationClauseClass::None | MutationClauseClass::NamedSite => Ok(()),
        MutationClauseClass::Missing => Err(PacketError::MutationClauseMissing {
            bead: bead.to_owned(),
        }),
        MutationClauseClass::Coarse(detail) => {
            if is_grandfathered(repo, bead) {
                Ok(())
            } else {
                Err(PacketError::MutationClauseTooCoarse {
                    bead: bead.to_owned(),
                    detail,
                })
            }
        }
    }
}

fn assignee_pane(assignee: &str) -> Option<&str> {
    if let Some(pane) = assignee
        .split(';')
        .find_map(|part| part.strip_prefix("pane="))
        .filter(|pane| pane.starts_with('%'))
    {
        return Some(pane);
    }
    assignee.rsplit_once('-').and_then(|(_, pane)| {
        (pane.starts_with('%') && pane[1..].bytes().all(|byte| byte.is_ascii_digit()))
            .then_some(pane)
    })
}

pub fn validate_bead_claim(
    snapshot: &BeadSnapshot,
    expected_pane: &str,
) -> Result<(), PacketError> {
    let bead = nonempty(snapshot.id()).ok_or(PacketError::PacketFieldMissing("objective"))?;
    let expected_pane = nonempty(expected_pane).ok_or(PacketError::PacketFieldMissing("pane"))?;
    let actual_assignee = snapshot.assignee().map(ToOwned::to_owned);
    let assigned_pane = snapshot.assignee().and_then(assignee_pane);
    if snapshot.status_label() == "in_progress" && assigned_pane == Some(expected_pane) {
        return Ok(());
    }
    Err(PacketError::BeadNotClaimed {
        bead: bead.to_owned(),
        expected_pane: expected_pane.to_owned(),
        actual_status: snapshot.status_label().to_owned(),
        actual_assignee,
    })
}

pub fn render(
    snapshot: &BeadSnapshot,
    target: &Path,
    why_now: Option<&str>,
    traps: Option<&str>,
) -> Result<String, PacketError> {
    render_with_pane(snapshot, target, None, None, why_now, traps)
}

pub fn render_with_pane(
    snapshot: &BeadSnapshot,
    target: &Path,
    pane: Option<&str>,
    receiver_agent: Option<&str>,
    why_now: Option<&str>,
    traps: Option<&str>,
) -> Result<String, PacketError> {
    let bead = nonempty(snapshot.id()).ok_or(PacketError::PacketFieldMissing("objective"))?;
    if let Some(pane) = pane {
        validate_bead_claim(snapshot, pane)?;
    }
    if let Some(marker) = filed_only_marker_for(snapshot) {
        return Err(PacketError::FiledOnlyRecord {
            bead: bead.to_owned(),
            marker,
        });
    }
    let objective = format!("Complete bead {bead}: {}", snapshot.title().trim());
    let scope = scope(snapshot);
    let acceptance = packet_acceptance(snapshot, bead)?;
    reject_coarse_mutation_clause(bead, &acceptance, target)?;
    let target = target.display().to_string();
    let stop = "when acceptance is met, when blocked on a named external, or when the packet contradicts the bead — say which";
    let done =
        explicit_done_signal(bead, &acceptance).ok_or(PacketError::PacketFieldMissing("done"))?;

    for (field, value) in [
        ("objective", objective.as_str()),
        ("target", target.as_str()),
        ("scope", scope.as_str()),
        ("acceptance", acceptance.as_str()),
        ("stop", stop),
    ] {
        if nonempty(value).is_none() {
            return Err(PacketError::PacketFieldMissing(field));
        }
    }
    if let Some(traps) = traps {
        reject_scope_additions(bead, traps)?;
    }

    let handoff = snapshot
        .assignee()
        .filter(|owner| owner.starts_with("supervisor:"))
        .map_or_else(String::new, |owner| {
            let action = receiver_agent.and_then(nonempty).map_or_else(
                || "the receiver must claim it before working".to_owned(),
                |receiver| {
                    format!(
                        "claim it as {receiver}: br update {bead} --assignee {receiver} --status in_progress --actor {receiver}"
                    )
                },
            );
            format!("Handoff: {owner} holds this bead; {action}.\n")
        });
    let pane_line = pane
        .and_then(nonempty)
        .map_or_else(String::new, |pane| format!("Pane: {pane}\n"));
    let ack = ack_instruction(bead);
    let mut packet = format!(
        "Objective: {objective}\n\nTarget: {target}. Read br show {bead} --json IN FULL before starting.\nEvery bead requires current-state validation: re-run br show {bead} --json immediately before editing; validate the bead's assumptions against the current repository and DAG; if the bead, plan, owner, files, dependencies, or acceptance changed, STOP and report it.\n{pane_line}{handoff}\nScope:\n{scope}\n\nAcceptance:\n{acceptance}\n\n{done}\n\nReceiver ACK (required; run exactly):\n{ack}\nThe ACK line must begin byte-exactly with ACK <token> on <pane_id> --. An ACK proves arrival and reading, never the work.\n\nStop: {stop}.\n"
    );
    if let Some(why_now) = why_now.and_then(nonempty) {
        packet.push_str(&format!("\nWhy this, why now: {why_now}\n"));
    }
    if let Some(traps) = traps.and_then(nonempty) {
        packet.push_str("\nAdditional traps (non-normative):\n");
        packet.push_str(traps);
        packet.push('\n');
    }
    Ok(packet)
}

/// Render a grading packet. Distinct from a work packet: the receiver re-runs
/// acceptance, does not implement, and is a different pane from the observer.
pub fn render_grading_packet(
    snapshot: &BeadSnapshot,
    target: &Path,
    grader_pane: &str,
    observer_pane: &str,
) -> Result<String, PacketError> {
    let grader_pane =
        nonempty(grader_pane).ok_or(PacketError::PacketFieldMissing("grader_pane"))?;
    let observer_pane =
        nonempty(observer_pane).ok_or(PacketError::PacketFieldMissing("observer_pane"))?;
    if grader_pane == observer_pane {
        return Err(PacketError::PacketAddsScope {
            bead: nonempty(snapshot.id()).unwrap_or("unknown").to_owned(),
            line: 0,
            detail: "grader_pane must differ from observer_pane".to_owned(),
        });
    }
    let work = render_with_pane(
        snapshot,
        target,
        Some(grader_pane),
        None,
        Some("peer grade assignment — re-run acceptance, do not implement"),
        None,
    )?;
    let mut packet = format!(
        "GRADE ASSIGNMENT (not implementation)\nObserver: {observer_pane} (may be WORKING; observer is not the grader).\nGrader pane: {grader_pane}\nDo not implement. Re-run the bead's acceptance. Close with MUTATION-VERIFIED if it holds; otherwise GAP/UNKNOWN.\nYou are not the author if your pane is distinct from ACK pane-scoped keys.\n\nCLOSE-REASON POLICY (emitted here because a packet that omits it owns the\nsilence it gets): if your close reason CITES A CARGO TEST FIGURE it MUST also\nname where that figure was produced -- `worker=<name>` for a remote run, or\n`local`. ack-spine refuses the row otherwise (CLOSE_REASON_WORKER_MISSING).\nMeasured 2026-09-11: 40 closes across six agents were left unlandable because\nno packet said this, so a whole session of grading could not reach the tree --\nand nobody but the closing agent can honestly supply the token afterwards.\n\n{work}"
    );
    packet.push_str(&crate::mutation_minimality::grading_instruction());
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claimed_snapshot(assignee: Option<&str>, status: &str) -> BeadSnapshot {
        BeadSnapshot::new_with_acceptance(
            "fixture",
            "packet fixture",
            "body",
            "typed acceptance",
            status,
            assignee,
        )
    }

    #[test]
    fn unassigned_bead_is_a_distinct_typed_refusal() {
        let error = render_with_pane(
            &claimed_snapshot(None, "open"),
            Path::new("/repo"),
            Some("%9"),
            Some("WildStone"),
            None,
            None,
        )
        .expect_err("unassigned bead must refuse");
        assert_eq!(error.code(), "BEAD_NOT_CLAIMED");
        assert_eq!(error.operator_exit_code(), 3);
        assert_eq!(
            error.to_string(),
            "BEAD_NOT_CLAIMED bead=fixture expected_pane=%9 status=open actual_assignee=unassigned reason=requires_in_progress_claim_on_receiving_pane"
        );
    }

    #[test]
    fn different_pane_refuses_even_when_persona_name_matches() {
        let error = render_with_pane(
            &claimed_snapshot(Some("pane=%8;incarnation=1;agent=WildStone"), "in_progress"),
            Path::new("/repo"),
            Some("%9"),
            Some("WildStone"),
            None,
            None,
        )
        .expect_err("a different pane must refuse");
        assert!(matches!(error, PacketError::BeadNotClaimed { .. }));
    }

    #[test]
    fn claimed_receiving_pane_renders_normally() {
        let packet = render_with_pane(
            &claimed_snapshot(Some("pane=%9;incarnation=1;agent=WildStone"), "in_progress"),
            Path::new("/repo"),
            Some("%9"),
            Some("WildStone"),
            None,
            None,
        )
        .expect("the receiving pane's active claim must render");
        assert!(packet.contains("Pane: %9"));
        assert!(packet.contains("typed acceptance"));
    }
    fn snapshot(id: &str, description: &str, acceptance: &str) -> BeadSnapshot {
        BeadSnapshot::new_with_acceptance(
            id,
            "packet fixture",
            description,
            acceptance,
            "open",
            None,
        )
    }

    #[test]
    fn empty_acceptance_is_a_typed_refusal() {
        let snap = snapshot("fixture", "no acceptance", "");
        let error = render(&snap, Path::new("/repo"), None, None)
            .expect_err("missing acceptance must refuse");
        assert_eq!(error, PacketError::PacketFieldMissing("acceptance"));
        assert_eq!(error.code(), "PACKET_FIELD_MISSING");
        assert_eq!(error.operator_exit_code(), 2);
        assert!(!has_typed_acceptance(&snap));
        assert_eq!(
            render(&snap, Path::new("/repo"), None, None).is_ok(),
            has_typed_acceptance(&snap)
        );
    }

    #[test]
    fn typed_acceptance_is_rendered_first() {
        let packet = render(
            &snapshot(
                "fixture",
                "## ACCEPTANCE\nwrong fallback",
                "typed acceptance\nRun cargo test; expect exit 0",
            ),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("typed acceptance should render");
        assert!(packet.contains("typed acceptance"));
        assert!(!packet.contains("wrong fallback"));
        assert!(packet.contains("Done:"));
        assert!(packet.contains("Stop:"));
    }

    #[test]
    fn acceptance_only_in_description_is_a_distinct_refusal() {
        let snap = snapshot(
            "fixture",
            "## ACCEPTANCE\nRun cargo test; expect exit 0\n\n## NO-CLAIM\nlimit",
            "",
        );
        let error = render(&snap, Path::new("/repo"), None, None)
            .expect_err("description-only acceptance must refuse");
        assert_eq!(
            error,
            PacketError::AcceptanceInDescriptionOnly {
                bead: "fixture".to_owned()
            }
        );
        assert_eq!(error.code(), "ACCEPTANCE_IN_DESCRIPTION_ONLY");
        assert_eq!(error.operator_exit_code(), 4);
        assert_ne!(
            error.code(),
            PacketError::PacketFieldMissing("acceptance").code(),
            "one label for two causes is the residual-label defect"
        );
        assert!(!has_typed_acceptance(&snap));
    }

    #[test]
    fn description_only_dispatchability_agrees_with_has_acceptance() {
        let snap = snapshot("aposg-shape", "## ACCEPTANCE\n1. pins serde\n", "");
        let dispatchable = render(&snap, Path::new("/repo"), None, None).is_ok();
        let captured = has_typed_acceptance(&snap);
        assert_eq!(
            dispatchable, captured,
            "dispatchability={dispatchable} has_acceptance={captured} must agree under B"
        );
        assert!(!dispatchable, "both refuse");
        assert!(!captured, "both refuse");
    }

    #[test]
    fn populated_field_without_heading_stays_accepted() {
        let snap = snapshot(
            "populated",
            "no heading here",
            "typed acceptance\nRun cargo test; expect exit 0",
        );
        assert!(has_typed_acceptance(&snap));
        let packet = render(&snap, Path::new("/repo"), None, None)
            .expect("672 populated-field rows must stay dispatchable");
        assert!(packet.contains("typed acceptance"));
    }

    #[test]
    fn resident_and_render_share_resolve_acceptance() {
        let resident = include_str!("resident.rs");
        assert!(
            resident.contains("dispatch_packet::has_typed_acceptance"),
            "resident.rs must call the unified reader, not trim the bare field"
        );
        let src = include_str!("dispatch_packet.rs");
        assert!(
            src.contains("packet_acceptance(snapshot, bead)"),
            "render must go through packet_acceptance"
        );
        let resolve_hits = src.matches("resolve_acceptance(snapshot)").count();
        assert!(
            resolve_hits >= 3,
            "resolve_acceptance must be the single reader, hits={resolve_hits}"
        );
    }

    #[test]
    fn nonduplicated_815_acceptance_is_carried_in_full() {
        let acceptance = (1..=41)
            .map(|line| format!("{line}. acceptance criterion {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let packet = render(
            &snapshot("omp-orchestrator-815", "scope is elsewhere", &acceptance),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("typed acceptance should render");
        for line in acceptance.lines() {
            assert!(packet.contains(line), "missing acceptance line: {line}");
        }
    }

    #[test]
    fn packet_requires_current_state_revalidation_before_work() {
        let packet = render(
            &snapshot("fixture", "body", "Run cargo test; expect exit 0"),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("packet should render");
        assert!(packet.contains(
            "Every bead requires current-state validation: re-run br show fixture --json immediately before editing;"
        ));
        assert!(packet.contains(
            "if the bead, plan, owner, files, dependencies, or acceptance changed, STOP and report it"
        ));
    }

    #[test]
    fn traps_that_add_acceptance_scope_are_refused() {
        let error = render(
            &snapshot("8e1g", "body", "typed acceptance"),
            Path::new("/repo"),
            None,
            Some("ACCEPTANCE (mine)\n1. MUST add another check"),
        )
        .expect_err("traps must not add acceptance scope");
        assert!(matches!(
            error,
            PacketError::PacketAddsScope { line: 1, .. }
        ));
        assert!(error
            .to_string()
            .contains("br update 8e1g --acceptance-criteria"));
    }

    #[test]
    fn packet_prints_actor_on_comment_and_close() {
        let packet = render(
            &snapshot("omp-orchestrator-gcyf", "body", "typed acceptance"),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("packet should render");
        assert!(
            packet.contains("br comments add omp-orchestrator-gcyf --actor"),
            "a92y/gcyf: comment instruction must print --actor"
        );
        assert!(
            packet.contains("br close omp-orchestrator-gcyf --actor"),
            "8zx1/gcyf: grade close instruction must print --actor"
        );
        assert!(packet.contains("DIFFERENT agent"));
    }

    #[test]
    fn grading_packet_is_distinct_from_a_work_packet() {
        let work = render(
            &snapshot("omp-orchestrator-lwdo.1", "body", "typed acceptance"),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("work");
        let grade = render_grading_packet(
            &BeadSnapshot::new_with_acceptance(
                "omp-orchestrator-lwdo.1",
                "packet fixture",
                "body",
                "typed acceptance",
                "in_progress",
                Some("pane=%3;incarnation=1;agent=WildStone"),
            ),
            Path::new("/repo"),
            "%3",
            "%9",
        )
        .expect("grade");
        assert!(grade.starts_with("GRADE ASSIGNMENT (not implementation)"));
        assert!(!work.starts_with("GRADE ASSIGNMENT"));
        assert!(grade.contains("Grader pane: %3"));
        assert!(grade.contains("Observer: %9"));
        assert!(grade.contains("Do not implement"));
        assert!(grade.contains("Pane: %3"));
        assert!(grade.contains("ACK lwdo.1 on $TMUX_PANE --"));
        // ⛔ THE REGRESSION THIS EXISTS FOR, measured 2026-09-11 and it is the DISPATCHER's
        // defect: 40 beads closed across six agents could not be staged, because every close
        // reason cited a cargo figure and none named where it ran. ack-spine refuses those
        // rows (CLOSE_REASON_WORKER_MISSING, close_reason.rs:218), so a whole session of
        // grading sat closed in the DB and invisible to any clone. Nobody but the CLOSING
        // agent can honestly supply the token afterwards -- a conductor filling it in is
        // forging provenance -- so the only repair is to say it BEFORE the work happens.
        //
        // Same shape as the ACK instruction asserted above: the enforcing half is a tested
        // crate, the instructing half was a sentence in a hand-written packet, and the
        // sentence was never written. Both now come from the renderer.
        assert!(
            grade.contains("worker=<name>") && grade.contains("CLOSE_REASON_WORKER_MISSING"),
            "a grading packet must state the close-reason worker policy: {grade}"
        );
        assert!(
            grade.contains("MINIMAL")
                && grade.contains("SUPERSET")
                && grade.contains("INERT")
                && grade.contains("classify_mutation_minimality"),
            "grading packet must carry fhsyv minimality instruction: {grade}"
        );
    }

    #[test]
    fn grading_packet_refuses_observer_as_grader() {
        let error = render_grading_packet(
            &snapshot("fixture", "body", "typed acceptance"),
            Path::new("/repo"),
            "%9",
            "%9",
        )
        .expect_err("same pane");
        assert!(matches!(error, PacketError::PacketAddsScope { .. }));
    }

    fn packet_for_acceptance(acceptance: &str) -> Result<String, PacketError> {
        render(
            &BeadSnapshot::new_with_acceptance(
                "omp-orchestrator-6we9q",
                "packet fixture",
                "body",
                acceptance,
                "open",
                None,
            ),
            Path::new("/repo"),
            None,
            None,
        )
    }

    #[test]
    fn acceptance_with_no_mutation_clause_still_renders() {
        let packet = packet_for_acceptance("Run cargo test -p dispatch-packet; expect exit 0")
            .expect(
                "item 4 as written would redden every historical bead; no-demand stays admissible",
            );
        assert!(packet.contains("Run cargo test -p dispatch-packet; expect exit 0"));
    }

    #[test]
    fn mutation_verified_close_prefix_is_not_a_mutation_clause() {
        let packet = packet_for_acceptance(
            "Close with MUTATION-VERIFIED after a DIFFERENT pane re-runs cargo test; expect exit 0",
        )
        .expect("close-reason prefixes must not be read as a mutation demand");
        assert!(packet.contains("MUTATION-VERIFIED"));
    }

    #[test]
    fn empty_mutation_heading_is_missing_not_coarse() {
        let error = packet_for_acceptance("1. Mutation:\n2. Run cargo test; expect exit 0")
            .expect_err("empty mutation clause is a distinct population");
        assert_eq!(error.code(), "MUTATION_CLAUSE_MISSING");
        assert_eq!(error.operator_exit_code(), 1);
        assert!(error.to_string().contains("MUTATION_CLAUSE_MISSING"));
        assert!(matches!(error, PacketError::MutationClauseMissing { .. }));
    }

    #[test]
    fn uncall_the_validator_is_too_coarse() {
        let error =
            packet_for_acceptance("Mutation: un-call the validator").expect_err("item 2 known-bad");
        assert_eq!(error.code(), "MUTATION_CLAUSE_TOO_COARSE");
        assert_eq!(error.operator_exit_code(), 1);
        assert!(error.to_string().contains("un-call the validator"));
        assert!(matches!(error, PacketError::MutationClauseTooCoarse { .. }));
    }

    #[test]
    fn uncall_validate_workflows_without_file_line_is_too_coarse() {
        let error = packet_for_acceptance("Mutation: un-call validate_workflows")
            .expect_err("item 1 OR would admit the dwj4v specimen");
        assert_eq!(error.code(), "MUTATION_CLAUSE_TOO_COARSE");
    }

    #[test]
    fn named_file_rs_line_stays_admissible() {
        let packet = packet_for_acceptance(
            "Mutation: duplicate_block_mapping_key at gate-reachability.rs:216",
        )
        .expect("item 2 known-good");
        assert!(packet.contains("gate-reachability.rs:216"));
    }

    #[test]
    fn three_named_site_acceptances_stay_admissible() {
        for acceptance in [
            "Mutation: duplicate_block_mapping_key at gate-reachability.rs:216",
            "KNOWN-BAD: comment out pane_admission at crates/fleet-monitor/src/lib.rs:323",
            "Mutation: un-wire check_referents at allowance_referents.rs:136",
        ] {
            packet_for_acceptance(acceptance)
                .unwrap_or_else(|error| panic!("{acceptance} must stay admissible: {error}"));
        }
    }

    #[test]
    fn missing_and_coarse_use_distinct_codes() {
        let missing = packet_for_acceptance("Mutation:").expect_err("missing");
        let coarse = packet_for_acceptance("Mutation: un-call the validator").expect_err("coarse");
        assert_ne!(missing.code(), coarse.code());
        assert_eq!(missing.code(), "MUTATION_CLAUSE_MISSING");
        assert_eq!(coarse.code(), "MUTATION_CLAUSE_TOO_COARSE");
    }

    #[test]
    fn unwired_and_uncalled_are_not_mutation_demands() {
        for acceptance in [
            "(unwired gates) left (census); expect exit 0",
            "another uncalled crate is not a demand; expect exit 0",
            "UNWIRED is a documented runner status, not an instruction; expect exit 0",
        ] {
            packet_for_acceptance(acceptance)
                .unwrap_or_else(|error| panic!("{acceptance} must stay admissible: {error}"));
        }
    }

    #[test]
    fn unwire_as_its_own_word_is_still_too_coarse() {
        let error = packet_for_acceptance("un-wire the hook with no file.rs:N")
            .expect_err("whole-word un-wire without a site is still a demand");
        assert_eq!(error.code(), "MUTATION_CLAUSE_TOO_COARSE");
    }

    fn write_ledger(root: &Path, id: &str, created: &str) {
        fs::create_dir_all(root.join(".beads")).unwrap();
        fs::write(
            root.join(".beads/issues.jsonl"),
            format!(r#"{{"id":"{id}","created_at":"{created}","status":"open","title":"t"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn pre_cutoff_coarse_is_grandfathered_at_render() {
        let tmp = tempfile::tempdir().unwrap();
        write_ledger(tmp.path(), "old-bead", "2026-09-11T12:00:00Z");
        let packet = render(
            &BeadSnapshot::new_with_acceptance(
                "old-bead",
                "packet fixture",
                "body",
                "Mutation: un-call the validator",
                "open",
                None,
            ),
            tmp.path(),
            None,
            None,
        )
        .expect("pre-cutoff coarse must not refuse dispatch");
        assert!(packet.contains("un-call the validator"));
    }

    #[test]
    fn post_cutoff_coarse_still_refuses_at_render() {
        let tmp = tempfile::tempdir().unwrap();
        write_ledger(tmp.path(), "new-bead", "2026-09-12T12:00:00Z");
        let error = render(
            &BeadSnapshot::new_with_acceptance(
                "new-bead",
                "packet fixture",
                "body",
                "Mutation: un-call the validator",
                "open",
                None,
            ),
            tmp.path(),
            None,
            None,
        )
        .expect_err("post-cutoff coarse must still refuse");
        assert_eq!(error.code(), "MUTATION_CLAUSE_TOO_COARSE");
    }

    #[test]
    fn contains_ascii_word_rejects_unwired_and_uncalled() {
        assert!(contains_ascii_word("un-call the validator", "un-call"));
        assert!(contains_ascii_word("uncall the validator", "uncall"));
        assert!(!contains_ascii_word("unwired gates", "unwire"));
        assert!(!contains_ascii_word("uncalled crate", "uncall"));
        assert!(contains_ascii_word("unwire the hook", "unwire"));
        assert!(!contains_ascii_word("delete the caller", "delete the call"));
        assert!(contains_ascii_word(
            "delete the call from decide()",
            "delete the call"
        ));
    }
}
