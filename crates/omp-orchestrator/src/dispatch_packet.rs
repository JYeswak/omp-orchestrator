use ack_stage::ack_instruction;
use dispatch_claim_fence::BeadSnapshot;
use std::fmt;
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
        }
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

fn acceptance(snapshot: &BeadSnapshot) -> Option<String> {
    nonempty(snapshot.acceptance_criteria())
        .map(ToOwned::to_owned)
        .or_else(|| section_from_description(snapshot.description(), "ACCEPTANCE"))
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
    if let Some(marker) = filed_only_marker_for(snapshot) {
        return Err(PacketError::FiledOnlyRecord {
            bead: bead.to_owned(),
            marker,
        });
    }
    let objective = format!("Complete bead {bead}: {}", snapshot.title().trim());
    let target = target.display().to_string();
    let scope = scope(snapshot);
    let acceptance = acceptance(snapshot).ok_or(PacketError::PacketFieldMissing("acceptance"))?;
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
    Ok(format!(
        "GRADE ASSIGNMENT (not implementation)\nObserver: {observer_pane} (may be WORKING; observer is not the grader).\nGrader pane: {grader_pane}\nDo not implement. Re-run the bead's acceptance. Close with MUTATION-VERIFIED if it holds; otherwise GAP/UNKNOWN.\nYou are not the author if your pane is distinct from ACK pane-scoped keys.\n\n{work}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let error = render(
            &snapshot("fixture", "no acceptance", ""),
            Path::new("/repo"),
            None,
            None,
        )
        .expect_err("missing acceptance must refuse");
        assert_eq!(error, PacketError::PacketFieldMissing("acceptance"));
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
    fn acceptance_falls_back_to_description_block() {
        let packet = render(
            &snapshot(
                "fixture",
                "## ACCEPTANCE\nRun cargo test; expect exit 0\n\n## NO-CLAIM\nlimit",
                "",
            ),
            Path::new("/repo"),
            None,
            None,
        )
        .expect("description acceptance should render");
        assert!(packet.contains("Run cargo test; expect exit 0"));
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
            &snapshot("omp-orchestrator-lwdo.1", "body", "typed acceptance"),
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
}
