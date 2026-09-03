#![forbid(unsafe_code)]

//! Acceptance suite for `inbox-monitor`. Every leg is named for the acceptance item it
//! discharges, so a red line names the requirement rather than a symptom.
//!
//! # Anti-vacuity discipline, applied here
//!
//! Every leg that scans, parses, or filters carries a POSITIVE CONTROL proving the reader
//! can see something that IS present. A leg that only asserts a zero is indistinguishable
//! from a leg whose instrument is broken — that is the defect family that produced a
//! confident "zero distinct exit codes" from a pattern that could not match.
//!
//! # Known-good legs, not attack-only
//!
//! Each known-bad arm is paired with the known-good arm that must stay green. An
//! attack-only suite ships an over-strict gate, and an over-strict gate gets routed around.

use inbox_monitor::{
    DaemonArm, UnreachableReason,
    classify, cursor_path_in, iso8601_utc, ledger_path_in, parse_event_page, parse_inbox_rows,
    read_cursor, resolve_pane, state_dir_in, unread, write_cursor_atomic, ConfigError, InboxRow,
    MonitorVerdict, EXIT_CLEAR, EXIT_CURSOR_BELOW_FLOOR, EXIT_CURSOR_REGRESSED, EXIT_MAIL_WAITING,
    EXIT_UNREACHABLE,
};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

// ---------------------------------------------------------------------------------------
// Fixtures — the REAL key sets, taken from the shared contract and confirmed live against
// the installed `am` on 2026-09-02.
// ---------------------------------------------------------------------------------------

/// The exact top-level and event key sets `am inbox-events --json` emits.
const EVENT_PAGE_JSON: &str = r#"{
  "events": [
    {
      "cursor": 2058,
      "message_id": 35789,
      "kind": "to",
      "delivered_ts": "2026-08-31T08:04:58.475827Z",
      "subject": "Contact request from AirTrafficControl",
      "from": "AirTrafficControl",
      "importance": "normal",
      "ack_required": true
    }
  ],
  "has_more": true,
  "next_cursor": 5059,
  "oldest_available_cursor": 2058,
  "tail_cursor": 5059
}"#;

/// The measured `am inbox --json` envelope: the row array is keyed `inbox`, read state is
/// carried by `priority`, and acknowledgement state by `ack_status`. `topic` is absent on
/// some rows, which is why it is not required.
const INBOX_JSON: &str = r#"{
  "_meta": { "command": "robot inbox", "agent": "AmberGate" },
  "count": 2,
  "inbox": [
    {
      "id": 40721,
      "priority": "unread",
      "from": "BlueLantern",
      "subject": "[PRIORITY] ROUNDS.md missing BlueLantern R23 sections",
      "thread": "40721",
      "age": "4h ago",
      "ack_status": "none",
      "importance": "normal"
    },
    {
      "id": 40772,
      "priority": "urgent",
      "from": "BlueLantern",
      "subject": "[identity] BlueLantern - lifecycle and inbox monitor protocol",
      "topic": "identity",
      "thread": "40772",
      "age": "3m ago",
      "ack_status": "required",
      "importance": "high"
    }
  ]
}"#;

/// A realistic multi-session `tmux list-panes -a -F '#{pane_id} #{session_name}:#{window_index}.#{pane_index}'`.
const TMUX_LISTING: &str = "\
%1390 fleet:0.0
%1397 fleet:0.1
%1402 fleet:0.2
%1411 review:0.0
%1412 review:0.1
%88 scratch:3.0
";

fn page() -> inbox_monitor::EventPage {
    parse_event_page(EVENT_PAGE_JSON).expect("the contract fixture must parse")
}

fn row(id: u64, from: &str, subject: &str, read_ts: Option<&str>) -> InboxRow {
    InboxRow {
        id,
        from: from.to_string(),
        subject: subject.to_string(),
        read_ts: read_ts.map(str::to_string),
        importance: "normal".to_string(),
        ack_required: false,
    }
}

/// A throwaway `$HOME` for the state-path legs. Unique per leg so the suite stays parallel.
fn temp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "inbox-monitor-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp home must be creatable");
    dir
}

/// `HOME` is process-global, so the one leg that mutates it serialises against itself and
/// restores the prior value. No other leg reads the ambient `HOME`.
static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

// ---------------------------------------------------------------------------------------
// Acceptance item 2 — cursor durability
// ---------------------------------------------------------------------------------------

#[test]
fn item2_cursor_durability_no_replay_and_no_gap() {
    let home = temp_home("item2");
    let cursor_file = cursor_path_in(&home, "AmberGate").expect("cursor path must resolve");

    // A never-positioned agent is Ok(None), NOT an error and NOT zero. Reading it as zero
    // would replay the entire retained feed on every fresh install.
    assert_eq!(
        read_cursor(&cursor_file).expect("absent cursor is a legitimate first-run state"),
        None
    );

    let page = page();
    let rows: Vec<InboxRow> = vec![row(
        40721,
        "BlueLantern",
        "already answered",
        Some("4h ago"),
    )];

    // PASS 1: process the page, then persist next_cursor exactly as the binary does.
    let first = classify(&page, &rows, None, DaemonArm::NotConsulted);
    assert_eq!(first, MonitorVerdict::Clear);
    write_cursor_atomic(&cursor_file, page.next_cursor).expect("cursor write must succeed");

    // Simulate a restart: forget everything in memory and re-read from disk.
    let after_restart = read_cursor(&cursor_file)
        .expect("cursor must be readable after a restart")
        .expect("cursor must be present after a durable write");
    assert_eq!(
        after_restart, page.next_cursor,
        "the persisted cursor must be exactly next_cursor — a smaller value replays, a \
         larger one skips"
    );

    // PASS 2 over the SAME page: no re-notification.
    let second = classify(&page, &rows, Some(after_restart), DaemonArm::NotConsulted);
    assert_eq!(
        second,
        MonitorVerdict::Clear,
        "re-running over the same page must not re-notify"
    );
    assert_eq!(second.exit_code(), EXIT_CLEAR);

    // NO GAP: the resume cursor is not ahead of the tail, so nothing between the two passes
    // was skipped. `next_cursor == tail_cursor` on a fully drained page is the measured
    // shape (5059 / 5059).
    assert!(
        after_restart <= page.tail_cursor,
        "a resume cursor beyond the tail would silently skip events"
    );
    assert!(
        after_restart >= page.oldest_available_cursor,
        "a resume cursor below the oldest retained cursor means the feed has already \
         discarded events we never saw"
    );

    // POSITIVE CONTROL: the same machinery DOES notify when there is something to notify
    // about, so the two `Clear`s above are not a broken reader.
    let unread_rows = vec![row(40772, "BlueLantern", "[identity] lifecycle", None)];
    assert!(matches!(
        classify(&page, &unread_rows, Some(after_restart), DaemonArm::NotConsulted),
        MonitorVerdict::MailWaiting { .. }
    ));

    // A rewrite must be atomic and total, never appended to.
    write_cursor_atomic(&cursor_file, page.next_cursor + 7).expect("rewrite must succeed");
    assert_eq!(
        read_cursor(&cursor_file).expect("re-read"),
        Some(page.next_cursor + 7)
    );

    let _ = std::fs::remove_dir_all(&home);
}

// ---------------------------------------------------------------------------------------
// Acceptance item 5 — fires on known bad
// ---------------------------------------------------------------------------------------

#[test]
fn item5_fires_on_known_bad_unread_message_is_loud() {
    let page = page();

    // KNOWN BAD: an unread row must be LOUD — nonzero, and naming who and what, because a
    // notification that does not name its sender forces a second lookup before it can be
    // answered.
    let unread_row = row(
        40772,
        "BlueLantern",
        "[identity] BlueLantern - lifecycle and inbox monitor protocol",
        None,
    );
    let loud = classify(&page, std::slice::from_ref(&unread_row), Some(5059), DaemonArm::NotConsulted);
    match &loud {
        MonitorVerdict::MailWaiting {
            unread,
            oldest_from,
            oldest_subject,
        } => {
            assert_eq!(*unread, 1);
            assert_eq!(oldest_from, "BlueLantern");
            assert_eq!(
                oldest_subject,
                "[identity] BlueLantern - lifecycle and inbox monitor protocol"
            );
        }
        other => panic!("an unread row must classify as MailWaiting, got {other:?}"),
    }
    assert_ne!(loud.exit_code(), 0, "a waiting message must exit nonzero");
    assert_eq!(loud.exit_code(), EXIT_MAIL_WAITING);
    // The human line must carry the sender and subject, not just a count.
    let line = loud.human_line();
    assert!(
        line.contains("BlueLantern"),
        "human line must name the sender: {line}"
    );
    assert!(
        line.contains("lifecycle"),
        "human line must name the subject: {line}"
    );

    // KNOWN GOOD: the SAME row, read, must be silent. Both arms asserted, so this is not an
    // attack-only leg that would pass with a gate wired permanently open.
    let read_row = InboxRow {
        read_ts: Some("2026-09-02T05:03:50Z".to_string()),
        ..unread_row
    };
    let quiet = classify(&page, std::slice::from_ref(&read_row), Some(5059), DaemonArm::NotConsulted);
    assert_eq!(quiet, MonitorVerdict::Clear);
    assert_eq!(quiet.exit_code(), EXIT_CLEAR);

    // The oldest unread is the LOWEST id, not the first in the array — the measured surface
    // reports `age` only as a relative string, so id is the sortable field.
    let mixed = vec![
        row(40772, "BlueLantern", "newer", None),
        row(40001, "AirTrafficControl", "older", None),
        row(40500, "AmberGate", "middle", Some("1h ago")),
    ];
    match classify(&page, &mixed, None, DaemonArm::NotConsulted) {
        MonitorVerdict::MailWaiting {
            unread,
            oldest_from,
            oldest_subject,
        } => {
            assert_eq!(unread, 2, "the read row must not be counted");
            assert_eq!(oldest_from, "AirTrafficControl");
            assert_eq!(oldest_subject, "older");
        }
        other => panic!("expected MailWaiting, got {other:?}"),
    }

    // A caller handing over the FULL mailbox must not be able to manufacture a
    // notification out of read rows — classify re-applies the filter.
    let all_read = vec![
        row(1, "a", "x", Some("1h ago")),
        row(2, "b", "y", Some("2h ago")),
    ];
    assert_eq!(unread(&all_read).len(), 0);
    assert_eq!(classify(&page, &all_read, None, DaemonArm::NotConsulted), MonitorVerdict::Clear);
}

// ---------------------------------------------------------------------------------------
// Acceptance item 6 — anti-vacuity: cannot-observe is not no-mail
// ---------------------------------------------------------------------------------------

#[test]
fn item6_anti_vacuity_unreachable_is_an_error_not_no_mail() {
    // MEASURED FALSE NEGATIVE this leg defends against, 2026-09-02: `am agent start`
    // reported "no listener on 127.0.0.1:8765" while `curl /health` returned
    // {"status":"ready"} and two robot calls returned live data. A monitor that folds
    // "cannot observe" into "nothing to report" is quietest exactly when it is blindest —
    // and `--direct` produces byte-identical output without announcing itself, so the
    // provenance of a quiet run is unrecoverable unless the verdict carries it.
    let unreachable = MonitorVerdict::Unreachable {
        detail: "am inbox-events exceeded its 20s deadline".to_string(),
        reason: UnreachableReason::Indeterminate,
    };

    assert_ne!(
        unreachable.exit_code(),
        0,
        "an unobservable mailbox must never exit 0"
    );
    assert_ne!(
        unreachable,
        MonitorVerdict::Clear,
        "Unreachable and Clear are different facts and must not compare equal"
    );
    assert_ne!(
        unreachable.exit_code(),
        MonitorVerdict::Clear.exit_code(),
        "Unreachable must not share Clear's code"
    );

    let waiting = MonitorVerdict::MailWaiting {
        unread: 1,
        oldest_from: "BlueLantern".to_string(),
        oldest_subject: "s".to_string(),
    };
    assert_ne!(
        unreachable.exit_code(),
        waiting.exit_code(),
        "'I could not look' and 'you have mail' are different facts and must not share a code"
    );
    assert_eq!(unreachable.exit_code(), EXIT_UNREACHABLE);
    assert_eq!(waiting.exit_code(), EXIT_MAIL_WAITING);

    // The nonzero codes must all live in the unallocated 5..=63 band the exit-code registry
    // reserves for new distinct meanings — never in 1 (three meanings already), never in
    // 101 (rustc panic), never in 102..=125 (toolchain and timeout(1)).
    for code in [EXIT_MAIL_WAITING, EXIT_UNREACHABLE, EXIT_CURSOR_REGRESSED] {
        assert!(
            (5..=63).contains(&code),
            "{code} is outside the unallocated band"
        );
    }

    // The operator-facing line must say the verdict is ABSENT, not negative.
    let line = unreachable.human_line();
    assert!(line.contains("UNREACHABLE"), "line: {line}");
    assert!(
        line.contains("NOT 'no mail'"),
        "the line must refuse the false-negative reading: {line}"
    );

    // POSITIVE CONTROL: the same comparison machinery DOES report equality when two
    // verdicts really are the same, so the assert_ne! wall above is not vacuous.
    assert_eq!(MonitorVerdict::Clear, MonitorVerdict::Clear);
    assert_eq!(
        MonitorVerdict::Unreachable {
            detail: "x".to_string(),
            reason: UnreachableReason::Indeterminate
        },
        MonitorVerdict::Unreachable {
            detail: "x".to_string(),
            reason: UnreachableReason::Indeterminate
        }
    );
    // And the REASON participates in equality, which is the whole point of typing it: two
    // Unreachable verdicts with the same detail and opposite remedies must not be equal.
    assert_ne!(
        MonitorVerdict::Unreachable {
            detail: "x".to_string(),
            reason: UnreachableReason::Absent
        },
        MonitorVerdict::Unreachable {
            detail: "x".to_string(),
            reason: UnreachableReason::Unauthorized { status: 401 }
        }
    );
}

// ---------------------------------------------------------------------------------------
// Cursor regression
// ---------------------------------------------------------------------------------------

#[test]
fn a_cursor_ahead_of_the_tail_is_a_regression_not_a_clear_run() {
    let page = page();
    assert_eq!(page.tail_cursor, 5059);

    // KNOWN BAD: our state claims to have consumed past the durable tail. Every subsequent
    // `--after` would ask for events beyond the end and receive nothing — a silent skip.
    let regressed = classify(&page, &[], Some(page.tail_cursor + 1), DaemonArm::NotConsulted);
    assert_eq!(
        regressed,
        MonitorVerdict::CursorRegressed {
            persisted: 5060,
            tail: 5059
        }
    );
    assert_eq!(regressed.exit_code(), EXIT_CURSOR_REGRESSED);
    assert_ne!(regressed.exit_code(), EXIT_CLEAR);
    assert_ne!(regressed.exit_code(), EXIT_MAIL_WAITING);
    assert_ne!(regressed.exit_code(), EXIT_UNREACHABLE);

    // KNOWN GOOD: exactly at the tail is the normal fully-drained state, not a regression.
    assert_eq!(
        classify(&page, &[], Some(page.tail_cursor), DaemonArm::NotConsulted),
        MonitorVerdict::Clear
    );
    // KNOWN GOOD: behind the tail is an ordinary resume.
    assert_eq!(
        classify(&page, &[], Some(page.oldest_available_cursor), DaemonArm::NotConsulted),
        MonitorVerdict::Clear
    );
    // KNOWN GOOD: never positioned is not a regression.
    assert_eq!(classify(&page, &[], None, DaemonArm::NotConsulted), MonitorVerdict::Clear);

    // PRECEDENCE INVERTED, 2026-09-02, deliberately and with the reason recorded.
    //
    // THIS ASSERTION PREVIOUSLY READ: "Mail waiting outranks a regression: the human can act
    // on mail now, and a state repair does not expire", asserting
    // `classify(&page, &rows, Some(page.tail_cursor + 1), DaemonArm::NotConsulted)` matched `MailWaiting { .. }`.
    // That intent is recorded here rather than deleted, because an inverted assertion with no
    // explanation is indistinguishable from an assertion someone flipped to get green.
    //
    // Why the old reasoning was WEAKER: it compared how ACTIONABLE two outcomes are, and
    // ignored what each one does to the state file. A cursor fault does not merely rank below
    // mail — it invalidates the page the mail claim is DERIVED FROM. A verdict about contents
    // is worthless when the position those contents came from is unproven. And measured: the
    // daemon serves a below-floor read from the floor with rc=0 and no error key anywhere in
    // the payload, so a mail-first order reports MailWaiting from a page the SERVER chose,
    // then (pre-fix) persisted that position as if everything before it had been consumed.
    let rows = vec![row(1, "BlueLantern", "s", None)];
    assert!(matches!(
        classify(&page, &rows, Some(page.tail_cursor + 1), DaemonArm::NotConsulted),
        MonitorVerdict::CursorRegressed { .. }
    ));
    // And the mail claim is still reachable the moment the position is provable — this is the
    // known-good arm that keeps the reordering from being an over-strict gate.
    assert!(matches!(
        classify(&page, &rows, Some(page.tail_cursor), DaemonArm::NotConsulted),
        MonitorVerdict::MailWaiting { .. }
    ));
}

// ---------------------------------------------------------------------------------------
// Cursor below the per-recipient floor
//
// The measured page these legs are built from, `am inbox-events --position-now --json`,
// 2026-09-02T05:1xZ, in this workspace's own Agent Mail project:
//
//   AmberGate     tail=5162  oldest=2058  span 3104
//   SnowyCanyon   tail=5165  oldest=5147  span   18
//   GreenFrog     tail=5161  oldest=2108  span 3053
//   BrightGorge                           span    0
//
// NOT an eviction window: a "~17 event retention" claim cannot coexist with a recipient
// retaining 3053 positions. `oldest_available_cursor` is the oldest event still retained
// FOR THAT RECIPIENT, and for a mailbox that started receiving recently it equals its FIRST
// EVENT. The reason the guard is unconditional is structural instead: a recipient's events
// are sparse and non-contiguous inside one GLOBAL monotonic sequence (GreenFrog holds 2108,
// 2109, 2126), so a stored cursor below the floor cannot be distinguished from a recipient
// that simply started receiving later. Continuity is unprovable either way.
// ---------------------------------------------------------------------------------------

/// The SnowyCanyon page, verbatim shape, with the measured floor of 5147 and tail of 5165.
/// `events` is empty on purpose: that is what a resume from an unresumable position looks
/// like from the client, and it is the shape a `Clear` would be manufactured out of.
const SNOWY_CANYON_PAGE_JSON: &str = r#"{
  "events": [],
  "has_more": false,
  "next_cursor": 5165,
  "oldest_available_cursor": 5147,
  "tail_cursor": 5165
}"#;

fn snowy_canyon_page() -> inbox_monitor::EventPage {
    parse_event_page(SNOWY_CANYON_PAGE_JSON).expect("the measured SnowyCanyon page must parse")
}

#[test]
fn a_cursor_below_the_recipient_floor_is_blindness_not_a_clear_run() {
    let page = snowy_canyon_page();
    // Instrument check before the claim: the fixture really does carry the measured floor.
    assert_eq!(page.oldest_available_cursor, 5147);
    assert_eq!(page.tail_cursor, 5165);
    assert!(page.events.is_empty());

    // KNOWN BAD: 5105 is the cursor the orchestrator circulated FLEET-WIDE. The sequence is
    // global, so 5105 is a perfectly resumable position for GreenFrog (floor 2108) and an
    // unresumable one for SnowyCanyon (floor 5147). Cursors are not portable across
    // recipients, and an empty `events` list from such a read is not evidence of no mail.
    let blind = classify(&page, &[], Some(5105), DaemonArm::NotConsulted);
    assert_eq!(
        blind,
        MonitorVerdict::CursorBelowFloor {
            persisted: 5105,
            oldest_available: 5147,
            tail: 5165,
        },
        "an unresumable position must not be reported as a drained mailbox"
    );

    // FOUR DISTINCT FACTS, FOUR DISTINCT CODES. The whole point of a fourth number is that
    // an operator holding only the integer can tell these apart.
    assert_eq!(blind.exit_code(), EXIT_CURSOR_BELOW_FLOOR);
    assert_eq!(blind.exit_code(), 15);
    assert_ne!(blind.exit_code(), EXIT_CLEAR);
    assert_ne!(blind.exit_code(), EXIT_MAIL_WAITING);
    assert_ne!(blind.exit_code(), EXIT_UNREACHABLE);
    assert_ne!(blind.exit_code(), EXIT_CURSOR_REGRESSED);
    assert_eq!(blind.label(), "cursor_below_floor");

    // The two numbers must be recoverable from the artifact alone. A row that says "below
    // floor" without the floor and the position it fell under is undiagnosable.
    let human = blind.human_line();
    assert!(human.contains("5105"), "human line must name the position");
    assert!(human.contains("5147"), "human line must name the floor");
    assert!(human.contains("5165"), "human line must name the tail");

    // AND THE CURSOR MUST NOT MOVE. A run that detects an unprovable position and then
    // persists a new one has repaired the SYMPTOM and kept the blindness: one loud exit,
    // then a silent re-base to whatever the server clamped to, then Clear forever.
    assert!(
        !blind.advances_cursor(),
        "a below-floor run must abstain from advancing the cursor"
    );
    assert!(
        !MonitorVerdict::CursorRegressed {
            persisted: 5166,
            tail: 5165
        }
        .advances_cursor(),
        "a regressed run must abstain too — the position is equally unproven"
    );
    // KNOWN GOOD for the same predicate, so it is not vacuously false everywhere.
    assert!(MonitorVerdict::Clear.advances_cursor());
    assert!(MonitorVerdict::MailWaiting {
        unread: 1,
        oldest_from: "SnowyCanyon".into(),
        oldest_subject: "s".into(),
    }
    .advances_cursor());

    // File-level proof of the abstention, through the same gate the binary uses: the cursor
    // file is BYTE-UNCHANGED across a below-floor observation.
    let home = temp_home("below-floor-abstains");
    let cursor_file = cursor_path_in(&home, "SnowyCanyon").expect("path must resolve");
    write_cursor_atomic(&cursor_file, 5105).expect("seed cursor must write");
    let before = std::fs::read(&cursor_file).expect("seeded cursor must be readable");
    if blind.advances_cursor() {
        write_cursor_atomic(&cursor_file, page.next_cursor).expect("write must succeed");
    }
    let after = std::fs::read(&cursor_file).expect("cursor must still be readable");
    assert_eq!(
        before, after,
        "the cursor file must be byte-unchanged after a below-floor run"
    );
    assert_eq!(
        read_cursor(&cursor_file).expect("cursor must parse"),
        Some(5105),
        "the unresumable position must survive as the operator's repair target"
    );
}

#[test]
fn a_first_run_with_no_persisted_cursor_is_a_baseline_not_below_floor() {
    let page = snowy_canyon_page();

    // KNOWN GOOD, and this leg is why the guard survives contact with the fleet. `None` is a
    // BASELINE: a first run has never been positioned, so it is neither ahead of the tail nor
    // below the floor. Without this arm the guard fires on every fresh install — and a guard
    // that fires on the known-good path is a guard that gets routed around.
    let baseline = classify(&page, &[], None, DaemonArm::NotConsulted);
    assert_eq!(baseline, MonitorVerdict::Clear);
    assert_eq!(baseline.exit_code(), EXIT_CLEAR);
    assert_ne!(baseline.exit_code(), EXIT_CURSOR_BELOW_FLOOR);
    assert!(!matches!(baseline, MonitorVerdict::CursorBelowFloor { .. }));

    // A first run with mail is still MailWaiting, not a cursor fault: absence of a stored
    // position is not a fault about a position.
    let rows = vec![row(1, "BrightGorge", "first contact", None)];
    assert!(matches!(
        classify(&page, &rows, None, DaemonArm::NotConsulted),
        MonitorVerdict::MailWaiting { .. }
    ));

    // FIRES-ON-KNOWN-BAD control, so the leg above is not passing because the comparison is
    // dead: the SAME page with a stored position one below the floor does fault.
    assert!(matches!(
        classify(&page, &[], Some(page.oldest_available_cursor - 1), DaemonArm::NotConsulted),
        MonitorVerdict::CursorBelowFloor { .. }
    ));
}

#[test]
fn a_cursor_inside_the_window_still_reports_mail_or_clear_normally() {
    let page = snowy_canyon_page();

    // KNOWN GOOD: 5150 sits inside [5147, 5165]. The new check must not swallow the normal
    // path — an over-strict guard is worse than none, because it gets disabled.
    assert_eq!(classify(&page, &[], Some(5150), DaemonArm::NotConsulted), MonitorVerdict::Clear);

    let rows = vec![row(7, "GreenFrog", "still routes", None)];
    match classify(&page, &rows, Some(5150), DaemonArm::NotConsulted) {
        MonitorVerdict::MailWaiting {
            unread,
            oldest_from,
            ..
        } => {
            assert_eq!(unread, 1);
            assert_eq!(oldest_from, "GreenFrog");
        }
        other => panic!("a cursor inside the window must still report mail, got {other:?}"),
    }

    // BOUNDARY, and it is the one that decides `<` versus `<=`: exactly AT the floor is a
    // legitimate full replay of everything retained, not a fault.
    assert_eq!(
        classify(&page, &[], Some(page.oldest_available_cursor), DaemonArm::NotConsulted),
        MonitorVerdict::Clear,
        "equal to the floor is a full replay, so the comparison must be strict"
    );
    // BOUNDARY: exactly AT the tail is the drained state.
    assert_eq!(
        classify(&page, &[], Some(page.tail_cursor), DaemonArm::NotConsulted),
        MonitorVerdict::Clear
    );
    // And the fault is still one step away in each direction, so neither boundary is passing
    // because the checks are dead.
    assert!(matches!(
        classify(&page, &[], Some(page.oldest_available_cursor - 1), DaemonArm::NotConsulted),
        MonitorVerdict::CursorBelowFloor { .. }
    ));
    assert!(matches!(
        classify(&page, &[], Some(page.tail_cursor + 1), DaemonArm::NotConsulted),
        MonitorVerdict::CursorRegressed { .. }
    ));
}

// ---------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------

#[test]
fn a_missing_json_key_is_a_parse_error_not_a_default() {
    // POSITIVE CONTROL FIRST: the complete payload, with the real key set, parses — so a
    // later Err is evidence about the missing key and not about a broken parser.
    let good = parse_event_page(EVENT_PAGE_JSON).expect("the complete contract payload must parse");
    assert_eq!(good.next_cursor, 5059);
    assert_eq!(good.tail_cursor, 5059);
    assert_eq!(good.oldest_available_cursor, 2058);
    assert!(good.has_more);
    assert_eq!(good.events.len(), 1);
    let event = &good.events[0];
    assert_eq!(event.cursor, 2058);
    assert_eq!(event.message_id, 35789);
    assert_eq!(event.kind, "to");
    assert_eq!(event.delivered_ts, "2026-08-31T08:04:58.475827Z");
    assert_eq!(event.from, "AirTrafficControl");
    assert_eq!(event.subject, "Contact request from AirTrafficControl");
    assert_eq!(event.importance, "normal");
    assert!(event.ack_required);

    // KNOWN BAD: strip `next_cursor`. A default of 0 here would rewind the cursor to the
    // start of the retained feed and replay ~3000 events, or - worse in the other direction
    // - be read as a legitimate position.
    let without_next = EVENT_PAGE_JSON.replace("\"next_cursor\": 5059,", "");
    assert!(
        !without_next.contains("next_cursor"),
        "the fixture mutation must actually remove the key"
    );
    let error = parse_event_page(&without_next)
        .expect_err("a missing next_cursor must be a hard parse error, never a default");
    assert!(
        error.detail.contains("next_cursor"),
        "the error must name the missing key: {error}"
    );

    // Every other top-level key is equally required.
    for key in [
        "events",
        "has_more",
        "oldest_available_cursor",
        "tail_cursor",
    ] {
        let mutated = EVENT_PAGE_JSON
            .lines()
            .filter(|line| !line.trim_start().starts_with(&format!("\"{key}\"")))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            parse_event_page(&mutated).is_err(),
            "a payload missing `{key}` must not parse"
        );
    }

    // Event-level keys too: a defaulted empty `from` is a notification that names nobody.
    let without_from = EVENT_PAGE_JSON.replace("\"from\": \"AirTrafficControl\",", "");
    assert!(
        parse_event_page(&without_from).is_err(),
        "an event missing `from` must not parse"
    );

    // A future `am` adding a key must NOT blind the monitor: unknown keys are tolerated.
    let with_extra = EVENT_PAGE_JSON.replace(
        "\"has_more\": true,",
        "\"has_more\": true,\n  \"a_field_from_a_newer_am\": 1,",
    );
    assert!(
        parse_event_page(&with_extra).is_ok(),
        "an ADDED key must not be a hard error; only a MISSING key manufactures a false zero"
    );

    // The mailbox surface, measured envelope. POSITIVE CONTROL: it parses and both rows are
    // unread, because `--unread` returns everything whose priority is not `read`.
    let rows = parse_inbox_rows(INBOX_JSON).expect("the measured inbox envelope must parse");
    assert_eq!(rows.len(), 2);
    assert_eq!(unread(&rows).len(), 2);
    assert_eq!(rows[0].id, 40721);
    assert_eq!(rows[0].from, "BlueLantern");
    assert!(
        !rows[0].ack_required,
        "ack_status `none` is not ack_required"
    );
    assert!(
        rows[1].ack_required,
        "ack_status `required` is ack_required"
    );
    assert_eq!(rows[1].importance, "high");

    // KNOWN BAD: a row with no `from` must not become a row with an empty sender.
    let anonymous = INBOX_JSON.replace("\"from\": \"BlueLantern\",", "");
    assert!(
        parse_inbox_rows(&anonymous).is_err(),
        "a row missing `from` must be a parse error, not a nameless notification"
    );

    // KNOWN BAD: a row carrying neither `read_ts` nor `priority` has unknowable read state
    // and must not be guessed either way.
    let no_read_state = r#"{"inbox":[{"id":1,"from":"a","subject":"s","importance":"normal"}]}"#;
    assert!(parse_inbox_rows(no_read_state).is_err());

    // KNOWN GOOD: a future `am` that DOES ship `read_ts` is honoured literally.
    let literal = r#"{"inbox":[{"id":1,"from":"a","subject":"s","importance":"normal",
        "read_ts":"2026-09-02T05:00:00Z","ack_required":false}]}"#;
    let literal_rows = parse_inbox_rows(literal).expect("a literal read_ts must parse");
    assert_eq!(
        literal_rows[0].read_ts.as_deref(),
        Some("2026-09-02T05:00:00Z")
    );
    assert_eq!(unread(&literal_rows).len(), 0);

    // An envelope with no recognisable row array is an error, never an empty mailbox — an
    // empty scan set is the false zero this whole suite exists to refuse.
    assert!(parse_inbox_rows(r#"{"_meta":{},"count":0}"#).is_err());
    // POSITIVE CONTROL for that leg: a bare array is accepted, so the error above is about
    // the missing array and not about the shape check being permanently closed.
    assert_eq!(
        parse_inbox_rows("[]").expect("a bare array parses").len(),
        0
    );
}

// ---------------------------------------------------------------------------------------
// Pane attribution
// ---------------------------------------------------------------------------------------

#[test]
fn pane_resolution_returns_the_pane_id_and_records_the_index_it_came_from() {
    // The row records `resolved_pane` AND the `session:index` it was resolved FROM, because
    // "notified pane 1" is undiagnosable while "notified %1397, resolved from fleet:1" is
    // evidence. Pane indices shifted twice in one evening; the pane_id is the handle.
    assert_eq!(
        resolve_pane(TMUX_LISTING, "fleet", 1).as_deref(),
        Some("%1397")
    );
    assert_eq!(
        resolve_pane(TMUX_LISTING, "fleet", 0).as_deref(),
        Some("%1390")
    );
    assert_eq!(
        resolve_pane(TMUX_LISTING, "fleet", 2).as_deref(),
        Some("%1402")
    );
    // Same index, different session: the session must discriminate, or every notification
    // would be attributed to whichever session listed first.
    assert_eq!(
        resolve_pane(TMUX_LISTING, "review", 1).as_deref(),
        Some("%1412")
    );
    // A non-zero window index still resolves.
    assert_eq!(
        resolve_pane(TMUX_LISTING, "scratch", 0).as_deref(),
        Some("%88")
    );

    // NEGATIVE CONTROL: an index that is not present returns None rather than guessing.
    assert_eq!(resolve_pane(TMUX_LISTING, "fleet", 9), None);
    assert_eq!(resolve_pane(TMUX_LISTING, "nosuchsession", 0), None);
    // A prefix that is not a full session name must not match `fleet`.
    assert_eq!(resolve_pane(TMUX_LISTING, "fle", 1), None);
    // An empty listing is not "pane 1 is gone"; it is no answer at all, and the caller
    // records the attempted index either way.
    assert_eq!(resolve_pane("", "fleet", 1), None);

    // AMBIGUITY: the same session:pane_index in two different WINDOWS has no correct
    // answer, so there is no answer. Guessing here would attribute a notification to a pane
    // that never ran the monitor.
    let ambiguous = "%10 fleet:0.1\n%20 fleet:1.1\n";
    assert_eq!(resolve_pane(ambiguous, "fleet", 1), None);
    // POSITIVE CONTROL for the ambiguity leg: with one of the two removed it resolves, so
    // the None above is ambiguity and not a broken matcher.
    assert_eq!(
        resolve_pane("%20 fleet:1.1\n", "fleet", 1).as_deref(),
        Some("%20")
    );

    // Garbage lines are skipped, not parsed into a pane id.
    let noisy = format!("no-such-shape\n\n{TMUX_LISTING}trailing garbage here too\n");
    assert_eq!(resolve_pane(&noisy, "fleet", 1).as_deref(), Some("%1397"));
    // A pane index of 10 must not be matched by the `.1` suffix of pane index 1.
    assert_eq!(resolve_pane("%5 fleet:0.10\n", "fleet", 1), None);
    assert_eq!(
        resolve_pane("%5 fleet:0.10\n", "fleet", 10).as_deref(),
        Some("%5")
    );
}

// ---------------------------------------------------------------------------------------
// State paths
// ---------------------------------------------------------------------------------------

#[test]
fn home_unset_is_a_typed_error_not_a_literal_path() {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = std::env::var_os("HOME");

    std::env::remove_var("HOME");
    let error = inbox_monitor::home_dir()
        .expect_err("HOME unset must be a typed error, never a hardcoded checkout");
    assert_eq!(error, ConfigError::HomeUnset);
    assert!(inbox_monitor::cursor_path("AmberGate").is_err());
    assert!(inbox_monitor::state_dir().is_err());
    assert!(inbox_monitor::ledger_path().is_err());

    // An EMPTY HOME is the same defect wearing a different mask: joining onto "" yields a
    // relative path that reads whatever directory the process happens to be in.
    std::env::set_var("HOME", "");
    assert_eq!(
        inbox_monitor::home_dir().expect_err("empty HOME must be rejected too"),
        ConfigError::HomeUnset
    );

    // POSITIVE CONTROL: with HOME set, the paths resolve UNDER it and nowhere else.
    let home = temp_home("home-set");
    std::env::set_var("HOME", &home);
    let resolved = inbox_monitor::cursor_path("AmberGate").expect("a set HOME must resolve");
    assert!(
        resolved.starts_with(&home),
        "{} must live under the resolved home",
        resolved.display()
    );
    assert_eq!(
        resolved,
        cursor_path_in(&home, "AmberGate").expect("pure form")
    );
    assert_eq!(
        inbox_monitor::ledger_path().expect("ledger path"),
        ledger_path_in(&home)
    );
    assert_eq!(
        state_dir_in(&home),
        home.join(".local")
            .join("state")
            .join("flywheel")
            .join("inbox-monitor")
    );

    match previous {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_corrupt_cursor_is_an_error_not_a_zero() {
    let home = temp_home("corrupt");
    let cursor_file = cursor_path_in(&home, "AmberGate").expect("path");
    std::fs::create_dir_all(cursor_file.parent().expect("parent")).expect("mkdir");

    // KNOWN BAD: garbage. Reading it as 0 would replay the whole retained feed.
    std::fs::write(&cursor_file, "not-a-cursor").expect("write");
    assert!(read_cursor(&cursor_file).is_err());
    // KNOWN BAD: an empty file, which is exactly what a crashed truncating write leaves.
    std::fs::write(&cursor_file, "").expect("write");
    assert!(read_cursor(&cursor_file).is_err());

    // KNOWN GOOD: a real cursor, with and without a trailing newline.
    std::fs::write(&cursor_file, "5059").expect("write");
    assert_eq!(read_cursor(&cursor_file).expect("read"), Some(5059));
    std::fs::write(&cursor_file, "5059\n").expect("write");
    assert_eq!(read_cursor(&cursor_file).expect("read"), Some(5059));

    // The atomic write must leave no temp file behind for the next run to trip over.
    write_cursor_atomic(&cursor_file, 6000).expect("atomic write");
    let leftovers: Vec<_> = std::fs::read_dir(cursor_file.parent().expect("parent"))
        .expect("readdir")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files left behind: {leftovers:?}"
    );
    assert_eq!(read_cursor(&cursor_file).expect("read"), Some(6000));

    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_ledger_row_is_durable_before_the_cursor_moves() {
    let home = temp_home("ledger");
    let ledger = ledger_path_in(&home);
    let cursor_file = cursor_path_in(&home, "AmberGate").expect("path");

    // The binary's ordering: append, fsync, THEN advance. This leg proves the append is
    // self-contained (it creates its own directory) so the cursor never advances past a row
    // that was never written.
    inbox_monitor::append_ledger(&ledger, r#"{"verdict":"mail_waiting"}"#).expect("append");
    write_cursor_atomic(&cursor_file, 5059).expect("cursor");

    inbox_monitor::append_ledger(&ledger, r#"{"verdict":"clear"}"#).expect("append");
    let body = std::fs::read_to_string(&ledger).expect("ledger readable");
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines.len(), 2, "the ledger is append-only: {body}");
    assert!(lines[0].contains("mail_waiting"));
    assert!(lines[1].contains("clear"));
    // Every line must be independently parseable JSON — a JSONL ledger whose lines do not
    // parse is a log file wearing a schema.
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each ledger line is JSON");
    }

    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_emitted_timestamp_is_rfc3339_utc() {
    // The ledger must be legible without a converter, and the conversion must be checkable.
    assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(iso8601_utc(1_000_000_000), "2001-09-09T01:46:40Z");
    assert_eq!(iso8601_utc(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(iso8601_utc(1_756_800_000), "2025-09-02T08:00:00Z");
}

// ---------------------------------------------------------------------------------------
// y256 — the primary is the daemon, the CLI is the oracle, and a disagreement is LOUD
// ---------------------------------------------------------------------------------------

/// The two numbers MEASURED on AmberGate's own mailbox, 2026-09-02, with commands that were
/// re-run rather than remembered:
///
/// * daemon — `fetch_inbox` over authenticated MCP with `unread_only:true, mark_read:false,
///   limit:500` → **96** unread of **128** total rows, of which **32** carry a non-null
///   `read_ts`.
/// * CLI — `am inbox --project … --agent AmberGate --unread --json` → **20**, matching the
///   envelope's own `count` field, split `{urgent: 10, ack-overdue: 4, unread: 6}`.
const MEASURED_DAEMON_UNREAD: usize = 96;
const MEASURED_CLI_UNREAD: usize = 20;

#[test]
fn y256_the_live_disagreement_is_reported_and_neither_side_is_picked() {
    // ACCEPTANCE 2. The bead asked for the disagreement leg to fire on today's real data.
    // It does — with CORRECTED numbers: the bead recorded the daemon at 0 unread, and a
    // non-mutating read says 96. See `y256_the_beads_own_premise_was_a_consuming_read`.
    let cli: Vec<InboxRow> = (0..MEASURED_CLI_UNREAD)
        .map(|i| row(40000 + i as u64, "GreenFrog", "ACK needed", None))
        .collect();
    let verdict = classify(
        &page(),
        &cli,
        None,
        DaemonArm::Unread(MEASURED_DAEMON_UNREAD),
    );
    assert_eq!(
        verdict,
        MonitorVerdict::AuthoritiesDisagree {
            daemon_unread: MEASURED_DAEMON_UNREAD,
            cli_unread: MEASURED_CLI_UNREAD,
        },
        "the live divergence must not resolve to either arm"
    );
    assert_eq!(verdict.exit_code(), 16, "a distinct code, not folded into 12");

    // BOTH numbers and BOTH sources in the line a human reads. A disagreement that names
    // one side is the defect this verdict replaces.
    let line = verdict.human_line();
    for needle in ["96", "20", "daemon", "CLI", "storage.sqlite3", "fetch_inbox"] {
        assert!(line.contains(needle), "the line must carry {needle:?}: {line}");
    }
    assert!(
        !line.contains("unread; oldest from"),
        "it must NOT wear the MailWaiting shape: {line}"
    );
}

#[test]
fn y256_agreement_reports_the_shared_answer_and_is_not_a_disagreement() {
    // KNOWN-GOOD ARM, and the one that matters: a check that fires on every run is a check
    // that gets routed around. Two arms agreeing must produce the ordinary verdicts.
    let unread_rows = vec![row(40772, "BlueLantern", "[identity] lifecycle", None)];
    assert!(matches!(
        classify(&page(), &unread_rows, None, DaemonArm::Unread(1)),
        MonitorVerdict::MailWaiting { unread: 1, .. }
    ));

    let read_rows = vec![row(
        40772,
        "BlueLantern",
        "[identity] lifecycle",
        Some("2026-09-02T00:00:00Z"),
    )];
    assert_eq!(
        classify(&page(), &read_rows, None, DaemonArm::Unread(0)),
        MonitorVerdict::Clear,
        "agreement on zero is Clear, not a disagreement about zero"
    );
}

#[test]
fn y256_a_cursor_fault_still_outranks_a_disagreement() {
    // ORDER. A disagreement about READ state says nothing about the delivery POSITION, and
    // the cursor faults are the ones that destroy their own evidence by advancing. So they
    // must still win, even while the arms are in conflict.
    let page = page();
    let cli = vec![row(1, "GreenFrog", "x", None)];

    let regressed = classify(
        &page,
        &cli,
        Some(page.tail_cursor + 1),
        DaemonArm::Unread(999),
    );
    assert!(
        matches!(regressed, MonitorVerdict::CursorRegressed { .. }),
        "a regressed cursor must outrank a disagreement, got {regressed:?}"
    );

    let below = classify(
        &page,
        &cli,
        Some(page.oldest_available_cursor - 1),
        DaemonArm::Unread(999),
    );
    assert!(
        matches!(below, MonitorVerdict::CursorBelowFloor { .. }),
        "a below-floor cursor must outrank a disagreement, got {below:?}"
    );

    // POSITIVE CONTROL: with a SANE cursor the same inputs do reach the disagreement, so the
    // two assertions above are about ORDER and not about the check being unreachable.
    let sane = classify(&page, &cli, Some(page.tail_cursor), DaemonArm::Unread(999));
    assert!(
        matches!(sane, MonitorVerdict::AuthoritiesDisagree { .. }),
        "the disagreement must be reachable at all, got {sane:?}"
    );
}

#[test]
fn y256_unauthorized_and_absent_are_not_the_same_verdict_or_the_same_line() {
    // ACCEPTANCE 3. `am agent start` once reported "no listener on 127.0.0.1:8765" about a
    // daemon that was running and answering /health — an AUTH FAILURE REPORTED AS ABSENCE.
    // The two must differ as VALUES, not merely in prose.
    let unauthorized = MonitorVerdict::Unreachable {
        detail: "the daemon ANSWERED and refused our credential (HTTP 401)".to_string(),
        reason: UnreachableReason::Unauthorized { status: 401 },
    };
    let absent = MonitorVerdict::Unreachable {
        detail: "nothing answered on the MCP endpoint".to_string(),
        reason: UnreachableReason::Absent,
    };
    assert_ne!(unauthorized, absent);
    assert_ne!(
        unauthorized.human_line(),
        absent.human_line(),
        "the two must not produce the same detail string"
    );
    assert_ne!(
        UnreachableReason::Unauthorized { status: 401 }.label(),
        UnreachableReason::Absent.label()
    );

    // The REMEDIES are opposite, and each line must carry its own.
    assert!(
        unauthorized.human_line().contains("do NOT restart"),
        "an auth failure must not read as 'start the daemon': {}",
        unauthorized.human_line()
    );
    assert!(
        absent.human_line().contains("START the daemon"),
        "an absent listener must say to start one: {}",
        absent.human_line()
    );

    // Both remain exit 13 — the CODE is the same fact ("no verdict"), the REASON is not.
    assert_eq!(unauthorized.exit_code(), 13);
    assert_eq!(absent.exit_code(), 13);
    // And neither advances the cursor.
    assert!(!unauthorized.advances_cursor() && !absent.advances_cursor());
}

#[test]
fn y256_the_disagreement_advances_the_cursor_and_says_why() {
    // The POSITION was proven — both cursor guards passed — so withholding the advance would
    // imply the position is suspect when only the mailbox is. Documented as intentional
    // because it is the opposite choice from the two cursor faults sitting beside it.
    let verdict = MonitorVerdict::AuthoritiesDisagree {
        daemon_unread: 96,
        cli_unread: 20,
    };
    assert!(verdict.advances_cursor());
    assert_eq!(verdict.label(), "authorities_disagree");
    // Not a failure, so it carries no `detail`, and it must not be confused with one.
    assert_ne!(verdict.exit_code(), 13);
}

#[test]
fn y256_the_beads_own_premise_was_a_consuming_read() {
    // The bead recorded `MCP fetch_inbox (daemon) -> 0 unread, every row carries a read_ts`.
    // Re-measured with `mark_read:false`, the daemon reports 96 unread of 128 rows with 32
    // carrying a non-null `read_ts`. The refutation names its own cause: `fetch_inbox`'s
    // `mark_read` DEFAULTS TO TRUE, so a read that omits it consumes the unread state it was
    // measuring — and a second read then honestly reports zero with every returned row
    // stamped. That is why the daemon arm sets it explicitly, and why `InboxRequest` carries
    // the field with an explicit `false` default instead of inheriting the daemon's.
    //
    // This leg pins the numbers so a future convergence is a visible change and not a quiet
    // one. If the arms converge, UPDATE it with the new measurement — do not delete it.
    assert_ne!(
        MEASURED_DAEMON_UNREAD, 0,
        "the bead's premise of a zero daemon count is refuted, not inherited"
    );
    assert_ne!(MEASURED_DAEMON_UNREAD, MEASURED_CLI_UNREAD);
    // 32 read + 96 unread = 128 total: the arithmetic the non-mutating read returned.
    assert_eq!(32 + MEASURED_DAEMON_UNREAD, 128);
}
