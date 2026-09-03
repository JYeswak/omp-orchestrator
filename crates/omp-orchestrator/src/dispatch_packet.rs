use dispatch_claim_fence::BeadSnapshot;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketError {
    PacketFieldMissing(&'static str),
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

impl std::error::Error for PacketError {}

fn nonempty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
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
        "Done: br comments add {bead} --actor <you> \"DONE ...\" then the grader re-runs the bead's acceptance"
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
    let mut packet = format!(
        "Objective: {objective}\n\nTarget: {target}. Read br show {bead} --json IN FULL before starting.\nEvery bead requires current-state validation: re-run br show {bead} --json immediately before editing; validate the bead's assumptions against the current repository and DAG; if the bead, plan, owner, files, dependencies, or acceptance changed, STOP and report it.\n{pane_line}{handoff}\nScope:\n{scope}\n\nAcceptance:\n{acceptance}\n\n{done}\n\nStop: {stop}.\n"
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
}
