#![forbid(unsafe_code)]

//! Pure decision logic for `omp-idle-dispatch`.
//!
//! The shell lane's high-risk rule is intentionally positional: scrollback is not state.
//! We select the last model-banner line, classify it fail-closed, and require two idle
//! observations before a reversible packet send is planned. Filesystem, clocks, tmux, and
//! process spawning stay in the binary so these guards are exercised by deterministic tests.

use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const LANE: &str = "omp-idle-dispatch";
pub const MODEL_BANNER: &str = "GPT-5.6-Luna";
pub const DEFAULT_COOLDOWN_SECONDS: u64 = 180;
pub const DEFAULT_CONFIRM_SECONDS: u64 = 20;
pub const QUEUE_WIDTH: usize = 3;
pub const ACCEPTANCE_FALLBACK: &str =
    "NO ACCEPTANCE IN BEAD — write one as your first act, then satisfy it.";

/// The only pane states that may reach dispatch planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleDispatchPaneState {
    Working,
    Idle,
    Unknown,
}

impl IdleDispatchPaneState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Working => "WORKING",
            Self::Idle => "IDLE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Result of the two-capture guard. `Changed` records that the first screenshot looked idle
/// but the second did not; it must never be collapsed into `Working`, because that would hide
/// a race in the evidence and make the mutation boundary harder to audit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleConfirmation {
    Idle,
    Working,
    Unknown,
    Changed,
}

impl IdleConfirmation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Working => "WORKING",
            Self::Unknown => "UNKNOWN",
            Self::Changed => "CHANGED",
        }
    }
}

fn is_working_banner(line: &str) -> bool {
    let mut chars = line.trim_start().chars();
    let Some(spinner) = chars.next() else { return false };
    if !(('\u{2800}'..='\u{28ff}').contains(&spinner)) {
        return false;
    }
    let rest = chars.as_str();
    let rest = rest.trim_start_matches(char::is_whitespace);
    let digit_count = rest.chars().take_while(char::is_ascii_digit).count();
    if digit_count == 0 {
        return false;
    }
    let unit = rest.chars().nth(digit_count);
    if !matches!(unit, Some('s' | 'm' | 'h')) {
        return false;
    }
    !matches!(rest.chars().nth(digit_count + 1), Some(c) if c.is_ascii_alphanumeric() || c == '_')
}

/// Classify a pane capture using the last model-banner line only.
///
/// This positional anchor prevents a stale spinner in scrollback from winning over a live
/// idle prompt. Unknown and banner-less captures refuse dispatch rather than guessing idle.
#[must_use]
pub fn classify_capture(capture: &str) -> IdleDispatchPaneState {
    let Some(status) = capture
        .lines()
        .filter(|line| line.contains(MODEL_BANNER))
        .next_back()
    else {
        return IdleDispatchPaneState::Unknown;
    };
    if is_working_banner(status) {
        IdleDispatchPaneState::Working
    } else if status.trim_start().starts_with('π') {
        IdleDispatchPaneState::Idle
    } else {
        IdleDispatchPaneState::Unknown
    }
}

/// Require idle evidence in both captures, independent of cooldown settings.
#[must_use]
pub const fn confirm_idle(first: IdleDispatchPaneState, second: IdleDispatchPaneState) -> IdleConfirmation {
    match first {
        IdleDispatchPaneState::Working => IdleConfirmation::Working,
        IdleDispatchPaneState::Unknown => IdleConfirmation::Unknown,
        IdleDispatchPaneState::Idle => match second {
            IdleDispatchPaneState::Idle => IdleConfirmation::Idle,
            IdleDispatchPaneState::Working | IdleDispatchPaneState::Unknown => IdleConfirmation::Changed,
        },
    }
}

/// Classify two raw captures with the production two-capture rule.
#[must_use]
pub fn confirm_capture_pair(first: &str, second: &str) -> IdleConfirmation {
    confirm_idle(classify_capture(first), classify_capture(second))
}
/// Return true only when a target capture records a real transition into named bead work.
///
/// The sender's exit status is intentionally absent from this predicate. A receiver proof needs
/// a changed target capture, the exact bead identifier in that capture, and a working model-banner
/// state after the send. A stale packet already in scrollback therefore cannot satisfy it.
#[must_use]
pub fn receiver_transition(before: &str, after: &str, bead_id: &str) -> bool {
    !bead_id.trim().is_empty()
        && before != after
        && !before.contains(bead_id)
        && after.contains(bead_id)
        && classify_capture(after) == IdleDispatchPaneState::Working
}

/// A ready bead with the acceptance carried into its packet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadyBead {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: i64,
}

fn acceptance_from_description(description: &str) -> String {
    let start = ["## ACCEPTANCE", "ACCEPTANCE", "## Acceptance"]
        .iter()
        .filter_map(|marker| description.find(marker))
        .min();
    let Some(start) = start else { return ACCEPTANCE_FALLBACK.to_string() };
    let extracted: String = description[start..].chars().take(420).collect();
    let flattened = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    flattened.replace('`', "'").replace('$', "S")
}

/// Parse and rank ready beads without trusting malformed queue payloads.
///
/// Both the list and `{ "issues": [...] }` forms emitted by `br` are accepted. Epics and
/// non-open rows are skipped, while malformed JSON produces an empty queue (never a guessed
/// target). Priority sorting is stable for equal priorities, matching Python's stable sort.
#[must_use]
pub fn pick_beads(json: &str, limit: usize) -> Vec<ReadyBead> {
    let Ok(value) = serde_json::from_str::<Value>(json) else { return Vec::new() };
    let rows = match value {
        Value::Array(rows) => rows,
        Value::Object(mut object) => match object.remove("issues") {
            Some(Value::Array(rows)) => rows,
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let mut selected = Vec::new();
    for row in rows {
        let Value::Object(object) = row else { continue };
        let id = object.get("id").and_then(Value::as_str).unwrap_or_default();
        let issue_type = object
            .get("issue_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if id.to_ascii_lowercase().contains("epic")
            || issue_type.eq_ignore_ascii_case("epic")
            || status != "open"
        {
            continue;
        }
        if id.is_empty() {
            continue;
        }
        let description = object
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let priority = object
            .get("priority")
            .and_then(Value::as_i64)
            .unwrap_or(9);
        let title: String = title.chars().take(110).collect();
        selected.push(ReadyBead {
            id: id.to_string(),
            title,
            description: acceptance_from_description(description),
            priority,
        });
    }
    selected.sort_by_key(|bead| bead.priority);
    selected.truncate(limit);
    selected
}

/// A cursor-based queue slice. The cursor advances on attempt, not send success.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuePlan {
    pub pane_queues: Vec<Vec<ReadyBead>>,
    pub next_cursor: usize,
}

/// Allocate disjoint three-bead queues to panes. Every pane attempt consumes three cursor
/// positions, including a failed send, preventing duplicate delivery after one error.
#[must_use]
pub fn plan_queues(beads: &[ReadyBead], pane_count: usize, cursor: usize) -> QueuePlan {
    let mut pane_queues = Vec::with_capacity(pane_count);
    let mut next_cursor = cursor;
    for _ in 0..pane_count {
        let start = next_cursor;
        next_cursor = next_cursor.saturating_add(QUEUE_WIDTH);
        let queue = beads
            .get(start..start.saturating_add(QUEUE_WIDTH).min(beads.len()))
            .unwrap_or_default()
            .to_vec();
        if queue.is_empty() {
            break;
        }
        pane_queues.push(queue);
    }
    QueuePlan {
        pane_queues,
        next_cursor,
    }
}

/// Render the literal packet sent to an idle pane.
#[must_use]
pub fn render_packet(timestamp: &str, ready_count: usize, queue: &[ReadyBead]) -> String {
    let mut packet = format!(
        concat!(
            "AUTO-DISPATCH {timestamp} ({lane}, cron)\n\n",
            "You were idle at a prompt with {ready_count} ready beads. Take the FIRST bead below and ship it.\n\n",
            "QUEUE — work them IN ORDER. Each carries its own ACCEPTANCE; that is your definition of done.\n"
        ),
        timestamp = timestamp,
        lane = LANE,
        ready_count = ready_count,
    );
    for bead in queue {
        packet.push_str(&format!(
            "  {}\n    WHAT: {}\n    {}\n",
            bead.id, bead.title, bead.description
        ));
    }
    packet.push_str(concat!(
        "HOW THIS GOES (ship-or-surface — do not open with a triage phase):\n",
        " 1. CLAIM it: br update <id> --status=in_progress\n",
        " 2. RESERVE the files you will edit (Agent Mail reservation, narrow paths — not '**/*').\n",
        " 3. EDIT THE CODE. No prose, no mental models, no summaries until a commit lands.\n",
        " 4. VERIFY as the ACCEPTANCE line demands — run the command, show the output.\n",
        " 5. COMMIT path-scoped: git commit -- <explicit paths>. Never -A. Shared index, 3 agents.\n",
        " 6. CLOSE with the sha and the measurement.\n\n",
        "'ALREADY IMPLEMENTED' IS NOT AN EXIT. It is one of exactly three outcomes, and two of them\n",
        "still end in a commit:\n",
        " (a) The ACCEPTANCE is fully satisfied by code already on main -> CLOSE THE BEAD NOW, citing\n",
        " the sha and the command output that proves it.\n",
        " (b) PARTIALLY implemented -> the REMAINING WORK IS YOUR TARGET. 'Some of it exists' is a\n",
        " smaller bead, never an empty queue. Name the gap and close it with code.\n",
        " (c) The bead is genuinely wrong or obsolete -> close it WONTFIX with the reason, or file\n",
        " the corrected bead and close this one as superseded.\n",
        "There is no fourth branch.\n\n",
        "CLOSE MECHANICS: the reason MUST START WITH MUTATION-VERIFIED / DONE / APPROVED / WONTFIX.\n",
        "Most of these are blocked by the PARENT EPIC cp-epic-fleet-work-quality-08l6.74; when it\n",
        "is the ONLY blocker, use --force and say so in the reason with the epic id.\n\n",
        "CREDIT RULES: real code + real tests in the SAME work item. Refusal-only implementation\n",
        "never closes a positive-capability item. Label it refusal-only and leave it open.\n",
        "NO-CLAIM LINE REQUIRED: state what green does NOT prove.\n",
    ));
    packet
}

/// A parsed dispatch ledger marker. No writer PID participates in identity: the marker is
/// keyed to the lane/pane/bead dispatch itself, so transient process replacement cannot bypass
/// the accept-latency guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchMarker {
    pub lane: String,
    pub pane: String,
    pub bead: String,
    pub timestamp: String,
}

impl DispatchMarker {
    pub fn identity(&self) -> String {
        format!("{}:{}:{}", self.lane, self.pane, self.bead)
    }
}

/// Parse one JSONL dispatch marker. Invalid timestamps and wrong actions are ignored.
#[must_use]
pub fn parse_dispatch_marker(line: &str) -> Option<DispatchMarker> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    if value.get("action").and_then(Value::as_str) != Some("dispatched") {
        return None;
    }
    let lane = value.get("lane").and_then(Value::as_str)?.to_string();
    let pane = value.get("pane").and_then(Value::as_str)?.to_string();
    let timestamp = value.get("ts").and_then(Value::as_str)?.to_string();
    if parse_utc_seconds(&timestamp).is_none() {
        return None;
    }
    Some(DispatchMarker {
        lane,
        pane,
        bead: value
            .get("bead")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        timestamp,
    })
}

/// Return whether a pane has a dispatch marker newer than the cutoff. `writer_pid`, if present
/// in a ledger row, is deliberately ignored.
#[must_use]
pub fn recently_dispatched(
    ledger: &str,
    pane: &str,
    now: SystemTime,
    cooldown: Duration,
) -> bool {
    let Ok(now) = now.duration_since(UNIX_EPOCH) else { return true };
    let cutoff = now.as_secs().saturating_sub(cooldown.as_secs());
    ledger.lines().filter_map(parse_dispatch_marker).any(|marker| {
        marker.lane == LANE
            && marker.pane == pane
            && parse_utc_seconds(&marker.timestamp)
                .is_some_and(|timestamp| timestamp >= cutoff)
    })
}

fn parse_utc_seconds(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T'
        || bytes[13] != b':' || bytes[16] != b':' || bytes[19] != b'Z'
    {
        return None;
    }
    let number = |start: usize, end: usize| -> Option<u32> {
        value.get(start..end)?.parse().ok()
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=12).contains(&month)
        || !(1..=max_day).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let y = i64::from(year) - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let year_of_era = y - era * 400;
    let month_prime = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + u64::from(hour * 3_600 + minute * 60 + second))
}

/// The lane's three-class result. A saturated fleet is BLOCKED, not RED: RED increments the
/// drift alarm while no-idle-capacity is the healthy external condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TickVerdict {
    Green,
    BlockedNoIdleCapacity,
    BlockedDriverDidNotDispatch,
    RedSendFailed,
}

impl TickVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Green => "GREEN",
            Self::BlockedNoIdleCapacity | Self::BlockedDriverDidNotDispatch => "BLOCKED",
            Self::RedSendFailed => "RED",
        }
    }
}

/// Classify an observed tick without consulting the standing `check.sh` verdict.
#[must_use]
pub const fn classify_tick(omp_seen: usize, idle_found: usize, dispatched: usize, send_failed: bool) -> TickVerdict {
    if send_failed {
        TickVerdict::RedSendFailed
    } else if idle_found == 0 {
        TickVerdict::BlockedNoIdleCapacity
    } else if dispatched > 0 {
        TickVerdict::BlockedDriverDidNotDispatch
    } else {
        let _ = omp_seen;
        TickVerdict::Green
    }
}

/// The typed blocker emitted for a saturated or backstop-dispatched tick.
#[must_use]
pub fn blocker_fields(verdict: TickVerdict, omp_seen: usize, idle_found: usize, ready: usize, dispatched: usize) -> Option<(String, String)> {
    match verdict {
        TickVerdict::BlockedNoIdleCapacity => Some((
            "infrastructure:no-idle-capacity".to_string(),
            format!("observed omp_panes={omp_seen} idle={idle_found} ready={ready}; capacity saturated"),
        )),
        TickVerdict::BlockedDriverDidNotDispatch => Some((
            "infrastructure:driver-did-not-dispatch".to_string(),
            format!("backstop dispatched {dispatched} pane(s) the wave driver left idle; ready={ready}"),
        )),
        TickVerdict::Green | TickVerdict::RedSendFailed => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDLE: &str = "π  > ◒ GPT-5.6-Luna > ⏸ Goal 555K > S37.17";
    const WORKING: &str = " ⠏ 10m  > ◒ GPT-5.6-Luna > S34.94";
    /// Prove a receiver-side state transition for one dispatched bead.
    ///
    /// A sender result is not sufficient: the target capture must change, name the bead, and show
    /// the worker in its working state. The pre-capture is passed separately so a stale packet already
    /// present in scrollback cannot count as new delivery.
    #[test]
    fn receiver_proof_requires_named_bead_and_working_transition() {
        let before = IDLE;
        let after = format!("{before}\nAUTO-DISPATCH cp-1\n{WORKING}");
        assert!(receiver_transition(before, &after, "cp-1"));
        assert!(!receiver_transition(before, &format!("{before}\n{WORKING}"), "cp-1"));
        assert!(!receiver_transition(&after, &after, "cp-1"));
        assert!(!receiver_transition(before, &format!("{before}\nAUTO-DISPATCH cp-2\n{WORKING}"), "cp-1"));
    }

    #[test]
    fn stale_spinner_above_live_idle_prompt_reads_idle() {
        let capture = " ⠹ 22m · ◒ GPT-5.6-Luna\nπ  > ◒ GPT-5.6-Luna > S37.17\n";
        assert_eq!(classify_capture(capture), IdleDispatchPaneState::Idle);
    }

    #[test]
    fn quoted_idle_above_live_spinner_reads_working() {
        let capture = "π  > ◒ GPT-5.6-Luna > quoted evidence\n ⠏ 10m > ◒ GPT-5.6-Luna > S34.94";
        assert_eq!(classify_capture(capture), IdleDispatchPaneState::Working);
    }

    #[test]
    fn unknown_shapes_and_plain_shell_fail_closed() {
        assert_eq!(classify_capture("??? > ◒ GPT-5.6-Luna > unknown"), IdleDispatchPaneState::Unknown);
        assert_eq!(classify_capture("josh@studio ~ % ls\nbin crates"), IdleDispatchPaneState::Unknown);
    }

    #[test]
    fn genuine_idle_and_working_are_both_detected() {
        let idle_capture = "TODO_COUNT=0\n".to_owned() + IDLE;
        assert_eq!(classify_capture(&idle_capture), IdleDispatchPaneState::Idle);
        assert_eq!(classify_capture(WORKING), IdleDispatchPaneState::Working);
    }

    #[test]
    fn unanchored_mutation_misreads_stale_scrollback_as_working() {
        let capture = " ⠹ 22m · ◒ GPT-5.6-Luna\nπ  > ◒ GPT-5.6-Luna > S37.17";
        let first = capture.lines().find(|line| line.contains(MODEL_BANNER)).unwrap_or_default();
        assert!(is_working_banner(first));
        assert_eq!(classify_capture(capture), IdleDispatchPaneState::Idle);
    }

    #[test]
    fn two_captures_are_required_and_changed_is_not_idle() {
        assert_eq!(confirm_idle(IdleDispatchPaneState::Idle, IdleDispatchPaneState::Idle), IdleConfirmation::Idle);
        assert_eq!(confirm_idle(IdleDispatchPaneState::Idle, IdleDispatchPaneState::Working), IdleConfirmation::Changed);
        assert_eq!(confirm_idle(IdleDispatchPaneState::Idle, IdleDispatchPaneState::Unknown), IdleConfirmation::Changed);
        assert_eq!(confirm_idle(IdleDispatchPaneState::Working, IdleDispatchPaneState::Idle), IdleConfirmation::Working);
        assert_eq!(confirm_idle(IdleDispatchPaneState::Unknown, IdleDispatchPaneState::Idle), IdleConfirmation::Unknown);
    }

    #[test]
    fn malformed_queue_is_empty_and_epics_closed_rows_are_skipped() {
        assert!(pick_beads("not json", 12).is_empty());
        let json = r###"{"issues":[
          {"id":"cp-epic","status":"open","issue_type":"epic","priority":0},
          {"id":"cp-closed","status":"closed","priority":1},
          {"id":"cp-epic-name","status":"open","priority":1},
          {"id":"cp-real","status":"open","priority":2,"title":"Ship it","description":"## ACCEPTANCE run test; expect green"}
        ]}"###;
        let rows = pick_beads(json, 12);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "cp-real");
    }

    #[test]
    fn queue_carries_acceptance_and_sanitizes_shell_metacharacters() {
        let json = r###"[{"id":"cp-1","status":"open","priority":2,"title":"A","description":"## ACCEPTANCE run `tool` and $(bad)"}]"###;
        let rows = pick_beads(json, 1);
        assert_eq!(rows[0].description, "## ACCEPTANCE run 'tool' and S(bad)");
        assert_ne!(rows[0].description, ACCEPTANCE_FALLBACK);
        let fallback = pick_beads(r#"[{"id":"cp-2","status":"open"}]"#, 1);
        assert_eq!(fallback[0].description, ACCEPTANCE_FALLBACK);
    }

    #[test]
    fn queue_sort_is_priority_order_and_cursor_advances_per_attempt() {
        let beads: Vec<ReadyBead> = (0..7)
            .map(|index| ReadyBead { id: format!("b{index}"), title: String::new(), description: String::new(), priority: index as i64 })
            .collect();
        let plan = plan_queues(&beads, 3, 0);
        assert_eq!(plan.pane_queues[0][0].id, "b0");
        assert_eq!(plan.pane_queues[1][0].id, "b3");
        assert_eq!(plan.pane_queues[2][0].id, "b6");
        assert_eq!(plan.next_cursor, 9);
    }

    #[test]
    fn packet_is_ship_or_surface_and_contains_close_traps() {
        let bead = ReadyBead { id: "cp-1".into(), title: "Title".into(), description: "ACCEPTANCE run X".into(), priority: 1 };
        let packet = render_packet("2026-08-31T00:00:00Z", 1, &[bead]);
        for phrase in [
            "br update <id> --status=in_progress",
            "ALREADY IMPLEMENTED' IS NOT AN EXIT",
            "REMAINING WORK IS YOUR TARGET",
            "MUTATION-VERIFIED / DONE / APPROVED / WONTFIX",
            "cp-epic-fleet-work-quality-08l6.74",
            "refusal-only",
        ] {
            assert!(packet.contains(phrase), "missing packet phrase: {phrase}");
        }
        assert!(!packet.contains("verify each is genuinely open and unimplemented"));
        assert!(!packet.contains("That is a correct answer, not a failure"));
    }

    #[test]
    fn cooldown_uses_dispatch_marker_not_writer_pid() {
        let ledger = r#"{"ts":"2026-08-31T00:00:00Z","lane":"omp-idle-dispatch","pane":"%1","action":"dispatched","bead":"b1","writer_pid":"old"}
{"ts":"not-a-time","lane":"omp-idle-dispatch","pane":"%1","action":"dispatched","bead":"b2"}
{"ts":"2026-08-31T00:00:00Z","lane":"omp-idle-dispatch","pane":"%2","action":"send_failed","bead":"b3"}"#;
        let now = UNIX_EPOCH + Duration::from_secs(1_788_134_460);
        let marker = parse_dispatch_marker(ledger.lines().next().unwrap_or_default()).unwrap();
        assert_eq!(marker.identity(), "omp-idle-dispatch:%1:b1");
        assert!(recently_dispatched(ledger, "%1", now, Duration::from_secs(180)));
        assert!(!recently_dispatched(ledger, "%2", now, Duration::from_secs(180)));
    }

    #[test]
    fn tick_classes_saturation_and_backstop_as_blocked() {
        assert_eq!(classify_tick(3, 0, 0, false), TickVerdict::BlockedNoIdleCapacity);
        assert_eq!(classify_tick(3, 0, 0, false).as_str(), "BLOCKED");
        assert_eq!(classify_tick(3, 1, 1, false), TickVerdict::BlockedDriverDidNotDispatch);
        assert_eq!(classify_tick(3, 1, 0, true), TickVerdict::RedSendFailed);
        assert_eq!(blocker_fields(TickVerdict::BlockedNoIdleCapacity, 3, 0, 487, 0).unwrap().0, "infrastructure:no-idle-capacity");
        assert!(blocker_fields(TickVerdict::BlockedNoIdleCapacity, 3, 0, 487, 0).unwrap().1.contains("capacity saturated"));
        assert_eq!(blocker_fields(TickVerdict::BlockedDriverDidNotDispatch, 3, 1, 494, 2).unwrap().0, "infrastructure:driver-did-not-dispatch");
    }
    #[test]
    fn malformed_markers_and_other_lanes_never_trigger_cooldown() {
        assert!(parse_dispatch_marker("not-json").is_none());
        assert!(parse_dispatch_marker(r#"{"action":"send_failed","lane":"omp-idle-dispatch","pane":"%1","ts":"2026-08-31T00:00:00Z"}"#).is_none());
        assert!(parse_dispatch_marker(r#"{"action":"dispatched","lane":"omp-idle-dispatch","pane":"%1","ts":"bad"}"#).is_none());
        let other = r#"{"action":"dispatched","lane":"other-lane","pane":"%1","bead":"b","ts":"2026-08-31T00:00:00Z"}"#;
        assert!(!recently_dispatched(other, "%1", UNIX_EPOCH + Duration::from_secs(1_788_134_460), Duration::from_secs(180)));
    }

    #[test]
    fn queue_limit_and_cursor_are_bounded_without_fabricating_work() {
        let bead = ReadyBead { id: "b".into(), title: "t".into(), description: "a".into(), priority: 1 };
        assert!(pick_beads("[]", 12).is_empty());
        assert!(parse_dispatch_marker(r#"{"action":"dispatched","lane":"omp-idle-dispatch","pane":"%1","ts":"2026-02-30T00:00:00Z"}"#).is_none());
        assert!(plan_queues(&[], 3, 0).pane_queues.is_empty());
        let plan = plan_queues(&[bead], 3, usize::MAX - 1);
        assert!(plan.pane_queues.is_empty());
        assert_eq!(plan.next_cursor, usize::MAX);
    }
}
