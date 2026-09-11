#![forbid(unsafe_code)]

//! L4-SRC-MAIL: derive the Agent Mail source's `age_ms` from `am robot status`.
//!
//! # The two clocks
//!
//! An age is a DIFFERENCE OF TWO CLOCKS, so both are named here:
//!
//! * the **writer clock** — the `am` host's wall clock, stamped into
//!   `_meta.timestamp` at the moment `am robot status` rendered its envelope;
//! * the **observer clock** — `now_ms`, the Unix-epoch millisecond reading taken
//!   by the caller (`ompo start` / `ompo portal`) at the moment it consumed that
//!   envelope.
//!
//! `age_ms = observer − writer`. The observer clock is the "now" because the
//! liveness question being asked is "how stale is what I am holding *right
//! now*", and the observer is the only clock the orchestrator can read
//! synchronously. The two clocks are independent, therefore they can disagree:
//! a writer stamp in the observer's future yields a NEGATIVE difference. A
//! negative age is never emitted and never clamped to `0`; it is SILENT with
//! `L4_MAIL_CLOCK_SKEW`, because a clamp would launder a broken clock into the
//! freshest possible reading.
//!
//! # An absent timestamp is not age zero
//!
//! A missing `_meta`, a missing `_meta.timestamp`, or a timestamp that does not
//! parse is UNKNOWN, never `Some(0)`. `age_ms` stays `None` and the source is
//! SILENT, which [`crate::liveness::classify`] already treats as not-live. A
//! malformed envelope is a hard `Err`, not a silent source: a source that could
//! not even be read is an instrument failure, not an observation.
//!
//! The `_meta.timestamp` shape is the measured one — RFC3339 with an explicit
//! offset, e.g. `2026-09-11T18:05:39.843414+00:00`, confirmed live against the
//! installed `am robot status --json` on 2026-09-11.

use crate::liveness::SourceVerdict;

/// The liveness source name Agent Mail answers to in [`crate::liveness::classify`].
pub const MAIL_SOURCE_NAME: &str = "agent-mail";

/// Freshness of the Agent Mail source, as read from one `am robot status` envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailFreshness {
    /// Both clocks were readable and ordered; `age_ms = observer − writer`.
    Age {
        age_ms: u64,
        /// The writer clock reading, in Unix epoch milliseconds.
        writer_ms: i64,
    },
    /// A clock was unreadable or the two clocks disagreed. `age_ms` stays UNKNOWN.
    Silent { reason_code: String },
}

impl MailFreshness {
    /// The derived age, or `None` when the reading is UNKNOWN.
    ///
    /// Never `Some(0)` for an absent or unparsable timestamp.
    #[must_use]
    pub fn age_ms(&self) -> Option<u64> {
        match self {
            Self::Age { age_ms, .. } => Some(*age_ms),
            Self::Silent { .. } => None,
        }
    }

    #[must_use]
    pub fn is_silent(&self) -> bool {
        matches!(self, Self::Silent { .. })
    }

    /// `"SILENT"` or `"FRESH"` — the word the acceptance lane greps for.
    #[must_use]
    pub fn status(&self) -> &'static str {
        if self.is_silent() { "SILENT" } else { "FRESH" }
    }

    #[must_use]
    pub fn reason_code(&self) -> &str {
        match self {
            Self::Age { .. } => "L4_MAIL_FRESH",
            Self::Silent { reason_code } => reason_code,
        }
    }
}

/// Read the Agent Mail writer clock out of one `am robot status --json` envelope.
///
/// `now_ms` is the OBSERVER clock (Unix epoch milliseconds). Callers pass their
/// own `SystemTime::now()` reading; this function reads no clock itself so the
/// derivation stays pure and testable.
///
/// # Errors
///
/// Returns `Err` when the envelope is not JSON or is not a JSON object: an
/// unreadable instrument is a hard error, never a silent source and never age 0.
pub fn mail_freshness(status_json: &str, now_ms: i64) -> Result<MailFreshness, String> {
    let envelope: serde_json::Value = serde_json::from_str(status_json)
        .map_err(|error| format!("L4_MAIL_UNREADABLE_ENVELOPE — {error}"))?;
    if !envelope.is_object() {
        return Err("L4_MAIL_UNREADABLE_ENVELOPE — am robot status must be a JSON object".to_owned());
    }

    let Some(stamp) = envelope
        .get("_meta")
        .and_then(|meta| meta.get("timestamp"))
        .and_then(serde_json::Value::as_str)
    else {
        // ABSENT IS UNKNOWN, NOT ZERO.
        return Ok(MailFreshness::Silent {
            reason_code: "L4_MAIL_MISSING_META — am robot status carried no _meta.timestamp; \
                          age is UNKNOWN, not 0"
                .to_owned(),
        });
    };

    let Some(writer_ms) = rfc3339_to_epoch_ms(stamp) else {
        return Ok(MailFreshness::Silent {
            reason_code: format!(
                "L4_MAIL_UNPARSABLE_TIMESTAMP — _meta.timestamp={stamp:?} is not RFC3339; \
                 age is UNKNOWN, not 0"
            ),
        });
    };

    let delta = now_ms - writer_ms;
    if delta < 0 {
        // The writer clock is ahead of the observer clock. Emitting 0 here would
        // report the stalest possible disagreement as the freshest possible read.
        return Ok(MailFreshness::Silent {
            reason_code: format!(
                "L4_MAIL_CLOCK_SKEW — writer clock is {skew}ms ahead of the observer clock \
                 (writer_ms={writer_ms} observer_ms={now_ms}); age is UNKNOWN, not 0",
                skew = -delta
            ),
        });
    }

    #[allow(clippy::cast_sign_loss)] // delta >= 0 was just proven.
    Ok(MailFreshness::Age {
        age_ms: delta as u64,
        writer_ms,
    })
}

/// Build the Agent Mail [`SourceVerdict`] for [`crate::liveness::classify`].
///
/// A SILENT freshness reading leaves `age_ms` as `None` and `fresh` as `false`,
/// which `classify` already converts into `NOT_LIVE`.
///
/// # Errors
///
/// Propagates the hard-error case of [`mail_freshness`].
pub fn mail_source(
    status_json: &str,
    now_ms: i64,
    panes: Vec<String>,
) -> Result<SourceVerdict, String> {
    let freshness = mail_freshness(status_json, now_ms)?;
    Ok(SourceVerdict {
        name: MAIL_SOURCE_NAME.to_owned(),
        available: true,
        fresh: !freshness.is_silent(),
        reason_code: freshness.reason_code().to_owned(),
        age_ms: freshness.age_ms(),
        panes,
    })
}

/// Parse the measured `_meta.timestamp` shape into Unix epoch milliseconds.
///
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fraction](Z|z|±HH:MM|±HHMM)`. Returns `None`
/// for anything else — an unparsable stamp is UNKNOWN, never 0.
fn rfc3339_to_epoch_ms(stamp: &str) -> Option<i64> {
    let bytes = stamp.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let year: i64 = stamp.get(0..4)?.parse().ok()?;
    let month: i64 = stamp.get(5..7)?.parse().ok()?;
    let day: i64 = stamp.get(8..10)?.parse().ok()?;
    let hour: i64 = stamp.get(11..13)?.parse().ok()?;
    let minute: i64 = stamp.get(14..16)?.parse().ok()?;
    let second: i64 = stamp.get(17..19)?.parse().ok()?;
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if bytes[10] != b'T' && bytes[10] != b't' && bytes[10] != b' ' {
        return None;
    }
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let mut rest = stamp.get(19..)?;
    let mut millis_of_second = 0_i64;
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits: String = fraction.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        let mut scaled = digits.clone();
        scaled.truncate(3);
        while scaled.len() < 3 {
            scaled.push('0');
        }
        millis_of_second = scaled.parse().ok()?;
        rest = rest.get(1 + digits.len()..)?;
    }

    let offset_minutes = parse_offset(rest)?;
    let days = days_from_civil(year, month, day);
    let utc_seconds =
        days * 86_400 + hour * 3_600 + minute * 60 + second - offset_minutes * 60;
    Some(utc_seconds * 1_000 + millis_of_second)
}

/// Signed UTC offset in minutes. An offset is REQUIRED: a naive stamp has no
/// knowable instant, so it is UNKNOWN rather than assumed to be UTC.
fn parse_offset(rest: &str) -> Option<i64> {
    if rest == "Z" || rest == "z" {
        return Some(0);
    }
    let sign = match rest.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let body = rest.get(1..)?;
    let (hours, minutes) = match body.len() {
        5 if body.as_bytes()[2] == b':' => (body.get(0..2)?, body.get(3..5)?),
        4 => (body.get(0..2)?, body.get(2..4)?),
        _ => return None,
    };
    let hours: i64 = hours.parse().ok()?;
    let minutes: i64 = minutes.parse().ok()?;
    if hours > 23 || minutes > 59 {
        return None;
    }
    Some(sign * (hours * 60 + minutes))
}

/// Days since 1970-01-01 for a proleptic Gregorian civil date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_shifted = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shifted + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
