//! Close-time lease guard (bead `omp-orchestrator-3w9l`).
//!
//! A file reservation is a lease that does not die with the work it protects:
//! measured 2026-09-06, a holder kept exclusive locks after its bead CLOSED,
//! and the listing surface showed nothing to the conductor. The daemon (an
//! external project) offers NO list-all-leases tool — probed 2026-09-12 over
//! all 45 advertised tools, schemas read: `check_file_reservation_conflicts`
//! and `file_reservation_paths` both require a path list, `release_*` needs
//! holder + paths/ids, `force_release_file_reservation` needs a reservation
//! id. So the gate cannot discover leases by bead id, and this module does
//! not pretend otherwise: the bead carries its own lease record, and the
//! gate verifies THAT record against the authoritative conflict endpoint.
//!
//! Record convention, one line per lease set, in the bead body or comments:
//!
//! ```text
//! LEASE bead=<bead-id> holder=<mail-name> [ack=<ack-name>] paths=<p1>,<p2>,...
//! ```
//!
//! `holder` is the Agent Mail identity (the only name the daemon answers
//! to); `ack` is the tracker/ACK name (a hub/pane handle the daemon can
//! never mint — descriptive names are refused at registration, so the two
//! namespaces are structurally disjoint and the join MUST be recorded, never
//! inferred). The refusal text prints both: that printed pair IS the join.
//!
//! Parsing is fail-closed: a line starting with `LEASE` that is not a valid
//! record is an error, never a skipped line. A typo'd record that silently
//! passed would be a close the guard claimed to check and did not.

use crate::journey::{
    check_conflicts_authoritative, AgentName, ProjectKey, ReservationConflictReport,
};
use crate::{MailClient, MailError};
use asupersync::Cx;

/// One bead's declared lease set, parsed from a `LEASE` record line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRecord {
    /// The bead whose close this record guards.
    pub bead_id: String,
    /// The Agent Mail identity holding the lease (daemon namespace).
    pub holder_mail: String,
    /// The tracker/ACK name, when the claim bound one (tracker namespace).
    pub ack_name: Option<String>,
    /// Reserved paths, as written at reserve time.
    pub paths: Vec<String>,
}

/// A malformed `LEASE` record line. Fail-closed by construction: there is no
/// "skip this line" outcome, so callers cannot launder a typo into a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseRecordError {
    /// The offending line, truncated for logs.
    pub line: String,
    /// What was wrong with it.
    pub detail: String,
}

impl std::fmt::Display for LeaseRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "LEASE_RECORD_INVALID line={:?} detail={}",
            self.line, self.detail
        )
    }
}

impl std::error::Error for LeaseRecordError {}

/// Parse every `LEASE` record in bead body/comment text.
///
/// Returns the records in line order. A line whose first token is `LEASE`
/// but which is not a valid record is an error covering the whole parse:
/// partial record sets are how a close slips past with half its leases
/// unchecked.
pub fn parse_lease_records(text: &str) -> Result<Vec<LeaseRecord>, LeaseRecordError> {
    let mut records = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("LEASE") => records.push(parse_lease_line(line)?),
            _ => continue,
        }
    }
    Ok(records)
}

fn parse_lease_line(line: &str) -> Result<LeaseRecord, LeaseRecordError> {
    let invalid = |detail: &str| LeaseRecordError {
        line: line.chars().take(160).collect(),
        detail: detail.to_owned(),
    };
    let mut bead_id: Option<String> = None;
    let mut holder_mail: Option<String> = None;
    let mut ack_name: Option<String> = None;
    let mut paths: Option<Vec<String>> = None;
    for token in line.split_whitespace().skip(1) {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| invalid("token without key=value shape"))?;
        if value.is_empty() {
            return Err(invalid("empty value"));
        }
        match key {
            "bead" => {
                if bead_id.is_some() {
                    return Err(invalid("duplicate bead key"));
                }
                bead_id = Some(value.to_owned());
            }
            "holder" => {
                if holder_mail.is_some() {
                    return Err(invalid("duplicate holder key"));
                }
                holder_mail = Some(value.to_owned());
            }
            "ack" => {
                if ack_name.is_some() {
                    return Err(invalid("duplicate ack key"));
                }
                ack_name = Some(value.to_owned());
            }
            "paths" => {
                if paths.is_some() {
                    return Err(invalid("duplicate paths key"));
                }
                let listed: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_owned)
                    .collect();
                if listed.is_empty() {
                    return Err(invalid("paths list names no path"));
                }
                paths = Some(listed);
            }
            _ => return Err(invalid("unknown key")),
        }
    }
    Ok(LeaseRecord {
        bead_id: bead_id.ok_or_else(|| invalid("missing bead key"))?,
        holder_mail: holder_mail.ok_or_else(|| invalid("missing holder key"))?,
        ack_name,
        paths: paths.ok_or_else(|| invalid("missing paths key"))?,
    })
}

/// One still-held lease: a conflict row that matches the bead's own record.
///
/// The `holder_mail` comes from the daemon's conflict row, the `ack_name`
/// from the bead record. Carrying both in one struct is the item-3 join:
/// neither namespace alone identifies the lease to both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldLease {
    pub path: String,
    pub holder_mail: String,
    pub ack_name: Option<String>,
    pub exclusive: bool,
}

/// Match a bead record against an authoritative conflict report.
///
/// A row matches only when ALL THREE hold: the path is one of the record's,
/// the row is exclusive, and the row's agent is the record's holder. A lease
/// on the same path held by someone else belongs to another bead's record;
/// matching it here would refuse a close whose own lease is already gone.
#[must_use]
pub fn match_held_leases(
    record: &LeaseRecord,
    report: &ReservationConflictReport,
) -> Vec<HeldLease> {
    report
        .conflicts()
        .iter()
        .filter(|conflict| {
            conflict.exclusive
                && conflict.agent == record.holder_mail
                && record.paths.iter().any(|path| path == &conflict.path)
        })
        .map(|conflict| HeldLease {
            path: conflict.path.clone(),
            holder_mail: conflict.agent.clone(),
            ack_name: record.ack_name.clone(),
            exclusive: conflict.exclusive,
        })
        .collect()
}

/// Render the close-commit refusal. Returns `None` when nothing is held so a
/// caller cannot print a refusal over an empty set.
///
/// The text names the mail holder AND the ACK name (the join), the exact
/// paths, and the release path: the holder releases via
/// `release_file_reservations`; a genuinely abandoned lease goes through
/// `force_release_file_reservation`, which checks abandonment heuristics
/// itself. The gate never releases: a pre-commit hook dropping another
/// agent's lease would be the destructive half of this guard.
#[must_use]
pub fn lease_refusal_text(bead_id: &str, held: &[HeldLease]) -> Option<String> {
    if held.is_empty() {
        return None;
    }
    let mut paths: Vec<&str> = held.iter().map(|lease| lease.path.as_str()).collect();
    paths.sort();
    paths.dedup();
    let holders: Vec<String> = {
        let mut seen = Vec::new();
        for lease in held {
            let ack = lease.ack_name.as_deref().unwrap_or("none");
            let rendered = format!("{} ack={}", lease.holder_mail, ack);
            if !seen.contains(&rendered) {
                seen.push(rendered);
            }
        }
        seen
    };
    Some(format!(
        "lease-guard: REFUSED bead={bead_id} still-held={} holder={} paths={} -- \
         release via release_file_reservations (holder) or force_release_file_reservation \
         (abandoned, heuristics-checked); then re-commit",
        held.len(),
        holders.join(","),
        paths.join(",")
    ))
}

/// True iff a daemon tool name is a list-all-leases endpoint.
///
/// Deliberately narrow: `file_reservation_paths` (the TAKE endpoint) and
/// `check_file_reservation_conflicts` (per-path query) both contain
/// "reserv" and neither enumerates, so "contains reserv" alone would admit
/// them and the ABSENT verdict below would be unreachable. Probed against
/// the live 45-tool catalogue 2026-09-12: zero matches.
#[must_use]
pub fn catalogue_has_lease_listing(tools: &[String]) -> bool {
    tools.iter().any(|name| {
        let lower = name.to_lowercase();
        lower.contains("reserv")
            && (lower.contains("list") || lower.contains("all_leases") || lower.contains("enumerat"))
    })
}

/// The enumeration the guard wishes it had, refused as typed ABSENT.
///
/// There is no daemon tool that lists every live exclusive lease, so any
/// "no leases held" derived from the conflict endpoint's empty set is a
/// per-path answer, never a global one. This function exists so the ABSENT
/// is a value callers must handle rather than a comment they must remember:
/// it probes the catalogue and returns [`MailError::NoLeaseEnumeration`]
/// naming the catalogue size. When the daemon gains the tool, the live leg
/// below reddens and this function is where the call lands.
pub async fn enumerate_exclusive_leases(
    cx: &Cx,
    client: &MailClient,
) -> Result<Vec<HeldLease>, MailError> {
    let tools = client.list_tools(cx).await?;
    if catalogue_has_lease_listing(&tools) {
        return Err(MailError::Protocol {
            detail: format!(
                "daemon now advertises a lease-listing tool (catalogue={}); \
                 enumerate_exclusive_leases must call it instead of refusing",
                tools.len()
            ),
        });
    }
    Err(MailError::NoLeaseEnumeration {
        catalogue_tools: tools.len(),
    })
}

/// The verdict of verifying one bead record against the live daemon.
///
/// There is no "assumed clear" arm: an empty conflict set over the record's
/// paths is `Released`, every daemon failure is `DaemonError`, and the two
/// are different types so a caller cannot collapse them into a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseLeaseVerdict {
    /// No recorded path is exclusively held by the record's holder.
    Released { checked_paths: usize },
    /// At least one recorded path is still exclusively held.
    StillHeld { held: Vec<HeldLease> },
    /// The daemon could not be consulted (down, auth, timeout, malformed).
    /// Fail-closed input to the commit gate: an ERROR for that commit,
    /// never a pass. Tracker closes are unaffected — this verdict only
    /// exists on the staged-mirror commit path.
    DaemonError { detail: String },
}

/// Verify one lease record against the live daemon as the gate identity.
///
/// `gate_agent` must be a registered identity that is never the recorded
/// holder (a pinned gate identity, registered once): the conflict endpoint
/// reports OTHER agents' leases, so querying as the holder would hide the
/// very lease under test behind the own-lease partition.
pub async fn verify_close_lease(
    cx: &Cx,
    client: &MailClient,
    project: &ProjectKey,
    gate_agent: &AgentName,
    record: &LeaseRecord,
) -> CloseLeaseVerdict {
    let report = match check_conflicts_authoritative(
        cx,
        client,
        project,
        gate_agent,
        &record.paths,
    )
    .await
    {
        Ok(report) => report,
        Err(error) => return CloseLeaseVerdict::DaemonError {
            detail: error.to_string(),
        },
    };
    let held = match_held_leases(record, &report);
    if held.is_empty() {
        CloseLeaseVerdict::Released {
            checked_paths: record.paths.len(),
        }
    } else {
        CloseLeaseVerdict::StillHeld { held }
    }
}
