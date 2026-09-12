#![forbid(unsafe_code)]

//! Admission by measured packet input dependency.
//!
//! The docs gate is a condition on packets that carry plan-document input, not
//! a blanket veto over every kind of work. This module keeps that distinction
//! typed and keeps the policy authorization separate from the pure classifier.

use admission_reason::Rule;
use omp_types::{DispatchAdmissibility, DispatchPacketClass};
use std::fmt;

const PLAN_NEEDLES: [&str; 2] = ["PLAN.md", "docs/plan"];
const GRADING_MARKER: &str = "GRADE ASSIGNMENT (not implementation)";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacketAdmissionError {
    EmptyPacket,
}

impl fmt::Display for PacketAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPacket => formatter.write_str(
                "PACKET_ADMISSION_ERROR reason=EMPTY_PACKET_CLASS -- no renderer input was classified",
            ),
        }
    }
}

impl std::error::Error for PacketAdmissionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketAdmission {
    pub packet_class: DispatchPacketClass,
    pub verdict: DispatchAdmissibility,
    pub reads_plan_document: bool,
    pub reason: String,
}

impl PacketAdmission {
    pub fn is_admitted(&self) -> bool {
        matches!(
            self.verdict,
            DispatchAdmissibility::Allowed | DispatchAdmissibility::Degraded { .. }
        )
    }

    pub fn naming_gate(&self) -> Option<&'static str> {
        self.verdict.naming_gate()
    }

    pub fn refused_class(&self) -> Option<DispatchPacketClass> {
        self.verdict.refused_class()
    }
}

/// Classify the renderer output by an explicit renderer marker.
///
/// A normal rendered packet is Work; a grading packet is identified by the
/// renderer's own stable marker. Empty input is an instrument error, never an
/// implicit Work class.
pub fn classify_packet(packet: &str) -> Result<DispatchPacketClass, PacketAdmissionError> {
    if packet.trim().is_empty() {
        return Err(PacketAdmissionError::EmptyPacket);
    }
    if packet.contains(GRADING_MARKER) {
        Ok(DispatchPacketClass::Grading)
    } else {
        Ok(DispatchPacketClass::Work)
    }
}

/// Whether the emitted packet carries a plan-document dependency.
pub fn reads_plan_document(packet: &str) -> bool {
    packet.lines().any(|line| {
        let trimmed = line.trim_start();
        !trimmed.starts_with('#') && PLAN_NEEDLES.iter().any(|needle| line.contains(needle))
    })
}

/// Classify a packet against the docs freshness gate.
///
/// `degraded_authorized` is supplied by the durable Joshua decision row. The
/// policy switch is therefore off for fixtures or deployments that have not
/// recorded that authority, even when the packet itself is independent.
pub fn evaluate(
    packet: &str,
    docs_stale: bool,
    degraded_authorized: bool,
) -> Result<PacketAdmission, PacketAdmissionError> {
    let packet_class = classify_packet(packet)?;
    evaluate_for_class(packet, packet_class, docs_stale, degraded_authorized)
}

/// Evaluate a packet when the caller has a typed renderer class already. The
/// Grade hold is an explicit renderer path, not a stakes heuristic.
pub fn evaluate_for_class(
    packet: &str,
    packet_class: DispatchPacketClass,
    docs_stale: bool,
    degraded_authorized: bool,
) -> Result<PacketAdmission, PacketAdmissionError> {
    if packet.trim().is_empty() {
        return Err(PacketAdmissionError::EmptyPacket);
    }
    let reads_plan_document = reads_plan_document(packet);
    let (verdict, reason) = if !docs_stale {
        (DispatchAdmissibility::Allowed, "docs_fresh".to_owned())
    } else if reads_plan_document {
        (
            DispatchAdmissibility::Refused,
            "packet_contains_plan_document_input".to_owned(),
        )
    } else if degraded_authorized {
        (
            DispatchAdmissibility::degraded(DispatchPacketClass::PlanDependent),
            format!("naming_gate={}", Rule::NameTheFailingGate.as_str()),
        )
    } else {
        (
            DispatchAdmissibility::Refused,
            "HD-0015_missing_or_unconfirmed".to_owned(),
        )
    };
    Ok(PacketAdmission {
        packet_class,
        verdict,
        reads_plan_document,
        reason,
    })
}

/// The durable policy amendment that authorizes the degraded arm.
pub fn authority_allows_degraded(decisions_jsonl: &str) -> bool {
    decisions_jsonl
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|row| {
            row.get("id").and_then(serde_json::Value::as_str) == Some("HD-0015")
                && row
                    .get("decision")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|decision| decision.contains("SCOPE IT"))
                && row
                    .get("condition")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|condition| condition.contains("DEGRADED"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPLETE: &str =
        "Objective: x\nTarget: y\nScope:\npacket\nAcceptance:\nrun\nDone: exit 0\nStop: now\n";

    #[test]
    fn empty_packet_is_an_instrument_error() {
        assert_eq!(classify_packet(""), Err(PacketAdmissionError::EmptyPacket));
        assert_eq!(
            classify_packet("   "),
            Err(PacketAdmissionError::EmptyPacket)
        );
    }

    #[test]
    fn renderer_source_has_no_plan_read_and_positive_control_is_present() {
        let renderer = include_str!("dispatch_packet.rs");
        assert!(!renderer.contains("PLAN.md"));
        assert!(!renderer.contains("docs/plan"));
        // `read_to_string` is not a plan read. The renderer looks up
        // `.beads/issues.jsonl` created_at for the mutation-site grandfather
        // (6we9q). Banning the fs primitive conflated those.
        assert!(
            renderer.contains(".beads/issues.jsonl"),
            "grandfather lookup must stay in the renderer, not a side channel"
        );
        assert!(renderer.contains("PACKET_REFUSED_FILED_ONLY"));
    }

    #[test]
    fn stale_plan_dependent_packet_is_refused() {
        let packet = format!("{COMPLETE}Read docs/PLAN.md before dispatch.");
        let admission = evaluate(&packet, true, true).expect("classify plan packet");
        assert_eq!(admission.packet_class, DispatchPacketClass::Work);
        assert!(admission.reads_plan_document);
        assert_eq!(admission.verdict, DispatchAdmissibility::Refused);
        assert!(!admission.is_admitted());
    }

    #[test]
    fn stale_grading_packet_is_degraded_and_names_the_gate() {
        let packet = format!("{GRADING_MARKER}\n{COMPLETE}");
        let admission = evaluate(&packet, true, true).expect("classify grade packet");
        assert_eq!(admission.packet_class, DispatchPacketClass::Grading);
        assert!(!admission.reads_plan_document);
        assert!(admission.is_admitted());
        assert_eq!(admission.verdict.as_str(), "degraded");
        assert_eq!(
            admission.refused_class(),
            Some(DispatchPacketClass::PlanDependent)
        );
        assert_eq!(
            admission.naming_gate(),
            Some(Rule::NameTheFailingGate.as_str())
        );
        assert!(admission.reason.contains("name_the_failing_gate"));
    }

    #[test]
    fn fresh_tree_is_allowed_for_both_renderer_classes() {
        for packet in [COMPLETE, &format!("{GRADING_MARKER}\n{COMPLETE}")] {
            let admission = evaluate(packet, false, false).expect("fresh admission");
            assert_eq!(admission.verdict, DispatchAdmissibility::Allowed);
            assert!(admission.is_admitted());
        }
    }

    #[test]
    fn degraded_policy_requires_the_durable_authority_row() {
        let packet = format!("{GRADING_MARKER}\n{COMPLETE}");
        assert_eq!(
            evaluate(&packet, true, false)
                .expect("policy refusal is still a typed result")
                .verdict,
            DispatchAdmissibility::Refused
        );
        let row = r#"{"id":"HD-0015","decision":"SCOPE IT","condition":"DEGRADED names the gate"}"#;
        assert!(authority_allows_degraded(row));
    }
}
